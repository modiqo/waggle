//! # waggle-store-cloudflare — the edge tier's store engine
//!
//! The full storage contract over a five-verb [`EdgeStorage`] seam
//! (design doc `08 §8`): the engine runs **natively** against an
//! in-memory fake — where conformance and the differential oracle
//! certify it — and **inside a Durable Object** against DO storage,
//! where the single-writer execution model provides the atomicity the
//! contract needs (the same principle as `waggled`, relocated).
//!
//! The worker itself (routes, auth, DO class) lives in the deploy crate;
//! this crate is deliberately runtime-free.

#![allow(async_fn_in_trait)] // ?Send by design — Workers futures aren't Send.

mod engine;
mod storage;

pub use engine::EdgeStore;
pub use storage::{EdgeStorage, MemoryEdgeStorage};
