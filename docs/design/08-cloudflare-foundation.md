# 08 — The Cloudflare Foundation

*The first hosted deployment of the storage contract. This design borrows a
production-proven assembly line: rote's telemetry infrastructure runs this
exact pattern (Rust→wasm workers, Queues → R2 NDJSON → Parquet compaction,
KV caching, per-env wrangler config, Miniflare-based integration tests).*

> **Revision 2 note.** Wasm here is a **build target of this deployment**,
> nothing more — the retired "bindings" story does not pass through this doc.
> One addition: the edge worker also exposes waggle as a **remote MCP server**
> (`/mcp` route, streamable-HTTP transport) so hosted teams get the same tool
> triplet (`mint`/`resolve`/`record`, optional `share`/`funnel`) that
> `waggle serve --stdio` gives a laptop — one interface, two radii. Auth for
> remote MCP rides the same per-tenant key scheme as `/api/mint` (§5).

## 1. Shape: two workers + one cron host

```text
                 ┌─ waggle-edge (worker, Rust→wasm via `worker` crate) ──────┐
POST /api/mint ──▶ auth → core::mint() → append Minted (Queue) → KV put     │
                 │   → SharePackage JSON (short_url, artifact, qr url)      │
GET  /x/{t} ─────▶ KV get manifest → core::negotiate(UA) → core::resolve()  │
                 │   bot → OG html · human → 301 · terminal → text card     │
                 │   + enqueue Event            (never blocks the response) │
POST /x/{t}/resolve ▶ body: A2A card → extractor → variant projection JSON  │
                 │   + enqueue Event{actor: Agent{family,harness}}          │
POST /api/events ▶ host-reported downstream stages (auth'd) → enqueue       │
GET  /x/{t}/qr.svg ▶ social::package() QR, Cache API, immutable            │
GET  /api/funnel/{t} ▶ AE hint (fast) | R2 fold (exact, ?exact=1)          │
GET  /api/manifest/{t} ▶ the retrievable attribution manifest (public)      │
                 └──────────────────────────────────────────────────────────┘
                          │ Queue (at-least-once)
                 ┌─ waggle-sink (queue consumer worker) ─────────────────────┐
                 │ batch → dedupe (token,nonce) in KV window → assign seq    │
                 │ → R2 append NDJSON (raw truth) → AE writeDataPoint        │
                 │ → KV view/materialization updates + cache invalidation    │
                 └──────────────────────────────────────────────────────────┘
                          │ nightly cron (native binary, CI-run)
                 ┌─ waggle-compact ──────────────────────────────────────────┐
                 │ R2 raw NDJSON → monthly Parquet (03 §4 schema) → R2       │
                 │ two-phase manifest commit · GC raw beyond retention       │
                 └───────────────────────────────────────────────────────────┘
```

## 2. Binding roles (and the v1 simplification)

| Binding | Role | v1? |
|---|---|---|
| **KV** | token → manifest view (read-hot); dedupe window; short-lived caches | ✅ |
| **Queues** | the append path; decouples redirects from durability | ✅ |
| **R2** | the log: raw NDJSON + compacted Parquet — the reconstruct substrate | ✅ |
| **Analytics Engine** | approximate real-time counters (`funnel_hint`) — powers live dashboards/conference walls | ✅ |
| **D1** | relational views (`tokens_for_target`, sharer reports) as data grows | v2 — v1 keeps these as KV secondary indexes with documented limits |
| **Rate-limit binding** | mint + event ingestion abuse control | ✅ (same binding pattern as rote's ingest worker) |
| **Cache API** | OG pages, QR SVGs, text cards | ✅ |

**Consistency, revised (rev 2.1 — the C1/C2 scenarios in 15 §3 broke the
rev-2 story):**

- **A KV miss is never authoritative (C-10 / G-7).** `GET /x/{t}` and
  `resolve` read-through to the system of record (D1/R2 view) on KV miss
  before returning `UnknownToken`, then warm KV; negative caching only after
  an authoritative miss. This is what preserves read-your-mint for the
  *recipient of a handoff* (mint in Frankfurt, subagent resolves in Oregon
  200 ms later) — the rev-2 text only covered the minter itself.
- **Two write paths (G-4/C3).** Lifecycle mutations (`Revoked`, `Superseded`,
  `ExpirySet`) and `mint`/`mint_child` go **directly to the origin store with
  CAS** (C-9) — never through the fire-and-forget queue, or C-7/C-9 would be
  unenforceable under cross-PoP interleaving. Events (commutative counts)
  stay on the queue.
- **Resolve consistency levels (G-8).** `resolve` accepts
  `consistency: eventual | strict`. `eventual` (default) serves KV within a
  declared staleness bound (seconds, global); `strict` read-throughs to the
  origin — and a manifest MAY mandate strict for sensitive variants, which
  overrides the caller. The strict-vs-eventual latency delta is a published
  bench (15 §5.4), not a footnote.

## 3. The sans-I/O payoff, concretely

`waggle-edge` contains **zero domain logic** — it is glue: parse request →
call `core::negotiate`/`core::resolve`/`social::package` (pure) → do I/O via
bindings. Entropy = `crypto.getRandomValues` through the worker API; time =
`Date.now()` per request. The exact same core functions run in the `fs`
backend's CLI. This is the architecture's proof moment: if the worker needs a
core change to work, 03 failed.

## 4. Caching and invalidation

- Manifest views in KV with generous TTL **plus** event-driven invalidation:
  the sink deletes/rewrites the KV entry when it ingests a mutation for that
  token (rote's debounced card-invalidation pattern).
- OG HTML and QR SVG in Cache API keyed by `(token, manifest_version)` —
  version-keyed means invalidation is free (new version = new key).
- Live counters (the conference wall) read AE with ~seconds staleness;
  `?exact=1` paths fold from R2 and are rate-limited.

## 5. Security posture

- **Mint/event auth v1**: per-tenant API keys (hashed at rest in KV);
  JWT/JWKS verification is the drop-in upgrade (the verification module
  pattern exists in rote's ingest worker and lifts directly).
- `GET /x/{t}` and `GET /api/manifest/{t}`: public by design (disclosure is
  the product); private tokens **[open]** for v2 (likely capability-suffixed
  URLs, mirroring the origin design's expiring share links).
- Per-IP rate limits on mint and resolve; **venue-NAT allowance** (an event
  token that raises limits for a declared window) — the conference lesson
  from the origin design, built in from day one.
- No cookies, no fingerprinting, no IP retention beyond the rate-limit
  window. UA → `ActorClass` classification happens at the edge and only the
  class survives.

## 6. Testing and deployment

- **Miniflare-based integration tests** driving the real workers with real
  bindings emulation (queue linkage between edge and sink in one Miniflare
  instance) — the harness pattern exists in rote (`CfPipelineHarness`, node
  bridge over wrangler's programmatic API) and is the model.
- Conformance suite (07 §5) runs against the Cloudflare store in CI via that
  harness — the edge backend is held to the same R-1..R-4 as the filesystem.
- Wrangler envs: `staging` auto-deploys on main; `production` gated. Bundle
  budget check in CI (workers have a 1 MiB gzipped class of limits — the
  sans-I/O core keeps us far under it).

## 7. Cost intuition (order-of-magnitude, not a quote)

The hot path is KV read + 301 + queue enqueue — single-digit-millisecond CPU,
pennies per million at Workers pricing tiers. R2 storage of 19-byte-ish rows
(NDJSON inflated, Parquet compressed) is negligible until billions of events.
The design has no per-request database dependency, which is both the latency
story and the cost story.
