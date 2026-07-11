//! Byte → token accounting.
//!
//! The benchmark's headline (design doc `22 §2.1`) is a *ratio* of costs,
//! and the tokenizer cancels in a ratio — so the crossover and asymptote
//! are tokenizer-invariant. The default here is therefore a documented
//! character ratio rather than a vendored BPE table (which would need a
//! network fetch and pin the result to one vendor). A real BPE can be
//! swapped in behind this trait without changing the conclusions.

/// A byte → token estimator.
pub(crate) trait Tokenizer {
    /// Estimated tokens for `bytes` bytes of artifact text.
    fn tokens(&self, bytes: usize) -> f64;
    /// A short, citable label recorded in every output file.
    fn label(&self) -> &str;
}

/// A fixed bytes-per-token ratio. `4.0` is the widely cited English-text
/// approximation for GPT-class BPEs; it is stated in every emitted file so
/// a reader can substitute their own and recompute.
pub(crate) struct CharRatio {
    bytes_per_token: f64,
    label: String,
}

impl CharRatio {
    /// The default English-text ratio (≈4 bytes/token).
    pub(crate) fn english() -> Self {
        Self {
            bytes_per_token: 4.0,
            label: "char-ratio/4.0".to_owned(),
        }
    }
}

impl Tokenizer for CharRatio {
    fn tokens(&self, bytes: usize) -> f64 {
        bytes as f64 / self.bytes_per_token
    }
    fn label(&self) -> &str {
        &self.label
    }
}
