# 13 — Engineering Standards: the Code Constitution

*New in revision 2. These are the non-negotiable engineering rules for the
waggle crate set. Every rule here is CI-enforceable or bench-measurable —
standards that can't be checked are opinions, and this document contains no
opinions. The execution plan (14) cites these rules as acceptance gates.*

## 1. Code organization

- **Crates split by function, not by size** (09 §1): core (domain), social
  (renderers), agent (extractors/lineage), store (contract+conformance),
  store-fs, store-cloudflare, mcp (tool schema + server plumbing), serve,
  cli, facade. A crate earns existence by having a dependency boundary worth
  policing, never by file count.
- **No source file exceeds 750 lines** (rustfmt-formatted, comments included,
  `#[cfg(test)] mod tests` excluded — tests may live beside code or in
  `tests/`). Enforced by an `xtask lint` rule in CI. Approaching the limit is
  a design signal: split by concept, never by scissors.
- **One concept per module.** A module's name states its concept
  (`token.rs`, `matcher.rs`, `fold.rs`); if a module needs "and" to describe
  itself, it's two modules.
- **Re-export discipline**: each crate's `lib.rs` is a facade under ~100
  lines — module declarations, curated `pub use`, crate docs. Deep paths
  (`waggle_core::manifest::variant::MatchExpr`) never appear in public API;
  the curated surface (`waggle_core::MatchExpr`) is the contract that
  `cargo-semver-checks` guards.
- **Visibility as documentation**: `pub(crate)` by default; `pub` is a
  promise. No `pub` field on any type with an invariant (constructors
  validate; fields stay private).

## 2. Documentation

- `#![deny(missing_docs)]` on every library crate. Every public item carries:
  what it is, when to use it, an example where non-obvious, and — where it
  participates in an invariant — a **citation** (`/// Upholds I-2: …`,
  `/// Store contract C-4.`). The invariant IDs in docs 02/04/07 are the
  shared vocabulary between design docs, rustdoc, and test names.
- **Crate-level docs are mini-guides**: each `lib.rs` doc block explains the
  crate in ≤60 lines with one complete, compiling example (doc-tested).
- **Doc-tests are executable spec**: the walkthrough in 06 §7 exists as a
  doc-test on `waggle-mcp` (scenario A, memory store) — if the story in the
  docs stops compiling, CI says so.
- Comments in code follow the same rule as rote's house style: explain *why*
  and constraints, never *what*; one line where possible.
- **CLI self-documentation (rev 2.5)**: the CLI is a clap-derive projection
  of the operations catalog (09 §2). Doc comments on the derive structs ARE
  the help text and MUST be the catalog's canonical description verbatim
  (parity-tested). Conventions: kebab-case commands and flags; every arg
  documented; every verb supports `--json` emitting the MCP envelope
  (17 §2); `xtask gen-docs` regenerates `COMMANDS.md`, man pages
  (`clap_mangen`), and shell completions (`clap_complete`) — committed and
  CI-diffed, so `--help`, the MCP description, the map, and the docs are one
  string that cannot fork.

## 3. Type-system usage: traits, inheritance, polymorphism — with intent

The design is deliberately polymorphic in four places and monomorphic
everywhere else:

| Seam | Mechanism | Why this mechanism |
|---|---|---|
| Effects (entropy) | `trait Entropy` blanket-impl'd for closures | function-passing; zero-cost; test doubles are literals |
| Storage | **supertrait split**: `trait ReadStore { manifest·scan·children }`, `trait AppendStore { append }`, `trait Store: ReadStore + AppendStore` | trait inheritance where it means something: read-only consumers (funnel queries, the resolver) take `&impl ReadStore` and *cannot* write — I-4 as a type bound |
| Context extraction | `trait ContextExtractor` + an ordered extractor chain | open set of input schemas (harness meta, A2A card, explicit) behind one seam |
| Folds (event sourcing) | `trait Fold { type State; fn init(); fn apply(&mut State, &LogRecord); fn finish(State) -> Self::Out; }` with tuple composition (`impl Fold for (A, B)`) | replay is generic: `replay(log, (ManifestFold, FunnelFold, LineageFold))` runs all folds in **one pass**; adding an analytic is a new `Fold` impl, never a new scan |
| Variant matching | **sealed** trait (03 §3) | polymorphism deliberately forbidden — determinism (I-2) is the product |

Rules: static dispatch by default (generics, enums for closed sets like
`LogRecord`); `dyn` only at host edges (the MCP server's extractor chain);
newtypes for every domain scalar (`Token`, `StageId`, `Seq`, `Timestamp` —
a bare `u64` in a public signature is a review rejection); typestate on the
`MintSpec` builder only if it stays readable — validation-at-`mint()` is the
fallback, documented either way.

## 4. Event sourcing, pure Rust

- The log is the truth (04); **folds are the only read model**. Every
  counter, view, and report is `Fold::finish(replay(...))` — no side-channel
  state, ever. The conformance suite's R-4 (views ≡ fold) is the enforcement.
- Folds are pure: `apply` takes `&LogRecord`, no I/O, no clock. Commutativity
  across tokens and seq-order within a token (04 §2) are **property-based
  tests** (proptest: random record interleavings respecting per-token seq →
  byte-identical states).
- `LogRecord` is a closed enum (static dispatch, exhaustive matches); new
  record kinds are additive and old folds must compile against them
  (`#[non_exhaustive]` + a mandatory `_ => {}` policy documented per fold:
  *ignoring unknown records is correct by design* — that's what makes the
  schema annex (09 §6) honest).

## 5. Testing: the pyramid, named

| Layer | Tool | What it proves | Gate |
|---|---|---|---|
| Unit | std `#[test]`, per module | each module's contract | per-CP in 14 |
| Property | `proptest` | token alphabet/no-bias, seq monotonicity, fold commutativity (R-1), dedup idempotency (R-3) | core ≥ 12 properties |
| Conformance | `waggle-store-conformance` (generic) | C-1..C-7, R-1..R-4 per backend | every backend, every CI run |
| Selection vectors | data-driven tables (the 11 §4 vectors) | matcher determinism incl. ties/near-misses | vectors file is the source; doc examples generated from it |
| Snapshot | `insta` | renderer byte-stability (05) | all `ChannelArtifact`s |
| Integration | spawn `waggle serve --stdio`, drive real MCP frames over stdio; Miniflare harness for CF (08 §6) | the tool schema end-to-end; scenario A as a test | 14 CP-6/CP-10 |
| Model checking | `loom` | the ReadState swap + watermark publication + two-lane intake under all explored interleavings (15 §5.2) | loom suite green; required for CP-5 |
| Fuzz | `cargo-fuzz` | manifest parser, token parser, matcher on arbitrary contexts | no crashes, 10⁶ execs in CI-nightly |
| Doc-tests | rustdoc | every public example compiles and runs | `#![deny(missing_docs)]` + doctest pass |

Coverage: ≥85% line coverage on `waggle-core` (llvm-cov, informational
elsewhere). Test names cite invariants (`i1_event_has_no_payload_field`,
`c4_duplicate_seq_dedupes`).

## 6. Performance: measured, budgeted, regression-gated

- **Criterion benches** live in `benches/` per crate; the canonical suite:
  `mint` (incl. entropy), `resolve` (manifest in hand), `fold_funnel_1m`
  (1M-event SoA scan), `append_p50` (fs group commit), `query_slice`
  (§8), `reconstruct_100k`.
- **Budgets** (initial targets — revised only by benchmark PR, never by
  drift): resolve < 1 µs (cache-hit path; cold SQLite read < 50 µs), fold 1M
  events < 10 ms, mint < 5 µs excluding entropy syscall, durable-append ack
  p50 < 1 ms (idle path, NVMe — adaptive batch commit, §8) / p99 < 5 ms
  under saturation, relaxed-durability `record` ack p50 < 50 µs, local MCP
  tool round-trip p50 < 2 ms (daemon HTTP or stdio-shim path). CI compares against
  committed baselines (`critcmp`-style) and fails on >10% regression.
- **wasm size budget**: core+agent+social ≤ 400 KB gzipped for the Workers
  build (08); tracked per PR by xtask.
- **A `stats` surface, not a metrics dependency**: every fold can emit a
  `Stats` struct (counts, distinct tokens, bytes scanned, elapsed); the
  MCP `funnel`/`query` tools return `stats` alongside results so
  *measurability is a user feature*, not an ops afterthought. A `metrics`
  facade integration is feature-gated (`metrics`) for hosts that want it.

## 7. Data structures (binding decisions, from 03 §4 + additions)

- `Token`: inline `Copy`, 24 bytes, no heap.
- Interning: `StageId(u16)` / `TokenId(u32)` append-only tables; analytics
  never compares strings.
- `EventLog`: **struct-of-arrays**, six columns (`token_id · stage_id ·
  actor · variant · at_ms · seq`), 1:1 with the Parquet schema; folds are
  sequential scans over 2–4 byte columns.
- Manifests: `Arc<AttributionManifest>` in an `arc-swap` cell (`perf`
  feature; `RwLock<Arc<…>>` fallback) — readers never block.
- Small collections: `SmallVec` for per-token row indexes and variant lists
  (typical N ≤ 8).
- Strings: `CompactString` for manifest text; **no strings in events** (I-1
  makes the hot path allocation-free).

## 8. Persistence: multi-read / multi-write, precisely defined (rev 2.2)

The concurrency model is **many concurrent readers, many concurrent write
*submitters*, one commit point**. Rev 2.2's decision: **the correctness
anchor is SQLite in WAL mode, not hand-rolled machinery** — clever
concurrency code we didn't have to write is the best kind. The full model,
scenario catalog, and gap-fix tracking live in **15**; the structure:

```text
writers (MCP tool calls in the tokio daemon — any task)
   │  TWO-LANE mpsc submit (G-6): durable lane (mint · lifecycle mutations ·
   │  durable records) + relaxed lane (analytics events) — committer drains
   │  durable-first with an anti-starvation quantum; a mint never queues
   │  behind an analytics storm
   ▼
single committer task (the sole SQLite writer): one transaction per batch —
mint_nonce dedupe via UNIQUE index (C-8) · lifecycle CAS via
`UPDATE … WHERE version = ?` (C-9) · per-token seq (C-3) · interning (G-1)
· append + view update · COMMIT (C-1: ack ⇒ committed). Adaptive batching:
commit immediately when the queue is empty; batch when busy.
   │  relaxed-lane events: ack on enqueue, durable next commit window.
   ▼
reads, two paths:
   • correctness path — WAL snapshot transactions: N readers, never blocking
     the writer, each seeing one consistent commit prefix NATIVELY. This is
     G-2 provided-by-construction; manifest/event skew is impossible within
     a read txn.
   • hot path — an in-memory arc-swap manifest cache over the anchor for the
     sub-µs resolve budget: invalidated in-process by the committer on
     write; a CACHE, never the correctness mechanism. If it is ever wrong,
     the anchor isn't.
   • compaction (04 §5): aged rows → Parquet, two-phase, then DELETE.
```

Properties, stated as tests: submitters never block each other (only await
their ack); readers never block or get blocked (WAL); crash loses only
unacked work (SQLite recovery + our replay tests on top); reader latency is
unaffected by write bursts (bench: p99 read under saturated write load).
The **loom suite's scope narrows to the cache layer** (publication/
invalidation ordering) — the anchor's concurrency is SQLite's, not ours.
Cross-*process*: WAL makes concurrent readers from other processes safe (a
bonus the journal design never had), but writes still go through the owning
daemon over MCP/HTTP (16); the Cloudflare backend keeps the same model via
Queues (the queue is the mpsc) with lifecycle writes CAS-ing the origin
store directly (08).

## 9. Low-latency querying: slices with guidance, never dumps

The rote lesson, applied to waggle itself: **a consumer should never pull a
whole document to extract one field.** New capability (added to 09's tool
table and the spec surface in 11 §2):

- `query(token, path)` — a JSON-Pointer-subset path into the manifest, a
  resolution projection, or a funnel report; returns the **slice** plus
  **guidance**: the immediate child keys available, and suggested next paths
  (`"next": ["/variants/0/match", "/funnel/by_stage"]`). Agents navigate by
  querying, not by loading.
- Response budgets: every MCP tool response targets **≤ 4 KB** by default;
  larger payloads return a truncation notice + the paths to fetch the rest.
  Budgets are parameters (`max_bytes`), not magic.
- Event access is **never** raw dumps: funnels return aggregates; `scan`
  exposure is seq-windowed (`from_seq`, `limit`) for the reconstruct path
  only.
- Bench (`query_slice`) and an integration test assert the budget holds and
  the guidance round-trips (an agent following `next` paths reaches every
  leaf).

## 10. Consolidated CI gate list

**OS matrix (rev 2.4, F-3): every gate below runs on `ubuntu-latest`,
`macos-latest`, and `windows-latest`** — local harnesses mean laptops, and
Unix sockets, path handling, and file locking all vary; Windows breakage
must be found by CI, not by the first user we most wanted to impress.
(Miniflare and wasm-size jobs run on Linux only; everything touching the
daemon, the store, or the CLI runs on all three.)

fmt · clippy (curated pedantic) · **xtask file-size lint (≤750)** ·
`deny(missing_docs)` + doctests · unit + property suites · conformance
(memory, sqlite, fs-jsonl; Miniflare for CF from 0.2) · selection vectors ·
snapshot tests · fuzz (nightly) · coverage floor (core ≥85%) · criterion
regression (>10% fails) · wasm32 build + size budget · `cargo-deny` ·
`cargo-semver-checks` · tool-schema↔signature correspondence (09 §2) ·
**fluency gates (17 §5): map reachability/reverse-totality,
envelope-next-valid, one-call mint, hint-on-every-error** ·
**catalog gates (rev 2.5, 09 §2): `ops_inventory_parity` (clap tree ↔
OPERATIONS, both directions) · generated-docs/completions/man-pages
diff-clean · CLI `--json` envelope parity with MCP**.
