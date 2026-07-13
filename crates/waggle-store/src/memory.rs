//! The in-memory reference backend: real enough to trust (it passes the
//! full conformance suite), simple enough to read in one sitting. Also the
//! test double for everything above the store (doc `07 §6`).
//!
//! Concurrency model: one `Mutex` around the whole state — the memory
//! backend models the *contract*, not the performance architecture (that's
//! `waggle-store-sqlite`, CP-5). Lock scope is a single commit or read.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use waggle_core::{
    AttributionManifest, CanonicalUrl, Change, Event, LogRecord, Seq, Sharer, Stage, Token,
};

use crate::error::StoreError;
use crate::traits::{AppendStore, ReadStore};
use crate::types::{AppendIntent, Appended, ManifestView, MintNonce};

#[derive(Debug, Default)]
struct Inner {
    /// The append-only log (C-2). Never mutated, never truncated.
    records: Vec<LogRecord>,
    /// Materialized manifest view (rebuildable; R-4-checked).
    manifests: BTreeMap<Token, AttributionManifest>,
    /// Next per-token seq (C-3). Minted is 0; assignments start at 1.
    next_seq: BTreeMap<Token, u32>,
    /// Mint idempotency (C-8): (sharer, nonce) → original token.
    nonces: HashMap<(Sharer, u64), Token>,
    /// Lineage index: parent → children in mint order.
    children: BTreeMap<Token, Vec<Token>>,
    /// Target index: canonical target → tokens in mint order.
    by_target: BTreeMap<String, Vec<Token>>,
    /// Materialized funnel counts (R-4-checked against the fold).
    funnels: BTreeMap<Token, BTreeMap<Stage, u64>>,
    /// Ingest dedup: (token, seq, kind) already applied (C-4).
    seen: std::collections::HashSet<(Token, u32, u8)>,
}

fn record_kind(rec: &LogRecord) -> u8 {
    match rec {
        LogRecord::Minted { .. } => 0,
        LogRecord::Mutation { .. } => 1,
        LogRecord::Event(_) => 2,
    }
}

/// The reference in-memory backend.
#[derive(Debug, Default)]
pub struct MemoryStore {
    inner: Mutex<Inner>,
}

impl MemoryStore {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend("lock poisoned".into()))
    }
}

impl Inner {
    fn take_seq(&mut self, token: Token) -> Seq {
        let next = self.next_seq.entry(token).or_insert(1);
        let seq = Seq(*next);
        *next += 1;
        seq
    }

    fn mint(
        &mut self,
        manifest: AttributionManifest,
        nonce: MintNonce,
    ) -> Result<Appended, StoreError> {
        // C-8: idempotent replay returns the original manifest.
        let key = (manifest.sharer.clone(), nonce.0);
        if let Some(original) = self.nonces.get(&key) {
            let view = ManifestView {
                manifest: self.manifests[original].clone().into(),
            };
            return Ok(Appended::Minted {
                view,
                replayed: true,
            });
        }
        // C-7: no children under a tombstone; parents must exist here.
        if let Some(parent) = manifest.parent {
            let p = self
                .manifests
                .get(&parent)
                .ok_or(StoreError::ParentUnknown(parent))?;
            if p.revoked_at.is_some() {
                return Err(StoreError::ParentRevoked(parent));
            }
            self.children
                .entry(parent)
                .or_default()
                .push(manifest.token);
        }
        let token = manifest.token;
        self.nonces.insert(key, token);
        self.by_target
            .entry(manifest.target.as_str().to_owned())
            .or_default()
            .push(token);
        self.manifests.insert(token, manifest.clone());
        self.next_seq.insert(token, 1);
        self.seen.insert((token, 0, 0));
        self.records.push(LogRecord::Minted {
            manifest: Box::new(manifest),
        });
        let view = ManifestView {
            manifest: self.manifests[&token].clone().into(),
        };
        Ok(Appended::Minted {
            view,
            replayed: false,
        })
    }

    fn mutate(
        &mut self,
        token: Token,
        change: Change,
        expected_version: Option<u32>,
        at: waggle_core::Timestamp,
    ) -> Result<Appended, StoreError> {
        let manifest = self
            .manifests
            .get_mut(&token)
            .ok_or(StoreError::UnknownToken(token))?;
        if change.is_lifecycle() {
            // C-9: CAS or refuse.
            let expected = expected_version.ok_or(StoreError::LifecycleRequiresVersion(token))?;
            if expected != manifest.version {
                return Err(StoreError::Conflict {
                    token,
                    expected,
                    actual: manifest.version,
                });
            }
        }
        waggle_core::apply_change(manifest, &change, at);
        let version = manifest.version;
        let seq = self.take_seq(token);
        self.seen.insert((token, seq.0, 1));
        self.records.push(LogRecord::Mutation {
            token,
            at,
            seq,
            change,
        });
        Ok(Appended::Mutated { seq, version })
    }

    fn event(&mut self, intent: &AppendIntent) -> Result<Appended, StoreError> {
        let AppendIntent::Event {
            token,
            stage,
            actor,
            variant,
            regions,
            entry,
            at,
        } = intent
        else {
            return Err(StoreError::Backend(
                "event() called with non-event intent".into(),
            ));
        };
        if !self.manifests.contains_key(token) {
            return Err(StoreError::UnknownToken(*token));
        }
        let seq = self.take_seq(*token);
        *self
            .funnels
            .entry(*token)
            .or_default()
            .entry(stage.clone())
            .or_insert(0) += 1;
        self.seen.insert((*token, seq.0, 2));
        self.records.push(LogRecord::Event(Event {
            token: *token,
            stage: stage.clone(),
            actor: *actor,
            at: *at,
            seq,
            variant: *variant,
            regions: *regions,
            entry: *entry,
        }));
        Ok(Appended::Event { seq })
    }
}

impl Inner {
    /// The replay/migration path (16 §4): apply a sequenced record as-is,
    /// idempotently (C-4). Views rebuild from the token's records so R-4
    /// holds after any ingest order.
    fn ingest(&mut self, record: LogRecord) -> bool {
        let key = (record.token(), record.seq().0, record_kind(&record));
        if !self.seen.insert(key) {
            return false;
        }
        let token = record.token();
        self.records.push(record);
        self.rebuild_token_views(token);
        true
    }

    fn rebuild_token_views(&mut self, token: Token) {
        let mut records: Vec<LogRecord> = self
            .records
            .iter()
            .filter(|r| r.token() == token)
            .cloned()
            .collect();
        records.sort_by_key(|r| (r.seq(), record_kind(r)));
        let world = waggle_core::reconstruct(records);
        if let Some(m) = world.manifests.get(&token) {
            if let Some(parent) = m.parent {
                let kids = self.children.entry(parent).or_default();
                if !kids.contains(&token) {
                    kids.push(token);
                }
            }
            let target_tokens = self
                .by_target
                .entry(m.target.as_str().to_owned())
                .or_default();
            if !target_tokens.contains(&token) {
                target_tokens.push(token);
            }
            self.manifests.insert(token, m.clone());
            let next = self
                .records
                .iter()
                .filter(|r| r.token() == token)
                .map(|r| r.seq().0)
                .max()
                .unwrap_or(0)
                + 1;
            self.next_seq.insert(token, next);
        }
        if let Some(f) = world.funnels.get(&token) {
            self.funnels.insert(token, f.clone());
        }
    }
}

impl AppendStore for MemoryStore {
    async fn append(&self, intent: AppendIntent) -> Result<Appended, StoreError> {
        let mut inner = self.lock()?;
        match intent {
            AppendIntent::Mint { manifest, nonce } => inner.mint(*manifest, nonce),
            AppendIntent::Mutate {
                token,
                change,
                expected_version,
                at,
            } => inner.mutate(token, change, expected_version, at),
            e @ AppendIntent::Event { .. } => inner.event(&e),
        }
    }

    async fn ingest(&self, record: LogRecord) -> Result<bool, StoreError> {
        Ok(self.lock()?.ingest(record))
    }
}

impl ReadStore for MemoryStore {
    async fn manifest(&self, token: Token) -> Result<Option<ManifestView>, StoreError> {
        let inner = self.lock()?;
        Ok(inner.manifests.get(&token).map(|m| ManifestView {
            manifest: m.clone().into(),
        }))
    }

    async fn children(&self, token: Token) -> Result<Vec<Token>, StoreError> {
        Ok(self
            .lock()?
            .children
            .get(&token)
            .cloned()
            .unwrap_or_default())
    }

    async fn tokens_for_target(&self, target: &CanonicalUrl) -> Result<Vec<Token>, StoreError> {
        Ok(self
            .lock()?
            .by_target
            .get(target.as_str())
            .cloned()
            .unwrap_or_default())
    }

    async fn scan_token(&self, token: Token, from_seq: Seq) -> Result<Vec<LogRecord>, StoreError> {
        let inner = self.lock()?;
        let mut records: Vec<LogRecord> = inner
            .records
            .iter()
            .filter(|r| r.token() == token && r.seq() >= from_seq)
            .cloned()
            .collect();
        records.sort_by_key(waggle_core::LogRecord::seq);
        Ok(records)
    }

    async fn scan_all(&self) -> Result<Vec<LogRecord>, StoreError> {
        Ok(self.lock()?.records.clone())
    }

    async fn funnel(&self, token: Token) -> Result<BTreeMap<Stage, u64>, StoreError> {
        Ok(self
            .lock()?
            .funnels
            .get(&token)
            .cloned()
            .unwrap_or_default())
    }
}
