# The tmux switchboard, step by step

Three complete sessions with `waggle-tmux`: same-family handoffs
(Fable ↔ Opus), cross-family with lineage (Claude → Codex), and a
delegation chain (Claude → a subagent inside Codex → back). Files and
folders both. Every command is real; every receipt shown is what the
store actually answers.

## 0 · One-time setup

```sh
brew install tmux
cargo install waggle-cli            # waggle + waggled
cargo install waggle-tmux           # the switchboard (or --path crates/waggle-tmux)
```

Everything below happens per-project and starts the same way:

```sh
cd ~/your-project
waggle-tmux up claude-code codex
```

`up` is convergent — run it again anytime; it repairs rather than
errors (dead panes are re-created, missing wiring re-applied). It
leaves you attached inside a **window-per-harness workspace**:

```text
status bar:   [waggle]  0:claude-code*  1:codex  2:wgd
              ─ your harnesses, by name; the active one starred ─

each window:  ┌─ claude-code | * Claude Code ──────────────┐
              │  the harness, running AS the pane process   │
              ├─ claude-code | waggle ──────────────────────┤
              │  the live board: lineage tree + receipts    │
              └─────────────────────────────────────────────┘
```

Every harness runs as its pane's own process (no shell in between —
nothing to race, nothing for `oh-my-zsh` to swallow), each window
carries its own **board strip**, and `wgd` is the single hidden
deliverer. The keyboard and mouse surface:

| Gesture | Does |
|---|---|
| `prefix+W` | the menu: switch to any session / mint by path / status / board |
| `prefix+B` | board cycle: strip (6 lines) → maximized (half) → minimized (1 line) |
| `prefix+z` | zoom the active pane truly full-screen (native) |
| `prefix+1`/`2` | jump to a harness window (native) |
| mouse | click a bar name to swap harnesses; drag the board border to size it |
| `/exit` in a harness | its window closes; the SURVIVOR is foregrounded; last one out closes the room |

And this convention is written into `CLAUDE.md`/`AGENTS.md` where every
agent reads it:

```markdown
## Harness handoffs (waggle-tmux)
When your task is COMPLETE, mint the outcome yourself and address it:
    waggle mint --target <file-or-dir> --snapshot --channel tmux/<destination>
```

That channel address is the entire routing protocol. The watcher sees
the minted token in the shared store and performs the jump: focuses the
destination pane and types the resolve instruction into its prompt.
The destination resolves as itself — its actor class lands in the
funnel, so consumption is *provable*, not assumed.

The three gears, from most manual to none:

| Gear | You do |
|---|---|
| by hand | `waggle-tmux mint <path> --to codex` then `waggle-tmux switch codex` |
| one key | `prefix+W` → `m` (mint) / `1..n` (switch) / `t` (status) |
| automatic | nothing — agents mint with `--channel tmux/<dest>`, the watcher jumps |

Two shapes for many outcomes: **several paths in one mint make a
bundle** (`waggle-tmux mint images one.md two.md --to codex` — a note
root with every piece as a child; one token travels), and independent
mints **queue** — a switch delivers everything pending for that
destination, in order, each with its own receipt.

---

## 1 · Same family, two minds: Fable drafts, Opus reviews

Two Claude Code sessions with different models — a drafter and a
reviewer — passing one FILE back and forth. First, teach the
switchboard the two profiles (`.waggle/tmux/config.toml` in the
workspace; user entries merge over the builtins):

```toml
[profiles.fable]
display_name = "Claude Fable"
family = "claude"
harness = "claude-code"
modalities = ["text", "shell"]
posture = "attended"
launch_command = "claude --model claude-fable-5"

[profiles.opus]
display_name = "Claude Opus"
family = "claude"
harness = "claude-code"
modalities = ["text", "shell"]
posture = "attended"
launch_command = "claude --model claude-opus-4-8"
```

```sh
waggle-tmux up fable opus
```

You land in the Fable pane. Ask for the draft, ending with the routing
line:

> Draft the API deprecation notice in `notice.md`. When done:
> `waggle mint --target notice.md --snapshot --channel tmux/opus`

Fable writes, mints, and — hands off your keyboard — the watcher flips
you to the Opus pane, where the instruction has typed itself:

```text
Resolve 7Kp2xQ9f via waggle for your working context. Use waggle
search/read for slices; record --stage run when you have used it.
```

Opus resolves the token (the snapshot travels, not your prompt), reads
it surgically, and reviews. Ask Opus to send its verdict back the same
way — `--channel tmux/fable` — and the screen returns to Fable with the
review token.

**The receipts** (`prefix+W` → `t`, or `waggle-tmux status` — and the
board strip under every window shows the same truth live):

```text
SESSION  PROFILE  PANE  OWNED  LAST TOKEN  CONSUMED?
fable    fable    %0    yes    mR4vT2xa    yes — 1 resolve(s)
opus     opus     %1    yes    7Kp2xQ9f    yes — 1 resolve(s)
```

And the store's view of any token: `waggle funnel --token 7Kp2xQ9f` →
`{resolve: 1, read: 2, run: 1}` — Opus resolved, sliced twice, used it.
Payload-free: the log knows *that* Opus read, never *what it searched*.

An honest note on granularity: variant targeting keys on the coarse
model FAMILY (`claude`, `gpt` — invariant I-7 keeps fine identity out
of the log), so Fable and Opus receive the same projection here. What
this pattern buys is separated contexts, per-session attribution, and
the drafter/reviewer rally without a single paste.

---

## 2 · Across families, with lineage: Claude plans, Codex builds a FOLDER

The cross-family handoff is where projections diverge (a `claude`-family
consumer and a `gpt`-family consumer can be served different variants of
the same token) — and where folders earn their keep.

```sh
waggle-tmux up claude-code codex
```

**Step 1 — the plan (a file).** In the Claude pane:

> Plan the rate-limiter refactor in `plan.md` — goals, files to touch,
> test strategy. When done, mint it to codex.

Claude mints (`--channel tmux/codex`), the watcher jumps, Codex
resolves *its* projection of the plan and implements.

**Step 2 — the result (a FOLDER).** Tell Codex (or let AGENTS.md say it
standing):

> Put your outputs in `handoff/` — the diff, the test log, notes. When
> done: `waggle mint --target handoff --snapshot --channel tmux/claude-code`

A directory target mints as an **indexed tree**: the folder token is the
root of a content-addressed tree — one node per folder, snapshot-pinned,
thousands of files in one mint. One token travels; behind it:

```sh
waggle read --token b2uQyZUC           # the root's table of contents:
#   files: [ handoff/diff.patch, handoff/test.log, handoff/notes.md ]
#   (subdirectories, if any, each carry a token to descend)

waggle search --token b2uQyZUC --pattern "FAILED"   # ONE call greps the
#   whole tree — Bloom-pruned, ranked — each match naming its file path
#   and owning node token, with a read-this-next chain into the first hit

waggle read --token b2uQyZUC --file test.log        # open one file by name
```

**Step 3 — lineage.** `waggle-tmux mint` chains each outcome to the
previous delivery automatically, so the rally IS a tree; to declare it
explicitly (as agents should when minting via MCP):

```sh
waggle mint --target handoff --snapshot --parent 7Kp2xQ9f --channel tmux/claude-code
```

Now the plan token is the ancestor of the implementation tree, and the
lineage answers questions flat logs cannot:

```sh
waggle funnel --token 7Kp2xQ9f     # the PLAN's funnel now carries a
#   "rollup" — its own stages plus every descendant's: the plan
#   answers for everything built from it

waggle map --token 7Kp2xQ9f        # where it stands, children counted,
#   ranked forward/reverse paths
```

**Step 3.5 — the review PROOF.** When Codex hands the folder back and
claims "reviewed", don't take its word — take the receipts:

```sh
waggle coverage --token b2uQyZUC
#  read 2/3 · run 0/3 · complete: false
#  MISSED: notes.md            ← the skipped file, named
#  next: read {token: pV5m}    ← close the gap, executable
```

Three honest levels per file: `unread` (never touched), `read` (bytes
actually served — resolve, read, or a deep search reaching it), `run`
(the consumer recorded using it). Misses are absence of RECEIPTS, not
surveillance — the funnel never sees content. And when the review must
be provable even against a side-reading local agent, seal the handoff:

```sh
waggle-tmux mint review_me --seal --to codex
# sealed: review_me moved to .waggle-handoffs/sealed/<token>/ — the
# token is now the ONLY door
```

The source moves out of the working tree (non-destructive — the vault
keeps it; move back to unseal), so the snapshot-backed token is the
only access path and coverage receipts become enforcement-grade.

**Step 4 — the kill switch.** The plan was wrong? One revocation
tombstones the whole line — plan, implementation tree, every file:

```sh
waggle mutate --token 7Kp2xQ9f --change revoke --expected-version 1
waggle resolve --token wN2k        # → revoked (through its lineage)
waggle read    --token wN2k        # → refused: revoked content serves nothing
```

---

## 3 · The delegation chain: Claude → a subagent inside Codex → back

The deepest pattern: your main Claude session delegates to Codex, Codex
delegates *internally* to its own subagent, and the result climbs back
up the same lineage — every hop attributed.

**Step 1 — the mission, from main Claude:**

> Create `mission.md`: "audit error handling in src/parser". Mint it to
> codex and note the token.

Watcher jumps to Codex, which resolves the mission token (call it
`M1ss10n0`).

**Step 2 — Codex delegates inward.** Codex doesn't paste the mission to
its subagent either — the stub taught it the same move it was received
with. In the Codex pane:

> Spawn a subagent for the parser audit. Hand it ONLY this line:
> "Resolve M1ss10n0 via waggle for your working context; use
> search/read for slices; record --stage run when done."

The subagent — fresh context, zero pasted content — resolves the SAME
token from the shared daemon, greps its slices, audits, and writes
`audit/findings.md` + `audit/evidence.log`.

**Step 3 — the result climbs back, parented to the mission:**

```sh
waggle mint --target audit --snapshot --parent M1ss10n0 --channel tmux/claude-code
```

The watcher flips you back to main Claude, which resolves the audit
tree's index and reads only the findings it needs.

**Step 4 — the whole rally, in one query.** Back in any pane:

```sh
waggle funnel --token M1ss10n0
```

```json
{
  "stages":  { "resolve": 2, "read": 5, "run": 1 },
  "children": ["aUd1tR00"],
  "rollup":  { "resolve": 3, "read": 7, "run": 1 }
}
```

Read that back as the story it is: the mission was resolved twice (Codex
AND its subagent — same token, two consumers, each counted), sliced five
times, run once; the rollup adds the audit tree's own consumption by
main Claude. Three contexts touched the work; the mission document
crossed a prompt boundary **zero** times.

---

## Troubleshooting

- **The instruction printed instead of typing itself** — the destination
  pane's foreground process is a bare shell (harness not up). Start the
  harness in that pane; delivery refuses to type into shells by design.
- **`status` says `not yet` but the agent claims it read** — trust
  `status`: it derives from the funnel, and the destination's own
  resolve is the only thing that flips it.
- **A stale outcome keeps getting delivered** — revoke it
  (`waggle mutate … --change revoke`); the watcher delivers tokens, but
  a revoked token resolves as a tombstone everywhere.
- **Killed the tmux server?** `waggle-tmux up …` again — it detects dead
  panes and rebuilds; tokens, snapshots, and receipts all live in
  `~/.waggle`, which tmux never touches.
- **Go manual** — kill the `wgd` window (the single deliverer);
  `prefix+W` still works. `waggle-tmux watch --headless` brings
  automation back; board strips (`watch --board-only`) are pure readers
  and never deliver.
- **Keys dead on a Mac?** Some terminal apps intercept `Ctrl-b` — the
  mouse does everything (click bar names, drag borders), or attach from
  a plain terminal. Bindings derive their workspace at keypress time,
  so multiple projects never poison each other.
- **Can't find a token?** `waggle find <name>` — basenames, tags
  (`mint --tag name=...`), channels, sharers; ranked candidates with
  disposition shown.
