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
