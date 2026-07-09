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

    // Focus layout: ONE WINDOW PER HARNESS (its name lives in the tmux
    // status bar — the other harnesses "appear in the bar"), each with a
    // titled harness pane on top and a slim board strip below. Switching
    // swaps windows; prefix+z zooms the active pane to full screen.
    let cwd = workspace.to_string_lossy();
    let live: std::collections::BTreeSet<String> = if tmux::has_session(tmux, SESSION) {
        tmux::list_panes(tmux)?
            .into_iter()
            .filter(|p| p.session == SESSION)
            .map(|p| p.pane_id)
            .collect()
    } else {
        std::collections::BTreeSet::new()
    };
    let mut session_is_new = false;
    if live.is_empty() {
        // The harness runs AS the first window's pane process — no shell
        // init to race, no update prompt to swallow the launch.
        let first = picked.first().ok_or_else(|| {
            Error::NotFound("no harnesses picked — waggle-tmux up claude-code codex".into())
        })?;
        tmux::new_session(
            tmux,
            SESSION,
            &first.id,
            &cwd,
            first.launch_command.as_deref(),
        )?;
        session_is_new = true;
    }
    // Mac-friendly driving: the mouse works everywhere — click a
    // window name in the bar to swap harnesses, click a pane to focus,
    // DRAG the board strip's border to expand/minimize it.
    let _ = tmux.run(&["set", "-t", SESSION, "mouse", "on"]);
    // Pane titles come from the profiles; show them on the borders.
    let _ = tmux.run(&["set", "-t", SESSION, "pane-border-status", "top"]);
    let _ = tmux.run(&[
        "set",
        "-t",
        SESSION,
        "pane-border-format",
        " #{pane_title} ",
    ]);

    for (i, p) in picked.iter().enumerate() {
        let st = state::load(workspace);
        if let Some(existing) = st.sessions.get(&p.id) {
            if live.contains(&existing.pane) {
                continue; // convergent: registered AND alive
            }
        }
        let pane = if i == 0 && session_is_new {
            tmux::list_panes(tmux)?
                .into_iter()
                .find(|info| info.session == SESSION)
                .map(|info| info.pane_id)
                .ok_or_else(|| Error::Tmux("first pane vanished".into()))?
        } else {
            tmux::new_window(tmux, SESSION, &p.id, &cwd, p.launch_command.as_deref())?
        };
        let _ = tmux.run(&["select-pane", "-t", &pane, "-T", &p.display_name]);
        ensure_board_strip(tmux, workspace, &p.id, &pane)?;
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
            "window `{}` up: {} + board strip (owned — seamless delivery on)",
            p.id, p.display_name
        );
    }
    ensure_watch_pane(tmux, workspace)?;
    write_agent_block(workspace, &picked);

    let session_ids: Vec<String> = state::load(workspace)
        .sessions
        .keys()
        .filter(|id| !id.starts_with('_'))
        .cloned()
        .collect();
    tmux::bind_keys(tmux, &session_ids);

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

/// Each harness window gets a slim BOARD strip below it (pure reader —
/// safe to replicate). `waggle-tmux board-toggle` (or the menu) flips it
/// between strip and half-screen.
fn ensure_board_strip<T: TmuxBackend>(
    tmux: &T,
    workspace: &Path,
    window_id: &str,
    harness_pane: &str,
) -> Result<()> {
    let key = format!("_board-{window_id}");
    let st = state::load(workspace);
    if let Some(existing) = st.sessions.get(&key) {
        if tmux::list_panes(tmux)?
            .iter()
            .any(|p| p.pane_id == existing.pane)
        {
            return Ok(());
        }
    }
    let pane = tmux.run(&[
        "split-window",
        "-v",
        "-l",
        "6",
        "-t",
        harness_pane,
        "-c",
        &workspace.to_string_lossy(),
        "-P",
        "-F",
        "#{pane_id}",
        "waggle-tmux",
        "watch",
        "--board-only",
    ])?;
    let pane = pane.trim().to_owned();
    let _ = tmux.run(&["select-pane", "-t", &pane, "-T", "waggle"]);
    // Focus back on the harness — the strip is furniture, not a seat.
    let _ = tmux.run(&["select-pane", "-t", harness_pane]);
    state::append(
        workspace,
        &Event::SessionRegistered {
            id: key,
            profile: "generic".into(),
            pane,
            owned: false,
        },
    )?;
    Ok(())
}

/// The single DELIVERER: one headless watcher in a background window
/// (visible in the bar as `wgd`; kill it to go manual). Exactly one —
/// duplicated deliverers would double-deliver.
fn ensure_watch_pane<T: TmuxBackend>(tmux: &T, workspace: &Path) -> Result<()> {
    let st = state::load(workspace);
    if let Some(existing) = st.sessions.get("_watch") {
        if tmux::list_panes(tmux)?
            .iter()
            .any(|p| p.pane_id == existing.pane)
        {
            return Ok(());
        }
    }
    let pane = tmux.run(&[
        "new-window",
        "-d",
        "-t",
        SESSION,
        "-n",
        "wgd",
        "-c",
        &workspace.to_string_lossy(),
        "-P",
        "-F",
        "#{pane_id}",
        "waggle-tmux",
        "watch",
        "--headless",
    ])?;
    let pane = pane.trim().to_owned();
    state::append(
        workspace,
        &Event::SessionRegistered {
            id: "_watch".into(),
            profile: "generic".into(),
            pane,
            owned: false,
        },
    )?;
    println!("deliverer live (window `wgd`) — agents mint to tmux/<session>, windows swap");
    Ok(())
}

/// The workspace instruction agents read (managed block in the harness
/// convention files): finish -> mint YOUR outcome, addressed by channel.
fn write_agent_block(workspace: &Path, picked: &[HarnessProfile]) {
    const BEGIN: &str = "<!-- waggle-tmux:begin (managed by `waggle-tmux up`) -->";
    const END: &str = "<!-- waggle-tmux:end -->";
    let all: Vec<String> = state::load(workspace)
        .sessions
        .keys()
        .filter(|id| !id.starts_with('_'))
        .cloned()
        .chain(picked.iter().map(|p| p.id.clone()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let block = format!(
        "{BEGIN}\n## Harness handoffs (waggle-tmux)\nThis workspace runs a switchboard. When your task is COMPLETE, mint the\noutcome yourself and address it — the watcher delivers it and the\ndestination harness takes over:\n\n    waggle mint --target <file-or-dir> --snapshot --channel tmux/<destination>\n\nDestinations here: {}. Address your review requests back the same way.\n{END}\n",
        all.join(", ")
    );
    for name in ["CLAUDE.md", "AGENTS.md"] {
        let path = workspace.join(name);
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let updated = if let (Some(b), Some(e)) = (existing.find(BEGIN), existing.find(END)) {
            format!(
                "{}{}{}",
                &existing[..b],
                block.trim_end(),
                &existing[e + END.len()..]
            )
        } else {
            format!("{existing}\n{block}")
        };
        let _ = std::fs::write(&path, updated);
    }
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
