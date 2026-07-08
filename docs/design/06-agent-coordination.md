# 06 — Agent-to-Agent Coordination

*Revision 2. The wedge. A waggle token is a stigmergic marker whose
interpretation travels with it: agents coordinate through shared tokens, and
each agent receives the projection matched to what it is. Changes in this
revision: consumption is MCP-first (no waggle code in any harness); the A2A
Agent Card is reframed from "the adopted schema" to **one adapter among
several** at the extractor seam; unverified ecosystem statistics are replaced
with the adversarially-verified chain (see 12); the worked walkthrough (§7)
is new; open question #2 (variant-on-Event) is settled — adopted.*

## 1. Resolver context: one neutral schema, adapters at the edge

Waggle's own `ResolverContext` (model_family · harness · modalities ·
posture) is the lingua franca. Nothing upstream owns it. Context arrives
through a **pluggable extractor**:

```rust
pub trait ContextExtractor {
    fn extract(&self, input: &CardInput) -> Result<ResolverContext, ExtractError>;
}
// CardInput ::= HarnessMeta(serde_json::Value)   — Claude Code, Codex, … metadata
//             | A2aCard(serde_json::Value)       — signed Agent Card (v1.0)
//             | Explicit(ResolverContext)        — bare JSON, any caller
//             | UserAgent(&str)                  — humans/bots/terminals
```

Default extractors ship for: **harness metadata** (what a Claude Code or
Codex subagent can state about itself — model family, tools available,
attended/headless), **A2A Agent Cards** (fields + the `x-waggle/*` extension
namespace for dims A2A doesn't standardize), and **explicit context**. If any
schema drifts, only its extractor changes — the sealed selection algorithm
(I-2) is untouched. Card *signature* verification (A2A v1.0 signed cards) is
the v0.3 gate for attributed resolution; anonymous resolution never needs it.

**Independence stance (ruling):** waggle depends on none of these. The
primary consumption path is the MCP tool triplet — usable today from every
MCP-speaking harness — plus plain HTTPS. A2A compatibility (tokens in
Artifact URL Parts, card extraction) is a thin call option that costs one
adapter and pays off if A2A's usage curve catches its endorsement curve
(01 §2).

## 2. Variant selection: the deterministic algorithm (normative)

A `Variant` declares a `MatchExpr` — a conjunction over the four dimensions,
each unconstrained or an allow-set:

```text
MatchExpr { model_family: Any | OneOf(set),
            harness:      Any | OneOf(set),
            modalities:   Any | Superset(required: ModalitySet),
            posture:      Any | OneOf(set) }
```

Selection, precisely:

1. A variant **matches** iff every constrained dimension accepts the context.
2. **Specificity** = number of constrained dimensions (0–4).
3. Highest specificity wins; ties break by **declaration order**.
4. A manifest MUST carry a catch-all variant (all `Any`) — validated at mint —
   so selection is **total**.

Pure, total, deterministic, dumb on purpose: no scoring weights, no regex, no
probabilistic matching — each would make "same context → same projection"
unauditable. Selection is sealed in code (03 §3); expressiveness grows by
adding *dimensions* to the data model, a visible, versioned act. The test
suite pins worked tables of ties and near-misses, because that's where
implementations rot. Every resolve event records **which variant served**
(`Event.variant`, 02) — manifest-referencing, I-1-compatible, and the raw
material for §6's feedback loop.

**Multimodal variants (rev 2.3)** are the matcher's best demo — one token
for "the whiteboard photo from the design session":

```text
variant 0  match {modalities ⊇ vision}   → MediaRef: the image (CAS, sha256)
variant 1  match {modalities ⊇ audio}    → MediaRef: the voice note
variant 2  match {Any}   (catch-all)     → inline: transcript + alt-text, 3 KB
```

The vision-capable subagent gets the image; the audio agent gets the voice
note; the small text-only model gets the transcript — deterministically, with
attribution and per-variant funnel telemetry on all three. Bytes live in the
content-addressed store (02 MediaRef, 07 §4); resolution returns URL + hash,
fetched out-of-band. No harness offers "the right modality of the same
artifact per agent."

## 3. Why this matters, in verified numbers

The case rests on first-party, adversarially-verified evidence (12):

- Multi-agent systems consume **~15× the tokens of chat** and **3–10× more
  than single agents**, with the overhead vendor-attributed to *"duplicating
  context across agents, coordination messages between agents, and
  summarizing results for handoffs"* — **"Each handoff loses context"**
  (Anthropic, yes 3–0).
- **~36.9% of multi-agent failures** trace to inter-agent misalignment /
  context loss at handoffs (MAST taxonomy, yes).
- Shared-artifact coordination cut tokens **43–72%** vs. message-forwarding
  with better quality (LbMAS blackboard, yes medium — preprint); simulated
  reference/invalidation schemes suggest more (yes low — simulation).

What a token changes mechanically: the orchestrator shares **one ~30-byte
reference**, not N tailored context dumps; each subagent pulls **its own
projection at need** (outside the context window until required); and the
funnel attributes success/failure per variant per model family — a feedback
loop none of the coordination literature has.

## 4. Lineage: the delegation forest

Delegation re-mints a **child token** (`channel: subagent/<role>`, parent in
the immutable core). The forest is three things at once:

- **Coordination trace** — who handed what to whom, reconstructable (04).
- **Attribution graph** — funnels roll up child→parent; "did the research
  swarm's work get used?" is a lineage query.
- **Revocation cascade** — revoking a node tombstones its subtree.
  **Contract clause (new in rev 2, closing the judged race):** a
  `mint_child` whose parent carries `revoked_at` MUST fail
  (`MintError::ParentRevoked`), and the cascade walk re-checks children
  minted concurrently — the conformance suite (07 §5) includes the
  revoke-vs-mint-child race.

Depth bounded at 16 (**settled** from rev-1's open list — generous for real
orchestration, finite for abuse).

## 5. Trust and privacy posture

- **Anonymous by default.** Resolution records coarse `ActorClass` dims only
  (family + harness class, never versions or instance IDs) — I-7.
- **Attributed resolution is opt-in** (v0.3): signed Agent Card verification
  recorded alongside, not inside, the event log.
- **Manifests are signable** (schema field + canonical serialization reserved
  now; Ed25519 detached signatures v0.3).
- **Variants are content, never code**; the consuming product's own gates
  remain the enforcement boundary.

## 6. The authoring feedback loop

`resolve(codex) high, run(codex) low` is a *variant bug report*, per model
family, for free — the author learns which representation fails which
consumer class. This is the telemetry loop skill/prompt authors and model
vendors both lack today, and it falls out of `Event.variant` + the funnel
fold; no new machinery.

## 7. The worked walkthrough (normative narrative)

### Scenario A — within one harness (Claude Code, orchestrator + subagents)

Setup: `waggle serve --stdio` registered once as an MCP server in the harness
config; fs backend (`~/.waggle`, JSONL). Zero infrastructure.

1. **Work happens.** The lead agent produces a 9,000-token market-analysis
   artifact (a file in the workspace).
2. **Mint, not paste.** Lead calls the `mint` tool: target = the artifact's
   URI; sharer `lead`; channel `subagent/pricing`; variants — (a)
   `{claude}` → section index + analysis-guidance body, (b)
   `{modalities ⊉ browser}`/small-context → executive summary only, (c)
   `{posture: headless|ci}` → fail-closed instructions, (d) catch-all.
   Returns `wg:7Kp2…` — ~30 bytes.
3. **Handoff is a reference.** The pricing subagent's prompt contains the
   token and one sentence ("resolve via waggle for your working context") —
   not the 9k-token document. Anthropic's "each handoff loses context"
   failure mode is attacked at the root: nothing is summarized away, because
   nothing is forwarded.
4. **Each consumer gets its shape.** The subagent calls `resolve` with its
   harness metadata; the sealed matcher returns variant (a). A cheaper
   verification subagent spawned later — different model family, small
   context — resolves the *same token* and receives variant (b).
   Deterministically; auditable later to the byte (04's time travel).
5. **Delegation deepens.** The pricing subagent delegates a data check:
   `mint` with `child_of wg:7Kp2`, channel `subagent/data-check` — the
   lineage forest grows; revoking the root would tombstone it.
6. **Observability for free.** Each resolve/run lands in the log. The lead
   ends with `funnel wg:7Kp2`: which roles resolved, which variant each got,
   which completed. A failed role is visible *as a funnel stage*, not as a
   mystery inside a transcript.

### Scenario B — across harnesses (Claude Code → human channel → Codex → A2A)

Same token; the hosted shape (08) or a team-shared fs server gives it an
HTTPS form: `https://wag.acme.dev/x/7Kp2`.

1. **The bridge is human.** The lead's operator drops the link in Slack.
   Slackbot's fetch gets OG meta from mint-time snapshot metadata
   (impression · bot recorded); a teammate reads a truthful unfurl.
2. **A different vendor's agent consumes it.** The teammate pastes the link
   into Codex. Codex's waggle MCP connection (remote) — or a bare HTTPS
   resolve — presents `{model_family: gpt, harness: codex, …}`; the matcher
   serves the Codex-shaped variant the author attached (tool-mapping
   phrasing, different guidance). No waggle code in Codex; no A2A anywhere.
3. **An A2A agent, if one shows up.** Somewhere else the token URL rides in
   an A2A Artifact Part's `url` field. The receiving agent POSTs its signed
   Agent Card to `/x/7Kp2/resolve`; the extractor maps card → context; it
   gets *its* projection; the event records `agent · gpt-family · a2a` —
   coarse dims only.
4. **One log, one story.** The funnel now spans harnesses and vendors:
   minted by `lead` in Claude Code, resolved by a Codex teammate, resolved by
   an A2A agent — one lineage, exactly reconstructable. A repair
   (`superseded_by`) propagates: late resolvers get the pointer to the fixed
   artifact. A revocation kills the Slack link, the Codex path, and the A2A
   path in the same act.

The two scenarios are the same protocol at two radii — which is the design's
core claim made concrete: **coordination within a harness and distribution
across harnesses are one primitive**, differing only in where the resolver
runs.

## 8. Honest unknowns

- **[open]** Variant body typing: `content_type: CompactString + bytes` in
  v0.1; typed registry later.
- **[open]** Cross-host lineage (child minted on a different waggle host than
  its parent) — v2+, requires federated naming; deliberately deferred.
- **[open]** Whether harness vendors expose enough self-metadata for rich
  `HarnessMeta` extraction everywhere; where they don't, `Explicit` context
  is the documented fallback.
