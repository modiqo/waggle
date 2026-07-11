<h1 align="center">
  <img src="docs/assets/logo.svg" width="52" alt="the waggle mark: a figure-eight dance with the waggle run as an arrow" align="center"> waggle
</h1>

<p align="center">
  <em>Tracked file paths for agents.</em><br>
  You already hand subagents <code>/tmp/result.md</code> — waggle makes that
  reference attributed, resolvable from any harness, revocable, and counted.
</p>

<p align="center">
  <a href="docs/design/essay.md">The essay</a> ·
  <a href="docs/WHY.md">Why it exists</a> ·
  <a href="docs/README.md">All docs</a> ·
  <a href="paper/">The paper</a> ·
  <a href="docs/design/19-interrogation-telemetry.md">What's next</a>
</p>

A **token** is a ~30-byte attributed name for an artifact. Mint one for
a file, hand off the one-line reference instead of the contents, and
the consumer pulls only the slices it needs — grep and windowed reads
*through* the token, under byte budgets. Every stage lands in a
payload-free event log: who resolved, who read, who ran, what stalled.
Corrections and revocations travel through the reference to every
holder.

The full case — the handoff problem, the bee that solved it, the
architecture at all three radii — lives in
**[the essay](docs/design/essay.md)** and
**[WHY.md](docs/WHY.md)**. The systems-paper treatment, *The Dance and
the Field: Name Semantics for Handoffs Between Distributed Agents*, is in
[**`paper/`**](paper/) (build with `tectonic waggle.tex`; the latest CI
build is attached to the
[`paper-latest`](https://github.com/modiqo/waggle/releases/tag/paper-latest)
release).

## Install & first handoff

```bash
cargo install waggle-cli                          # on crates.io
claude mcp add waggle -- waggle serve --stdio     # ...and the same line in Codex/Cursor
waggle init                                       # the 5-line agent stub, into CLAUDE.md/AGENTS.md
```

```bash
waggle mint --target "file://$PWD/q3-report.md" --snapshot
#  → { "token": "b2uQyZUC",
#      "handoff": "resolve b2uQyZUC via waggle for your working context" }

waggle search --token b2uQyZUC --pattern "pricing"   # grep THROUGH the token
waggle read   --token b2uQyZUC --lines 40-80         # a window, never the whole artifact
waggle funnel --token b2uQyZUC                       # { "resolve": 1, "read": 2, "run": 1 }
```

`just demo` runs the whole arc against a throwaway store. The
**[documentation map](docs/README.md)** holds the eleven guides in
reading order — five-minute loop, harness wiring, federation, the
Cloudflare edge, the tmux switchboard.

## Status

**v0.1.0 on [crates.io](https://crates.io/crates/waggle-cli)**; the 0.3
feature set is complete on `main` — the full verb loop, snapshots,
federation, the edge tier, Ed25519 trust, the spec with conformance
vectors, and the tmux switchboard. Every claim is a passing test in CI
(three-OS matrix + wasm + live Miniflare edge; ~170 tests). Measured
numbers live in [benches/PERF.md](benches/PERF.md).

**In design:** [interrogation telemetry](docs/design/19-interrogation-telemetry.md)
— convergence classification of consumer traces, receipt-driven model
routing, and distilling accepted reading paths into scaffolds for
weaker model families. This README will be rewritten around the
implemented system when that work lands (P5 of the plan).

## License

MIT OR Apache-2.0, at your option.

---

<p align="center">
  <em>She never carries the field home. She dances, and the hive knows.</em>
</p>
