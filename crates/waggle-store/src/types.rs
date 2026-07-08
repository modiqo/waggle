//! The value types crossing the store boundary: append intents in (the
//! store assigns sequence — C-3), views and receipts out.

use std::sync::Arc;

use waggle_core::{ActorClass, AttributionManifest, Change, Seq, Stage, Timestamp, Token};

/// Client-supplied idempotency nonce for mint (C-8). The MCP layer
/// auto-generates one when absent; retries reuse it, and the store returns
/// the *original* manifest instead of minting a duplicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MintNonce(pub u64);

/// What a caller asks the store to append. Sequencing is absent on
/// purpose: the committer assigns it (C-3) — callers state intent, stores
/// establish order.
#[derive(Debug, Clone)]
pub enum AppendIntent {
    /// A freshly minted manifest (version 1, from [`waggle_core::mint`]).
    Mint {
        /// The manifest to persist. Its `parent`, if any, is C-7-checked.
        /// Boxed: manifests dwarf the other variants (clippy
        /// `large_enum_variant`) and mint is the rare intent.
        manifest: Box<AttributionManifest>,
        /// Idempotency key, scoped per sharer (C-8).
        nonce: MintNonce,
    },
    /// A manifest mutation. Lifecycle changes require `expected_version`
    /// (C-9); cosmetic changes ignore it (LWW).
    Mutate {
        /// The token to change.
        token: Token,
        /// The change.
        change: Change,
        /// CAS guard for lifecycle changes.
        expected_version: Option<u32>,
        /// When the mutation was decided.
        at: Timestamp,
    },
    /// A funnel event (payload-free — I-1).
    Event {
        /// The token the stage applies to.
        token: Token,
        /// The stage.
        stage: Stage,
        /// Coarse actor dimensions.
        actor: ActorClass,
        /// Which variant served a resolve, when applicable.
        variant: Option<u8>,
        /// When it happened.
        at: Timestamp,
    },
}

/// The store's receipt for an accepted append.
#[derive(Debug, Clone)]
pub enum Appended {
    /// A mint landed — or replayed (C-8): `replayed == true` means the
    /// nonce was seen before and `view` is the *original* manifest.
    Minted {
        /// The persisted (or original, on replay) manifest.
        view: ManifestView,
        /// True when this was an idempotent replay, not a fresh mint.
        replayed: bool,
    },
    /// A mutation committed; the manifest is now at `version`.
    Mutated {
        /// Assigned per-token sequence.
        seq: Seq,
        /// The manifest version after the change.
        version: u32,
    },
    /// An event committed.
    Event {
        /// Assigned per-token sequence.
        seq: Seq,
    },
}

/// A read of one manifest at the store's current prefix. `Arc` because the
/// hot path shares it, never copies it (doc `13 §7`).
#[derive(Debug, Clone)]
pub struct ManifestView {
    /// The manifest state.
    pub manifest: Arc<AttributionManifest>,
}

impl ManifestView {
    /// The mutable-section version (the CAS baseline callers cite in
    /// lifecycle mutations).
    #[must_use]
    pub fn version(&self) -> u32 {
        self.manifest.version
    }
}
