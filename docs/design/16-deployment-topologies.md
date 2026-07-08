# 16 — Deployment Topologies: Laptop to Edge

*New in revision 2.2. The three tiers, the one-line bootstrap, the daemon/
shim architecture, and replay-as-migration. Answers: "can this operate
locally with any agent harness via a simple insertion bootstrap, then extend
to cloud for cross-agent, cross-harness?" — yes, by construction.*

## 1. The three tiers

```text
Tier 1 · SOLO      waggled (auto-started), per-user store       one agent's subagents
Tier 2 · MACHINE   the same waggled, shared by every harness    cross-harness, one laptop
                   on the machine                               — no cloud required
Tier 3 · CLOUD     waggle-serve on Workers, remote /mcp         cross-agent · cross-harness
                                                                · cross-machine · humans
                                                                via links (06 §7 scenario B)
```

Moving up a tier is a config change plus (for tier 3) one replay command —
never a rewrite, never a data migration project.

## 2. The bootstrap (one binary, one line, zero accounts)

```bash
cargo install waggle-cli          # or the curl installer / brew

# Claude Code — HTTP transport (preferred; talks to the daemon directly)
claude mcp add --transport http waggle http://127.0.0.1:7411/mcp

# Claude Code — stdio (compatibility; the shim auto-starts the daemon)
claude mcp add waggle -- waggle serve --stdio

# Codex (~/.codex/config.toml)
[mcp_servers.waggle]
command = "waggle"
args = ["serve", "--stdio"]

# any MCP harness (.mcp.json)
{ "mcpServers": { "waggle": { "command": "waggle", "args": ["serve", "--stdio"] } } }
```

The store self-initializes (`~/.waggle/waggle.db`, SQLite WAL — 07 §4). From
that moment the agent has `mint / resolve / record / funnel / query` as
tools; scenario A (06 §7) works offline on a plane.

## 3. The daemon/shim architecture (rev 2.2 — why stdio stopped being scary)

stdio's problem was never the bytes; it was the **process model**: a server
spawned per client, lifecycle owned by the harness, colliding on the store
lock when a second harness starts. Inverted:

- **`waggled`** — the local daemon: tokio async, streamable-HTTP MCP bound to
  127.0.0.1:7411, sole owner of the SQLite store, serving N concurrent
  clients (Claude Code + Codex + a Python script, simultaneously — tier 2
  is just tier 1 with more clients). Auto-started on first use; idle
  shutdown optional; observable like any daemon.
- **`waggle serve --stdio`** — a ~thin proxy shim for harnesses that only
  spawn stdio servers: detects a running daemon (starts one if absent) and
  forwards frames. Adds transport, never semantics — a conformance
  assertion, not a hope.

## 4. Replay-as-migration (the event-sourcing dividend)

```bash
waggle export                      # LogRecords as JSONL (the wire format)
waggle replay --to https://wag.acme.dev   # stream them into the cloud store
```

The log is the migration format. Properties we specified for other reasons
make migration boringly safe: **idempotent append** (C-4/C-8) makes replay
retry-safe if it dies halfway; **determinism** (R-1) guarantees the cloud
state equals the local state; tokens survive unchanged because the token,
not the URL, is identity. **Blobs migrate by content address** (rev 2.3):
replay walks live MediaRefs and syncs `~/.waggle/blobs/<sha256>` → R2 by
hash — deduplicated (same hash never uploads twice), resumable (hash =
progress marker), and verifiable (the manifest's sha256 is the receipt).
MediaRef URIs re-render to the tier-3 host; the hashes never change. Then each teammate's harness swaps its config to
the remote URL — same tokens, now resolving for Codex agents elsewhere, A2A
consumers via Artifact URLs, and humans via unfurls, with G-7 read-through
preserving read-your-mint across regions.

Tiers can also **coexist**: a laptop daemon for private work, the team cloud
for shared tokens — the client config lists both; namespacing across hosts
is deliberate future work (cross-host lineage, 06 §8 **[open]**).

## 5. Honest boundaries & local security (rev 2.4 — audit F-2/F-4)

- **URL scoping**: short *URLs* rendered in tiers 1–2 are localhost-scoped;
  tokens migrate perfectly, but a URL printed on a slide during the laptop
  era resolves globally only after tier 3 exists. Manifests are
  host-independent; renderings aren't. (Say it in the quickstart.)
- **Single writer per store**: cross-process writes always go through the
  owning daemon (WAL makes cross-process *reads* safe — a rev-2.2 bonus).
- **Local daemon auth (F-2)**: loopback TCP is reachable by every local user
  and process, so it is *not* the trust boundary. Default transport on
  macOS/Linux is a **Unix domain socket** (`~/.waggle/waggled.sock`,
  dir mode 0700); the TCP listener (Windows, and clients that require it)
  requires a **bearer token** from `~/.waggle/daemon.token` (0600,
  generated at first start; the shim reads it automatically — zero user
  ceremony). Gate: `it_local_auth` — an unauthenticated TCP client is
  refused; socket permissions verified.
- **Version handshake (F-4)**: the daemon advertises `{waggle_version,
  schema_version}` at MCP initialize; a newer shim/CLI triggers a graceful
  daemon drain-and-restart (finish in-flight commits, swap binary, resume) —
  an upgrade never leaves a stale daemon serving a new client. Gate:
  `it_version_skew` — old-daemon/new-shim converges to the new version with
  zero lost acked writes.
- **Tier-3 auth**: the moment a store leaves the machine, per-tenant keys
  apply (08 §5).

## 6. Checkpoint hooks

Tier 1–2 land in **CP-6** (daemon + shim + auto-start + idle lifecycle, with
an integration test: two simulated harness clients sharing one daemon).
`export`/`replay` land in **CP-6** (local) and are re-verified against the
cloud store in **CP-10** (`it_replay_migration`: local journal → cloud →
reconstruct ≡, using C-8 dedupe under an injected mid-replay kill).


## Appendix · Daemon lifecycle (rev 2.8): no orphans, by design

`waggled` gains explicit management — `waggle daemon <status|start|stop|restart>`
— plus three mechanisms that make lingering orphans structurally unlikely:

1. **A pidfile beside the socket** (`waggled.pid`): written at bind,
   removed at shutdown. `status` and `stop` consult both — a live socket
   answers over RPC; a dead socket with a live pidfile is diagnosed as an
   orphan and `stop` terminates it by pid.
2. **Graceful shutdown over the socket**: `stop` sends the daemon-level
   `waggled/shutdown` method (intercepted before the MCP dispatcher —
   management is not a tool agents see); the daemon replies, cleans its
   socket and pidfile, and exits. Durability is unaffected: every acked
   write already survived (`synchronous=FULL`).
3. **Idle exit**: `WAGGLE_IDLE_SECS` (shim auto-starts set 1800 unless
   overridden) — a daemon with zero connections and no activity for the
   window cleans up and exits. The next shim start revives it. A daemon
   nobody is using does not outlive its usefulness.

`status` reports pid, version, store path, socket path, uptime, and
active connections — the observability half of "no orphans". `start` is
idempotent (already-running reports the pid and exits 0); `restart` is
stop-then-start with the pid change shown.

4. **`purge` (rev 2.9)** — the last resort `stop` structurally cannot
   be: daemons whose socket AND pidfile were deleted out from under
   them (crashed tests, swept temp dirs) are findable only by process
   table. `purge` pgreps every `waggle serve --daemon` owned by the
   user, TERMs, escalates to KILL after a grace period, and reports
   `{purged, count, needed_sigkill}`. Tested against manufactured
   zombies (state dirs removed from under live daemons).
