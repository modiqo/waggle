//! # waggle-store — the storage contract
//!
//! The log-shaped trait family every backend implements (design doc `07`),
//! with the supertrait split that makes read-only paths **type-enforced**:
//! a function taking `&impl ReadStore` cannot append — invariant I-4 as a
//! bound, checked by a `compile_fail` doctest below.
//!
//! Stores assign sequence numbers (C-3), dedupe mint nonces (C-8), and
//! CAS-check lifecycle mutations (C-9) at their single commit point; the
//! [`conformance`] suite is the contract's teeth — a backend without a
//! green conformance run is not a waggle backend (`07 §5`).
//!
//! Async is native (`async fn` in traits) and deliberately **`?Send`** —
//! Cloudflare Workers futures aren't `Send`, and forcing `Send` here would
//! exile the edge backend. Native hosts that need `Send` wrap at their own
//! boundary.
//!
//! ```compile_fail
//! // I-4 by bound: a ReadStore consumer cannot append.
//! fn funnel_only(store: &impl waggle_store::ReadStore) {
//!     let _ = store.append(todo!());
//! }
//! ```
//!
//! ```
//! use waggle_store::{AppendIntent, AppendStore, MemoryStore, MintNonce, ReadStore};
//! # pollster::block_on(async {
//! let store = MemoryStore::default();
//! # let mut entropy = |b: &mut [u8]| { b.fill(7); Ok(()) };
//! # let manifest = waggle_core::mint(
//! #     waggle_core::MintSpec::new(
//! #         waggle_core::CanonicalUrl::new("ws://a/b").unwrap(),
//! #         waggle_core::Sharer::new("lead").unwrap(),
//! #         waggle_core::Channel::subagent_general(),
//! #     ),
//! #     &waggle_core::MintOptions::default(), &mut entropy,
//! #     waggle_core::Timestamp::from_unix_ms(1),
//! # ).unwrap();
//! let token = manifest.token;
//! store.append(AppendIntent::Mint { manifest: Box::new(manifest), nonce: MintNonce(1) }).await.unwrap();
//! assert!(store.manifest(token).await.unwrap().is_some()); // C-6: read-your-mint
//! # });
//! ```

#![allow(async_fn_in_trait)] // ?Send by design (Workers) — documented above.

mod error;
mod memory;
mod traits;
mod types;

pub mod conformance;

pub use error::StoreError;
pub use memory::MemoryStore;
pub use traits::{AppendStore, BlobSink, NoBlobs, ReadStore, Store};
pub use types::{AppendIntent, Appended, ManifestView, MintNonce};
