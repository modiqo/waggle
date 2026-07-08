# 18 — Content Access: the token as a file descriptor

*New in revision 2.6. Closes the surgical-access gap: `query` slices a
token's metadata document; nothing sliced its **content**. Consumers had
to fetch whole artifacts (or grep a local path outside waggle — invisible
to attribution, impossible across machines). This doc adds `read` and
`search`: grep/sed semantics through the reference itself.*

## 1 · The principle

A waggle token names an artifact. With `read`/`search` it also **answers
questions about the artifact's content** — under the same discipline as
`query`: hard byte budgets, responses that name the bytes they spared
you, executable `next` guidance, errors that name the fix.

The one-sentence version: *the token tells you where the coat is;
`read`/`search` reach into its pocket without taking the coat.*

## 2 · The lens stack

Formats are not special-cased; they select **lenses**. Every text
artifact gets lens 0; content type adds lens 1. Lenses are discoverable
(the `read` overview lists them), never guessed.

| Lens | Applies to | Addressing |
|---|---|---|
| **0 · lines + search** | any text (`text/*`, json, yaml, code, csv, logs) | `lines: "A-B"`, regex `pattern` |
| **1 · outline/section** | `text/markdown` | `path: "/outline"`, `section: "<heading>"` |
| **1 · pointer** | `application/json` | `path: "/deps/react/version"` — **reuses the CP-7 `slice_at` engine on parsed content** |
| media | binary (`image/*`, `audio/*`, docx, pdf) | not line-addressable: `MediaRef` fetch (exists); extract-at-mint variant makes them sliceable |

Explicit v1 boundaries (recorded, not hidden): YAML gets lens 0 only
(no maintained serde-yaml — parser choice deferred); code symbols
(tree-sitter) deferred — `search "fn resolve"` + `read` window covers
the loop at zero dependency cost; docx/pdf extraction is author-side.

## 3 · Where the bytes come from (and `mint --snapshot`)

Resolution order, deterministic:

1. **`manifest.content`** — a new **immutable-core** field:
   `Option<MediaRef>`, set only at mint. `mint --snapshot` reads the
   target, stores it content-addressed in the blob CAS, and pins the
   `MediaRef`. This is the load-bearing addition: the searched content is
   **immutable by hash** (what you grep is what was minted), and it
   travels wherever blobs replicate — search works on machines where the
   file never existed (the 0.2 edge story).
2. **The target**, when it is a locally readable `file://` path — the
   daemon reads it fresh (mutable artifacts: you search the file as it is
   now; the doc says so).
3. Neither → the error names the fix: *"no readable content behind this
   token — mint with --snapshot or --attach"*.

Size cap: 16 MB per read (hint: split or snapshot a subset). Text rule:
content types `text/*`, `application/json`, `application/yaml` slice;
anything else is binary → media path, with the extract-at-mint hint.

## 4 · The operations

**`read`** — token (req); one of `lines: "A-B"` · `section: "<heading>"` ·
`path: "<pointer>"`; `max-bytes` (default 4096, floor 64). With no
address: the **overview** — total lines/bytes, content type, available
lenses, and (markdown) the outline with line numbers or (json) the root
shape. Windows clamp to the budget and report the range actually
returned; `next` continues the window.

**`search`** — token (req), `pattern` (regex, req), `context` (lines,
default 2), `max-matches` (default 5, cap 50), `max-bytes`. Returns
matches `{line, text, before, after}`, **`total_matches` counted in
full** even when truncated, and `next` chains the first match into a
`read` window — the grep→open loop, through the reference.

Map edges: `resolve → search` ("interrogate before ingesting"),
`search → read`, `read → read` (continue). Both ops: catalog + CLI +
MCP, parity-guarded like everything else.

## 5 · Attribution and I-1

Every `read`/`search` records the new well-known stage **`read`** —
a count with actor class. **Never the pattern, never the matched text,
never the ranges** (I-1 unchanged: no payload field exists). Authors
learn "searched 14× before anyone ran" — content stays dark to the log.

## 6 · Capability note

Locally the daemon already runs as the user; a token adds no privilege.
Remotely (token-gated TCP, edge) a token becomes a **read capability**
for its content — scoped to `manifest.content` blobs (never live
filesystem reads for remote callers), governed by CP-11 signing.

## 7 · The format boundary: who decodes PDF, docx, images, voice

**Waggle does not decode formats. Ever.** The boundary is principled,
not provisional, and it is division of labor — not delegation:

- **Extraction is the harness's job, once, at mint.** The minting agent
  is multimodal — it reads PDFs and images natively, better than any
  bundled Rust extractor will (tables, layout, scans, accents). It
  extracts with its own abilities and passes the text to waggle.
- **Persistence, integrity, serving, and attribution of that extraction
  are waggle's job, forever.** `mint --content <extracted.txt>` stores
  the extraction content-addressed and pins it as `manifest.content`;
  every downstream consumer gets budgeted `read`/`search` against it, on
  any machine, with `read`-stage receipts.

Why not bundle extractors (rejected explicitly): a format treadmill
(pdf, docx, pptx, epub, HEIC, …), mediocre quality exactly where it
matters, binary bloat against the plumbing ethos — and it duplicates a
capability the models improve at without us shipping a byte.

Why not leave it wholly to consumers (rejected explicitly): N consumers
× M machines re-reading a 60-page PDF with vision calls is the
token-waste problem in its most expensive form. **Extract once at
check-in; search forever.**

The three mint shapes, spelled out:

| Artifact | Call | `manifest.content` | Variants |
|---|---|---|---|
| text file | `mint --snapshot` | the target's own bytes | catch-all |
| PDF/docx | `mint --content extracted.txt` (harness extracted it) | the extraction | `--attach` the original for vision/human consumers |
| voice memo | `mint --content transcript.txt --attach memo.m4a` | the transcript | audio `MediaRef` for listeners |

`--snapshot` and `--content` are mutually exclusive (both claim
`manifest.content`); passing both is refused with the distinction named.
The agent-stub instruction (17, one added sentence): *"when minting a
binary artifact, extract its text with your own abilities and pass it
via content."* Every multimodal harness becomes an extraction worker for
the network — for free.

Roadmap here (small, format-agnostic): byte-range reads on `MediaRef`
blobs (audio segments, resumable pulls — ranges, never decoding).

## 8 · Gates (CP-7.5)

- lens-0 unit suite: window clamping, 1-based inclusive ranges, regex
  matching with context, `total_matches` honesty under truncation,
  budget property (no response exceeds `max-bytes`, randomized);
- lens-1: markdown outline/section extraction; JSON pointer ≡ `slice_at`
  on parsed content (delegation test);
- integration: mint a real file → search → read-the-match window over
  dispatch; **the binary story**: mint a PDF-shaped target with
  `--content extracted.txt` → search hits the extraction while the
  target stays binary; `--snapshot`+`--content` together refused; **snapshot immortality** (mint --snapshot, delete the file,
  search still answers from the CAS, hash-verified); binary refusal with
  hint; funnel shows `read` counts; `envelope_next_valid` over every
  response;
- catalog: parity + reachability green with `read`/`search`; COMMANDS.md
  regenerated; tutorial (guide 07) written from live transcripts.
