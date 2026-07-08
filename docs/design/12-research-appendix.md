# 12 — Research Appendix: Verified and Refuted Claims

*New in revision 2. The adversarially-verified evidence base from the
deep-research pass (July 2026: 5 search angles, 21 sources fetched, 104
claims extracted, 25 claims through 3-vote adversarial verification — each
claim independently attacked by three skeptic verifiers with primary-source
access; ≥2/3 refutations kill a claim). This appendix is the **citation
policy** for the whole doc set: docs cite only the Verified table; the
Refuted table is the do-not-cite list, kept precisely so those numbers don't
creep back in.*

## 1. Verified claims (cite freely, with the stated confidence)

| Claim | Confidence | Vote | Source |
|---|---|---|---|
| Multi-agent systems use ~15× the tokens of chat (single agents ~4×); 3–10× more than single agents for equivalent tasks | **high** (first-party, self-critical posts) | 3–0, 3–0 | [Anthropic engineering](https://www.anthropic.com/engineering/multi-agent-research-system) · [claude.com blog, Jan 2026](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them) |
| Overhead vendor-attributed to handoff mechanics: "duplicating context across agents, coordination messages, summarizing results for handoffs"; "Each handoff loses context"; coordination exceeded actual work in a role-specialization experiment | **high** | 3–0, 3–0 | [claude.com blog](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them) |
| Handoff/context-loss failures observed in real systems (duplicate subagent work in Anthropic's shipped Research feature); MAST taxonomy: ~36.9% of multi-agent failures = inter-agent misalignment/context loss | **high** | 3–0 | [Anthropic](https://www.anthropic.com/engineering/multi-agent-research-system) · [MAST, arXiv 2503.13657](https://arxiv.org/abs/2503.13657) |
| Naive broadcast context-sync scales multiplicatively: ~2.05M tokens for a 5-agent/50-step workflow over one 8,192-token doc | **medium** (single-author preprint; arithmetic verified) | 3–0 | [arXiv 2603.15183](https://arxiv.org/pdf/2603.15183) |
| MESI-inspired reference/invalidation: 84–95% token savings **in simulation** | **low** (simulation only; 1 dissent) | 2–1 | [arXiv 2603.15183](https://arxiv.org/pdf/2603.15183) |
| LbMAS blackboard (shared-artifact, no direct handoffs): 43–72% fewer tokens than message-forwarding baselines with best average quality (+4.33% over CoT) | **medium** (preprint, self-reported benchmarks) | 3–0 ×3 | [arXiv 2507.01701](https://arxiv.org/abs/2507.01701) |
| A2A trajectory: 50→150+ backing orgs in year one, v1.0 (multi-protocol, multi-tenancy, Signed Agent Cards), Linux Foundation governance, 22k+ stars, 5 SDK languages | **high** (backing/logo counts, not production) | 3–0 ×3 | [Linux Foundation press](https://www.linuxfoundation.org/press/a2a-protocol-surpasses-150-organizations-lands-in-major-cloud-platforms-and-sees-enterprise-production-use-in-first-year) · [a2a-protocol.org](https://a2a-protocol.org/latest/announcing-1.0/) |
| A2A v1.0 standardizes `Artifact` (results SHOULD be Artifacts; Parts support reference-by-URL) — **but defines no resolution semantics: auth, lifetime, provenance/attribution remain implementation-specific** | **high** (verified against spec text) | 3–0 ×4 | [A2A spec §3.7, §4.1.6–4.1.7](https://a2a-protocol.org/latest/specification/) |
| "Ad-hoc integrations are difficult to scale, secure, and generalize" (mid-2025 framing) — being actively closed by protocol consolidation; the surviving unmet need is the attributed/resolvable reference layer, not handoff itself | **high** | 3–0 | [arXiv 2505.02279](https://arxiv.org/html/2505.02279v1) |
| No direct competitor (attributed, resolvable artifact references for agent handoffs) confirmed — **absence of evidence, not confirmed white space** | **low** | — | competitor-scan angle, no surviving claims |

## 2. Refuted claims (DO NOT CITE — anywhere in this doc set)

| Claim | Vote | Why it matters |
|---|---|---|
| 86% (flat) / 72% (linear) context-duplication rates in framework topologies | 1–2 | circulating stat; failed verification |
| Production MAS consume 42–71k tokens/invocation, 29–38% redundant | 0–3 | circulating stat; failed verification |
| PSMAS phase-scheduling achieved 27.3% mean token reduction | 1–2 | failed verification |
| Framework inefficiency structurally caused by simultaneous activation + full-context broadcast (as stated by that paper) | 0–3 | failed verification |
| **Anthropic already runs a shared-artifact reference pattern in production** | 0–3 | the white space is emptier than assumed — and we cannot cite them as validation |
| **A2A in verified enterprise production across industries** | 0–3 | "150+ orgs" is endorsement, not usage — say "backing," never "deployed" |
| **A2A `contextId` provides standardized context-passing for handoffs** | 0–3 | it is an opaque correlation ID; strengthens the gap claim |

Also downgraded by policy (not formally refuted, but unverified): the "~80%
token reduction from stigmergy coordination" GitHub-discussion figure cited
in rev-1 docs — replaced everywhere by the verified chain above.

## 3. Questions the research did not answer (feeding 10 §5)

1. Do agent-memory platforms (Letta/Zep class) or LangGraph
   stores/checkpoints already provide attributed, resolvable references?
   Competitor scan inconclusive → **targeted diligence before 0.1 code
   freeze** (10 §5 #13).
2. Do simulated/benchmarked savings hold in production LLM workloads once the
   coordination protocol's own overhead is counted? → our benchmark harness
   is a 0.1 deliverable for exactly this reason.
3. Per-framework handoff mechanics and adoption numbers (LangGraph, CrewAI,
   AutoGen/AG2, OpenAI Agents SDK, Claude Code, ADK, smolagents): no claims
   survived → demand-side segmentation unquantified.
4. Will A2A v1.x standardize resolution semantics for Artifact URL Parts? →
   the window question; monitored; the 11 §5 A2A proposal is the hedge
   (become the layer before one is invented in-spec).

## 4. Method note

Verification was adversarial by construction: each claim was assigned three
independent verifier agents prompted to *refute* it against primary sources
(spec texts, first-party posts, the papers themselves), with ≥2/3 refutations
killing the claim. Verbatim-quote checks, arithmetic re-derivation, and
source-quality grading were applied. Residual risks: preprint-heavy
quantitative pillars (flagged per-claim above), press-release adoption
metrics (flagged), and time-sensitivity — A2A ships fast; re-run the
protocol-gap checks before the 0.3 spec draft.
