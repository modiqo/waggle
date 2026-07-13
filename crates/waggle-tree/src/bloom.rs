//! A fixed-size, union-composable Bloom filter over [`Trigram`]s.
//!
//! Each directory node in a tree carries one of these as a summary of *every*
//! trigram in its entire subtree. Search uses it as a prune gate: if the filter
//! says a query's trigrams are *definitely absent*, the whole subtree is skipped
//! without a blob fetch. That is what makes search over a deeply nested tree
//! sublinear — cost tracks the branches that could match, not the file count.
//!
//! Three properties earn its place here:
//!
//! * **Composable.** A parent's summary is the bitwise-OR of its children's
//!   ([`Bloom::union`]). So summaries build bottom-up in O(1) per node, and depth
//!   costs nothing.
//! * **Deterministic.** Fixed size, fixed hash count, a pinned seed — the same
//!   trigrams always yield the same bits, so a manifest carrying a Bloom stays
//!   byte-stable (invariant I-2).
//! * **Small.** [`Bloom::BYTES`] is 256, so it inlines in a node's manifest and a
//!   prune decision is a pure manifest read.
//!
//! It saturates gracefully: a subtree with more distinct trigrams than the filter
//! can hold answers "maybe" more often, never "no" wrongly (a Bloom has no false
//! negatives). So a huge root filter degrades to "descend", and the *smaller*
//! subtree filters below it do the real pruning.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::trigram::Trigram;

/// A fixed-size Bloom filter. Construct with [`Bloom::new`], fill with
/// [`Bloom::insert`], combine with [`Bloom::union`], test with
/// [`Bloom::might_contain`].
#[derive(Clone, PartialEq, Eq)]
pub struct Bloom {
    /// [`Bloom::BYTES`] bytes of bit storage. Boxed so a `Bloom` is a thin
    /// pointer rather than 256 bytes on the stack.
    bits: Box<[u8; Bloom::BYTES]>,
}

impl Bloom {
    /// Storage size in bytes. 256 B = 2048 bits — selective for a focused
    /// subtree (a few thousand distinct trigrams), small enough to inline.
    pub const BYTES: usize = 256;

    /// Bit count (`BYTES * 8`).
    pub const BITS: usize = Bloom::BYTES * 8;

    /// Hashes per element. Four keeps the false-positive rate low at the trigram
    /// densities real documents produce, without over-saturating the bits.
    pub const K: u32 = 4;

    /// Pinned seed for the base hash. Changing it changes every Bloom the system
    /// has ever written, so it is a wire-format constant, not a tunable.
    const SEED: u64 = 0x776167676c655f74; // "waggle_t"

    /// An empty filter — contains nothing, so [`Bloom::might_contain`] is always
    /// `false` until something is inserted.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bits: Box::new([0u8; Bloom::BYTES]),
        }
    }

    /// Record a trigram. Idempotent — inserting twice sets the same bits.
    pub fn insert(&mut self, tri: Trigram) {
        for pos in self.positions(tri) {
            self.bits[pos / 8] |= 1 << (pos % 8);
        }
    }

    /// Insert every trigram of `text`.
    pub fn insert_text(&mut self, text: &str) {
        for tri in Trigram::all(text) {
            self.insert(tri);
        }
    }

    /// `false` means the trigram is *definitely absent* — safe to prune.
    /// `true` means *possibly present* — the caller must look closer.
    #[must_use]
    pub fn might_contain(&self, tri: Trigram) -> bool {
        self.positions(tri)
            .into_iter()
            .all(|pos| self.bits[pos / 8] & (1 << (pos % 8)) != 0)
    }

    /// Could this subtree contain *all* of a query's trigrams? A grep pattern
    /// matches only where every one of its trigrams is present, so a subtree is
    /// prunable the moment any query trigram is definitely absent.
    #[must_use]
    pub fn might_contain_all(&self, query: &str) -> bool {
        Trigram::all(query).all(|tri| self.might_contain(tri))
    }

    /// Fold another filter in by bitwise-OR — the composition that lets a parent
    /// summarise its children in O(1).
    pub fn union(&mut self, other: &Bloom) {
        for (a, b) in self.bits.iter_mut().zip(other.bits.iter()) {
            *a |= *b;
        }
    }

    /// The fraction of bits set — a saturation gauge. Near 1.0 means the filter
    /// has stopped discriminating (a very large subtree) and will answer "maybe"
    /// for almost anything. Useful for diagnostics, never for correctness.
    #[must_use]
    pub fn saturation(&self) -> f32 {
        let set: u32 = self.bits.iter().map(|b| b.count_ones()).sum();
        set as f32 / Bloom::BITS as f32
    }

    /// The `K` bit positions for a trigram, via double hashing: two independent
    /// hashes `h1`, `h2` generate `h1 + i*h2` for `i in 0..K`. Standard, and it
    /// avoids `K` separate hash passes.
    fn positions(&self, tri: Trigram) -> [usize; Bloom::K as usize] {
        let bytes = tri.bytes();
        let h1 = fnv1a(Bloom::SEED, &bytes);
        let h2 = fnv1a(Bloom::SEED ^ 0x9e37_79b9_7f4a_7c15, &bytes) | 1; // odd → full period
        let mut out = [0usize; Bloom::K as usize];
        for (i, slot) in out.iter_mut().enumerate() {
            let h = h1.wrapping_add((i as u64).wrapping_mul(h2));
            *slot = (h % Bloom::BITS as u64) as usize;
        }
        out
    }

    /// Lowercase-hex of the bit storage — the form inlined in a manifest. Pairs
    /// with [`Bloom::from_hex`].
    #[must_use]
    pub fn to_hex(&self) -> String {
        to_hex(self.bits.as_ref())
    }

    /// Parse the hex form back into a filter. Errors if the string is not exactly
    /// [`Bloom::BYTES`] bytes of hex.
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let bytes = from_hex(hex)?;
        let arr: [u8; Bloom::BYTES] = bytes.try_into().map_err(|v: Vec<u8>| {
            format!("bloom: expected {} bytes, got {}", Bloom::BYTES, v.len())
        })?;
        Ok(Self {
            bits: Box::new(arr),
        })
    }
}

impl Default for Bloom {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Bloom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bloom({:.0}% set)", self.saturation() * 100.0)
    }
}

/// FNV-1a, 64-bit. Deterministic and dependency-free — exactly what a
/// wire-stable, seed-pinned filter needs.
fn fnv1a(seed: u64, data: &[u8]) -> u64 {
    let mut h = seed ^ 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// --- serde: a compact lowercase-hex string, so a manifest stays readable and
// byte-stable rather than carrying a 256-element number array. ---

impl Serialize for Bloom {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&to_hex(self.bits.as_ref()))
    }
}

impl<'de> Deserialize<'de> for Bloom {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        let bytes = from_hex(&hex).map_err(serde::de::Error::custom)?;
        let arr: [u8; Bloom::BYTES] = bytes.try_into().map_err(|v: Vec<u8>| {
            serde::de::Error::custom(format!(
                "bloom: expected {} bytes, got {}",
                Bloom::BYTES,
                v.len()
            ))
        })?;
        Ok(Self {
            bits: Box::new(arr),
        })
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("bloom hex: odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("bloom hex: {e}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri(a: u8, b: u8, c: u8) -> Trigram {
        Trigram::from_bytes([a, b, c])
    }

    #[test]
    fn absent_is_definitely_absent() {
        let b = Bloom::new();
        assert!(!b.might_contain(tri(b'a', b'b', b'c')));
    }

    #[test]
    fn inserted_is_possibly_present() {
        let mut b = Bloom::new();
        b.insert(tri(b'a', b'b', b'c'));
        assert!(b.might_contain(tri(b'a', b'b', b'c')));
    }

    #[test]
    fn union_is_bitwise_or_and_preserves_membership() {
        let mut a = Bloom::new();
        a.insert(tri(b'x', b'y', b'z'));
        let mut b = Bloom::new();
        b.insert(tri(b'1', b'2', b'3'));
        a.union(&b);
        assert!(a.might_contain(tri(b'x', b'y', b'z')));
        assert!(a.might_contain(tri(b'1', b'2', b'3')));
    }

    #[test]
    fn might_contain_all_prunes_on_any_missing_trigram() {
        let mut b = Bloom::new();
        b.insert_text("retry budget");
        // "retry budget" trigrams are present…
        assert!(b.might_contain_all("retry"));
        // …but a word that shares no trigram is prunable.
        assert!(!b.might_contain_all("zzzzzz"));
    }

    #[test]
    fn deterministic_across_instances() {
        let mut a = Bloom::new();
        let mut b = Bloom::new();
        a.insert_text("the escalation policy");
        b.insert_text("the escalation policy");
        assert_eq!(a, b);
    }

    #[test]
    fn serde_round_trips_through_hex() {
        let mut b = Bloom::new();
        b.insert_text("runbook 07 violates the ceiling");
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.starts_with('"') && json.len() == Bloom::BYTES * 2 + 2);
        let back: Bloom = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn no_false_negatives_over_many_inserts() {
        let mut b = Bloom::new();
        let words: Vec<String> = (0..500).map(|i| format!("token{i:04}")).collect();
        for w in &words {
            b.insert_text(w);
        }
        for w in &words {
            assert!(b.might_contain_all(w), "false negative for {w}");
        }
    }
}
