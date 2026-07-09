//! # waggle-mcp — the tool surface agents actually touch
//!
//! Three layers, each testable alone (design docs `09 §2`, `17`):
//!
//! - [`Envelope`] — every response is `{result, next, hint, stats}`;
//!   `next` entries are executable, schema-valid calls
//!   (`envelope_next_valid`), errors always carry a fix-naming `hint`;
//! - [`Handler`] — catalog operation → store call → envelope, generic
//!   over any [`waggle_store::Store`], clock- and randomness-free
//!   (effects are the transport's);
//! - [`handle_message`] — the MCP JSON-RPC wire (`initialize`,
//!   `tools/list`, `tools/call`), tool schemas generated from
//!   `waggle-ops` so the MCP surface cannot drift from the catalog.
//!
//! The `map` engine (17 §3) lives here too: `here` is derived from
//! (manifest, funnel) at call time — the map can never be stale
//! instruction.

#![allow(async_fn_in_trait)]

pub mod content;
mod content_handlers;
mod discovery;
mod envelope;
mod handlers;
mod lineage;
mod map;
pub mod query;
mod rpc;

pub use envelope::{validate_next, Envelope, NextCall, Stats};
pub use handlers::Handler;
pub use map::{global_map, handoff_line, token_map};
pub use rpc::{handle_message, tool_list, PROTOCOL_VERSION};
