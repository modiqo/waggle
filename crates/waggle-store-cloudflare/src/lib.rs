//! # waggle-store-cloudflare — the edge backend (stub, CP-10 / 0.2)
//!
//! KV as the read cache (a miss is never authoritative — C-10), Queues as
//! the append path, R2 NDJSON as the log, Analytics Engine as the
//! approximate-counter accelerator, with lifecycle writes CAS-ing the
//! origin store directly (design doc `08`). The `worker` dependency is
//! added when this crate lands — stubs stay dependency-free.
