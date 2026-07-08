//! # waggle-store-sqlite — the production laptop store (stub, CP-5)
//!
//! `SQLite` in WAL mode is the correctness anchor (design docs `07 §4`,
//! `13 §8`): WAL snapshot reads provide consistency by construction, the
//! single-writer committer transaction carries seq assignment, nonce
//! dedupe, and CAS; an in-memory cache serves the hot resolve path as an
//! accelerator over the anchor, never as the mechanism. Content-addressed
//! blob storage for `MediaRef`s lives beside the database.
