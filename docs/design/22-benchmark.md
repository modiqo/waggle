# 22 — A credible benchmark: what would make the claims falsifiable

*Status: technical plan + pre-registration. This document commits, before
the runs, to the metrics, baselines, task substrate, and statistics the
evaluation will use. It exists so the numbers cannot be chosen after the
fact to flatter the system. Sections 0–1 set the bar; §2–4 specify the
three tiers of evidence; §5 is the statistical protocol; §6 the threats we
hold ourselves to; §7–8 reproducibility and task manifests; §9 the phased
build and its mapping into the paper (`paper/waggle.tex §`\ref{sec:eval}).*

---

## 0 · The claim

The paper (`§`Evaluation) already poses three questions:

1. **Is the reference cheap enough to prefer over a copy?**
2. **Does verification-without-trust work on a real delegation?**
3. **How does interrogation-through-a-token compare with lexical search?**

Today it answers them with, respectively, an *analytical* arithmetic
(§"The arithmetic, honestly"), a *single* live trial we explicitly label an
existence proof, and a *qualitative* argument. A credible benchmark is the
controlled quantification of exactly these three questions — no new claims,
just harder evidence for the ones already made. This document does not
propose to make waggle look good; it proposes to make its claims *able to
fail*.

## 1 · The credibility bar

Reviewers of agent-infrastructure work discount three things reflexively,
and we design against each:

- **Single-model, single-seed numbers.** → Every model-dependent result is
  run on ≥2 models and N≥40 paired instances with bootstrap confidence
  intervals; the model-*independent* results (token accounting, determinism,
  latency) are labelled as such and carry the load.
- **Synthetic tasks that obviously favour the system.** → The end-to-end
  tier is anchored on *public, accepted* datasets (SWE-bench Lite; a public
  long-document QA set), never a hand-authored task.
- **"Fewer tokens = better," with no quality control.** → Cost is never
  reported without a paired task-success measure on the *same* instances.
  The headline is a *frontier* (cost vs success), not a single ratio.

Two further disciplines, borrowed from systems papers that aged well:

- **A cost model that predicts, then is confirmed.** We state the cost
  analytically (§2.1), compute it exactly with a tokenizer, and then feed
  *real* Tier-3 transcripts through the *same* accounting functions. The
  confirmation is the same code path applied to real logs, not a second
  hand-tuned model.
- **The strong baseline, honestly.** The copy baseline includes
  within-vendor prompt caching (a naïve re-send baseline is a straw man).
  waggle's advantage must survive a *cached* copy, and its real edge is
  stated where caching cannot reach: across turns, across vendors, across
  machines, and in attribution.

## 2 · Tier 1 — deterministic, model-independent (the bedrock)

Zero API spend. Fully reproducible from `bench/`. This tier alone can carry
a systems paper; the later tiers make it an *agent* paper.

### 2.1 Cost model + exact accounting

For one artifact of size `S` bytes handed to `H` consumers, each taking `T`
turns, with `R` corrections, and `q` interrogation bytes per question with
`Q` questions per consumer. Let `tok(·)` be a tokenizer; `b = 30` bytes is
the token line; `p` the projection (digest) size; `d ∈ [0,1]` the
prompt-cache billing discount for a cached prefix.

- **Copy, no cache** (the cross-vendor / cross-machine reality):
  `C_copy = H·T·tok(S)  +  R·H·tok(S)`
- **Copy, within-vendor cache** (the strong baseline):
  `C_copy^cache = H·tok(S)  +  H·(T−1)·d·tok(S)  +  R·H·tok(S)`
  (a correction invalidates the cached prefix → full re-price).
- **waggle** (name semantics):
  `C_wag = H·tok(b)  +  H·(tok(p) or Q·tok(q))  +  H·(T−1)·tok(b)  +  R·H·tok(b)`

The tokenizer cancels in the *ratio* `C_copy / C_wag`, so the crossover and
asymptote are **tokenizer-invariant** — the headline conclusion does not
depend on the choice of `tok(·)`. We report the ratio as primary and
absolute counts (under a documented tokenizer) as secondary.

**Sweep.** `S ∈ {4, 16, 40, 160, 640} KB`, `H ∈ {1,3,5,10}`,
`T ∈ {1,3,5,10}`, `R ∈ {0,1,3}`, with the paper's exact cell
(`S=40KB, H=5, T=5, R=1`) reproduced from source. Output: `paper/data/
cost_sweep.dat` (pgfplots) → the crossover figure, and `paper/tables/
cost_model.tex` → the representative table. The "arithmetic, honestly"
paragraph becomes a figure with a crossover point, not a worked example.

### 2.2 Reconstruction determinism (a correctness property)

Backs "replays identically on any machine." Generate a log of `K` tokens
with mixed mint / mutation / event records; produce `M` random
permutations *and* duplications (C-8 replay); `reconstruct()` each; assert
all `M` serialized `WorldState`s are byte-identical (hash-equal). Report
`(K, E, M, all-identical)` and the fold time. This is a property, not a
vibe: any non-determinism is a hard failure.

### 2.3 Hot paths

The existing criterion benches (`hot_paths`, `store_paths`, `query_paths`,
`extract`) are the source of Table~\ref{tab:perf}. Tier 1 adds p50/p99 and
the 10M-event fold; the table is emitted, not transcribed.

## 3 · Tier 2 — verification-without-trust (the decisive experiment)

The paper's own Limitations name this as the deciding metric: *"the
reliability of receipts under seal, across many trials … remains to be
measured."* This tier measures it.

- **Ground truth we control:** whether a subagent must, and does, consume
  the required region to answer a planted question.
- **Conditions:** *sealed* (source in a vault — the token is the only access
  path) vs *side-door* (source directly readable on a shared FS).
- **Adversary:** *bluffers* — subagents instructed to report completion
  without reading — mixed in at a known rate.
- **Metric:** precision / recall / F1 of the "consumed required region"
  signal (served-byte + coverage fold) against ground truth; the
  side-door false-negative rate; the bluffer-detection rate; an ROC over
  the coverage threshold. ≥2 models, N≥40 trials per cell.
- **Predicted shape:** sealed → high precision (bluffers caught, few
  bypasses); side-door leaks a quantified false-negative rate. This turns
  "the coverage flip was proof" into a detector with an operating curve.

The task is deliberately minimal and controlled (a planted fact inside a
required region), because the *mechanism*, not task difficulty, is under
test here.

## 4 · Tier 3 — cost at fixed quality (the flagship)

Pre-empts "you just sent less and lost information." Anchored on public
data so the task cannot be accused of favouring us.

- **Code substrate:** SWE-bench Lite instances — real repos, gradeable by
  test pass. An orchestrator delegates localization + patch to subagents
  via file references.
- **Prose substrate:** a public long-document QA set — the delegated
  artifact; graded by EM / F1.
- **Strategies (same instances, paired):** `copy-paste`, `raw-path`,
  `RAG-chunk`, `waggle`.
- **Headline:** the *cost vs task-success frontier*. The credible claim is
  "waggle holds equal success at materially lower cost **and** yields
  attribution the other three structurally cannot" — the last clause
  survives even a modest cost delta.
- **Interrogation (question 3), quantified here:** on the SWE-bench repos,
  ops-to-locate and tokens-to-locate for `read --symbol` (precision@1 to
  the correct extent) vs `rg` (typically 2–3 ops, no receipt). This is the
  numeric form of §"Interrogation versus lexical search."

Tier-3 transcripts are fed through the *Tier-1 accounting functions*
(§2.1), so the cost model is confirmed on real logs by construction.

## 5 · Statistical protocol (pre-registered)

- Paired design: identical instances across all strategies/conditions.
- ≥2 models (one frontier, one mid) — the systems metrics to show
  model-independence, the behavioural metrics to bound generality.
- N≥40 instances per cell; medians with 95 % bootstrap CIs (10k resamples);
  a paired test (Wilcoxon signed-rank) for cost and for success.
- Fixed decoding (temperature, seeds where the API allows); the harness
  commit hash and model snapshot recorded in every result file.
- **Pre-registration:** the metric definitions and the primary comparison
  are fixed *in this document* before the runs; deviations are reported as
  deviations. Null and negative results are reported.

## 6 · Threats to validity (the anti-patterns we refuse)

1. **Straw-man baseline.** The copy baseline models within-vendor caching;
   we state where waggle's edge is *not* caching (cross-turn/vendor/machine,
   attribution).
2. **Cost without quality.** Never reported alone; always the frontier.
3. **Favourable synthetic task.** Public datasets for Tier 3; the Tier-2
   task is minimal *by design* and its minimality is stated.
4. **Tokenizer cherry-pick.** Headline is the tokenizer-invariant ratio.
5. **Comprehension ≠ consumption.** Receipts prove bytes served, not
   understanding; `run` is self-reported corroboration, never the primary
   signal (already stated in Limitations; the benchmark honours it).
6. **Goodhart on routing features.** Interrogation-shape features feed an
   outcome-labelled judgment, never a free-standing reward.
7. **Irreproducibility.** Everything below ships.

## 7 · Reproducibility artifact

- `bench/` (this crate): Tier-1 fully; Tier-2/3 harness + drivers.
- `bench/manifests/` — task manifests: *pointers* to public datasets
  (SWE-bench Lite ids; the QA set), never vendored data.
- `bench/traces/` — raw receipts / transcripts from our runs (payload-free
  where the log is; model outputs where grading needs them).
- `paper/data/*.dat`, `paper/tables/*.tex` — figures/tables regenerated by
  `just bench-paper` (a.k.a. `make bench`), so a reviewer re-derives every
  number from source.
- A tagged release + Zenodo DOI cited by the paper.

## 8 · Task manifest format

A manifest is a small JSON descriptor the harness runs without embedding
data:

```json
{
  "id": "swe-lite/django__django-11099",
  "substrate": "code",
  "source": { "dataset": "swe-bench-lite", "instance": "django__django-11099" },
  "artifact": "the repo snapshot minted at the failing commit",
  "planted_region": { "kind": "symbol", "name": "URLValidator.__call__" },
  "question": "graded by the instance's own fail-to-pass tests",
  "strategies": ["copy-paste", "raw-path", "rag-chunk", "waggle"],
  "grader": "swebench-harness"
}
```

The `AgentDriver` trait abstracts the model call; a `RecordedDriver`
replays fixtures (so the harness is testable with no API), and an
`ApiDriver` (feature-gated) runs live when keys are present.

## 9 · Phased delivery, and where it lands in the paper

- **Phase 1 (now, deterministic):** §2 in `bench/` — cost-model sweep +
  determinism, emitting the paper's cost figure/table. Upgrades §"The
  arithmetic, honestly" from a worked example to a validated model.
- **Phase 2 (cheap model calls):** §3 receipt-reliability under seal +
  bluffer ROC. Turns §"A live delegation" from an existence proof into a
  controlled measurement; fills the gap Limitations flag as decisive.
- **Phase 3 (flagship spend):** §4 cost-vs-success frontier on SWE-bench
  Lite + long-doc QA; symbol-lens-vs-rg locate numbers. Becomes the new
  quantitative core of §Evaluation.

For a **pre-print**, Phase 1 (fully) + Phase 2 (fully) + a Phase-3 *pilot*
(≈30 SWE-bench-Lite instances, 2 models, scoped honestly as a pilot) is
defensible and publishable. The honest voice of the current draft is an
asset — the benchmark keeps it: pre-registered, CI'd, null results
reported.
