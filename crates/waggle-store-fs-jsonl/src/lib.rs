//! # waggle-store-fs-jsonl — the minimalist backend and the wire format (stub, CP-5)
//!
//! JSONL is waggle's permanent wire format: `waggle export` and
//! `waggle replay --to` stream `LogRecord`s between any two backends —
//! idempotent by C-4/C-8, deterministic by R-1 (design docs `07 §6`,
//! `16 §4`). This crate is also the optional minimalist backend: real
//! enough to trust, simple enough to read in an afternoon.
