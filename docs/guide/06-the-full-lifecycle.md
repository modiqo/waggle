# The full lifecycle — one mission, followed end to end

One orchestrator, three agents, four tokens. This page follows a single
mission all the way down and back, showing where each token takes shape,
how the family tree forms, and why nobody's context window ever carries
a document it didn't ask for.

The mental model, before any diagram: **a waggle token is a coat-check
ticket.** You don't carry the coat around the party — you carry a ticket,
and whoever holds the ticket can retrieve exactly the coat, exactly when
they need it, while the coat-check counter quietly remembers every
retrieval. The coat never gets photocopied into anyone's pockets.

**Don't just read it — run it:** `just demo` executes all six acts below
against a throwaway store, printing the real envelopes.

## The cast

```
ORCHESTRATOR          plans the mission, spawns agents, reads the receipts
 ├── RESEARCHER       produces the 40 KB market report
 └── WRITER           drafts the launch brief from the report
      └── FACT-CHECKER  (sub-subagent) verifies the report's claims
```

## Act 1 · The mission gets a name

The orchestrator writes a task brief and mints the **root token**:

```
mint { target: "ws://mission/launch-brief-task.md" }
→ token M9kQ2vRe        "resolve M9kQ2vRe via waggle for your working context"
```

That one line — ~30 bytes — goes into each subagent's spawn prompt.
Not the brief. The ticket.

```
┌──────────────────────────────────────────────────────────────────────┐
│  ORCHESTRATOR's context after spawning two subagents:                │
│                                                                      │
│   "…spawn researcher: analyze the market.                            │
│       resolve M9kQ2vRe via waggle for your working context"          │
│   "…spawn writer: draft the launch brief.                            │
│       resolve M9kQ2vRe via waggle for your working context"          │
│                                                                      │
│   Total mission payload in flight: 2 × 30 bytes.                     │
└──────────────────────────────────────────────────────────────────────┘
```

## Act 2 · The researcher works, and the tree grows

The researcher resolves the ticket (the counter records it — that's
attribution happening, nobody opted in), reads the brief, and produces
`market-report.md` — 40 KB of findings. Now the critical move. It does
**not** return the report's text to the orchestrator. It mints a child:

```
mint { target: "ws://swarm/market-report.md", parent: "M9kQ2vRe" }
→ token 7Kp2mQ9x
```

`parent` is where **lineage** takes shape. The new token is born pointing
at its origin, and the family tree now exists as data — not as a
convention someone remembered to follow:

```
M9kQ2vRe  (the mission)
 └── 7Kp2mQ9x  (the market report — minted BY the researcher, UNDER the mission)
```

The researcher's entire report back to the orchestrator:

```
"Done. resolve 7Kp2mQ9x via waggle for your working context"
```

Thirty bytes crossed the boundary. The 40 KB stayed on disk, minted once,
named forever.

## Act 3 · The writer retrieves like a surgeon, not a shopper

The orchestrator forwards `7Kp2mQ9x`'s handoff line to the writer. Watch
what the writer's context actually ingests:

**First, orient — never guess:**

```
map { token: "7Kp2mQ9x" }
→ here: "7Kp2mQ9x — active · 2 variants · 1 resolve · 0 runs · 0 children"
  next: [ resolve …, record … ]                                (~300 bytes)
```

**Then resolve — and receive a projection, not the blob.** The researcher
declared variants at mint: a 2 KB executive summary as the catch-all, the
full-report pointer for consumers who declare they need it. The writer's
resolve returns *its* variant — the summary:

```
resolve { token: "7Kp2mQ9x" }
→ { variant: 1, body: { inline: "…2 KB executive summary…" },
    as_of: …, revalidate_after: … }                            (~2 KB)
```

**Need one specific thing? Slice, don't pull.** The token's document
(manifest, funnel, lineage) answers by path, under a byte budget, with
guidance deeper:

```
query { token: "7Kp2mQ9x", path: "/manifest/variants" , max-bytes: 512 }
→ { slice: { kind: "array", len: 2, bytes: 41337 },
    next_paths: ["/manifest/variants/0", "/manifest/variants/1"] } (≤512 bytes)
```

`bytes: 41337` is the point made visceral: **the response tells you the
size of the payload you just avoided ingesting.** No reply exceeds the
budget you set — an oversized value returns its *shape* plus paths deeper,
so the writer walks exactly as far as it needs and not one level more.

```
  The two ways to move a 40 KB report to 3 consumers
  ──────────────────────────────────────────────────
  paste into contexts:   40 KB × 3 places  = 120 KB in windows
                         …then re-sent with EVERY subsequent turn
                         (5 more turns each → ~600 KB of re-reads)

  waggle:                3 × 30 B handoffs = 90 bytes in flight
                         each consumer pulls its projection once:
                         2 KB + 2 KB + 40 KB (one full reader) = 44 KB total
                         re-turns re-send the 30-byte line, not the report
```

## Act 4 · Delegation goes deeper — the tree keeps its shape

The writer spawns a fact-checker and simply forwards the same ticket —
tokens are forwardable, and every holder's resolve is attributed
separately (actor *class* only: agent/human, model family — never
identity). The fact-checker verifies claims and reports its stage:

```
record { token: "7Kp2mQ9x", stage: "assess" }
```

Then the writer finishes the brief and mints its own child under the
mission:

```
mint { target: "ws://swarm/launch-brief.md", parent: "M9kQ2vRe" }
→ token Xw4tR8nA
record { token: "7Kp2mQ9x", stage: "run" }      ← "I used the report"
```

The tree, now — queryable by anyone holding the root:

```
M9kQ2vRe  mission
 ├── 7Kp2mQ9x  market report      funnel: resolve 3 · assess 1 · run 1
 └── Xw4tR8nA  launch brief       funnel: resolve 0 (just born)
```

## Act 5 · The correction — where lineage earns its keep

The fact-checker found a bad number. The researcher fixes the report and
**supersedes** — compare-and-swap guarded, so two concurrent correctors
can't silently stomp each other:

```
mint { target: "ws://swarm/market-report-v2.md", parent: "M9kQ2vRe" }
→ token 9rTq3wXk
mutate { token: "7Kp2mQ9x", change: "supersede=9rTq3wXk", expected-version: 1 }
```

Here is the moment every context-pasting swarm gets wrong. The stale
report is *already in flight* — the writer resolved it an hour ago. But
waggle resolutions are **knowledge with an expiry hint, not a copy with
no memory**: each carried `revalidate_after`. When the writer re-resolves
before its final draft:

```
resolve { token: "7Kp2mQ9x" }
→ { disposition: { superseded: { by: "9rTq3wXk" } },
    body: …still served…,
    next: [ resolve 9rTq3wXk — "the corrected artifact lives here" ] }
```

Nobody hunts through contexts deleting stale paragraphs. The reference
knows it was replaced, says so, and points forward. (A `revoke` is the
harsher sibling: a tombstone that serves nothing — and children tombstone
with it, the whole branch at once.)

## Act 6 · The orchestrator reads the receipts

The mission ends where it began — one token, and questions the
orchestrator could never answer in a context-pasting swarm:

```
funnel { token: "M9kQ2vRe" }     Which handoffs were consumed? Which stalled?
map    { token: "7Kp2mQ9x" }     "superseded — follow the pointer to 9rTq3wXk"
query  { token: "M9kQ2vRe", path: "/children" }     the delegation tree, as data
```

And because every record in that story is a line in an append-only log,
the whole thing **replays**: shuffle the events, duplicate them, ship
them to another machine — `reconstruct` rebuilds the identical state,
byte for byte. The swarm's coordination isn't a memory anyone holds.
It's a ledger anyone can re-derive.

## The five sentences to remember

1. **Mint, don't paste** — the artifact stays put; a 30-byte ticket moves.
2. **`parent` at mint is the lineage** — the delegation tree is data, not discipline.
3. **Resolve returns your projection** — the consumer gets what fits it, not the blob.
4. **Query slices under a budget** — ask for the page, never the book; the reply names the bytes you were spared.
5. **Supersede/revoke travel through the reference** — corrections reach late readers because the *name* knows, not because someone chased down every copy.
