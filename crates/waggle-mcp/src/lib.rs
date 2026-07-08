//! # waggle-mcp — the primary interface (stub, CP-6)
//!
//! Projects the operations catalog (`waggle-ops`) into MCP tool schemas,
//! wraps every response in the fluency envelope
//! (`{result, next, hint, stats}` — design doc `17 §2`), and provides the
//! stdio + streamable-HTTP server plumbing consumed by `waggled` and the
//! shim. The tool descriptions ARE the catalog descriptions — one voice,
//! parity-tested.
