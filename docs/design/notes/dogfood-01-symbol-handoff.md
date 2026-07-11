# Dogfood 01 — the symbol handoff, live (2026-07-11)

*The first H1/H2 evidence entry for docs 19/20: a real orchestrator →
subagent delegation over this repository, on the `feat/symbol-lens`
build. Conditions: isolated store (`WAGGLE_DIRECT=1`, scratch
`WAGGLE_STORE`), CLI transport, **unsealed** handoff — the artifact sat
readable on disk throughout, so the side door was open and only the
prompt kept it shut.*

## Setup

- **Artifact:** `crates/waggle-core/src/contract.rs` (this repo, live)
- **Mint:** `waggle mint --target file://…/contract.rs --snapshot
  --require symbol:evaluate --tag name=contract-rs` → token `9u6KEr6F`
- The `symbol:evaluate` requirement resolved at mint via tree-sitter to
  lines **213–226** and entered the signed contract; the outline
  extracted **28 symbols**; overview lenses: `lines, search, symbol`.
- **Task handed to a fresh subagent** (its prompt contained ONLY the
  handoff line, the env prefix, the verb list, and two questions whose
  answers live inside the required region): derive `evaluate`'s
  permille arithmetic for 2-of-3 regions at threshold 667, and quote
  its `# Panics` section. Rules: waggle CLI only, no direct file reads,
  `record --stage run` when done.

## Receipts

| Stage | Receipt |
|---|---|
| baseline | coverage `met: false`, miss **named** (`evaluate @ 213-226`); funnel `{read: 1}` — the orchestrator's own overview peek, honestly counted; outcome `pending` |
| subagent (5 calls) | `resolve` → `read --symbol evaluate` → `search FULL_COVERAGE_PERMILLE` → `search '# Panics'` → `record run` |
| after | coverage `met: true` (1/1, permille 1000, `missed: []`); funnel `{resolve: 1, read: 4, run: 1}` |
| judgment | `record accepted` → outcome `accepted` |

Funnel arithmetic closes exactly: orchestrator's 1 overview read +
subagent's 1 symbol read + 2 searches = `read: 4`; `resolve: 1` and
`run: 1` are the subagent's alone.

## The answers (both correct)

1. `permille = 2 × 1000 / 3 = 666` (truncating division), `met =
   666 ≥ 667 = false` — derived from the actual code in the required
   region.
2. The `# Panics` section quoted verbatim: *"Never in practice:
   `touched ≤ required ≤ 8` keeps the permille within `u16`; the
   `expect` documents the invariant."*

## What this is evidence for

- **Verification without trust (19's thesis).** The subagent's
  self-report ("five commands, no direct reads") was corroborated
  *independently and exactly* by the funnel and the coverage flip —
  the orchestrator never had to believe the report.
- **H1 (orientation).** The subagent reached the load-bearing region
  in ONE content call (`read --symbol evaluate`) — no overview needed,
  no window guessing, no grep-then-guess loop.
- **H2 (contracts become writable).** `symbol:evaluate` was declared
  in one flag and evaluated by machinery that never learned anything
  new — the P1 bitmask did all the work.
- **The carrot beats the stick (one favorable data point).** Unsealed
  handoff, file on disk, and the subagent still went through the
  token: the prompt framed the token as its working context and the
  symbol lens made that the easiest path. Conditions were favorable
  (strong model, explicit rules); the sealed-vs-unsealed reliability
  number across many runs remains the metric that matters (19 §4.8).

## Caveats / next

- Isolated CLI-direct store; the daemon + MCP-tool path was not
  exercised (the session's mounted MCP server ran the older installed
  daemon). Re-run through freshly mounted MCP tools after a daemon
  restart.
- One run, one model, instruction-following conditions. The dogfood
  series should add: an under-instructed subagent (does the stub +
  envelopes alone suffice?), a sealed run, and a deliberately lazy
  prompt to watch the funnel catch a non-reader.
