//! Store errors: every variant is actionable (design doc `09 §5`), and
//! error text names the fix (doc `17 §2`'s hint discipline applied at the
//! error source).

use thiserror::Error;
use waggle_core::Token;

/// Why a store operation failed.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The token has no record in the system of record (C-10: only after
    /// an authoritative check — a cache miss must never produce this).
    #[error("unknown token {0}")]
    UnknownToken(Token),
    /// CAS mismatch on a lifecycle mutation (C-9): re-read the manifest,
    /// re-decide, retry with the current version.
    #[error(
        "version conflict on {token}: expected {expected}, current {actual} — re-read and retry"
    )]
    Conflict {
        /// The token whose manifest was contested.
        token: Token,
        /// The version the caller decided against.
        expected: u32,
        /// The version actually current.
        actual: u32,
    },
    /// Lifecycle mutations require `expected_version` (C-9) — pass the
    /// version from the manifest you read.
    #[error("lifecycle change on {0} requires expected_version (CAS) — read the manifest first")]
    LifecycleRequiresVersion(Token),
    /// The parent of a `mint_child` is revoked (C-7): the delegation tree
    /// is tombstoned; mint from a live token instead.
    #[error("parent {0} is revoked — children cannot be minted under a tombstone")]
    ParentRevoked(Token),
    /// The declared parent does not exist in this store.
    #[error("parent {0} is unknown to this store")]
    ParentUnknown(Token),
    /// Backend-specific failure (I/O, lock, remote) — the message is the
    /// backend's own.
    #[error("store backend: {0}")]
    Backend(String),
    /// Encoding/decoding failure at the storage boundary.
    #[error("store codec: {0}")]
    Codec(String),
}
