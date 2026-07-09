# Seamless Mode: Choose Harnesses, Mint Outcomes, Resolve On Switch

Status: design v2 — the waggle-native product mode, layered ON the spine
defined in [harness-switching-standards.md](harness-switching-standards.md).
Nothing here relaxes that contract; this specifies the mode where the
switchboard OWNS the panes because the user chose the harnesses up front.

Reconciliation rule, stated once: the standards doc governs externally
registered panes (manual-first, print-the-line, no injection). This doc
governs panes created by `waggle-tmux up` (owned panes), where delivery by
injection is the seamless default with per-switch and per-profile opt-out.
Both modes share every other mechanism: profiles, state, checkpoints,
funnels, security.

## 1. The Product In One Paragraph

The user picks their harnesses once (`claude-code + codex`, extensible via
profiles). The switchboard creates the tmux workspace, wires the waggle MCP
server and the convention stub into every chosen harness, and ensures
`waggled` is up — every pane is waggle-fluent before any work starts. From
then on the loop is three gestures: **work** in whichever harness; **mint
the outcome** (one command, one keybinding, or the agent mints via MCP —
a file, or a whole handoff directory as a `--tree`); **switch** — and the
switch itself carries the token to the destination and has it resolved
there. The switch IS the handoff.

```sh
waggle-tmux up claude-code codex   # choose once; everything gets wired
# ... work in the Claude Code pane ...
waggle-tmux mint plan.md           # or: mint --tree .handoff/task-01/
waggle-tmux switch codex           # switches pane AND resolves the token there
# ... Codex continues from ITS projection ...
waggle-tmux switch claude-code     # the return trip, same gesture
waggle-tmux status                 # funnel-driven: was each handoff consumed?
```

## 2. Binding Principles

1. **The switch is the waggle moment.** Mint-if-dirty (offered), preview,
   deliver, resolve — all fire on `switch`. A switchboard where waggle is
   a side command the user must remember has failed its purpose.
2. **Profiles feed the matcher.** The destination profile's
   `ResolverContext` (family, harness, modalities, posture) drives a
   resolve PREVIEW at switch time — the human sees the projection that
   harness will receive. Codex and Claude Code get different projections
   from the same token (I-2); profiles that never touch the matcher are
   dead configuration.
3. **Orchestrate, never impersonate.** Injection places the resolve
   instruction into the destination's prompt; the destination agent
   performs its own resolve. Attribution stays honest (I-7 — the funnel
   shows Codex resolved, because Codex did) and the harness's approval
   model is never bypassed: the switchboard types an instruction, it never
   confirms dialogs or answers approval prompts.
4. **The switchboard is a waggled client.** Same unix socket, same store,
   same token space as the harnesses it manages; its own sharer identity
   signs what it mints. Shelling to the `waggle` binary is scaffolding for
   the first spike only.
5. **Everything durable is a token.** tmux can die; outcomes survive as
   snapshot-pinned tokens with lineage, greppable and revocable from any
   harness — or from the edge.

## 3. `waggle-tmux up` — choose harnesses, get a wired workspace

```sh
waggle-tmux up                        # interactive picker over detected harnesses
waggle-tmux up claude-code codex      # explicit
waggle-tmux up claude-code codex --layout side-by-side --cwd .
```

1. Detect installed harnesses (`claude --version`, `codex --version`,
   profile registry for the rest); picker for anything unspecified.
2. **Wire waggle into each chosen harness, idempotently:**
   - Claude Code: `claude mcp add waggle -- waggle serve --stdio`
     (skipped when `claude mcp list` already shows waggle)
   - Codex: ensure `[mcp_servers.waggle]` in `~/.codex/config.toml`
   - Workspace: `waggle init` (the stub into CLAUDE.md/AGENTS.md)
   - Daemon: `waggle daemon start` if not running
3. Create the tmux session: one pane per harness (launched from its
   profile), plus a shell pane; window named after the task id.
4. Register every created pane with `owned: true` — the injection
   permission bit. Externally registered panes (standards doc §1) are
   `owned: false` and always get print-the-line delivery.
5. Install keybindings (prefix+W): `m` mint picker, `s` switch menu,
   `t` status.

`up` is convergent and re-runnable: it re-wires a missing MCP entry or
restarts a dead pane instead of erroring.

## 4. `waggle-tmux mint` — mint any outcome

```sh
waggle-tmux mint plan.md                   # file → snapshot token
waggle-tmux mint --tree .handoff/task-01/  # directory → root + children, one command
waggle-tmux mint                           # picker: git-modified, handoff dir, bounded tail
```

- Always `--snapshot` (outcomes outlive panes), `--channel tmux/outcome`,
  `--parent` chained to the task's previous outcome (lineage = task
  history), signed by the switchboard identity.
- Directories are `--tree` mints: the root token IS the handoff — the
  destination resolves the root to its index, deep-greps the whole bundle
  through it, and one revocation tombstones everything.
- The newest outcome becomes the task's **pending handoff**.
- **Agents minting over MCP land in the same place**: the switchboard
  watches the store for new tokens on the task's channel and adopts the
  newest as pending. Two doors, one register.

## 5. `waggle-tmux switch` — resolve upon switch

```sh
waggle-tmux switch codex               # deliver the pending outcome
waggle-tmux switch codex --token <t>   # deliver a specific token
waggle-tmux switch codex --no-inject   # print the line instead (this switch only)
waggle-tmux next                       # follow the pending handoff's destination
```

The one-verb sequence:

1. **Mint-if-dirty, offered:** unminted candidate outcomes in the source
   pane trigger the mint picker first (declinable) — nothing strands in
   scrollback.
2. **Preview with the destination's context:** resolve with the
   destination profile's `ResolverContext`; show one line of what that
   harness will receive (variant + body head). The matcher runs at switch
   time — this is the native integration.
3. **Record** `handoff-sent` on the token.
4. **Select** the destination pane.
5. **Deliver:** `owned` pane → inject via `send-keys` using the profile's
   `inject_template`:

   ```text
   Resolve <token> via waggle for your working context. Use waggle
   search/read for slices; record --stage run when you have used it.
   ```

   Non-owned pane or `--no-inject` → print the identical line. Either
   way the DESTINATION executes the resolve.
6. **Confirm consumption from the funnel:** when the destination's
   `resolve` event appears on the token, the handoff flips
   `delivered → consumed` in `status`. No sentinel needed on the common
   path; the standards doc's sentinel remains for agents that cannot run
   commands.

## 6. `waggle-tmux status` — funnel-driven truth

```text
Task wg-task-20260709-001            store: ~/.waggle/waggle.db

SESSION   PROFILE      PANE  OWNED  PENDING/LAST TOKEN  CONSUMED?
claude    claude-code  %1    yes    7Kp2xQ9f (sent)     yes — resolve + 3 reads
codex     codex        %2    yes    b2uQyZUC (pending)  —

Lineage: plan(7Kp2xQ9f) → diff+log tree(b2uQyZUC)
```

`CONSUMED?` derives from the token's funnel — true even if local state is
lost, because the store is the truth.

## 7. Deltas Against The Standards Doc

| Standards doc | Seamless mode |
|---|---|
| register existing panes | `up` creates + auto-registers owned panes (register still exists) |
| checkpoint command builds files | demoted to convenience; the primitive is `mint` (file or `--tree`) |
| print the resolve line | inject on owned panes (opt-out); print for everything else |
| `handoff-resumed` stage | dropped — the funnel's automatic resolve/read is the consumption signal |
| completion: manual/sentinel | plus funnel-watch (consumption confirmed from the store) |

Everything else — profile standard, state standard (§10), naming (§11),
adapter boundaries (§12), review gates (§13), what stays out of core
(§15) — applies to this mode verbatim.

## 8. Done Criteria (the seamless dogfood)

1. `waggle-tmux up claude-code codex` on a clean checkout: both harnesses
   open in tmux, MCP wired in both, stub installed, daemon up — zero
   manual configuration.
2. Claude pane produces `plan.md`; `waggle-tmux mint plan.md` (or prefix+W m).
3. `waggle-tmux switch codex`: pane switches, Codex's prompt receives the
   instruction, Codex resolves ITS projection and implements.
4. The diff+log directory returns via `mint --tree` + `switch claude-code`;
   Claude deep-greps the log through the root token.
5. `status` shows both handoffs CONSUMED — from funnels, not bookkeeping.
6. Revoking the stale plan tombstones it (and its tree) for both panes.

Choose your harnesses once; from then on outcomes move between them as
30-byte tokens — minted in one gesture, resolved by the act of switching.


## 9. As Built (post-implementation deltas)

Field-driving reshaped several choices; the code is the authority:

- **Focus layout replaced split panes**: one WINDOW per harness (names
  in the status bar are the switcher), each with a board strip; a
  handoff SWAPS windows. `prefix+z` zooms; mouse mode is on.
- **The watch loop split into modes**: exactly one DELIVERER
  (`watch --headless`, the `wgd` window — duplicates would
  double-deliver) and freely replicated BOARDS (`watch --board-only`,
  one strip per window, height-adaptive, 3-state cycle via
  `board-toggle`/`prefix+B`).
- **Harnesses run AS the pane process** — the send-keys launch race is
  structurally gone; delivery refuses to type into bare shells.
- **Exits are first-class**: a `pane-exited` hook (plus a deliverer-tick
  sweep) reaps dead harnesses, forwards focus to a survivor, and closes
  the room after the last exit (the room is recorded at registration).
- **Bindings derive their workspace at keypress time**
  (`#{pane_current_path}`) — server-global binds can't go stale across
  projects.
- **Pending is a QUEUE; multi-path mints are lineage BUNDLES**; and
  `mint --seal` moves sources into the vault so the token is the only
  door (coverage receipts become enforcement-grade locally).
