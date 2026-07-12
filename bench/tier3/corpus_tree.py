"""Tier-3 v5 corpus: the ONE shape that can answer the filtering question.

The v3 `folder` shape is 12 files. A 12-file projection fits in any context
window, so the agent never has to narrow it — which means that shape cannot
tell us whether narrowing is *necessary*. It can only tell us it isn't needed
at 12 files. Asking a question of a corpus that cannot answer it is how you
get a benchmark that flatters the product.

`bigtree` is the regime where the answer is forced: ~180 files across a nested
tree, ~450 KB. Now three things are true at once, and they are the whole test:

  - copy must paste the entire tree, or choose blind which files to paste;
  - reference gets a path and must walk it with ls/grep, opening whole files;
  - waggle's own FULL PROJECTION no longer fits either — it truncates.

That last one is the honest part. If the projection truncates, then waggle at
this scale needs a way to get a SUBSET of the tree — and `search` over the
parent is exactly that: it returns only the files that matched, each with its
own child token, which is a filtered listing that never paid for the rest.

So the shape is designed to make waggle's *unfiltered* affordance fail. If
search-over-tree does not rescue it, filtering is necessary and missing, and
the benchmark says so. That is the point.

Two questions, one corpus:
  bigtree_find    a needle in one file of ~180 — can the tree be narrowed at all?
  bigtree_count   how many files declare a deprecated API? — needs the SUBSET,
                  not one hit: a filtered listing is the answer, and an agent
                  that can only fetch one file at a time has to guess.
"""

from __future__ import annotations

import json
import os
import random

OUT = os.environ.get("TIER3_CORPUS3", "/tmp/tier3/corpus3")
SEED = 20260712
N_PER = int(os.environ.get("TIER3_N3", "4"))

FILLER = (
    "The service reconciles inbound records against the ledger before the "
    "nightly compaction window. Back-pressure is applied per shard rather "
    "than per partition, and retries are bounded by the lease horizon rather "
    "than by wall-clock time. Escalation follows the on-call rotation. "
)

AREAS = ["ingest", "ledger", "compaction", "routing", "billing", "identity"]
KINDS = ["handler", "worker", "client", "policy", "store", "codec"]


def _module(area: str, kind: str, n: int, body: str) -> str:
    return (
        f"# {area}/{kind}_{n:02d}\n\n## Overview\n\n{FILLER * 2}\n\n"
        f"## Behaviour\n\n{body}\n\n## Notes\n\n{FILLER}\n"
    )


def make_bigtree(i: int, rng, n_files: int = 180):
    """~180 markdown modules in a nested tree; one holds the needle, and a
    known number of them carry a deprecation marker."""
    d = f"{OUT}/bigtree_{i}"
    code = f"TRW-{rng.randrange(1000, 9999)}"
    files, total = [], 0

    # how many files carry the deprecation marker — the count question's answer
    n_dep = rng.randrange(5, 10)
    dep_idx = set(rng.sample(range(n_files), n_dep))
    needle_idx = rng.randrange(n_files)

    for f in range(n_files):
        area = AREAS[f % len(AREAS)]
        kind = KINDS[(f // len(AREAS)) % len(KINDS)]
        os.makedirs(f"{d}/{area}", exist_ok=True)
        body = FILLER * 3
        if f in dep_idx:
            body += ("\n\nThis module still calls the **deprecated** "
                     "`legacy_reconcile()` entry point.\n")
        if f == needle_idx:
            body += f"\n\nThe teardown reconciliation code is `{code}`.\n"
        text = _module(area, kind, f, body)
        p = f"{d}/{area}/{kind}_{f:03d}.md"
        open(p, "w").write(text)
        files.append(p)
        total += len(text)

    needle_area = AREAS[needle_idx % len(AREAS)]
    needle_kind = KINDS[(needle_idx // len(AREAS)) % len(KINDS)]
    where = f"{needle_area}/{needle_kind}_{needle_idx:03d}.md"
    return d, code, n_dep, total, len(files), where


def main() -> int:
    rng = random.Random(SEED)
    os.makedirs(OUT, exist_ok=True)
    items = []
    for i in range(N_PER):
        d, code, n_dep, total, nf, where = make_bigtree(i, rng)
        items.append(dict(
            id=f"bigtree_find_{i}", modality="bigtree_find", path=d, answer=code,
            region="files:all", region_kind="files", bytes=total,
            note=f"{nf} files; needle in {where}"))
        items.append(dict(
            id=f"bigtree_count_{i}", modality="bigtree_count", path=d,
            answer=str(n_dep), region="files:all", region_kind="files",
            bytes=total, note=f"{nf} files; {n_dep} carry the deprecation marker"))
    json.dump(items, open(f"{OUT}/manifest.json", "w"), indent=1)
    mb = sum(x["bytes"] for x in items[::2]) / 1e6
    print(f"wrote {len(items)} items over {N_PER} trees "
          f"({mb:.2f} MB of tree, {items[0]['note']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
