//! # waggle — attributed references for agent handoffs
//!
//! One forager returns from a find and performs an encoded signal; every
//! hive-mate decodes it according to its own role, then flies to the
//! target. That is the bee waggle dance, and it is this library: a shared
//! marker, adaptive interpretation per consumer, recruitment success
//! observable at the hive.
//!
//! Most users never depend on this crate: waggle is consumed as an MCP
//! server (`waggle serve`) by any harness in any language. This facade
//! exists for the rare in-process embedder and re-exports the family.
//!
//! ```
//! // The operations catalog is the map of everything waggle can do.
//! assert!(waggle::ops::find("mint").is_some());
//! ```

pub use waggle_core as core;
pub use waggle_ops as ops;
