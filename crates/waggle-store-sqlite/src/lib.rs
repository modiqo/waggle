//! # waggle-store-sqlite — the production laptop store
//!
//! `SQLite` in WAL mode as the correctness anchor (design docs `07 §4`,
//! `13 §8`): every append is one transaction carrying seq assignment
//! (C-3), nonce dedupe via a UNIQUE index (C-8), CAS via
//! `UPDATE … WHERE version = ?` (C-9), and the revoked-parent check (C-7).
//! WAL gives many readers that never block the single writer — the
//! multi-read/multi-write model provided by construction rather than
//! built (15 §4).
//!
//! A read-through manifest cache (`RwLock`; the `perf` arc-swap upgrade is
//! design doc `13 §7`) accelerates the hot resolve path — a cache over the
//! anchor, invalidated in-commit, never the correctness mechanism.
//!
//! The blob CAS sidecar ([`BlobStore`], rev 2.3) lives beside the
//! database: bytes named by SHA-256, atomic writes, free dedupe, verified
//! reads, mark-and-sweep GC. Parquet compaction and the loom-checked cache
//! layer are the remaining CP-5 tail (tracked in `docs/design/14`).

mod blobs;
mod schema;
mod store;

pub use blobs::BlobStore;
pub use store::SqliteStore;
