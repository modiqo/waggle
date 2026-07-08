//! # waggle-core — the sans-I/O domain
//!
//! The core performs **no I/O, owns no clock, and generates no entropy**
//! (design doc `03 §1`). Every effect is a parameter:
//!
//! - randomness arrives through [`Entropy`] (blanket-implemented for
//!   closures — function passing, not global state),
//! - time arrives as a [`Timestamp`] value in every signature that needs
//!   one,
//! - storage never appears here at all (see `waggle-store`).
//!
//! This is what lets the identical code run in the native daemon, in
//! Cloudflare Workers wasm, and under deterministic tests.
//!
//! CP-0 ships the foundation trio: [`Token`], [`Timestamp`], [`Entropy`].
//! Manifests, variants, resolution, and folds land in CP-1..CP-3
//! (design docs `02`–`04`).
//!
//! ```
//! use waggle_core::{Entropy, Token};
//!
//! // A deterministic entropy source: fine for tests, never for production.
//! let mut counter = 0u8;
//! let mut entropy = |buf: &mut [u8]| {
//!     for b in buf.iter_mut() {
//!         counter = counter.wrapping_add(41);
//!         *b = counter;
//!     }
//!     Ok(())
//! };
//! let token = Token::generate(8, &mut entropy).expect("entropy never fails here");
//! assert_eq!(token.as_str().len(), 8);
//! ```

mod entropy;
mod time;
mod token;

pub use entropy::{Entropy, EntropyError};
pub use time::Timestamp;
pub use token::{Token, TokenError, TOKEN_ALPHABET};
