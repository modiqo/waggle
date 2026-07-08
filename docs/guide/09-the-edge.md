# The edge — waggle on Cloudflare

The local tier gives every harness on one machine a shared substrate;
federation (guide 08) connects machines that can reach each other. The
**edge tier** removes the last constraint: tokens that resolve — and
grep — from anywhere, with the owner's laptop asleep.

The architecture in one sentence: **your `waggled`, relocated** — a
Durable Object per tenant is the same single-writer commit point,
running the same certified engine, behind the same MCP frames
(design doc 08 §8). Nothing about the agent's loop changes.

## Deploy (once, ~5 minutes)

Prerequisites: a Cloudflare account (free tier suffices — the DO uses
SQLite-backed classes, which are free) and `npx wrangler login`.

```bash
cd edge-worker

# 1 · the two bindings (skip if you don't need content search / unfurl cache)
npx wrangler kv namespace create CACHE     # paste the id into wrangler.toml
npx wrangler r2 bucket create waggle-blobs

# 2 · the tenant bearer — the key to /mcp and /store
openssl rand -hex 24 | tee /dev/tty | npx wrangler secret put TENANT_TOKEN

# 3 · ship it
npx wrangler deploy
# → https://waggle-edge.<your-subdomain>.workers.dev
```

Both bindings are optional and degrade gracefully: no R2 → content
search answers with a hint instead of matches; no KV → unfurls skip
caching. The worker never breaks for a missing binding.

## The `waggle edge` command

Configure once (env or flags):

```bash
export WAGGLE_EDGE_URL=https://waggle-edge.<you>.workers.dev
export WAGGLE_EDGE_BEARER=<the tenant token>
```

**`waggle edge status`** — is it up, is the bearer right, how many tools:

```json
{"url":"https://waggle-edge.….workers.dev","health":"ok","tools":9}
```

**`waggle edge push`** — replicate this machine's store to the edge:
every record (idempotent ingest — rerun anytime, duplicates are free by
C-4), plus the **snapshot blobs** manifests reference, uploaded
content-addressed so `read`/`search` answer at the edge for files that
never existed there:

```json
{"records_scanned":142,"records_new":142,"blobs_pushed":3,
 "hint":"rerun anytime — ingest is idempotent (C-4)"}
```

**`waggle edge smoke`** — the proof loop: mint → resolve → funnel on
the real deployment, one command.

## What the edge serves

| Route | Auth | What |
|---|---|---|
| `POST /mcp` | bearer | the full tool surface — any agent harness, any language, pointed straight at it |
| `POST /store` | bearer | the replication RPC (`waggle edge push`, certification) |
| `GET /t/:token` | public | the unfurl: OG meta from the mint snapshot (I-3), a link onward — **revoked tokens answer 410, never a stale page**; every render records an `impression` |
| `GET /health` | public | liveness |

Every unfurl impression, remote resolve, and edge-side `read` lands in
the token's funnel as counts (I-1 — never payloads, never patterns).

## Federating a daemon to the edge

A `waggled` can use the edge as its upstream, making remote tokens
resolve transparently for every local harness *and* the plain CLI:

```bash
export WAGGLE_UPSTREAM=http://…   # http today (Miniflare, LAN, tunnels)
export WAGGLE_UPSTREAM_TOKEN=<the tenant token>
waggle daemon restart
```

Honest limitation, tracked in the plan: the daemon's upstream client
speaks plain `http://` (fine for `wrangler dev`, LAN, and tunnels such
as `cloudflared`); TLS federation to the public `workers.dev` URL lands
with the trust tier (CP-11). Direct `/mcp` access over HTTPS — what a
remote agent harness or `waggle edge` uses — works today.

## What it costs, and how to leave

Free tier: the worker, the SQLite-backed Durable Object, KV (100k
reads/day), and R2 (10 GB) all fit it. There is no queue dependency
(events write inside the DO invocation; Queues are the deferred scale
upgrade). Teardown is one command — `npx wrangler delete` — and your
data was never *only* there: the log replays home the same way it
replayed up.

## The verification story (how we know it's complete)

The edge shipped against a published completeness matrix
([doc 08 §9](../design/08-cloudflare-foundation.md)): the engine passes
the same conformance suite as every backend, natively and over the
wire; a **differential oracle** replays identical operation sequences
against the edge and SQLite and demands byte-identical worlds; and the
three-tier chain — CLI → daemon → edge — runs as a test with strict
revocation biting end to end. The full matrix runs in CI on every push
(Miniflare — no account, no secrets), and the E13 row records the dated
manual smoke against the real deployment.
