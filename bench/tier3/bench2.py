"""Tier-3 v2 — the substrate across six modalities (design doc 22 §4).

Three turn-matched arms answer the SAME question about the SAME artifact.
Only the handoff differs:

  copy       the artifact's text, pasted into the prompt (today's default)
  reference  a path + ordinary file tools (open / grep) — the honest
             competitor the paper names. A path to an MP4 hands a text-only
             consumer nothing; that is the point, not a bug in the arm.
  waggle     a ~30-byte token + the substrate's verbs: resolve to an
             overview, then lens (section / symbol / lines) and search.
             Minted under a consumption contract, so coverage is measured.

Graded per run: correctness (the audit code, exact match), tokens, ops,
bytes that entered the window, and — waggle only — whether the receipts show
the required region was actually consumed. The absence of that column in the
other two arms is itself the finding.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, asdict

import rote_models as M

CORPUS = os.environ.get("TIER3_CORPUS", "/tmp/tier3/corpus")
OUT = os.environ.get("TIER3_OUT2", "/tmp/tier3/out2")
MAX_TURNS = 8
MAX_TOK = 800

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
    ingested: int = 0          # chars of artifact content that entered the window
    artifact_bytes: int = 0
    coverage_met: bool | None = None   # waggle only
    coverage_permille: int | None = None
    answered: str = ""
    error: str = ""
    transport_error: bool = False      # excluded from the analysis (doc 22 §4.5)

    @property
    def dollars(self) -> float:
        pin, pout = PRICING[self.model]
        return (self.in_tokens * pin + self.out_tokens * pout) / 1_000_000


def wag(args: list[str]) -> dict:
    r = subprocess.run(["waggle", *args], capture_output=True, text=True, timeout=180)
    try:
        return json.loads(r.stdout).get("result") or {}
    except Exception:
        return {}


def indexed_text(it: dict) -> str:
    """The text waggle indexes: the file itself, or the extracted content."""
    p = it.get("content") or it["path"]
    return open(p, encoding="utf-8", errors="replace").read()


def contract_for(it: dict) -> list[str]:
    """The required region, in the richest lens the modality affords."""
    if it["modality"] == "markdown":
        return ["--require", f"section:{it['region']}"]
    if it["modality"] == "code":
        return ["--require", f"symbol:{it['region']}"]
    txt = indexed_text(it)
    for n, line in enumerate(txt.splitlines(), 1):
        if it["answer"] in line:
            return ["--require", f"lines:{n}-{n}"]
    return []


def mint(it: dict) -> str:
    args = ["mint", "--target", it["path"], *contract_for(it)]
    if it.get("content"):
        args += ["--content", it["content"]]
        if it["modality"] in ("voice", "video"):
            args += ["--attach", it["path"]]
    else:
        args += ["--snapshot"]
    return wag(args).get("token", "")


QUESTION = (
    "Find the AUDIT CODE stated inside this artifact and report it. "
    "The code looks like X9Y-1234."
)
PROTO = """Reply with EXACTLY ONE JSON object, no prose, no fences.
When you have it: {"cmd":"answer","code":"X9Y-1234"}"""

TOOLS = {
    "copy": "",
    "reference": """You have a file path and ordinary file tools:
{"cmd":"open","path":"<p>"}                  -> the file's bytes as text
{"cmd":"grep","path":"<p>","pattern":"<re>"} -> matching lines
Binary files (pdf/audio/video) read as raw bytes; ordinary tools cannot
interpret them.""",
    "waggle": """The artifact is a waggle token. Interrogate it; you never receive the
whole artifact unless you ask:
{"cmd":"overview"}                       -> size, content type, lenses, outline
{"cmd":"section","name":"<heading>"}     -> that markdown section
{"cmd":"symbol","name":"<symbol>"}       -> that code symbol's source
{"cmd":"lines","range":"120-180"}        -> a line window
{"cmd":"search","pattern":"<regex>"}     -> matches with line numbers
Pull only what you need.""",
}


def ref_exec(cmd: dict, it: dict) -> tuple[str, int]:
    c, p = cmd.get("cmd"), cmd.get("path") or it["path"]
    if not os.path.isfile(p):
        return f"error: no such file {p}", 0
    raw = open(p, "rb").read()
    is_bin = it["modality"] in ("pdf", "voice", "video")
    if c == "open":
        if is_bin:
            # The honest truth of a raw path to media: unreadable bytes.
            body = repr(raw[:800])
            return f"(binary, {len(raw)} bytes; not text)\n{body}", len(body)
        t = raw.decode("utf-8", "replace")
        return t, len(t)
    if c == "grep":
        if is_bin:
            return "(binary file; grep finds no text lines)", 0
        t = raw.decode("utf-8", "replace")
        hits = [f"{n}: {l}" for n, l in enumerate(t.splitlines(), 1)
                if re.search(cmd.get("pattern", ""), l)][:20]
        body = "\n".join(hits) or "(no matches)"
        return body, len(body)
    return f"error: unknown cmd {c!r}", 0


def wag_exec(cmd: dict, tok: str) -> tuple[str, int]:
    c = cmd.get("cmd")
    if c == "outline":
        r = wag(["read", "--token", tok])
        out = json.dumps((r.get("outline") or (r.get("symbols") or {}).get("symbols") or []))
        return out, len(out)
    if c == "overview":
        r = wag(["read", "--token", tok])
        syms = (r.get("symbols") or {}).get("symbols", [])
        out = json.dumps({
            "content_type": r.get("content_type"), "lines": r.get("total_lines"),
            "bytes": r.get("total_bytes"), "lenses": r.get("lenses"),
            "outline": ([f"{s['name']} ({s['kind']}) L{s['lines']}" for s in syms][:60]
                        or [f"{o.get('heading')} L{o.get('line')}" for o in (r.get("outline") or [])][:60]),
        })
        return out, len(out)
    if c == "section":
        r = wag(["read", "--token", tok, "--section", cmd.get("name", ""), "--max-bytes", "3000"])
    elif c == "symbol":
        r = wag(["read", "--token", tok, "--symbol", cmd.get("name", ""), "--max-bytes", "3000"])
    elif c == "lines":
        r = wag(["read", "--token", tok, "--lines", cmd.get("range", ""), "--max-bytes", "3000"])
    elif c == "search":
        r = wag(["search", "--token", tok, "--pattern", cmd.get("pattern", ""),
                 "--max-matches", "8", "--max-bytes", "3000"])
        # Forward the WHOLE search reply. The earlier harness forwarded only
        # `matches`, so a zero-match search reached the model as a bare `[]` —
        # discarding the total, the hint and the next-steps waggle actually
        # returns. That was a harness defect that under-represented the
        # substrate, and it is the arm's own metadata, not a hint we invented.
        out = json.dumps({k: r.get(k) for k in
                          ("matches", "total_matches", "truncated", "hint", "next")
                          if r.get(k) is not None})
        return out, len(out)
    else:
        return f"error: unknown cmd {c!r}", 0
    out = json.dumps({"lines": r.get("lines"), "text": r.get("text", "")})
    return out, len(out)


def parse(text: str) -> dict | None:
    t = re.sub(r"```(?:json)?", "", text).strip()
    i = t.find("{")
    if i < 0:
        return None
    try:
        return json.JSONDecoder().raw_decode(t[i:])[0]
    except Exception:
        return None


def run_one(it: dict, model: str, arm: str) -> Run:
    r = Run(it["id"], it["modality"], model, arm, artifact_bytes=it["bytes"])
    try:
        system = f"You retrieve facts from artifacts.\n{PROTO}\n\n{TOOLS[arm]}"
        tok = ""
        if arm == "copy":
            body = indexed_text(it)
            r.ingested = len(body)
            convo = f"{QUESTION}\n\n## Artifact ({it['modality']})\n{body}"
        elif arm == "reference":
            convo = f"{QUESTION}\n\nThe artifact is at path: {it['path']}\n(modality: {it['modality']})"
        else:
            tok = mint(it)
            if not tok:
                r.error = "mint failed"
                return r
            convo = (f"{QUESTION}\n\nThe artifact is waggle token: {tok}\n"
                     f"(modality: {it['modality']})")

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
                r.correct = it["answer"] in r.answered
                break
            r.ops += 1
            if arm == "copy":
                convo += "\n\nThe artifact is already above. Answer now."
                continue
            out, n = (ref_exec(obj, it) if arm == "reference" else wag_exec(obj, tok))
            r.ingested += n
            convo += f"\n\n> {json.dumps(obj)}\n{out[:6000]}"
        if not r.answered and not r.error:
            r.error = "no answer"

        if arm == "waggle" and tok:
            cov = wag(["coverage", "--token", tok])
            if cov:
                r.coverage_met = bool(cov.get("met"))
                r.coverage_permille = cov.get("permille")
    except RuntimeError as e:  # transport gave up after retries -> excluded
        r.transport_error = True
        r.error = str(e)[:140]
    except Exception as e:
        r.error = f"{type(e).__name__}: {e}"[:140]
    return r


def main() -> int:
    items = json.load(open(f"{CORPUS}/manifest.json"))
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
            r = f.result()
            runs.append(r)
            if n % 10 == 0 or n == len(futs):
                ok = sum(1 for x in runs if x.correct)
                print(f"[{n}/{len(futs)}] correct so far {ok}", flush=True)
            json.dump([asdict(x) for x in runs], open(f"{OUT}/runs2.json", "w"), indent=1)

    rows = [{**asdict(x), "dollars": x.dollars} for x in runs]
    json.dump(rows, open(f"{OUT}/runs2.json", "w"), indent=1)
    ex_n = sum(1 for x in runs if x.transport_error)
    print(f"wrote {OUT}/runs2.json (excluded transport errors: {ex_n})", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
