//! # waggle-store-fs-jsonl — the minimalist backend and the wire format
//!
//! JSONL is waggle's permanent wire format (design docs `07 §6`, `16 §4`):
//! one `LogRecord` per line. This backend is that format used *as* a store —
//! an in-memory contract implementation ([`waggle_store::MemoryStore`])
//! journaled to an append-only file, replayed on open via the same
//! [`waggle_store::AppendStore::ingest`] path migration uses. Real enough
//! to trust (it passes the full conformance suite), simple enough to read
//! in one sitting. Durability model: `Ok` means written and flushed;
//! single owning process (the daemon), like every local backend (16 §5).

#![allow(async_fn_in_trait)]

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use waggle_core::{CanonicalUrl, Event, LogRecord, Seq, Stage, Token};
use waggle_store::{
    AppendIntent, AppendStore, Appended, ManifestView, MemoryStore, ReadStore, StoreError,
};

/// The JSONL-journaled backend.
pub struct FsJsonlStore {
    memory: MemoryStore,
    path: PathBuf,
    file: Mutex<std::fs::File>,
}

impl FsJsonlStore {
    /// Open (or create) a store journaled at `path`, replaying any
    /// existing lines through the ingest path (C-4 makes reopen safe even
    /// over a journal with duplicated lines).
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| StoreError::Backend(format!("journal dir: {e}")))?;
        }
        let memory = MemoryStore::default();
        if path.exists() {
            let text = std::fs::read_to_string(path)
                .map_err(|e| StoreError::Backend(format!("journal read: {e}")))?;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let record: LogRecord = serde_json::from_str(line)
                    .map_err(|e| StoreError::Codec(format!("journal line: {e}")))?;
                pollster_ingest(&memory, record)?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| StoreError::Backend(format!("journal open: {e}")))?;
        Ok(Self {
            memory,
            path: path.to_path_buf(),
            file: Mutex::new(file),
        })
    }

    /// Where the journal lives.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn journal(&self, record: &LogRecord) -> Result<(), StoreError> {
        let line = serde_json::to_string(record)
            .map_err(|e| StoreError::Codec(format!("journal encode: {e}")))?;
        let mut file = self
            .file
            .lock()
            .map_err(|_| StoreError::Backend("journal lock poisoned".into()))?;
        writeln!(file, "{line}")
            .map_err(|e| StoreError::Backend(format!("journal append: {e}")))?;
        file.flush()
            .map_err(|e| StoreError::Backend(format!("journal flush: {e}")))?;
        Ok(())
    }
}

/// Bridge for the synchronous open path (the trait is async; replay at
/// open happens before any executor exists).
fn pollster_ingest(memory: &MemoryStore, record: LogRecord) -> Result<bool, StoreError> {
    pollster::block_on(memory.ingest(record))
}

/// Rebuild the `LogRecord` an accepted intent produced, from the intent's
/// parts and the store's receipt — what the journal line must say.
fn record_of(intent: &AppendIntent, receipt: &Appended) -> Option<LogRecord> {
    match (intent, receipt) {
        (
            AppendIntent::Mint { manifest, .. },
            Appended::Minted {
                replayed: false, ..
            },
        ) => Some(LogRecord::Minted {
            manifest: manifest.clone(),
        }),
        (
            AppendIntent::Mutate {
                token, change, at, ..
            },
            Appended::Mutated { seq, .. },
        ) => Some(LogRecord::Mutation {
            token: *token,
            at: *at,
            seq: *seq,
            change: change.clone(),
        }),
        (
            AppendIntent::Event {
                token,
                stage,
                actor,
                variant,
                regions,
                at,
            },
            Appended::Event { seq },
        ) => Some(LogRecord::Event(Event {
            token: *token,
            stage: stage.clone(),
            actor: *actor,
            at: *at,
            seq: *seq,
            variant: *variant,
            regions: *regions,
        })),
        // Mint replays journal nothing (the original line already exists);
        // mismatched intent/receipt pairs cannot occur but journal nothing.
        _ => None,
    }
}

impl AppendStore for FsJsonlStore {
    async fn append(&self, intent: AppendIntent) -> Result<Appended, StoreError> {
        // Memory first (it enforces the whole contract), then the journal.
        // Ack = journaled: a receipt without its line would break replay.
        let receipt = self.memory.append(intent.clone()).await?;
        if let Some(record) = record_of(&intent, &receipt) {
            self.journal(&record)?;
        }
        Ok(receipt)
    }

    async fn ingest(&self, record: LogRecord) -> Result<bool, StoreError> {
        let fresh = self.memory.ingest(record.clone()).await?;
        if fresh {
            self.journal(&record)?;
        }
        Ok(fresh)
    }
}

impl ReadStore for FsJsonlStore {
    async fn manifest(&self, token: Token) -> Result<Option<ManifestView>, StoreError> {
        self.memory.manifest(token).await
    }
    async fn children(&self, token: Token) -> Result<Vec<Token>, StoreError> {
        self.memory.children(token).await
    }
    async fn tokens_for_target(&self, target: &CanonicalUrl) -> Result<Vec<Token>, StoreError> {
        self.memory.tokens_for_target(target).await
    }
    async fn scan_token(&self, token: Token, from_seq: Seq) -> Result<Vec<LogRecord>, StoreError> {
        self.memory.scan_token(token, from_seq).await
    }
    async fn scan_all(&self) -> Result<Vec<LogRecord>, StoreError> {
        self.memory.scan_all().await
    }
    async fn funnel(&self, token: Token) -> Result<BTreeMap<Stage, u64>, StoreError> {
        self.memory.funnel(token).await
    }
}
