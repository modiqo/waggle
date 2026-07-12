"""Tier-3 v4 — closing the gaps the v3 run exposed (doc 22 §4).

v3 left five honest weaknesses. Four are addressed here; each fix is named so
the reader can discount it.

1. ACCURACY. v3's failures were not search failures: the models answered with
   `X9Y-1234` — the placeholder from our own question. They gave up and
   parroted the example. Two changes: the prompt no longer contains a literal
   fake code (all arms), and a fourth arm, `waggle+gate`, refuses an answer the
   receipt does not back. The gate reads ONLY `coverage.met` — never the
   answer, never the region's name. It says "your receipt shows you have not
   consumed what the author requires; keep looking." No other arm can do this,
   which is the point: it is the receipt turned from a diagnostic into a
   control.

2. TURNS. Three hops cost three round-trips, and the conversation is re-sent
   each time. Both interrogating arms may now BATCH ops into one turn, which is
   what a real agent does with parallel tool calls. Given to `reference` too.

3. GREP'S ECONOMY. waggle's search returned +/-2 context lines where grep
   returns one. The agent can now set `context`, so it can be as terse as grep.

4. REASONING. The corpus was all needles. `reasoning` adds a judgement: each
   runbook declares a retry budget, a policy declares a ceiling, and the
   question asks which runbook VIOLATES it. No line contains that answer — it
   exists only in the relation between regions.

Not fixed, and stated plainly: pasting is still the ceiling on artifacts a
window can hold, and a local grep on a small text file is still cheaper than a
token.
"""

from __future__ import annotations

import glob
import json
import os
import re
import signal
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, asdict

import rote_models as M

OUT = os.environ.get("TIER3_OUT4", "/tmp/tier3/out4")
MAX_TURNS = 10
MAX_TOK = 900
ARMS = ("copy", "reference", "waggle", "waggle+gate")

# The binary UNDER TEST, pinned. Never `waggle` off PATH: a `cargo build` during
# a sweep would swap the substrate mid-experiment and blend two builds into one
# result set, with no record of which run got which. Freeze it, hash it, report it.
WAGGLE_BIN = os.environ.get("WAGGLE_BIN", "waggle")


def assert_clean_substrate() -> None:
    """No foreign daemon, and say which binary is under test.

    A `waggle serve --daemon` left over from `cargo test` is built from a
    DIFFERENT source tree and holds the same store open. Plain verbs dispatch
    in-process so it cannot answer for us, but it contends on the store, and a
    measurement that shares its substrate with an unknown build is not a
    measurement. Fail loudly rather than produce a number we would have to
    caveat later.
    """
    ps = subprocess.run(["ps", "ax", "-o", "command"], capture_output=True, text=True).stdout
    stray = [l for l in ps.splitlines() if "waggle serve --daemon" in l]
    if stray:
        raise SystemExit(
            "REFUSING TO RUN: a waggle daemon is alive and shares the store:\n  "
            + "\n  ".join(stray[:4])
            + "\n\nKill it (pkill -f 'waggle serve --daemon') and re-run.")
    h = subprocess.run(["shasum", "-a", "256", WAGGLE_BIN], capture_output=True, text=True)
    print(f"binary under test: {WAGGLE_BIN}\n  sha256 {h.stdout.split()[0][:16] if h.stdout else '?'}",
          flush=True)
BINARY = ("pdf", "voice", "video")
TREE = ("folder", "reasoning", "bigtree_find", "bigtree_count")

# `files:all` means the consumer was SERVED every file. That is the honest
# contract for `reasoning` — you cannot know which runbook breaks the ceiling
# without reading them all. It is the WRONG contract for a question a filter can
# answer: the right way to count the files mentioning X is to search, which
# serves you only the ones that matched. Demanding files:all there would force
# the agent to ingest all 180 files — turning waggle into copy. So the bigtree
# shapes carry no contract, and the gate is inert on them BY DESIGN.
CONTRACT_ALL = ("reasoning",)

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
    gate_rejections: int = 0
    answered: str = ""
    error: str = ""
    transport_error: bool = False

    @property
    def dollars(self) -> float:
        pin, pout = PRICING[self.model]
        return (self.in_tokens * pin + self.out_tokens * pout) / 1_000_000


def wag(args: list[str]) -> dict:
    """Run a waggle op and return its result — WITH the `next` hints.

    The harness used to drop `next`. That is waggle's guidance-at-point-of-use
    mechanism, the way the substrate tells a consumer what it can do here, and
    throwing it away under-represented the system: we watched an agent fan a
    lens out by hand across ten child tokens because nothing told it the parent
    could do it in one call.
    """
    p = subprocess.run([WAGGLE_BIN, *args], capture_output=True, text=True, timeout=240)
    try:
        env = json.loads(p.stdout)
        res = env.get("result") or {}
        if isinstance(res, dict) and env.get("next"):
            res = {**res, "next": env["next"]}
        return res
    except Exception:
        return {}


def files_of(d: str) -> list[str]:
    return sorted(glob.glob(f"{d}/**/*.md", recursive=True))


def indexed_text(it: dict) -> str:
    if it["modality"] in TREE:
        return "\n\n".join(
            f"### FILE: {os.path.basename(p)}\n" + open(p, encoding="utf-8", errors="replace").read()
            for p in files_of(it["path"]))
    p = it.get("content") or it["path"]
    return open(p, encoding="utf-8", errors="replace").read()


def contract_for(it: dict) -> list[str]:
    k, path = it["region_kind"], it["path"]
    if k == "sections" and path.endswith(".md"):
        out: list[str] = []
        for r in it["region"]:
            out += ["--require", f"section:{r}"]
        return out
    if k == "section" and path.endswith(".md"):
        return ["--require", f"section:{it['region']}"]
    if k == "symbol" and path.endswith(".py"):
        return ["--require", f"symbol:{it['region']}"]
    txt = indexed_text(it)
    for n, line in enumerate(txt.splitlines(), 1):
        if str(it["answer"]) in line:
            return ["--require", f"lines:{n}-{n}"]
    return []


def mint(it: dict) -> str:
    if it["modality"] in TREE:
        a = ["mint", "--target", it["path"], "--tree"]
        if it["modality"] in CONTRACT_ALL:
            # This delegation genuinely needs every file: the answer is the
            # relation between the policy and ALL the runbooks. The author says
            # so at mint; the receipt then refuses to call it met while any file
            # is unread. Not circular — "read everything" is a completeness
            # requirement, not the answer.
            a += ["--require", "files:all"]
        return wag(a).get("token", "")
    args = ["mint", "--target", it["path"], *contract_for(it)]
    if it.get("content"):
        args += ["--content", it["content"]]
        if it["modality"] in ("voice", "video"):
            args += ["--attach", it["path"]]
    else:
        args += ["--snapshot"]
    return wag(args).get("token", "")


# --------------------------------------------------------------- questions
def question(it: dict) -> str:
    m = it["modality"]
    if m == "reasoning":
        return ("An escalation policy sets a CEILING on the retry budget any runbook "
                "may declare. Exactly one runbook VIOLATES it. Read the policy, read "
                "the runbooks, and report which runbook violates the ceiling. Answer "
                "with its number, e.g. {\"cmd\":\"answer\",\"code\":\"runbook_07\"}.")
    if m == "bigtree_find":
        return ("Somewhere in this large tree of files, exactly ONE file states a "
                "teardown reconciliation code, of the form three alphanumeric "
                "characters, a hyphen, four digits. Find it and report it exactly. "
                "The tree is far too large to read in full — narrow it.")
    if m == "bigtree_count":
        return ("Some files in this tree still call the DEPRECATED `legacy_reconcile()` "
                "entry point. Report HOW MANY files mention it — a single integer, e.g. "
                "{\"cmd\":\"answer\",\"code\":\"4\"}. The tree is far too large to "
                "read in full; you must narrow it and count what you find.")
    if m == "multihop":
        return (f"The recovery key is assembled from THREE fragments held in three "
                f"different runbooks, forming a chain. Start at the runbook named "
                f"'{it['entry']}'; each fragment names where the next one lives. "
                f"Follow the chain, collect all three, and report the complete key.")
    # No literal placeholder: v3's failures were models parroting the example.
    return ("Find the AUDIT CODE stated inside this artifact and report it. It has "
            "the form: three alphanumeric characters, a hyphen, then four digits. "
            "Report the code exactly as it appears; never invent one.")


PROTO = """Reply with EXACTLY ONE JSON object, no prose, no fences.
Answer only when you have SEEN the value: {"cmd":"answer","code":"<verbatim>"}
You may batch independent lookups into one turn:
{"cmd":"batch","ops":[{...},{...}]}"""

TOOL_REF = """You have a path and ordinary file tools:
{"cmd":"ls","path":"<p>"}                               -> list a directory (recursive)
{"cmd":"open","path":"<p>"}                             -> the file's bytes as text
{"cmd":"grep","path":"<p>","pattern":"<re>"}            -> matching lines (recurses a directory)
Binary files (pdf/audio/video) read as raw bytes; ordinary tools cannot
interpret them."""

TOOL_WAG = """The artifact is a waggle token. Interrogate it; you never receive the
whole artifact unless you ask. Any command may carry "token" to address a CHILD
(a folder's search returns each file's own token):
{"cmd":"overview"}                                  -> size, type, lenses, outline
{"cmd":"section","name":"<heading>"}                -> that markdown section
{"cmd":"symbol","name":"<symbol>"}                  -> that code symbol's source
{"cmd":"lines","range":"120-180"}                   -> a line window
{"cmd":"search","pattern":"<re>","context":0}       -> matches; context 0 is terse/cheap
If the token is a FOLDER: {"cmd":"overview"} lists every file (with its own
token and outline), and a lens applied to the FOLDER token answers for EVERY
file at once, e.g. {"cmd":"section","name":"Retry Policy"} returns that section
from all of them in ONE call. If it comes back complete=false, you have NOT seen
the whole folder — continue with {"cmd":"section","name":...,"from":<the cursor>}
before you conclude anything.
Pull only what you need."""


def ref_exec(cmd: dict, it: dict) -> tuple[str, int]:
    c = cmd.get("cmd")
    p = cmd.get("path") or it["path"]
    is_bin = it["modality"] in BINARY
    if c == "ls":
        names = [f[len(it["path"]):].lstrip("/") for f in files_of(p)] if os.path.isdir(p) else [p]
        body = "\n".join(names[:300]) or "(not a directory)"
        return body, len(body)
    if c == "grep":
        if is_bin:
            return "(binary file; grep finds no text lines)", 0
        targets = files_of(p) if os.path.isdir(p) else [p]
        hits = []
        for t in targets:
            try:
                for n, l in enumerate(open(t, encoding="utf-8", errors="replace"), 1):
                    if re.search(cmd.get("pattern", ""), l):
                        hits.append(f"{os.path.basename(t)}:{n}: {l.rstrip()}")
            except OSError:
                pass
        body = "\n".join(hits[:30]) or "(no matches)"
        return body, len(body)
    if c == "open":
        if os.path.isdir(p):
            body = "\n".join(os.path.basename(x) for x in files_of(p))
            return f"(directory)\n{body}", len(body)
        if not os.path.isfile(p):
            return f"error: no such file {p}", 0
        raw = open(p, "rb").read()
        if is_bin:
            b = repr(raw[:800])
            return f"(binary, {len(raw)} bytes; not text)\n{b}", len(b)
        t = raw.decode("utf-8", "replace")
        return t, len(t)
    return f"error: unknown cmd {c!r}", 0


def as_int(v, default: int) -> int:
    """Model-supplied values must never crash the harness.

    An agent that echoes a placeholder (`"from": "<the cursor>"`) or sends a
    string where a number belongs should get a sane default, not a dead run.
    We lost runs to `int("<the cursor>")` — the harness being brittle about
    the model's input, which is the harness's fault, never the model's.
    """
    try:
        return int(v)
    except (TypeError, ValueError):
        return default


def wag_exec(cmd: dict, parent: str) -> tuple[str, int]:
    c = cmd.get("cmd")
    tok = cmd.get("token") or parent
    if c in ("overview", "outline"):
        r = wag(["read", "--token", tok])
        if r.get("kind") == "tree":
            # The directory projection: the folder's table of contents, with
            # each child's own token so the consumer can address it.
            out = json.dumps({"kind": "tree", "files": r.get("files"),
                              "total_bytes": r.get("total_bytes"),
                              "children": r.get("children", []),
                              "next": r.get("next", [])})
            return out, len(out)
        syms = (r.get("symbols") or {}).get("symbols", [])
        struct = ([f"{s['name']} ({s['kind']}) L{s['lines']}" for s in syms][:60]
                  or [f"{o.get('heading')} L{o.get('line')}" for o in (r.get("outline") or [])][:60])
        out = json.dumps(struct if c == "outline" else {
            "content_type": r.get("content_type"), "lines": r.get("total_lines"),
            "bytes": r.get("total_bytes"), "lenses": r.get("lenses"),
            "outline": struct, "next": r.get("next")})
        return out, len(out)
    if c == "section":
        a = ["read", "--token", tok, "--section", cmd.get("name", ""), "--max-bytes", "8000"]
        if cmd.get("from") is not None:
            frm = as_int(cmd["from"], -1)
            if frm >= 0:
                a += ["--from", str(frm)]
        r = wag(a)
    elif c == "symbol":
        r = wag(["read", "--token", tok, "--symbol", cmd.get("name", ""), "--max-bytes", "3000"])
    elif c == "lines":
        r = wag(["read", "--token", tok, "--lines", cmd.get("range", ""), "--max-bytes", "3000"])
    elif c == "search":
        ctx = str(max(0, as_int(cmd.get("context", 1), 1)))
        r = wag(["search", "--token", tok, "--pattern", cmd.get("pattern", ""),
                 "--context", ctx, "--max-matches", "10", "--max-bytes", "3500"])
        out = json.dumps({k: r.get(k) for k in
                          ("matches", "files", "total_matches", "truncated", "hint", "next")
                          if r.get(k) is not None})
        return out, len(out)
    else:
        return f"error: unknown cmd {c!r}", 0
    if r.get("kind") == "tree-lens":
        out = json.dumps({k: r.get(k) for k in
                          ("kind", "lens", "of", "total_files", "examined", "matched",
                           "skipped", "complete", "truncated", "files", "next")
                          if r.get(k) is not None})
        return out, len(out)
    out = json.dumps({k: v for k, v in
                      {"lines": r.get("lines"), "text": r.get("text", ""),
                       "next": r.get("next")}.items() if v})
    return out, len(out)


def receipt_gap(it: dict, tok: str) -> list[str]:
    """What the receipt says the consumer has NOT been served.

    A refusal without a path is a dead end, and we had made that mistake three
    times when this was written — and then made it a fourth, right here: this
    read only `unread`, which a TREE reports. A region-contract token reports
    `missed`, with the author's label AND the line range AND a ready-made `read`
    call in `next`. On multihop the gate therefore said "not supported" and
    handed back nothing at all, and we watched gpt-4.1 burn all ten turns
    against a locked door. waggle emitted the guidance; the harness dropped it.

    Naming a missed region is not leaking the answer. The label is the AUTHOR's
    declared demand — that is what a completeness contract IS. The answer is the
    content inside the region, which the gate never reads.
    """
    cov = wag(["coverage", "--token", tok])
    if not cov:
        return []
    # tree: whole files were never served
    gap = [str(u.get("target", "")).rsplit("/", 1)[-1] for u in (cov.get("unread") or [])]
    # region contract: named regions were never served — say which, and where
    gap += [f"section '{m.get('label')}' (lines {m.get('lines')})" if m.get("label")
            else f"lines {m.get('lines')}"
            for m in (cov.get("missed") or [])]
    return gap


def receipt_backs_it(it: dict, tok: str) -> bool | None:
    """Does the consumer's own receipt support a claim of completion?

    Reads ONLY the receipt — never the answer, never the region's name.

    Two shapes, because waggle reports two. A contract-bearing token answers
    with `met`. A --tree parent answers with `read: "k/n"` and the `unread`
    children: on a tree the receipt attests that content was *served*, which is
    a weaker instrument than a region contract, but it still refuses a consumer
    that opened nothing — and for a governance prerequisite (you must be served
    the escalation policy before your judgement is accepted) it is exactly the
    right one. Returns None when there is nothing to gate on.
    """
    cov = wag(["coverage", "--token", tok])
    if not cov:
        return None
    if "met" in cov:            # region contract, or a tree with files:all
        return bool(cov["met"])
    if "read" in cov:                                  # tree, no contract
        unread = {str(u.get("target", "")) for u in (cov.get("unread") or [])}
        if it["modality"] in CONTRACT_ALL:
            # Governance: the policy is a PREREQUISITE, not the answer.
            return not any("escalation_policy" in u for u in unread)
        served = int(str(cov["read"]).split("/")[0] or 0)
        return served > 0                              # you answered having opened nothing
    return None


def norm(s: str) -> str:
    return re.sub(r"[^A-Z0-9]", "", (s or "").upper())


def graded(it: dict, answered: str) -> bool:
    if it["modality"] == "bigtree_count":
        nums = [int(x) for x in re.findall(r"\d+", answered or "")]
        return len(nums) == 1 and nums[0] == int(it["answer"])
    if it["modality"] == "reasoning":
        # "runbook_07", "Runbook 7", "7" all name the same runbook. Compare the
        # NUMBER on both sides — the expected answer is itself "runbook_NN".
        want = re.search(r"\d+", str(it["answer"]))
        if not want:
            return False
        nums = [int(x) for x in re.findall(r"\d+", answered or "")]
        return int(want.group()) in nums
    return norm(str(it["answer"])) in norm(answered)


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
    gated = arm == "waggle+gate"
    is_wag = arm.startswith("waggle")
    try:
        tools = {"copy": "", "reference": TOOL_REF}.get(arm, TOOL_WAG)
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


        for _turn in range(MAX_TURNS):
            rep = M.call(model, system, convo, max_tokens=MAX_TOK)
            r.in_tokens += rep.in_tokens
            r.out_tokens += rep.out_tokens
            r.turns += 1
            obj = parse(rep.text)
            if not obj:
                convo += "\n\nReply with ONE JSON object only."
                continue

            if obj.get("cmd") == "answer":
                ans = str(obj.get("code", ""))
                if gated:
                    # THE GATE: refuse a claim the consumer's own trail does not
                    # support. Reads only the receipt — never the answer.
                    backed = receipt_backs_it(it, tok)
                    if backed is False:
                        r.gate_rejections += 1
                        gap = receipt_gap(it, tok)
                        where = (" You have NOT been served these files: "
                                 + ", ".join(gap[:8]) + ". Read them.") if gap else ""
                        convo += ("\n\nREJECTED: your receipt shows you have not consumed what "
                                  "the author requires, so this answer is not yet supported by "
                                  "your own trail." + where +
                                  " Do not answer again until you have SEEN what you need.")
                        continue
                r.answered = ans
                r.correct = graded(it, ans)
                break

            # A reply that is valid JSON but not a known command must be met with
            # the schema, not with "unknown cmd". gpt-4o-mini found the value on
            # turn 0 and reported it as {"AUDIT_CODE": "..."} — the right answer
            # in the wrong shape — and the old error taught it nothing, so it
            # repeated itself to the turn cap. A dead end again; a fork now.
            known = {"copy": set(), "reference": {"open", "grep", "ls"}}.get(
                arm, {"overview", "outline", "section", "symbol", "lines", "search"})
            if obj.get("cmd") not in known | {"answer", "batch"}:
                convo += ('\n\nThat is not a valid command. To report the value you have '
                          'found, reply EXACTLY:\n{"cmd":"answer","code":"<the value>"}\n'
                          'To keep looking, use one of the listed commands.')
                continue

            ops = obj["ops"] if obj.get("cmd") == "batch" and isinstance(obj.get("ops"), list) else [obj]
            chunks = []
            for o in ops[:6]:
                r.ops += 1
                out, n = (ref_exec(o, it) if arm == "reference" else wag_exec(o, tok))
                r.ingested += n
                chunks.append(f"> {json.dumps(o)}\n{out[:4500]}")
            if arm == "copy":
                convo += "\n\nThe artifact is already above. Answer now."
            else:
                convo += "\n\n" + "\n\n".join(chunks)

        if not r.answered and not r.error:
            r.error = "no answer"
        if is_wag and tok:
            r.coverage_met = receipt_backs_it(it, tok)
    except RuntimeError as e:
        msg = str(e)
        overflow = any(k in msg.lower() for k in
                       ("context_length", "context length", "too long", "maximum context",
                        "prompt is too long", "max_tokens_to_sample", "string too long",
                        "invalid_request_error"))
        # Copy cannot fit a 450 KB tree. That is a real, attributable defeat for
        # the strategy — record it as a wrong answer, not as a transport error to
        # be excluded. Excluding it would hide precisely what we set out to show.
        r.transport_error = not overflow
        r.error = ("context_overflow: " if overflow else "") + msg[:120]
    except Exception as e:
        r.error = f"{type(e).__name__}: {e}"[:140]
    return r


def _reap_children() -> None:
    """Take our subprocesses down with us.

    A killed sweep used to leave its in-flight `rote` calls running — dozens of
    them, still spending tokens, still holding API concurrency, and starving the
    next sweep to the point where it looked like waggle had hung. Own the
    cleanup: put every child in our process group and signal the group.
    """
    try:
        os.killpg(os.getpgid(0), signal.SIGTERM)
    except Exception:
        pass


def main() -> int:
    assert_clean_substrate()
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda *_: (_reap_children(), sys.exit(130)))
    items = []
    for c in (os.environ.get("TIER3_CORPUS", "/tmp/tier3/corpus"),
              os.environ.get("TIER3_CORPUS2", "/tmp/tier3/corpus2"),
              os.environ.get("TIER3_CORPUS3", "/tmp/tier3/corpus3")):
        f = f"{c}/manifest.json"
        if os.path.isfile(f):
            items += json.load(open(f))
    models = os.environ.get("TIER3_MODELS", ",".join(PRICING)).split(",")
    arms = os.environ.get("TIER3_ARMS", ",".join(ARMS)).split(",")
    shapes = os.environ.get("TIER3_SHAPES", "")
    if shapes:
        keep = set(shapes.split(","))
        items = [it for it in items if it["modality"] in keep]
    # A thin slice across EVERY shape catches harness faults for a fraction of a
    # sweep. Full sweeps are for measuring, not for finding bugs.
    per = int(os.environ.get("TIER3_PER_SHAPE", "0"))
    if per:
        seen: dict[str, int] = {}
        kept = []
        for it in items:
            n = seen.get(it["modality"], 0)
            if n < per:
                kept.append(it)
                seen[it["modality"]] = n + 1
        items = kept
    os.makedirs(OUT, exist_ok=True)

    jobs = [(it, m, a) for it in items for m in models for a in arms]
    print(f"runs: {len(jobs)} ({len(items)} artifacts x {len(models)} models x {len(arms)} arms)",
          flush=True)
    runs: list[Run] = []
    t0 = time.time()
    done = 0
    with ThreadPoolExecutor(max_workers=10) as ex:
        futs = {ex.submit(run_one, it, m, a): (it, m, a) for (it, m, a) in jobs}
        for f in as_completed(futs):
            r = f.result()
            runs.append(r)
            done += 1
            if done % 10 == 0 or done == len(futs):
                ok = sum(1 for x in runs if x.correct)
                print(f"[{done}/{len(futs)}] {time.time()-t0:6.0f}s  correct {ok} "
                      f"({ok/done:.0%})  last={r.modality}/{r.model.split('-')[0]}/{r.arm}",
                      flush=True)
                json.dump([{**asdict(x), "dollars": x.dollars} for x in runs],
                          open(f"{OUT}/runs4.json", "w"), indent=1)
    json.dump([{**asdict(x), "dollars": x.dollars} for x in runs],
              open(f"{OUT}/runs4.json", "w"), indent=1)
    print(f"wrote {OUT}/runs4.json "
          f"(excluded transport errors: {sum(1 for x in runs if x.transport_error)})", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
