//! Trigrams and the inverted index built from them.
//!
//! A [`Trigram`] is a 3-byte, case-folded shingle of text — the atom of the
//! search index. A pattern can only match a file where *every* trigram of the
//! pattern is present, so a trigram index turns "grep this pattern across N
//! files" into "look up a few short posting lists and confirm the survivors."
//! This is the technique code-search engines (Zoekt, ripgrep's `--pre`, Google
//! Code Search) use to stay sublinear.
//!
//! Everything here is a pure function of the bytes: the same content yields the
//! same index, so an index blob is content-addressable and byte-stable (I-2).
//! Case folding is ASCII-only and deterministic; a query is folded the same way,
//! so matching is case-insensitive by construction.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A case-folded 3-byte shingle. Ordered and hashable so it can key a map.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Trigram([u8; 3]);

impl Trigram {
    /// Build directly from three raw bytes (no folding). Mainly for tests and
    /// callers that already hold folded bytes.
    #[must_use]
    pub fn from_bytes(b: [u8; 3]) -> Self {
        Self(b)
    }

    /// The underlying bytes — the [`crate::bloom`] hasher consumes these.
    #[must_use]
    pub fn bytes(self) -> [u8; 3] {
        self.0
    }

    /// Every trigram of `text`, in order, over its **case-folded** bytes. Text
    /// shorter than three bytes yields nothing (nothing to shingle). Callers get
    /// an iterator so a large document never materialises a trigram `Vec`.
    pub fn all(text: &str) -> impl Iterator<Item = Trigram> + '_ {
        let folded: Vec<u8> = text.bytes().map(fold).collect();
        (0..folded.len().saturating_sub(2))
            .map(move |i| Trigram([folded[i], folded[i + 1], folded[i + 2]]))
    }
}

/// ASCII lowercasing — the one folding rule, applied identically to indexed
/// content and to queries. Non-ASCII bytes pass through unchanged.
fn fold(b: u8) -> u8 {
    b.to_ascii_lowercase()
}

/// Which documents a query's trigrams point at. `File(id)` indexes into the
/// caller's own document list (a directory node's file entries).
pub type DocId = u32;

/// An inverted index: trigram → the sorted, de-duplicated documents that contain
/// it. Built with [`TrigramIndex::builder`]; queried with
/// [`TrigramIndex::candidates`].
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TrigramIndex {
    /// Serialised as a plain map so an index blob is inspectable. Keys are the
    /// three folded bytes as a lossy string is *not* safe (bytes may not be
    /// UTF-8), so we key by the raw triple via a helper representation.
    #[serde(with = "postings_serde")]
    postings: BTreeMap<Trigram, Vec<DocId>>,
    /// Number of documents the index was built over — lets a reader sanity-check
    /// candidate ids without holding the document list.
    docs: u32,
}

impl TrigramIndex {
    /// Start building. Feed each document's text with
    /// [`TrigramIndexBuilder::add`] in id order.
    #[must_use]
    pub fn builder() -> TrigramIndexBuilder {
        TrigramIndexBuilder {
            postings: BTreeMap::new(),
            next: 0,
        }
    }

    /// The documents that *could* match `query`: those carrying **all** of the
    /// query's trigrams (posting-list intersection). Empty query trigrams (text
    /// under three bytes) returns every document, because a trigram index cannot
    /// narrow a pattern too short to shingle — the caller falls back to a full
    /// grep of the small candidate set.
    #[must_use]
    pub fn candidates(&self, query: &str) -> Vec<DocId> {
        let grams: Vec<Trigram> = Trigram::all(query).collect();
        if grams.is_empty() {
            return (0..self.docs).collect();
        }
        // Intersect posting lists, smallest first so the running set only shrinks.
        let mut lists: Vec<&Vec<DocId>> = Vec::with_capacity(grams.len());
        for g in &grams {
            match self.postings.get(g) {
                Some(list) => lists.push(list),
                None => return Vec::new(), // a trigram absent everywhere ⇒ no match
            }
        }
        lists.sort_by_key(|l| l.len());
        let mut acc = lists[0].clone();
        for list in &lists[1..] {
            acc = intersect_sorted(&acc, list);
            if acc.is_empty() {
                break;
            }
        }
        acc
    }

    /// Documents indexed.
    #[must_use]
    pub fn doc_count(&self) -> u32 {
        self.docs
    }

    /// Distinct trigrams — an index-size gauge.
    #[must_use]
    pub fn trigram_count(&self) -> usize {
        self.postings.len()
    }
}

/// Accumulates postings one document at a time. Document ids are assigned in the
/// order documents are added, starting at 0.
pub struct TrigramIndexBuilder {
    postings: BTreeMap<Trigram, Vec<DocId>>,
    next: DocId,
}

impl TrigramIndexBuilder {
    /// Index one document's text and return its assigned id.
    pub fn add(&mut self, text: &str) -> DocId {
        let id = self.next;
        // De-dup trigrams within a doc so a posting list holds each id once.
        let mut seen: BTreeMap<Trigram, ()> = BTreeMap::new();
        for tri in Trigram::all(text) {
            if seen.insert(tri, ()).is_none() {
                self.postings.entry(tri).or_default().push(id);
            }
        }
        self.next += 1;
        id
    }

    /// Finish. Posting lists are already sorted, because ids are added in order.
    #[must_use]
    pub fn build(self) -> TrigramIndex {
        TrigramIndex {
            postings: self.postings,
            docs: self.next,
        }
    }
}

/// Intersection of two ascending, de-duplicated id lists.
fn intersect_sorted(a: &[DocId], b: &[DocId]) -> Vec<DocId> {
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

/// Serialise the posting map with hex trigram keys, so the wire form is valid
/// JSON regardless of whether the folded bytes are printable.
mod postings_serde {
    use super::{DocId, Trigram};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<Trigram, Vec<DocId>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let hexed: BTreeMap<String, &Vec<DocId>> =
            map.iter().map(|(k, v)| (hex3(k.bytes()), v)).collect();
        hexed.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<Trigram, Vec<DocId>>, D::Error> {
        let hexed: BTreeMap<String, Vec<DocId>> = BTreeMap::deserialize(d)?;
        let mut out = BTreeMap::new();
        for (k, v) in hexed {
            let b = unhex3(&k).map_err(serde::de::Error::custom)?;
            out.insert(Trigram::from_bytes(b), v);
        }
        Ok(out)
    }

    fn hex3(b: [u8; 3]) -> String {
        let mut s = String::with_capacity(6);
        for byte in b {
            s.push(char::from_digit((byte >> 4) as u32, 16).unwrap());
            s.push(char::from_digit((byte & 0xf) as u32, 16).unwrap());
        }
        s
    }

    fn unhex3(s: &str) -> Result<[u8; 3], String> {
        if s.len() != 6 {
            return Err(format!(
                "trigram key: expected 6 hex chars, got {}",
                s.len()
            ));
        }
        let mut b = [0u8; 3];
        for (i, slot) in b.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|e| format!("trigram key: {e}"))?;
        }
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shingles_are_case_folded() {
        let up: Vec<_> = Trigram::all("ABC").collect();
        let lo: Vec<_> = Trigram::all("abc").collect();
        assert_eq!(up, lo);
        assert_eq!(up.len(), 1);
    }

    #[test]
    fn short_text_yields_no_trigrams() {
        assert_eq!(Trigram::all("ab").count(), 0);
        assert_eq!(Trigram::all("").count(), 0);
    }

    #[test]
    fn candidates_intersect_all_query_trigrams() {
        let mut b = TrigramIndex::builder();
        let d0 = b.add("the retry budget is three"); // has "retry"
        let d1 = b.add("the escalation policy ceiling"); // no "retry"
        let _d2 = b.add("retry retry retry");
        let idx = b.build();
        let mut hits = idx.candidates("retry");
        hits.sort_unstable();
        assert_eq!(hits, vec![d0, 2]);
        assert!(!hits.contains(&d1));
    }

    #[test]
    fn absent_pattern_returns_nothing() {
        let mut b = TrigramIndex::builder();
        b.add("alpha beta gamma");
        let idx = b.build();
        assert!(idx.candidates("zzzzzz").is_empty());
    }

    #[test]
    fn too_short_query_returns_all_docs() {
        let mut b = TrigramIndex::builder();
        b.add("one");
        b.add("two");
        let idx = b.build();
        assert_eq!(idx.candidates("x"), vec![0, 1]);
    }

    #[test]
    fn serde_round_trip_preserves_queries() {
        let mut b = TrigramIndex::builder();
        b.add("runbook 07 violates the ceiling");
        b.add("runbook 03 is within budget");
        let idx = b.build();
        let json = serde_json::to_string(&idx).unwrap();
        let back: TrigramIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(idx.candidates("violates"), back.candidates("violates"));
        assert_eq!(back.doc_count(), 2);
    }

    #[test]
    fn deterministic_index_bytes() {
        let build = || {
            let mut b = TrigramIndex::builder();
            b.add("the escalation policy");
            b.add("a retry budget of three");
            serde_json::to_string(&b.build()).unwrap()
        };
        assert_eq!(build(), build());
    }
}
