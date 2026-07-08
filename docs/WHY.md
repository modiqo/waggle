# Why waggle exists — the real state of the agent handoff

*Everything in this document is either mechanically verifiable (how the
APIs bill, what the frameworks do by default) or cited to a primary
source. The numbers survived an adversarial verification pass; the ones
that didn't are not here.*

---

## 1 · The mechanics nobody writes down

Start with the fact that shapes everything else: **LLM APIs are
stateless.** Every turn of a conversation re-sends the entire
conversation. When an agent pastes a 40 KB report into its context, it
does not pay for that report once — it pays for it on *every subsequent
turn* of that agent's life, because the whole history rides along each
time.

Now put a second agent in the room. The orchestrator receives the
researcher's report and forwards it to a writer subagent. The report now
lives in **two** context windows, each re-billing it per turn. Add a
fact-checker: three. This is not an implementation bug in any one
framework — it is the default shape of the ecosystem:

- **LangGraph**'s `create_handoff_tool` *"by default passes the full
  message history"* to the next agent (their documentation, not our
  paraphrase).
- **Anthropic**, engineering their own multi-agent research system,
  measured agents at ~**4×** the tokens of chat and multi-agent systems
  at ~**15×** — and wrote the sentence this project is built on: *"each
  handoff loses context."*
- The **MAST** failure taxonomy (Berkeley) attributes **36.9%** of
  multi-agent system failures to inter-agent misalignment — agents
  acting on divergent, stale, or partial copies of what should have been
  the same information.

Copies are the root defect. A copy has no identity, no version, no
owner, no lifecycle. The moment the researcher corrects the report,
every pasted copy is silently wrong, and no mechanism exists to find
them.

## 2 · The four boundaries — a grounded matrix

"Passing a resource between agents" is four different problems wearing
one name, depending on which boundary the resource crosses.

| Boundary | How it works today | What actually travels | Token & context impact | Lineage / attribution | Corrections propagate? |
|---|---|---|---|---|---|
| **Same harness, same model family** (Claude Code orchestrator → subagents) | Subagent's final message returns into the orchestrator's context; orchestrator pastes what the next subagent needs. Or: shared filesystem paths + convention. | The full text, again, per recipient. A file path travels cheaply — but carries no versioning, no access story, no receipts. | payload × recipients × every subsequent turn. Softened by provider caching *within its TTL* (§3). | None. The orchestrator's memory of "who made this" is prose, gone at compaction. | Manual re-paste. Stale copies persist silently in sibling contexts. |
| **Same harness, mixed families** (router/cascade setups: Claude plans, GPT executes) | Same paste — but now the payload crosses a **billing boundary**. | The full text, re-tokenized under a different tokenizer. | Full price on each side. No cache crosses vendors. Ever. | None. | None. |
| **Across harnesses, same machine** (Claude Code + Codex on one laptop) | Files on disk + convention files (`CLAUDE.md`, `AGENTS.md`); or a human copy-pastes between panes. | A path with no semantics, or the text itself. | Each harness re-ingests the whole file. Neither knows the other read it. | None — the filesystem records mtime, not meaning. | None. Delete the file and one side breaks silently; edit it and nobody re-reads. |
| **Across machines / organizations** | Slack, email, tickets, shared drives; A2A artifact URLs at the frontier. | Whole artifacts — or URLs whose **resolution semantics no standard defines** (A2A v1.0 standardizes artifact-by-URL and stops there). | The full payload per hop, then per-turn re-billing at every destination. | Whatever the messaging tool retains — divorced from the artifact. | Effectively impossible. The artifact has been photocopied across trust boundaries. |

Read the last two columns top to bottom: they are empty. That emptiness
is the product gap. The token cost is what everyone feels; the missing
lineage and the impossibility of correction are what actually break
swarms (MAST's 36.9% has a mailing address).

## 3 · Why a provider's optimization doesn't save you

Every provider is attacking the token cost — inside its own walls:

- **Anthropic prompt caching**: cached prefixes read at a fraction of
  base input price — with a **5-minute TTL**, within one API account, on
  byte-identical prefixes.
- **OpenAI automatic caching**: discounted cached prefixes — for OpenAI
  models, behind OpenAI's endpoint.
- **Harness compaction** (Claude Code and peers): summarizes a long
  context — a *lossy, local* fix whose summary is not a transferable
  artifact.

These are real savings and worth having. But notice their common shape:
**every one is scoped to a single vendor's billing and serving
boundary.** A cached Anthropic prefix means nothing at OpenAI's
tokenizer. A compaction summary in Claude Code does not exist in Codex.
The optimization **does not render across the boundary** — cross any
line in the matrix above and you pay full freight again, because what
crossed was a *copy of bytes*, and bytes have no identity a foreign
system could recognize.

This is the structural argument for waggle: portability cannot be an
optimization applied to copies. It has to be a property of the
**reference**. A ~30-byte token is the same 30 bytes in every context
window, every tokenizer, every vendor, every machine. What varies is
what it *resolves to* — and that is computed fresh, per consumer, where
the bytes live (the computation-travels-to-the-data contract, design
doc 08 §0).

## 4 · What the bees knew

A foraging bee returns from a field two kilometers out. She has
information worth sharing: where, how far, how good. Here is what she
does **not** do: she does not carry the field home. She does not carry
enough nectar for the colony to evaluate. She dances.

The **waggle dance** — von Frisch's figure-eight choreography — is a
~30-second encoded reference:

- the **angle** of the waggle run against vertical encodes direction
  relative to the sun;
- the **duration** encodes distance;
- the **vigor** and repetition encode quality — how hard this source is
  worth working.

And then the colony does something every distributed-systems engineer
should study. Each follower **resolves the reference herself** — flies
her own flight, with her own senses, from her own position. The dance
is not the nectar; it is an *attributed, resolvable claim* that the
nectar exists. Recruitment is **measurable**: you can count who
followed, who arrived, who came back and danced the same field in turn
— a recruitment tree, growing from one dance. And the information
**expires honestly**: bees dance only while the source still pays;
when the nectar dries up, the dancing stops, and the colony's attention
decays with it. No bee has to chase down stale copies of yesterday's
directions — there were never any copies to chase.

This is **stigmergy** — coordination through durable marks rather than
direct messages — and the mapping to waggle is not decorative:

| The dance | The token |
|---|---|
| figure-eight encodes vector + quality in seconds | ~30-byte token names artifact + attribution manifest |
| each follower flies her own flight | each consumer resolves *its* projection (sealed matcher: model family, modalities, posture) |
| the follower's senses at the field | `read`/`search`: interrogate the content surgically on arrival, never carry the field |
| countable recruitment | the funnel: resolve → read → run → repeat, as receipts |
| dancers who recruit dancers | lineage: children minted under parents — the delegation tree as data |
| dancing stops when nectar stops | `revalidate_after`, `supersede`, `revoke` — freshness and correction travel through the reference |
| the dance floor | the append-only log: the shared medium every mark lands on, replayable by anyone |

The colony solved the multi-agent handoff problem with zero shared
context windows: **share names, not payloads; let consumers resolve per
their own capability; make consumption observable; let stale claims
die at the source.** Waggle is that choreography, made durable and
queryable.

## 5 · The paradigm, stated plainly

Three ways exist to move information between computational actors:

1. **Copy semantics** (message passing): send the bytes. Simple, and
   every pathology in §1 follows — n copies, no identity, corrections
   don't propagate. This is today's default.
2. **Place semantics** (shared memory): both parties touch one location.
   Fixes duplication, but requires shared infrastructure and trust, and
   a raw location says nothing about *who may see what* — every reader
   gets the same bytes regardless of what it is.
3. **Name semantics** (references): send a *claim* — small, immutable,
   attributed — and let the resolution be computed per consumer, at the
   data, on demand.

Waggle is a commitment to the third, with four design consequences:

- **Information exchange becomes projection, not transmission.** A
  resolve answers with the variant matched to *this* consumer — the
  Claude-tuned digest, the image for the vision agent, the transcript
  for the one without ears, the fail-closed instructions for CI. One
  name, many truthful renderings; the sealed matcher guarantees the
  same context always gets the same projection.
- **Retrieval becomes interrogation.** `read` and `search` move the
  question to the bytes and return budgeted slices that *name the
  payload they spared you*. The consumer that needs three facts from a
  60-page report ingests a few hundred bytes, ever.
- **Lineage becomes data, not discipline.** `parent` at mint forms the
  delegation tree in the log itself — who handed what to whom is a
  query, not an archaeology project. Revoking a parent tombstones the
  branch.
- **History becomes reconstructable.** Every mint, resolve, read, and
  correction is an event in an append-only, payload-free log. Shuffle
  it, duplicate it, ship it to another machine: `reconstruct` rebuilds
  identical state, byte for byte. The swarm's coordination is not a
  memory any agent holds; it is a ledger anyone can re-derive — and the
  log stays dark about content (no payload field *exists*), so the
  receipts never become the leak.

## 6 · The arithmetic, honestly

One 40 KB report, five consumers, five turns each:

```
copy semantics:   40 KB × 5 contexts               =  200 KB placed
                  + re-sent in each of 5 turns × 5  ≈ 1.0 MB re-read
                  (provider caching may discount the re-reads —
                   inside one vendor, inside one TTL)

name semantics:   5 handoffs × ~30 B               =  150 bytes placed
                  each consumer pulls its projection once (≈2 KB summary,
                  or surgical slices at ~1 KB per question)
                  re-turns re-send the 30-byte line, not the report
```

The ratio is not the deep point, though it is large. The deep point is
in the *other* columns: under name semantics the author knows the
report was resolved five times, searched fourteen, run twice; the
correction reached the writer who resolved an hour early; and the whole
exchange replays on any machine. Under copy semantics, all of that is
not expensive — it is **nonexistent**.

---

*The bee never carries the field home. She dances, and the hive knows —
who danced, who flew, who found nectar, and when the field went dry.*
