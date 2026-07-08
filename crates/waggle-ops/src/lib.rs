//! # waggle-ops — the operations catalog
//!
//! Every waggle operation is declared **exactly once**, in [`OPERATIONS`].
//! Four surfaces project from this table and are forbidden to drift from it
//! (design doc `09 §2`):
//!
//! 1. **MCP tool schemas** — generated from [`OperationSpec::args`].
//! 2. **The clap CLI** — `waggle-cli` wires its `about`/`help` strings to
//!    the [`OperationSpec::description`] constants and is held to the
//!    catalog by the `ops_inventory_parity` test (both directions).
//! 3. **The `map` tool's edges** — [`OperationSpec::forward`] /
//!    [`OperationSpec::reverse`] (design doc `17 §3`).
//! 4. **Docs / man pages / completions** — `xtask gen-docs` renders this
//!    table.
//!
//! The descriptions are written **for agents first**: they are the MCP tool
//! descriptions, which are the primary teaching surface (`17 §1`). Keep them
//! in one voice — when to use, when not to, the one-call form first.
//!
//! This crate performs no I/O and depends only on `serde`. It compiles to
//! `wasm32-unknown-unknown` unchanged.
//!
//! ```
//! let mint = waggle_ops::find("mint").expect("mint is a catalog operation");
//! assert!(mint.description.contains("reference"));
//! ```

use serde::Serialize;

/// Which surfaces expose an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Surface {
    /// Exposed as an MCP tool **and** a CLI subcommand.
    Both,
    /// CLI-only (daemon lifecycle, export/replay, maintenance).
    CliOnly,
}

/// The durability/effect class of an operation (design doc `13 §8`:
/// two-lane committer intake routes on this).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum OpKind {
    /// Must be committed before ack (mint, lifecycle mutations).
    DurableWrite,
    /// May ack on enqueue; durable within the next commit window (events).
    RelaxedWrite,
    /// No writes; safe before consent, identity, or trust.
    Read,
}

/// One argument of an operation — the unit both the MCP JSON schema and the
/// clap arg are generated/parity-checked from.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ArgSpec {
    /// Kebab-case argument name (CLI flag name and MCP property name).
    pub name: &'static str,
    /// Whether the argument must be supplied (defaults exist otherwise).
    pub required: bool,
    /// One-sentence, agent-first documentation string.
    pub doc: &'static str,
}

/// A navigational edge for the `map` tool: from the owning operation to
/// `to`, with the reason an agent would take it (design doc `17 §3`).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct EdgeSpec {
    /// Name of the target operation (must exist in [`OPERATIONS`]).
    pub to: &'static str,
    /// Why an agent would take this edge — one calm sentence.
    pub why: &'static str,
}

/// A single operation, declared once, projected everywhere.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct OperationSpec {
    /// Kebab-case operation name (MCP tool name and CLI subcommand).
    pub name: &'static str,
    /// Which surfaces expose it.
    pub surface: Surface,
    /// Durability/effect class.
    pub kind: OpKind,
    /// The canonical, agent-first description. This exact string is the MCP
    /// tool description and the clap `about` text — one voice everywhere.
    pub description: &'static str,
    /// Arguments (generated into the MCP schema; parity-checked in clap).
    pub args: &'static [ArgSpec],
    /// Forward edges for the `map` tool.
    pub forward: &'static [EdgeSpec],
    /// Reverse edges (undo paths). Empty means irreversible by design —
    /// the map must then offer compensating guidance (`17 §3`).
    pub reverse: &'static [EdgeSpec],
    /// Fully-qualified core function this operation is pinned to by the
    /// schema↔signature correspondence test (design doc `09 §2`).
    pub core_fn: &'static str,
}

/// `mint` — the seed operation of the whole system.
pub const MINT: OperationSpec = OperationSpec {
    name: "mint",
    surface: Surface::Both,
    kind: OpKind::DurableWrite,
    description: "Create an attributed reference (a waggle token) for an artifact instead of pasting its content into a prompt. One call: `mint { target }` — sharer and channel are defaulted and a catch-all variant is synthesized. The response's first `next` entry is the exact handoff line to give a subagent.",
    args: &[
        ArgSpec { name: "target", required: true, doc: "Canonical URI of the artifact (file path, workspace URI, or URL)." },
        ArgSpec { name: "sharer", required: false, doc: "Who is distributing this; defaults to the session identity." },
        ArgSpec { name: "channel", required: false, doc: "Where this share lives (e.g. subagent/researcher); defaults to subagent/general." },
    ],
    forward: &[
        EdgeSpec { to: "resolve", why: "self-check the projection each consumer will receive" },
        EdgeSpec { to: "map", why: "orient: see all paths available from this fresh token" },
    ],
    reverse: &[EdgeSpec { to: "mutate", why: "revoke or supersede the token if the artifact must be withdrawn" }],
    core_fn: "waggle_core::mint",
};

/// `resolve` — consume a token, receiving the projection for your context.
pub const RESOLVE: OperationSpec = OperationSpec {
    name: "resolve",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "Fetch the projection of a waggle token matched to your context (model family, harness, modalities, posture). Read-only and safe before trust. The response carries as_of and revalidate_after — re-resolve before acting on stale knowledge.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token to resolve." },
        ArgSpec { name: "context", required: false, doc: "Resolver context (harness metadata, A2A agent card, or explicit JSON); defaults to negotiated." },
    ],
    forward: &[
        EdgeSpec { to: "record", why: "report downstream stages (run, repeat) so the funnel stays honest" },
        EdgeSpec { to: "map", why: "orient: see what this token expects of you next" },
    ],
    reverse: &[],
    core_fn: "waggle_core::resolve",
};

/// `record` — report a downstream stage against a token.
pub const RECORD: OperationSpec = OperationSpec {
    name: "record",
    surface: Surface::Both,
    kind: OpKind::RelaxedWrite,
    description: "Report a lifecycle stage (run, repeat, or a custom stage) against a token so the funnel reflects reality. Events are counts with no payload — nothing about your data leaves your machine. Append-only: there is no un-record; record a correcting stage instead.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token the stage applies to." },
        ArgSpec { name: "stage", required: true, doc: "Well-known stage (run, repeat, assess, ...) or a custom kebab-case slug." },
    ],
    forward: &[EdgeSpec { to: "map", why: "orient: see what the funnel now suggests" }],
    reverse: &[],
    core_fn: "waggle_core::event",
};

/// `mutate` — lifecycle and cosmetic changes to a token's manifest.
pub const MUTATE: OperationSpec = OperationSpec {
    name: "mutate",
    surface: Surface::Both,
    kind: OpKind::DurableWrite,
    description: "Change a token's manifest. Lifecycle changes (revoke, supersede, expiry) require expected_version and fail with a conflict on mismatch — retry after re-reading. Cosmetic changes (campaign, labels) are last-writer-wins. Revoking a token tombstones its children.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token to change." },
        ArgSpec { name: "change", required: true, doc: "The change: revoke, supersede=<token>, expire=<ts>, or label k=v." },
        ArgSpec { name: "expected-version", required: false, doc: "Required for lifecycle changes: the manifest version this change was decided against (CAS)." },
    ],
    forward: &[EdgeSpec { to: "map", why: "confirm the token's new disposition and remaining paths" }],
    reverse: &[EdgeSpec { to: "mutate", why: "a supersede can itself be superseded; revocation is final" }],
    core_fn: "waggle_core::mutate",
};

/// `map` — "I am here; what are my forward and reverse paths?"
pub const MAP: OperationSpec = OperationSpec {
    name: "map",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "Orientation. With no arguments: the global map of operations from where you stand. With a token: its current state (here), ranked forward paths, and reverse paths — derived live from the manifest and funnel, so it can never be stale instruction.",
    args: &[ArgSpec { name: "token", required: false, doc: "Token to orient around; omit for the global map." }],
    forward: &[
        EdgeSpec { to: "mint", why: "start: turn an artifact into an attributed reference" },
        EdgeSpec { to: "resolve", why: "consume: fetch a token's projection for your context" },
    ],
    reverse: &[],
    core_fn: "waggle_core::map",
};

/// `serve` — run the local daemon / stdio shim (CLI only).
pub const SERVE: OperationSpec = OperationSpec {
    name: "serve",
    surface: Surface::CliOnly,
    kind: OpKind::Read,
    description: "Run the waggle daemon (waggled): the single owner of the local store, serving every harness on this machine over MCP. With --stdio, act as a proxy shim for harnesses that spawn stdio servers (auto-starts the daemon if absent).",
    args: &[ArgSpec { name: "stdio", required: false, doc: "Run as a stdio proxy shim instead of the HTTP daemon." }],
    forward: &[EdgeSpec { to: "map", why: "after the daemon is up, orient from the global map" }],
    reverse: &[],
    core_fn: "waggle_cli::serve",
};

/// The catalog. Order is presentation order (CLI help, docs, global map).
pub const OPERATIONS: &[&OperationSpec] = &[&MINT, &RESOLVE, &RECORD, &MUTATE, &MAP, &SERVE];

/// Look an operation up by name.
pub fn find(name: &str) -> Option<&'static OperationSpec> {
    OPERATIONS.iter().copied().find(|op| op.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_kebab_case() {
        let mut seen = std::collections::BTreeSet::new();
        for op in OPERATIONS {
            assert!(seen.insert(op.name), "duplicate operation name {}", op.name);
            assert!(
                op.name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
                "operation name {} is not kebab-case",
                op.name
            );
        }
    }

    #[test]
    fn descriptions_are_agent_worthy() {
        for op in OPERATIONS {
            assert!(
                op.description.len() >= 40,
                "description of {} is too thin to teach an agent",
                op.name
            );
        }
        for op in OPERATIONS {
            for arg in op.args {
                assert!(
                    !arg.doc.is_empty(),
                    "undocumented arg {} on {}",
                    arg.name,
                    op.name
                );
            }
        }
    }

    #[test]
    fn edges_point_at_catalog_operations() {
        for op in OPERATIONS {
            for edge in op.forward.iter().chain(op.reverse.iter()) {
                assert!(
                    find(edge.to).is_some(),
                    "edge {} -> {} targets an unknown operation",
                    op.name,
                    edge.to
                );
            }
        }
    }

    #[test]
    fn every_tool_is_reachable_from_the_global_map() {
        // map_reachability (17 §5), catalog-level: walk forward edges from
        // `map` and require every Both-surface operation to be visited.
        let mut visited = std::collections::BTreeSet::new();
        let mut stack = vec!["map"];
        while let Some(name) = stack.pop() {
            if visited.insert(name) {
                if let Some(op) = find(name) {
                    stack.extend(op.forward.iter().chain(op.reverse.iter()).map(|e| e.to));
                }
            }
        }
        for op in OPERATIONS {
            if op.surface == Surface::Both {
                assert!(
                    visited.contains(op.name),
                    "{} unreachable from the global map",
                    op.name
                );
            }
        }
    }

    #[test]
    fn find_misses_politely() {
        assert!(find("does-not-exist").is_none());
    }
}
