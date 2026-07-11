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
        ArgSpec { name: "parent", required: false, doc: "Parent token: forms the delegation tree at mint; revoking the parent tombstones this child." },
        ArgSpec { name: "snapshot", required: false, doc: "Pin the target's bytes content-addressed at mint: read/search then work anywhere the blobs replicate, immutable by hash." },
        ArgSpec { name: "private", required: false, doc: "Mint a capability URL: a 16-char unguessable token (possession IS the credential); public unfurls and social renders refuse it." },
        ArgSpec { name: "tree", required: false, doc: "For a DIRECTORY target: also mint every file inside (recursive, snapshot-pinned) as children of this token — one revocation covers the whole tree, and the folder's funnel rolls its children up." },
        ArgSpec { name: "tag", required: false, doc: "Name the token for humans (repeatable, k=v or a bare name): cosmetic labels that `find` matches on. A tag is a convenience, never identity — resolution stays token-only." },
        ArgSpec { name: "content", required: false, doc: "Path to extracted text for a BINARY target (you extracted it with your own abilities): becomes the searchable content while the target stays the original. Mutually exclusive with snapshot." },
        ArgSpec { name: "attach", required: false, doc: "Path to media (image/audio) stored content-addressed; vision/audio consumers receive it, others get the catch-all." },
        ArgSpec { name: "attach-type", required: false, doc: "Content type of the attachment; inferred from the extension when omitted." },
        ArgSpec { name: "require", required: false, doc: "Consumption contract region (repeatable, max 8): lines:START-END, section:HEADING (markdown), or symbol:NAME (code — resolved against the symbol outline at mint). `coverage` then reports met/unmet with untouched regions NAMED. Signed with the core — a contract is not renegotiable." },
        ArgSpec { name: "min-coverage", required: false, doc: "Fraction (0-1] of required regions a consumer must touch for the contract to be met; default 1.0 (every region)." },
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
        ArgSpec { name: "level", required: false, doc: "For tokens owned elsewhere: eventual (default) serves a cached resolution inside its revalidate window; strict always revalidates at the owner — revocations bite immediately." },
    ],
    forward: &[
        EdgeSpec { to: "search", why: "interrogate the content before ingesting any of it" },
        EdgeSpec { to: "query", why: "slice a large manifest by path instead of pulling it whole" },
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
    description: "Report a lifecycle stage (run, repeat, or a custom stage) against a token so the funnel reflects reality. As the judge of a delegation, record `accepted` or `rejected` — the verdict is the stage itself, and a rejection's response teaches the escalation path (re-mint, supersede). Events are counts with no payload — nothing about your data leaves your machine. Append-only: there is no un-record; record a correcting stage instead.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token the stage applies to." },
        ArgSpec { name: "stage", required: true, doc: "Well-known stage (run, repeat, assess, accepted, rejected, ...) or a custom kebab-case slug." },
    ],
    forward: &[
        EdgeSpec { to: "funnel", why: "see the counts your report just moved" },
        EdgeSpec { to: "map", why: "orient: see what the funnel now suggests" },
    ],
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

/// `funnel` — a token's stage counts: the attribution answer.
pub const FUNNEL: OperationSpec = OperationSpec {
    name: "funnel",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "A token's funnel: stage counts (impression → resolve → run → repeat) plus the judged outcome (pending/accepted/rejected/contested) and lineage roll-up. This is the attribution answer — which handoffs were consumed, which stalled, which delivered repeat value. Counts only; no payloads exist to leak (I-1).",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token whose funnel to report." },
    ],
    forward: &[
        EdgeSpec { to: "coverage", why: "a lineage root? see which files were ACTUALLY consumed" },
        EdgeSpec { to: "map", why: "orient: the funnel feeds the map's ranked suggestions" },
        EdgeSpec { to: "mutate", why: "a stalled or wrong share can be revoked or superseded" },
    ],
    reverse: &[],
    core_fn: "waggle_store::ReadStore::funnel",
};

/// `read` — ranged content access through the token (doc 18).
pub const READ: OperationSpec = OperationSpec {
    name: "read",
    surface: Surface::Both,
    kind: OpKind::RelaxedWrite,
    description: "Read the token's CONTENT surgically: a line window, a markdown section, a code symbol, or a JSON pointer path — never the whole artifact. With no address: the overview (size, content type, available lenses, outline; source code carries its symbol table of contents). Every response fits max-bytes and names the bytes you avoided.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token whose content to read." },
        ArgSpec { name: "lines", required: false, doc: "Line window, 1-based inclusive (e.g. 120-180)." },
        ArgSpec { name: "section", required: false, doc: "Markdown heading whose section to read (text/markdown lens)." },
        ArgSpec { name: "symbol", required: false, doc: "Code symbol whose definition to read (symbol lens — tokens minted with a snapshot of source code); the overview's `symbols` lists what exists." },
        ArgSpec { name: "path", required: false, doc: "JSON pointer into parsed content (application/json lens), e.g. /dependencies/react." },
        ArgSpec { name: "max-bytes", required: false, doc: "Response budget in bytes (default 4096, floor 64)." },
    ],
    forward: &[
        EdgeSpec { to: "read", why: "continue the window or follow the outline deeper" },
        EdgeSpec { to: "record", why: "report run when the content did its job" },
    ],
    reverse: &[],
    core_fn: "waggle_mcp::content::read",
};

/// `search` — grep through the token (doc 18).
pub const SEARCH: OperationSpec = OperationSpec {
    name: "search",
    surface: Surface::Both,
    kind: OpKind::RelaxedWrite,
    description: "Grep the token's CONTENT: regex matches with line numbers and context, capped and budgeted — the matches travel, the artifact stays put. total_matches is counted in full even when the list is truncated. Works wherever the content's blobs replicate.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token whose content to search." },
        ArgSpec { name: "pattern", required: true, doc: "Regex (Rust syntax; (?i) prefix for case-insensitive)." },
        ArgSpec { name: "context", required: false, doc: "Context lines around each match (default 2)." },
        ArgSpec { name: "max-matches", required: false, doc: "Maximum matches returned (default 5, cap 50)." },
        ArgSpec { name: "max-bytes", required: false, doc: "Response budget in bytes (default 4096, floor 64)." },
    ],
    forward: &[
        EdgeSpec { to: "read", why: "open a match's neighborhood as a line window" },
    ],
    reverse: &[],
    core_fn: "waggle_mcp::content::search",
};

/// `query` — budgeted slices with guidance, never whole-response pulls.
pub const QUERY: OperationSpec = OperationSpec {
    name: "query",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "Slice a token's document (manifest, funnel, lineage) by path instead of pulling it whole. Every response fits max-bytes (default 4 KB); oversized values return their shape plus next paths deeper — walk exactly as far as you need.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The waggle token whose document to slice." },
        ArgSpec { name: "path", required: false, doc: "JSON-pointer-style path (e.g. /manifest/variants/0); omit for the root shape." },
        ArgSpec { name: "max-bytes", required: false, doc: "Response budget in bytes (default 4096, floor 64)." },
    ],
    forward: &[EdgeSpec { to: "query", why: "follow a next path one level deeper" }],
    reverse: &[],
    core_fn: "waggle_mcp::query::slice_at",
};

/// `coverage` — the folder handoff's proof of reading.
pub const COVERAGE: OperationSpec = OperationSpec {
    name: "coverage",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "For a lineage root (a folder or bundle): which descendants were actually consumed? Three honest levels per file — unread (never touched), read (bytes served: a resolve, read, or search reached it), run (the consumer recorded using it). For a single token minted with a contract (mint --require): which required regions did the served bytes reach — met/unmet against the declared threshold. Either way, misses are NAMED: the unread list is the proof of what a review skipped.",
    args: &[
        ArgSpec { name: "token", required: true, doc: "The lineage root (or contract-bearing token) to audit." },
    ],
    forward: &[
        EdgeSpec { to: "read", why: "close the gap: read the first unread file" },
        EdgeSpec { to: "funnel", why: "the root's stage counts and rollup" },
    ],
    reverse: &[],
    core_fn: "waggle_mcp::lineage::coverage",
};

/// `find` — discovery by name, never identity.
pub const FIND: OperationSpec = OperationSpec {
    name: "find",
    surface: Surface::Both,
    kind: OpKind::Read,
    description: "Find tokens by what humans remember: matches the query against target basenames, tags, channel, and sharer. Returns ranked CANDIDATES (newest first, disposition shown) — you choose which token to resolve; a name never resolves by itself.",
    args: &[
        ArgSpec { name: "query", required: true, doc: "Substring to match (case-insensitive) against basename, tags, channel, sharer." },
    ],
    forward: &[EdgeSpec { to: "resolve", why: "resolve the candidate you meant" }],
    reverse: &[],
    core_fn: "waggle_mcp::handlers::find",
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
        EdgeSpec { to: "find", why: "don't remember the token? find it by name or tag" },
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
    args: &[
        ArgSpec { name: "stdio", required: false, doc: "Speak MCP over stdin/stdout — as a shim to the shared daemon (unix), or directly." },
        ArgSpec { name: "daemon", required: false, doc: "Run waggled in the foreground: the single owner of the local store, on a unix socket every harness shares." },
    ],
    forward: &[EdgeSpec { to: "map", why: "after the daemon is up, orient from the global map" }],
    reverse: &[],
    core_fn: "waggle_cli::serve",
};

/// `daemon` — lifecycle management for waggled (CLI only).
pub const DAEMON: OperationSpec = OperationSpec {
    name: "daemon",
    surface: Surface::CliOnly,
    kind: OpKind::Read,
    description: "Manage waggled: status (pid, store, uptime, connections, live resource subscriptions, disk weight of the store and blob CAS), start (idempotent), stop (graceful over the socket; terminates orphans by pidfile), restart. Pidfile + idle exit make lingering orphans structurally unlikely.",
    args: &[
        ArgSpec { name: "action", required: true, doc: "status | start | stop | restart | purge (kill EVERY waggled of yours, even zombies whose sockets/pidfiles are gone)." },
        ArgSpec { name: "idle-secs", required: false, doc: "For start/restart: exit after this many seconds with no connections (shim auto-starts default to 1800)." },
    ],
    forward: &[EdgeSpec { to: "map", why: "with the daemon up, orient from the global map" }],
    reverse: &[],
    core_fn: "waggle_cli::daemon::manage",
};

/// `init` — make a repo waggle-fluent (CLI only).
pub const INIT: OperationSpec = OperationSpec {
    name: "init",
    surface: Surface::CliOnly,
    kind: OpKind::RelaxedWrite,
    description: "Install the five-line agent stub into this repo's harness convention files (CLAUDE.md, AGENTS.md, .cursorrules) — creating AGENTS.md and CLAUDE.md when none exist. Idempotent: re-running refreshes the managed block in place. Pair with: claude mcp add waggle -- waggle serve --stdio.",
    args: &[
        ArgSpec { name: "file", required: false, doc: "Target exactly this file instead of auto-detecting convention files." },
    ],
    forward: &[EdgeSpec { to: "map", why: "orient: the tools teach everything past the stub" }],
    reverse: &[],
    core_fn: "waggle_cli::init::run",
};

/// `edge` — interact with a deployed waggle edge (CLI only).
pub const EDGE: OperationSpec = OperationSpec {
    name: "edge",
    surface: Surface::CliOnly,
    kind: OpKind::RelaxedWrite,
    description: "Interact with a deployed waggle edge over HTTPS: status (health + tool surface), push (replicate this store's records and snapshot blobs so tokens resolve and grep there), smoke (mint→resolve→funnel round-trip). Configure with WAGGLE_EDGE_URL and WAGGLE_EDGE_BEARER or the flags. Deploying the worker itself is `npx wrangler deploy` (guide 09).",
    args: &[
        ArgSpec { name: "action", required: true, doc: "status | push | smoke." },
        ArgSpec { name: "url", required: false, doc: "The edge base URL (overrides WAGGLE_EDGE_URL), e.g. https://waggle-edge.you.workers.dev." },
        ArgSpec { name: "bearer", required: false, doc: "The tenant bearer (overrides WAGGLE_EDGE_BEARER)." },
    ],
    forward: &[EdgeSpec { to: "map", why: "with the edge reachable, orient from the global map" }],
    reverse: &[],
    core_fn: "waggle_cli::edge::run",
};

/// `identity` — the host's signing identity (CLI only, CP-11).
pub const IDENTITY: OperationSpec = OperationSpec {
    name: "identity",
    surface: Surface::CliOnly,
    kind: OpKind::RelaxedWrite,
    description: "The host's Ed25519 signing identity: show (public key, or note its absence) | init (generate ~/.waggle/identity; every mint from then on is signed over its immutable core — mutations never invalidate it). Consumers see signature status on every resolve.",
    args: &[
        ArgSpec { name: "action", required: true, doc: "show | init." },
    ],
    forward: &[EdgeSpec { to: "mint", why: "with an identity, mints carry provenance" }],
    reverse: &[],
    core_fn: "waggle_cli::identity::run",
};

/// The catalog. Order is presentation order (CLI help, docs, global map).
pub const OPERATIONS: &[&OperationSpec] = &[
    &MINT, &RESOLVE, &RECORD, &MUTATE, &FUNNEL, &READ, &SEARCH, &QUERY, &FIND, &COVERAGE, &MAP,
    &INIT, &SERVE, &DAEMON, &EDGE, &IDENTITY,
];

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
