//! The edge store engine (design doc `08 §8`): the full waggle storage
//! contract over [`EdgeStorage`] — the same `Inner` shape the local
//! backends proved, keyed for a KV world:
//!
//! ```text
//! rec:{token}:{kind}{seq:010}   → LogRecord JSON      (the log — never deleted)
//! man:{token}                   → manifest JSON        (view)
//! seq:{token}                   → next seq             (view)
//! non:{sharer}:{nonce}          → manifest JSON        (C-8 registry: the claim
//!                                                       carries the manifest so a
//!                                                       replayed mint can answer)
//! tgt:{target}:{token}          → ""                   (index)
//! chd:{parent}:{n:010}:{token}  → ""                   (index, MINT-ORDERED —
//!                                                       the oracle caught key-order
//!                                                       diverging from the contract)
//! chq:{parent}                  → next child ordinal
//! fun:{token}:{stage}           → count                (view)
//! ```
//!
//! Zero-padded seq keeps prefix listings in replay order. Views are
//! rebuildable from `rec:` alone (R-4); `ingest` proves it on every call.

use waggle_core::{
    apply_change, AttributionManifest, CanonicalUrl, Event, LogRecord, Seq, Stage, Token,
};
use waggle_store::{
    AppendIntent, AppendStore, Appended, ManifestView, MintNonce, ReadStore, StoreError,
};

use crate::storage::EdgeStorage;

/// The engine. `S` is the runtime's storage; the contract is the same
/// everywhere.
pub struct EdgeStore<S> {
    storage: S,
}

fn rec_key(token: Token, kind: u8, seq: u32) -> String {
    format!("rec:{token}:{kind}{seq:010}")
}

fn kind_of(rec: &LogRecord) -> u8 {
    match rec {
        LogRecord::Minted { .. } => 0,
        LogRecord::Mutation { .. } => 1,
        LogRecord::Event(_) => 2,
    }
}

impl<S: EdgeStorage> EdgeStore<S> {
    /// Wrap a storage.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Children must list in MINT order (the trait contract) — a plain
    /// token key would sort lexicographically, so an ordinal leads.
    async fn register_child(&self, parent: Token, child: Token) -> Result<(), StoreError> {
        let seq_key = format!("chq:{parent}");
        let n: u32 = self
            .storage
            .get(&seq_key)
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        self.storage.put(&seq_key, &(n + 1).to_string()).await?;
        self.storage
            .put(&format!("chd:{parent}:{n:010}:{child}"), "")
            .await
    }

    async fn load_manifest(&self, token: Token) -> Result<Option<AttributionManifest>, StoreError> {
        let Some(doc) = self.storage.get(&format!("man:{token}")).await? else {
            return Ok(None);
        };
        serde_json::from_str(&doc)
            .map(Some)
            .map_err(|e| StoreError::Codec(format!("manifest: {e}")))
    }

    async fn store_manifest(&self, m: &AttributionManifest) -> Result<(), StoreError> {
        let doc = serde_json::to_string(m).map_err(|e| StoreError::Codec(e.to_string()))?;
        self.storage.put(&format!("man:{}", m.token), &doc).await
    }

    async fn take_seq(&self, token: Token) -> Result<Seq, StoreError> {
        let key = format!("seq:{token}");
        let next: u32 = self
            .storage
            .get(&key)
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        self.storage.put(&key, &(next + 1).to_string()).await?;
        Ok(Seq(next))
    }

    async fn push_record(&self, rec: &LogRecord) -> Result<bool, StoreError> {
        let key = rec_key(rec.token(), kind_of(rec), rec.seq().0);
        if self.storage.get(&key).await?.is_some() {
            return Ok(false); // C-4: (token, seq, kind) dedup
        }
        let payload = serde_json::to_string(rec).map_err(|e| StoreError::Codec(e.to_string()))?;
        self.storage.put(&key, &payload).await?;
        Ok(true)
    }

    async fn records_of(&self, token: Token) -> Result<Vec<LogRecord>, StoreError> {
        let mut records: Vec<LogRecord> = Vec::new();
        for (_, payload) in self.storage.list(&format!("rec:{token}:")).await? {
            records.push(
                serde_json::from_str(&payload).map_err(|e| StoreError::Codec(e.to_string()))?,
            );
        }
        records.sort_by_key(|r| (r.seq(), kind_of(r)));
        Ok(records)
    }

    async fn bump_funnel(&self, token: Token, stage: &Stage) -> Result<(), StoreError> {
        let key = format!("fun:{token}:{}", stage.as_str());
        let count: u64 = self
            .storage
            .get(&key)
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        self.storage.put(&key, &(count + 1).to_string()).await
    }

    async fn mint(
        &self,
        manifest: AttributionManifest,
        nonce: MintNonce,
    ) -> Result<Appended, StoreError> {
        let non_key = format!("non:{}:{}", manifest.sharer.as_str(), nonce.0);
        if let Some(original) = self.storage.get(&non_key).await? {
            let m: AttributionManifest = serde_json::from_str(&original)
                .map_err(|e| StoreError::Codec(format!("nonce registry: {e}")))?;
            let view = ManifestView {
                manifest: std::sync::Arc::new(m),
            };
            return Ok(Appended::Minted {
                view,
                replayed: true,
            });
        }
        if let Some(parent) = manifest.parent {
            match self.load_manifest(parent).await? {
                None => return Err(StoreError::ParentUnknown(parent)),
                Some(p) if p.revoked_at.is_some() => return Err(StoreError::ParentRevoked(parent)),
                Some(_) => {
                    self.register_child(parent, manifest.token).await?;
                }
            }
        }
        let doc = serde_json::to_string(&manifest).map_err(|e| StoreError::Codec(e.to_string()))?;
        // The nonce claim CARRIES the manifest: a replayed mint answers
        // from the registry even if later steps raced a crash (08 §8).
        self.storage.put(&non_key, &doc).await?;
        self.storage
            .put(
                &format!("tgt:{}:{}", manifest.target.as_str(), manifest.token),
                "",
            )
            .await?;
        self.store_manifest(&manifest).await?;
        self.push_record(&LogRecord::Minted {
            manifest: Box::new(manifest.clone()),
        })
        .await?;
        let view = ManifestView {
            manifest: std::sync::Arc::new(manifest),
        };
        Ok(Appended::Minted {
            view,
            replayed: false,
        })
    }

    async fn mutate(
        &self,
        token: Token,
        change: waggle_core::Change,
        expected_version: Option<u32>,
        at: waggle_core::Timestamp,
    ) -> Result<Appended, StoreError> {
        let mut manifest = self
            .load_manifest(token)
            .await?
            .ok_or(StoreError::UnknownToken(token))?;
        if change.is_lifecycle() {
            let expected = expected_version.ok_or(StoreError::LifecycleRequiresVersion(token))?;
            if expected != manifest.version {
                return Err(StoreError::Conflict {
                    token,
                    expected,
                    actual: manifest.version,
                });
            }
        }
        apply_change(&mut manifest, &change, at);
        self.store_manifest(&manifest).await?;
        let seq = self.take_seq(token).await?;
        self.push_record(&LogRecord::Mutation {
            token,
            at,
            seq,
            change,
        })
        .await?;
        Ok(Appended::Mutated {
            seq,
            version: manifest.version,
        })
    }
}

impl<S: EdgeStorage> AppendStore for EdgeStore<S> {
    async fn append(&self, intent: AppendIntent) -> Result<Appended, StoreError> {
        match intent {
            AppendIntent::Mint { manifest, nonce } => self.mint(*manifest, nonce).await,
            AppendIntent::Mutate {
                token,
                change,
                expected_version,
                at,
            } => self.mutate(token, change, expected_version, at).await,
            AppendIntent::Event {
                token,
                stage,
                actor,
                variant,
                at,
            } => {
                if self.load_manifest(token).await?.is_none() {
                    return Err(StoreError::UnknownToken(token));
                }
                let seq = self.take_seq(token).await?;
                self.bump_funnel(token, &stage).await?;
                self.push_record(&LogRecord::Event(Event {
                    token,
                    stage,
                    actor,
                    at,
                    seq,
                    variant,
                }))
                .await?;
                Ok(Appended::Event { seq })
            }
        }
    }

    async fn ingest(&self, record: LogRecord) -> Result<bool, StoreError> {
        let token = record.token();
        if !self.push_record(&record).await? {
            return Ok(false);
        }
        // R-4 by reconstruction, same as every backend's replay path.
        let world = waggle_core::reconstruct(self.records_of(token).await?);
        if let Some(m) = world.manifests.get(&token) {
            self.store_manifest(m).await?;
            if let Some(parent) = m.parent {
                self.storage
                    .put(&format!("chd:{parent}:{token}"), "")
                    .await?;
            }
            self.storage
                .put(&format!("tgt:{}:{token}", m.target.as_str()), "")
                .await?;
            let max_seq = self
                .records_of(token)
                .await?
                .iter()
                .map(|r| r.seq().0)
                .max()
                .unwrap_or(0);
            self.storage
                .put(&format!("seq:{token}"), &(max_seq + 1).to_string())
                .await?;
        }
        for (key, _) in self.storage.list(&format!("fun:{token}:")).await? {
            self.storage.delete(&key).await?;
        }
        if let Some(stages) = world.funnels.get(&token) {
            for (stage, count) in stages {
                self.storage
                    .put(
                        &format!("fun:{token}:{}", stage.as_str()),
                        &count.to_string(),
                    )
                    .await?;
            }
        }
        Ok(true)
    }
}

impl<S: EdgeStorage> ReadStore for EdgeStore<S> {
    async fn manifest(&self, token: Token) -> Result<Option<ManifestView>, StoreError> {
        Ok(self.load_manifest(token).await?.map(|m| ManifestView {
            manifest: std::sync::Arc::new(m),
        }))
    }

    async fn children(&self, token: Token) -> Result<Vec<Token>, StoreError> {
        let prefix = format!("chd:{token}:");
        self.storage
            .list(&prefix)
            .await?
            .into_iter()
            .map(|(k, _)| {
                // key = chd:{parent}:{ordinal}:{child} — ordinal keeps
                // mint order; the child is the last segment.
                let child = k.rsplit(':').next().unwrap_or_default();
                Token::parse(child).map_err(|e| StoreError::Codec(format!("child key: {e}")))
            })
            .collect()
    }

    async fn tokens_for_target(&self, target: &CanonicalUrl) -> Result<Vec<Token>, StoreError> {
        let prefix = format!("tgt:{}:", target.as_str());
        self.storage
            .list(&prefix)
            .await?
            .into_iter()
            .map(|(k, _)| {
                Token::parse(&k[prefix.len()..])
                    .map_err(|e| StoreError::Codec(format!("target key: {e}")))
            })
            .collect()
    }

    async fn scan_token(&self, token: Token, from_seq: Seq) -> Result<Vec<LogRecord>, StoreError> {
        Ok(self
            .records_of(token)
            .await?
            .into_iter()
            .filter(|r| r.seq() >= from_seq)
            .collect())
    }

    async fn scan_all(&self) -> Result<Vec<LogRecord>, StoreError> {
        let mut records = Vec::new();
        for (_, payload) in self.storage.list("rec:").await? {
            records.push(
                serde_json::from_str(&payload).map_err(|e| StoreError::Codec(e.to_string()))?,
            );
        }
        Ok(records)
    }

    async fn funnel(
        &self,
        token: Token,
    ) -> Result<std::collections::BTreeMap<Stage, u64>, StoreError> {
        let prefix = format!("fun:{token}:");
        let mut out = std::collections::BTreeMap::new();
        for (key, value) in self.storage.list(&prefix).await? {
            let stage = Stage::new(&key[prefix.len()..])
                .map_err(|e| StoreError::Codec(format!("stage key: {e}")))?;
            out.insert(stage, value.parse().unwrap_or(0));
        }
        Ok(out)
    }
}
