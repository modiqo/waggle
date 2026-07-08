# Five minutes to your first handoff

You'll mint a token for a file, resolve it the way a consumer would, report
work against it, and read the attribution back. Everything below is real
output — copy the commands, you'll see the same shapes.

## Install

Prerequisites: a Rust toolchain (`rustup`, stable ≥ 1.85) and
[`just`](https://github.com/casey/just) (`cargo install just` or
`brew install just`).

```bash
git clone https://github.com/modiqo/waggle && cd waggle
just dev-install        # cargo install --path crates/waggle-cli
waggle --version        # sanity check
```

Then, in any repo where agents will work:

```bash
waggle init             # installs the 5-line agent stub into CLAUDE.md/AGENTS.md/.cursorrules
```

The store lives at `~/.waggle/waggle.db` (SQLite, WAL). Two knobs, both
optional: `WAGGLE_STORE` moves it, `WAGGLE_SHARER` names your session in
attribution (defaults to `session`).

## 1 · Mint

Turn an artifact — any file, workspace URI, or URL — into an attributed
reference:

```bash
$ waggle mint --target "file:///Users/you/findings/market-report.md"
{
  "result": {
    "token": "b2uQyZUC",
    "handoff": "resolve b2uQyZUC via waggle for your working context",
    "replayed": false,
    "variants": 1
  },
  "next": [
    { "tool": "resolve", "args": { "token": "b2uQyZUC" },
      "why": "self-check the projection consumers will receive" },
    { "tool": "map", "args": { "token": "b2uQyZUC" },
      "why": "orient around the new token" }
  ],
  "stats": { "records": 1, "seq": 0 }
}
```

Three things to notice, because every waggle response has them:

- **`result.handoff`** is the whole point: one sentence you (or an
  orchestrating agent) paste to a teammate instead of the artifact's
  contents. The token is 8 characters. The report can be 40 pages.
- **`next`** is not documentation — each entry is a ready-to-execute call.
  Agents follow these instead of memorizing a manual.
- **`stats`** tells you what the call cost and touched.

## 2 · Resolve — what the consumer sees

```bash
$ waggle resolve --token b2uQyZUC
{
  "result": {
    "disposition": "active",
    "variant": 0,
    "body": {
      "inline": {
        "content_type": "text/markdown",
        "data": "Fetch the artifact at file:///Users/you/findings/market-report.md and use it as your working context."
      }
    },
    "target": "file:///Users/you/findings/market-report.md",
    "as_of": 1783493617452,
    "revalidate_after": 1783494517452
  },
  ...
}
```

With no variants declared, mint synthesized a **catch-all**: a plain
instruction pointing at the target. (Declaring per-consumer variants — a
Claude-tuned body, an image for vision agents — is
[tutorial 3](03-variants-and-media.md).)

`as_of` and `revalidate_after` matter more than they look: a resolution is
**knowledge, not a lease**. The consumer knows exactly when it learned
this and when to re-check — which is how revocation actually works in a
distributed handoff.

Also: that resolve was **recorded**. You didn't ask; attribution isn't
optional bookkeeping, it's what the token is for.

## 3 · Report work

```bash
$ waggle record --token b2uQyZUC --stage run
{ "result": { "recorded": "run", "token": "b2uQyZUC" }, ... }
```

Stages are the funnel vocabulary: `impression → click → resolve → assess →
consent → install → signin → credential → run → repeat` — or any custom
kebab-case slug. Events are **counts with no payload** (invariant I-1):
nothing about your artifact's content enters the analytics, by type-system
construction.

## 4 · Read the attribution

```bash
$ waggle funnel --token b2uQyZUC
{
  "result": {
    "token": "b2uQyZUC",
    "stages": { "resolve": 1, "run": 1 },
    "children": []
  },
  ...
}
```

One consumer resolved, one execution happened. When you mint child tokens
(`--parent`), delegation forms a tree here.

## 5 · Orient — the map

Lost? Never read a manual; ask where you are:

```bash
$ waggle map --token b2uQyZUC
{
  "result": {
    "here": "b2uQyZUC — active · 1 variant(s) · 1 resolve(s) · 1 run(s) · 0 child(ren)",
    "reverse": [
      { "tool": "mutate",
        "args": { "token": "b2uQyZUC", "change": "revoke", "expected-version": 1 },
        "why": "withdraw — children tombstone with it" },
      { "tool": "mutate",
        "args": { "token": "b2uQyZUC", "change": "supersede=<new-token>", "expected-version": 1 },
        "why": "replace with a corrected artifact; late resolvers follow the pointer" }
    ],
    "irreversible": { "events": "history does not un-happen — record a correcting stage instead" }
  },
  "next": [ ...ranked forward paths... ]
}
```

`here` is computed from the manifest and funnel at call time — the map is
never stale, because it is never stored. Note the `reverse` entries carry
the current `expected-version`: lifecycle changes are compare-and-swap, and
the map hands you the correct baseline.

## Where next

- Wire it into your agent harness → [tutorial 2](02-claude-code.md)
- Different consumers, different projections → [tutorial 3](03-variants-and-media.md)
- Revoke, supersede, query slices, lineage → [tutorial 4](04-lifecycle-and-query.md)
- Use the crates directly from Rust → [tutorial 5](05-embedding-rust.md)
