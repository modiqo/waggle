# The paper

*The Dance and the Field: Name Semantics for Handoffs Between Distributed
Agents* — a systems paper arguing that the agent handoff is a
distributed-systems problem and that name semantics is its right shape.

## Build

Requires [Tectonic](https://tectonic-typesetting.github.io) (self-contained;
resolves packages and runs BibTeX internally):

```sh
tectonic waggle.tex        # → waggle.pdf
```

Or any TeX Live with `latexmk`:

```sh
latexmk -pdf waggle.tex
```

## Files

| File | Contents |
|---|---|
| `waggle.tex` | The paper: two-column `article`, Computer Modern, hand-specified TikZ figures, `algorithm2e` pseudocode, `booktabs` tables |
| `references.bib` | 25 references — the systems lineage (Linda, NDN, capabilities, leases, content addressing, the log), the biology (von Frisch, stigmergy), and the contemporary agent literature |

## Provenance of the numbers

The microbenchmarks (Table 3) come from `../benches/PERF.md`; the live
delegation (§8.2) is the run recorded in
`../docs/design/notes/dogfood-01-symbol-handoff.md`; the design maps to
the corpus in `../docs/design/` (name semantics ← 02/03/04, the sealed
matcher ← 06, receipts ← 19, the symbol lens ← 20, the resource
projection ← 21, three radii ← 08/16).
