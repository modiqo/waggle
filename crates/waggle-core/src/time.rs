//! Time as a value. The core never asks a clock (design doc `03 §1`) —
//! callers pass `now` explicitly, which is also what makes replay (doc `04`)
//! trivially deterministic.

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch, as a value.
///
/// There is deliberately no `Timestamp::now()`: the host supplies time
/// (natively from `SystemTime`, in Workers from `Date.now()`, in tests as a
/// constant). Ordering and arithmetic are the only operations the domain
/// needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Wrap a unix-epoch-milliseconds value.
    #[must_use]
    pub const fn from_unix_ms(ms: u64) -> Self {
        Self(ms)
    }

    /// The raw unix-epoch-milliseconds value.
    #[must_use]
    pub const fn as_unix_ms(self) -> u64 {
        self.0
    }

    /// This timestamp advanced by `ms` milliseconds (saturating).
    #[must_use]
    pub const fn plus_ms(self, ms: u64) -> Self {
        Self(self.0.saturating_add(ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_is_chronological() {
        let earlier = Timestamp::from_unix_ms(1_000);
        let later = Timestamp::from_unix_ms(2_000);
        assert!(earlier < later);
        assert_eq!(earlier.plus_ms(1_000), later);
    }

    #[test]
    fn plus_saturates_instead_of_wrapping() {
        let max = Timestamp::from_unix_ms(u64::MAX);
        assert_eq!(max.plus_ms(5), max);
    }

    #[test]
    fn serde_is_transparent() {
        let ts = Timestamp::from_unix_ms(42);
        let json = serde_json::to_string(&ts).unwrap();
        assert_eq!(json, "42");
        let back: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ts);
    }
}
