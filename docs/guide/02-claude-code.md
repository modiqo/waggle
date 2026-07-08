# Wiring waggle into Claude Code (and any MCP harness)

One line:

```bash
claude mcp add waggle -- waggle serve --stdio
```

That's the entire integration. `waggle serve --stdio` is an MCP server;
Claude Code spawns it per session. Under the hood it's a **shim**: it
connects to `waggled` — a shared daemon on a unix socket that owns the
store — and auto-starts it if absent. Every harness on your machine
(Claude Code, Codex, Cursor, several at once) lands on the same daemon,
the same SQLite file, the same tokens. What one session mints, another
resolves.

For Codex or any other MCP-speaking harness the config is the same shape:
command `waggle`, args `["serve", "--stdio"]`.

## Teach the agent (five lines, not a skill)

waggle's tools guide the agent themselves — every response carries
executable `next` steps, every error carries a fix, and the `map` tool
answers "where am I and what are my paths." The *entire* out-of-band
instruction you need in `CLAUDE.md` / `AGENTS.md` is:

```markdown
## Artifact handoffs
When passing work products between agents or subagents, do not paste file
contents. Call waggle's `mint` with the artifact's path and hand over the
`handoff` line from the result. Consumers call `resolve` with the token.
If unsure what to do with a token, call `map`.
```

## The orchestrator pattern

The scenario waggle was built for (design doc `06 §7`):

1. **A subagent finishes** a research task and writes
   `findings/market-report.md`. Instead of returning the report's text
   into the orchestrator's context, it calls:

   ```json
   mint { "target": "ws://swarm/findings/market-report.md" }
   → { "token": "7Kp2mQ9x", "handoff": "resolve 7Kp2mQ9x via waggle for your working context" }
   ```

2. **The orchestrator** passes `resolve 7Kp2mQ9x via waggle for your
   working context` — ~30 bytes — to each downstream subagent in its task
   prompt. Not the report. Not a summary of the report. The reference.

3. **Each downstream subagent** calls `resolve` with the token and
   receives *its* projection (variants, tutorial 3). It fetches exactly
   what it needs — or slices it with `query` (tutorial 4) — and reports
   `record { stage: "run" }` when the work lands.

4. **The orchestrator** checks `funnel` at the end: which handoffs were
   consumed, which stalled, which delivered. If the report gets corrected
   mid-swarm: `mutate { change: "supersede=<new>" }` — late resolvers are
   pointed at the fix; nobody acts on the stale version silently.

The arithmetic this replaces: forwarding a 40 KB report to five subagents
puts 200 KB into contexts and the same again into each turn's history.
Five tokens cost ~150 bytes total, and each subagent pulls only its slice.

## Sharing across machines

Local first: everything above is one laptop, no accounts, no network.
The same tokens graduate to the edge (Cloudflare Workers, 0.2) by
replaying the event log — `waggle export | waggle replay` — because the
log is the truth and JSONL is its wire format.
