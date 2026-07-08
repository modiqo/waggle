# The waggle documentation map

Four kinds of documents live here, for four kinds of readers. Start
where your question is.

## Why does this exist? *(the case)*

| | |
|---|---|
| [**WHY.md**](WHY.md) | The cornerstone essay: how resources actually pass between agents today (the four-boundary matrix), what it costs, why provider optimizations don't transfer, the bee that inspired the design — and how the substrate works at every radius. |

## How do I use it? *(the guides — in reading order)*

**Getting started**

1. [Five minutes to a first token](guide/01-five-minutes.md) — install, mint, resolve, hand off
2. [waggle in Claude Code](guide/02-claude-code.md) — the MCP wiring, `waggle init`, subagent handoffs

**The core moves**

3. [Variants & media](guide/03-variants-and-media.md) — one token, per-consumer projections; attachments
4. [Lifecycle & query](guide/04-lifecycle-and-query.md) — revoke/supersede/expire; slicing documents by path
5. [The full lifecycle, illustrated](guide/06-the-full-lifecycle.md) — orchestrator → subagents → back, the URI taking shape end to end

**The sharp edges**

6. [Surgical content access](guide/07-surgical-content.md) — read/search *through* the token: the grep travels, the artifact stays
7. [Embedding in Rust](guide/05-embedding-rust.md) — the crates as a library

**Running it**

8. [waggled & federation](guide/08-daemon-and-federation.md) — lifecycle verbs, two-machine setup, strict vs eventual freshness
9. [The edge](guide/09-the-edge.md) — deploy to Cloudflare in 5 minutes; `waggle edge status|push|smoke`
10. [The edge, walked through](guide/10-edge-walkthrough.md) — every command against a real account, with diagrams

## What exactly does it promise? *(the reference)*

| | |
|---|---|
| [**The specification**](../spec/waggle-spec.md) | Normative (RFC-2119): token, three-zone manifest, sealed matcher, log guarantees, storage contract, trust, resolution semantics |
| [Conformance vectors](../spec/vectors/) | Generated FROM the implementation, drift-checked in CI — the portable half of the spec |
| [COMMANDS.md](../COMMANDS.md) | Every operation, every argument — generated from the catalog, drift-checked in CI |
| [PERF.md](../benches/PERF.md) | Measured numbers with their benchmarks: 7.4 ns resolves to 2 ms edge round-trips |

## How was it designed? *(the design docs — the contract)*

The [design corpus](design/) predates the code and governs it. By
concern:

- **The domain** — [02 domain model](design/02-domain-model.md) · [03 core architecture](design/03-core-architecture.md) · [04 event sourcing](design/04-event-sourcing.md)
- **The agent experience** — [06 agent coordination](design/06-agent-coordination.md) · [17 agent fluency](design/17-agent-fluency.md) · [18 content access](design/18-content-access.md) · [05 social minting](design/05-social-minting.md)
- **Storage & concurrency** — [07 storage interface](design/07-storage-interface.md) · [15 concurrency model](design/15-concurrency-model.md) · [16 deployment topologies](design/16-deployment-topologies.md)
- **The edge** — [08 cloudflare foundation](design/08-cloudflare-foundation.md) (incl. the E1-E13 completeness matrix)
