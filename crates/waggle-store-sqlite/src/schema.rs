//! The database shape (design doc `07 §4`). One migration for now; the
//! schema annex rule applies (doc `09 §6`): additive changes only.

use rusqlite::Connection;
use waggle_store::StoreError;

/// Apply pragmas and create tables. Idempotent.
pub fn init(conn: &Connection) -> Result<(), StoreError> {
    // WAL = readers never block the writer; FULL sync = an acked commit
    // survives power loss (C-1 — durability is the contract, not a mood).
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(sql)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(sql)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(sql)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS records(
            token   TEXT    NOT NULL,
            seq     INTEGER NOT NULL,
            kind    INTEGER NOT NULL, -- 0 minted · 1 mutation · 2 event
            payload TEXT    NOT NULL, -- the LogRecord, JSON (the wire format)
            PRIMARY KEY (token, seq, kind)
        );
        CREATE TABLE IF NOT EXISTS manifests(
            token   TEXT PRIMARY KEY,
            doc     TEXT    NOT NULL, -- AttributionManifest, JSON
            version INTEGER NOT NULL,
            target  TEXT    NOT NULL,
            parent  TEXT,
            revoked INTEGER NOT NULL DEFAULT 0,
            rowid_order INTEGER       -- mint order for stable listings
        );
        CREATE INDEX IF NOT EXISTS idx_manifests_target ON manifests(target);
        CREATE INDEX IF NOT EXISTS idx_manifests_parent ON manifests(parent);
        CREATE TABLE IF NOT EXISTS nonces(
            sharer TEXT    NOT NULL,
            nonce  INTEGER NOT NULL,
            token  TEXT    NOT NULL,
            PRIMARY KEY (sharer, nonce)  -- C-8 by UNIQUE constraint
        );
        CREATE TABLE IF NOT EXISTS seqs(
            token TEXT PRIMARY KEY,
            next  INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS funnels(
            token TEXT NOT NULL,
            stage TEXT NOT NULL,
            count INTEGER NOT NULL,
            PRIMARY KEY (token, stage)
        );
        ",
    )
    .map_err(sql)
}

/// Map a rusqlite error into the contract's backend error.
#[allow(clippy::needless_pass_by_value)] // ergonomic map_err(sql)
pub fn sql(e: rusqlite::Error) -> StoreError {
    StoreError::Backend(format!("sqlite: {e}"))
}

/// Map a serde error into the contract's codec error.
#[allow(clippy::needless_pass_by_value)] // ergonomic map_err(codec)
pub fn codec(e: serde_json::Error) -> StoreError {
    StoreError::Codec(format!("json: {e}"))
}
