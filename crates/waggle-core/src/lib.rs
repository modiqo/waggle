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
//! CP-0 shipped the foundation trio: [`Token`], [`Timestamp`], [`Entropy`].
//! CP-1 adds the domain model: slugs ([`Sharer`], [`Channel`], [`Stage`]),
//! targets ([`CanonicalUrl`], [`TargetMeta`], [`MediaRef`]), the
//! three-zone [`AttributionManifest`] with variants, and [`mint`] — a pure
//! function of `(spec, options, entropy, now)`. The sealed variant matcher
//! and folds land in CP-2/CP-3 (design docs `02`–`04`).
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

mod context;
mod entropy;
mod event;
mod fold;
mod log;
mod manifest;
mod matcher;
mod mint;
mod reconstruct;
mod resolve;
mod slug;
mod soa;
mod target;
mod time;
mod token;
pub mod trust;

pub use context::{negotiate, ConsumerHint, ConsumerKind, ResolverContext};
pub use entropy::{Entropy, EntropyError};
pub use event::{ActorClass, Event, FamilyClass, HarnessClass, Seq};
pub use fold::{replay, Fold, FunnelFold, LineageFold, ManifestFold};
pub use log::{Change, LogRecord};
pub use manifest::{
    apply_change, AttributionManifest, Constraint, Disposition, MatchExpr, ModalitySet, Posture,
    SignatureBlock, Variant, VariantBody, MANIFEST_SCHEMA_VERSION,
};
pub use matcher::{select_variant, Selected};
pub use mint::{mint, MintError, MintOptions, MintSpec};
pub use reconstruct::{apply_suffix, reconstruct, WorldState};
pub use resolve::{resolve, Resolution, DEFAULT_REVALIDATE_MS};
pub use slug::{Channel, Sharer, SlugError, Stage};
pub use soa::{EventLog, InternTables, StageId, TokenId};
pub use target::{
    CanonicalUrl, MediaRef, Sha256Error, Sha256Hex, TargetError, TargetMeta,
    INLINE_THRESHOLD_BYTES, MANIFEST_SIZE_CAP_BYTES,
};
pub use time::Timestamp;
pub use token::{Token, TokenError, TOKEN_ALPHABET};
