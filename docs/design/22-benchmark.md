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

## 4 · Tier 3 — the substrate under load (restructured)

### 4.0 What the SWE-bench pilot exposed — and why it is withdrawn

A first Tier-3 was built against SWE-bench Lite (9 models × 12 instances ×
{copy, waggle}, test-graded in Docker). It ran, it graded, and it produced a
tempting headline (`gpt-4o-mini` resolved an instance through waggle on
4.3k tokens where the paste arm failed on 16.4k). **We withdraw it.** The
pilot was a good harness for the wrong experiment, and it failed on four
counts we would not accept from anyone else:

1. **It under-represented the system.** The waggle arm used only
   `mint --snapshot` plus `read`/`search`. It never exercised a consumption
   contract, never read a funnel back, never resolved a *projection*, never
   used lineage, revocation, or `--tree`. It measured waggle as a lazy file
   reader — and a lazy file reader can only ever produce a token-cost
   number, the least distinctive of our claims. Attribution, the thing a
   path cannot do, went **entirely untested**.
2. **The arms differed in more than the handoff.** `copy` was effectively
   single-shot with everything pasted; `waggle` was a multi-turn agentic
   loop. Turn count was confounded with mechanism, and the honest
   competitor the paper itself names — *a raw path plus ordinary file
   tools* — was not an arm at all. Beating a paste baseline proves little.
3. **The task leaked its own answer.** The candidate file set was the
   gold-patch files plus siblings: oracle localization, handed to both arms.
   That both inflates the resolve rate and deletes the retrieval problem
   where a symbol lens is supposed to earn its keep.
4. **Measurement integrity broke.** Anthropic token counts were
   *approximated* (the gateway strips `usage`), so cross-family cost was not
   apples-to-apples; transport timeouts were silently recorded as model
   failures; and — the disqualifying one — the patch-applier was made more
   permissive **after** observing that the waggle arm was failing. That is
   post-hoc tuning toward a preferred result. Pre-registration exists to
   make exactly that impossible, and it caught us.

Only one artefact of the pilot survives: the evidence that the pipeline
*can* be built (Docker grading resolves a gold patch in ~59 s; both model
families are callable). The results are quarantined and are not reported.

### 4.1 The right experiment: the substrate, across every modality

Waggle is not a code-retrieval trick; it is a handoff substrate with lenses
over heterogeneous artifacts. Tier 3 is therefore restructured as a
**feature × modality matrix**, benchmarking what the system actually claims:

- **Mint** — what minting costs and, more importantly, what it *discovers*
  (a symbol outline for code, a heading tree for markdown, page/segment
  structure for PDF, timecoded segments for video/voice).
- **Resolution vs. a reference** — the head-to-head. A path or URI is the
  competitor; the token is the claim.
- **Lens projections and querying** — outline / section / lines / symbol /
  JSON-path / search, per modality: how surgical is the slice, and how many
  bytes did it spare?
- **Per-consumer projection** — one token, different truthful renderings
  (the sealed matcher): digest for a small-context model, media for a vision
  or audio consumer, transcript for a consumer with neither.

Across **six resource types**: `text`, `markdown`, `code`, `pdf`, `video`,
`voice`. The binary three are the interesting ones, because they are where a
raw reference degenerates entirely: a path to an MP4 hands the consumer
nothing it can read, while a waggle token carries the transcript to the
text-only consumer and the media itself to the one that can watch or listen.

### 4.2 Three arms, turn-matched

Every arm gets the *same* question, the *same* turn budget, and the *same*
grading. Only the handoff differs.

| arm | the handoff | what it can do |
|---|---|---|
| `copy` | the artifact's content, pasted | today's default; whole artifact enters the window |
| `reference` | a path/URI + ordinary file tools (open, grep) | the honest competitor the paper names |
| `waggle` | a ~30-byte token + the substrate's verbs | resolve → projection; lens; search; receipts |

The `reference` arm is mandatory. Without it we are only beating a straw
man, and the paper's own framing ("waggle's competitor is *`Here's
/tmp/analysis.md`*") is left unanswered.

### 4.3 The corpus and its ground truth

Each modality contributes artifacts carrying a **planted fact** inside a
known region, so a question is answerable *only* by reaching that region.
This gives three graded quantities at once: **correctness** (did it answer),
**cost** (bytes/tokens ingested), and **coverage** (did the receipts show it
actually consumed the region — the attribution claim, measurable only in the
waggle arm, and that asymmetry is itself the finding).

- `text`, `markdown` — long documents; fact inside a specific section.
- `code` — real source files; fact is a symbol's behaviour.
- `pdf` — a real paper; fact on a specific page.
- `video`, `voice` — media with transcripts; fact at a specific timecode.

### 4.4 What is measured

Per (modality × arm × model):

- **tokens-to-correct-answer** and **ops-to-correct-answer** (the frontier).
- **bytes spared** — the artifact's size minus what actually entered the
  window (waggle names this in every response; the other arms cannot).
- **correctness** — graded against the planted fact.
- **coverage / receipts** — did the funnel and coverage fold record the
  consumption the model claims? (`waggle` only; the *absence* elsewhere is
  the point.)
- **projection fidelity** — for the same token, does a vision consumer get
  the media, a text-only consumer the transcript, a small-context consumer
  the digest — and does the tuned projection change the *outcome* for the
  weak model?
- **mint cost** — latency and the structure discovered, per modality.

### 4.5 Measurement hygiene (the pilot's failures, closed)

- **Exact token usage for both families.** No approximation. If a gateway
  strips `usage`, it is fixed or that family's cost is reported only as a
  within-family ratio, explicitly labelled.
- **Transport errors are not model failures.** They are classified,
  retried within a bounded budget, and any run with an unrecovered transport
  error is *excluded and reported as excluded*.
- **One grader, frozen before the run**, applied identically to every arm.
  No post-hoc leniency, for any arm, in either direction.
- **Selection is published.** The corpus and the sampling seed are fixed in
  advance; no instance is dropped after seeing its result.

### 4.6 Power

≥ 30 artifacts per modality, ≥ 3 seeds, all 9 models, three arms; medians
with bootstrap CIs and a paired test, per §5. Anything less is a
demonstration, not a benchmark, and will be labelled as such.

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
- **Phase 3 (flagship spend):** §4 as restructured — the feature × modality
  matrix (mint · resolution-vs-reference · lens projection · querying)
  across `text`, `markdown`, `code`, `pdf`, `video`, `voice`, over three
  turn-matched arms (`copy`, `reference`, `waggle`) and all nine models.
  Becomes the new quantitative core of §Evaluation. The withdrawn SWE-bench
  pilot (§4.0) is reported as a negative methodological result, not as a
  measurement — the honest thing to do with an experiment that failed its
  own pre-registration.

For a **pre-print**, Phase 1 (fully) + Phase 2 (fully) + a Phase-3 *pilot*
(≈30 SWE-bench-Lite instances, 2 models, scoped honestly as a pilot) is
defensible and publishable. The honest voice of the current draft is an
asset — the benchmark keeps it: pre-registered, CI'd, null results
reported.


---

## 10 · What the benchmark taught the product

The point of a benchmark is not to score the system; it is to find out where
the system is wrong. Ours did, four times, and every one of the fixes below is
a *substrate* change that the traces demanded — not a harness tweak, and not a
prompt trick.

| The trace | What it revealed | The fix |
|---|---|---|
| A model searched `X9Y-[0-9]{4}` (our own example, taken literally), got `[]`, and answered "NOT FOUND" | A zero-match search was a **cliff**: no total, no signal, nowhere to go | An empty search now returns the count, a hint, and next steps |
| Consumers of an extracted PDF/transcript could only guess a regex | `text/plain` carried **no outline** — nothing to steer by | Plain text gets a structural outline (structure, never content) |
| Both models opened a folder with an overview call and got 83 bytes of nulls | A folder could be **grepped but not described** | `read` on a tree returns the tree: files, tokens, sizes, outlines |
| An agent issued `section: "Retry Policy"` **ten times**, once per child | A tree could be grepped but not **lensed** | A lens on the folder token fans out over every file |
| The fan-out truncated at 9 of 10 runbooks — the missing one was the violator — and the agent answered **confidently and wrongly** | Incompleteness was **silent** | `complete`/`examined`/`total_files` + a `from` cursor, and a `next` that says INCOMPLETE in words |

Two of these were found *only* because the receipts existed. The coverage fold
recorded consumers guessing, and that is what sent us to the traces. An
evaluation that looked only at answers would have scored the model down and
shipped the defect.

### 10.1 A harness failure worth naming

The harness was **dropping waggle's `next` hints** — the substrate's
guidance-at-point-of-use mechanism — and thereby under-representing the system
it was measuring. That is why the agent never discovered the fan-out on its
own: nothing told it. Any harness that evaluates waggle must surface `next`,
because that channel *is* part of the interface. It is now carried on every
response.

### 10.2 The open question the corpus cannot answer

Whether a folder needs **filtering** (narrow the file set before lensing —
by glob, by outline, by search hit) is *not* settled by this benchmark: our
folders are eleven files, and the listing fits in one response. The evidence
justified **completeness** (pagination, loud truncation), not filtering.
Deciding filtering honestly requires a **large tree** — hundreds of files,
where the listing itself truncates — and that is a corpus shape to add, not a
feature to assume.
