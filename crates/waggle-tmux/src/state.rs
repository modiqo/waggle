//! Switchboard state: append-only JSONL replayed into maps — the same
//! shape as waggle's own log discipline, at control-plane scale. Tokens
//! are the durable identity; this file is only what the switchboard
//! needs to pick panes and pending handoffs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use std::io::Write as _;

use crate::error::{Error, Result};

/// One event; the file is the truth, `State` is the fold.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    /// A pane became a named session.
    SessionRegistered {
        /// Local session id (e.g. `claude-code`).
        id: String,
        /// Profile id.
        profile: String,
        /// tmux pane id (`%N`).
        pane: String,
        /// Created by `up` → injection allowed.
        owned: bool,
    },
    /// An outcome token was minted (or adopted); becomes pending.
    OutcomeMinted {
        /// The waggle token.
        token: String,
        /// What it names.
        target: String,
        /// Destination session, when the minter declared one.
        to: Option<String>,
    },
    /// `switch` delivered the token; the funnel baseline lets `status`
    /// detect the DESTINATION's own resolve later (consumption).
    HandoffSent {
        /// The token delivered.
        token: String,
        /// Destination session id.
        to: String,
        /// Resolve-stage count at delivery time (preview included).
        resolves_at_send: u64,
    },
    /// Focus moved.
    SwitchedTo {
        /// Destination session id.
        id: String,
    },
}

/// A registered session, replayed.
#[derive(Debug, Clone)]
pub struct Session {
    /// Profile id.
    pub profile: String,
    /// tmux pane id.
    pub pane: String,
    /// Injection permission (seamless §3.4).
    pub owned: bool,
    /// Last token delivered TO this session, with its baseline.
    pub last_delivery: Option<(String, u64)>,
}

/// The fold over the event log.
#[derive(Debug, Default)]
pub struct State {
    /// session id → session.
    pub sessions: BTreeMap<String, Session>,
    /// The pending handoff: newest minted, not yet delivered.
    pub pending: Option<(String, String, Option<String>)>, // (token, target, to)
    /// Where focus last went.
    pub focused: Option<String>,
}

/// Where state lives, relative to a workspace.
#[must_use]
pub fn events_path(workspace: &Path) -> PathBuf {
    workspace.join(".waggle/tmux/events.jsonl")
}

/// Append one event (creating parents).
pub fn append(workspace: &Path, event: &Event) -> Result<()> {
    let path = events_path(workspace);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let line = serde_json::to_string(event).map_err(|e| Error::State(e.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Replay the log into current state. Unknown lines are skipped with a
/// note (forward compatibility) — never a crash.
#[must_use]
pub fn load(workspace: &Path) -> State {
    let path = events_path(workspace);
    let mut state = State::default();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return state;
    };
    for line in raw.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(event) = serde_json::from_str::<Event>(line) else {
            eprintln!("waggle-tmux: skipping unknown state line (newer version?)");
            continue;
        };
        apply(&mut state, event);
    }
    state
}

fn apply(state: &mut State, event: Event) {
    match event {
        Event::SessionRegistered {
            id,
            profile,
            pane,
            owned,
        } => {
            state.sessions.insert(
                id,
                Session {
                    profile,
                    pane,
                    owned,
                    last_delivery: None,
                },
            );
        }
        Event::OutcomeMinted { token, target, to } => {
            state.pending = Some((token, target, to));
        }
        Event::HandoffSent {
            token,
            to,
            resolves_at_send,
        } => {
            if state.pending.as_ref().is_some_and(|(t, _, _)| *t == token) {
                state.pending = None;
            }
            if let Some(session) = state.sessions.get_mut(&to) {
                session.last_delivery = Some((token, resolves_at_send));
            }
        }
        Event::SwitchedTo { id } => state.focused = Some(id),
    }
}

/// A session by id, with a fix-naming error.
pub fn session<'s>(state: &'s State, id: &str) -> Result<&'s Session> {
    state.sessions.get(id).ok_or_else(|| {
        let known: Vec<&str> = state.sessions.keys().map(String::as_str).collect();
        Error::NotFound(format!(
            "session `{id}` — known: [{}]. Create one: waggle-tmux up {id} … or register a pane: waggle-tmux register {id} --profile <p> --pane %N",
            known.join(", ")
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_folds_registration_minting_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        append(
            ws,
            &Event::SessionRegistered {
                id: "codex".into(),
                profile: "codex".into(),
                pane: "%2".into(),
                owned: true,
            },
        )
        .unwrap();
        append(
            ws,
            &Event::OutcomeMinted {
                token: "7Kp2xQ9f".into(),
                target: "file:///plan.md".into(),
                to: Some("codex".into()),
            },
        )
        .unwrap();

        let mid = load(ws);
        assert_eq!(mid.pending.as_ref().unwrap().0, "7Kp2xQ9f");
        assert!(mid.sessions["codex"].owned);

        append(
            ws,
            &Event::HandoffSent {
                token: "7Kp2xQ9f".into(),
                to: "codex".into(),
                resolves_at_send: 1,
            },
        )
        .unwrap();
        append(ws, &Event::SwitchedTo { id: "codex".into() }).unwrap();

        let end = load(ws);
        assert!(end.pending.is_none(), "delivery consumes pending");
        assert_eq!(
            end.sessions["codex"].last_delivery,
            Some(("7Kp2xQ9f".into(), 1))
        );
        assert_eq!(end.focused.as_deref(), Some("codex"));
    }

    #[test]
    fn unknown_lines_are_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = events_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{\"event\":\"from-the-future\",\"x\":1}\n").unwrap();
        assert!(load(dir.path()).sessions.is_empty());
    }

    #[test]
    fn missing_session_error_names_the_fix() {
        let err = session(&State::default(), "planner").unwrap_err();
        assert!(err.to_string().contains("waggle-tmux register planner"));
    }
}
