"""Tier-3 v3 corpus: the shapes real work actually has (doc 22 §4.3).

The v2 corpus used single artifacts of ~24 KB — small enough for a context
window to simply hold, which is the one regime where pasting wins. These are
the shapes people actually hand to agents:

  folder    a directory of many markdown runbooks (~100 KB total). Pasting
            means pasting *every* file. waggle mints it with --tree: one
            parent token whose search greps the whole tree and hands back the
            child token for each hit.
  bigcode   a real source file of >2000 lines. Too big to paste comfortably;
            the symbol lens should shine.
  multihop  the answer is SPLIT into three fragments in three different
            sections of a long document. One lucky grep cannot win it: the
            consumer must reach three regions, and the contract requires all
            three — so coverage finally measures multi-region consumption
            rather than a single needle.
"""

from __future__ import annotations

import json
import os
import random
import subprocess
import sys

OUT = os.environ.get("TIER3_CORPUS2", "/tmp/tier3/corpus2")
REPOS = os.environ.get("TIER3_REPOS", "/tmp/tier3/repos")
SEED = 20260712
N_PER = int(os.environ.get("TIER3_N2", "4"))

FILLER = (
    "The pipeline stage reconciles inbound records against the ledger before "
    "the nightly compaction window. Operators should note that back-pressure "
    "is applied per shard, not per partition, and that retries are bounded by "
    "the lease horizon rather than by wall-clock time. Escalation follows the "
    "on-call rotation; the secondary is paged only after the primary's lease "
    "expires. "
)


def code(rng) -> str:
    return f"{rng.choice('QWXZ')}{rng.randint(1,9)}{rng.choice('KMPR')}-{rng.randint(1000,9999)}"


def runbook(n: int, needle: str | None) -> str:
    parts = [f"# Runbook {n}: Recovery Path\n\n"]
    for s in range(1, 9):
        parts.append(f"## Stage {s}: Reconciliation\n\n{FILLER * 3}\n\n")
        if needle and s == 5:
            parts.append(f"AUDIT CODE: {needle}\n\n")
    return "".join(parts)


# ----------------------------------------------------------------- folder
def make_folder(i: int, rng, n_files: int = 12):
    d = f"{OUT}/folder_{i}"
    os.makedirs(d, exist_ok=True)
    c = code(rng)
    hit = rng.randrange(1, n_files + 1)
    total = 0
    for f in range(1, n_files + 1):
        body = runbook(f, c if f == hit else None)
        p = f"{d}/runbook_{f:02d}.md"
        open(p, "w").write(body)
        total += len(body)
    return d, c, f"runbook_{hit:02d}.md", total


# ----------------------------------------------------------------- bigcode
def big_sources(min_lines: int = 2000) -> list[str]:
    out = []
    for root, _, files in os.walk(REPOS):
        for f in files:
            if not f.endswith(".py"):
                continue
            p = os.path.join(root, f)
            try:
                if sum(1 for _ in open(p, encoding="utf-8", errors="replace")) >= min_lines:
                    out.append(p)
            except OSError:
                pass
    out.sort()
    return out


def make_bigcode(src: str, rng):
    lines = open(src, encoding="utf-8", errors="replace").read().splitlines(True)
    cand = []
    for i, l in enumerate(lines):
        s = l.lstrip()
        if not (s.startswith("def ") and l.rstrip().endswith(":") and "(" in l):
            continue
        nm = s[4:].split("(")[0].strip()
        if nm.startswith("_"):
            continue
        if sum(1 for x in lines if x.lstrip().startswith(f"def {nm}(")) != 1:
            continue
        cand.append((i, l, nm))
    if not cand:
        return None
    i, l, name = cand[rng.randrange(len(cand))]
    indent = " " * (len(l) - len(l.lstrip()) + 4)
    c = code(rng)
    lines.insert(i + 1, f'{indent}AUDIT_CODE = "{c}"  # operational audit marker\n')
    return "".join(lines), c, name, len(lines)


# ----------------------------------------------------------------- multihop
CODENAMES = [
    "Vermilion Cascade", "Basalt Harbour", "Cobalt Meridian", "Umber Lattice",
    "Cinnabar Reach", "Slate Ferrule", "Marigold Bastion", "Onyx Culvert",
]


def make_multihop(i: int, rng, n_sec: int = 18):
    """A genuine POINTER CHAIN, not three copies of one greppable marker.

    A single `grep "AUDIT PART"` would have found all three fragments at once,
    which is not multi-hop at all — it is one lucky lexical shot. Here each
    hop's location is a codename you can only learn from the *previous* hop.
    You cannot grep for a section whose name you have not read yet: you have to
    navigate. Both arms must take three hops, so this tests navigation, not
    grep-immunity — which is the honest comparison.
    """
    c = code(rng)                        # e.g. Z9K-8782
    head, tail = c.split("-")
    frags = [head, tail[:2], tail[2:]]   # Z9K / 87 / 82
    names = rng.sample(CODENAMES, 3)
    slots = sorted(rng.sample(range(2, n_sec), 3))

    titles = [f"Runbook {s}: Recovery Path" for s in range(1, n_sec + 1)]
    for k, s in enumerate(slots):        # the three hops get their codenames
        titles[s - 1] = names[k]

    body = ["# Operations Handbook\n\n"]
    for s in range(1, n_sec + 1):
        body.append(f"## {titles[s-1]}\n\n{FILLER * 4}\n\n")
        if s in slots:
            k = slots.index(s)
            if k < 2:
                body.append(
                    f"The recovery key continues with `{frags[k]}`. "
                    f"The next fragment is held in the runbook named "
                    f"**{names[k+1]}**.\n\n"
                )
            else:
                body.append(
                    f"The recovery key ends with `{frags[k]}`. The key is now complete.\n\n"
                )
    return "".join(body), c, [titles[s - 1] for s in slots], names[0]


# ----------------------------------------------------------------- reasoning
def make_reasoning(i: int, rng, n_files: int = 10):
    """Not a needle: a JUDGEMENT that requires reading several regions.

    Each runbook declares a retry budget. The escalation policy declares a
    ceiling. The question asks which runbook VIOLATES the policy — which cannot
    be grepped, because no single line contains the answer: it exists only in
    the relation between two regions. Retrieval finds strings; this needs the
    consumer to read, compare, and decide.
    """
    d = f"{OUT}/reasoning_{i}"
    os.makedirs(d, exist_ok=True)
    ceiling = rng.choice([3, 4, 5])
    budgets, total = {}, 0
    violator = rng.randrange(1, n_files + 1)
    for f in range(1, n_files + 1):
        b = ceiling + rng.randint(1, 3) if f == violator else rng.randint(1, ceiling)
        budgets[f] = b
        body = (f"# Runbook {f}: Recovery Path\n\n## Retry Policy\n\n{FILLER*2}\n\n"
                f"This runbook sets a retry budget of {b} attempts before escalation.\n\n"
                f"## Stage 2: Reconciliation\n\n{FILLER*3}\n\n")
        p = f"{d}/runbook_{f:02d}.md"
        open(p, "w").write(body)
        total += len(body)
    pol = (f"# Escalation Policy\n\n## Ceiling\n\n{FILLER*2}\n\n"
           f"No runbook may set a retry budget exceeding {ceiling} attempts.\n\n")
    open(f"{d}/00_escalation_policy.md", "w").write(pol)
    total += len(pol)
    return d, f"runbook_{violator:02d}", total, ceiling, budgets


# ----------------------------------------------------------------- build
def main() -> int:
    rng = random.Random(SEED)
    os.makedirs(OUT, exist_ok=True)
    items = []

    bigs = big_sources()
    print(f"source files >2000 lines available: {len(bigs)}")

    for i in range(N_PER):
        # folder of many markdown files
        d, c, where, total = make_folder(i, rng)
        items.append(dict(id=f"folder_{i}", modality="folder", path=d, answer=c,
                          region="Stage 5: Reconciliation", region_kind="section",
                          bytes=total, note=where))

        # a large real codebase file
        if bigs:
            got = make_bigcode(bigs[rng.randrange(len(bigs))], rng)
            if got:
                body, c2, sym, nlines = got
                p = f"{OUT}/bigcode_{i}.py"
                open(p, "w").write(body)
                items.append(dict(id=f"bigcode_{i}", modality="bigcode", path=p, answer=c2,
                                  region=sym, region_kind="symbol", bytes=len(body),
                                  note=f"{nlines} lines"))

        # multi-hop: three fragments, three regions
        body, c3, regions, entry = make_multihop(i, rng)
        p = f"{OUT}/multihop_{i}.md"
        open(p, "w").write(body)
        items.append(dict(id=f"multihop_{i}", modality="multihop", path=p, answer=c3,
                          region=regions, region_kind="sections", bytes=len(body),
                          entry=entry, note="pointer chain: 3 hops"))

        # a REASONING task: the answer is in the relation between regions
        d, viol, total, ceiling, _ = make_reasoning(i, rng)
        items.append(dict(id=f"reasoning_{i}", modality="reasoning", path=d, answer=viol,
                          region="Retry Policy", region_kind="section", bytes=total,
                          note=f"ceiling {ceiling}"))

    json.dump(items, open(f"{OUT}/manifest.json", "w"), indent=1)
    by: dict[str, int] = {}
    for it in items:
        by[it["modality"]] = by.get(it["modality"], 0) + 1
    print("corpus2:", by)
    for it in items[:3]:
        print(f"  {it['id']:11} {it['bytes']:8}B answer={it['answer']} note={it.get('note')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
