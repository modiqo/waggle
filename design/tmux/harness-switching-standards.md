# Harness Switching Interface Standards

Status: design contract for a future `waggle-tmux` implementation.

Purpose: define the standards for a harness-switching interface that can move
work across Claude Code, Codex, and other agent harnesses while preserving the
engineering discipline already present in Waggle.

This document is intentionally stricter than the product sketch in
`design/tmux/README.md`. It describes what must be true for the implementation
to belong in this repository.

## 1. The Correct Interaction Model

The primary workflow is manual-first:

1. The user starts a tmux session and opens harness panes however they prefer.
2. The user registers each live pane with a harness profile.
3. The user switches into a harness pane, for example Claude Code.
4. The user interacts naturally inside that harness prompt.
5. The harness writes one or more handoff artifacts.
6. The user or harness runs `waggle-tmux checkpoint`.
7. `waggle-tmux` mints a durable checkpoint and marks the next session.
8. The user runs `waggle-tmux next`.
9. `waggle-tmux` switches tmux control to the next registered harness pane.
10. The user asks that harness to resolve the checkpoint token with Waggle.

The switchboard does not need to remote-control prompts to be useful.

The default handoff command should therefore look like this:

```sh
waggle-tmux register planner --profile claude-code --pane %1
waggle-tmux register implementer --profile codex --pane %2

waggle-tmux checkpoint \
  --session planner \
  --artifact .waggle-handoffs/wg-task-20260709-001/plan.md \
  --next implementer

waggle-tmux next
```

Inside Codex, the user can then type:

```text
Resolve <checkpoint-token> with Waggle and continue from it.
```

Prompt injection through `tmux send-keys` may exist later as an explicit
`inject` command, but it is not the core interface. The core interface is
profile, register, checkpoint, next, resolve.

## 2. Simple Switchboard MVP

The first implementation should be a switchboard, not an agent manager.

It should do:

- keep a profile registry for harness types
- register existing tmux panes as named sessions
- mint checkpoint artifacts through Waggle
- store pending handoffs locally
- select the next tmux pane from pending handoff state
- print the resolve line for the human to use inside the target harness

It should not do first:

- launch and own harness processes
- manage a full TUI
- inject prompts by default
- infer completion from screen scraping
- create git worktrees automatically
- add new `waggle-core` primitives

The MVP command spine is:

```sh
waggle-tmux init
waggle-tmux profile list
waggle-tmux register <session> --profile <profile> --pane <pane>
waggle-tmux checkpoint --session <session> --artifact <path> --next <session>
waggle-tmux next
waggle-tmux status
```

## 3. Non-Negotiable Fit With Waggle

The implementation must respect Waggle's existing shape:

- `waggle-core` remains transport-independent.
- `ResolverContext` remains the neutral schema for model family, harness,
  modalities, and posture.
- rich harness detection lives at the edge, not in the sealed matcher.
- event records never gain payload fields.
- analytics use coarse actor classes only.
- the switchboard must not introduce a second artifact protocol.
- checkpointing must be expressible as normal Waggle mints, records, and
  mutations before any new core primitive is proposed.

If an implementation needs to violate one of these, the design is not ready.

## 4. Interface Boundary

Use three layers.

```text
Layer 1: Harness profiles
  Local, rich, operational details:
  command, args, cwd, tmux pane, model family hint, harness name,
  completion mode, worktree policy, transcript policy.

Layer 2: Resolver context
  The neutral Waggle shape:
  kind, model_family, harness, modalities, posture.

Layer 3: Event actor class
  The analytics downgrade:
  agent/human/terminal/bot + coarse family class + coarse harness class.
```

The direction of information is one-way:

```text
HarnessProfile -> ResolverContext -> ActorClass
```

Never infer back upward. A coarse `ActorClass::Gpt` event must not be used to
reconstruct that the exact model was `gpt-5-codex-2026-07-09`, and no event
should ever store that exact string.

## 5. Harness Profile Standard

The local switchboard needs richer data than Waggle core. That is fine as long
as the data stays local.

Suggested profile shape:

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
launch_command = "aider"
args = []
```

Rules:

- `family` and `harness` are normalized slugs before use.
- model versions are allowed in local config only if stored under a separate
  non-analytic field such as `local_model_label`.
- `ResolverContext.model_family` receives only the family slug.
- `ResolverContext.harness` receives only the harness slug.
- unknown harnesses should still work through `family = "other"` and
  `harness = "other"` or explicit user-provided slugs.
- the implementation must not hardcode Claude and Codex as the only options.

The user-facing promise is "any harness," not "two harnesses with special
cases."

## 6. Model Family Standard

Model family is a routing and projection dimension, not an identity claim.

Acceptable family slugs:

- `claude`
- `gpt`
- `gemini`
- `other`
- future coarse families as reviewed additions

Unacceptable values for `ResolverContext.model_family`:

- `claude-opus-4.1`
- `gpt-5-codex-2026-07-09`
- provider deployment ids
- account-specific model aliases
- endpoint names

Local profile data may remember exact labels for display and debugging, but the
Waggle context and event path must stay coarse.

This matches the existing `ActorClass` downgrade in `crates/waggle-core/src/event.rs`.

## 7. Harness Standard

Harness is the tool environment, not the model.

Acceptable harness slugs:

- `claude-code`
- `codex`
- `aider`
- `opencode`
- `cursor`
- `other`

The exact taxonomy should stay open. Do not add a Rust enum for every harness
in the first implementation. Use a validated slug in switchboard state and
downcast only when creating `ActorClass`.

Why: the repository already treats `ResolverContext.harness` as `Option<String>`.
The sealed matcher accepts harness constraints as data. The switchboard should
not narrow that design prematurely.

## 8. Manual Completion Standard

The primary completion mode is explicit checkpointing:

```sh
waggle-tmux checkpoint --session <session> --status <status> --next <session>
```

This must work even if:

- the harness exposes no machine-readable completion signal
- the terminal prompt shape changes
- the harness is a new CLI unknown to Waggle
- the user has interacted manually for an hour

Other completion modes are optional:

- `process-exit` for non-interactive runs
- `sentinel` for agents that can print `WAGGLE_DONE {...}`
- `inject` for explicit tmux `send-keys`

Do not make screen scraping a correctness dependency. It can support a TUI idle
indicator, but it must not be the source of durable state.

## 9. Checkpoint Standard

A checkpoint is a minted artifact, not a hidden row in a switchboard database.

Required properties:

- writes a human-readable `checkpoint.md`
- mints every explicit artifact with `waggle mint --snapshot`
- mints the checkpoint file with `waggle mint --snapshot`
- records `checkpoint-created`
- records `handoff-sent` when a next session is selected
- records `handoff-resumed` when the next session begins using it
- stores local switchboard state as replayable events

The checkpoint file should include:

- task id
- checkpoint id
- source session
- source harness
- source model family
- status
- parent checkpoint token
- artifact tokens
- next session
- concise next action

It should not include:

- full terminal transcript by default
- secrets
- provider account identifiers
- exact model version in an analytics position
- unbounded logs

## 10. Switchboard State Standard

Use append-only local state first:

```text
.waggle/tmux/events.jsonl
```

Derived state may be cached:

```text
.waggle/tmux/state.json
```

The log should contain operational facts:

- profile upserted
- session registered
- session selected
- checkpoint created
- handoff marked next
- handoff resumed
- pane missing
- retry attempted

The log should not become a second Waggle. Artifact identity belongs to Waggle
tokens. Switchboard state only remembers local session mechanics and pointers
to tokens.

Effective data structures:

```rust
pub struct SwitchboardState {
    pub profiles: BTreeMap<ProfileId, HarnessProfile>,
    pub sessions: BTreeMap<SessionId, RegisteredSession>,
    pub checkpoints: BTreeMap<CheckpointId, CheckpointRecord>,
    pub pending_by_session: BTreeMap<SessionId, Vec<PendingHandoff>>,
    pub current_task: Option<TaskId>,
}
```

The important joins are:

```text
RegisteredSession.profile_id -> HarnessProfile
CheckpointRecord.next_session -> RegisteredSession
PendingHandoff.checkpoint_token -> Waggle token
```

`waggle-tmux next` should be a deterministic read of this state:

1. find the latest pending handoff;
2. find the destination registered session;
3. select its tmux pane;
4. print the checkpoint token and resolve line.

## 11. API Naming Standard

Names should match the user workflow.

Use:

- `profile`: manage harness profiles
- `register`: bind a live tmux pane to a profile
- `switch`: select tmux control for a session
- `next`: select the pending destination session from checkpoint state
- `checkpoint`: mint durable handoff state
- `handoff`: mark a checkpoint as next for another session
- `resume`: select a session and display the checkpoint it should resolve
- `inject`: optional explicit automation

Avoid making `send` central. It implies the switchboard pushes instructions
into another prompt. That is not the target workflow.

Avoid making `route` central. It sounds like automated orchestration. The first
product is a human-directed switchboard.

## 12. Adapter Standard

Every external tool integration needs a narrow trait and fake implementation.

Suggested traits:

```rust
pub trait TmuxBackend {
    fn current_pane(&self) -> anyhow::Result<TmuxPane>;
    fn pane_exists(&self, pane: &TmuxPane) -> anyhow::Result<bool>;
    fn select(&self, pane: &TmuxPane) -> anyhow::Result<()>;
    fn capture_tail(&self, pane: &TmuxPane, lines: usize) -> anyhow::Result<String>;
}

pub trait ProfileRegistry {
    fn get(&self, name: &ProfileId) -> anyhow::Result<HarnessProfile>;
    fn upsert(&self, profile: HarnessProfile) -> anyhow::Result<()>;
    fn context_for(&self, name: &ProfileId) -> anyhow::Result<ResolverContext>;
}

pub trait WaggleClient {
    fn mint_snapshot(&self, path: &Path, channel: &str, parent: Option<&str>)
        -> anyhow::Result<MintResult>;
    fn record(&self, token: &str, stage: &str) -> anyhow::Result<()>;
    fn resolve(&self, token: &str) -> anyhow::Result<serde_json::Value>;
}
```

Rules:

- no shell command strings built by concatenating user input
- no tests that require real Claude Code or Codex by default
- real tmux tests gated by an environment variable
- fake tmux and fake Waggle clients for unit tests
- exact command argv covered by tests

## 13. Review Gates

A first implementation should not be accepted unless these pass:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- unit tests for profile parsing
- unit tests for model family normalization
- unit tests for harness slug normalization
- unit tests for `HarnessProfile -> ResolverContext`
- unit tests proving model version strings do not enter `ResolverContext`
- unit tests for checkpoint rendering
- unit tests for fake tmux selection
- unit tests for pending handoff selection in `next`
- unit tests for session registration against fake tmux panes
- unit tests for replaying `.waggle/tmux/events.jsonl`

Optional integration gates:

```sh
WAGGLE_TMUX_TESTS=1 cargo test -p waggle-tmux
```

Use real tmux only in these gated tests.

## 14. Compatibility With Existing Design Docs

This interface should explicitly align with:

- `docs/design/02-domain-model.md`: token, channel, stage, event, actor
  class, resolver context, invariants.
- `docs/design/06-agent-coordination.md`: resolver context as neutral schema,
  adapters at the edge, deterministic variant selection.
- `docs/design/16-deployment-topologies.md`: one local daemon shared by every
  harness on the machine.
- `docs/design/17-agent-fluency.md`: tool output teaches next steps; guidance
  must not drift from executable operations.
- `docs/design/18-content-access.md`: receiving harness should use
  `read`/`search` over minted content instead of pasted context.

If `waggle-tmux` eventually adds commands to the main `waggle` CLI, it must
follow the operations-catalog discipline. No standalone clap-only command
surface should be added to the main binary.

## 15. What Must Stay Out Of Core

Do not put these in `waggle-core`:

- tmux pane ids
- local process ids
- harness launch commands
- exact model labels
- terminal transcript paths
- switchboard UI state
- completion sentinels
- prompt templates

These are host/switchboard concerns. Core should continue to know only the
neutral resolver schema and artifact/token semantics.

## 16. Minimal Acceptance Scenario

The implementation earns its place when this works:

1. User starts Claude Code and Codex in tmux however they prefer.
2. User registers `planner` as the Claude Code pane.
3. User registers `implementer` as the Codex pane.
4. User switches to `planner` and interacts normally.
5. Planner writes `plan.md`.
6. User runs:

   ```sh
   waggle-tmux checkpoint --session planner --artifact plan.md --next implementer
   ```

7. The command prints a checkpoint token and records it as next for
   `implementer`.
8. User runs:

   ```sh
   waggle-tmux next
   ```

9. `waggle-tmux next` selects the registered Codex pane and prints the resolve
   line.
10. User asks Codex:

   ```text
   Resolve <checkpoint-token> with Waggle and continue.
   ```

11. Codex uses `waggle read` or `waggle search` to inspect the checkpoint and
   artifacts.
12. Codex writes a diff/test log and checkpoints back to `planner`.

No prompt injection is required. No pasted context is required. No special
Claude/Codex-only path is required.

## 17. Implementation North Star

The interface should feel like tmux for control and Waggle for memory.

tmux answers:

- where is my live harness?
- which pane do I control now?
- which sessions are still alive?

Waggle answers:

- what artifact was handed off?
- who produced it?
- what should the next harness resolve?
- what superseded it?
- which harness/model family consumed it?

Do not blur those responsibilities. That separation is how this can support
any harness across model families without lowering the standards of the repo.
