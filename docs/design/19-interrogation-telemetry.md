# 19 — Interrogation telemetry: convergence, receipt-driven routing, and trace distillation

*Status: technical plan — approved design, pre-implementation. This
document extends the corpus the way 17 (agent fluency) and 18 (content
access) did: it commits the design before the code, and the code will be
held to it. Sections 1–3 make the case with the research it stands on;
sections 4–9 scope the work completely against the existing invariants;
section 10 is the phased delivery plan.*

---

## 0 · The claim

Waggle already records *that* a consumer touched a handoff — the funnel
counts resolve, read, run. This document commits waggle to recording
*how well* the consumer interrogated it, and to closing the loop that
follows:

1. **Discern convergence.** Classify a consumer's interrogation trace —
   payload-free — as *efficient*, *exploring*, *churning*, or
   *surrendered*.
2. **Route on receipts.** Give orchestrators the fold they need to
   low-score a (model family × channel) pairing on evidence and
   escalate to a stronger family — with the escalation itself recorded
   as lineage.
3. **Distill for the weak.** Turn the interrogation paths of *accepted*
   runs into navigation scaffolds served — through the existing sealed
   variant matcher — to the model families that need them. The policy
   improvement lands in the projection, not the weights: in-context
   process distillation, which is the only place a cross-vendor
   substrate can put it.

Nothing in this plan weakens an existing invariant. Two are extended
(one field on the event, one zone entry on the manifest), one is added
(I-8), and every extension is opt-in at mint.

## 1 · The problem: consumption is a signal, volume is not

A cheap-model subagent that consumed 10% of its handoff *might* be
unready to comprehend it — or might have run one perfect grep and read
exactly the forty lines that mattered. Waggle's own thesis celebrates
the second case: `search` and `read` exist precisely so a consumer
never has to carry the field home. So the naive metric — fraction of
bytes served — is confounded in both directions:

- **Low consumption** is either surgical precision or surrender.
- **High consumption** is either honest thoroughness or churn.

Cycle counts fail the same way: a strong model converges in few cycles,
and a weak model *also* produces few cycles when it gives up early. The
discriminating signal is not volume or count. It is **the shape of the
transitions** — whether each action lands where the previous action
pointed — joined with **the outcome** of the run. Both halves are
already almost in reach of the event log; this plan closes the gap.

## 2 · What the research establishes

The design leans on four bodies of work, each load-bearing for a
specific decision below.

### 2.1 Behavioral traces predict success without seeing content

The exact ambiguity above — long effortful sessions that are either
productive exploration or unproductive struggle — was studied at scale
in web search. Hassan, White, Dumais & Wang built classifiers that
disambiguate *struggling* from *exploring* sessions using behavioral
and topical features alone, and showed the distinction materially
improves satisfaction prediction [1]. The discriminators were
transition-shaped, not volume-shaped: struggling users emit
increasingly diverse query reformulations with few "success clicks"
(engagements with real dwell), while explorers branch *and engage*
between branches. Earlier work established that behavior measurably
shifts as search difficulty rises — more diverse queries, more operator
use, longer dwell on result pages [2] — and that query-transition
features predict session success better than click features. Two
licenses follow for waggle: (a) success-relevant classification from
interaction geometry is achievable, and (b) it is achievable **without
content** — which is the regime I-1 already forces.

### 2.2 Convergence has a formal definition: scent-following

Information Foraging Theory [3] models an information seeker as a
forager following **scent** (proximal cues) between **patches**
(content clusters), with patch-leaving governed by the marginal value
theorem [4]: leave when the marginal rate of gain drops below the
environment's average. Recent work imports this calculus directly into
search-augmented LLM reasoning, scoring retrieval behavior by the
cost–gain structure of IFT [5], and normative patch-leaving models
give the stopping rule a principled form [6]. This yields the
operational definition this plan builds on:

> **A trace converges when successive actions follow scent** — each
> lands where the previous action's cues pointed, specificity rises
> monotonically, and the stop is rational (after the paying regions
> were reached, not before).

Every clause of that definition is computable from *positions and
ordering* — lens used, window offsets, intersection with prior search
hits — never from bytes.

### 2.3 Process signals are features, not rewards

Process supervision — scoring the intermediate steps of a trajectory
rather than only its outcome — trains markedly better reasoners than
outcome-only supervision [7], and step-level quality assessment for
*tool-using* agents specifically is now an active benchmark frontier:
ToolPRMBench [8], AgentProcessBench [9], and ToolRM [10] all construct
exactly the step-granular trajectory datasets that waggle's log emits
as a by-product. But Search-R1 [11] — the reference result for
RL-trained search agents — found that *intermediate retrieval rewards
had limited impact* while outcome-based reward drove the gains
(41% over RAG baselines on Qwen-7B), a finding echoed by successors
[12]. The design consequence is firm: **interrogation-shape features
enter the system as classifier inputs joined with outcome labels,
never as free-standing scores.** A shape score that routes traffic by
itself is a Goodhart target (§9); a shape feature conditioned on
recorded outcomes is evidence.

### 2.4 Distilling winners' traces into weak consumers works — in context

Agent Workflow Memory [13] induces reusable workflows from an agent's
*successful* trajectories and serves them in-context to guide later
runs: a 51.1% relative success-rate improvement on WebArena, 24.6% on
Mind2Web, generalizing to unseen sites — with **no weight updates**.
Agentic Context Engineering [14] and trajectory-informed memory
generation [15] extend the same result. This validates the L2 move in
§4.7: the strong family's interrogation path, rendered as content and
served as the weak family's projection, is a known-effective
mechanism — and waggle's variant system is *already shaped* to deliver
it (one token, per-family projections, sealed selection).

### 2.5 The gap this occupies

Model routers route **pre-hoc**, on prompt features and preference data
[16]. No shipping system routes **post-hoc on consumption evidence**,
because no other system holds a cross-vendor, payload-free record of
what each model family actually did with its inputs. And the cost of
not routing well is the cost waggle already cites: inter-agent
misalignment accounts for ~37% of multi-agent failures [17], and the
vendor-measured overhead of multi-agent systems traces to the handoff
seam [18]. Receipts that grade the handoff are the missing instrument.

## 3 · The signal, precisely

### 3.1 The per-trace feature vector (all payload-free)

| Feature | Definition | Computed from |
|---|---|---|
| `landing` | did this `read` window intersect the hit positions of the consumer's prior `search`? | serve-time intersection; positions only |
| `next_adherence` | did this call match a `next` step the previous envelope offered? | serve-time comparison against the envelope just issued |
| `narrowing` | is the window span ≤ the previous span from this consumer (overview → section → lines)? | window geometry |
| `revisit` | does this window substantially overlap a region already served to this consumer? | window geometry |
| `conversion` | fraction of searches followed by a read at a hit | fold over the above |
| `contract_coverage` | fraction of the mint-declared required regions touched | fold vs. §4.2 contract |
| `terminal_stage` | deepest stage reached (`resolve` … `run`) | existing funnel |
| `outcome` | `accepted` / `rejected`, recorded by the orchestrator | §4.1 stages |

### 3.2 The classification taxonomy

The fold classifies a token's consumption story into four verdicts —
deliberately mirroring the struggling/exploring result [1], which is
the empirical proof this taxonomy is learnable from behavior alone:

| Verdict | Signature | Orchestrator's move |
|---|---|---|
| **efficient** | few cycles · landings in paying regions · contract met · accepted | trust; credit the (family × channel) pair |
| **exploring** | many cycles · broad but *engaged* (high conversion, low revisit) · accepted | trust; the task was genuinely open |
| **churning** | many cycles · low conversion · high revisit · searches without landings | escalate; debit the pair |
| **surrendered** | resolve-only (or near) on a contract-bearing artifact · contract unmet | escalate; debit the pair |

The two failure verdicts are exactly the two ways "few cycles vs many
cycles" refuses to discriminate on its own — the join with landings
and outcome is what splits them.

### 3.3 Paying regions

"Where the answers live" is defined empirically, not editorially: the
**paying regions** of a token (or lineage) are the positions touched by
its *accepted* runs — a fold over geometry events joined on outcome
stages. Cold-start, before any accepted run exists, the mint-declared
contract regions (§4.2) seed the set. This is the marginal-value
structure of §2.2 made concrete: a rational stop is a stop after the
paying regions; a surrender is a stop before them.

## 4 · Scoping in waggle — the design decisions

Each decision is stated against the invariant it must not break.

### 4.1 Outcome is a stage, not a payload

`Stage` is an open slug type; the funnel already orders well-known
stages (`impression → … → run → repeat`). Two well-known stages are
added:

- **`accepted`** — the orchestrator (or a human reviewer) accepted the
  work this token's consumer produced;
- **`rejected`** — it did not.

`record --stage accepted` is the entire API. The verdict is the stage
itself, so **I-1 is untouched** — no payload field is added, nothing
about *why* enters the log. Funnel folds gain the outcome join for
free, and every downstream fold in this plan keys on it. (The existing
`assess` stage remains the consumer's own pre-commitment signal; the
new pair is the *judge's*.)

### 4.2 Consumption contracts live in the immutable core

Mint gains an optional `contract`:

```
mint … --require "lines:120-480" --require "section:Compensation" --min-coverage 0.9
```

- Serialized as `contract: { regions: [...], min-permille }` in the
  **immutable core** — a contract you can re-negotiate after delegation
  is not a contract. It is therefore covered by Ed25519 signatures.
- **Compatibility rule:** the field is optional and, when absent,
  serializes to nothing — the canonical core bytes of every existing
  manifest are unchanged, so **existing signatures remain valid** and
  the signature vectors gain cases rather than changing them.
- `coverage` — today a lineage-root verb — extends to a single
  contract-bearing token: it reports `met | unmet` with the untouched
  required regions **named**, the same honest-misses posture the
  folder form already takes.
- **As built (P1):** contract satisfaction does not wait for P2's
  geometry events. Contracts are capped at **8 regions** and each serve
  on a contract-bearing token stamps a `regions` **bitmask** on its
  `read` event — bit *i* names region *i* of the signed declaration.
  This is manifest-referencing exactly the way `variant` is, so I-1
  needs no amendment for it; the coverage fold is a plain OR
  (commutative and duplicate-immune — R-1/R-3 for free). `section:`
  requirements are sugar resolved against the outline **at mint**, the
  one moment the artifact is at hand; the manifest stores plain line
  ranges and nothing re-resolves later. P2's geometry remains the
  richer signal (spans, landings, `next`-adherence); the bitmask is the
  contract-scoped subset that ships without it.

### 4.3 Interrogation geometry: positions, never bytes — and opt-in (I-8)

The event today is exactly `{token, stage, actor, at, seq, variant?}`,
fixed-width, payload-free by type. Geometry extends it with one
optional field, kept fixed-width for the SoA log:

```
geometry: { lens: u8, start: u32, len: u32, flags: u8 }
//          which lens   window position    landing | next_adherence |
//          served       (lines or bytes,   revisit — two bits each,
//                        per lens)          tri-state (§4.4)
```

This is a real widening of the telemetry surface and the plan treats
it as one. A heatmap of *which sections* of a compensation document
were searched is not payload, but it is not nothing. Hence a new
invariant, enforced at the same boundary that enforces I-1:

> **I-8 — geometry names positions, never bytes; and geometry exists
> only for tokens whose author opted in at mint**
> (`--telemetry geometry`; the default remains counts-only).

Search patterns and matched text remain absolutely excluded (spec §8
already forbids them; this plan does not relax that by a byte). The
funnel's marketing claim — "counts only, the funnel never sees
content" — remains literally true for every token minted without the
flag, and becomes "positions only" for authors who choose the
convergence product. Geometry is stored on the record at append time,
so R-1..R-4 hold trivially on replay: features are *data*, never
recomputed.

### 4.4 Correlation without identity

`landing`, `next_adherence`, and `revisit` require correlating an
action with *the same consumer's* previous action — but I-7 forbids
instance identity in the log, and this plan keeps it that way. The
correlation happens in the daemon's **connection-scoped ephemeral
state**: each shim connection is one harness session, so the daemon
holds (per connection, per token) the last search-hit positions, the
last envelope's `next` offers, and the served-window set — in memory,
never persisted, dropped on disconnect. Only the resulting **flags**
enter the log. A consumer that arrives on a fresh connection (or via
the edge, stateless) gets tri-state `unknown` flags — degraded
honestly, never guessed. Identity dies at the boundary, exactly where
`ActorClass::from_context` already kills it.

### 4.5 Classification is a fold, and handoff tokens make it per-consumer for free

`trace` — a new catalog operation — classifies a token's consumption
story (§3.2) as a **pure fold** over its geometry events and outcome
stages: sans-I/O, deterministic, total (every event sequence maps to
exactly one verdict), property-tested alongside the funnel fold it
generalizes. No per-consumer identity is needed because the handoff
pattern already provides the partition: **a child token minted for a
delegation has one intended consumer**, so token-level classification
*is* consumer-level classification. For deliberately shared tokens,
`trace` reports per actor-class aggregates and says so.

### 4.6 The scorecard: the routing fold

`scorecard` — the second new operation — is the cross-token fold keyed
`(channel × family class)`: accepted/rejected counts, verdict mix,
escalation rate. An **escalation** is not a new verb; it is the
existing choreography, now legible: a `rejected` child, superseded by
a re-mint under the same parent targeted at a stronger family. The
scorecard counts that pattern from lineage + stages alone. This is the
table an orchestrator consults *before* delegating ("haiku-class on
`subagent/pricing`: 68% escalation — route straight to the strong
family") — the receipt-driven complement to pre-hoc routers [16], and
the "telemetry loop model vendors lack" that doc 06 §6 anticipated.

### 4.7 Distillation: scaffolds ride the variant system, via supersede

The author's daemon holds both halves of the join no one else can
make: the geometry of accepted traces *and* the artifact bytes (the
computation-travels-to-data contract, doc 08 §0). `distill` — the
third new operation, author-side only — folds the paying regions of
accepted runs against the artifact's outline and renders a
**navigation scaffold**: an ordered reading path with real headings
and line anchors ("outline first; then §Competitor Pricing, lines
847–920; verify against §Assumptions"). Delivery reuses two existing
mechanisms, because both were built for exactly this shape of change:

- Variants are immutable core, so the scaffold arrives by **minting a
  successor** manifest (same target, scaffold added as a variant
  constrained to the weaker family classes) and **superseding** the
  original. Late readers follow the pointer automatically; early
  resolutions stay honest under their own `as_of`.
- **I-2 survives by versioning**: each manifest's matcher remains
  sealed and deterministic; the scaffold *evolving* is expressed as a
  new manifest version, never as a mutable projection. Same context,
  same manifest ⇒ same projection, always.

Envelope `next` steps gain paying-region awareness in the same stroke
("accepted readers went to §4 next") — they are computed live from
state today, so this is a richer fold, not a new mechanism. This is
AWM's mechanism [13] relocated to the only layer that can serve it
cross-vendor: the weak model is never trained, its *projection* gets
the strong model's foraging policy.

### 4.8 Sealing is load-bearing

The moment interrogation shape carries routing consequences, an
unsealed handoff is a broken instrument: a consumer honing with disk
`rg` appears surrendered, and the orchestrator escalates its best
performer. Enforcement-grade classification therefore **requires the
sealed handoff** (guide 11): source in the vault, the token as the
only door. The plan's posture: `trace` reports its confidence as
`enforcement` (sealed) or `advisory` (unsealed), and seal-by-default
is recommended whenever a contract is declared. The grep becomes the
evidence — but only if the grep must travel through the instrument.

## 5 · Surface changes

One operations catalog, four projections (MCP tools, clap CLI,
COMMANDS.md, `map`) — every row below lands once in `waggle-ops` and
the parity tests propagate or fail the build:

| Op | Kind | Summary |
|---|---|---|
| `record` | extended | well-known stages `accepted`, `rejected` |
| `mint` | extended | `--require <region>` (repeatable), `--min-coverage <f>`, `--telemetry counts\|geometry` |
| `coverage` | extended | single-token contract evaluation: `met \| unmet`, misses named |
| `trace` | **new** | classify a token's consumption story; verdict + feature vector + confidence (`enforcement \| advisory`) |
| `scorecard` | **new** | `(channel × family)` acceptance/verdict/escalation fold; the routing table |
| `distill` | **new** | author-side: paying regions × artifact → scaffold variant → supersede; prints the successor's handoff line |
| `funnel` | extended | outcome stages in the fold; `--by-family` split (I-7 granularity) |

Every new response carries executable `next` steps like every existing
one: `trace` → `scorecard` → (on a debit) the re-mint escalation; the
envelopes teach the loop the way they teach everything else (doc 17).

## 6 · Spec and vector impact

| Spec section | Change | Vectors |
|---|---|---|
| §2 manifest | optional `contract` in immutable core; absence ⇒ canonical bytes unchanged | signature vectors: add contract-bearing cases; existing cases MUST pass unmodified |
| §4 event log | optional fixed-width `geometry`; I-1 statement annotated with I-8 | new `geometry.json` vectors; replay vectors gain geometry-bearing streams |
| §8 content access | serve-time feature computation defined; patterns/text exclusion restated unchanged | — |
| §9 invariants | add I-8 (positions never bytes; opt-in at mint) | — |
| new §10 | trace classification: the fold, the four verdicts, totality and determinism requirements | new `trace.json` vectors — the portable definition of the classifier, generated FROM the implementation, drift-checked in CI |

The conformance posture is unchanged: an independent implementation
that matches the vectors classifies identically. **The classifier is
sealed the way the matcher is sealed** — same trace, same verdict,
no hooks — because a routing signal you can quietly reshape is not
evidence.

## 7 · Crate-by-crate work plan

| Crate | Work |
|---|---|
| `waggle-core` | `geometry` on `Event` (+ SoA columns, pack/unpack); `contract` in manifest core + canonical serialization rule; `trace` fold in `fold.rs` (pure, total); paying-regions fold; scaffold rendering as a pure function `(regions, outline) → variant body`; new well-known stages |
| `waggle-store*` | schema migration for the geometry columns (additive, WAL-safe); conformance suite gains geometry/contract cases — the suite change certifies all three backends at once |
| `waggle-ops` | the seven catalog rows (§5); parity tests carry them to all four projections |
| `waggle-mcp` | serve-time feature computation (the only stateful piece: §4.4 connection-scoped correlation); envelope `next` biasing |
| `waggle-cli` | flags per §5; `waggled` session-state plumbing |
| `waggle-store-cloudflare` | geometry replication; edge `trace` (folds run where the records are); stateless edge ⇒ `unknown` flags documented in the completeness matrix (new E-row) |
| `spec/` | §6 changes + regenerated vectors |
| `benches/` | trace fold beside the funnel fold (the million-event funnel folds in 334 µs; trace must stay the same order) |

## 8 · Implementation standards

The bar is the one the corpus already sets; this feature ships under
all of it, none of it new:

- **Sans-I/O discipline** (doc 03, guide 05): every fold — trace,
  paying regions, scorecard, scaffold rendering — is a pure function in
  `waggle-core`; no clock, no entropy, no storage; identical under
  native and wasm. The *only* stateful addition is the daemon's
  connection-scoped correlation map, at the edge where effects already
  live.
- **Read-only by type** (I-4): `trace`, `scorecard`, `coverage` take
  `&impl ReadStore` — they cannot write, checked by the existing
  `compile_fail` doctest pattern.
- **Property tests**: classification totality and determinism (any
  event sequence ⇒ exactly one verdict; permutation-stable per R-1);
  geometry pack/roundtrip; canonical-bytes stability for contract-free
  manifests; budget invariants untouched (every response ≤ `max-bytes`,
  floor 64).
- **Replay equivalence**: R-1..R-4 re-proven over geometry-bearing
  streams; `reconstruct` remains shuffle- and duplicate-immune.
- **Catalog parity**: new ops exist in one table; drift between the
  four projections fails the build, as today.
- **CI matrix**: three OS + wasm + Miniflare edge matrix; the
  differential oracle holds edge `trace` byte-identical to SQLite over
  the same streams.
- **`just preflight` green**: fmt, clippy `-D warnings`, file-size
  lint, tests, wasm — per commit, no exceptions.

## 9 · Non-goals, Goodhart, and honesty

- **No comprehension claims.** Receipts prove consumption geometry,
  never understanding. `trace` verdicts are named for behavior
  (churning, surrendered), not cognition.
- **No model training.** Waggle emits process-supervision-grade data
  (the JSONL export is already the trajectory dataset [8][9][10] are
  starved for — payload-free, cross-vendor, outcome-joined); training
  on it is downstream of the substrate, permanently.
- **No per-instance tracking.** I-7 stands. Family-class granularity is
  the routing key and the ceiling.
- **Goodhart resistance, by construction:** shape features are
  classifier inputs joined with outcomes, never free-standing rewards
  (§2.3); enforcement-grade requires sealing (§4.8); and orchestrators
  are advised (in the guide this ships with) to keep an exploration
  floor — occasionally route against the scorecard to keep baselines
  calibrated, the standard bandit hygiene.
- **The privacy trade is named, not hidden:** geometry is a real
  widening of telemetry (section-level heatmaps), which is why I-8
  makes it opt-in at mint and why counts-only remains the default. The
  author who wants convergence receipts pays for them with position
  visibility on their own artifact — no one else's.

## 10 · Phases

Each phase is independently shippable and gate-kept by its tests.

| Phase | Delivers | Gate |
|---|---|---|
| **P0** ✅ | `accepted`/`rejected` stages; funnel outcome join (`pending/accepted/rejected/contested`); escalation choreography in the record envelope + guide 04 | funnel fold tests; COMMANDS/parity |
| **P1** ✅ | mint contracts (`--require`, `--min-coverage`, `section:` sugar); region-touch bitmask on read events; single-token `coverage` with named misses; signature compatibility | signature vectors: old cases byte-identical (held); contract-bearing case added; end-to-end receipts test |
| **P2** | geometry events (opt-in, I-8); connection-scoped correlation; store migrations | replay R-1..R-4 over geometry streams; conformance suite on all backends |
| **P3** | `trace` + `scorecard`; sealed-classifier vectors; edge parity | trace vectors drift-checked; differential oracle green; bench within order of funnel fold |
| **P4** | `distill` scaffolds via supersede; `next` biasing; guide 12 (receipt-driven routing) | I-2 property per manifest version; end-to-end dogfood: weak-family success rate with vs. without scaffold, measured on this repo |
| **P5** | README rewritten to the implemented system (per the repo decision that moved the essay to `essay.md`); docs map updated | docs drift-check |

## 11 · References

1. A. Hassan, R. W. White, S. T. Dumais, Y.-M. Wang. *Struggling or
   Exploring? Disambiguating Long Search Sessions.* WSDM 2014.
   <https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/HassanWSDM14.pdf>
2. A. Aula, R. M. Khan, Z. Guan. *How does search behavior change as
   search becomes more difficult?* CHI 2010.
   <https://dl.acm.org/doi/10.1145/1753326.1753333>
3. P. Pirolli, S. K. Card. *Information Foraging.* Psychological
   Review 106(4), 1999.
4. E. L. Charnov. *Optimal foraging, the marginal value theorem.*
   Theoretical Population Biology 9(2), 1976.
5. *Scent of Knowledge: Optimizing Search-Enhanced Reasoning with
   Information Foraging.* 2025. arXiv:2505.09316.
6. Z. P. Kilpatrick, J. D. Davidson, A. El Hady. *Normative theory of
   patch foraging decisions.* 2020. arXiv:2004.10671.
7. H. Lightman et al. *Let's Verify Step by Step.* 2023.
   arXiv:2305.20050.
8. *ToolPRMBench: Evaluating and Advancing Process Reward Models for
   Tool-using Agents.* 2026. arXiv:2601.12294.
9. *AgentProcessBench: Diagnosing Step-Level Process Quality in
   Tool-Using Agents.* 2026. arXiv:2603.14465.
10. *ToolRM: Towards Agentic Tool-Use Reward Modeling.* 2025.
    arXiv:2510.26167.
11. B. Jin et al. *Search-R1: Training LLMs to Reason and Leverage
    Search Engines with Reinforcement Learning.* 2025.
    arXiv:2503.09516.
12. *s3: You Don't Need That Much Data to Train a Search Agent via
    RL.* 2025. arXiv:2505.14146.
13. Z. Z. Wang, J. Mao, D. Fried, G. Neubig. *Agent Workflow Memory.*
    ICML 2025. arXiv:2409.07429.
14. *Agentic Context Engineering: Evolving Contexts for Self-Improving
    Language Models.* 2025. arXiv:2510.04618.
15. *Trajectory-Informed Memory Generation for Self-Improving Agent
    Systems.* 2026. arXiv:2603.10600.
16. I. Ong et al. *RouteLLM: Learning to Route LLMs with Preference
    Data.* 2024. arXiv:2406.18665.
17. M. Cemri et al. *Why Do Multi-Agent LLM Systems Fail?* (the MAST
    taxonomy). 2025. arXiv:2503.13657.
18. Anthropic. *How we built our multi-agent research system.*
    Engineering blog, June 2025 — the "each handoff loses context"
    source, already load-bearing in [WHY.md](../WHY.md).

---

*The dance already told the hive where the field is. This document
teaches the hive to notice which foragers fly straight, to stop
sending the ones that circle, and to sketch the flight path on the
comb for the ones still learning.*
