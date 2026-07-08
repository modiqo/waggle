//! The token: a short, non-enumerable name for one act of distribution.
//!
//! Design commitments (docs `02 §2`, `03 §4`): inline storage (24 bytes,
//! `Copy`, zero heap — two tokens fit a cache line in maps), the Bitcoin
//! base58 alphabet (no `0OIl` ambiguity — tokens get read aloud and typed),
//! and **rejection-sampled** generation so no alphabet position is favored
//! (modulo bias is how "non-enumerable" quietly stops being true).

use core::fmt;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::entropy::Entropy;

/// The Bitcoin base58 alphabet: 58 symbols, no `0`, `O`, `I`, or `l`.
pub const TOKEN_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Largest byte value usable without modulo bias: `232 = 58 * 4`.
const REJECTION_BOUND: u8 = 232;

const MAX_LEN: usize = 23;
const DEFAULT_ALLOC: usize = 64;

/// Why a token could not be parsed or generated.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    /// Length outside `1..=23` characters.
    #[error("token length {0} outside 1..=23")]
    Length(usize),
    /// A character outside the base58 alphabet.
    #[error("token contains a character outside the base58 alphabet")]
    Alphabet,
    /// The entropy source failed while generating.
    #[error("entropy source failed while generating a token: {0}")]
    Entropy(String),
}

/// A waggle token: inline, `Copy`, 24 bytes total.
///
/// Tokens are *names*, not data — comparison, hashing, and display are the
/// whole interface. Construction is either [`Token::parse`] (validated) or
/// [`Token::generate`] (rejection-sampled from an [`Entropy`] source).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token {
    len: u8,
    buf: [u8; MAX_LEN],
}

impl Token {
    /// Parse and validate an existing token string.
    pub fn parse(s: &str) -> Result<Self, TokenError> {
        let bytes = s.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_LEN {
            return Err(TokenError::Length(bytes.len()));
        }
        if !bytes.iter().all(|b| TOKEN_ALPHABET.contains(b)) {
            return Err(TokenError::Alphabet);
        }
        let mut buf = [0u8; MAX_LEN];
        buf[..bytes.len()].copy_from_slice(bytes);
        #[allow(clippy::cast_possible_truncation)] // bytes.len() <= 23
        Ok(Self {
            len: bytes.len() as u8,
            buf,
        })
    }

    /// Generate a fresh token of `len` characters from `entropy`.
    ///
    /// Rejection sampling: bytes ≥ 232 are discarded rather than folded,
    /// so every alphabet symbol is exactly equally likely.
    pub fn generate(len: usize, entropy: &mut impl Entropy) -> Result<Self, TokenError> {
        if len == 0 || len > MAX_LEN {
            return Err(TokenError::Length(len));
        }
        let mut buf = [0u8; MAX_LEN];
        let mut filled = 0usize;
        let mut pool = [0u8; DEFAULT_ALLOC];
        while filled < len {
            entropy
                .fill(&mut pool)
                .map_err(|e| TokenError::Entropy(e.to_string()))?;
            for &b in &pool {
                if b < REJECTION_BOUND {
                    buf[filled] = TOKEN_ALPHABET[(b % 58) as usize];
                    filled += 1;
                    if filled == len {
                        break;
                    }
                }
            }
        }
        #[allow(clippy::cast_possible_truncation)] // len <= 23
        Ok(Self {
            len: len as u8,
            buf,
        })
    }

    /// The token as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Invariant: buf[..len] came from TOKEN_ALPHABET (pure ASCII), so
        // this cannot fail; unsafe from_utf8_unchecked is not worth the
        // audit burden for a cold path.
        core::str::from_utf8(&self.buf[..self.len as usize]).unwrap_or("")
    }
}

impl PartialOrd for Token {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Token {
    /// Lexicographic by string form — deterministic map ordering is what
    /// R-1's byte-identical serialization rests on.
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token({})", self.as_str())
    }
}

impl Serialize for Token {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Token {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A visitor, not <&str>::deserialize: `serde_json::from_value`
        // (the edge's store RPC) cannot lend borrowed strings — the
        // borrowed-only form worked everywhere until the first consumer
        // that couldn't borrow.
        struct TokenVisitor;
        impl de::Visitor<'_> for TokenVisitor {
            type Value = Token;
            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("a waggle token string")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Token, E> {
                Token::parse(s).map_err(de::Error::custom)
            }
        }
        deserializer.deserialize_str(TokenVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counting_entropy() -> impl FnMut(&mut [u8]) -> Result<(), crate::EntropyError> {
        let mut state = 0u8;
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                state = state.wrapping_add(37);
                *b = state;
            }
            Ok(())
        }
    }

    fn assert_copy<T: Copy>() {}

    #[test]
    fn size_is_inline_and_copy() {
        assert_eq!(core::mem::size_of::<Token>(), 24);
        assert_copy::<Token>();
    }

    #[test]
    fn generate_parse_roundtrip() {
        let mut entropy = counting_entropy();
        let token = Token::generate(8, &mut entropy).unwrap();
        assert_eq!(token.as_str().len(), 8);
        let reparsed = Token::parse(token.as_str()).unwrap();
        assert_eq!(reparsed, token);
    }

    #[test]
    fn parse_rejects_bad_lengths_and_alphabet() {
        assert_eq!(Token::parse(""), Err(TokenError::Length(0)));
        assert_eq!(
            Token::parse("aaaaaaaaaaaaaaaaaaaaaaaa"), // 24 chars
            Err(TokenError::Length(24))
        );
        assert_eq!(Token::parse("abc0def"), Err(TokenError::Alphabet)); // '0' excluded
        assert_eq!(Token::parse("abcOdef"), Err(TokenError::Alphabet)); // 'O' excluded
        assert_eq!(Token::parse("abc def"), Err(TokenError::Alphabet));
    }

    #[test]
    fn generate_bounds_are_enforced() {
        let mut entropy = counting_entropy();
        assert_eq!(Token::generate(0, &mut entropy), Err(TokenError::Length(0)));
        assert_eq!(
            Token::generate(24, &mut entropy),
            Err(TokenError::Length(24))
        );
        assert!(Token::generate(MAX_LEN, &mut entropy).is_ok());
    }

    #[test]
    fn rejection_sampling_skips_biased_bytes() {
        // A source that emits one over-bound byte (would alias symbol 0 if
        // folded) then a valid byte: the token must use only the valid one.
        let mut phase = 0usize;
        let mut entropy = move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                *b = if phase % 2 == 0 { 250 } else { 1 };
                phase += 1;
            }
            Ok(())
        };
        let token = Token::generate(4, &mut entropy).unwrap();
        // byte value 1 -> alphabet index 1 -> '2'
        assert_eq!(token.as_str(), "2222");
    }

    #[test]
    fn uniform_distribution_smoke() {
        // Not a statistical proof (that's a CP-1 property test) — a tripwire
        // that every alphabet bucket is hit under a spread-out source.
        // xorshift32: deterministic, but covers the byte range evenly —
        // unlike the fixed-stride counter, whose consumed positions cycle.
        let mut state = 0x2026_0707_u32;
        let mut entropy = move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *b = (state & 0xFF) as u8;
            }
            Ok(())
        };
        let mut seen = [0u32; 58];
        for _ in 0..2_000 {
            let t = Token::generate(8, &mut entropy).unwrap();
            for b in t.as_str().bytes() {
                let idx = TOKEN_ALPHABET.iter().position(|&a| a == b).unwrap();
                seen[idx] += 1;
            }
        }
        assert!(seen.iter().all(|&c| c > 0), "alphabet bucket never hit");
    }

    #[test]
    fn entropy_failure_surfaces() {
        let mut broken = |_: &mut [u8]| Err(crate::EntropyError("dead".into()));
        let err = Token::generate(8, &mut broken).unwrap_err();
        assert!(matches!(err, TokenError::Entropy(_)));
    }

    #[test]
    fn serde_roundtrip_and_validation() {
        let mut entropy = counting_entropy();
        let token = Token::generate(8, &mut entropy).unwrap();
        let json = serde_json::to_string(&token).unwrap();
        let back: Token = serde_json::from_str(&json).unwrap();
        assert_eq!(back, token);
        assert!(serde_json::from_str::<Token>("\"bad token!\"").is_err());
    }
}
