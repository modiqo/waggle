# 17 — Agent Fluency: the Self-Teaching Tool Surface

*New in revision 2.4 (audit finding F-1). The adoption risk was never the
mechanism — it was that nothing makes an agent reach for it over
`/tmp/analysis.md`. The rote lesson, adopted wholesale: **instructions don't
live in prompts or skills; they live in the tool surface itself** — tool
descriptions written as instruction, guidance in every output, and a `map`
tool that answers "I am here — what are my forward and reverse paths?"
Skills drift from tools as tools evolve; a map generated from the tool
registry cannot. Drift-proofing is the design, not a hope.*

## 1. The doctrine (four rules)

1. **Tool descriptions are the primary teaching surface.** They are written
   as agent instruction — when to use, when *not* to, the one-call form
   first — and reviewed with the same rigor as the API (they *are* the API's
   documentation to its real audience).
2. **Guidance lives in output.** Every tool response carries executable next
   steps; every error carries a recovery hint. The agent reads what to do
   next from the response, not from a stale skill.
3. **Zero-ceremony default, escalation optional.** The cheap path must beat
   the file path: `mint {target}` is one call — sharer defaulted from the
   session, channel defaulted to `subagent/general`, catch-all variant
   auto-generated from the target. Variants, media, TTLs are escalations,
   never prerequisites.
4. **Minimal footprint.** No bundled 700-line skill. The entire out-of-band
   instruction is a ~5-line AGENTS.md/skill stub: *"waggle passes attributed
   references between agents instead of pasted content. When unsure, call
   `map`."* Everything else the surface teaches itself.

## 2. The response envelope (normative)

Every tool response is:

```jsonc
{
  "result": { /* the tool's payload */ },
  "next": [                       // ≤3, ordered, EXECUTABLE — not prose
    { "tool": "resolve",
      "args": { "token": "wg:7Kp2", "context": "explicit:self" },
      "why": "verify your variants serve as intended before handing off" }
  ],
  "hint": null,                   // errors only: one calm recovery sentence
  "stats": { /* 13 §6 — measurability as a user feature */ }
}
```

Rules: `next` entries are schema-valid calls (CI-enforced, §5) with real
argument values where known, templates (`"<your-subagent-role>"`) where not.
`mint`'s first `next` is always **the handoff line** — the exact sentence to
place in a subagent's prompt (*"resolve wg:7Kp2 via waggle for your working
context"*) — because that's the moment the orchestrator needs it. Error
`hint`s name the fix, not the failure (`"variant list has no catch-all — add
MatchExpr::any() or omit variants entirely"`).

## 3. The `map` tool: here · forward · reverse

```jsonc
// map()                — global orientation (empty store, first contact)
// map { token }        — orientation for one token, derived from its
//                        manifest + funnel state (stateless computation)
{
  "here": "wg:7Kp2 — minted 4m ago · 2 variants · 0 resolves · active ·
           1 child (subagent/data-check)",
  "forward": [
    { "tool": "resolve", "why": "self-check the projection each consumer will get", "args": {…} },
    { "tool": "record",  "why": "mark downstream stages as your task progresses", "args": {…} },
    { "tool": "funnel",  "why": "see which subagents resolved and which stalled", "args": {…} }
  ],
  "reverse": [
    { "tool": "mutate", "args": { "change": "revoked", "expected_version": 1 },
      "why": "withdraw — children tombstone with it (C-7)" },
    { "tool": "mutate", "args": { "change": { "superseded_by": "<new-token>" } },
      "why": "replace with a corrected artifact; late resolvers follow the pointer" }
  ],
  "guidance": "no consumer has resolved this yet — hand off with:
               'resolve wg:7Kp2 via waggle for your working context'"
}
```

Design rules:

- **`here` is derived, never stored** — a pure function of (manifest,
  funnel prefix), so the map is always true at its snapshot and testable as
  a fold.
- **Forward paths are ranked by state**: an unminted store suggests `mint`;
  a minted-unresolved token leads with the handoff line; a resolved token
  leads with `funnel`; a drifted/superseded token leads with the repair
  pointer. The map is the skill, computed.
- **Reverse paths are honest about append-only**: mutations reverse via
  revoke/supersede/expire (CAS-guarded); *events do not reverse* — the map
  says so and offers the compensating move ("record a correcting stage") —
  the same honesty discipline as everything else.
- **`query`'s `next` guidance (13 §9) is this same system** at path
  granularity — one navigation model at two zoom levels.

## 4. Drift-proofing (why this beats skills structurally)

The forward/reverse edges are **declared in the operations catalog**
(`waggle-ops`, 09 §2 — the same `OperationSpec` table that generates the MCP
tool schemas and parity-checks the clap CLI). Consequences:

- A new tool ships with its edges or fails CI — the map can't lag the
  surface.
- `envelope_next_valid` (§5) machine-checks every emitted `next`/`forward`/
  `reverse` against the target tool's schema — guidance is *executable or
  rejected*, never prose that rots.
- Skills describing tools go stale the day a tool changes; here the
  description, the schema, and the navigation are one artifact. This is the
  file-size-lint philosophy applied to instruction: standards that can't be
  checked are opinions.

## 5. Tests (into 15 §5; gates on CP-6)

| Test | Asserts |
|---|---|
| `map_reachability` | from `map()` global, every tool is reachable via forward edges (no orphan tools) |
| `map_reverse_totality` | every mutating tool has ≥1 reverse edge or an explicit `irreversible: true` with compensating guidance (events) |
| `envelope_next_valid` | every `next`/`forward`/`reverse` emitted in the full integration suite validates against the target tool's schema |
| `map_state_table` | data-driven: (manifest, funnel) fixtures → expected `here`/top-forward, incl. minted-unresolved → handoff line first |
| `one_call_mint` | `mint {target}` alone succeeds: defaults applied, catch-all synthesized, handoff line in `next[0]` |
| `hint_on_every_error` | every error variant across all tools carries a non-empty, fix-naming `hint` (exhaustive enum walk) |

## 6. The competitive bar this exists to clear

The incumbent for local handoffs is not a product; it is **the file path**
(01 §2). A path is one reflexive act with zero new concepts. Waggle wins
only if the first call is as cheap (`one_call_mint`) and the *second*
interaction pays visibly (the handoff line arrives unprompted; `funnel`
shows what the orchestrator could never see; revoke/supersede fix what a
stale path silently corrupts). The fluent surface is how that value
advertises itself without a skill, a doc, or a demo — the tools are the
onboarding.
