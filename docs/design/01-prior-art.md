# 01 — Prior Art, the Gap, and the Adoption Case

*Revision 2. Every claim in §1–§3 is source-linked; claims marked ✅ survived
3-vote adversarial verification in the deep-research pass (see
[12-research-appendix.md](12-research-appendix.md)); claims that failed
verification are quarantined there and not used here.*

## 1. The pain, first — vendor-quantified and verified

The reason this library should exist is no longer an inference:

- ✅ **Multi-agent systems consume ~15× the tokens of chat** (single agents
  ~4×), and **3–10× more than single agents for equivalent tasks** — from
  Anthropic's own self-critical engineering posts, not marketing
  ([multi-agent research system](https://www.anthropic.com/engineering/multi-agent-research-system),
  [when and how to use multi-agent](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them)).
- ✅ The overhead is attributed by the vendor to exactly the mechanism waggle
  replaces: *"duplicating context across agents, coordination messages between
  agents, and summarizing results for handoffs"* — and, flatly: **"Each
  handoff loses context."** Anthropic documented role-specialized subagents
  spending more tokens on coordination than on actual work.
- ✅ Handoff failure is measured: the MAST failure taxonomy attributes
  **~36.9% of multi-agent failures to inter-agent misalignment / context loss
  at handoffs** ([arXiv 2503.13657](https://arxiv.org/abs/2503.13657));
  Anthropic's shipped Research feature had subagents redundantly
  investigating the same topic.
- ✅ Reference-passing demonstrably attacks the cost: a blackboard architecture
  (LbMAS) used **43–72% fewer tokens** than message-forwarding baselines with
  *better* benchmark quality ([arXiv 2507.01701](https://arxiv.org/abs/2507.01701),
  medium confidence — preprint); a MESI-inspired reference/invalidation scheme
  showed 84–95% savings in simulation ([arXiv 2603.15183](https://arxiv.org/pdf/2603.15183),
  low confidence — simulation only, one dissenting verifier).

## 2. The protocol landscape — endorsement vs. usage, kept separate

**A2A** is winning the *standards* race: ✅ 50 → 150+ backing organizations in
one year, Linux Foundation governance, v1.0 (April 2026) with Signed Agent
Cards, SDKs in five languages, ACP absorbed
([Linux Foundation](https://www.linuxfoundation.org/press/a2a-protocol-surpasses-150-organizations-lands-in-major-cloud-platforms-and-sees-enterprise-production-use-in-first-year)),
and default-integration into Azure AI Foundry, Copilot Studio, and Bedrock
AgentCore. **But endorsement ≠ usage**: the claim of verified enterprise
production use was *refuted 0–3* in our verification pass (logo counts), and
practitioner analyses find A2A "alive but narrower than the hype," genuinely
valuable specifically across trust boundaries
([Credal](https://www.credal.ai/blog/what-happened-to-a2a-protocol),
[Glukhov](https://www.glukhov.org/ai-systems/comparisons/a2a-protocol-2026-adoption)).
Meanwhile **MCP is where daily usage lives** (~97M monthly downloads by
March 2026) — and most real multi-agent work in 2026 is *intra-framework*
subagents (Claude Code, LangGraph, CrewAI), where A2A is unused by design.

**The decisive finding on artifacts** (the strongest counter-evidence found,
and the sharpest gap definition): ✅ A2A v1.0 already standardizes an
`Artifact` object — task results SHOULD be returned as Artifacts, whose Parts
support **reference-by-URL** ([spec](https://a2a-protocol.org/latest/specification/)).
So "artifact passing is unstandardized" is false. **However** — verified with
the same rigor — *A2A provides the URL field and defines no resolution
semantics: authentication, lifetime, provenance, and attribution of the
referenced artifact remain implementation-specific and ad hoc.* And A2A's
`contextId` is an opaque correlation ID, not context passing (claim that it
was refuted 0–3).

**Directories and discovery** (AGNTCY/OASF/ADS, ANP) remain agent-centric —
who the agents are, not what travels between them
([AGNTCY](https://docs.agntcy.org/pages/agws/manifest.html),
[ANP](https://agentnetworkprotocol.com/en/specs/08-anp-agent-discovery-protocol-specification/));
the Q1 2026 state: 104k+ agents, 15+ registries, "zero interoperability"
([survey](https://global-chat.io/discovery-landscape)).

**Competitor scan**: no product or OSS project offering attributed,
resolvable artifact references for agent handoffs was confirmed — with the
honest caveat that this is absence of evidence (LangGraph stores/checkpoints
and agent-memory platforms were not conclusively ruled out; open question in
doc 12).

**The real local incumbent is not a product — it is the file path**
(rev 2.4). Verified state of practice in the flagship harnesses: Claude Code
subagents are fully isolated — *"the only way to pass information to a
subagent is through the prompt string… the only way back is the final
response"* ([SDK docs](https://code.claude.com/docs/en/agent-sdk/subagents));
Codex collects parallel subagent results into one response with JSONL
session resume. In both, the de facto artifact mechanism is *write a file,
pass the path in prose* — and both vendors' 2026 features (Dynamic
Workflows' hundreds of parallel subagents; Codex orchestration) multiply
handoff counts without touching the handoff substrate. A path is a 30-byte
reference with **no attribution, no adaptive projection, no lifecycle
(a stale path silently serves wrong data), no telemetry, and no reach beyond
the machine**. That delta is waggle's local value — and it sets the UX bar
that governs 17: minting must cost one call, or agents will keep using
paths and be right to.

## 3. The human-side landscape (context for the capability, not a battlefront)

[Dub.co](https://github.com/dubinc/dub) is the open-source incumbent in link
attribution (AGPLv3 core, conversion/revenue tracking, QR, deep links);
self-hosted shorteners ([Shlink](https://shlink.io/), Kutt) are services with
click analytics; mobile attribution (AppsFlyer OneLink/Branch) does
context-adaptive resolution via device fingerprinting — the surveillance
posture waggle rejects
([AppsFlyer](https://www.appsflyer.com/glossary/deferred-deep-linking/)).
The Rust ecosystem has no embeddable attribution library at all
([`urlshortener`](https://crates.io/crates/urlshortener) is a client for
third-party services).

**Ruling (revision 2, standing):** waggle does not compete with Dub. The
human-facing renderers ship as a *capability* of the same token (05), because
rote needs them and because "the same token unfurls in Slack and hands the
Codex subagent its own variant" is the sentence that makes the primitive
legible. "Dub alternative" appears in no positioning. The comparison survives
only as this paragraph.

## 4. The gap, restated precisely (post-verification)

Waggle is **the resolution and attribution layer for artifact references** —
the layer A2A explicitly left undefined and intra-framework orchestration
never had:

1. **Attributed identity**: who minted, for which channel/role, from which
   parent — retrievable as a manifest, forever.
2. **Resolution semantics**: disposition (active/expired/revoked/superseded),
   deterministic per-consumer projection, anonymous-by-default with coarse
   actor classes.
3. **Provenance and coherence**: event-sourced lifecycle, lineage trees,
   supersede/repair propagation, exact reconstruct.
4. Delivered where usage actually is: **MCP tools** for every harness today,
   plain HTTPS for everything else, an A2A Artifact-URL mapping for wherever
   that curve lands.

## 5. The adoption case — agent-first, stated honestly

**For:** the pain is first-party quantified (§1) and lives in intra-framework
subagent handoffs — reachable through one stdio MCP server with no protocol
adoption asked of anyone; the mitigation-by-reference approach has
directional evidence; the incumbent standardized the *container* and left the
*reference semantics* to us; and a first production user (rote) carries the
requirements list.

**Against (design against these):** the quantitative pillars beyond
Anthropic's are preprints; A2A moves fast — the resolution-semantics gap
"could close within one or two spec revisions" (hence 11's ride-don't-race
posture and the 0.1 speed priority); the competitor scan was inconclusive,
not conclusive (a targeted agent-memory-platform check remains open); and
per-framework handoff mechanics went unverified, so the demand-side
segmentation is unquantified until our own benchmark harness produces
numbers.

**Verdict** (unchanged from the deep-research pass): the opportunity is
substantially real and the window is now — provided 0.1 ships the MCP facade
fast and the benchmark numbers are ours, not borrowed.
