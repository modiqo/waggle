"""Tier-3 — SWE-bench-Lite cost-at-fixed-quality frontier (design doc 22 §4).

The experiment isolates the *handoff mechanism*, not retrieval. For each
SWE-bench-Lite instance the orchestrator fixes one candidate file set (the
files the fix lives in, plus distractors). Both arms see the SAME
information; only the delivery differs:

  copy    — every candidate file's full text is pasted into the prompt.
  waggle  — one ~30-byte token per file; the subagent interrogates through
            the real substrate (read overview / read --symbol / search) and
            pulls only the slices it needs. Receipts are recorded.

Both arms return edits, which become a git-applyable patch, graded by the
official SWE-bench harness (FAIL_TO_PASS + PASS_TO_PASS in Docker).

Measured per (instance, model, strategy): resolved (test-graded), tokens,
dollars, and — for waggle — the interrogation ops the receipts recorded.
"""

from __future__ import annotations

import difflib
import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field, asdict

import rote_models as M

WORK = os.environ.get("TIER3_WORK", "/tmp/tier3")
REPOS = f"{WORK}/repos"
OUT = f"{WORK}/out"
MAX_TURNS = 8
MAX_TOK = 4000

# $ per 1M tokens (input, output). Public list prices at time of run.
PRICING = {
    "claude-opus-4-1-20250805": (15.0, 75.0),
    "claude-sonnet-4-5-20250929": (3.0, 15.0),
    "claude-haiku-4-5-20251001": (1.0, 5.0),
    "gpt-5": (1.25, 10.0),
    "gpt-5-mini": (0.25, 2.0),
    "gpt-4.1": (2.0, 8.0),
    "gpt-4.1-mini": (0.4, 1.6),
    "gpt-4o": (2.5, 10.0),
    "gpt-4o-mini": (0.15, 0.6),
}
FAMILY = {m: ("anthropic" if m in M.ANTHROPIC else "openai") for m in PRICING}


@dataclass
class Run:
    instance_id: str
    model: str
    strategy: str
    patch: str = ""
    in_tokens: int = 0
    out_tokens: int = 0
    ops: int = 0
    turns: int = 0
    error: str = ""
    resolved: bool = False

    @property
    def dollars(self) -> float:
        pin, pout = PRICING[self.model]
        return (self.in_tokens * pin + self.out_tokens * pout) / 1_000_000


# ---------------------------------------------------------------- repo prep
def sh(cmd: list[str], cwd: str | None = None) -> str:
    r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=900)
    return r.stdout


def ensure_repo(inst) -> str:
    """Clone the repo at base_commit once; return the checkout path.

    Called serially before the sweep — workers must never race on a checkout.
    """
    d = f"{REPOS}/{inst['instance_id']}"
    if os.path.isdir(f"{d}/.git"):
        return d
    os.makedirs(REPOS, exist_ok=True)
    subprocess.run(["rm", "-rf", d], check=False)
    url = f"https://github.com/{inst['repo']}.git"
    subprocess.run(["git", "clone", "-q", url, d], check=True, timeout=1800)
    subprocess.run(["git", "checkout", "-q", inst["base_commit"]], cwd=d, check=True)
    return d


def gold_files(inst) -> list[str]:
    return re.findall(r"^diff --git a/(\S+)", inst["patch"], re.M)


def candidates(inst, repo: str, n_distract: int = 3) -> list[str]:
    """The handoff: the file(s) the fix lives in + same-package distractors."""
    gold = [p for p in gold_files(inst) if os.path.isfile(f"{repo}/{p}")]
    picks = list(gold)
    if gold:
        pkg = os.path.dirname(gold[0])
        sibs = sorted(
            f"{pkg}/{f}"
            for f in os.listdir(f"{repo}/{pkg}")
            if f.endswith(".py") and f"{pkg}/{f}" not in gold
        )
        # Deterministic distractors: largest siblings (the costly ones to paste).
        sibs.sort(key=lambda p: -os.path.getsize(f"{repo}/{p}"))
        picks += sibs[:n_distract]
    return picks


# ---------------------------------------------------------------- patching
def apply_edit(src: str, old: str, new: str) -> str | None:
    """Apply one old→new edit, tolerating whitespace/indent drift.

    The waggle arm shows the model *slices*, so its `old` snippet can differ
    from the file in trailing whitespace or indentation. A strict substring
    match would fail those edits and penalise the arm for a harness artifact,
    so fall back to matching a contiguous block of lines by their stripped
    form (which is what the model actually saw).
    """
    if old in src:
        return src.replace(old, new, 1)

    src_lines = src.splitlines(True)
    old_lines = [l for l in old.splitlines() if l.strip()]
    if not old_lines:
        return None
    want = [l.strip() for l in old_lines]
    n = len(want)
    for i in range(len(src_lines) - n + 1):
        window = [l.strip() for l in src_lines[i : i + n]]
        if window == want:
            return "".join(src_lines[:i]) + (new if new.endswith("\n") else new + "\n") + "".join(
                src_lines[i + n :]
            )
    return None


def make_patch(repo: str, edits: list[dict]) -> str:
    """Apply old→new edits in memory; emit a git-applyable unified diff."""
    out = []
    for e in edits or []:
        if not isinstance(e, dict):
            continue
        p = (e.get("path") or "").lstrip("./")
        old, new = e.get("old") or "", e.get("new") or ""
        fp = f"{repo}/{p}"
        if not (p and old and os.path.isfile(fp)):
            continue
        src = open(fp, encoding="utf-8", errors="replace").read()
        dst = apply_edit(src, old, new)
        if not dst or dst == src:
            continue
        body = "".join(
            difflib.unified_diff(
                src.splitlines(True), dst.splitlines(True), f"a/{p}", f"b/{p}", n=3
            )
        )
        if body:
            out.append(f"diff --git a/{p} b/{p}\n{body}")
    return "".join(out)


# ---------------------------------------------------------------- waggle arm
def wag(args: list[str]) -> dict:
    r = subprocess.run(["waggle", *args], capture_output=True, text=True, timeout=120)
    try:
        # `result` can be present-but-null on an error reply — coerce to {}.
        return json.loads(r.stdout).get("result") or {}
    except Exception:
        return {}


def mint_files(repo: str, files: list[str]) -> dict[str, str]:
    toks = {}
    for p in files:
        res = wag(["mint", "--target", f"{repo}/{p}", "--snapshot"])
        if res.get("token"):
            toks[p] = res["token"]
    return toks


def waggle_exec(cmd: dict, toks: dict[str, str]) -> str:
    """Execute one interrogation command against the real substrate."""
    c = cmd.get("cmd")
    tok = toks.get(cmd.get("path", ""), "")
    if not tok:
        return f"error: unknown path {cmd.get('path')!r}; known: {list(toks)}"
    if c == "overview":
        r = wag(["read", "--token", tok])
        syms = r.get("symbols", {}).get("symbols", [])
        return json.dumps(
            {
                "lines": r.get("total_lines"),
                "bytes": r.get("total_bytes"),
                "symbols": [f"{s['name']} ({s['kind']}) L{s['lines']}" for s in syms],
            }
        )
    if c == "symbol":
        r = wag(["read", "--token", tok, "--symbol", cmd.get("symbol", ""), "--max-bytes", "3000"])
        return json.dumps({"lines": r.get("lines"), "text": r.get("text", "")})
    if c == "lines":
        r = wag(["read", "--token", tok, "--lines", cmd.get("lines", ""), "--max-bytes", "3000"])
        return json.dumps({"lines": r.get("lines"), "text": r.get("text", "")})
    if c == "search":
        r = wag(["read" if False else "search", "--token", tok,
                 "--pattern", cmd.get("pattern", ""), "--max-matches", "8", "--max-bytes", "3000"])
        return json.dumps({"matches": r.get("matches", [])})
    return f"error: unknown cmd {c!r}"


# ---------------------------------------------------------------- prompts
SYSTEM = """You are a senior engineer fixing a bug in a Python repository.
Reply with EXACTLY ONE JSON object and nothing else (no prose, no fences).

When you are ready to fix, reply:
{"cmd":"submit","edits":[{"path":"<file>","old":"<exact snippet from the file>","new":"<replacement>"}]}
`old` must be a VERBATIM substring of the current file (copy it exactly,
including indentation) and unique enough to match once. Keep edits minimal."""

WAGGLE_PROTO = """You may first interrogate the files through waggle. Each file is a
token; you never receive the whole file unless you ask. Commands:
{"cmd":"overview","path":"<file>"}            -> size + symbol table (cheap; start here)
{"cmd":"symbol","path":"<file>","symbol":"<name>"}  -> that symbol's exact source
{"cmd":"lines","path":"<file>","lines":"120-180"}   -> a line window
{"cmd":"search","path":"<file>","pattern":"<regex>"} -> matches with line numbers
Pull only what you need, then submit."""


def user_copy(inst, repo: str, files: list[str]) -> str:
    parts = [f"## Problem\n{inst['problem_statement'][:6000]}\n\n## Files"]
    for p in files:
        src = open(f"{repo}/{p}", encoding="utf-8", errors="replace").read()
        parts.append(f"\n### {p}\n```python\n{src}\n```")
    parts.append("\nSubmit your fix now as the submit JSON.")
    return "\n".join(parts)


def user_waggle(inst, files: list[str]) -> str:
    listing = "\n".join(f"- {p}" for p in files)
    return (
        f"## Problem\n{inst['problem_statement'][:6000]}\n\n"
        f"## Files available through waggle (interrogate, don't assume)\n{listing}\n\n"
        f"{WAGGLE_PROTO}"
    )


def parse_json(text: str) -> dict | None:
    t = text.strip()
    t = re.sub(r"^```(?:json)?|```$", "", t, flags=re.M).strip()
    try:
        return json.JSONDecoder().raw_decode(t[t.find("{"):])[0]
    except Exception:
        return None


# ---------------------------------------------------------------- the agent
def run_one(inst, model: str, strategy: str) -> Run:
    r = Run(inst["instance_id"], model, strategy)
    try:
        repo = ensure_repo(inst)
        files = candidates(inst, repo)
        if not files:
            r.error = "no candidate files"
            return r

        # Both arms get the same turn budget; only the delivery differs.
        if strategy == "copy":
            system, convo, toks = SYSTEM, user_copy(inst, repo, files), {}
        else:
            toks = mint_files(repo, files)
            system, convo = SYSTEM + "\n\n" + WAGGLE_PROTO, user_waggle(inst, files)

        for turn in range(MAX_TURNS):
            rep = M.call(model, system, convo, max_tokens=MAX_TOK)
            r.in_tokens += rep.in_tokens
            r.out_tokens += rep.out_tokens
            r.turns = turn + 1
            obj = parse_json(rep.text)
            if not obj:
                convo += "\n\nYour reply was not a single JSON object. Reply with ONLY the submit JSON."
                continue
            if obj.get("cmd") == "submit":
                r.patch = make_patch(repo, obj.get("edits", []))
                if r.patch:
                    break
                convo += ("\n\nThat edit did not apply: `old` must be a VERBATIM substring of the "
                          "current file. Re-read the exact text and submit again.")
                continue
            if strategy == "copy":
                convo += "\n\nThe full file contents are already above. Submit the fix now."
                continue
            r.ops += 1
            result = waggle_exec(obj, toks)
            convo += f"\n\n> {json.dumps(obj)}\n{result[:4000]}"
        if not r.patch and not r.error:
            r.error = "no patch"
    except Exception as e:  # keep the sweep alive
        r.error = f"{type(e).__name__}: {e}"[:160]
    return r


# ---------------------------------------------------------------- grading
def grade(runs: list[Run], dataset: str, venv_py: str) -> None:
    """Grade one (model, strategy) group with the official SWE-bench harness."""
    by_group: dict[tuple[str, str], list[Run]] = {}
    for r in runs:
        by_group.setdefault((r.model, r.strategy), []).append(r)

    os.makedirs(OUT, exist_ok=True)
    for (model, strat), group in by_group.items():
        tag = f"{model}__{strat}".replace("/", "_").replace(".", "-")
        preds = [
            {
                "instance_id": g.instance_id,
                "model_name_or_path": tag,
                "model_patch": g.patch,
            }
            for g in group
            if g.patch
        ]
        if not preds:
            continue
        pf = f"{OUT}/preds_{tag}.json"
        json.dump(preds, open(pf, "w"))
        subprocess.run(
            [venv_py, "-m", "swebench.harness.run_evaluation",
             "--dataset_name", dataset, "--predictions_path", pf,
             "--run_id", tag, "--max_workers", "4", "--cache_level", "instance",
             "--timeout", "1800"],
            cwd=OUT, capture_output=True, text=True, timeout=7200,
        )
        rep_path = f"{OUT}/{tag}.{tag}.json"
        if os.path.isfile(rep_path):
            rep = json.load(open(rep_path))
            done = set(rep.get("resolved_ids", []))
            for g in group:
                g.resolved = g.instance_id in done


# ---------------------------------------------------------------- main
def main() -> None:
    from datasets import load_dataset

    k = int(os.environ.get("TIER3_K", "10"))
    models = os.environ.get("TIER3_MODELS", ",".join(PRICING)).split(",")
    strategies = os.environ.get("TIER3_STRATS", "copy,waggle").split(",")
    dataset = "princeton-nlp/SWE-bench_Lite"
    venv_py = os.environ["TIER3_VENV_PY"]

    ds = load_dataset(dataset, split="test")
    light = [x for x in ds if x["repo"] in
             ("psf/requests", "pallets/flask", "marshmallow-code/marshmallow",
              "pylint-dev/pylint", "pydicom/pydicom", "sqlfluff/sqlfluff")]
    light.sort(key=lambda x: len(x["patch"]))
    insts = light[:k]
    print(f"instances: {[i['instance_id'] for i in insts]}", flush=True)

    # Clone serially up front: parallel workers must never race on a checkout.
    os.makedirs(OUT, exist_ok=True)
    for i in insts:
        ensure_repo(i)
    print("repos ready", flush=True)

    jobs = [(i, m, s) for i in insts for m in models for s in strategies]
    print(f"runs: {len(jobs)} ({len(insts)} inst x {len(models)} models x {len(strategies)} strat)", flush=True)

    runs: list[Run] = []
    with ThreadPoolExecutor(max_workers=4) as ex:
        futs = [ex.submit(run_one, i, m, s) for (i, m, s) in jobs]
        for n, f in enumerate(futs, 1):
            r = f.result()
            runs.append(r)
            print(f"[{n}/{len(futs)}] {r.model:28} {r.strategy:6} {r.instance_id:22} "
                  f"tok={r.in_tokens + r.out_tokens:7} ops={r.ops} "
                  f"{'patch' if r.patch else 'NO-PATCH ' + r.error}", flush=True)

    json.dump([asdict(r) for r in runs], open(f"{OUT}/runs_pregrade.json", "w"), indent=1)
    print("grading with the official SWE-bench harness...", flush=True)
    grade(runs, dataset, venv_py)

    rows = [{**asdict(r), "dollars": r.dollars, "family": FAMILY[r.model]} for r in runs]
    json.dump(rows, open(f"{OUT}/runs.json", "w"), indent=1)
    print(f"wrote {OUT}/runs.json", flush=True)


if __name__ == "__main__":
    sys.exit(main())
