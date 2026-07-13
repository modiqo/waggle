//! `waggle-tree` — pure, deterministic index and search primitives for directory
//! trees.
//!
//! A `mint --tree` no longer flattens a directory into thousands of individually
//! minted files. It records a **Merkle hierarchy of directory nodes**, each
//! carrying three derived structures this crate defines:
//!
//! * [`dirindex::DirIndex`] — the entries of one directory: files pinned by
//!   content hash, subdirectories addressed by their own subtree token, with the
//!   size totals that let a node report its weight without a walk.
//! * [`trigram::TrigramIndex`] — an inverted index over a node's file contents,
//!   so "grep across the subtree" becomes "look up a few posting lists and
//!   confirm the survivors."
//! * [`bloom::Bloom`] — a fixed-size, union-composable summary of *all* trigrams
//!   beneath a node, small enough to inline in the node's manifest. It is the
//!   prune gate that keeps search over a deep tree sublinear.
//!
//! [`search`] holds the pure decisions that tie them together — whether a node is
//! worth entering, and how confirmed hits are ranked.
//!
//! The crate is **sans-I/O by design**: everything is a pure function of bytes,
//! so the daemon and the wasm edge can both build and query these structures, and
//! so an index blob is content-addressable and byte-stable (invariant I-2). The
//! filesystem walk that produces entries, the blob store that holds file bytes,
//! and the lineage traversal that drives search all live in `waggle-mcp`, using
//! the types defined here.

#![forbid(unsafe_code)]

pub mod bloom;
pub mod dirindex;
pub mod search;
pub mod trigram;

pub use bloom::Bloom;
pub use dirindex::{DirIndex, Entry, FileEntry, SubdirEntry};
pub use search::{candidates, prune, rank, Hit, Prune};
pub use trigram::{DocId, Trigram, TrigramIndex, TrigramIndexBuilder};
