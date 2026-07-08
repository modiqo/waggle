//! # waggle-store — the storage contract (stub, CP-4)
//!
//! Defines the log-shaped `Store` trait family (design doc `07 §2`) with
//! the supertrait split that makes read-only paths type-enforced
//! (`ReadStore` consumers cannot append — invariant I-4 as a bound), the
//! normative contract clauses C-1..C-10, and the generic conformance suite
//! (`07 §5`): a backend without a green conformance run is not a waggle
//! backend.
