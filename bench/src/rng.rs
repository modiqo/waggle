//! A tiny deterministic PRNG (linear congruential). No external dependency
//! and seed-stable, so every benchmark reproduces exactly on any machine.

/// A seeded LCG. Not cryptographic — a reproducible stream for shuffling
/// and Bernoulli draws in the harness.
pub(crate) struct Lcg(u64);

impl Lcg {
    /// Seed the stream.
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Next 64-bit value.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// A value in `0..n`.
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() >> 33) as usize % n
    }

    /// A Bernoulli draw: `true` with probability `p` (clamped to `[0,1]`).
    pub(crate) fn chance(&mut self, p: f64) -> bool {
        // 53-bit uniform in [0,1).
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        u < p
    }
}
