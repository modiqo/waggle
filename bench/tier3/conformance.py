"""Zero-LLM conformance suite: find every substrate/harness fault BEFORE the sweep.

We spent a day discovering bugs with a language model as the detector. Every one
of them was DETERMINISTIC — a zero-match search forging a receipt, a projection
truncating in silence, a fan-out dropping two files of eleven, a response
overrunning the caller's byte budget, the harness showing the model half of what
waggle served while the receipt certified all of it. Not one of those needed a
model to find. Each cost twenty minutes and a sweep to notice, and each was
noticed alone, so the next sweep found the next one.

This is the detector that should have existed first. It drives the substrate with
a SCRIPTED consumer — no model, no tokens, no nondeterminism — and asserts the
properties the benchmark depends on. It runs in seconds and reports EVERY failure
at once, so the fixes land in one batch instead of one per expensive run.

Two consumers are simulated:

  the ORACLE   — plays perfectly: uses the affordances waggle advertises, in the
                 order waggle advertises them. If the oracle cannot satisfy a
                 contract in a few calls, no model will, and the contract is a
                 trap rather than a check.

  the LAZY one — answers having read nothing. The gate MUST refuse it. A gate that
                 passes this is not a gate.

Green here is the precondition for spending money on a sweep.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

WAGGLE = os.environ.get("WAGGLE_BIN", "/tmp/tier3/bin/waggle-under-test")
VIEW_BYTES = 8000          # must match bench4.VIEW_BYTES — what a consumer SEES

FAILS: list[str] = []
CHECKS = [0]


def check(cond: bool, name: str, detail: str = "") -> bool:
    CHECKS[0] += 1
    if not cond:
        FAILS.append(f"{name}: {detail}")
    return cond


def wag(args: list[str]) -> dict:
    p = subprocess.run([WAGGLE, *args], capture_output=True, text=True, timeout=120)
    try:
        return json.loads(p.stdout)
    except Exception:
        return {}


def result(args: list[str]) -> dict:
    return (wag(args).get("result") or {})


def mint_tree(path: str, require_all: bool) -> str:
    a = ["mint", "--target", path, "--tree"]
    if require_all:
        a += ["--require", "files:all"]
    return result(a).get("token", "")


# ---------------------------------------------------------------- invariants
def inv_budget_respected(tok: str, kind: str, val: str) -> None:
    """A byte budget is a CONTRACT WITH THE CALLER.

    We returned 9,255 bytes against a budget of 8,000. The client showed its model
    the first 4,500 and dropped the rest — while our receipt certified all ten
    files as read. A response that overruns the budget is a receipt that lies,
    because we cannot know what the client truncates.
    """
    env = wag(["read", "--token", tok, f"--{kind}", val, "--max-bytes", str(VIEW_BYTES)])
    n = len(json.dumps(env.get("result") or {}))
    check(n <= VIEW_BYTES, "budget-respected",
          f"asked {VIEW_BYTES}B, response {n}B — the consumer will not see {n - VIEW_BYTES}B "
          f"of what our receipt claims it read")


def inv_zero_match_forges_nothing(path: str) -> None:
    """A grep that matches nothing must not stamp `read` on the files it swept.

    One zero-match search once took a folder from 0/11 met=false to 11/11
    met=true. Bytes touched by the matcher and content served to the consumer are
    different ledgers; only the second is a receipt.
    """
    tok = mint_tree(path, True)
    before = result(["coverage", "--token", tok]).get("read")
    wag(["search", "--token", tok, "--pattern", "ZZQQ_NO_SUCH_STRING_ZZQQ"])
    after = result(["coverage", "--token", tok]).get("read")
    check(before == after, "zero-match-forges-nothing",
          f"coverage moved {before} -> {after} on a search that matched NOTHING")


def inv_projection_is_honest(path: str, n_files: int) -> None:
    """A truncated listing must say it is truncated, and hand back the way on."""
    tok = mint_tree(path, False)
    env = wag(["read", "--token", tok])
    d = env.get("result") or {}
    check(d.get("total_files") == n_files, "projection-knows-its-denominator",
          f"total_files={d.get('total_files')} but the tree has {n_files}")
    if not d.get("complete"):
        check(bool(d.get("hint")), "projection-truncation-is-loud",
              "listing is incomplete and says nothing about it")
        cursors = [x for x in (env.get("next") or []) if x["args"].get("from") is not None]
        check(bool(cursors), "projection-truncation-is-resumable",
              "listing is incomplete and hands back no cursor")


def inv_fanout_covers_or_pages(path: str, n_files: int, section: str, fact: str) -> None:
    """The fan-out must serve WHOLE files, never drop them silently, and page.

    `max_bytes / 6` — a hardcoded six-file assumption — served nine of eleven and
    dropped two. That made `--require files:all` unsatisfiable in one call BY
    CONSTRUCTION. And the naive repair (thin every file to fit) is worse: the
    load-bearing sentence is at the END of a section, so a thin prefix of every
    file serves everything and cuts the fact out of each. Full coverage, zero
    information.
    """
    tok = mint_tree(path, True)
    seen, page, guard = 0, 0, 0
    frm = None
    while guard < 12:
        guard += 1
        a = ["read", "--token", tok, "--section", section, "--max-bytes", str(VIEW_BYTES)]
        if frm is not None:
            a += ["--from", str(frm)]
        env = wag(a)
        d = env.get("result") or {}
        files = d.get("files") or []
        seen += len(files)
        page += 1
        # every served file keeps the fact — depth was not silently thinned away
        intact = sum(1 for f in files if fact in (f.get("text") or ""))
        check(intact == len(files), "fanout-keeps-the-fact",
              f"{len(files) - intact} of {len(files)} served files had the load-bearing "
              f"line truncated away — coverage without information")
        if d.get("complete"):
            break
        nxt = [x for x in (env.get("next") or []) if x["args"].get("from") is not None]
        if not check(bool(nxt), "fanout-truncation-is-resumable",
                     "fan-out incomplete and no `from` cursor offered"):
            return
        frm = nxt[0]["args"]["from"]
    check(page <= 3, "fanout-completes-cheaply",
          f"took {page} pages to fan out over {n_files} files — a contract this "
          f"expensive is a trap, not a check")


def inv_contract_satisfiable_by_oracle(path: str, section: str) -> None:
    """A perfect consumer must be able to CLOSE a files:all contract in few calls.

    If the oracle cannot, no model will: it will burn its turns and answer nothing,
    and the gate will (correctly, uselessly) refuse it. That is what happened —
    gpt-4o-mini HELD the right answer, was refused four times, and died at the cap
    while the ungated arm simply gave the same answer and was believed.
    """
    tok = mint_tree(path, True)
    calls = 0

    # play exactly what waggle advertises, in the order it advertises it
    frm = None
    for _ in range(6):
        a = ["read", "--token", tok, "--section", section, "--max-bytes", str(VIEW_BYTES)]
        if frm is not None:
            a += ["--from", str(frm)]
        env = wag(a)
        calls += 1
        d = env.get("result") or {}
        if d.get("complete"):
            break
        nxt = [x for x in (env.get("next") or []) if x["args"].get("from") is not None]
        if not nxt:
            break
        frm = nxt[0]["args"]["from"]

    # files with no such section are never served, so read them directly
    for _ in range(6):
        cov = result(["coverage", "--token", tok])
        unread = cov.get("unread") or []
        if not unread:
            break
        wag(["read", "--token", unread[0]["token"]])
        calls += 1

    cov = result(["coverage", "--token", tok])
    check(cov.get("met") is True, "oracle-can-close-the-contract",
          f"a PERFECT consumer ended at read={cov.get('read')} met={cov.get('met')} — "
          f"the contract is unsatisfiable, so the gate is a trap")
    check(calls <= 5, "contract-closes-cheaply",
          f"oracle needed {calls} calls to satisfy files:all — models have ~10 turns "
          f"and must also reason; this will burn them out")


def inv_gate_refuses_the_lazy(path: str) -> None:
    """A consumer that read nothing must not be believed. Else there is no gate."""
    tok = mint_tree(path, True)
    cov = result(["coverage", "--token", tok])
    check(cov.get("met") is False, "gate-refuses-the-lazy",
          f"a consumer that has read NOTHING shows met={cov.get('met')}")


def inv_search_identifies_its_files(path: str, pattern: str) -> None:
    """A tree search must say WHICH file matched, or the matches are unusable."""
    tok = mint_tree(path, False)
    d = result(["search", "--token", tok, "--pattern", pattern, "--context", "0"])
    files = d.get("files") or []
    check(bool(files), "search-finds-matches", f"no matches for {pattern!r}")
    for f in files[:3]:
        check(bool(f.get("target") or f.get("name")), "search-identifies-its-files",
              "a matched file carries no name/target — the consumer cannot tell which")
        check(bool(f.get("token")), "search-hands-back-a-token",
              "a matched file carries no token — the consumer cannot open it")


def main() -> int:
    corpus2 = os.environ.get("TIER3_CORPUS2", "/tmp/tier3/corpus2")
    corpus3 = os.environ.get("TIER3_CORPUS3", "/tmp/tier3/corpus3")
    reasoning = f"{corpus2}/reasoning_0"
    folder = f"{corpus2}/folder_0"
    bigtree = f"{corpus3}/bigtree_0"

    print(f"substrate under test: {WAGGLE}")
    h = subprocess.run(["shasum", "-a", "256", WAGGLE], capture_output=True, text=True)
    print(f"  sha256 {h.stdout.split()[0][:16] if h.stdout else '?'}\n")

    # --- the reasoning tree: 11 files, a files:all contract, the gate's home turf
    inv_gate_refuses_the_lazy(reasoning)
    inv_zero_match_forges_nothing(reasoning)
    inv_budget_respected(mint_tree(reasoning, True), "section", "Retry Policy")
    inv_fanout_covers_or_pages(reasoning, 11, "Retry Policy", "retry budget of")
    inv_contract_satisfiable_by_oracle(reasoning, "Retry Policy")
    inv_search_identifies_its_files(reasoning, "retry budget of")

    # --- a small folder
    inv_projection_is_honest(folder, 12)

    # --- the big tree: where the projection MUST truncate
    inv_projection_is_honest(bigtree, 180)
    inv_zero_match_forges_nothing(bigtree)
    inv_search_identifies_its_files(bigtree, "legacy_reconcile")
    inv_budget_respected(mint_tree(bigtree, False), "section", "Overview")

    print(f"{CHECKS[0]} checks, {len(FAILS)} FAILED\n")
    for f in FAILS:
        print(f"  FAIL  {f}")
    if not FAILS:
        print("  all green — the substrate is fit to measure")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
