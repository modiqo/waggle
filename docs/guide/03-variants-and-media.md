# Variants: one token, the right projection for each consumer

A token doesn't serve one blob to everyone. Its manifest holds
**variants** — different bodies for different consumers — and a **sealed,
deterministic matcher** picks per resolver context. Same context, same
projection, always: that's a guarantee agents can build on, not a router
you can misconfigure.

## Declaring variants

`mint` takes a `variants` array (MCP) — each entry is a `match` expression
plus a `body`:

```json
mint {
  "target": "ws://swarm/findings/market-report.md",
  "variants": [
    {
      "match": { "model_family": { "one-of": ["claude"] } },
      "body": { "inline": {
        "content_type": "text/markdown",
        "data": "# Claude-tuned guidance\nRead §3 first; use the table in §5 verbatim."
      } },
      "revalidate_after_ms": 60000
    }
  ]
}
```

A catch-all is synthesized automatically if you don't declare one — the
matcher is **total** over minted tokens; nobody ever resolves to nothing.

`match` can constrain four dimensions, all optional:

| Dimension | Example | Matches when |
|---|---|---|
| `model_family` | `{"one-of": ["claude"]}` | consumer declared that family |
| `harness` | `{"one-of": ["claude-code"]}` | consumer declared that harness |
| `modalities` | `8` (VISION) | consumer has **all** required modalities |
| `posture` | `["headless", "ci"]` | consumer's posture is in the set |

Selection is: most-specific match wins (count of constrained dimensions),
ties break by declaration order, catch-all last. An **undeclared** context
value fails a constrained dimension — a variant asking for `claude` never
serves an anonymous consumer.

Modality bits: `TEXT=1 · BROWSER=2 · SHELL=4 · VISION=8 · AUDIO=16`
(sum them: text+vision = `9`).

## Resolving with a context

Consumers present who they are; unknown consumers default to the safe
catch-all path:

```bash
waggle resolve --token 7Kp2mQ9x \
  --context '{"kind":"agent","model_family":"claude","modalities":9,"posture":"headless"}'
```

The response's `variant` field is the index that served — it's also what
gets recorded in the funnel, so you can later see *which projection*
actually got consumed (the authoring feedback loop).

## Media: images and voice ride content-addressed, never inline

```bash
waggle mint --target "ws://standup/whiteboard-discussion" \
  --attach ./whiteboard.png
```

The file is stored in a **content-addressed blob store**
(`~/.waggle/blobs/<sha>`, deduplicated, integrity-verified on read) and
the mint gains a variant automatically:

- consumers with **vision** resolve to a `MediaRef` —
  `{ uri: "blob://<sha256>", content_type: "image/png", size, sha256 }` —
  fetch the bytes out of band and verify the hash;
- everyone else falls through to the catch-all (your transcript or alt
  text — declare it as a variant or let the default point at the target).

`image/*` attachments serve vision consumers, `audio/*` serve listeners;
content type is inferred from the extension (`--attach-type` overrides).
A tampered blob fails its hash check at read — a resolver never trusts
bytes that don't match the manifest.

## Freshness

Each variant may carry `revalidate_after_ms`. Resolutions stamp
`as_of` + `revalidate_after`: "re-resolve before acting after this
instant." Short windows for sensitive artifacts, the 15-minute default
otherwise. This is the polite half of revocation — the strict half is in
[tutorial 4](04-lifecycle-and-query.md).
