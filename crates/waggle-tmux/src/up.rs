//! `waggle-tmux up`: choose harnesses once, get a wired workspace —
//! MCP into every chosen harness, the stub into the repo, the daemon
//! up, one owned pane per harness (seamless §3). Convergent and
//! re-runnable: it repairs rather than errors.

use std::io::Write as _;
use std::path::Path;

use crate::error::{Error, Result};
use crate::profile::{self, HarnessProfile};
use crate::state::{self, Event};
use crate::tmux::{self, TmuxBackend};

/// The tmux session name the switchboard owns.
pub const SESSION: &str = "waggle";

/// Run `up` for the chosen profile ids.
pub fn run<T: TmuxBackend>(tmux: &T, workspace: &Path, chosen: &[String]) -> Result<()> {
    let profiles = profile::load(&workspace.join(".waggle/tmux/config.toml"))?;
    let picked = pick(&profiles, chosen)?;

    for p in &picked {
        wire_harness(p)?;
        println!("wired: {} (waggle MCP ready)", p.display_name);
    }
    ensure_workspace_stub(workspace);
    ensure_daemon();

    let window = task_window();
    if !tmux::has_session(tmux, SESSION) {
        tmux::new_session(tmux, SESSION, &window, &workspace.to_string_lossy())?;
    }
    let mut panes = existing_panes(tmux, workspace)?;
    for p in &picked {
        if state::load(workspace).sessions.contains_key(&p.id) {
            continue; // convergent: already registered
        }
        let pane = match panes.pop() {
            Some(free) => free,
            None => tmux::split(tmux, SESSION, &workspace.to_string_lossy())?,
        };
        if let Some(cmd) = &p.launch_command {
            tmux::send_line(tmux, &pane, cmd)?;
        }
        state::append(
            workspace,
            &Event::SessionRegistered {
                id: p.id.clone(),
                profile: p.id.clone(),
                pane: pane.clone(),
                owned: true,
            },
        )?;
        println!(
            "session `{}` in pane {pane} (owned — seamless delivery on)",
            p.id
        );
    }
    let session_ids: Vec<String> = state::load(workspace).sessions.keys().cloned().collect();
    tmux::bind_keys(tmux, workspace, &session_ids);

    // Land the user INSIDE the first harness pane — `up` should feel
    // like walking into the room, not being handed a map of it.
    if let Some(first) = picked.first() {
        if let Some(session) = state::load(workspace).sessions.get(&first.id) {
            let _ = tmux::select(tmux, &session.pane);
        }
    }
    println!("workspace up — prefix+W is the switchboard menu (switch / mint / status)");
    attach(tmux);
    Ok(())
}

/// Step inside: switch-client when already in tmux, attach on a real
/// terminal, print the command otherwise (CI, scripts).
fn attach<T: TmuxBackend>(tmux: &T) {
    use std::io::IsTerminal as _;
    if std::env::var("TMUX").is_ok() {
        let _ = tmux.run(&["switch-client", "-t", SESSION]);
    } else if std::io::stdout().is_terminal() {
        // exec replaces this process with the attach — the natural end
        // of `up` on a terminal. (Unix-only, like tmux itself; the
        // crate still compiles everywhere the workspace lints.)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            let err = std::process::Command::new("tmux")
                .args(["attach", "-t", SESSION])
                .exec();
            eprintln!("attach yourself: tmux attach -t {SESSION} ({err})");
        }
        #[cfg(not(unix))]
        println!("attach: tmux attach -t {SESSION}");
    } else {
        println!("attach: tmux attach -t {SESSION}");
    }
}

fn pick(profiles: &[HarnessProfile], chosen: &[String]) -> Result<Vec<HarnessProfile>> {
    if chosen.is_empty() {
        let detected: Vec<HarnessProfile> = profiles
            .iter()
            .filter(|p| profile::detected(p))
            .cloned()
            .collect();
        if detected.is_empty() {
            return Err(Error::NotFound(
                "no known harnesses detected — name them explicitly: waggle-tmux up claude-code codex".into(),
            ));
        }
        println!(
            "detected: {}",
            detected
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Ok(detected);
    }
    chosen
        .iter()
        .map(|id| profile::find(profiles, id).cloned())
        .collect()
}

/// The idempotent per-harness MCP wiring (seamless §3.2).
fn wire_harness(p: &HarnessProfile) -> Result<()> {
    match p.harness.as_str() {
        "claude-code" => wire_claude(),
        "codex" => wire_codex(),
        _ => Ok(()), // generic harnesses bring their own wiring
    }
}

fn wire_claude() -> Result<()> {
    let list = std::process::Command::new("claude")
        .args(["mcp", "list"])
        .output();
    let already = list
        .as_ref()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("waggle"));
    if already {
        return Ok(());
    }
    let out = std::process::Command::new("claude")
        .args(["mcp", "add", "waggle", "--", "waggle", "serve", "--stdio"])
        .output()
        .map_err(|e| Error::Config(format!("claude not runnable ({e})")))?;
    if !out.status.success() {
        return Err(Error::Config(format!(
            "claude mcp add failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

/// Codex config lives at `$CODEX_HOME/config.toml` (default `~/.codex`).
fn wire_codex() -> Result<()> {
    let home = std::env::var("CODEX_HOME").unwrap_or_else(|_| {
        format!(
            "{}/.codex",
            std::env::var("HOME").unwrap_or_else(|_| ".".into())
        )
    });
    let path = std::path::PathBuf::from(home).join("config.toml");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("[mcp_servers.waggle]") {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(
        file,
        "\n[mcp_servers.waggle]\ncommand = \"waggle\"\nargs = [\"serve\", \"--stdio\"]"
    )?;
    Ok(())
}

fn ensure_workspace_stub(workspace: &Path) {
    let _ = std::process::Command::new("waggle")
        .arg("init")
        .current_dir(workspace)
        .output();
}

fn ensure_daemon() {
    let _ = std::process::Command::new("waggle")
        .args(["daemon", "start"])
        .output();
}

/// Free panes in our session — live, and NOT already bound to a
/// registered session (never hand someone's working pane to a new
/// harness).
fn existing_panes<T: TmuxBackend>(tmux: &T, workspace: &Path) -> Result<Vec<String>> {
    let bound: std::collections::BTreeSet<String> = state::load(workspace)
        .sessions
        .values()
        .map(|s| s.pane.clone())
        .collect();
    Ok(tmux::list_panes(tmux)?
        .into_iter()
        .filter(|p| p.session == SESSION && !bound.contains(&p.pane_id))
        .map(|p| p.pane_id)
        .rev()
        .collect())
}

fn task_window() -> String {
    // No wall clock needed for uniqueness — the pid is enough for a
    // window name, and state carries the real identity.
    format!("wg-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_wiring_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("CODEX_HOME", dir.path());
        wire_codex().unwrap();
        wire_codex().unwrap(); // second run: no duplicate
        let written = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert_eq!(written.matches("[mcp_servers.waggle]").count(), 1);
        assert!(written.contains("\"serve\", \"--stdio\""));
        std::env::remove_var("CODEX_HOME");
    }
}
