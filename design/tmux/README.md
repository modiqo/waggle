# Waggle + tmux: Durable Agent Sessions and Checkpointed Handoffs

Status: design proposal for implementation. Read with its two companions:
[harness-switching-standards.md](harness-switching-standards.md) (the
discipline contract — what must be true to belong in this repo) and
[seamless-mode.md](seamless-mode.md) (design v2: choose harnesses at `up`,
auto-wire waggle MCP, mint any outcome, resolve upon switch — the
waggle-native mode for panes the switchboard owns).

Audience: the next Codex, Claude Code, or human session that will implement a
tmux-backed Waggle extension for switching work between agent harnesses.

Primary goal: make it practical to move a coding task between harnesses such as
Codex and Claude Code without losing the state that matters. tmux keeps sessions
alive. Waggle makes the handoff durable, attributable, searchable, revocable,
and resolvable by another harness.

## 1. Executive Summary

Waggle already gives agents a compact, resolvable token for artifacts:

- `waggle mint --target <path> --snapshot`
- `waggle resolve --token <token>`
- `waggle read --token <token>`
- `waggle search --token <token> --pattern <regex>`
- `waggle record --token <token> --stage <stage>`
- `waggle mutate --token <token> --change supersede=<new-token>`
- `waggle funnel --token <token>`

The missing product layer is a local "switchboard" that understands live agent
sessions. tmux is the right substrate for this first because it can keep
interactive CLIs alive while the user moves between panes, tabs, shells, or
machines.

The proposed extension is a new wrapper, tentatively named `waggle-tmux`, that:

1. Registers and tracks tmux-backed agent sessions.
2. Knows which harness is in each pane: Codex, Claude Code, or a generic CLI.
3. Creates a Waggle checkpoint when a session reaches a useful stopping point.
4. Makes the next checkpoint available to another session and switches control
   to that tmux pane. The human stays inside the destination harness prompt and
   asks it to resolve the token there.
5. Records lifecycle events so the user can answer: who produced what, which
   harness consumed it, what changed, and what superseded it?

This is not trying to replace Codex, Claude Code, tmux, git, `rg`, RAG, or MCP.
It is a local durable handoff layer for grep-first coding agents.

## 2. Why This Is Useful

Interactive agent sessions are currently fragile handoff boundaries:

- A Claude Code pane may contain a useful plan, but Codex cannot reliably
  recover that plan unless it is copied into a prompt or written to a file.
- Codex may produce a failing test log or patch, but Claude Code cannot know
  whether the log is stale, complete, or the latest one.
- A user may inspect two panes and remember the sequence, but another harness
  has no durable lineage.
- tmux preserves terminal state, but terminal state is not an artifact protocol.

Waggle can turn session outputs into explicit objects:

- A plan becomes a token.
- A diff becomes a token.
- A failing log becomes a token.
- A checkpoint summary becomes a token that points to the other tokens.
- A later correction can supersede the earlier checkpoint.
- A consuming harness can resolve, read, or search the token without being given
  a wall of pasted context.

The useful unit is not "a tmux pane." The useful unit is a checkpointed handoff.

## 3. Product Shape

The user should be able to run something like this:

```sh
waggle-tmux init
waggle-tmux register planner --profile claude-code --pane %1
waggle-tmux register implementer --profile codex --pane %2
waggle-tmux switch planner
# interact in Claude Code; write .waggle-handoffs/wg-task-20260709-001/plan.md
waggle-tmux checkpoint --session planner --artifact .waggle-handoffs/wg-task-20260709-001/plan.md --next implementer
waggle-tmux next
```

`waggle-tmux next` selects the tmux pane registered as `implementer` and
prints the short resolve line:

```text
Resolve <checkpoint-token> with Waggle and continue the implementation.
```

Prompt injection is optional automation for later. The first-class interaction
model is: start the harnesses however you prefer, register their tmux panes,
work normally, checkpoint, let the switchboard move tmux control to the next
registered session, and resolve from that harness.

The simple switchboard MVP is only:

- profiles: what each harness is (`claude-code`, `codex`, `generic`, ...)
- registration: which live tmux pane belongs to which profile
- checkpointing: mint artifacts and checkpoint files through Waggle
- pending handoffs: remember which checkpoint is next for which session
- switching: select the next tmux pane and print the resolve line

It is not a TUI, not a launcher, not a prompt injector, and not a replacement
for tmux or the existing Waggle MCP/CLI surface.

For the human, the workflow should feel like a tmux switchboard:

- one pane for Claude Code
- one pane for Codex
- one pane for shell/test commands
- one pane for handoff queue/status
- one profile table that says what each harness is and which model family it
  belongs to

For the agents, the workflow should feel like a normal harness session plus a
small set of Waggle-aware instructions.

## 3.5 Native Integration Principle (added after review)

The switchboard is not tmux glue that happens to call waggle on the side.
**The switch itself is the waggle moment**, by design:

1. **Profiles feed the matcher.** A profile's `family`/`harness`/`modalities`/
   `posture` projects into a `ResolverContext` — and `next`/`switch` MUST use
   it: preview the checkpoint with the DESTINATION's context
   (`waggle resolve --token <t> --context <profile-context>`) so the human
   sees the projection that harness will actually receive before control
   moves. Profiles that never touch the matcher are dead configuration.

2. **Orchestrate, never impersonate.** The destination harness performs its
   own resolve. That keeps the funnel's attribution honest (the actor class
   in the log is really Codex) and the projection adaptive per consumer.
   The switchboard records `handoff-sent`, previews, places the resolve
   line — the consuming resolve belongs to the consumer.

3. **Speak to waggled directly.** The switchboard is another client on the
   shared unix socket — same daemon, same store, same token space as every
   harness it manages. Shelling to the `waggle` binary is acceptable
   scaffolding for the first spike only; the native shape is the socket's
   line protocol (or `waggle_mcp::Handler` in-process), with the
   switchboard's own sharer identity signing checkpoints.

4. **Checkpoint on switch-away.** Leaving a session with uncheckpointed work
   is the natural prompt point. `switch`/`next` should offer (never force)
   a checkpoint of the pending handoff directory before moving focus, so no
   stopping point is stranded in terminal scrollback.

### 3.6 Post-0.3 simplifications this doc predates

Waggle now has folder-native lineage, which collapses most of §11's
mechanics:

- **One checkpoint = one `--tree` mint.** `waggle mint --target <ckpt-dir>
  --tree --channel tmux/checkpoint` mints the checkpoint root AND every
  artifact as snapshot-pinned children in one command — keeping every
  per-artifact property §11.2 wants (supersede one artifact, per-artifact
  funnels, read only what you need).
- **Deep search over the root.** The receiving harness greps the WHOLE
  checkpoint bundle through the root token; matches come back grouped per
  file (diff vs test log vs plan) with a grep-to-open chain.
- **The root resolves to its index.** The destination discovers the
  artifact tokens by resolving the root — embedding the token list in
  checkpoint.md is nice for humans but no longer load-bearing.
- **One revocation tombstones the whole checkpoint** — a stale handoff dies
  atomically, resolution and content both.
- **Trim the custom stages.** `handoff-resumed` is redundant: the funnel
  records the destination's `resolve`/`read` automatically, which is the
  honest consumption signal. Keep `checkpoint-created`, `handoff-sent`,
  `ready-for-review`, `needs-human`.
- **`status` should surface the funnel.** The killer switchboard column is
  "consumed?" — derived from the checkpoint token's funnel, not from local
  switchboard state.

## 4. Relationship To Existing Waggle Design

This proposal should initially live above Waggle core.

Do not add a new core primitive until the wrapper has been dogfooded. A
"checkpoint" can be represented today as a content artifact plus normal records:

- mint every checkpoint file with `--snapshot`
- mint every artifact file with `--snapshot`
- optionally mint a directory with `--tree`
- record lifecycle stages using `waggle record --stage <custom-kebab-slug>`
- use `waggle mutate --change supersede=<new-token>` for corrected checkpoints
- inspect adoption with `waggle funnel`

The current Waggle CLI supports custom stage slugs. That is enough for the MVP.

Proposed custom stages:

- `tmux-session-started`
- `checkpoint-created`
- `handoff-sent`
- `handoff-resumed`
- `agent-completed`
- `ready-for-review`
- `needs-human`
- `superseded`

If these stages become stable, they can later be promoted into documented
well-known stages.

## 5. Core Concepts

### 5.1 Agent Session

An Agent Session is a long-lived harness process running inside a tmux pane.

It has:

- a stable local id, for example `planner` or `codex-impl`
- a harness type, for example `claude`, `codex`, or `generic`
- a tmux session/window/pane address
- a working directory
- an optional git worktree path
- an optional current task id
- the last checkpoint token it produced or consumed
- a state: `new`, `running`, `waiting`, `dead`, `detached`

The session is not the source of truth for artifacts. tmux can die. The pane can
be cleared. The durable state is the checkpoint tokens and local switchboard
state file.

### 5.2 Checkpoint

A Checkpoint is a durable summary of a useful stopping point.

It should include:

- task id
- source session id
- source harness
- timestamp
- status
- parent checkpoint token, if any
- summary text
- artifact token list
- optional transcript tail token
- optional git diff token
- optional test log token
- optional next session
- explicit next action

The checkpoint is itself stored as a markdown or JSON artifact and minted with
Waggle. The checkpoint token is the compact reference passed to another harness.

### 5.3 Handoff

A Handoff is a pending checkpoint for another Agent Session.

It includes:

- source session
- destination session
- checkpoint token
- intent, for example `implement`, `review`, `debug`, `summarize`
- optional resolve-line template for the destination
- state: `pending`, `selected`, `resumed`, `failed`

The first implementation can treat handoff as local state. Later, handoff can be
recorded more formally in Waggle if needed.

### 5.4 Switchboard

The Switchboard is the local controller.

Responsibilities:

- create tmux sessions/panes
- list sessions
- select/switch panes
- select panes and present the next checkpoint to the human
- capture transcript tails
- watch for completion signals
- create checkpoints
- store pending handoffs
- keep a replayable local state log

This can be a CLI first. A terminal UI can come later.

## 6. Architecture

Recommended first shape:

```text
crates/waggle-tmux/
  src/
    main.rs
    cli.rs
    tmux.rs
    profile.rs
    session.rs
    checkpoint.rs
    state.rs
    waggle.rs
    git.rs
    handoff.rs
    watch.rs
```

Logical architecture:

```text
User
  |
  v
waggle-tmux CLI
  |
  +-- State store
  |     .waggle/tmux/events.jsonl
  |     .waggle/tmux/state.json
  |
  +-- tmux adapter
  |     list-panes, select-pane, select-window,
  |     capture-pane, pipe-pane, optional send-keys
  |
  +-- profile registry
  |     claude-code, codex, generic, user-defined profiles
  |
  +-- session registry
  |     local session id -> harness profile -> tmux pane
  |
  +-- checkpoint builder
  |     checkpoint markdown/json, transcript tail, git diff, logs
  |
  +-- waggle adapter
  |     waggle mint/record/read/search/resolve/mutate/funnel
  |
  +-- git/worktree adapter
        status, diff, branch, optional isolated worktrees
```

Keep the implementation boring:

- call the existing `waggle` binary for MVP
- call the existing `tmux` binary for MVP
- register existing panes instead of owning harness launch on day one
- add typed Rust wrappers around those calls
- add direct library integration only after behavior stabilizes

This avoids dragging tmux concerns into `waggle-core` or `waggle-mcp`.

## 7. Where This Should Live

Preferred path:

```text
crates/waggle-tmux/
```

with binary:

```text
waggle-tmux
```

This avoids overloading the main `waggle` CLI until the command model is proven.

Possible later merge:

```sh
waggle session init
waggle session profile
waggle session register
waggle session checkpoint
waggle session handoff
waggle session switch
waggle session next
```

Do not start there. A separate crate gives room to experiment without changing
the stable operations catalog.

## 8. tmux Integration

### 8.1 Local Prerequisite

tmux was not installed in the local environment when this design was written.
The implementation should start with a clear preflight:

```sh
tmux -V
```

If missing, fail with:

```text
tmux is required for waggle-tmux. Install it, then run waggle-tmux init again.
```

On macOS:

```sh
brew install tmux
```

### 8.2 Rust Library Choice

There is a Rust crate named `tmux_interface`:

- docs: https://docs.rs/tmux_interface/latest/tmux_interface/
- repo: https://github.com/imsnif/tmux-interface-rs

It is useful for building tmux commands such as `new-session`, `new-window`,
`split-window`, and `kill-session`.

Important caveat: control mode support is not the main thing to rely on for the
MVP. The wrapper can use plain `std::process::Command` or `tokio::process` for
tmux commands and introduce `tmux_interface` only where it reduces boilerplate.

Recommendation:

- MVP: raw tmux command wrapper, thoroughly tested with a fake backend.
- Later: use `tmux_interface` for command construction if it improves clarity.

### 8.3 Required tmux Commands

Minimum commands:

```sh
tmux new-session -d -s <session> -c <cwd>
tmux new-window -t <session> -n <window> -c <cwd>
tmux split-window -t <target> -h -c <cwd>
tmux split-window -t <target> -v -c <cwd>
tmux send-keys -t <pane> -- <text> Enter   # optional inject command only
tmux capture-pane -t <pane> -p -S -2000
tmux pipe-pane -t <pane> -o "cat >> <log-path>"
tmux select-pane -t <pane>
tmux select-window -t <target>
tmux list-panes -a -F "#{session_name}\t#{window_index}\t#{pane_id}\t#{pane_current_path}\t#{pane_current_command}"
```

Implementation note: never build shell command strings by concatenating user
input. Use `Command::new("tmux").args([...])`.

### 8.4 Pane Addressing

Store tmux pane ids, not only pane indexes.

tmux pane indexes can change after splits and closes. Pane ids like `%3` are
more stable during a tmux server lifetime.

State should store:

```json
{
  "tmux_session": "waggle",
  "tmux_window": "wg-task-20260709-001",
  "tmux_pane": "%3"
}
```

### 8.5 Transcript Capture

Use two levels:

1. Best effort continuous logs with `tmux pipe-pane`.
2. On-demand tail capture with `tmux capture-pane`.

Continuous logs are useful for debugging, but they can include secrets or noisy
interactive output. Do not mint full logs by default. The checkpoint builder
should mint only a bounded transcript tail unless the user opts in.

Suggested default:

- capture last 2000 lines
- trim to 64 KiB
- redact obvious env assignment lines
- store under `.waggle/tmux/transcripts/`
- mint with `waggle mint --snapshot`

## 9. Harness Profiles

Harness profiles are local data. They describe what a registered pane contains
well enough for the switchboard to build a `ResolverContext`, show useful
status, and later support optional launch helpers. Profiles should not become
per-harness Rust plugins in the MVP.

### 9.1 Codex Profile

Observed local command:

```sh
codex
codex exec
codex review
codex mcp
codex mcp-server
```

Use cases:

- registered interactive pane: `codex`
- non-interactive completion: `codex exec <prompt>`
- review command: `codex review`

MVP profile:

```toml
[profiles.codex]
display_name = "Codex"
family = "gpt"
harness = "codex"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = "codex"
args = []
```

Non-interactive profile:

```toml
[profiles.codex-exec]
display_name = "Codex Exec"
family = "gpt"
harness = "codex"
modalities = ["text", "shell"]
posture = "headless"
completion = "process-exit"
launch_command = "codex"
args = ["exec"]
```

### 9.2 Claude Code Profile

Observed local command supports:

```sh
claude
claude --print
claude --output-format json
claude --output-format stream-json
claude --input-format stream-json
claude --bg
claude --tmux
claude --worktree
```

Use cases:

- registered interactive pane: `claude`
- non-interactive completion: `claude --print --output-format json`
- background/tmux/worktree features can be used later, but should not be
  required for the first version.

MVP profile:

```toml
[profiles.claude-code]
display_name = "Claude Code"
family = "claude"
harness = "claude-code"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = "claude"
args = []
```

Non-interactive profile:

```toml
[profiles.claude-print]
display_name = "Claude Print"
family = "claude"
harness = "claude-code"
modalities = ["text", "shell"]
posture = "headless"
completion = "process-exit"
launch_command = "claude"
args = ["--print", "--output-format", "json"]
```

### 9.3 Generic Profile

The generic profile lets users bring another terminal agent:

```toml
[profiles.generic]
display_name = "Generic Agent"
family = "other"
harness = "other"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = "aider"
args = []
```

The profile system must support:

- custom launch hint
- custom completion sentinel
- custom resolve-line template
- optional environment variables

## 10. Completion Detection

Completion detection is the hardest part. Use layered detection rather than one
fragile trick.

### 10.1 Best: Explicit Checkpoint Command

The most reliable completion signal is an explicit command:

```sh
waggle-tmux checkpoint \
  --session implementer \
  --status ready-for-review \
  --artifact .waggle-handoffs/wg-task-20260709-001/diff.patch \
  --artifact .waggle-handoffs/wg-task-20260709-001/test.log \
  --next planner
```

For interactive agent sessions, prompt the agent to run this command when done.
If agents cannot run shell commands directly, prompt them to write a sentinel
line and let the watcher perform the checkpoint.

### 10.2 Good: Process Exit For Non-Interactive Runs

For one-shot jobs:

```sh
codex exec "<prompt>"
claude --print --output-format json "<prompt>"
```

The wrapper can wait for process exit, collect stdout/stderr, write a result
file, mint it, and store a pending handoff.

This is the cleanest path for automated routing.

### 10.3 Acceptable: Sentinel Line

For interactive tmux panes, ask the agent to print:

```text
WAGGLE_DONE {"status":"ready-for-review","to":"planner","artifacts":[".waggle-handoffs/wg-task-20260709-001/diff.patch",".waggle-handoffs/wg-task-20260709-001/test.log"]}
```

The watcher can read the `pipe-pane` log and parse this line.

Rules:

- sentinel must be one line
- JSON must be valid
- artifact paths must be workspace-relative or allowlisted
- missing artifacts should fail the checkpoint, not silently create a handoff

### 10.4 Last Resort: Screen Heuristics

Avoid relying on prompt-shape detection such as "the agent prompt is visible
again." It will break across versions and themes.

Heuristics may be used only to display "probably idle" in a TUI. They should
not trigger durable handoff automatically.

## 11. Checkpoint Semantics

### 11.1 Checkpoint File

Every checkpoint should produce a human-readable file:

```text
.waggle-handoffs/<task-id>/<checkpoint-id>/checkpoint.md
```

Suggested markdown:

```md
# Waggle tmux Checkpoint

Task: wg-task-20260709-001
Checkpoint: ckpt-20260709-143012-codex-impl
Source session: implementer
Source harness: codex
Status: ready-for-review
Parent checkpoint: <parent-token-or-none>
Next session: planner

## Summary

Implemented the parser change and updated the failing test.

## Artifacts

- diff: <token>
- test log: <token>
- transcript tail: <token>

## Next Action

Review the diff and decide whether to apply the superseding fix.
```

Mint it:

```sh
waggle mint \
  --target .waggle-handoffs/wg-task-20260709-001/ckpt-20260709-143012-codex-impl/checkpoint.md \
  --snapshot \
  --channel tmux/checkpoint
```

Then record:

```sh
waggle record --token <checkpoint-token> --stage checkpoint-created
```

If `--next` is present:

```sh
waggle record --token <checkpoint-token> --stage handoff-sent
```

When the destination starts using it:

```sh
waggle record --token <checkpoint-token> --stage handoff-resumed
```

### 11.2 Artifact Minting

Each artifact should be minted separately before the checkpoint file is minted.

Examples:

```sh
waggle mint --target .waggle-handoffs/wg-task-20260709-001/plan.md --snapshot --channel tmux/artifact
waggle mint --target .waggle-handoffs/wg-task-20260709-001/diff.patch --snapshot --channel tmux/artifact
waggle mint --target .waggle-handoffs/wg-task-20260709-001/test.log --snapshot --channel tmux/artifact
```

The checkpoint file should contain the resulting artifact tokens.

Why separate artifacts?

- `waggle search` can target the exact test log.
- a later correction can supersede only the bad artifact.
- the receiving agent can read only the artifact it needs.
- artifact-level tokens produce better funnels.

### 11.3 Parent Lineage

Use the `--parent` flag when minting a checkpoint if there is a previous
checkpoint for the same task:

```sh
waggle mint \
  --target <checkpoint.md> \
  --snapshot \
  --parent <previous-checkpoint-token> \
  --channel tmux/checkpoint
```

This makes the task lineage explicit at the Waggle layer.

### 11.4 Superseding A Bad Handoff

If a checkpoint is wrong or stale, create a new checkpoint and supersede the old
one:

```sh
waggle mutate \
  --token <old-checkpoint-token> \
  --change supersede=<new-checkpoint-token> \
  --expected-version <version>
```

The implementation must capture the manifest version returned by resolve/query
before applying lifecycle changes.

For MVP, if the exact version is inconvenient, do not automate supersede. Print
the old/new tokens and let the user run the mutate command manually.

## 12. Local State Model

Use append-only local state first:

```text
.waggle/tmux/events.jsonl
```

Derive current state into:

```text
.waggle/tmux/state.json
```

This mirrors Waggle's preference for replayable logs and avoids early SQLite
schema churn. Move to SQLite only if query complexity grows.

The effective data structures are:

```text
HarnessProfile       what a harness is
RegisteredSession    which live tmux pane carries that harness
CheckpointRecord     what durable artifact was created
PendingHandoff       which checkpoint should be resumed next, and by whom
SwitchboardState     replayed view over the JSONL event log
```

Keep these structures intentionally small. Waggle tokens are the durable
artifact identity; switchboard state is only the local control plane.

### 12.1 HarnessProfile JSON

```json
{
  "type": "profile_upserted",
  "at": "2026-07-09T14:25:00Z",
  "profile_id": "codex",
  "display_name": "Codex",
  "family": "gpt",
  "harness": "codex",
  "modalities": ["text", "shell"],
  "posture": "attended",
  "completion": "manual-checkpoint",
  "launch_command": "codex",
  "args": []
}
```

Profiles are local operational configuration. They may include display labels
and optional launch hints, but only the coarse `family`, `harness`,
`modalities`, and `posture` fields are projected into `ResolverContext`.

### 12.2 RegisteredSession JSON

```json
{
  "type": "session_registered",
  "at": "2026-07-09T14:30:12Z",
  "session_id": "implementer",
  "profile_id": "codex",
  "cwd": "/Users/chetanconikee/tulving/waggle",
  "worktree": null,
  "tmux": {
    "session": "waggle",
    "window": "wg-task-20260709-001",
    "pane": "%3"
  },
  "state": "running",
  "current_task": "wg-task-20260709-001",
  "last_checkpoint_token": null
}
```

### 12.3 CheckpointRecord JSON

```json
{
  "type": "checkpoint_created",
  "at": "2026-07-09T14:45:20Z",
  "checkpoint_id": "ckpt-20260709-144520-codex-impl",
  "task_id": "wg-task-20260709-001",
  "source_session": "implementer",
  "source_profile": "codex",
  "source_family": "gpt",
  "source_harness": "codex",
  "status": "ready-for-review",
  "parent_checkpoint_token": "<token>",
  "checkpoint_token": "<token>",
  "artifacts": [
    {
      "kind": "diff",
      "path": ".waggle-handoffs/wg-task-20260709-001/ckpt-20260709-144520-codex-impl/diff.patch",
      "token": "<token>"
    },
    {
      "kind": "test-log",
      "path": ".waggle-handoffs/wg-task-20260709-001/ckpt-20260709-144520-codex-impl/test.log",
      "token": "<token>"
    }
  ],
  "next_session": "planner"
}
```

### 12.4 PendingHandoff JSON

```json
{
  "type": "handoff_pending",
  "at": "2026-07-09T14:46:00Z",
  "task_id": "wg-task-20260709-001",
  "from": "implementer",
  "to": "planner",
  "checkpoint_token": "<token>",
  "intent": "review",
  "state": "pending"
}
```

When `waggle-tmux next --resume` is run, append:

```json
{
  "type": "handoff_resumed",
  "at": "2026-07-09T14:47:15Z",
  "task_id": "wg-task-20260709-001",
  "session_id": "planner",
  "checkpoint_token": "<token>"
}
```

### 12.5 SwitchboardState

The replayed state should be a few maps:

```rust
pub struct SwitchboardState {
    pub profiles: BTreeMap<ProfileId, HarnessProfile>,
    pub sessions: BTreeMap<SessionId, RegisteredSession>,
    pub checkpoints: BTreeMap<CheckpointId, CheckpointRecord>,
    pub pending_by_session: BTreeMap<SessionId, Vec<PendingHandoff>>,
    pub current_task: Option<TaskId>,
}
```

This makes `waggle-tmux next` cheap and deterministic:

1. choose the pending handoff for `--session`, `--task`, or the latest pending
   handoff globally;
2. look up the destination `RegisteredSession`;
3. select its tmux pane;
4. print the checkpoint token and resolve line.

## 13. CLI Design

### 13.1 `init`

```sh
waggle-tmux init
```

Does:

- verifies `tmux -V`
- verifies `waggle --help`
- creates `.waggle/tmux/`
- creates `.waggle-handoffs/`
- writes default config if missing
- records the active tmux session name when run inside tmux
- does not launch Claude Code, Codex, or any other harness

Default config:

```toml
[switchboard]
tmux_session = "waggle"
handoff_dir = ".waggle-handoffs"
state_dir = ".waggle/tmux"
transcript_tail_lines = 2000
transcript_tail_bytes = 65536

[profiles.claude-code]
display_name = "Claude Code"
family = "claude"
harness = "claude-code"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = "claude"
args = []

[profiles.codex]
display_name = "Codex"
family = "gpt"
harness = "codex"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = "codex"
args = []

[profiles.generic]
display_name = "Generic Agent"
family = "other"
harness = "other"
modalities = ["text", "shell"]
posture = "attended"
completion = "manual-checkpoint"
launch_command = ""
args = []
```

### 13.2 `profile`

```sh
waggle-tmux profile list
waggle-tmux profile add opencode \
  --family other \
  --harness opencode \
  --modalities text,shell \
  --posture attended
```

Profiles describe what a harness is. They do not identify a live terminal.

Profile fields:

```text
display_name      Human display label.
family            Coarse model family: claude | gpt | gemini | other | custom slug.
harness           Harness slug: claude-code | codex | aider | opencode | custom slug.
modalities        text, shell, browser, vision, audio.
posture           attended | headless | ci.
completion        manual-checkpoint first; process-exit/sentinel later.
launch_command    Optional convenience command, not required for register/switch.
args              Optional launch args, not used by the simple MVP.
```

### 13.3 `register`

```sh
waggle-tmux register planner --profile claude-code --pane %1
waggle-tmux register implementer --profile codex --pane %2
```

or, from inside the active pane:

```sh
waggle-tmux register planner --profile claude-code --current-pane
```

Options:

```text
<session-id>           Local name, for example planner or implementer.
--profile <name>       Harness profile from config.
--pane <tmux-pane-id>  tmux pane id, for example %1.
--current-pane         Discover the active pane from TMUX.
--cwd <path>           Working directory; default pane current path if known.
--task <task-id>       Attach to task; creates one if omitted.
--worktree <path>      Optional existing worktree path.
```

Behavior:

- validates that the profile exists
- validates that the tmux pane exists
- stores `session id -> profile -> pane`
- starts pipe-pane logging if configured
- records local `session-registered`
- does not start or control the harness process

### 13.4 `next`

```sh
waggle-tmux next
```

Behavior:

- finds the latest pending checkpoint whose `next_session` is set
- selects the tmux pane for that session
- prints the checkpoint token and resolve line
- records `handoff-resumed` when requested with `--resume`

Useful flags:

```text
--task <id>       Restrict to one task.
--session <id>    Jump to that session's pending checkpoint.
--print-only      Print the next target/token without switching tmux.
--resume          Record handoff-resumed for the checkpoint.
```

### 13.5 `handoff`

```sh
waggle-tmux handoff implementer \
  --checkpoint <checkpoint-token> \
  --intent implement
```

Behavior:

- records that the checkpoint is queued for the destination session
- records `handoff-sent` on the checkpoint token if one exists
- prints the exact short resolve line for the user to use inside the target
  harness prompt
- does not inject text into the target pane by default

Example output:

```text
Next session: implementer
Checkpoint: <checkpoint-token>

In the Codex prompt, run or ask:
  Resolve <checkpoint-token> with Waggle and continue the implementation.
```

Optional automation can be added later as a separate explicit command:

```sh
waggle-tmux inject implementer --checkpoint <checkpoint-token>
```

`inject` must stay opt-in because the intended workflow is interactive
harness control, not remote-controlling another prompt from the switchboard.

### 13.6 `checkpoint`

```sh
waggle-tmux checkpoint \
  --session implementer \
  --status ready-for-review \
  --summary "Parser change implemented; one test still failing." \
  --artifact .waggle-handoffs/wg-task-20260709-001/diff.patch \
  --artifact .waggle-handoffs/wg-task-20260709-001/test.log \
  --next planner
```

Behavior:

- validates artifact paths
- captures optional transcript tail
- captures optional git state
- mints artifacts
- writes checkpoint file
- mints checkpoint file
- records `checkpoint-created`
- if `--next` is present, records `handoff-sent` and stores a pending
  switchboard handoff

Useful flags:

```text
--session <id>             Source session.
--task <id>                Task id, default session current task.
--status <slug>            ready-for-review | needs-human | blocked | done | custom.
--summary <text>           Inline summary.
--summary-file <path>      Summary from file.
--artifact <path>          Repeatable.
--artifact-kind <kind>     Optional kind for previous artifact.
--include-transcript       Capture bounded transcript tail.
--include-git-diff         Write and mint git diff.
--include-git-status       Write and mint git status.
--next <session-id>        Mark this checkpoint as next for another session.
--parent <token>           Override parent checkpoint.
```

### 13.7 `switch`

```sh
waggle-tmux switch planner
```

Behavior:

- selects the tmux pane/window for session
- prints current task and latest checkpoint token

### 13.8 `status`

```sh
waggle-tmux status
```

Example output:

```text
Task wg-task-20260709-001

SESSION       PROFILE      FAMILY  STATE    PANE  LAST CHECKPOINT  NEXT
planner       claude-code  claude  waiting  %2    <token>          implementer
implementer   codex        gpt     running  %3    <token>          planner

Next checkpoint:
  <token>
  status: ready-for-review
  artifacts: diff, test-log, transcript-tail
```

### 13.9 `watch`

```sh
waggle-tmux watch
```

Behavior:

- tails pane logs
- parses sentinel lines
- turns sentinel lines into checkpoint commands
- records pending handoffs for destination sessions

This can be added after manual `checkpoint` works.

### 13.10 `resume`

```sh
waggle-tmux resume --checkpoint <token> --session implementer
```

Behavior:

- resolves checkpoint
- creates or selects destination session
- prints the short resolve line for use inside the selected harness prompt
- records `handoff-resumed`

## 14. Prompt Templates

Prompt templates should be plain files under:

```text
.waggle/tmux/templates/
```

### 14.1 Generic Resume Prompt

```text
You are continuing a local coding task through Waggle.

First resolve this checkpoint:
  {{checkpoint_token}}

Use these commands as needed:
  waggle resolve --token {{checkpoint_token}}
  waggle read --token {{checkpoint_token}} --max-bytes 12000
  waggle search --token {{checkpoint_token}} --pattern "<term>"

Do not ask the user to paste context that is already in the checkpoint.
Read the checkpoint, inspect only the artifacts you need, then continue.

When you reach a durable stopping point, write artifacts under:
  {{handoff_dir}}/{{task_id}}/{{next_checkpoint_id}}/

Then create a handoff checkpoint:
  waggle-tmux checkpoint --session {{session_id}} --status <status> --next <next-session>
```

### 14.2 Implementer Prompt

```text
You are the implementer for task {{task_id}}.

Resolve and read:
  {{checkpoint_token}}

Expected output:
  - a minimal implementation
  - a diff artifact
  - a test log artifact
  - a short checkpoint summary

Prefer precise code edits and focused tests. Use rg to inspect the repository.
Do not perform unrelated refactors.
```

### 14.3 Reviewer Prompt

```text
You are the reviewer for task {{task_id}}.

Resolve and read:
  {{checkpoint_token}}

Review the implementation artifacts. Focus on:
  - behavioral bugs
  - missing tests
  - unsafe assumptions
  - mismatch with the requested scope

Write a review artifact and checkpoint it back to the implementer if changes
are needed.
```

## 15. File Layout

Recommended local files:

```text
.waggle/
  tmux/
    config.toml
    events.jsonl
    state.json
    panes/
      planner.log
      implementer.log
    transcripts/
      wg-task-20260709-001/
    templates/
      resume.txt
      implement.txt
      review.txt

.waggle-handoffs/
  wg-task-20260709-001/
    ckpt-20260709-143012-claude-planner/
      checkpoint.md
      plan.md
      transcript-tail.txt
    ckpt-20260709-144520-codex-implementer/
      checkpoint.md
      diff.patch
      test.log
      git-status.txt
      transcript-tail.txt
```

`.waggle-handoffs/` should probably be gitignored by default. The artifacts are
for agent handoff, not necessarily source history. If a project wants durable
review records in git, it can opt in.

## 16. Rust Implementation Details

### 16.1 Types

```rust
pub struct HarnessProfile {
    pub id: ProfileId,
    pub display_name: String,
    pub family: String,
    pub harness: String,
    pub modalities: Vec<Modality>,
    pub posture: Posture,
    pub completion: CompletionMode,
    pub launch_command: Option<String>,
    pub args: Vec<String>,
}

pub struct RegisteredSession {
    pub id: SessionId,
    pub profile: ProfileId,
    pub cwd: PathBuf,
    pub worktree: Option<PathBuf>,
    pub tmux: TmuxPane,
    pub state: SessionState,
    pub current_task: Option<TaskId>,
    pub last_checkpoint: Option<String>,
}

pub struct TmuxPane {
    pub session: String,
    pub window: String,
    pub pane: String,
}

pub struct Checkpoint {
    pub id: CheckpointId,
    pub task_id: TaskId,
    pub source_session: SessionId,
    pub source_profile: ProfileId,
    pub source_family: String,
    pub source_harness: HarnessName,
    pub status: String,
    pub parent_token: Option<String>,
    pub checkpoint_token: String,
    pub artifacts: Vec<ArtifactRef>,
    pub next_session: Option<SessionId>,
}

pub struct PendingHandoff {
    pub task_id: TaskId,
    pub from: SessionId,
    pub to: SessionId,
    pub checkpoint_token: String,
    pub intent: String,
    pub state: HandoffState,
}

pub struct ArtifactRef {
    pub kind: String,
    pub path: PathBuf,
    pub token: String,
}
```

### 16.2 Tmux Backend Trait

```rust
pub trait TmuxBackend {
    fn current_pane(&self) -> anyhow::Result<TmuxPane>;
    fn pane_exists(&self, pane: &TmuxPane) -> anyhow::Result<bool>;
    fn send_text(&self, pane: &TmuxPane, text: &str) -> anyhow::Result<()>; // explicit inject only
    fn capture_tail(&self, pane: &TmuxPane, lines: usize) -> anyhow::Result<String>;
    fn pipe_pane_to_file(&self, pane: &TmuxPane, path: &Path) -> anyhow::Result<()>;
    fn select(&self, pane: &TmuxPane) -> anyhow::Result<()>;
    fn list_panes(&self) -> anyhow::Result<Vec<TmuxPaneInfo>>;
}
```

Use a fake implementation in unit tests. The real implementation should be a
small wrapper over `Command::new("tmux")`.

### 16.3 Profile Registry

```rust
pub trait ProfileRegistry {
    fn get(&self, profile: &ProfileId) -> anyhow::Result<HarnessProfile>;
    fn upsert(&self, profile: HarnessProfile) -> anyhow::Result<()>;
    fn resolver_context(&self, profile: &ProfileId) -> anyhow::Result<ResolverContext>;
}

pub enum CompletionMode {
    ManualCheckpoint,
    ProcessExit,
    Sentinel { prefix: String },
}
```

Profiles should be data-driven from TOML. Special harness code is an
optimization, not the core abstraction.

### 16.4 Waggle Client Trait

For MVP, call the `waggle` binary and parse JSON envelopes.

```rust
pub trait WaggleClient {
    fn mint_snapshot(
        &self,
        path: &Path,
        channel: &str,
        parent: Option<&str>,
    ) -> anyhow::Result<MintResult>;

    fn record(&self, token: &str, stage: &str) -> anyhow::Result<()>;
    fn resolve(&self, token: &str) -> anyhow::Result<serde_json::Value>;
    fn mutate(&self, token: &str, change: &str, expected_version: u32) -> anyhow::Result<()>;
}
```

Later, `waggle-tmux` can use `waggle_mcp::Handler` directly. Do not start there
unless the CLI envelope parsing becomes painful.

### 16.5 State Store

Start with:

```rust
pub trait StateStore {
    fn append(&self, event: SwitchboardEvent) -> anyhow::Result<()>;
    fn load(&self) -> anyhow::Result<SwitchboardState>;
}
```

Append to JSONL. Rebuild state by replaying events. Write `state.json`
opportunistically as a cache.

Events:

```rust
pub enum SwitchboardEvent {
    Initialized { at: DateTime<Utc> },
    ProfileUpserted { profile: HarnessProfile },
    SessionRegistered { session: RegisteredSession },
    SessionStateChanged { session_id: SessionId, state: SessionState },
    CheckpointCreated { checkpoint: Checkpoint },
    HandoffPending { handoff: PendingHandoff },
    HandoffResumed { session_id: SessionId, checkpoint_token: String },
    PaneDied { session_id: SessionId },
}
```

## 17. Worktree Strategy

The switchboard can register multiple harness panes in the same working
directory, but that is risky for coding tasks. Two harnesses can edit the same
files concurrently.

Support three modes:

```text
none              Use current working tree.
create            Create a git worktree per session.
existing:<path>   Use an existing worktree.
```

For MVP, default to `none` and print a warning when more than one writing
session is attached to the same cwd:

```text
planner and implementer share the same working tree. Concurrent edits can
conflict. Use --worktree create for isolated implementation sessions.
```

Later, a launch helper or worktree helper can run:

```sh
git worktree add ../waggle-worktrees/<task-id>-<session-id> -b <branch>
```

Checkpoint should include:

- branch name
- `git status --short`
- optional `git diff`
- optional commit sha if committed

Do not auto-commit unless explicitly requested.

## 18. Security And Safety

### 18.1 Do Not Mint Secrets By Accident

Checkpointing terminal sessions can capture secrets.

Defaults:

- do not mint full pane logs
- only mint bounded transcript tail when requested or configured
- redact obvious lines matching:
  - `*_TOKEN=`
  - `*_KEY=`
  - `*_SECRET=`
  - `Authorization:`
  - `Bearer `
- ignore files under `.env`, `.ssh`, `.aws`, `.gcloud`, `node_modules`,
  `target`, `.git`, and other large/generated directories
- reject artifacts outside the workspace unless `--allow-outside-workspace`

### 18.2 Do Not Auto-Approve Harness Actions

The switchboard should switch and checkpoint. It should not bypass the approval
model of Codex, Claude Code, or any other harness.

### 18.3 Make Failure Durable

If checkpoint creation fails after writing local files but before minting:

- keep the checkpoint directory
- append a local `checkpoint_failed` event
- print a retry command

Example:

```sh
waggle-tmux checkpoint retry --id ckpt-20260709-144520-codex-impl
```

If handoff presentation fails:

- do not discard tokens
- mark handoff as `failed`
- print the checkpoint token and recovery command

## 19. Error Handling

Common errors:

```text
tmux not installed
tmux server not running
pane id no longer exists
harness command not found
artifact path missing
artifact path outside workspace
waggle mint failed
waggle record failed
checkpoint token could not be resolved
destination session not found
```

User-facing errors should include the next command to recover.

Example:

```text
Destination session "planner" does not exist.

Register it:
  waggle-tmux register planner --profile claude-code --current-pane

Or inspect existing sessions:
  waggle-tmux status
```

## 20. Testing Plan

### 20.1 Unit Tests

Use fake backends.

Test:

- config parsing
- state replay
- task id generation
- checkpoint markdown rendering
- artifact path validation
- sentinel parsing
- prompt template rendering
- Waggle envelope parsing
- tmux command argument construction
- profile-to-resolver-context conversion
- pending handoff selection for `next`

### 20.2 Integration Tests Without Real Agents

Gate real tmux tests behind an environment variable:

```sh
WAGGLE_TMUX_TESTS=1 cargo test -p waggle-tmux
```

Test with shell commands instead of real agents:

```sh
tmux new-session -d -s waggle-test -n fake sh
waggle-tmux register fake --profile generic --pane %1
waggle-tmux handoff fake --checkpoint <checkpoint-token>
```

### 20.3 Integration Tests With Waggle

Use a temporary `WAGGLE_STORE`:

```sh
WAGGLE_STORE=/tmp/waggle-tmux-test/waggle.db cargo test -p waggle-tmux
```

Verify:

- artifact file minted
- checkpoint file minted
- record stage written
- checkpoint can be read by `waggle read`
- checkpoint can be searched by `waggle search`

### 20.4 Manual Dogfood

Manual flow:

```sh
waggle-tmux init
waggle-tmux register planner --profile claude-code --pane %1
waggle-tmux register implementer --profile codex --pane %2
waggle-tmux checkpoint --session planner --summary-file plan.md --artifact plan.md --next implementer
waggle-tmux next
waggle-tmux checkpoint --session implementer --include-git-diff --artifact test.log --next planner
waggle-tmux next
```

The dogfood is successful if the reviewer session can continue from the token
without the user pasting the plan, diff, or log.

## 21. MVP Implementation Phases

### Phase 0: Manual Protocol

No new code.

- create `.waggle-handoffs/`
- write checkpoint files manually
- mint them with `waggle mint --snapshot`
- manually resolve tokens inside Codex and Claude Code

This validates the prompt shape.

### Phase 1: Minimal `waggle-tmux` Crate

Implement:

- `init`
- `profile list`
- `profile add`
- `register`
- `switch`
- `next`
- `status`
- raw tmux backend
- profile registry
- JSONL state store
- config loading

No harness launching and no automatic checkpointing yet.

### Phase 2: Checkpoint Command

Implement:

- `checkpoint`
- artifact validation
- artifact minting
- checkpoint file rendering
- checkpoint minting
- stage recording

This is the first real value milestone.

### Phase 3: Handoff Queue And Manual Resume

Implement:

- `handoff`
- `resume`
- prompt templates
- `handoff-sent`
- `handoff-resumed`

At this point, the user can move between Claude Code and Codex with one
Waggle-aware switchboard command:

```sh
waggle-tmux next
```

### Phase 4: Watcher And Sentinel

Implement:

- `watch`
- pane log tailing
- sentinel parser
- automatic checkpoint on `WAGGLE_DONE`

This enables completion-based switching.

### Phase 5: Worktree Support

Implement:

- `--worktree create`
- branch naming
- worktree cleanup status
- git status/diff artifacts

Do not auto-merge.

### Phase 6: TUI

Optional.

Use `ratatui` and `crossterm` if a TUI is worth it.

Views:

- sessions
- task queue
- latest checkpoints
- artifact list
- pending handoffs
- pane selector

Keep the CLI complete even if the TUI exists.

## 22. Differentiation And Prior Art

Known nearby tool:

- Claude Squad: https://github.com/smtg-ai/claude-squad

Claude Squad is a tmux/git-worktree TUI for running multiple AI coding agents.
It is close to the session-management part of this idea.

Waggle should not compete by cloning the pane manager. The differentiation is:

- checkpointed handoffs, not just concurrent sessions
- artifact tokens, not pane transcripts
- `read` and `search` over handoff artifacts
- supersedable and revocable handoff records
- harness-neutral continuation across Codex, Claude Code, and generic agents
- eventual MCP compatibility because Waggle already exposes MCP tools

Relevant substrate docs:

- tmux man page and control mode: https://man7.org/linux/man-pages/man1/tmux.1.html
- Rust tmux interface crate: https://docs.rs/tmux_interface/latest/tmux_interface/

The niche is:

```text
durable local handoffs for grep-first coding agents
```

Not:

```text
another RAG system
another vector database
another all-in-one agent framework
```

## 23. Real Use Cases To Validate

### 23.1 Claude Plans, Codex Implements

Flow:

1. Claude Code investigates architecture and writes `plan.md`.
2. `waggle-tmux checkpoint --session planner --artifact plan.md --next implementer`
3. Codex resolves the checkpoint and implements the patch.
4. Codex checkpoints `diff.patch` and `test.log` back to Claude.
5. Claude reviews.

Why Waggle matters:

- plan token is stable
- implementation session does not need pasted context
- reviewer can inspect exact diff/test artifacts

### 23.2 Codex Runs Tests, Claude Debugs Failure

Flow:

1. Codex runs focused tests.
2. It writes `test.log` and `repro.md`.
3. It checkpoints to Claude with status `blocked`.
4. Claude searches the log token for error signatures.

Why Waggle matters:

- large logs are not pasted into chat
- receiving harness can use `waggle search`
- stale logs can be superseded

### 23.3 Claude Reviews, Codex Applies Fixups

Flow:

1. Codex checkpoints a diff.
2. Claude checkpoints a review artifact.
3. Codex resolves the review token and applies fixups.

Why Waggle matters:

- review comments are artifacts
- fixup checkpoint can point to review checkpoint as parent
- funnel shows the review actually got consumed

### 23.4 Multi-File Refactor Coordination

Flow:

1. Planner session mints a file inventory and migration plan.
2. Implementer session handles one module.
3. Reviewer session checks blast radius.
4. Test session captures failures.

Why Waggle matters:

- each stage produces a compact checkpoint
- each harness can read/search only what it needs
- task lineage does not depend on terminal scrollback

### 23.5 Bug Repro Bundle

Flow:

1. One agent creates a minimal repro, command transcript, and failing output.
2. Another agent fixes from the repro bundle.
3. A third session reviews the fix against the repro.

Why Waggle matters:

- repro is an inspectable artifact
- failing output is searchable
- fix checkpoint can supersede a failed attempt

## 24. How This Helps When Agents Already Use `rg`

`rg` is still the best local sensing tool for code. Waggle should not compete
with it.

The wrapper should encourage this pattern:

1. Agent uses `rg` to inspect files quickly.
2. Agent writes the important result to an artifact.
3. `waggle-tmux checkpoint` snapshots the artifact.
4. Another harness resolves the checkpoint and continues.

Example:

```sh
rg -n "ResolverContext|ConsumerHint|negotiate" crates docs > .waggle-handoffs/wg-task-001/rg-context.txt
waggle-tmux checkpoint --session planner --artifact .waggle-handoffs/wg-task-001/rg-context.txt --next implementer
```

The value is not replacing `rg`; it is making the result of that sensing step
portable, attributable, and durable across harnesses.

## 25. Open Design Questions

1. Should this be `crates/waggle-tmux` or `waggle session` inside the main CLI?
   Recommendation: start separate.

2. Should checkpoints become a first-class Waggle operation?
   Recommendation: no for MVP. Use minted artifacts and custom stages.

3. Should local switchboard state use SQLite?
   Recommendation: no for MVP. Use JSONL replay.

4. Should completion detection use tmux control mode?
   Recommendation: not first. Start with explicit command, process exit, and
   sentinel parsing.

5. Should worktrees be default?
   Recommendation: no for MVP. Warn on shared cwd, support `--worktree create`
   soon after.

6. Should the wrapper support existing Claude Squad sessions?
   Recommendation: later. First prove Waggle checkpoints with native tmux
   sessions. Then consider an import/adapter mode.

## 26. Concrete Next Steps For Implementation

Start here:

1. Add `crates/waggle-tmux` to the workspace.
2. Add dependencies:
   - `clap`
   - `serde`
   - `serde_json`
   - `toml`
   - `anyhow`
   - `time` or `chrono`
   - `camino` optional
   - `tempfile` for tests
3. Implement `config.rs`.
4. Implement fakeable `TmuxBackend`.
5. Implement `init`, `profile list`, `profile add`, `register`, `switch`,
   `next`, and `status`.
6. Implement `StateStore` over `.waggle/tmux/events.jsonl`.
7. Implement `WaggleClient` over the `waggle` binary.
8. Implement `checkpoint`.
9. Add integration tests gated by `WAGGLE_TMUX_TESTS=1`.
10. Dogfood Claude-to-Codex and Codex-to-Claude on this repository.

Suggested first milestone PR:

```text
Add waggle-tmux MVP with profiles, pane registration, next, status, and JSONL state
```

Suggested second milestone PR:

```text
Add checkpoint minting and manual handoff queue
```

## 27. Copy/Paste Prompt For The Next Agent Session

Use this when switching to another tab or harness:

```text
Read /Users/chetanconikee/tulving/waggle/design/tmux/README.md.

Implement the Phase 1 MVP for waggle-tmux:

- add a new workspace crate at crates/waggle-tmux
- create a binary named waggle-tmux
- implement init, profile list/add, register, switch, next, and status
- use raw std::process::Command calls to tmux through a small TmuxBackend trait
- do not launch or own Claude Code/Codex in the MVP; register existing panes
- store local switchboard events in .waggle/tmux/events.jsonl
- do not change waggle-core or waggle-mcp
- do not implement the TUI yet
- include unit tests with a fake tmux backend
- gate real tmux integration tests behind WAGGLE_TMUX_TESTS=1

After Phase 1 works, implement checkpoint as Phase 2:

- validate artifact paths
- mint artifacts with waggle mint --snapshot
- render checkpoint.md
- mint checkpoint.md
- record checkpoint-created
- store the next session as a pending handoff consumed by waggle-tmux next

Keep the implementation small and dogfoodable. The goal is durable handoff
between Claude Code and Codex sessions, not a full agent framework.
```

## 28. Done Criteria

The extension is useful when this works end to end:

1. User starts Claude Code and Codex in tmux however they prefer.
2. User registers both panes with profiles.
3. Claude writes a plan artifact.
4. `waggle-tmux checkpoint --next implementer` mints the plan and stores a
   pending handoff.
5. `waggle-tmux next` switches tmux control to Codex and prints the resolve
   line.
6. Codex reads/searches the checkpoint through Waggle.
7. Codex produces a diff/test-log checkpoint and points `--next planner`.
8. `waggle-tmux next` switches back to Claude.
9. `waggle funnel` shows the checkpoint was produced and consumed.

That is the wedge: a durable local handoff path across harnesses that already
live in the terminal.
