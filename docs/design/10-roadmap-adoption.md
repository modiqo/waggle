# 10 — Roadmap, Adoption Wedges, and Open Questions

*Revision 2. Reordered agent-first per the standing ruling; the MCP facade is
a 0.1 deliverable (arguably THE 0.1 deliverable); bindings deleted; metrics
re-centered; the open-questions table reconciled (several settled).*

## 1. Phased delivery

| Phase | Ships | Proves |
|---|---|---|
| **0.1 — the primitive, in the harness** | `waggle-core` (mint/negotiate/resolve/fold/reconstruct, variants, lineage) · **`waggle-mcp` + `waggle serve --stdio`** (fs store) · `waggle-agent` extractors (HarnessMeta, explicit; A2A card parsing) · `waggle-store` + conformance (C-1..C-7, R-1..R-4) · `memory` + `fs` backends · `waggle-social` renderers (`qr` feature) · CLI | a Claude Code / Codex orchestrator doing token-referenced subagent handoffs on a laptop, **plus our own benchmark harness** publishing token-cost deltas vs. context forwarding |
| **0.2 — the edge** | `waggle-store-cloudflare` + `waggle-serve` (edge, sink, compact cron, **remote `/mcp`**) · AE-backed live funnels · venue-NAT allowance · staging/prod envs | the same core unchanged at the edge; cross-harness scenario B (06 §7) end-to-end |
| **0.3 — trust** | manifest signing (Ed25519, canonical serialization) · signed-card attributed resolution · revocation cascades hardened (C-7 at scale) · private tokens (capability URLs) · redaction record design | agents can trust strangers' tokens |
| **1.0 — the spec** | schema freeze · spec document + public conformance vectors published (11) · facade crate · community backend(s) | infrastructure others build on — and can reimplement |

Order rationale: the deterministic matcher, the reconstruct guarantee, and
the MCP facade are the *claims* — they ship first with CI teeth. The
benchmark harness is in 0.1 because the deep-research pass showed
second-hand token statistics don't survive verification (12): **our numbers
must be ours.** Trust features wait until there's something worth attacking;
the spec freezes only after real deployments shape it.

## 2. Adoption wedges (agent-first, in order)

1. **Intra-harness subagent handoffs** (the wedge). One stdio MCP server, one
   orchestrator recipe, benchmark numbers published. Claude Code and Codex
   first — where the verified pain lives (15×, "each handoff loses context")
   and where zero protocol adoption is asked of anyone.
2. **rote (production user #0).** Play-URI channel links, per-meeting sales
   links, conference QR, funnel metrics — the social capability earns its
   keep here while the agent wedge drives positioning.
3. **Orchestrator framework integrations.** A handoff-channel integration PR
   into one popular framework (LangGraph/CrewAI class), carrying the
   benchmark results — the beachhead becomes a default.
4. **A2A artifact references.** The `x-waggle` mapping published as a
   proposal to their community (11): waggle URIs as the resolution layer for
   Artifact URL Parts. A call option that costs an adapter.
5. **The hosted foundation** as a deploy-in-an-hour template; a managed
   instance only if pull demands it.

Social minting as a market: not pursued (ruling, 01 §3). The dissent
conditions — agent wedge stalls for six months, unforced AGPL-refugee
inbound, or Dub moving onto the agent square — are recorded there and govern
any reopening.

## 3. What the standard track requires beyond code

Owned by [11-standard-track.md](11-standard-track.md): the minimal spec
(manifest schema, resolution semantics, stage vocabulary,
`/.well-known/waggle`, MCP tool schema), public conformance vectors,
the A2A extension proposal, and governance humility (call it a spec with a
reference implementation until two independent implementations exist).

## 4. Success metrics (re-centered)

- **The wedge**: token-cost delta vs. context forwarding in *our published
  benchmark* (the number that recruits everyone else); resolves/day through
  stdio servers; distinct harnesses observed in funnels.
- **The loop**: variant-level funnel usage (authors acting on
  `resolve high / run low` per model family) — evidence the feedback loop is
  real, not just designed.
- **The contract**: external backends passing conformance; reconstruct-vs-hint
  divergence at zero beyond declared staleness.
- **The spec**: a second implementation attempt by someone who isn't us.
- Vanity metrics (crates.io downloads of social renderers): noted, ignored.

## 5. Consolidated open questions

| # | Question | Doc | Status / leaning |
|---|---|---|---|
| 1 | Post-mint variant additions | 02 | leaning: allow via mutation events, 0.3 |
| 2 | `variant` on Event | 02/06 | **settled — adopted** (rev 2) |
| 3 | Variant body typing | 06 | `content_type + bytes` in 0.1; registry later |
| 4 | Lineage depth bound | 06 | **settled — 16** |
| 5 | `perf` features in core | 03 | behind `perf`, off by default |
| 6 | Facade crate | 09 | **settled — yes** |
| 7 | Private tokens mechanism | 08 | capability-suffixed URLs, 0.3 |
| 8 | GDPR redaction record kind | 04 | design in 0.3 with signing |
| 9 | UTM importer | 05 | demand-driven, v2+; not roadmapped |
| 10 | Edition 2021 vs 2024 | 09 | 2021 until MSRV comfort says otherwise |
| 11 | D1 adoption point (CF backend) | 08 | when KV secondary indexes hurt |
| 12 | Spec publication timing | 11 | draft 0.3, publish 1.0 |
| 13 | Agent-memory platforms as competitors (Letta/Zep/LangGraph stores class) | 01/12 | **open — targeted diligence before 0.1 code freeze** |
| 14 | HarnessMeta richness per harness | 06 | survey during 0.1; `Explicit` is the fallback |

## 6. The one-line version

> **waggle** makes one act of sharing a first-class object: a ~30-byte
> attributed reference that replaces context-forwarding between agents —
> minted with provenance, resolved per consumer, replayable to the last
> event — consumed over MCP by any harness and any language, from a JSONL
> file on a laptop to Cloudflare's edge, with the same sans-I/O core.
