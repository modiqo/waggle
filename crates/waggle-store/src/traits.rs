//! The trait family (design doc `07 §2`): [`ReadStore`] + [`AppendStore`]
//! compose into [`Store`]. The split is trait inheritance with intent —
//! read-only consumers take `&impl ReadStore` and *cannot* write (I-4 as a
//! type bound, `13 §3`).

use std::collections::BTreeMap;

use waggle_core::{CanonicalUrl, LogRecord, MediaRef, Seq, Stage, Token};

use crate::error::StoreError;
use crate::types::{AppendIntent, Appended, ManifestView};

/// Read side of the contract. Everything here is answerable from the
/// materialized views (rebuildable — R-4 holds views to fold-equality).
pub trait ReadStore {
    /// The manifest for `token`, or `None` after an **authoritative** miss
    /// (C-10 — a cache miss must consult the system of record first).
    async fn manifest(&self, token: Token) -> Result<Option<ManifestView>, StoreError>;

    /// Children minted under `token` (lineage), in mint order.
    async fn children(&self, token: Token) -> Result<Vec<Token>, StoreError>;

    /// Tokens minted for a canonical target, in mint order.
    async fn tokens_for_target(&self, target: &CanonicalUrl) -> Result<Vec<Token>, StoreError>;

    /// The token's records from `from_seq` (inclusive), seq-ascending —
    /// the reconstruct path (C-2: nothing is ever modified or deleted).
    async fn scan_token(&self, token: Token, from_seq: Seq) -> Result<Vec<LogRecord>, StoreError>;

    /// Every record in the store — replay/export (the wire format's
    /// source; doc `16 §4`).
    async fn scan_all(&self) -> Result<Vec<LogRecord>, StoreError>;

    /// Materialized stage counts for one token. May be accelerated; must
    /// equal the fold (R-4 — the conformance suite diffs both).
    async fn funnel(&self, token: Token) -> Result<BTreeMap<Stage, u64>, StoreError>;
}

/// Write side of the contract: one method, intent in, receipt out. The
/// commit point owns sequencing (C-3), nonce dedupe (C-8), CAS (C-9), and
/// the revoked-parent check (C-7).
pub trait AppendStore {
    /// Append one intent. `Ok` means durable per the backend's documented
    /// model (C-1).
    async fn append(&self, intent: AppendIntent) -> Result<Appended, StoreError>;

    /// The replay/migration path (doc `16 §4`): apply an already-sequenced
    /// record verbatim, idempotently by `(token, seq, kind)` (C-4).
    /// Returns `true` when newly applied, `false` on a dedup hit. Views
    /// must converge to fold-equality afterward (R-4). Never used by live
    /// traffic — live appends state intent and receive sequence.
    async fn ingest(&self, record: LogRecord) -> Result<bool, StoreError>;
}

/// The full contract. Blanket-implemented — a backend implements the two
/// halves and gets `Store` for free.
pub trait Store: ReadStore + AppendStore {}

impl<T: ReadStore + AppendStore> Store for T {}

/// Content-addressed blob storage (rev 2.3): bytes in, [`MediaRef`] out;
/// reads verify. Backends with a sidecar implement this; hosts hand it to
/// the tool layer so `mint --attach` and media resolution work.
pub trait BlobSink {
    /// Store bytes; identical bytes yield the identical address.
    fn put(&self, bytes: &[u8], content_type: &str) -> Result<MediaRef, StoreError>;
    /// Fetch and integrity-verify the bytes a `MediaRef` names.
    fn get(&self, media: &MediaRef) -> Result<Vec<u8>, StoreError>;
}
