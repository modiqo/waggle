"""Tier-3 v3 — the substrate at the shapes real work has (doc 22 §4).

Runs the same three turn-matched arms (copy / reference / waggle) over BOTH
corpora:

  the six modalities (text, markdown, code, pdf, voice, video)
  the real shapes  (folder of many files, >2000-line source, multi-hop)

Two things changed in the substrate since v2, both of them fixes the v2 run
itself exposed:
  * a plain-text overview now carries an outline (it was empty — a consumer
    with no structure to steer by can only guess);
  * a zero-match search now returns the total, a hint and next steps (it was
    a bare `[]` — a cliff, and consumers walked off it and hallucinated).
The harness bug those exposed is fixed too: it used to forward only the
`matches` array, discarding the metadata waggle actually returns.

v2 numbers are kept and reported. This is a re-run after a product fix, not
a re-grade — the grader and the corpus are untouched.
"""

from __future__ import annotations

import glob
import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, asdict

import rote_models as M

OUT = os.environ.get("TIER3_OUT3", "/tmp/tier3/out3")
MAX_TURNS = 10
MAX_TOK = 900

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
ARMS = ("copy", "reference", "waggle")
BINARY = ("pdf", "voice", "video")


@dataclass
class Run:
    art: str
    modality: str
    model: str
    arm: str
    correct: bool = False
    in_tokens: int = 0
    out_tokens: int = 0
    ops: int = 0
    turns: int = 0
    ingested: int = 0
    artifact_bytes: int = 0
    coverage_met: bool | None = None
    coverage_permille: int | None = None
    answered: str = ""
    error: str = ""
    transport_error: bool = False

    @property
    def dollars(self) -> float:
        pin, pout = PRICING[self.model]
        return (self.in_tokens * pin + self.out_tokens * pout) / 1_000_000


def wag(args: list[str]) -> dict:
    r = subprocess.run(["waggle", *args], capture_output=True, text=True, timeout=240)
    try:
        return json.loads(r.stdout).get("result") or {}
    except Exception:
        return {}


# ------------------------------------------------------------------ content
def folder_files(d: str) -> list[str]:
    return sorted(glob.glob(f"{d}/**/*.md", recursive=True))


def indexed_text(it: dict) -> str:
    if it["modality"] == "folder":
        return "\n\n".join(
            f"### FILE: {os.path.basename(p)}\n" + open(p, encoding="utf-8", errors="replace").read()
            for p in folder_files(it["path"])
        )
    p = it.get("content") or it["path"]
    return open(p, encoding="utf-8", errors="replace").read()


def contract_for(it: dict) -> list[str]:
    """The required region, in the richest lens the INDEXED CONTENT affords.

    The lens follows the content type waggle actually indexes, not the
    modality's nominal shape. A PDF's indexed content is the *extracted text*
    — plain text, which has no section lens — so a `section:` contract on it
    is unresolvable and mint rightly refuses. Only real markdown gets
    `section:`; only real source gets `symbol:`; everything else is addressed
    by line.
    """
    k = it["region_kind"]
    is_markdown = it["path"].endswith(".md")
    is_source = it["path"].endswith(".py")

    if k == "sections" and is_markdown:      # multi-hop: every fragment required
        out: list[str] = []
        for r in it["region"]:
            out += ["--require", f"section:{r}"]
        return out
    if k == "section" and is_markdown:
        return ["--require", f"section:{it['region']}"]
    if k == "symbol" and is_source:
        return ["--require", f"symbol:{it['region']}"]

    # Everything else (plain text, extracted PDF, transcripts) is line-addressed.
    txt = indexed_text(it)
    for n, line in enumerate(txt.splitlines(), 1):
        if it["answer"] in line:
            return ["--require", f"lines:{n}-{n}"]
    return []


def mint(it: dict) -> str:
    if it["modality"] == "folder":
        # A folder is minted as a TREE: one parent, every file a child.
        return wag(["mint", "--target", it["path"], "--tree"]).get("token", "")
    args = ["mint", "--target", it["path"], *contract_for(it)]
    if it.get("content"):
        args += ["--content", it["content"]]
        if it["modality"] in ("voice", "video"):
            args += ["--attach", it["path"]]
    else:
        args += ["--snapshot"]
    return wag(args).get("token", "")


# ------------------------------------------------------------------ prompts
def question(it: dict) -> str:
    if it["modality"] == "multihop":
        return (f"The recovery key is assembled from THREE fragments held in three "
                f"different runbooks, forming a chain. Start at the runbook named "
                f"'{it['entry']}'; each fragment names where the next one lives. "
                f"Follow the chain, collect all three fragments, and report the "
                f"complete key (format X9Y-1234).")
    return ("Find the AUDIT CODE stated inside this artifact and report it. "
            "The code looks like X9Y-1234.")


PROTO = """Reply with EXACTLY ONE JSON object, no prose, no fences.
When you have it: {"cmd":"answer","code":"X9Y-1234"}"""

TOOL_REF = """You have a path and ordinary file tools:
{"cmd":"open","path":"<p>"}                  -> the file's bytes as text
{"cmd":"grep","path":"<p>","pattern":"<re>"} -> matching lines (recurses a directory)
Binary files (pdf/audio/video) read as raw bytes; ordinary tools cannot
interpret them."""

TOOL_WAG = """The artifact is a waggle token. Interrogate it; you never receive the
whole artifact unless you ask. Every command may take an optional "token" to
address a CHILD (a folder's search returns each file's own token):
{"cmd":"overview"}                        -> size, content type, lenses, outline
{"cmd":"outline"}                         -> the structure alone
{"cmd":"section","name":"<heading>"}      -> that markdown section
{"cmd":"symbol","name":"<symbol>"}        -> that code symbol's source
{"cmd":"lines","range":"120-180"}         -> a line window
{"cmd":"search","pattern":"<regex>"}      -> matches (a folder searches the whole tree)
Pull only what you need."""


# ------------------------------------------------------------------ executors
def ref_exec(cmd: dict, it: dict) -> tuple[str, int]:
    c = cmd.get("cmd")
    p = cmd.get("path") or it["path"]
    is_bin = it["modality"] in BINARY
    if c == "grep":
        pat = cmd.get("pattern", "")
        if is_bin:
            return "(binary file; grep finds no text lines)", 0
        targets = folder_files(p) if os.path.isdir(p) else [p]
        hits = []
        for t in targets:
            try:
                for n, l in enumerate(open(t, encoding="utf-8", errors="replace"), 1):
                    if re.search(pat, l):
                        hits.append(f"{os.path.basename(t)}:{n}: {l.rstrip()}")
            except OSError:
                pass
        body = "\n".join(hits[:25]) or "(no matches)"
        return body, len(body)
    if c == "open":
        if os.path.isdir(p):
            body = "\n".join(os.path.basename(x) for x in folder_files(p))
            return f"(directory)\n{body}", len(body)
        if not os.path.isfile(p):
            return f"error: no such file {p}", 0
        raw = open(p, "rb").read()
        if is_bin:
            body = repr(raw[:800])
            return f"(binary, {len(raw)} bytes; not text)\n{body}", len(body)
        t = raw.decode("utf-8", "replace")
        return t, len(t)
    return f"error: unknown cmd {c!r}", 0


def wag_exec(cmd: dict, parent: str) -> tuple[str, int]:
    c = cmd.get("cmd")
    tok = cmd.get("token") or parent          # child addressing
    if c in ("overview", "outline"):
        r = wag(["read", "--token", tok])
        syms = (r.get("symbols") or {}).get("symbols", [])
        struct = ([f"{s['name']} ({s['kind']}) L{s['lines']}" for s in syms][:60]
                  or [f"{o.get('heading')} L{o.get('line')}" for o in (r.get("outline") or [])][:60])
        body = struct if c == "outline" else {
            "content_type": r.get("content_type"), "lines": r.get("total_lines"),
            "bytes": r.get("total_bytes"), "lenses": r.get("lenses"), "outline": struct,
        }
        out = json.dumps(body)
        return out, len(out)
    if c == "section":
        r = wag(["read", "--token", tok, "--section", cmd.get("name", ""), "--max-bytes", "3000"])
    elif c == "symbol":
        r = wag(["read", "--token", tok, "--symbol", cmd.get("name", ""), "--max-bytes", "3000"])
    elif c == "lines":
        r = wag(["read", "--token", tok, "--lines", cmd.get("range", ""), "--max-bytes", "3000"])
    elif c == "search":
        r = wag(["search", "--token", tok, "--pattern", cmd.get("pattern", ""),
                 "--max-matches", "8", "--max-bytes", "3500"])
        # Forward the WHOLE reply — matches, totals, hint, next, and (for a
        # tree) the per-file child tokens. v2 forwarded only `matches`.
        out = json.dumps({k: r.get(k) for k in
                          ("matches", "files", "total_matches", "truncated", "hint", "next")
                          if r.get(k) is not None})
        return out, len(out)
    else:
        return f"error: unknown cmd {c!r}", 0
    out = json.dumps({"lines": r.get("lines"), "text": r.get("text", "")})
    return out, len(out)


def norm(s: str) -> str:
    """Grading normal form: upper-case, punctuation stripped.

    A consumer that reports `Z2M4318` has retrieved `Z2M-4318`; marking that
    wrong would be grading punctuation, not retrieval. Applied identically to
    every arm — this is the one grader, and it is frozen for all three.
    """
    return re.sub(r"[^A-Z0-9]", "", (s or "").upper())


def graded(expected: str, answered: str) -> bool:
    return norm(expected) in norm(answered)


def parse(text: str) -> dict | None:
    t = re.sub(r"```(?:json)?", "", text).strip()
    i = t.find("{")
    if i < 0:
        return None
    try:
        return json.JSONDecoder().raw_decode(t[i:])[0]
    except Exception:
        return None


# ------------------------------------------------------------------ the run
def run_one(it: dict, model: str, arm: str) -> Run:
    r = Run(it["id"], it["modality"], model, arm, artifact_bytes=it["bytes"])
    try:
        tools = {"copy": "", "reference": TOOL_REF, "waggle": TOOL_WAG}[arm]
        system = f"You retrieve facts from artifacts.\n{PROTO}\n\n{tools}"
        tok = ""
        q = question(it)
        if arm == "copy":
            body = indexed_text(it)
            r.ingested = len(body)
            convo = f"{q}\n\n## Artifact ({it['modality']})\n{body}"
        elif arm == "reference":
            convo = f"{q}\n\nThe artifact is at path: {it['path']}\n(modality: {it['modality']})"
        else:
            tok = mint(it)
            if not tok:
                r.error = "mint failed"
                return r
            convo = f"{q}\n\nThe artifact is waggle token: {tok}\n(modality: {it['modality']})"

        for turn in range(MAX_TURNS):
            rep = M.call(model, system, convo, max_tokens=MAX_TOK)
            r.in_tokens += rep.in_tokens
            r.out_tokens += rep.out_tokens
            r.turns = turn + 1
            obj = parse(rep.text)
            if not obj:
                convo += "\n\nReply with ONE JSON object only."
                continue
            if obj.get("cmd") == "answer":
                r.answered = str(obj.get("code", ""))
                r.correct = graded(it["answer"], r.answered)
                break
            r.ops += 1
            if arm == "copy":
                convo += "\n\nThe artifact is already above. Answer now."
                continue
            out, n = (ref_exec(obj, it) if arm == "reference" else wag_exec(obj, tok))
            r.ingested += n
            convo += f"\n\n> {json.dumps(obj)}\n{out[:7000]}"
        if not r.answered and not r.error:
            r.error = "no answer"

        # A --tree parent carries no contract of its own, so coverage is not
        # applicable to the folder shape; we report it as such rather than as a
        # failure.
        if arm == "waggle" and tok and it["modality"] != "folder":
            cov = wag(["coverage", "--token", tok])
            if cov:
                r.coverage_met = bool(cov.get("met"))
                r.coverage_permille = cov.get("permille")
    except RuntimeError as e:
        r.transport_error = True
        r.error = str(e)[:140]
    except Exception as e:
        r.error = f"{type(e).__name__}: {e}"[:140]
    return r


def main() -> int:
    items = []
    for c in (os.environ.get("TIER3_CORPUS", "/tmp/tier3/corpus"),
              os.environ.get("TIER3_CORPUS2", "/tmp/tier3/corpus2")):
        f = f"{c}/manifest.json"
        if os.path.isfile(f):
            items += json.load(open(f))
    models = os.environ.get("TIER3_MODELS", ",".join(PRICING)).split(",")
    arms = os.environ.get("TIER3_ARMS", ",".join(ARMS)).split(",")
    os.makedirs(OUT, exist_ok=True)

    jobs = [(it, m, a) for it in items for m in models for a in arms]
    print(f"runs: {len(jobs)} ({len(items)} artifacts x {len(models)} models x {len(arms)} arms)",
          flush=True)

    runs: list[Run] = []
    with ThreadPoolExecutor(max_workers=4) as ex:
        futs = [ex.submit(run_one, it, m, a) for (it, m, a) in jobs]
        for n, f in enumerate(futs, 1):
            runs.append(f.result())
            if n % 20 == 0 or n == len(futs):
                ok = sum(1 for x in runs if x.correct)
                print(f"[{n}/{len(futs)}] correct {ok}", flush=True)
                json.dump([{**asdict(x), "dollars": x.dollars} for x in runs],
                          open(f"{OUT}/runs3.json", "w"), indent=1)

    json.dump([{**asdict(x), "dollars": x.dollars} for x in runs],
              open(f"{OUT}/runs3.json", "w"), indent=1)
    ex_n = sum(1 for x in runs if x.transport_error)
    print(f"wrote {OUT}/runs3.json (excluded transport errors: {ex_n})", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
