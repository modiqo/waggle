# Running waggled — lifecycle, zombies, and federation

`waggled` is the single owner of your machine's waggle state: one
process, one SQLite store, one blob CAS, every harness connecting
through it. You rarely think about it — the shim auto-starts it — but
when you need to operate it, these are the controls.

## The lifecycle verbs

```bash
waggle daemon status     # {"pid":9016,"store":"…","uptime_secs":42,"active_connections":1}
waggle daemon start      # idempotent — already-running reports the pid
waggle daemon restart    # stop → start; fresh pid shown
waggle daemon stop       # graceful shutdown over the socket
waggle daemon purge      # the zombie killer (below)
```

`status` output is one JSON line on purpose: `waggle daemon status | jq .pid`.
Exit codes are honest: `status`/`stop` exit 1 when there's nothing
running.

### stop vs purge — the scalpel and the broom

`stop` reaches the daemon at your configured socket — and, via the
pidfile written beside it, an **orphan** whose socket died but whose
process lives (`status` diagnoses these explicitly). What `stop`
*cannot* reach is a daemon whose socket **and** pidfile were deleted out
from under it — crashed test runs, swept temp directories. For those:

```bash
waggle daemon purge
# {"purged":[8123,8140],"count":2,"needed_sigkill":[]}
```

`purge` finds every `waggle serve --daemon` you own (shims excluded),
TERMs them, escalates to KILL for survivors, and reports what it did.
Any stale socket files left behind are absorbed by the next start.

### Why zombies are rare in the first place

Three passive defenses run always: the daemon writes a pidfile at bind
and sweeps it at every exit path; concurrent auto-starts are race-safe
(the loser detects the live socket and exits 0); and **idle exit** —
`WAGGLE_IDLE_SECS` (shim auto-starts default to 1800) makes a daemon
with no connections and no activity clean up and leave. A daemon nobody
uses does not outlive its usefulness.

## The knobs (all env)

| Variable | Meaning | Default |
|---|---|---|
| `WAGGLE_STORE` | the SQLite store path | `~/.waggle/waggle.db` |
| `WAGGLE_SOCK` | the unix socket | `~/.waggle/waggled.sock` |
| `WAGGLE_SHARER` | your session's attribution identity | `session` |
| `WAGGLE_IDLE_SECS` | idle-exit window (0 = never) | none (1800 for auto-starts) |
| `WAGGLE_DIRECT` | CLI/stdio bypass the daemon entirely | off |

One guard worth knowing: the shim **verifies at connect** that the
daemon owns the store this session expects — a mismatch fails loudly
("store skew … run `waggle daemon restart`") instead of serving answers
from the wrong store.

## Federation: two machines, one handoff

The owner exposes its daemon on **token-gated TCP**; a peer points its
daemon upstream. That's the whole setup:

```bash
# OWNER machine (where artifacts live)
export WAGGLE_TCP=0.0.0.0:7411
export WAGGLE_TCP_TOKEN=$(openssl rand -hex 16)   # share this with the peer
waggle daemon restart

# PEER machine
export WAGGLE_UPSTREAM=owner-host:7411
export WAGGLE_UPSTREAM_TOKEN=<the same token>
waggle daemon restart
```

Now any token minted on the owner resolves on the peer — from agents
over MCP *and* from the plain CLI (both route through the daemon). The
rules of the road:

- **Computation stays at the owner** (design doc 08 §0): a peer's
  `search` runs where the bytes live; only matches travel. `read`,
  `query`, `funnel` likewise. The artifact never crosses.
- **`mint` never forwards** — you mint where you stand.
- **Events flow home**: a peer's `record` lands in the owner's funnel,
  so attribution stays whole (counts only, as always).
- **The daemon refuses to listen unauthenticated**: `WAGGLE_TCP`
  without a ≥16-char `WAGGLE_TCP_TOKEN` is an error, not a fallback. A
  connection with a bad bearer is dropped without a byte served.

### Freshness: strict vs eventual

Resolutions of remote tokens are cached at the peer, each entry
honoring **its own** `revalidate_after` stamp — never a made-up TTL:

```bash
waggle resolve --token 7Kp2mQ9x                   # eventual (default):
                                                  #   cache OK inside its window
waggle resolve --token 7Kp2mQ9x --level strict    # always revalidate at the
                                                  #   owner — revocations bite NOW
```

The trade, stated plainly: eventual may serve a resolution the owner
revoked seconds ago — but only inside the freshness window the *author*
chose at mint (`revalidate_after_ms` on the variant), and every
response carries its `as_of` so the consumer knows exactly what it
holds. `strict` costs a round trip and is never stale — and a strict
consult refreshes the cache, so subsequent eventual calls benefit. Use
strict before consequential/irreversible actions; eventual everywhere
else.

If a token is unreachable (no upstream configured, owner offline and
cache expired), the refusal names both fixes: configure the upstream,
or replay the owner's log locally.
