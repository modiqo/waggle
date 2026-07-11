# waggle-bench — the reproducible benchmark harness

The evaluation behind the paper (`paper/waggle.tex`), specified and
pre-registered in **[design doc 22](../docs/design/22-benchmark.md)**. The
benchmark exists to make the paper's claims *able to fail*, not to flatter
the system.

## What runs today (Tier 1 — deterministic, no API spend)

```
just bench-paper        # or: make bench
```

This regenerates, under `paper/generated/`:

- **`cost_sweep.dat`** — the handoff cost model (§2.1) swept over artifact
  size, pgfplots-ready. waggle's cost is flat in artifact size (it never
  sends the artifact); copy scales linearly. The crossover is the figure.
- **`cost_table.tex`** — representative scenarios including the paper's
  exact cell (40 KB, 5 consumers, 5 turns, 1 correction), priced against
  the *strong* (cached) copy baseline. The cost ratio is tokenizer-invariant.
- **`determinism.tex`** — the reconstruction-determinism gate (§2.2): one
  log reconstructed from hundreds of shuffled + duplicated orderings, every
  `WorldState` byte-identical. **Non-determinism fails the command** (a real
  gate, not a report).

Subcommands: `cargo run -p waggle-bench -- [cost-model|determinism|all] [out-dir]`.

## What is specified and seam-ready (Tiers 2–3 — model-driven)

Doc 22 §3–4 defines these; they run when API keys and public datasets are
supplied. `manifests/` holds example task descriptors (pointers to public
data — nothing is vendored):

- **Tier 2 — verification without trust** (`receipt-seal-example.json`):
  receipt reliability under *seal* vs *side-door*, with bluffers injected.
  Precision/recall/F1 of the "consumed the required region" signal, plus a
  coverage ROC. This is the metric the paper's Limitations call decisive.
- **Tier 3 — cost at fixed quality** (`swe-lite-example.json`): the
  cost-vs-task-success frontier on SWE-bench Lite + a long-doc QA set,
  across `{copy-paste, raw-path, rag-chunk, waggle}`, plus symbol-lens vs
  `rg` locate numbers. Real transcripts feed the *same* Tier-1 accounting,
  so the cost model is confirmed on real logs by construction.

## Credibility disciplines (enforced by the design)

Tokenizer-invariant headline · strong (cached) copy baseline · cost never
reported without paired task-success · public datasets, not favorable
synthetic tasks · ≥2 models with bootstrap CIs · pre-registered metrics ·
null results reported. See doc 22 §5–6.
