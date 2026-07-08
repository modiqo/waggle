//! Entropy injection — randomness as a parameter, not an ambient source.

use thiserror::Error;

/// Failure of an entropy source.
///
/// Carries the host's own message; the core never inspects it beyond
/// propagation (the caller chose the source, the caller understands its
/// failures).
#[derive(Debug, Error)]
#[error("entropy source failed: {0}")]
pub struct EntropyError(pub String);

/// A source of cryptographically secure random bytes.
///
/// Blanket-implemented for closures so hosts pass a function, not a global:
/// natively `|b| getrandom::getrandom(b).map_err(...)`, in Workers the JS
/// crypto equivalent, in tests a counter. Upholds the sans-I/O law
/// (design doc `03 §1`).
pub trait Entropy {
    /// Fill `buf` entirely with random bytes, or fail.
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError>;
}

impl<F> Entropy for F
where
    F: FnMut(&mut [u8]) -> Result<(), EntropyError>,
{
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closures_are_entropy_sources() {
        let mut calls = 0usize;
        let mut src = |buf: &mut [u8]| {
            calls += 1;
            buf.fill(0xA5);
            Ok(())
        };
        let mut buf = [0u8; 4];
        src.fill(&mut buf).unwrap();
        assert_eq!(buf, [0xA5; 4]);
        assert_eq!(calls, 1);
    }

    #[test]
    fn failures_propagate() {
        let mut src = |_: &mut [u8]| Err(EntropyError("no randomness today".into()));
        let mut buf = [0u8; 1];
        let err = src.fill(&mut buf).unwrap_err();
        assert!(err.to_string().contains("no randomness"));
    }
}
