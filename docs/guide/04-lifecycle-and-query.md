# Lifecycle, attribution, and querying without pulling

## The funnel is the product

Every resolve is recorded with the variant that served; downstream stages
are one call:

```bash
waggle record --token 7Kp2mQ9x --stage run
waggle funnel --token 7Kp2mQ9x
# → { "stages": { "resolve": 3, "run": 2, "repeat": 1 }, "children": [...] }
```

Counts only — events are payload-free by construction. The funnel answers
the orchestrator's real questions: which handoffs were consumed, which
stalled at resolve, which delivered repeat value.

## Judging the work: outcomes and the escalation choreography

Consumption is the consumer's story; the **verdict** is yours. When a
delegation comes back, record it — the verdict is a stage, so nothing
about *why* ever enters the log:

```bash
waggle record --token 7Kp2mQ9x --stage accepted    # or: rejected
waggle funnel --token 7Kp2mQ9x
# → { "stages": { ..., "rejected": 1 }, "outcome": "rejected", ... }
```

`outcome` is derived from counts, order-free: neither verdict →
`pending`, one → that verdict, both → `contested` (surfaced for you to
resolve, never silently overwritten — re-judgments should supersede,
not overwrite).

A rejection's response teaches the **escalation choreography**: re-mint
the same target under the *same parent* (aimed at a stronger consumer),
then supersede the rejected child so late readers follow the pointer:

```bash
waggle mint --target file://$PWD/plan.md --parent M9kQ2vRe   # → 9rTq3wXk
waggle mutate --token 7Kp2mQ9x --change supersede=9rTq3wXk --expected-version 1
```

The escalation is now **lineage, not lore**: a rejected child
superseded by a sibling under the same parent is a queryable fact —
the raw material for the routing scorecard (design doc 19 §4.6).

## Proof of reading: consumption contracts

When the handoff has load-bearing sections, declare them at mint —
`section:` sugar resolves against the outline right then; the manifest
stores plain line ranges, signed with the core:

```bash
waggle mint --target file://$PWD/plan.md --snapshot \
    --require section:Pricing --require "lines:120-180" --min-coverage 1.0
```

Every served `read` window and `search` hit stamps which required
regions it overlapped (positions only — patterns and text never enter
the log). `coverage` on the token then answers the question no
orchestrator could ask before:

```bash
waggle coverage --token 7Kp2mQ9x
# → { "met": false, "contract": { "required": 2, "touched": 1, ... },
#     "missed": [ { "region": 0, "label": "Pricing", "lines": "847-920" } ] }
```

Misses are **named**. A subagent that claims to have followed the plan
with `met: false` gets caught before its answer is trusted — check the
receipt, then record your verdict.

## Correcting the record: supersede and revoke

Handoffs go stale. The report gets corrected. Two lifecycle moves, both
**compare-and-swap guarded** so concurrent correctors can't stomp each
other:

```bash
# Read the current version first (map and resolve both show it)…
waggle map --token 7Kp2mQ9x        # reverse paths carry expected-version

# …then supersede: late resolvers get content PLUS the pointer forward
waggle mutate --token 7Kp2mQ9x --change "supersede=9rTq3wXk" --expected-version 1

# …or revoke: a tombstone; nothing is served, children tombstone with it
waggle mutate --token 7Kp2mQ9x --change revoke --expected-version 1
```

Send a stale `--expected-version` and you get a **conflict that names the
fix** — re-read, re-decide, retry:

```json
{ "hint": "version conflict on 7Kp2mQ9x: expected 1, current 2 — re-read and retry",
  "next": [ { "tool": "resolve", "args": { "token": "7Kp2mQ9x" },
              "why": "re-read the manifest for the current version, then retry" } ] }
```

Cosmetic changes (`--change "label team=research"`, `campaign`) are
last-writer-wins and need no version.

## Delegation trees

Mint children with `--parent`; revoking the parent tombstones the tree;
`funnel` and `map` show the children. This is the coordination trace: who
handed what to whom, replayable.

```bash
waggle mint --target "ws://swarm/subtask-brief.md" --parent 7Kp2mQ9x
```

## Query: slices with guidance, never whole responses

The document behind a token (manifest + funnel + lineage) can be big.
Don't pull it — slice it:

```bash
waggle query --token 7Kp2mQ9x                          # root shape, 4 KB budget
waggle query --token 7Kp2mQ9x --path /manifest/variants/0/body
waggle query --token 7Kp2mQ9x --path /funnel --max-bytes 256
```

Guarantees worth relying on:

- **No response ever exceeds `--max-bytes`** (default 4096, floor 64). An
  oversized value returns its shape — `{ kind, bytes, keys }` — plus the
  paths deeper.
- **`next_paths` are complete**: walking them from the root reaches every
  leaf. Guidance is tested, not decorative.
- A wrong path errs with the **valid roots named**.

## Export / replay: the store is a stream

```bash
# every record, one JSON object per line — the permanent wire format
waggle query --token 7Kp2mQ9x --path /manifest   # inspect…
```

The JSONL journal backend and the SQLite backend both speak the same
`LogRecord` lines; migration between stores (laptop → team box → edge) is
export → replay, idempotent under retries and duplicates by contract
(C-4/C-8). The `export`/`replay` CLI verbs land with the 0.2 edge tier;
the mechanics are already tested end to end.


## Finding tokens: tags and `find`

Nobody remembers `7Kp2xQ9f`. Tag at mint (cosmetic labels — outside the
signed core, so renaming never breaks a signature):

```sh
waggle mint --target ./design_docs --tree --tag design_docs --tag kind=reference
waggle mint --target plan.md --snapshot --tag "name=q3 plan"
```

Then discover by anything a human remembers — basename, tag, channel,
sharer:

```sh
waggle find design_docs
#  rszskHrD  design_docs  active  {name: design_docs, kind: reference}
waggle find q3
#  eQTEmhf3 ... -> next: resolve {token: eQTEmhf3}
```

The design line: **names are LOOKUP, tokens are IDENTITY.** `find`
returns ranked candidates (newest first, disposition visible — a
revoked `plan.md` says so instead of resolving silently); you choose
what to resolve. A name never resolves by itself. Old tokens join the
party retroactively: `waggle mutate --token <t> --change "label name=..."`.
