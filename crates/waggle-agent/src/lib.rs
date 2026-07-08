//! # waggle-agent — resolver-context extraction (stub, CP-2/CP-6)
//!
//! Maps consumer self-descriptions — harness metadata (Claude Code, Codex),
//! signed A2A Agent Cards, or explicit JSON — into the neutral
//! `ResolverContext` the sealed variant matcher consumes (design doc
//! `06 §1`). The extractor seam is what keeps waggle independent of any one
//! protocol: if a schema drifts, only its extractor changes.
//!
//! This crate is a documented stub until CP-2 lands `ResolverContext` in
//! core; the module boundary and dependency direction are fixed now so the
//! workspace shape is honest from the first commit.
