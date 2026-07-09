# Surgical content access — grep through the token

Agents are surgical on their own filesystem: `rg` the pattern, `sed` the
window, never `cat` the whole file. `read` and `search` give a waggle
token the same moves — **through the reference**, so they survive the
handoff, work where the file doesn't exist, stay inside byte budgets,
and show up in attribution.

Every output below is a real transcript. Follow along:

```bash
waggle mint --target "file://$PWD/market-report.md" --snapshot
# → token Fei4AUN1
```

`--snapshot` is the important flag: the file's bytes are stored
**content-addressed** (SHA-256) in the blob store at mint. What you grep
later is exactly what was minted — and it travels wherever the blobs do.
(Without `--snapshot`, `read`/`search` fall back to the live local file:
fine on one machine, gone across machines.)

## Step 1 · The overview — what does this content afford?

A `read` with no address returns the shape of what's behind the token:

```bash
$ waggle read --token Fei4AUN1
{
  "content_type": "text/markdown",
  "total_lines": 12,
  "total_bytes": 290,
  "lenses": ["lines", "search", "outline", "section"],
  "outline": [
    { "line": 1,  "level": 1, "heading": "Market Report" },
    { "line": 3,  "level": 2, "heading": "Methodology" },
    { "line": 7,  "level": 2, "heading": "Competitor Pricing" },
    { "line": 11, "level": 2, "heading": "Risks" }
  ]
}
```

**Lenses are discovered, never guessed.** Markdown affords an outline and
sections; JSON affords pointer paths; everything textual affords lines
and search. The agent reads the table of contents — a few hundred bytes —
and decides where to cut.

## Step 2 · Search — the matches travel, the artifact stays put

```bash
$ waggle search --token Fei4AUN1 --pattern '(?i)pricing' --max-matches 2
{
  "matches": [
    { "line": 7, "text": "## Competitor Pricing",
      "before": ["against at least two independent citations.", ""],
      "after":  ["Pricing clusters at $49-79/mo for the mid tier.",
                 "Enterprise pricing is bespoke and gated."] },
    { "line": 8, "text": "Pricing clusters at $49-79/mo for the mid tier.", … }
  ],
  "total_matches": 4,
  "returned": 2,
  "truncated": true
}
next: [ { "tool": "read", "args": { "token": "Fei4AUN1", "lines": "1-17" },
          "why": "open the first match's neighborhood" } ]
```

Three honesty guarantees, all load-bearing:

- **`total_matches` is counted in full** even when the list is truncated —
  you always know what you didn't see;
- every response fits `--max-bytes` (default 4 KB, floor 64);
- `next` chains the first hit into a `read` window — the **grep → open
  loop** agents already live by, expressed as executable guidance.

## Step 3 · Read — windows, sections, pointers

**A markdown section**, case-insensitive, spanning to the next sibling
heading:

```bash
$ waggle read --token Fei4AUN1 --section "competitor pricing"
{
  "lines": "7-10",
  "text": "## Competitor Pricing\nPricing clusters at $49-79/mo for the mid tier.\nEnterprise pricing is bespoke and gated.\n",
  "total_lines": 12,
  "truncated": false,
  "next_window": "11-12"
}
```

**A line window** (`--lines 120-180`) clamps to the budget and reports
the range actually returned, with `next_window` to continue precisely.

**A JSON pointer** — this is the CP-7 slice engine pointed at *parsed
content*, so a lockfile behind a token answers like a database:

```bash
$ waggle read --token <lockfile-token> --path /dependencies/react/version
{ "path": "/dependencies/react/version", "slice": "18.3.1",
  "truncated": false, "full_bytes": 8 }
```

One value, ~40 bytes, out of a file that could be 2 MB.

## Step 4 · The part that changes the game: content outlives the file

```bash
$ rm market-report.md                       # the source is GONE
$ waggle search --token Fei4AUN1 --pattern bespoke
{ "total_matches": 1, ... }                 # …and the token still answers
```

Because `--snapshot` pinned the bytes content-addressed, the token's
content is **immutable by hash and independent of the filesystem**.
Edit the file, delete it, hand the token to an agent on another machine
(once blobs replicate, 0.2) — what they grep is what was minted. That's
the property no `grep`-a-path workflow can have.

## Step 5 · Attribution — reads are receipts

```bash
$ waggle funnel --token Fei4AUN1
{ "stages": { "read": 4 } }
```

Every `read`/`search` recorded the `read` stage — **a count, an actor
class, and nothing else**. Never the pattern, never the matched text
(invariant I-1: the log has no payload field to leak into). The author
learns *"my report was searched four times before anyone ran with it"* —
content stays dark to analytics.

## Binary artifacts: extract once at mint — the PDF story

Waggle **never decodes formats** — no bundled PDF parsers, no OCR. The
division of labor (design doc 18 §7): *you* (the minting agent) are
multimodal — you read the PDF with your own abilities, once. Waggle
persists, serves, and attributes that extraction forever:

```bash
$ waggle mint --target "file://$PWD/q3-report.pdf" \
              --content q3-report.extracted.md
# → token faBV9rNK   (target stays the binary; the extraction is the searchable content)

$ waggle search --token faBV9rNK --pattern '(?i)revenue grew'
{ "matches": [ { "line": 4, "text": "Revenue grew 34% quarter over quarter." } ],
  "total_matches": 1 }

$ waggle read --token faBV9rNK
{ "content_type": "text/markdown", "lenses": ["lines","search","outline","section"], … }
```

The economics are the point: the expensive multimodal read happens
**once, at check-in**, by the agent best equipped to do it. Every
downstream consumer — any machine, any harness — gets 40-byte answers
out of it. Five subagents re-reading a 60-page PDF with vision calls is
the token-waste problem in its most expensive form; this is its
inverse. Same pattern for voice: transcribe once, `--content` the
transcript, `--attach` the audio for consumers with ears.

Guard rails, tested: `--snapshot` and `--content` together are refused
(the hint names the difference — snapshot pins the *target's* bytes,
content pins *your extraction*), and passing a binary as the
"extraction" is refused with the fix.

## Formats at a glance

| Content | Lenses | Notes |
|---|---|---|
| `md` | lines · search · outline · section | headings inside code fences ignored |
| `json` | lines · search · path | pointer lens = the `query` engine on parsed content |
| `txt`, code, csv, logs, `yaml` | lines · search | universal — how `rg` treats them anyway |
| `pdf`, `docx`, voice | via `--content` (your extraction) | extract once at mint; original rides as `--attach` for vision/audio consumers |
| images, audio, other binary | — | refused with a hint: `MediaRef` fetch, or `--content` an extraction |

Errors always name the fix: a bad section returns the outline; a bad
pointer returns the valid roots; a token with no readable content says
*"mint with snapshot=true so content travels with the token."*


## Coverage: proof the tree was read

For any lineage root (a `--tree` mint or a bundle), the per-child
funnels already know which files were consumed — `coverage` turns that
into proof, with misses NAMED:

```sh
waggle coverage --token <root>
#  read 2/3 · run 0/3 · complete: false
#  unread: [.../notes.md]      <- what the review skipped
```

Three honest levels: `unread` / `read` (bytes served — a deep search
over the root counts, because it really reads every file) / `run` (the
consumer recorded use — the strong, intentional bar). It's receipts,
not surveillance (payload-free, I-1), and receipts turn
enforcement-grade when the handoff is SEALED (`waggle-tmux mint --seal`)
or remote — where the token is the only door.
