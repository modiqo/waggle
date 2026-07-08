# 09 — Crate Layout, Public API, and Engineering Policy

*Revision 2. Changes: `waggle-mcp` promoted to the primary interface and a
0.1 deliverable; npm/pyo3 language bindings **deleted** (consumption is
protocol-shaped — MCP + HTTPS — so any language participates with zero waggle
code); the core function surface is pinned 1:1 to the MCP tool schema; the
facade-crate question settled (yes).*

*Revision 2.2: the local runtime is a **tokio daemon** (`waggled`, HTTP-MCP
on 127.0.0.1) with `--stdio` reduced to an auto-starting proxy shim; the
primary local store is **`waggle-store-sqlite`** (07 §4); `fs` renamed
`fs-jsonl` (optional/minimalist). Note the tokio rule was never violated:
"no tokio" binds **library crates** (runtime-agnostic, `?Send`); binaries
always could and now explicitly do use tokio.*

## 1. Workspace

```text
waggle/                        (one repo, one workspace)
├── crates/
│   ├── waggle-core            sans-I/O domain (03): mint · negotiate ·
│   │                          resolve · events · folds · reconstruct
│   ├── waggle-social          SharePackage renderers · OG · QR (05)
│   ├── waggle-agent           extractors (HarnessMeta · A2A card · explicit),
│   │                          lineage helpers (06)
│   ├── waggle-store           Store trait · contract types · conformance
│   ├── waggle-store-sqlite    ★ primary local backend (rusqlite bundled,
│   │                          WAL) + arc-swap hot cache — behind the daemon
│   ├── waggle-store-fs-jsonl  optional minimalist backend; JSONL is also
│   │                          the export/replay wire format (07 §6, 16)
│   ├── waggle-store-cloudflare  KV/Queues/R2/AE backend (08)
│   ├── waggle-ops             ★★ the operations catalog (rev 2.5): one
│   │                          `OPERATIONS` table — specs, agent-first
│   │                          descriptions, args, forward/reverse edges,
│   │                          surface (cli|mcp|both). Single source of
│   │                          truth; everything below projects from it
│   ├── waggle-mcp             ★ the MCP projection: tool JSON schemas
│   │                          generated from waggle-ops + server plumbing
│   │                          (streamable-HTTP + stdio transports)
│   ├── waggle-serve           the Cloudflare workers (08), incl. /mcp route
│   ├── waggle-cli             the clap projection (rev 2.5): `waggled` ·
│   │                          `waggle serve --stdio` (shim) · mint/resolve/
│   │                          record/funnel/query/map/mutate/share ·
│   │                          export/replay/compact/gc/status — derive API,
│   │                          doc-comments = help, parity-tested vs the
│   │                          catalog; completions + man pages generated
│   └── waggle                 facade crate: re-exports core+social+agent for
│   │                          the rare in-process embedder; owns the
│   │                          crates.io umbrella name (settled: yes)
└── xtask/                     conformance orchestration, wasm-size check
```

Deleted from rev 1: `waggle-js`/`waggle-py` bindings (never listed as crates,
but implied by the roadmap — now explicitly out; the protocol is the
portability layer, and doc 11's conformance vectors are how a second
implementation happens if anyone wants one).

## 2. The operations catalog — one source, four projections (rev 2.5)

Every operation is declared **once**, in `waggle-ops`:

```rust
pub struct OperationSpec {
    pub name: &'static str,            // "mint"
    pub surface: Surface,              // Both | CliOnly (daemon lifecycle,
                                       //   export/replay/compact/gc/status)
    pub kind: OpKind,                  // DurableWrite | RelaxedWrite | Read
    pub description: &'static str,     // ONE canonical, agent-first string —
                                       //   the MCP tool description AND the
                                       //   clap about text. One voice.
    pub args: &'static [ArgSpec],      // name, type, required, default, doc
    pub forward: &'static [EdgeSpec],  // the map's forward paths (17)
    pub reverse: &'static [EdgeSpec],  // …and reverse paths / irreversible
    pub core_fn: &'static str,         // symbol the correspondence test pins
}
```

The four projections, each mechanical:

| Projection | Mechanism | Drift guard (CI) |
|---|---|---|
| **MCP tools** | JSON schema generated from `args` | `tool_schema_from_ops` — generated, so can't drift |
| **clap CLI** | derive API with doc comments (ergonomics, self-documenting `--help`) | `ops_inventory_parity` — introspects the built `clap::Command` tree at test time (subcommands, args, help strings) and diffs it against `OPERATIONS` both directions: every `Both`/`CliOnly` op has its subcommand, every arg matches name/required/doc, nothing undeclared exists |
| **map edges** (17) | read directly from `forward`/`reverse` | `envelope_next_valid`, `map_reachability` (already specified) |
| **docs** | `xtask gen-docs` → `COMMANDS.md` + `clap_mangen` man pages + `clap_complete` shell completions | committed output diffed in CI — stale docs fail the build |

**Why clap-plus-parity rather than a hand-rolled parser:** rote solves this
same drift problem by owning its parser and validating every command body
against its catalog — correct, but expensive. Clap's derive gives us
self-documenting help, completions, man pages, and arg validation for free;
runtime introspection of the `Command` tree gives us the same
single-source-of-truth guarantee through a test instead of a parser. Less
code, same discipline.

**CLI/MCP envelope parity:** every CLI verb supports `--json`, emitting the
*same* `{result, next, hint, stats}` envelope as the MCP tool (17 §2) — a
human in a terminal and an agent over MCP read the same shape, and the
guidance system teaches both.

Pinned correspondence to core (a CI test deserializes each generated tool
schema and asserts it maps onto the core signature — drift fails the build):

| MCP tool | Core function | Notes |
|---|---|---|
| `mint` | `core::mint(spec, opts, entropy, now)` | args: target, meta, sharer, channel, variants[], parent?, ttl?, `mint_nonce` (auto-generated by the MCP layer if absent — idempotent under retry, C-8), `attach[]` (rev 2.3: file paths — bytes → CAS, body becomes a MediaRef; ≤64 KB inlines automatically) → manifest + short URL |
| `resolve` | `core::negotiate` + `core::resolve` | args: token, context (HarnessMeta \| A2A card \| explicit), `consistency: eventual\|strict` (G-8; manifest may mandate strict) → disposition + projection + variant index + `as_of` + `revalidate_after`; MediaRef bodies return `{url, sha256, content_type, size}` — bytes fetched out-of-band, never through the tool response |
| `record` | `core::event` + `Store::append` | args: token, stage, actor-class hints, `durability: durable\|relaxed` → seq |
| `mutate` | lifecycle/cosmetic manifest changes | lifecycle requires `expected_version` (CAS, C-9 → `Conflict`); cosmetic is LWW |
| `funnel` | `core::fold_funnel` over `Store::scan_token` | args: token \| target → report (read-only) |
| `share` *(optional)* | `social::package` | args: token, channel → SharePackage artifact |
| `query` | guided slice engine (13 §9) | args: token, path, max_bytes? → slice + guidance (`next` paths) + stats — agents navigate, never dump |
| `map` | derived from the tool registry + (manifest, funnel) state (17 §3) | args: token? → `here` + ranked `forward[]` + `reverse[]` + one guidance sentence — "I am here; what are my paths?" The skill, computed |

**Response envelope (rev 2.4, normative — 17 §2):** every tool returns
`{result, next[], hint?, stats}` where `next` entries are schema-valid
executable calls (CI-checked), `mint`'s `next[0]` is always the subagent
handoff line, and every error carries a fix-naming `hint`. Tool
*descriptions* are written as agent instruction and reviewed as API surface.
Forward/reverse edges live in the same registry as the schemas —
instruction cannot drift from implementation.

Transports: **stdio** (local, `waggle serve --stdio`, fs store — zero infra)
and **streamable-HTTP** (the hosted `/mcp` route, 08). Same tools, two radii.
Plain HTTPS routes (`GET /x/{t}`, `POST /x/{t}/resolve`, `GET
/api/manifest/{t}`) remain for non-MCP consumers and unfurl bots.

## 3. Dependency policy (the whole tree, honest)

| Crate | Required deps | Optional (feature) |
|---|---|---|
| core | `serde`, `thiserror` | `compact_str`, `smallvec`, `arc-swap` (`perf`) |
| social | core, `serde` | `qrcodegen` (`qr`), PNG encoding (`qr-png`) |
| agent | core, `serde`, `serde_json` | `ed25519-dalek` (v0.3, `signed`) |
| store | core, `thiserror` | `async` (default), `sync` |
| store-sqlite | store, `rusqlite` (bundled), `arc-swap` | `parquet`, `arrow` (`archive`) |
| store-fs-jsonl | store, `serde_json` | — |
| store-cloudflare | store, `worker` | — |
| ops | `serde` (specs only — no I/O, no deps beyond serde) | — |
| mcp | core, agent, store, ops, `serde_json` | transport features: `http`, `stdio` |
| serve / cli | the above + **`tokio`, `axum`-class HTTP, `clap` (derive) — binaries only; the library no-tokio/no-clap rule is untouched** | `clap_complete`, `clap_mangen` (`gen` feature, used by xtask) |

Rules: no `anyhow` in libraries; no `tokio` in any library crate (async
traits are runtime-agnostic, `?Send`); `waggle-core` compiles to
`wasm32-unknown-unknown` with default features (CI-enforced — this is the
Workers build-target requirement, 08, not a distribution mechanism).

## 4. Public Rust API sketch (secondary interface, for embedders)

```rust
// ── waggle-core ─────────────────────────────────────────────────────────
pub struct MintSpec { /* target, meta, sharer, channel, variants, parent, ttl */ }
impl MintSpec {
    pub fn new(target: CanonicalUrl, sharer: Sharer, channel: Channel) -> Self;
    pub fn meta(self, meta: TargetMeta) -> Self;
    pub fn variant(self, m: MatchExpr, body: VariantBody) -> Self;
    pub fn child_of(self, parent: Token) -> Self;
    pub fn ttl_ms(self, ttl: u64) -> Self;
}

pub fn mint(spec: MintSpec, opts: &MintOptions,
            entropy: &mut impl Entropy, now: Timestamp)
    -> Result<AttributionManifest, MintError>;
pub fn negotiate(hint: ConsumerHint<'_>) -> ResolverContext;
pub fn resolve<'m>(m: &'m AttributionManifest, ctx: &ResolverContext,
                   now: Timestamp) -> Resolution<'m>;
pub fn fold_funnel(events: impl IntoIterator<Item = EventView>) -> FunnelReport;
pub fn reconstruct(log: impl IntoIterator<Item = LogRecord>)
    -> Result<WorldState, ReplayError>;

// ── waggle-agent ────────────────────────────────────────────────────────
pub trait ContextExtractor { fn extract(&self, i: &CardInput) -> Result<ResolverContext, ExtractError>; }
pub struct HarnessMetaExtractor;   // Claude Code / Codex metadata (default chain, first)
pub struct A2aExtractor;           // Agent Card (+ x-waggle/* extensions)
pub fn mint_child(parent: &AttributionManifest, spec: ChildSpec, /* effects */)
    -> Result<AttributionManifest, MintError>;   // fails on revoked parent (06 §4)

// ── waggle-store ────────────────────────────────────────────────────────
pub trait Store { /* 07 §2, clauses C-1..C-7 */ }
pub mod conformance { pub async fn run_all<S: Store>(h: Harness<S>); }
```

## 5. Error taxonomy

```rust
MintError    ::= InvalidTarget | InvalidChannel | InvalidSharer
               | MissingCatchAllVariant | ParentRevoked | Collision{retries}
               | Entropy(..)
               // duplicate mint_nonce is NOT an error — idempotent replay
               // returns the original manifest (C-8)
ResolveError ::= (none — resolve is total over a manifest; absence of a
                  manifest is the store's UnknownToken)
StoreError   ::= UnknownToken | Conflict{token,seq} | ParentRevoked
               | Backend(..) | Io(..) | Codec(..)
ExtractError ::= UnrecognizedCard | MalformedCard(..)
ReplayError  ::= SeqRegression{token} | CorruptRecord{pos, ..}
```

## 6. Versioning, MSRV, quality gates

- **Semver with a schema annex**: the wire/log schema (`schema: u16`) versions
  independently; minor crate bumps may *add* record kinds, never reinterpret
  old ones — the replay promise is forever. The MCP tool schema versions with
  the spec (11), not with the crates.
- MSRV: current stable minus ~4 releases, CI-pinned; edition 2021 (**[open]**
  revisit at 1.0).
- CI gates: clippy (curated pedantic), rustfmt, `cargo-deny`, wasm32 build of
  core+social+agent, conformance on memory+fs, Miniflare conformance on
  cloudflare, **tool-schema↔core-signature correspondence test** (§2), doc
  examples tested, `cargo-semver-checks`.
- License: MIT OR Apache-2.0.

## 7. Naming

crates.io: `waggle` verified unclaimed (July 2026). Settled: publish the
facade crate under the umbrella name; users who embed write `waggle::mint`,
docs stay one crate wide, internals stay split. The MCP server is what most
users install (`waggle-cli`), and never requires knowing any of this.
