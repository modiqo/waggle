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
| extractable | PDF, HTML (deterministic text layer) | extracted + indexed at mint (§7); read/search work over the text, provenance recorded |
| media | `image/*`, `audio/*`, `video/*` | no text layer: `MediaRef` fetch; read directs the consumer to its own vision/speech model |

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
a PDF or HTML text layer is extracted at mint (§7); other binary → media path, and read directs the consumer to perceive it.

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

*The remote form of everything in this doc is governed by the
computation-travels-to-the-data contract — doc `08 §0`: the caller never
learns where the bytes are and only ever receives budgeted answers.*

Locally the daemon already runs as the user; a token adds no privilege.
Remotely (token-gated TCP, edge) a token becomes a **read capability**
for its content — scoped to `manifest.content` blobs (never live
filesystem reads for remote callers), governed by CP-11 signing.

## 7 · The extraction boundary: determinism, not format

The line is **not** "text versus binary." It is **deterministic versus
model-requiring**, and it runs straight through the binary formats:

- A **PDF's embedded text layer** and an **HTML document's text** are
  *pure functions of the bytes*. `pdftotext` reads a text stream the file
  literally stores; stripping tags reads the DOM. Same bytes in, same
  text out, no model, forever. The substrate does this **at mint**,
  records the extractor's identity, and signs the result into the core as
  `manifest.extraction` — so the token *carries* the searchable text and
  `read`/`search`/`coverage` work over the artifact itself.
- A **scanned page**, **audio**, or **video** carries no text layer.
  Recovering its content takes a *model* — OCR, ASR — and a model's output
  is an opinion that drifts, is worse than the consumer's own perception,
  and is chosen before the question is known. The substrate refuses. It
  pins the raw bytes and the projection says so plainly: *"content is
  audio/mp4 — the substrate does not read this; fetch the bytes and
  interpret them with your own speech model."*

**Why determinism is the right boundary, and why it is the same boundary
as the receipts.** A substrate that transcodes must *read* what it
carries, and reading content non-deterministically is exactly what I-1
(payload-free log) and I-2 (sealed, reproducible matcher) forbid. A
deterministic extraction breaks neither: the log still records only that a
token was minted, and the matcher over the extracted text reproduces
exactly. The moment the substrate ran a model, a coverage receipt could
attest to a transcript nobody can reproduce — so the receipts are
trustworthy *because* the extractor is deterministic. `manifest.extraction`
carries a `deterministic` flag for this reason: a text-layer extraction is
`true`, and a registered model extractor (opt-in, never a default) would be
`false`, stamping its text as an opinion so a reader weighs the receipt
accordingly.

**Why not leave it wholly to the harness (the old design, now rejected).**
Passing text via `--content` leaves the extraction as a *loose file the
next agent must locate*, and the receipt then attests to text the substrate
never saw come out of the PDF — a different harness, a corrupted
extraction, or a substituted string all produce the same confident receipt.
For the token to be a trustworthy reference *to a PDF*, the substrate must
own the bytes→text relationship. `--content` remains for a format the
substrate does not read, or to override its extraction, but it is no longer
the common path.

The mint shapes, spelled out:

| Artifact | Call | searchable content | provenance |
|---|---|---|---|
| text file | `mint --snapshot` | the target's own bytes | verbatim |
| PDF / HTML | `mint --snapshot` | the substrate's text-layer extraction | `pdf-textlayer` / `html-strip`, deterministic |
| voice / video | `mint --snapshot` | none — raw bytes pinned | `read` directs the consumer to its own model |
| a format we don't read | `mint --content extracted.txt` | your extraction | harness-supplied |

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


---

## 9 · The directory affordances (added; see doc 22 §4)

A token may name a **folder** (`mint --tree`: one parent, every file a child).
These three affordances were not designed at a whiteboard — each exists
because the cross-modality benchmark caught an agent needing it and not
having it, and the traces name the failure exactly.

### 9.1 Describe (`read` on a tree)

`search` had always grepped a tree. `read` answered **null**. So a folder
could be searched and never *described* — and the first move every agent
makes with a shared directory is to ask what is in it. Both models we traced
opened with an overview call and received 83 bytes of nulls, then had to
guess a regex blind; one guessed wrong and got zero matches.

`read` on a tree now returns **the tree**: each file, its own token, size,
content type, and outline. The folder's table of contents. Listing is *not*
consumption — no `read` stage is stamped for the children, because a table of
contents tells you what exists and does not serve you the bytes.

### 9.2 Lens (`read --section/--symbol/--lines` on a tree)

A tree could be grepped but not *lensed*. We watched an agent issue
`section: "Retry Policy"` **ten times**, once per child token — hand-rolling
a fan-out the substrate should perform in one call. A lens on the folder
token now answers for **every file at once** (13 ops → 3; 11k tokens → 4.5k).
Unlike the listing, this *serves bytes*, so every file it answers for is
stamped read: the receipt records what the consumer actually got.

### 9.3 Finish (`complete` / `examined` / `total_files` / `--from`)

And that fan-out promptly produced a **confident wrong answer**. It exhausted
its budget after nine of ten runbooks; the missing one was the one that
mattered; the agent reasoned over a partial folder and answered with
conviction. **A truncated fan-out that reads like a whole one is worse than a
slow one.** The response now carries `total_files`, `examined`, and
`complete`, and when it is short it returns a `from` cursor and a `next` that
says INCOMPLETE in words. Given that, the same agent resumes and gets it
right.

The rule the three share is the one this document has argued from the start,
now stated for trees: **a projection must never be a dead end, and it must
never pretend to be whole when it is not.**
