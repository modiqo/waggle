//! The tmux seam: one trait method (`run`), everything else built on it
//! so a fake backend exercises every code path (standards doc §12).
//! Arguments are always vectors — never concatenated shell strings.

use crate::error::{Error, Result};

/// The only thing a backend must do: run one tmux command.
pub trait TmuxBackend {
    /// Run `tmux <args>`, returning stdout on success.
    fn run(&self, args: &[&str]) -> Result<String>;
}

/// The real thing: `Command::new("tmux")`.
pub struct RealTmux;

impl TmuxBackend for RealTmux {
    fn run(&self, args: &[&str]) -> Result<String> {
        let out = std::process::Command::new("tmux")
            .args(args)
            .output()
            .map_err(|e| Error::Tmux(format!("is tmux installed? ({e})")))?;
        if !out.status.success() {
            return Err(Error::Tmux(format!(
                "`tmux {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// One live pane, as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    /// tmux session name.
    pub session: String,
    /// Window index.
    pub window: String,
    /// Stable-for-the-server-lifetime pane id, e.g. `%3`.
    pub pane_id: String,
    /// The command currently running in the pane.
    pub current_cmd: String,
}

const LIST_FORMAT: &str = "#{session_name}\t#{window_index}\t#{pane_id}\t#{pane_current_command}";

/// All panes on the server.
pub fn list_panes<T: TmuxBackend>(tmux: &T) -> Result<Vec<PaneInfo>> {
    let raw = tmux.run(&["list-panes", "-a", "-F", LIST_FORMAT])?;
    Ok(parse_panes(&raw))
}

pub(crate) fn parse_panes(raw: &str) -> Vec<PaneInfo> {
    raw.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            Some(PaneInfo {
                session: f.next()?.to_owned(),
                window: f.next()?.to_owned(),
                pane_id: f.next()?.to_owned(),
                current_cmd: f.next().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}

/// Does a session exist?
pub fn has_session<T: TmuxBackend>(tmux: &T, name: &str) -> bool {
    tmux.run(&["has-session", "-t", name]).is_ok()
}

/// Create a detached session with one window; returns nothing — pane ids
/// come from [`list_panes`] afterward (never trust indexes).
pub fn new_session<T: TmuxBackend>(tmux: &T, name: &str, window: &str, cwd: &str) -> Result<()> {
    tmux.run(&["new-session", "-d", "-s", name, "-n", window, "-c", cwd])?;
    Ok(())
}

/// Split the target, returning the NEW pane's id via -P.
pub fn split<T: TmuxBackend>(tmux: &T, target: &str, cwd: &str) -> Result<String> {
    let id = tmux.run(&[
        "split-window",
        "-h",
        "-t",
        target,
        "-c",
        cwd,
        "-P",
        "-F",
        "#{pane_id}",
    ])?;
    Ok(id.trim().to_owned())
}

/// Focus a pane (and its window).
pub fn select<T: TmuxBackend>(tmux: &T, pane_id: &str) -> Result<()> {
    tmux.run(&["select-window", "-t", pane_id])?;
    tmux.run(&["select-pane", "-t", pane_id])?;
    Ok(())
}

/// Deliver literal text into a pane's prompt, then Enter. This is the
/// seamless delivery path — an instruction typed, never an action taken
/// (seamless §2.3). `-l` keeps the text literal.
pub fn send_line<T: TmuxBackend>(tmux: &T, pane_id: &str, text: &str) -> Result<()> {
    tmux.run(&["send-keys", "-t", pane_id, "-l", "--", text])?;
    tmux.run(&["send-keys", "-t", pane_id, "Enter"])?;
    Ok(())
}

/// Bounded tail of a pane's visible history (the Phase-4 watcher's
/// door; integration tests use it via raw tmux today).
#[allow(dead_code)]
pub fn capture_tail<T: TmuxBackend>(tmux: &T, pane_id: &str, lines: u32) -> Result<String> {
    tmux.run(&[
        "capture-pane",
        "-t",
        pane_id,
        "-p",
        "-S",
        &format!("-{lines}"),
    ])
}

/// Bind prefix+W to the switchboard menu: switch to any session (the
/// switch delivers the pending handoff), mint an outcome by path, see
/// status. Best effort — a missing bind never blocks the workspace.
pub fn bind_keys<T: TmuxBackend>(tmux: &T, workspace: &std::path::Path, sessions: &[String]) {
    // run-shell executes in the tmux SERVER's cwd — every command must
    // carry the workspace (found live: switch bookkeeping landed in the
    // wrong directory while the delivery itself worked).
    let ws = workspace.display();
    let mut args: Vec<String> = ["bind-key", "W", "display-menu", "-T", "waggle"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    for (i, id) in sessions.iter().enumerate() {
        args.push(format!("switch to {id}"));
        args.push((i + 1).to_string());
        args.push(format!(
            "run-shell 'cd {ws} && waggle-tmux switch {id} >/tmp/waggle-tmux.last 2>&1; tmux display-message \"$(tail -1 /tmp/waggle-tmux.last)\"'"
        ));
    }
    args.push("mint outcome".into());
    args.push("m".into());
    args.push(format!(
        "command-prompt -p 'mint (file or dir):' {{run-shell 'cd {ws} && waggle-tmux mint %1 >/tmp/waggle-tmux.last 2>&1; tmux display-message \"$(tail -1 /tmp/waggle-tmux.last)\"'}}"
    ));
    args.push("status".into());
    args.push("t".into());
    args.push(format!(
        "display-popup -E 'cd {ws} && waggle-tmux status; echo; echo [any key]; read -n 1'"
    ));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let _ = tmux.run(&refs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_parsing_is_tab_exact() {
        let parsed = parse_panes("waggle\t0\t%1\tclaude\nwaggle\t0\t%2\tcodex\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].pane_id, "%1");
        assert_eq!(parsed[1].current_cmd, "codex");
    }

    /// A fake that records calls — the whole seam is `run`.
    pub struct FakeTmux(pub std::cell::RefCell<Vec<Vec<String>>>);
    impl TmuxBackend for FakeTmux {
        fn run(&self, args: &[&str]) -> Result<String> {
            self.0
                .borrow_mut()
                .push(args.iter().map(|s| (*s).to_owned()).collect());
            Ok(String::new())
        }
    }

    #[test]
    fn send_line_is_literal_then_enter() {
        let fake = FakeTmux(std::cell::RefCell::new(Vec::new()));
        send_line(&fake, "%3", "Resolve 7Kp2xQ9f via waggle -- literally").unwrap();
        let calls = fake.0.borrow();
        assert_eq!(calls[0][..4], ["send-keys", "-t", "%3", "-l"]);
        assert_eq!(calls[0][4], "--");
        assert!(calls[0][5].contains("literally"));
        assert_eq!(calls[1][3], "Enter");
    }
}
