"""Zero-LLM conformance suite: find every substrate/harness fault BEFORE the sweep.

We spent a day discovering bugs with a language model as the detector. Every one
of them was DETERMINISTIC — a zero-match search forging a receipt, a projection
lying about its size, a search that could not say WHICH file matched, a response
overrunning the caller's byte budget. Not one of those needed a model to find.
Each cost twenty minutes and a sweep to notice, and each was noticed alone, so
the next sweep found the next one.

This is the detector that should have existed first. It drives the substrate with
a SCRIPTED consumer — no model, no tokens, no nondeterminism — and asserts the
properties the benchmark depends on. It runs in seconds and reports EVERY failure
at once, so the fixes land in one batch instead of one per expensive run.

The substrate under test serves a big corpus as an INDEXED TREE (design doc:
tree-scale): `mint --tree` builds one directory node per folder — Merkle-addressed
content, a trigram index, and a Bloom summary per node — instead of one token per
file. That lifts the old per-file cap (a thousand files mint in one call) and
changes the interaction contract, which is what this suite now asserts:

  a folder READ (no address) is a table of contents — local files (name/size/type)
  and subdirectories (name/token/totals), bounded, no truncation;

  a folder READ --file <name> serves ONE file from its content-addressed blob and
  stamps it as a per-file read;

  a SEARCH spans the whole lineage in ONE call — Bloom-pruned, trigram-narrowed,
  regex-confirmed, ranked — and every match names its file (path + owning token);

  COVERAGE is per-file: `files: "read/total"`, `complete`, and the unread files
  NAMED. Reading part of a tree reports part; a search that matches nothing moves
  it not at all.

Two consumers are simulated:

  the ORACLE   — plays perfectly: uses the affordances waggle advertises, in the
                 order it advertises them. If the oracle cannot close per-file
                 coverage in a few calls, no model will.

  the LAZY one — certifies nothing it has not read. Coverage on an untouched tree
                 must report incomplete; a consumer that read nothing is refused.

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


def mint_tree(path: str) -> str:
    return result(["mint", "--target", path, "--tree"]).get("token", "")


def cov_ratio(tok: str) -> tuple[int, int]:
    """Coverage's `files: "read/total"` parsed to (read, total)."""
    s = result(["coverage", "--token", tok]).get("files", "0/0")
    r, _, n = str(s).partition("/")
    return int(r or 0), int(n or 0)


# ---------------------------------------------------------------- invariants
def inv_projection_knows_its_denominator(path: str, n_files: int) -> None:
    """A folder's table of contents must state the true size of the whole tree.

    The denominator is what tells a consumer whether it has seen everything. A
    projection that under- or over-counts the tree is a projection that cannot be
    trusted to say `complete`.
    """
    tok = mint_tree(path)
    d = wag(["read", "--token", tok]).get("result") or {}
    check(d.get("kind") == "tree", "projection-is-a-tree",
          f"read of a --tree token returned kind={d.get('kind')!r}")
    check(d.get("total_files") == n_files, "projection-knows-its-denominator",
          f"total_files={d.get('total_files')} but the tree has {n_files}")


def inv_projection_hands_back_the_way_down(path: str) -> None:
    """Every subdirectory in a listing must carry the token to descend into it,
    and every local file a name to open it — or the tree is a dead end."""
    tok = mint_tree(path)
    d = wag(["read", "--token", tok]).get("result") or {}
    for sub in d.get("dirs") or []:
        check(bool(sub.get("token")), "subdir-carries-a-token",
              f"subdir {sub.get('name')!r} has no token — unreachable")
    for f in d.get("children") or []:
        check(bool(f.get("name")), "file-carries-a-name",
              "a listed file has no name — cannot be opened with --file")


def inv_search_identifies_its_files(path: str, pattern: str) -> None:
    """A tree search must say WHICH file matched, or the matches are unusable."""
    d = result(["search", "--token", mint_tree(path), "--pattern", pattern])
    matches = d.get("matches") or []
    check(bool(matches), "search-finds-matches", f"no matches for {pattern!r}")
    for m in matches[:3]:
        check(bool(m.get("path")), "search-identifies-its-files",
              "a match carries no path — the consumer cannot tell which file")
        check(bool(m.get("token")), "search-hands-back-a-token",
              "a match carries no token — the consumer cannot open it")


def inv_absent_pattern_prunes_the_tree(path: str) -> None:
    """A literal pattern the Bloom excludes everywhere must visit ZERO nodes.

    The prune gate is what makes one-call search over thousands of files viable.
    If an absent needle still walks the whole tree, the index is decoration.
    """
    d = result(["search", "--token", mint_tree(path),
                "--pattern", "ZZQQ_NO_SUCH_STRING_ZZQQ"])
    check(d.get("total_matches") == 0, "absent-pattern-finds-nothing",
          f"a nonsense pattern matched {d.get('total_matches')} files")
    check(d.get("nodes_visited") == 0, "absent-pattern-prunes-the-tree",
          f"an absent literal still walked {d.get('nodes_visited')} nodes — "
          f"the Bloom prune gate did not bite")


def inv_zero_match_forges_nothing(path: str) -> None:
    """A grep that matches nothing must not stamp `read` on the files it swept.

    One zero-match search once took a folder from 0/11 to 11/11. Bytes touched by
    the matcher and content served to the consumer are different ledgers; only the
    second is a receipt.
    """
    tok = mint_tree(path)
    before = cov_ratio(tok)
    wag(["search", "--token", tok, "--pattern", "ZZQQ_NO_SUCH_STRING_ZZQQ"])
    after = cov_ratio(tok)
    check(before == after, "zero-match-forges-nothing",
          f"coverage moved {before[0]}/{before[1]} -> {after[0]}/{after[1]} "
          f"on a search that matched NOTHING")


def inv_coverage_refuses_the_unread(path: str) -> None:
    """A tree nobody has read must report incomplete. Else there is no receipt."""
    tok = mint_tree(path)
    r, n = cov_ratio(tok)
    complete = result(["coverage", "--token", tok]).get("complete")
    check(r == 0 and n > 0 and complete is False, "coverage-refuses-the-unread",
          f"an untouched tree shows files={r}/{n} complete={complete}")


def inv_per_file_receipt_is_precise(path: str, n_files: int) -> None:
    """Reading ONE file of many must report exactly one read — and NAME the rest.

    This is the headline the indexing work stands on: a receipt that says the
    agent opened this file and not that one, not merely "the folder was touched".
    """
    tok = mint_tree(path)
    d = wag(["read", "--token", tok]).get("result") or {}
    names = [f["name"] for f in (d.get("children") or [])]
    if not check(bool(names), "per-file-needs-a-flat-node",
                 f"{path} has no files directly on its root node to probe"):
        return
    opened = names[0]
    served = wag(["read", "--token", tok, "--file", opened]).get("result") or {}
    check(served.get("name") == opened, "read-file-serves-the-named-file",
          f"read --file {opened!r} returned name={served.get('name')!r}")
    r, n = cov_ratio(tok)
    check((r, n) == (1, n_files), "per-file-receipt-is-precise",
          f"opened 1 file, coverage says {r}/{n} (want 1/{n_files})")
    cov = result(["coverage", "--token", tok])
    missing = [f for u in (cov.get("unread") or []) for f in (u.get("first_missing") or [])]
    check(opened not in missing, "read-file-clears-its-own-name",
          f"{opened!r} was read yet still listed unread")
    check(any(nm in missing for nm in names[1:]), "unread-files-are-named",
          "coverage named none of the files left unread")


def inv_search_stamps_every_match(path: str, pattern: str, expect: int) -> None:
    """A search that serves K files must stamp exactly K as read — the one-call
    equivalent of the old fan-out, and cheaper."""
    tok = mint_tree(path)
    d = result(["search", "--token", tok, "--pattern", pattern])
    served = d.get("total_matches")
    r, _ = cov_ratio(tok)
    check(served == expect, "search-serves-the-expected-set",
          f"search {pattern!r} matched {served} files, expected {expect}")
    check(r == served, "search-stamps-every-match",
          f"search served {served} files but coverage moved to {r} read")


def inv_oracle_closes_per_file(path: str, pattern: str, n_files: int) -> None:
    """A perfect consumer must CLOSE per-file coverage in a few calls.

    The move the indexed tree advertises: one search serves the bulk; coverage
    then NAMES the stragglers; read --file opens each by name. If the oracle
    cannot finish cheaply, no model with ten turns and a question to answer will.
    """
    tok = mint_tree(path)
    calls = 0

    # one search serves every file that carries the pattern
    wag(["search", "--token", tok, "--pattern", pattern])
    calls += 1

    # coverage names the files with no such match; open each by name on its node
    for _ in range(n_files):
        cov = result(["coverage", "--token", tok])
        if cov.get("complete"):
            break
        gap = next((u for u in (cov.get("unread") or []) if u.get("first_missing")), None)
        if not gap:
            break
        wag(["read", "--token", gap["token"], "--file", gap["first_missing"][0]])
        calls += 1

    cov = result(["coverage", "--token", tok])
    check(cov.get("complete") is True, "oracle-can-close-per-file-coverage",
          f"a PERFECT consumer ended at files={cov.get('files')} "
          f"complete={cov.get('complete')} — the tree is not closable")
    check(calls <= 4, "per-file-coverage-closes-cheaply",
          f"oracle needed {calls} calls to read all {n_files} files — a corpus "
          f"where one search + a couple of reads will not close is a trap")


def inv_budget_respected(path: str, pattern: str) -> None:
    """The bounded responses — a table of contents, a search result — are a
    CONTRACT WITH THE CALLER: what we return must fit the budget it asked for,
    because we cannot know what the client truncates past it."""
    tok = mint_tree(path)
    toc = wag(["read", "--token", tok, "--max-bytes", str(VIEW_BYTES)])
    n = len(json.dumps(toc.get("result") or {}))
    check(n <= VIEW_BYTES, "toc-fits-the-budget",
          f"table of contents was {n}B against a {VIEW_BYTES}B budget")
    srch = wag(["search", "--token", tok, "--pattern", pattern,
                "--max-matches", "10"])
    n = len(json.dumps(srch.get("result") or {}))
    check(n <= VIEW_BYTES, "search-fits-the-budget",
          f"search result was {n}B against a {VIEW_BYTES}B budget")


def main() -> int:
    corpus2 = os.environ.get("TIER3_CORPUS2", "/tmp/tier3/corpus2")
    corpus3 = os.environ.get("TIER3_CORPUS3", "/tmp/tier3/corpus3")
    reasoning = f"{corpus2}/reasoning_0"   # flat: 11 files, "retry budget of" in 10
    folder = f"{corpus2}/folder_0"         # flat: 12 files
    bigtree = f"{corpus3}/bigtree_0"       # nested: 180 files across subdirectories

    print(f"substrate under test: {WAGGLE}")
    h = subprocess.run(["shasum", "-a", "256", WAGGLE], capture_output=True, text=True)
    print(f"  sha256 {h.stdout.split()[0][:16] if h.stdout else '?'}\n")

    # --- the reasoning tree: a flat folder, the per-file receipt's home turf
    inv_coverage_refuses_the_unread(reasoning)
    inv_zero_match_forges_nothing(reasoning)
    inv_per_file_receipt_is_precise(reasoning, 11)
    inv_search_stamps_every_match(reasoning, "retry budget of", 10)
    inv_oracle_closes_per_file(reasoning, "retry budget of", 11)
    inv_search_identifies_its_files(reasoning, "retry budget of")
    inv_budget_respected(reasoning, "retry budget of")

    # --- a small folder: the projection must know its own size and hand the way on
    inv_projection_knows_its_denominator(folder, 12)
    inv_projection_hands_back_the_way_down(folder)

    # --- the big nested tree: denominator, pruning, and named matches at scale
    inv_projection_knows_its_denominator(bigtree, 180)
    inv_projection_hands_back_the_way_down(bigtree)
    inv_absent_pattern_prunes_the_tree(bigtree)
    inv_search_identifies_its_files(bigtree, "legacy_reconcile")
    inv_budget_respected(bigtree, "legacy_reconcile")

    print(f"{CHECKS[0]} checks, {len(FAILS)} FAILED\n")
    for f in FAILS:
        print(f"  FAIL  {f}")
    if not FAILS:
        print("  all green — the substrate is fit to measure")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
