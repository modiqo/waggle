"""Tier-3 corpus: six modalities, one needle each (design doc 22 §4.3).

Every artifact is realistic in structure and size and carries exactly one
unique AUDIT CODE at a *known* region. That gives three graded quantities at
once, uniformly across modalities:

  correctness — did the consumer report the code (exact match)
  cost        — bytes/tokens that entered the window to get it
  coverage    — did the receipts show the required region was consumed

Modalities and how the needle is placed:
  text     long plain-text report; needle inside a numbered section
  markdown long document; needle inside a named heading's section
  code     REAL source files; needle is a constant inside a real function
  pdf      a real PDF (rendered with tectonic); needle on a known page
  voice    speech (macOS TTS) whose transcript states the code
  video    the same speech muxed to a video track

For pdf/voice/video the artifact is binary: waggle carries the extracted
text via --content and the media itself via --attach. The transcript is
ground truth *by construction* — we are benchmarking the substrate's
handling of the modality, not speech recognition, and we say so.
"""

from __future__ import annotations

import json
import os
import random
import shutil
import subprocess
import sys

OUT = os.environ.get("TIER3_CORPUS", "/tmp/tier3/corpus")
REPOS = os.environ.get("TIER3_REPOS", "/tmp/tier3/repos")
SEED = 20260711  # published sampling seed (doc 22 §4.5)
N_PER = int(os.environ.get("TIER3_N", "4"))

FILLER = (
    "The pipeline stage reconciles inbound records against the ledger before "
    "the nightly compaction window. Operators should note that back-pressure "
    "is applied per shard, not per partition, and that retries are bounded by "
    "the lease horizon rather than by wall-clock time. "
)


def code(rng) -> str:
    return f"{rng.choice('QWXZ')}{rng.randint(1,9)}{rng.choice('KMPR')}-{rng.randint(1000,9999)}"


def section_text(i: int, needle: str | None) -> str:
    body = FILLER * 6
    if needle:
        body += f"\n\nAUDIT CODE: {needle}\n\n" + FILLER * 3
    return body


# --------------------------------------------------------------- text / md
def make_text(idx: int, rng, n_sec: int = 14):
    c = code(rng)
    hit = rng.randrange(3, n_sec - 1)
    parts, region = [], f"Section {hit}"
    for s in range(1, n_sec + 1):
        parts.append(f"Section {s}. Operational Notes\n{'=' * 40}\n")
        parts.append(section_text(s, c if s == hit else None))
        parts.append("\n\n")
    return "".join(parts), c, region


def make_md(idx: int, rng, n_sec: int = 14):
    c = code(rng)
    hit = rng.randrange(3, n_sec - 1)
    names = [f"Runbook {s}: Recovery Path" for s in range(1, n_sec + 1)]
    parts = ["# Operations Handbook\n\n"]
    for s in range(1, n_sec + 1):
        parts.append(f"## {names[s-1]}\n\n")
        parts.append(section_text(s, c if s == hit else None))
        parts.append("\n\n")
    return "".join(parts), c, names[hit - 1]


# --------------------------------------------------------------- code
def real_sources() -> list[str]:
    out = []
    for root, _, files in os.walk(REPOS):
        for f in files:
            p = os.path.join(root, f)
            if f.endswith(".py") and 8_000 < os.path.getsize(p) < 90_000:
                out.append(p)
    out.sort()
    return out


def make_code(src: str, rng):
    """Inject the needle as a constant inside a REAL function of a REAL file."""
    lines = open(src, encoding="utf-8", errors="replace").read().splitlines(True)
    cand = []
    for i, l in enumerate(lines):
        s = l.lstrip()
        if not (s.startswith("def ") and l.rstrip().endswith(":") and "(" in l):
            continue
        nm = s[4:].split("(")[0].strip()
        # The region must be UNAMBIGUOUS: skip dunders/private and any name
        # defined more than once in the file (coverage would be undecidable).
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
    return "".join(lines), c, name


# --------------------------------------------------------------- pdf
def make_pdf(path_md: str, out_pdf: str) -> bool:
    tex = out_pdf.replace(".pdf", ".tex")
    md = open(path_md).read()
    body = []
    for line in md.splitlines():
        if line.startswith("## "):
            body.append("\\section*{" + line[3:].replace("&", "\\&") + "}")
        elif line.startswith("# "):
            body.append("\\section*{" + line[2:] + "}")
        elif line.strip():
            body.append(line.replace("&", "\\&").replace("_", "\\_"))
        else:
            body.append("")
    open(tex, "w").write(
        "\\documentclass[11pt]{article}\\usepackage[margin=2cm]{geometry}\n"
        "\\begin{document}\n" + "\n".join(body) + "\n\\end{document}\n"
    )
    r = subprocess.run(
        ["tectonic", os.path.basename(tex), "--outdir", os.path.dirname(out_pdf)],
        cwd=os.path.dirname(tex), capture_output=True, text=True, timeout=300,
    )
    return os.path.isfile(out_pdf)


def pdf_text(p: str) -> str:
    from pypdf import PdfReader

    return "\n".join((pg.extract_text() or "") for pg in PdfReader(p).pages)


# --------------------------------------------------------------- voice / video
def make_voice(out_aiff: str, rng):
    """Speak a briefing; the transcript is a realistic timecoded, multi-line
    record with the needle on its own line — so the `lines` lens is meaningful
    and the needle format matches every other modality (`AUDIT CODE: X`)."""
    c = code(rng)
    sentences = [
        "Welcome to the operations briefing.",
        "The pipeline stage reconciles inbound records against the ledger.",
        "Back-pressure is applied per shard, not per partition.",
        "Retries are bounded by the lease horizon, not by wall clock time.",
        "The following is the item you were asked to retrieve.",
        f"AUDIT CODE: {c}",
        "Operators should confirm the code before the compaction window.",
        "That concludes the briefing.",
    ]
    # Spoken form: letters/digits read out so TTS is intelligible.
    spoken_code = " ".join(c.replace("-", " dash "))
    say_lines = [s if not s.startswith("AUDIT CODE") else f"Audit code: {spoken_code}."
                 for s in sentences]
    subprocess.run(["say", "-o", out_aiff, " ".join(say_lines)], check=True, timeout=300)

    # Ground-truth transcript, timecoded (by construction, not by ASR).
    transcript = "\n".join(f"[{i*8//60:02d}:{i*8%60:02d}] {s}" for i, s in enumerate(sentences))
    return transcript, c


def make_video(aiff: str, out_mp4: str) -> bool:
    r = subprocess.run(
        ["ffmpeg", "-y", "-f", "lavfi", "-i", "color=c=navy:s=640x360:r=5",
         "-i", aiff, "-shortest", "-c:v", "libx264", "-pix_fmt", "yuv420p",
         "-c:a", "aac", out_mp4],
        capture_output=True, text=True, timeout=600,
    )
    return os.path.isfile(out_mp4)


# --------------------------------------------------------------- build
def main() -> int:
    rng = random.Random(SEED)
    os.makedirs(OUT, exist_ok=True)
    items = []

    srcs = real_sources()
    print(f"real source files available: {len(srcs)}")

    for i in range(N_PER):
        # text
        t, c, reg = make_text(i, rng)
        p = f"{OUT}/text_{i}.txt"
        open(p, "w").write(t)
        items.append(dict(id=f"text_{i}", modality="text", path=p, answer=c,
                          region=reg, region_kind="lines", bytes=len(t)))

        # markdown
        m, c, reg = make_md(i, rng)
        p = f"{OUT}/md_{i}.md"
        open(p, "w").write(m)
        items.append(dict(id=f"md_{i}", modality="markdown", path=p, answer=c,
                          region=reg, region_kind="section", bytes=len(m)))

        # code (real file, real function)
        got = None
        while srcs and got is None:
            got = make_code(srcs[rng.randrange(len(srcs))], rng)
        if got:
            body, c, sym = got
            p = f"{OUT}/code_{i}.py"
            open(p, "w").write(body)
            items.append(dict(id=f"code_{i}", modality="code", path=p, answer=c,
                              region=sym, region_kind="symbol", bytes=len(body)))

        # pdf (rendered from a fresh markdown doc)
        m2, c2, reg2 = make_md(100 + i, rng)
        tmp_md = f"{OUT}/_pdf_src_{i}.md"
        open(tmp_md, "w").write(m2)
        p = f"{OUT}/doc_{i}.pdf"
        if make_pdf(tmp_md, p):
            txt = f"{OUT}/doc_{i}.txt"
            open(txt, "w").write(pdf_text(p))
            items.append(dict(id=f"pdf_{i}", modality="pdf", path=p, content=txt,
                              answer=c2, region=reg2, region_kind="section",
                              bytes=os.path.getsize(p)))

        # voice
        aiff = f"{OUT}/voice_{i}.aiff"
        spoken, c3 = make_voice(aiff, rng)
        tr = f"{OUT}/voice_{i}.txt"
        open(tr, "w").write(spoken)
        items.append(dict(id=f"voice_{i}", modality="voice", path=aiff, content=tr,
                          answer=c3, region="the audit code", region_kind="lines",
                          bytes=os.path.getsize(aiff)))

        # video (same speech, muxed)
        mp4 = f"{OUT}/video_{i}.mp4"
        if make_video(aiff, mp4):
            items.append(dict(id=f"video_{i}", modality="video", path=mp4, content=tr,
                              answer=c3, region="the audit code", region_kind="lines",
                              bytes=os.path.getsize(mp4)))

    json.dump(items, open(f"{OUT}/manifest.json", "w"), indent=1)
    by = {}
    for it in items:
        by[it["modality"]] = by.get(it["modality"], 0) + 1
    print("corpus:", by, f"-> {OUT}/manifest.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
