<p align="center">
  <strong>waggle</strong><br>
  <em>Agents already pass references. Waggle makes the reference intelligent.</em>
</p>

<p align="center">
  <a href="#the-dance">Why "waggle"</a> ·
  <a href="#the-problem">The problem</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#status">Status</a> ·
  <a href="docs/design/">Design docs</a>
</p>

---

## The dance

A honeybee returns from a find. On the vertical comb, in the dark, she
performs a figure-eight dance — the **waggle dance** — whose angle encodes
direction, whose duration encodes distance, whose vigor encodes quality.
She does not carry the field to the hive. She carries a **reference**.

And here is the part that matters: every bee that reads the dance decodes it
**according to its own role and state**, then flies to the target itself.
One shared marker. Adaptive interpretation per consumer. Recruitment success
observable at the hive.

Twenty million years before context windows, evolution solved the handoff
problem — and it did not solve it by pasting the meadow into the prompt.

## The problem

We are entering the world of agent harnesses: Claude Code orchestrators
fanning out hundreds of subagents, Codex sessions delegating in parallel,
cross-vendor agents discovering each other over open protocols. And every
one of these handoffs, today, works the same way: **forward the context and
hope**.

The costs are measured, not hypothetical. Multi-agent systems consume ~15×
the tokens of a chat session — with the overhead attributed by the vendor
itself to *"duplicating context across agents, coordination messages between
agents, and summarizing results for handoffs."* Their words: **"Each handoff
loses context."** Roughly 37% of multi-agent failures trace to exactly this
seam. (Sources and adversarial verification of every number:
[docs/design/12-research-appendix.md](docs/design/12-research-appendix.md).)

The workaround every practitioner reaches for is the file path —
`/tmp/analysis.md`, pasted in prose. A path *is* a 30-byte reference, and
that instinct is correct. But a path has **no attribution** (who made this,
from what), **no adaptation** (the 4k-context model gets the same 9,000
tokens as the frontier model), **no lifecycle** (a stale path silently
serves wrong data forever), **no telemetry** (which subagent actually read
its input? which stalled?), and **no reach** (it dies at the machine
boundary).

## How it works

Waggle is the reference, made first-class. A **token** is a ~30-byte
attributed name for an artifact, minted in one call:

```
mint { target: "ws://analysis/market-report.md" }
  → wg:7Kp2…
  → next: hand off with — "resolve wg:7Kp2 via waggle for your working context"
```

Behind the token, an **attribution manifest**: who minted it, for which
channel, from which parent (delegation forms a lineage tree), with
**variants** — different projections for different consumers. When an agent
resolves the token it presents its context (model family, harness,
modalities, posture — or an A2A Agent Card), and a **sealed, deterministic
matcher** returns *its* projection: the section index for the frontier
model, the executive summary for the small one, the fail-closed instructions
for the CI runner, the image for the vision agent and the transcript for the
one without eyes.

Everything that happens — every resolve, every downstream stage, every
revocation — is an **event in an append-only log**. Funnels are folds over
that log; any statistic is exactly reconstructable; and events carry **no
payload by construction** — the type system, not a policy page, keeps your
data out of the analytics.

```text
one waggle token
├── for humans     unfurls in Slack, renders as a QR, 301s in a browser
├── for agents     resolves to the variant matched to what they are
├── for the author attribution, funnel, revoke/supersede — observability
│                  no orchestrator has today
└── for the swarm  a lineage tree: who handed what to whom, replayable
```

**Consumption is protocol-shaped**: waggle is an MCP server. One config line
in Claude Code, Codex, Cursor, or anything MCP-speaking — no SDK, no
language bindings, no accounts. Locally it is one binary and a SQLite file
(`waggled`, the daemon every harness on your machine shares). The same
tokens later graduate to the edge (Cloudflare Workers) by **replaying the
log** — migration is a stream, because the log is the truth.

```bash
# the bootstrap, in its entirety
just dev-install                                  # (crates.io release at 0.1)
claude mcp add waggle -- waggle serve --stdio
```

Behind that line, `waggle serve --stdio` is a shim onto **`waggled`** — a
shared daemon on a unix socket that owns the store. Every harness on your
machine lands on the same tokens: what a Claude Code session mints, a
Codex session resolves.

## Getting started

Sixty seconds from a checkout:

```bash
just dev-install
waggle mint --target "file:///$PWD/README.md"
```

```json
{
  "result": {
    "token": "b2uQyZUC",
    "handoff": "resolve b2uQyZUC via waggle for your working context",
    "variants": 1
  },
  "next": [
    { "tool": "resolve", "args": { "token": "b2uQyZUC" },
      "why": "self-check the projection consumers will receive" }
  ]
}
```

That `handoff` line is what you give a teammate — human or agent — instead
of the file's contents. Then walk the loop the way a consumer would:
`waggle resolve --token …` → `waggle record --token … --stage run` →
`waggle funnel --token …` → `waggle map --token …`. Every response carries
executable `next` steps; if you're ever unsure, `map` tells you where you
are and what your paths forward and back are.

**The guide** (real commands, real outputs):

1. [Five minutes to your first handoff](docs/guide/01-five-minutes.md) — mint → resolve → record → funnel → map
2. [Wiring into Claude Code & any MCP harness](docs/guide/02-claude-code.md) — one config line, the 5-line agent stub, the orchestrator pattern
3. [Variants & media](docs/guide/03-variants-and-media.md) — one token, the right projection per consumer; images by modality
4. [Lifecycle, attribution & guided query](docs/guide/04-lifecycle-and-query.md) — supersede/revoke with CAS, funnels, slices under byte budgets
5. [Embedding in Rust](docs/guide/05-embedding-rust.md) — the sans-I/O core, the store contract, reconstruct
6. [The full lifecycle](docs/guide/06-the-full-lifecycle.md) — one mission followed end to end: lineage, projections, slices, and the correction that reaches late readers (**`just demo` runs it live**)
7. [Surgical content access](docs/guide/07-surgical-content.md) — grep through the token: `search`/`read` with lenses, budgets, and snapshots that outlive the file

Still landing before 0.1: the published handoff benchmark and the
crates.io release (names are claimed).

| Crate | Role |
|---|---|
| `waggle-core` | sans-I/O domain: tokens, time-as-value, entropy injection |
| `waggle-ops` | the operations catalog — one source, four projections |
| `waggle-agent` | resolver-context extraction (harness metadata, A2A cards) |
| `waggle-social` | the human face: unfurls, share packages, QR |
| `waggle-store*` | the storage contract + SQLite/JSONL/Cloudflare backends |
| `waggle-mcp` | the MCP projection: tool schemas, envelope, transports |
| `waggle-cli` | `waggle` verbs + `waggled`, the local daemon |

```bash
just dev-install   # build & install the CLI from this checkout
just preflight     # fmt-check · clippy -D warnings · file-size lint · tests · wasm
```


## What makes it credible

This repository is design-first and unusually explicit about its own
discipline — the [design docs](docs/design/) are the contract:

- **Sans-I/O core** — no clock, no entropy, no storage in the domain crates;
  every effect is a parameter. The same code runs in the native daemon and
  in Workers wasm, and every function is deterministic under test.
- **Deterministic adaptivity** — same context, same projection, always;
  the variant matcher is sealed so the trust claim survives.
- **Event-sourced with a reconstruct guarantee** — counters are cache; the
  log is truth; replay-equivalence is a CI property, not a slogan.
- **One operations catalog** — the MCP tools, the clap CLI, the `map`
  navigation, and `COMMANDS.md` are four projections of one table, with
  parity tests that fail the build on drift. The tools teach the agent
  themselves (`map`: *"I am here — what are my forward and reverse paths?"*)
  so instruction cannot rot the way skills do.
- **Adversarially reviewed before code** — the concurrency model survived a
  scenario-by-scenario attack (eight gaps found, fixed, and test-specified);
  the market claims survived a 103-agent verification pass that killed seven
  circulating statistics we now refuse to cite.

## Status

**Pre-0.1 — usable.** The full local loop works end to end and every claim
below is a passing test in CI (three-OS matrix + wasm; ~105 tests;
[execution plan](docs/design/14-execution-plan.md) tracks each gate):

- **mint → handoff → resolve → record → funnel → map** over MCP and CLI —
  scenario A from the design docs runs as an integration test on real
  JSON-RPC frames over the real SQLite store;
- the **sealed matcher** serves each consumer its variant (selection-vector
  table + determinism over 10⁴ random contexts);
- the **event log reconstructs** (shuffle-immune, duplicate-immune,
  snapshot+suffix ≡ full — R-1..R-4 as property tests);
- three backends pass one **conformance suite** (memory, SQLite/WAL,
  JSONL journal), with CAS mutations, idempotent mint, revoked-parent
  rejection, and export→replay migration proven — plus a content-addressed
  **blob sidecar** with verified reads and GC;
- `waggle serve --stdio` is a **working MCP server**: the test spawns the
  real binary, speaks the protocol through its pipes, and reads the writes
  back from a second process — and `waggled` (unix socket) serves **many
  harnesses over one store**: the two-clients test has a Claude-like and a
  Codex-like session exchanging a token through their own shims;
- **measured, not promised** ([benches/PERF.md](benches/PERF.md)):
  cache-hit resolve read **39 ns**, durable event append **39 µs**
  (real fsync), a million-event funnel fold in **334 µs** — every
  design-budget beaten with 25–30× headroom.

## License

MIT OR Apache-2.0, at your option.

---

<p align="center">
  <em>She never carries the field home. She dances, and the hive knows.</em>
</p>
