# waggle — design documents

*Attributed, resolvable artifact references for agent handoffs — and for humans.
A waggle token is a ~30-byte reference that replaces context-forwarding between
agents and subagents: minted with attribution, resolved adaptively per consumer
(human, bot, terminal, or agent), with every downstream stage event-sourced and
exactly reconstructable. Consumed over **MCP** (stdio or remote) and plain
HTTPS; compatible with A2A Artifact references without depending on A2A.*

> The bee waggle dance: one forager returns from a find, performs an encoded
> signal, and every hive-mate decodes it **according to its own role**, then
> flies to the target. A shared marker, adaptive interpretation per consumer,
> recruitment success observable at the hive. That is this library.

## Status

Design phase, **revision 2** (2026-07-07). Revision 2 incorporates: (a) the
adversarially-verified deep-research findings (12-research-appendix.md), (b)
the strategic ruling to lead with agent coordination and demote social minting
from battlefront to capability, and (c) the **MCP-first delivery pivot** —
waggle is consumed as an MCP server by any language or harness; language
bindings are deleted from the plan. Every prior-art claim carries a source;
statistics that failed adversarial verification are quarantined in the
research appendix and are not cited elsewhere. Anything speculative is marked
**[open]**.

## Reading order

| Doc | Contents |
|---|---|
| [01-prior-art.md](01-prior-art.md) | Verified landscape, the precise gap (A2A left resolution semantics undefined), the agent-first adoption case |
| [02-domain-model.md](02-domain-model.md) | Token, Manifest, Channel, Stage, Event, ResolverContext — and invariants I-1..I-7 |
| [03-core-architecture.md](03-core-architecture.md) | Sans-I/O core, function passing, sealed deterministic matcher, the performance data structures |
| [04-event-sourcing.md](04-event-sourcing.md) | The log as truth: ordering, idempotency, snapshots, the reconstruct algorithm |
| [05-social-minting.md](05-social-minting.md) | The social capability (not battlefront): SharePackage renderers, QR, OG — the same token, human-facing |
| [06-agent-coordination.md](06-agent-coordination.md) | The wedge: subagent handoffs, deterministic variants, lineage trees — with the worked walkthrough (within-harness and cross-harness) |
| [07-storage-interface.md](07-storage-interface.md) | The pluggable Store contract, conformance suite, filesystem (JSONL→Parquet) backend |
| [08-cloudflare-foundation.md](08-cloudflare-foundation.md) | The hosted shape: Workers (core→wasm32 as a build target), KV/Queues/R2/AE |
| [09-crate-layout.md](09-crate-layout.md) | Workspace, the **operations catalog** (one source → MCP schemas · clap CLI · map edges · docs), API↔tool-schema pinning, feature flags, semver |
| [10-roadmap-adoption.md](10-roadmap-adoption.md) | Agent-first phases, wedges, metrics, consolidated open questions |
| [11-standard-track.md](11-standard-track.md) | Becoming a standard: spec-first minimalism, MCP as distribution rail, A2A as adapter, governance humility |
| [12-research-appendix.md](12-research-appendix.md) | The verified/refuted claims tables from the deep-research pass — citation policy for all docs |
| [13-engineering-standards.md](13-engineering-standards.md) | The code constitution: ≤750-line files, trait/polymorphism map, fold-based event sourcing, test pyramid, perf budgets, MRMW persistence, guided slice queries |
| [14-execution-plan.md](14-execution-plan.md) | Checkpoints CP-0..CP-12 with acceptance gates and the tracking table |
| [15-concurrency-model.md](15-concurrency-model.md) | The consistency model, the adversarial scenario catalog (A1–D3), gap-fixes G-1..G-8 with tracking, and the normative unit/loom/integration test specification |
| [16-deployment-topologies.md](16-deployment-topologies.md) | Solo → machine → cloud: the one-line bootstrap, the `waggled` daemon + stdio shim, local auth, version handshake, replay-as-migration |
| [17-agent-fluency.md](17-agent-fluency.md) | The self-teaching tool surface: guidance-in-output envelope, the `map` tool (here · forward · reverse), zero-ceremony defaults, drift-proofing — the skill, computed |

## The design in six sentences

1. A **token** is one act of distribution: a short, non-enumerable name minted
   for a canonical target, bound at mint time to an **attribution manifest**
   (sharer, channel, metadata snapshot, variants) that is retrievable forever.
2. **Resolution is content negotiation generalized**: bots get unfurl metadata,
   humans get a 301, terminals get a text card, and agents presenting their
   context (harness metadata, an A2A Agent Card, or bare JSON) get a
   deterministic **variant projection** matched to their model family,
   harness, modalities, and posture.
3. **Consumption is protocol-shaped, not binding-shaped**: `mint` / `resolve` /
   `record` are MCP tools (stdio for local, remote for teams) and plain HTTPS
   routes — any language and any harness participates with zero waggle code.
4. Everything that happens to a token — impression, click, resolve, and any
   host-reported downstream stage (assess, consent, run, …) — is an **event in
   an append-only log**; counters and funnels are folds over that log and are
   therefore exactly reconstructable.
5. The core crate is **sans-I/O** (no clock, no entropy, no storage — all
   passed in), which is what lets the same code run in the native binary and
   in the Cloudflare Worker unchanged, and what makes every function
   deterministic under test.
6. Storage is a **trait contract with a conformance suite**; the filesystem
   backend (JSONL journal, Parquet compaction — also the store behind the
   local stdio-MCP server) and the Cloudflare foundation are the first two
   implementations, not special cases.

## Two invariants that are the product

- **Count the event, never the data.** `Event` has no payload field. The type
  system, not policy, prevents recipient data from entering the analytics.
- **Same context, same projection.** Variant selection is a pure, total,
  deterministic function — and MCP-first means one implementation performs it,
  so the guarantee is enforced at one point instead of promised at many.

## Revision 2 changelog

- MCP facade (`waggle-mcp`) promoted to the primary interface; npm/pyo3
  bindings deleted; wasm reframed as the Workers build target only.
- Positioning: agent coordination is the wedge; social minting is a capability;
  "Dub alternative" framing removed everywhere.
- A2A: adapter, not anchor — waggle URIs slot into A2A Artifact URL Parts and
  the Agent Card maps through a pluggable extractor; no dependency either way.
- Citation hygiene: unverified ecosystem statistics replaced with the
  adversarially-verified chain (Anthropic 15×/3–10×, "each handoff loses
  context", MAST 36.9%); refuted claims quarantined in doc 12.

## Revision 2.1 changelog (concurrency review)

An adversarial multi-read/multi-write/stale-read scenario review found eight
genuine gaps in rev 2; all are fixed and tracked in **15**: G-1 committer-
owned interning · G-2 the atomic `Arc<ReadState>` snapshot (keystone) ·
G-3 `Resolution.as_of`/`revalidate_after` · G-4 CAS lifecycle mutations ·
G-5 idempotent mint (`mint_nonce`) · G-6 two-lane committer intake ·
G-7 origin read-through on cache miss · G-8 `strict|eventual` resolve
consistency. Storage contract grew clauses **C-8..C-10**; the test pyramid
grew a **loom model-checking layer**; checkpoint gates in 14 now cite the
15 §5 test IDs.

## Revision 2.2 changelog (credible-store pivot)

The local architecture hardened along two axes. **Runtime/transport:** the
local server is now `waggled`, a tokio async daemon (HTTP-MCP on 127.0.0.1)
with `--stdio` demoted to an auto-starting proxy shim — fixing the
per-client-process fragility and enabling cross-harness sharing on one
machine (16). **Storage:** the primary local backend is **SQLite/WAL**
(`waggle-store-sqlite`) — G-1/G-2/G-4/G-5 become provided-by-construction
(WAL snapshot reads, single-writer transactions, `UNIQUE` nonce index, SQL
CAS), the hand-rolled journal machinery is retired, JSONL becomes the
export/replay **wire format** (replay-as-migration, 16 §4), and the loom
suite narrows to the hot-cache layer. The contract (C-1..C-10) and all §5
test suites are unchanged — the contract outranks the mechanism.

## Revision 2.3 changelog (multimodal / MediaRef)

Images, voice, and binary artifacts: **bytes never ride the log** — variant
bodies and targets are either inline (≤ ~64 KB, the range where SQLite beats
the filesystem) or a **`MediaRef`** `{uri, content_type, size, sha256}`
pointing into a **content-addressed store** (`~/.waggle/blobs/` at tiers
1–2; R2 at tier 3 — same layout). Delivery is out-of-band (resolve returns
URL + hash; the ≤4 KB tool-response budget survives); integrity is the
sha256 (verify-what-you-fetched); dedupe and resumable blob migration come
free from content addressing (16 §4). Multimodal consumers ride the existing
variant matcher — image to the vision agent, voice to the audio agent,
transcript to the catch-all (06 §2) — with per-variant funnel telemetry.
New tests: `blob_roundtrip` · `cas_dedupe` · `inline_threshold_automatic` ·
`media_variant_by_modality` (15 §5.1; gates on CP-5/CP-6).

## Revision 2.4 changelog (local-credibility audit — F-1..F-4)

The pre-flight audit found four unmitigated local-harness risks; all closed:
**F-1 (adoption — the big one)** → doc 17, the **fluent tool surface**: a
normative response envelope (`next` = executable schema-valid calls, `hint`
on every error, `mint`'s next[0] = the handoff line), the **`map` tool**
("I am here — forward paths · reverse paths", derived from manifest+funnel
state), zero-ceremony one-call mint, and a ≤5-line stub as the *entire*
skill footprint — instruction is generated from the tool registry, so it
**cannot drift** the way skills do. The file-path-as-incumbent analysis and
UX bar landed in 01 §2. **F-2** → Unix-socket default + token-gated TCP
(16 §5). **F-3** → three-OS CI matrix (13 §10). **F-4** → version handshake
with drain-and-restart (16 §5). CP-6 gates updated; fluency test suite in
17 §5 / 15 §5.

## Revision 2.5 changelog (operations catalog — CLI/tool shape)

One **operations catalog** (`waggle-ops`: name · surface · kind · canonical
agent-first description · args · forward/reverse edges · core_fn) becomes
the single source from which four projections are generated or
parity-tested: **MCP tool schemas** (generated), **the clap CLI** (derive
with doc-comments-as-help, `ops_inventory_parity` introspects the built
Command tree against the catalog in both directions), **the `map` edges**
(read directly), and **docs/completions/man pages** (`xtask gen-docs`,
CI-diffed). Every CLI verb emits the MCP envelope under `--json` — one
shape, one voice, for humans and agents. Rationale vs rote's hand-rolled
parser: clap + introspection parity buys the same single-source guarantee
with far less code. Gates in 13 §10; CP-0 proves the guard fails correctly
before it guards anything.
