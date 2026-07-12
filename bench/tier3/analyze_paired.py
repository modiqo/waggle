"""Paired before/after on the HARD shapes: did the substrate fixes move them?

Same seed corpus, same artifacts, same models, same arms — the only thing that
changed between the two runs is waggle itself. So this is a paired comparison,
not two independent samples, and the per-cell deltas are attributable.

The four fixes under test (commit 54f871b):
  1. a zero-match search no longer stamps `read` on the files it swept
  2. code children project symbols in the tree listing
  3. children are named by path relative to the tree root
  4. `--require files:all` — the completeness contract at tree scale

Fixes 1 and 4 are what the `gate` arm stands on: before, one broad grep took a
folder to met=true, so the gate had nothing to refuse. It should now bite.
"""

from __future__ import annotations

import json
import os
from collections import defaultdict

BEFORE = os.environ.get("PAIRED_BEFORE", "/tmp/tier3/out4/runs4.json")
AFTER = os.environ.get("PAIRED_AFTER", "/tmp/tier3/out5/runs4.json")
SHAPES = ["folder", "bigcode", "multihop", "reasoning"]
ARMS = ["copy", "reference", "waggle", "waggle+gate"]


def mean(xs):
    xs = list(xs)
    return sum(xs) / len(xs) if xs else 0.0


def load(p, shapes):
    runs = [r for r in json.load(open(p))
            if r["modality"] in shapes and not r.get("transport_error")]
    g = defaultdict(list)
    for r in runs:
        g[(r["modality"], r["arm"])].append(r)
    return g


def cell(rs):
    return dict(n=len(rs),
                ok=100 * mean(1 if r["correct"] else 0 for r in rs),
                ing=mean(r["ingested"] for r in rs),
                turns=mean(r["turns"] for r in rs))


def main() -> int:
    b, a = load(BEFORE, SHAPES), load(AFTER, SHAPES)

    print("accuracy %, before → after (same corpus, same models; Δ is the fix)\n")
    hdr = f"{'shape':<10}" + "".join(f"{x:>22}" for x in ARMS)
    print(hdr)
    print("-" * len(hdr))
    for s in SHAPES:
        row = f"{s:<10}"
        for arm in ARMS:
            bc, ac = cell(b[(s, arm)]), cell(a[(s, arm)])
            if not ac["n"]:
                row += f"{'—':>22}"
                continue
            d = ac["ok"] - bc["ok"]
            sign = f"{d:+.0f}" if abs(d) >= 0.5 else "="
            row += f"{bc['ok']:>7.0f} →{ac['ok']:>5.0f} {sign:>7}"
        print(row)

    print("\noverall (hard shapes)")
    print("-" * len(hdr))
    row = f"{'ALL':<10}"
    for arm in ARMS:
        bs = [r for s in SHAPES for r in b[(s, arm)]]
        as_ = [r for s in SHAPES for r in a[(s, arm)]]
        bc, ac = cell(bs), cell(as_)
        d = ac["ok"] - bc["ok"]
        row += f"{bc['ok']:>7.0f} →{ac['ok']:>5.0f} {d:>+7.0f}"
    print(row)

    print("\nbytes ingested (mean), after")
    for arm in ARMS:
        as_ = [r for s in SHAPES for r in a[(s, arm)]]
        print(f"  {arm:<14} {mean(r['ingested'] for r in as_):>9.0f}B"
              f"   turns {mean(r['turns'] for r in as_):>4.1f}")

    # The gate's whole claim: it refuses answers the receipt does not back.
    # If fix 1 worked, `met` should now be FALSE on runs where the consumer
    # never actually read the file it answered from.
    print("\ngate behaviour — did fix 1 give it something to refuse?")
    for s in SHAPES:
        rs = a[(s, "waggle+gate")]
        fired = sum(1 for r in rs if r.get("gate_rejections", 0) > 0)
        ok = 100 * mean(1 if r["correct"] else 0 for r in rs)
        base = 100 * mean(1 if r["correct"] else 0 for r in a[(s, "waggle")])
        print(f"  {s:<10} gate fired on {fired:>2}/{len(rs):<3} runs   "
              f"waggle {base:>3.0f}% → gated {ok:>3.0f}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
