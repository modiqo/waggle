//! The store implementation: one writer connection behind a mutex (the
//! daemon's committer task is the single caller in production — 13 §8),
//! WAL snapshot reads, and a read-through manifest cache invalidated
//! in-commit.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use waggle_core::{
    apply_change, AttributionManifest, CanonicalUrl, Event, LogRecord, Seq, Stage, Token,
};
use waggle_store::{
    AppendIntent, AppendStore, Appended, ManifestView, MintNonce, ReadStore, StoreError,
};

use crate::schema::{codec, init, sql};

/// The SQLite-anchored store (design doc `07 §4`).
pub struct SqliteStore {
    conn: Mutex<Connection>,
    /// Read-through hot cache: token → shared manifest. Invalidated inside
    /// the commit path — a cache over the anchor, never the anchor.
    /// Isolated in [`crate::cache`] so loom model-checks its semantics.
    cache: crate::cache::Cache<Token, Arc<AttributionManifest>>,
}

impl SqliteStore {
    /// Open (or create) a store at `path`.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(sql)?;
        init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            cache: crate::cache::Cache::new(),
        })
    }

    /// An in-memory `SQLite` store — for tests and ephemeral use.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory().map_err(sql)?;
        init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            cache: crate::cache::Cache::new(),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError::Backend("connection lock poisoned".into()))
    }

    fn cache_put(&self, manifest: &AttributionManifest) {
        self.cache.put(manifest.token, Arc::new(manifest.clone()));
    }

    fn cache_drop(&self, token: Token) {
        self.cache.drop_key(&token);
    }
}

fn load_manifest(
    conn: &Connection,
    token: Token,
) -> Result<Option<AttributionManifest>, StoreError> {
    let doc: Option<String> = conn
        .query_row(
            "SELECT doc FROM manifests WHERE token = ?1",
            params![token.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    doc.map(|d| serde_json::from_str(&d).map_err(codec))
        .transpose()
}

fn store_manifest(
    conn: &Connection,
    m: &AttributionManifest,
    fresh: bool,
) -> Result<(), StoreError> {
    let doc = serde_json::to_string(m).map_err(codec)?;
    if fresh {
        conn.execute(
            "INSERT INTO manifests(token, doc, version, target, parent, revoked)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                m.token.as_str(),
                doc,
                m.version,
                m.target.as_str(),
                m.parent.map(|p| p.as_str().to_owned()),
                i32::from(m.revoked_at.is_some()),
            ],
        )
        .map_err(sql)?;
    } else {
        conn.execute(
            "UPDATE manifests SET doc = ?2, version = ?3, revoked = ?4 WHERE token = ?1",
            params![
                m.token.as_str(),
                doc,
                m.version,
                i32::from(m.revoked_at.is_some())
            ],
        )
        .map_err(sql)?;
    }
    Ok(())
}

fn insert_record(conn: &Connection, rec: &LogRecord) -> Result<bool, StoreError> {
    let kind = match rec {
        LogRecord::Minted { .. } => 0,
        LogRecord::Mutation { .. } => 1,
        LogRecord::Event(_) => 2,
    };
    let payload = serde_json::to_string(rec).map_err(codec)?;
    let inserted = conn
        .execute(
            "INSERT OR IGNORE INTO records(token, seq, kind, payload) VALUES (?1, ?2, ?3, ?4)",
            params![rec.token().as_str(), rec.seq().0, kind, payload],
        )
        .map_err(sql)?;
    Ok(inserted > 0)
}

fn take_seq(conn: &Connection, token: Token) -> Result<Seq, StoreError> {
    conn.execute(
        "INSERT INTO seqs(token, next) VALUES (?1, 2)
         ON CONFLICT(token) DO UPDATE SET next = next + 1",
        params![token.as_str()],
    )
    .map_err(sql)?;
    let next: u32 = conn
        .query_row(
            "SELECT next FROM seqs WHERE token = ?1",
            params![token.as_str()],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(Seq(next - 1))
}

fn parse_token(s: &str) -> Result<Token, StoreError> {
    Token::parse(s).map_err(|e| StoreError::Codec(format!("stored token: {e}")))
}

impl AppendStore for SqliteStore {
    async fn append(&self, intent: AppendIntent) -> Result<Appended, StoreError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        let (receipt, touched) = match intent {
            AppendIntent::Mint { manifest, nonce } => mint_tx(&tx, *manifest, nonce)?,
            AppendIntent::Mutate {
                token,
                change,
                expected_version,
                at,
            } => mutate_tx(&tx, token, change, expected_version, at)?,
            AppendIntent::Event {
                token,
                stage,
                actor,
                variant,
                regions,
                entry,
                at,
            } => event_tx(&tx, token, &stage, actor, variant, regions, entry, at)?,
        };
        tx.commit().map_err(sql)?;
        // Cache maintenance strictly after commit: never serve uncommitted.
        for (token, manifest) in touched {
            match manifest {
                Some(m) => self.cache_put(&m),
                None => self.cache_drop(token),
            }
        }
        Ok(receipt)
    }

    async fn ingest(&self, record: LogRecord) -> Result<bool, StoreError> {
        let token = record.token();
        let mut conn = self.lock()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql)?;
        if !insert_record(&tx, &record)? {
            tx.commit().map_err(sql)?;
            return Ok(false); // C-4: duplicate, nothing changes
        }
        rebuild_views_tx(&tx, token)?;
        tx.commit().map_err(sql)?;
        self.cache_drop(token);
        Ok(true)
    }
}

type Touched = Vec<(Token, Option<AttributionManifest>)>;

fn mint_tx(
    tx: &Connection,
    manifest: AttributionManifest,
    nonce: MintNonce,
) -> Result<(Appended, Touched), StoreError> {
    // C-8: the UNIQUE(sharer, nonce) row is the dedup authority.
    let existing: Option<String> = tx
        .query_row(
            "SELECT token FROM nonces WHERE sharer = ?1 AND nonce = ?2",
            params![
                manifest.sharer.as_str(),
                i64::from_ne_bytes(nonce.0.to_ne_bytes())
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if let Some(tok) = existing {
        let original = load_manifest(tx, parse_token(&tok)?)?
            .ok_or_else(|| StoreError::Backend("nonce points at missing manifest".into()))?;
        let view = ManifestView {
            manifest: Arc::new(original),
        };
        return Ok((
            Appended::Minted {
                view,
                replayed: true,
            },
            Vec::new(),
        ));
    }
    // C-7: parents must exist here and be alive.
    if let Some(parent) = manifest.parent {
        let revoked: Option<i32> = tx
            .query_row(
                "SELECT revoked FROM manifests WHERE token = ?1",
                params![parent.as_str()],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        match revoked {
            None => return Err(StoreError::ParentUnknown(parent)),
            Some(r) if r != 0 => return Err(StoreError::ParentRevoked(parent)),
            Some(_) => {}
        }
    }
    let token = manifest.token;
    tx.execute(
        "INSERT INTO nonces(sharer, nonce, token) VALUES (?1, ?2, ?3)",
        params![
            manifest.sharer.as_str(),
            i64::from_ne_bytes(nonce.0.to_ne_bytes()),
            token.as_str()
        ],
    )
    .map_err(sql)?;
    tx.execute(
        "INSERT INTO seqs(token, next) VALUES (?1, 1)
         ON CONFLICT(token) DO NOTHING",
        params![token.as_str()],
    )
    .map_err(sql)?;
    store_manifest(tx, &manifest, true)?;
    insert_record(
        tx,
        &LogRecord::Minted {
            manifest: Box::new(manifest.clone()),
        },
    )?;
    let view = ManifestView {
        manifest: Arc::new(manifest.clone()),
    };
    Ok((
        Appended::Minted {
            view,
            replayed: false,
        },
        vec![(token, Some(manifest))],
    ))
}

fn mutate_tx(
    tx: &Connection,
    token: Token,
    change: waggle_core::Change,
    expected_version: Option<u32>,
    at: waggle_core::Timestamp,
) -> Result<(Appended, Touched), StoreError> {
    let mut manifest = load_manifest(tx, token)?.ok_or(StoreError::UnknownToken(token))?;
    if change.is_lifecycle() {
        let expected = expected_version.ok_or(StoreError::LifecycleRequiresVersion(token))?;
        // C-9: the guard rides the UPDATE below too, but check first for a
        // fix-naming error instead of a silent no-op.
        if expected != manifest.version {
            return Err(StoreError::Conflict {
                token,
                expected,
                actual: manifest.version,
            });
        }
    }
    apply_change(&mut manifest, &change, at);
    store_manifest(tx, &manifest, false)?;
    let seq = take_seq(tx, token)?;
    insert_record(
        tx,
        &LogRecord::Mutation {
            token,
            at,
            seq,
            change,
        },
    )?;
    let version = manifest.version;
    Ok((
        Appended::Mutated { seq, version },
        vec![(token, Some(manifest))],
    ))
}

// The args mirror the `AppendIntent::Event` variant's fields 1:1; bundling them
// into a throwaway struct would only relocate the same fields.
#[allow(clippy::too_many_arguments)]
fn event_tx(
    tx: &Connection,
    token: Token,
    stage: &Stage,
    actor: waggle_core::ActorClass,
    variant: Option<u8>,
    regions: Option<u8>,
    entry: Option<u32>,
    at: waggle_core::Timestamp,
) -> Result<(Appended, Touched), StoreError> {
    if load_manifest(tx, token)?.is_none() {
        return Err(StoreError::UnknownToken(token));
    }
    let seq = take_seq(tx, token)?;
    tx.execute(
        "INSERT INTO funnels(token, stage, count) VALUES (?1, ?2, 1)
         ON CONFLICT(token, stage) DO UPDATE SET count = count + 1",
        params![token.as_str(), stage.as_str()],
    )
    .map_err(sql)?;
    insert_record(
        tx,
        &LogRecord::Event(Event {
            token,
            stage: stage.clone(),
            actor,
            at,
            seq,
            variant,
            regions,
            entry,
        }),
    )?;
    Ok((Appended::Event { seq }, Vec::new()))
}

/// After an ingest, rebuild the token's materialized rows from its records
/// (R-4 by reconstruction — the replay path favors correctness over speed).
fn rebuild_views_tx(tx: &Connection, token: Token) -> Result<(), StoreError> {
    let mut stmt = tx
        .prepare("SELECT payload FROM records WHERE token = ?1 ORDER BY seq, kind")
        .map_err(sql)?;
    let records: Vec<LogRecord> = stmt
        .query_map(params![token.as_str()], |r| r.get::<_, String>(0))
        .map_err(sql)?
        .filter_map(Result::ok)
        .map(|p| serde_json::from_str(&p).map_err(codec))
        .collect::<Result<_, _>>()?;
    let max_seq = records.iter().map(|r| r.seq().0).max().unwrap_or(0);
    let world = waggle_core::reconstruct(records);
    if let Some(m) = world.manifests.get(&token) {
        let fresh = load_manifest(tx, token)?.is_none();
        store_manifest(tx, m, fresh)?;
        tx.execute(
            "INSERT INTO seqs(token, next) VALUES (?1, ?2)
             ON CONFLICT(token) DO UPDATE SET next = MAX(next, ?2)",
            params![token.as_str(), max_seq + 1],
        )
        .map_err(sql)?;
    }
    tx.execute(
        "DELETE FROM funnels WHERE token = ?1",
        params![token.as_str()],
    )
    .map_err(sql)?;
    if let Some(stages) = world.funnels.get(&token) {
        for (stage, count) in stages {
            tx.execute(
                "INSERT INTO funnels(token, stage, count) VALUES (?1, ?2, ?3)",
                params![token.as_str(), stage.as_str(), count],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}

impl ReadStore for SqliteStore {
    async fn manifest(&self, token: Token) -> Result<Option<ManifestView>, StoreError> {
        // Hot path: the cache. Correctness path: the anchor (C-10 — a
        // cache miss consults SQLite before answering None).
        if let Some(m) = self.cache.get(&token) {
            return Ok(Some(ManifestView { manifest: m }));
        }
        let conn = self.lock()?;
        let Some(manifest) = load_manifest(&conn, token)? else {
            return Ok(None);
        };
        drop(conn);
        let arc = Arc::new(manifest);
        self.cache.put(token, arc.clone());
        Ok(Some(ManifestView { manifest: arc }))
    }

    async fn children(&self, token: Token) -> Result<Vec<Token>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT token FROM manifests WHERE parent = ?1 ORDER BY rowid")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![token.as_str()], |r| r.get::<_, String>(0))
            .map_err(sql)?
            .filter_map(Result::ok)
            .map(|s| parse_token(&s))
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    async fn tokens_for_target(&self, target: &CanonicalUrl) -> Result<Vec<Token>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT token FROM manifests WHERE target = ?1 ORDER BY rowid")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![target.as_str()], |r| r.get::<_, String>(0))
            .map_err(sql)?
            .filter_map(Result::ok)
            .map(|s| parse_token(&s))
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    async fn scan_token(&self, token: Token, from_seq: Seq) -> Result<Vec<LogRecord>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT payload FROM records WHERE token = ?1 AND seq >= ?2 ORDER BY seq, kind",
            )
            .map_err(sql)?;
        let records: Result<Vec<LogRecord>, StoreError> = stmt
            .query_map(params![token.as_str(), from_seq.0], |r| {
                r.get::<_, String>(0)
            })
            .map_err(sql)?
            .filter_map(Result::ok)
            .map(|p| serde_json::from_str(&p).map_err(codec))
            .collect();
        records
    }

    async fn scan_all(&self) -> Result<Vec<LogRecord>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT payload FROM records ORDER BY token, seq, kind")
            .map_err(sql)?;
        let records: Result<Vec<LogRecord>, StoreError> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(sql)?
            .filter_map(Result::ok)
            .map(|p| serde_json::from_str(&p).map_err(codec))
            .collect();
        records
    }

    async fn funnel(&self, token: Token) -> Result<BTreeMap<Stage, u64>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT stage, count FROM funnels WHERE token = ?1")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![token.as_str()], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
            })
            .map_err(sql)?
            .filter_map(Result::ok);
        let mut out = BTreeMap::new();
        for (stage, count) in rows {
            let stage = Stage::new(&stage).map_err(|e| StoreError::Codec(format!("stage: {e}")))?;
            out.insert(stage, count);
        }
        Ok(out)
    }
}
