# 21 — The resource projection: MCP resources, subscriptions, and why the verbs are tools

*Status: design + as-built in the same stroke. This document records a
decision the corpus had left implicit — why waggle's MCP surface is
tools-first — and commits the complement: a thin resource projection
with subscriptions, so lifecycle corrections travel as protocol-native
push. It follows docs 09/17 (one catalog, many projections) and doc 16
(transports).*

---

## 0 · The critique, and the ruling

The critique: *"waggle exposes everything as MCP tools; the protocol
says content belongs in resources."* Factually true — the server
advertised `capabilities: { tools: {} }` and nothing else. The ruling
this document commits: **the interrogation verbs are tools because
MCP's own control hierarchy says so; the passive faces of a token —
enumeration, plain reads, update push — become a resource projection.**
Both, not either.

## 1 · Why the verbs are tools (the recorded rationale)

MCP assigns primitives by *who controls them*: resources are
**application-controlled** (the host decides what enters context),
tools are **model-controlled** (the agent invokes them mid-reasoning).
Three properties of waggle's core loop make tools the semantically
correct primitive, not a convenience:

1. **Resolution is context-adaptive.** `resources/read` assumes a URI
   names stable content — clients may cache it. The sealed matcher
   (I-2) deliberately returns *different projections per consumer
   context* from one token. Same URI, different truthful renderings:
   the resource model's central caching assumption does not hold.
2. **Reads have consequences — that is the product.** Every resolve,
   read, and search appends a receipt event; the funnel exists because
   consumption is observable. Resources presume side-effect-free
   fetches; the catalog's `OpKind` classes (`Read` vs `RelaxedWrite`)
   state the truth the resource model cannot carry.
3. **Interrogation is parameterized.** `search --pattern`,
   `read --lines/--section/--symbol`, byte budgets, resolver contexts:
   resource templates parameterize URIs, not lens grammars.

And the agentic reality: the consumer that waggle exists for is the
*model* deciding mid-task to interrogate its handoff — the exact case
the tools primitive was designed for.

## 2 · What resources ARE right for

Two faces of a token are genuinely application-controlled:

- **Passive attachment.** A host listing artifacts for a user to pick,
  or attaching a token's projection to context *before* the model
  runs. That is `resources/list` + `resources/read`.
- **Update push.** "The correction reaches every holder" has been a
  pull (re-resolve within `revalidate_after`). MCP's
  `resources/subscribe` → `notifications/resources/updated` is the
  protocol-native push for exactly this: a holder subscribes to its
  handoff; when the author revokes or supersedes, the notification
  arrives on the holder's connection — the freshness contract (spec
  §7) gains a proactive edge without changing a single semantic.

## 3 · The design

**One catalog, another projection** (09 §2): the resource surface adds
NO operations. It is a wire-level projection of `resolve` and the
manifest views — which is why `COMMANDS.md`, the CLI, and the map are
untouched.

| Method | Behavior |
|---|---|
| `initialize` | capabilities now `{ tools: {}, resources: { subscribe: true, listChanged: false } }` |
| `resources/templates/list` | one template: `waggle://{token}` — the URI scheme is the token, nothing more |
| `resources/list` | active, public tokens (revoked/expired/private excluded — private tokens are capability URLs and MUST NOT enumerate, spec §6), newest first, capped and saying so |
| `resources/read` | `waggle://TOKEN` → the token's projection via the SAME dispatcher the `resolve` tool uses — the resolve is recorded, the funnel stays honest, ancestor revocation cascades. Inline bodies serve as their own content type; everything else serves the resolution envelope as JSON |
| `resources/subscribe` / `unsubscribe` | per-connection subscription set; a **lifecycle** mutation (revoke, supersede, expiry — never cosmetic churn) on a subscribed token emits `notifications/resources/updated { uri }` |

**Sessions, not globals.** Subscriptions live in a `Session` value
owned by the transport's connection loop — the same place effects
already live. The rpc layer gains `handle_session(handler, &mut
Session, …) → { reply, notifications, lifecycle }`: the reply as
before, any frames due to *this* connection's subscriptions, and the
lifecycle-mutated token for the transport's hub. `handle_message`
(stateless) remains for transports without connections — and answers
`resources/subscribe` with an honest refusal.

**The daemon hub.** Cross-connection push — author revokes on one
connection, holder hears on another — is transport wiring, and it
lives where connections live: `waggled` carries a broadcast channel in
its shared state; each connection task selects over (socket line,
hub event); a lifecycle event whose token is in *this* session's
subscriptions writes the notification frame. The mutation's own
connection is served by `handle_session` directly; the hub serves
everyone else. Nothing new is stored: subscriptions are ephemeral
connection state, dropped on disconnect — exactly like the geometry
correlation state of 19 §4.4, and for the same reason (I-7: no
identity persists).

**The edge stays stateless, honestly.** The HTTP worker keeps
`handle_message`: list/templates/read work (reads are one-shot);
subscribe answers that subscriptions need a stateful connection at the
owner's daemon — the same graceful-degradation posture as tri-state
geometry flags. If edge push notifications are ever wanted, that is a
Durable-Object-alarms design, not a session hack.

## 4 · Invariants and budgets

- **I-1/I-7 untouched**: a resource read records the same payload-free
  resolve event as the tool; subscriptions never enter the log.
- **I-2 untouched**: `resources/read` presents the same negotiated
  default context the tool surface uses for context-less calls — same
  context, same projection.
- **Spec §6**: private tokens never enumerate; possession of the full
  URI remains the credential for reads.
- **Perf**: `resources/list` folds over `scan_all` like the global map
  does — O(records), capped output, fine for local stores; a
  materialized listing view is the recorded follow-up if hosts poll
  it. Subscribe/notify is O(subscriptions-per-connection) with one
  broadcast per lifecycle mutation — mutations are rare by design.
- **File discipline**: the projection is one new module
  (`resources.rs`) plus routing; the four-projection parity tests are
  unaffected because no catalog rows changed.

## 5 · Non-goals

- No resource-based interrogation (§1 — the verbs stay tools).
- No `listChanged` notifications (mints are frequent; a listing feed
  is telemetry, not a resource contract — revisit only with a real
  host use case).
- No persistent subscriptions: reconnect ⇒ resubscribe, the MCP norm.
- No prompts surface: the handoff line is already the envelope's
  teaching (17 §1); duplicating it as a prompt template adds a second
  place to rot.
