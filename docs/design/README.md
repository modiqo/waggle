# The design corpus

*Attributed, resolvable artifact references for agent handoffs — and for
humans. A waggle token is a ~30-byte reference that replaces
context-forwarding between agents and subagents: minted with attribution,
resolved adaptively per consumer, with every downstream stage
event-sourced and exactly reconstructable.*

These documents predate the code and govern it — the parts that are
materially important to understanding *what waggle is and why it is
shaped this way*. (Internal process documents — execution plans,
engineering standards, research appendices — live in the project's
design workspace, not in this repository.)

Read in this order if you're new:

| Doc | What it settles |
|---|---|
| [essay](essay.md) | The long-form front door (formerly the repository README): the dance, the problem, the sixty-second demo, the architecture at all three radii |
| [02 — domain model](02-domain-model.md) | Tokens, the three-zone attribution manifest, variants, channels, lineage: the nouns and their invariants |
| [03 — core architecture](03-core-architecture.md) | The sans-I/O discipline: time as a value, entropy as a parameter, effects at the edges |
| [04 — event sourcing](04-event-sourcing.md) | The append-only log as truth; payload-free events; the reconstruct guarantees R-1..R-4 |
| [06 — agent coordination](06-agent-coordination.md) | The handoff choreography: orchestrators, subagents, resolver contexts, the sealed matcher |
| [17 — agent fluency](17-agent-fluency.md) | Why the tools teach themselves: the envelope, `next` steps, the live `map` |
| [18 — content access](18-content-access.md) | Read/search *through* the token: lenses, budgets, the format boundary |
| [07 — storage interface](07-storage-interface.md) | The store contract C-1..C-10 and the conformance suite that defines "backend" |
| [15 — concurrency model](15-concurrency-model.md) | Single-writer commit points, the cache layer, the G-series gap fixes |
| [16 — deployment topologies](16-deployment-topologies.md) | One machine, many machines, the daemon, the shim principle |
| [08 — cloudflare foundation](08-cloudflare-foundation.md) | Computation-travels-to-data; the Durable Object decision; the E1-E13 completeness matrix |
| [05 — social minting](05-social-minting.md) | The human face: unfurls, mint-time snapshots (I-3), QR, share packages |
| [19 — interrogation telemetry](19-interrogation-telemetry.md) | *(plan)* Convergence classification of consumer traces, receipt-driven routing, scaffold distillation — the research grounding and the complete scoping |
| [20 — the symbol lens](20-symbol-lens.md) | *(design)* Source-code handoffs: mint-time symbol outlines via tree-sitter tags, `symbol:` contracts, zero serve-time cost — hypothesis, performance budget, reference pseudo-code |
| [21 — the resource projection](21-resource-projection.md) | Why the verbs are MCP tools (the control-hierarchy ruling), and the complement: resources/list+read as projections of resolve, subscriptions pushing lifecycle corrections to holders |
