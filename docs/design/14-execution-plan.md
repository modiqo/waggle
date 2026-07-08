# 14 — Execution Plan: Checkpoints to Completion

*New in revision 2. The build, checkpointed. Each checkpoint (CP) has
deliverables, **acceptance gates** (citing 13's standards — a CP is done when
its gates are green in CI, not when its code exists), and a status row in the
tracking table. Checkpoints are sequential unless marked ∥ (parallelizable).*

## Phase 0.1 — the primitive, in the harness

### CP-0 · Workspace scaffold & CI teeth
Workspace per 09 §1 (all crates stubbed, incl. **`waggle-ops`** with 2–3
seed OperationSpecs), xtask with the **file-size lint** and `gen-docs`
skeleton, clap-derive CLI skeleton wired to the catalog, CI pipeline:
fmt/clippy/deny/missing-docs/wasm32-build/semver-checks on the **three-OS
matrix**.
**Gates:** CI green on stubs across all three OSes; file-size lint
demonstrably fails a 751-line fixture; wasm32 core stub builds;
**`ops_inventory_parity` green on the seed ops and demonstrably fails when
a clap subcommand is added without a catalog entry** (the guard proves
itself before it guards anything).

### CP-1 · Core types & manifests
`Token` (inline, rejection-sampled), newtypes (`Sharer`, `Channel`, `Stage`,
`Timestamp`, `Seq`), `TargetMeta`, `AttributionManifest` (3 zones),
`MintSpec` builder, `mint()` with `Entropy` function-passing.
**Gates:** property tests — alphabet/no-modulo-bias, collision retry, slug
normalization (≥6 properties); every public item documented; unit coverage.

### CP-2 · The sealed matcher
`MatchExpr`, `ResolverContext`, `negotiate()` (UA classes), `select_variant`
(sealed; match → specificity → declaration order → catch-all), `resolve()`
returning borrowed `Resolution<'_>` **with `as_of` + `revalidate_after`
(G-3)**.
**Gates:** the **selection-vector table** (11 §4) passes, including tie and
near-miss rows; determinism property (same ctx ⇒ same index, 10⁴ random
contexts); `resolve` takes `&impl ReadStore`-free inputs (I-4 by signature);
`g3_resolution_carries_freshness` (15 §5.1).

### CP-3 · Event log & fold engine
SoA `EventLog` (6 columns), intern tables, `LogRecord` enum, the `Fold`
trait + tuple composition, `ManifestFold`/`FunnelFold`/`LineageFold`,
`reconstruct()`.
**Gates:** R-1 (determinism under interleaving, proptest), R-2 (snapshot
equivalence), R-3 (duplicate immunity); `fold_funnel_1m` bench lands under
budget (13 §6); one-pass multi-fold verified (single scan, N folds).

### CP-4 · Store contract + memory backend + conformance suite ∥
`ReadStore`/`AppendStore`/`Store` supertraits, `MemoryStore`, the generic
conformance library (**C-1..C-10** + R-1..R-4, incl. the revoke-vs-mint-child
race, idempotent-mint, CAS-conflict, and authoritative-miss tests — 15 §5.1).
**Gates:** memory backend passes full conformance; read-only bounds compile-
checked (trybuild); `g4_*` and `g5_*` suites green on memory.

### CP-5 · SQLite backend (the production laptop store — rev 2.2)
`waggle-store-sqlite`: WAL mode, single-writer committer task with two-lane
intake (G-6) and adaptive batch commit; C-3 seq, C-8 nonce dedupe (UNIQUE
index), C-9 CAS (`UPDATE … WHERE version = ?`), committer-owned interning
(G-1) — all inside one transaction per batch; **arc-swap hot cache over the
anchor**; Parquet compaction (two-phase); JSONL `export`/`replay` wire
format; the optional `fs-jsonl` backend.
**Gates:** full conformance green on sqlite **and** fs-jsonl (C-1..C-10);
the **loom suite scoped to the cache layer** (15 §5.2 rev); `it_retry_storm`
crash-point matrix; `it_revoke_mid_swarm`; `it_analytics_flood` (G-6
budget); reader p99 under saturated writes (WAL); cache-hit resolve < 1 µs /
cold read < 50 µs budgets; compaction round-trip; export→replay→reconstruct
≡ (R-1) round-trip; **CAS blob suite (rev 2.3)**: `blob_roundtrip`,
`cas_dedupe`, `inline_threshold_automatic`, CAS GC mark-and-sweep test.

### CP-6 · waggle-mcp + the daemon — the interface ships (rev 2.2)
Tool schema (`mint`/`resolve`/`record`/`funnel`/`mutate`; `share` stub),
**`waggled` tokio daemon** (streamable-HTTP MCP on 127.0.0.1, auto-start,
idle lifecycle) + **stdio proxy shim**, extractor chain (HarnessMeta,
Explicit; A2A card parsing), `waggle-cli` verbs incl. `export`/`replay`.
**Gates:** tool-schema↔core-signature correspondence test (09 §2);
**scenario A (06 §7) as an executable test** over both transports;
**two-clients-one-daemon test** (16 §6 — Claude-Code-like + Codex-like
clients share one store, tier 2); shim-adds-no-semantics conformance;
round-trip p50 < 2 ms; local export→replay round-trip;
`media_variant_by_modality` end-to-end (rev 2.3: `mint --attach` an image +
transcript catch-all, two contexts resolve to MediaRef vs inline, bytes
fetched out-of-band and hash-verified); **the fluency surface (rev 2.4, 17)**:
`map` tool + response envelope (`next`/`hint`/`stats`) with edges declared in
the tool registry — gates: `map_reachability`, `map_reverse_totality`,
`envelope_next_valid`, `map_state_table`, `one_call_mint`,
`hint_on_every_error`; **local security & lifecycle (F-2/F-4, 16 §5)**: Unix
socket default + token-gated TCP (`it_local_auth`), version handshake with
drain-and-restart (`it_version_skew`); the ≤5-line AGENTS.md stub is the
*entire* out-of-band instruction; **catalog completeness (rev 2.5)**: every
shipped verb/tool declared in `waggle-ops`, `ops_inventory_parity` green on
the full surface, `--json` envelope parity CLI↔MCP, generated
COMMANDS.md/man/completions diff-clean.

### CP-7 · Guided query engine
`query(token, path)` — JSON-Pointer subset over manifest/projection/funnel,
slice + guidance (`next` paths), response budgets (≤4 KB default,
`max_bytes` param).
**Gates:** guidance-walk integration test (following `next` reaches every
leaf); budget property (no response exceeds `max_bytes`); `query_slice`
bench under budget.

### CP-8 · Social renderers ∥
`SharePackage`, channel artifacts, OG meta from snapshot (I-3), QR
(`qrcodegen`, `qr`/`qr-png` features), optional `share` MCP tool wired.
**Gates:** `insta` snapshots for every artifact; purity check (same inputs ⇒
byte-identical, property test); wasm32 build stays under size budget.

### CP-9 · The benchmark harness — 0.1 exit
The public numbers (12 §3 Q2 answered with **our** data): scripted
orchestrator task run two ways — context-forwarding vs. token-referenced —
across ≥2 model families; token counts, latency, failure classes; published
as `benches/handoff-report.md` with methodology.
**Gates:** report reproducible from one command; results cited in README;
**0.1 release** — tagged, crates publishable, all CI gates green.

## Phase 0.2 — the edge

### CP-10 · Cloudflare backend + serve
KV/Queues/R2/AE store impl, edge worker (routes incl. remote `/mcp`), sink
worker (dedupe, seq, R2 NDJSON, AE), compaction cron, venue-NAT allowance —
**with the rev-2.1 consistency architecture (08): origin read-through on KV
miss (C-10/G-7), two-path writes (lifecycle → origin CAS, events → queue),
and `strict|eventual` resolve levels (G-8).**
**Gates:** conformance via Miniflare harness (incl. C-8..C-10); scenario B
(06 §7) end-to-end; `it_cross_pop_handoff` and `it_strict_vs_eventual_revoke`
(15 §5.3); redirect p50 < 10 ms local-Miniflare measure;
`strict_resolve_overhead` bench published; reconstruct-vs-AE divergence
bounded test; `it_replay_migration` (16 §6 — local SQLite → cloud via JSONL
replay with an injected mid-replay kill; C-8 dedupe; reconstruct ≡).

## Phase 0.3 — trust

### CP-11 · Signing & attributed resolution
Canonical manifest serialization + Ed25519 detached signatures, signed-card
verification path, capability-URL private tokens, redaction record design,
cascade hardening at scale.
**Gates:** signature round-trip vectors; attributed-resolve integration test;
C-7 soak test (10⁴ concurrent mint-child vs revoke).

## Phase 1.0 — the spec

### CP-12 · Spec + conformance vectors published
The 11 §2 spec document, public selection/parse/reconstruct vectors, schema
freeze, facade crate polish.
**Gates:** vectors pass against the reference implementation from a clean
checkout; a documented walkthrough for a second implementation; semver/schema
annex published.

## Concurrency gap-fix mapping (rev 2.1)

The eight gap-fixes from the adversarial concurrency review live in
**15 §4** (design/tests/impl/verified per gap) with their test suites in
**15 §5**; checkpoint gates above cite them. Quick map: G-3 → CP-2 ·
G-1/G-2/G-6 → CP-5 (loom suite mandatory) · G-4/G-5 → CP-4+CP-5+CP-6 ·
G-7/G-8 → CP-10.

## Tracking table

| CP | Title | Phase | Status | Exit evidence |
|---|---|---|---|---|
| 0 | Scaffold & CI teeth | 0.1 | ✅ done | [run 28917719272](https://github.com/modiqo/waggle/actions/runs/28917719272) — 3-OS matrix + wasm + docs-drift green, commit `1c408ce`; parity guard fails-on-rogue proven by test |
| 1 | Core types & manifests | 0.1 | ✅ done | [run 28918237617](https://github.com/modiqo/waggle/actions/runs/28918237617) — 54 tests incl. properties P1..P8; largest file 354/750 |
| 2 | Sealed matcher | 0.1 | ◐ in progress | vector-suite run |
| 3 | Event log & folds | 0.1 | ☐ not started | R-1..R-3 + bench baseline |
| 4 | Store + conformance | 0.1 | ☐ not started | conformance run (memory) |
| 5 | fs backend | 0.1 | ☐ not started | conformance + crash test |
| 6 | MCP server + CLI | 0.1 | ☐ not started | scenario-A test run |
| 7 | Guided queries | 0.1 | ☐ not started | guidance-walk test |
| 8 | Social renderers | 0.1 | ☐ not started | snapshot suite |
| 9 | Benchmark harness | 0.1 | ☐ not started | handoff-report.md |
| 10 | Cloudflare | 0.2 | ☐ not started | scenario-B test run |
| 11 | Trust | 0.3 | ☐ not started | signature vectors |
| 12 | Spec & vectors | 1.0 | ☐ not started | vectors from clean checkout |

*Update discipline: a CP flips to ✅ only with its exit-evidence link filled
in; partial work is noted in-row (☐ → ◐ with a one-line status). This table
is the single source of truth for progress; the session task tracker mirrors
it.*

## Pre-code gates (before CP-1 merges)

- [x] **Diligence: agent-memory platforms** (10 §5 #13 / 12 §3 Q1) —
  **done 2026-07-08, white space confirmed** (12 §3): memory platforms are
  fact-memory layers; LangGraph's handoff default is full-context forwarding
  — evidence of the problem, not a competitor.
- [ ] Name claim: publish `waggle` placeholder crates (facade + core) to
  crates.io — **awaiting owner's cargo credentials** (the one human-gated
  item; code proceeds meanwhile, the risk is name squatting only).
