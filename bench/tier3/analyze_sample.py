"""Read a stratified PAIRED sample and say what it can — and cannot — support.

A short run's job is to be honest about its own resolution. Two arms differing by
6 points on 30 runs each is not a finding, and reporting it as one is how a
benchmark starts flattering its author. So this prints:

  * a Wilson interval on each arm's accuracy — the range the true value plausibly
    sits in, which at n=30 is roughly +/-15 points and needs to be seen;

  * the PAIRED delta between arms, which is the whole reason for pairing. Because
    every arm ran on the same (artifact, model) pairs, we can throw away the runs
    where two arms agreed — they carry no information about the difference — and
    look only at the DISCORDANT ones: where A was right and B wrong, and the
    reverse. That comparison is far sharper than differencing two averages, and
    it is the only one that survives a small sample.

If the discordant counts are tiny, the honest answer is "this run cannot tell",
and it says so rather than reporting a direction.
"""

from __future__ import annotations

import json
import math
import os
import sys
from collections import defaultdict

RUNS = os.environ.get("TIER3_RUNS", "/tmp/tier3/sample/runs4.json")
ARMS = ["copy", "reference", "waggle", "waggle+gate"]


def wilson(k: int, n: int) -> tuple[float, float]:
    """95% Wilson interval — behaves at small n, where normal approx does not."""
    if not n:
        return (0.0, 0.0)
    z, p = 1.96, k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def main() -> int:
    runs = [r for r in json.load(open(RUNS)) if not r.get("transport_error")]
    # key a run by the work it faced, so arms line up pair-for-pair
    by_arm: dict[str, dict[tuple, dict]] = defaultdict(dict)
    for r in runs:
        by_arm[r["arm"]][(r["art"], r["model"])] = r

    print(f"n={len(runs)} runs\n")
    print("accuracy, with 95% Wilson interval")
    for a in ARMS:
        rs = list(by_arm[a].values())
        if not rs:
            continue
        k, n = sum(1 for r in rs if r["correct"]), len(rs)
        lo, hi = wilson(k, n)
        ing = sum(r["ingested"] for r in rs) / n
        print(f"  {a:<13} {100*k/n:>5.1f}%  [{100*lo:>4.1f}, {100*hi:>5.1f}]  n={n:<4} "
              f"ingest {ing:>8.0f}B")

    print("\npaired deltas — only the runs where the two arms DISAGREED carry information")
    for a, b in [("waggle", "copy"), ("waggle+gate", "waggle"),
                 ("waggle+gate", "copy"), ("waggle", "reference")]:
        shared = set(by_arm[a]) & set(by_arm[b])
        a_only = sum(1 for k in shared if by_arm[a][k]["correct"] and not by_arm[b][k]["correct"])
        b_only = sum(1 for k in shared if by_arm[b][k]["correct"] and not by_arm[a][k]["correct"])
        disc = a_only + b_only
        if disc == 0:
            print(f"  {a:>11} vs {b:<11} identical on all {len(shared)} shared pairs")
            continue
        # Sign test on the discordant pairs: under "no difference" each is a coin
        # flip, so the chance of a split this lopsided (or worse), either way, is:
        p = min(1.0, 2 * sum(math.comb(disc, i) for i in range(min(a_only, b_only) + 1))
                / (2 ** disc))
        verdict = ("cannot tell — too few disagreements" if disc < 6
                   else "REAL (p<0.05)" if p < 0.05 else f"not significant (p={p:.2f})")
        print(f"  {a:>11} vs {b:<11} {a} wins {a_only:>3}, {b} wins {b_only:>3}  "
              f"of {len(shared)} pairs  ->  {verdict}")

    print("\nper shape (accuracy by arm)")
    shapes = sorted({r["modality"] for r in runs})
    print(f"  {'shape':<15}" + "".join(f"{a:>14}" for a in ARMS))
    for s in shapes:
        row = f"  {s:<15}"
        for a in ARMS:
            rs = [r for r in by_arm[a].values() if r["modality"] == s]
            row += (f"{100*sum(1 for r in rs if r['correct'])/len(rs):>10.0f}% ({len(rs):>1})"
                    if rs else f"{'—':>14}")
        print(row)
    return 0


if __name__ == "__main__":
    sys.exit(main())
