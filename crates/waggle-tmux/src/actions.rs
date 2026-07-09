//! The daily verbs: mint an outcome, switch (and the switch resolves),
//! status from funnels (seamless §4–§6).

use std::path::Path;

use crate::error::{Error, Result};
use crate::profile;
use crate::state::{self, Event, State};
use crate::tmux::{self, TmuxBackend};
use crate::waggle::{self, WaggleClient};

/// `mint` with SEVERAL paths: one lineage BUNDLE. A note file becomes
/// the root; every path (files and folders alike — folders as trees)
/// is minted as its child. ONE token travels; the destination resolves
/// the root's index and picks its pieces; one revocation kills all.
pub fn mint_bundle<W: WaggleClient>(
    waggle_client: &W,
    workspace: &Path,
    paths: &[String],
    to: Option<&str>,
) -> Result<String> {
    for path in paths {
        if !workspace.join(path).exists() {
            return Err(Error::NotFound(format!(
                "`{path}` does not exist — every bundle piece must be inside the workspace"
            )));
        }
    }
    let dir = workspace.join(".waggle-handoffs");
    std::fs::create_dir_all(&dir)?;
    let note = dir.join(format!("bundle-{}.md", std::process::id()));
    let listing: Vec<String> = paths.iter().map(|p| format!("- {p}")).collect();
    std::fs::write(
        &note,
        format!(
            "# Handoff bundle\n\nResolve this token's children for the pieces:\n{}\n",
            listing.join("\n")
        ),
    )?;
    let root_target = format!("file://{}", note.canonicalize()?.display());
    let root = waggle::mint(waggle_client, &root_target, false, None)?;
    for path in paths {
        let full = workspace.join(path).canonicalize()?;
        let target = format!("file://{}", full.display());
        waggle::mint(waggle_client, &target, full.is_dir(), Some(&root))?;
    }
    state::append(
        workspace,
        &Event::OutcomeMinted {
            token: root.clone(),
            target: root_target,
            to: to.map(str::to_owned),
        },
    )?;
    println!(
        "minted bundle {root} ({} piece(s) as children) — pending handoff{}",
        paths.len(),
        to.map(|t| format!(" for `{t}`")).unwrap_or_default()
    );
    Ok(root)
}

/// `mint`: any outcome → a pending handoff. A directory becomes a
/// `--tree` (root + snapshot children); a file becomes a snapshot.
pub fn mint<W: WaggleClient>(
    waggle_client: &W,
    workspace: &Path,
    path: &str,
    to: Option<&str>,
) -> Result<String> {
    mint_inner(waggle_client, workspace, path, to, false)
}

/// `mint --seal`: after snapshot-minting, MOVE the source out of the
/// working tree into `.waggle-handoffs/sealed/<token>/` — the token
/// becomes the ONLY door, so coverage receipts are enforcement-grade
/// even locally. Non-destructive: the bytes live in the CAS AND the
/// sealed archive; unseal by moving the directory back.
pub fn mint_sealed<W: WaggleClient>(
    waggle_client: &W,
    workspace: &Path,
    path: &str,
    to: Option<&str>,
) -> Result<String> {
    mint_inner(waggle_client, workspace, path, to, true)
}

fn mint_inner<W: WaggleClient>(
    waggle_client: &W,
    workspace: &Path,
    path: &str,
    to: Option<&str>,
    seal: bool,
) -> Result<String> {
    let full = workspace.join(path);
    if !full.exists() {
        return Err(Error::NotFound(format!(
            "`{path}` does not exist — mint takes a file or directory inside the workspace"
        )));
    }
    let tree = full.is_dir();
    let target = format!("file://{}", full.canonicalize()?.display());
    let parent_state = state::load(workspace);
    let parent = parent_state
        .sessions
        .values()
        .filter_map(|s| s.last_delivery.as_ref())
        .next_back()
        .map(|(t, _)| t.clone());
    let token = waggle::mint(waggle_client, &target, tree, parent.as_deref())?;
    state::append(
        workspace,
        &Event::OutcomeMinted {
            token: token.clone(),
            target,
            to: to.map(str::to_owned),
        },
    )?;
    if seal {
        let vault = workspace.join(".waggle-handoffs/sealed").join(&token);
        std::fs::create_dir_all(&vault)?;
        let dest = vault.join(full.file_name().unwrap_or_default());
        std::fs::rename(&full, &dest)?;
        println!(
            "sealed: {path} moved to {} — the token is now the only door",
            dest.display()
        );
    }
    println!(
        "minted {token}{} — pending handoff{}",
        if tree {
            " (tree: directory + children)"
        } else {
            ""
        },
        to.map(|t| format!(" for `{t}`")).unwrap_or_default()
    );
    Ok(token)
}

/// `mint` with no path: the git picker — changed files as candidates.
pub fn pick_git(workspace: &Path) -> Result<Vec<String>> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace)
        .output()
        .map_err(|e| Error::Config(format!("git not available ({e})")))?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.get(3..).map(str::to_owned))
        .collect())
}

/// `switch`: the waggle moment (seamless §5). Preview with the
/// DESTINATION's context, record handoff-sent with a funnel baseline,
/// focus the pane, deliver the instruction (owned panes), done.
pub fn switch<T: TmuxBackend, W: WaggleClient>(
    tmux_backend: &T,
    waggle_client: &W,
    workspace: &Path,
    dest: &str,
    token: Option<&str>,
    no_inject: bool,
) -> Result<()> {
    let st = state::load(workspace);
    let session = state::session(&st, dest)?;
    // Everything queued for this destination travels on one switch —
    // explicitly addressed first, unaddressed riding along.
    let tokens: Vec<String> = match token {
        Some(t) => vec![t.to_owned()],
        None => st
            .pending
            .iter()
            .filter(|(_, _, to)| to.as_deref().is_none_or(|d| d == dest))
            .map(|(t, _, _)| t.clone())
            .collect(),
    };
    if tokens.is_empty() {
        return Err(Error::NotFound(
            "no pending outcome for this destination — mint one first: waggle-tmux mint <paths…>"
                .into(),
        ));
    }

    // The matcher runs at switch time, with the destination's context.
    let profiles = profile::load(&workspace.join(".waggle/tmux/config.toml"))?;
    let dest_profile = profile::find(&profiles, &session.profile)?;
    tmux::select(tmux_backend, &session.pane)?;
    let foreground = tmux::list_panes(tmux_backend)?
        .into_iter()
        .find(|p| p.pane_id == session.pane)
        .map(|p| p.current_cmd)
        .unwrap_or_default();

    for token in tokens {
        match waggle::preview(waggle_client, &token, &dest_profile.resolver_context()) {
            Ok(line) => println!("{dest} will receive: {line}"),
            Err(e) => println!("preview unavailable ({e}) — delivering anyway"),
        }
        waggle::record(waggle_client, &token, "handoff-sent")?;
        let baseline = waggle::resolve_count(waggle_client, &token)?;
        let line = dest_profile.inject_line(&token);
        if tmux::is_shell(&foreground) {
            // No harness listening — typing here becomes zsh noise.
            println!(
                "{dest}'s pane is a bare shell ({foreground}) — start the harness there, then paste:\n  {line}"
            );
        } else if session.owned && !no_inject {
            tmux::send_line(tmux_backend, &session.pane, &line)?;
            println!("delivered into {dest}'s prompt — it resolves as itself");
        } else {
            println!("paste into {dest}:\n  {line}");
        }
        state::append(
            workspace,
            &Event::HandoffSent {
                token,
                to: dest.to_owned(),
                resolves_at_send: baseline,
            },
        )?;
    }
    state::append(
        workspace,
        &Event::SwitchedTo {
            id: dest.to_owned(),
        },
    )?;
    Ok(())
}

/// `next`: follow the pending outcome's declared destination.
pub fn next<T: TmuxBackend, W: WaggleClient>(
    tmux_backend: &T,
    waggle_client: &W,
    workspace: &Path,
) -> Result<()> {
    let st = state::load(workspace);
    let Some((token, _, to)) = st.pending.first().cloned() else {
        return Err(Error::NotFound(
            "no pending outcome — mint one first: waggle-tmux mint <paths…>".into(),
        ));
    };
    let dest = to.or_else(|| other_session(&st)).ok_or_else(|| {
        Error::NotFound("pending outcome has no destination — waggle-tmux switch <session>".into())
    })?;
    switch(
        tmux_backend,
        waggle_client,
        workspace,
        &dest,
        Some(&token),
        false,
    )
}

fn other_session(st: &State) -> Option<String> {
    st.sessions
        .keys()
        .find(|id| st.focused.as_ref() != Some(*id))
        .cloned()
}

/// `status`: consumption from the FUNNEL, not bookkeeping (seamless §6):
/// consumed = the token's resolve count moved past the delivery baseline.
pub fn status<W: WaggleClient>(waggle_client: &W, workspace: &Path) {
    let st = state::load(workspace);
    if st.sessions.is_empty() {
        println!("no sessions — start with: waggle-tmux up claude-code codex");
        return;
    }
    println!(
        "{:<12} {:<12} {:<6} {:<6} {:<12} CONSUMED?",
        "SESSION", "PROFILE", "PANE", "OWNED", "LAST TOKEN"
    );
    for (id, s) in &st.sessions {
        let (token, consumed) = match &s.last_delivery {
            Some((token, baseline)) => {
                let now = waggle::resolve_count(waggle_client, token).unwrap_or(*baseline);
                let mark = if now > *baseline {
                    format!("yes — {} resolve(s)", now - baseline)
                } else {
                    "not yet".into()
                };
                (token.clone(), mark)
            }
            None => ("-".into(), "-".into()),
        };
        println!(
            "{id:<12} {:<12} {:<6} {:<6} {token:<12} {consumed}",
            s.profile,
            s.pane,
            if s.owned { "yes" } else { "no" },
        );
    }
    for (token, target, to) in &st.pending {
        println!(
            "\npending: {token} ({target}){}",
            to.as_ref().map(|t| format!(" → {t}")).unwrap_or_default()
        );
    }
}

/// `register`: bring-your-own pane (standards doc §1). Never owned —
/// delivery prints the line instead of injecting.
pub fn register<T: TmuxBackend>(
    tmux_backend: &T,
    workspace: &Path,
    id: &str,
    profile_id: &str,
    pane: &str,
) -> Result<()> {
    let profiles = profile::load(&workspace.join(".waggle/tmux/config.toml"))?;
    profile::find(&profiles, profile_id)?;
    let live = tmux::list_panes(tmux_backend)?;
    let Some(info) = live.iter().find(|p| p.pane_id == pane) else {
        return Err(Error::NotFound(format!(
            "pane {pane} not found — list live panes: tmux list-panes -a"
        )));
    };
    state::append(
        workspace,
        &Event::SessionRegistered {
            id: id.to_owned(),
            profile: profile_id.to_owned(),
            pane: pane.to_owned(),
            owned: false,
            room: Some(info.session.clone()),
        },
    )?;
    println!("registered `{id}` → {pane} (external — delivery prints the resolve line)");
    Ok(())
}

/// Toggle the current window's board strip: short strip <-> half screen.
pub fn board_toggle<T: TmuxBackend>(tmux_backend: &T) -> Result<()> {
    let panes = tmux_backend.run(&[
        "list-panes",
        "-F",
        "#{pane_id}\t#{pane_title}\t#{pane_height}",
    ])?;
    for line in panes.lines() {
        let mut f = line.split('\t');
        let (Some(id), Some(title), Some(height)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        if title == "waggle" {
            // Cycle: strip (6) -> maximized (half) -> minimized (2) -> strip.
            let h = height.parse::<u32>().unwrap_or(6);
            let (target, name) = if h <= 3 {
                ("6", "strip")
            } else if h <= 12 {
                ("50%", "maximized")
            } else {
                ("2", "minimized")
            };
            tmux_backend.run(&["resize-pane", "-t", id, "-y", target])?;
            let _ = tmux_backend.run(&["display-message", &format!("waggle board: {name}")]);
            return Ok(());
        }
    }
    Err(Error::NotFound(
        "no board strip in this window — waggle-tmux up creates one per harness window".into(),
    ))
}

/// Reconcile exits: a harness whose pane died gets closed out — its
/// board strip killed (the window follows), its registration ended —
/// and a SURVIVING harness is foregrounded. When the last one leaves,
/// the whole workspace closes gracefully. Idempotent and quiet: safe
/// from hooks and from every watcher tick.
pub fn reap<T: TmuxBackend>(tmux_backend: &T, workspace: &Path) -> Result<()> {
    let st = state::load(workspace);
    let live: std::collections::BTreeSet<String> = tmux::list_panes(tmux_backend)?
        .into_iter()
        .map(|p| p.pane_id)
        .collect();
    let dead: Vec<String> = st
        .sessions
        .iter()
        .filter(|(id, s)| !id.starts_with('_') && !live.contains(&s.pane))
        .map(|(id, _)| id.clone())
        .collect();
    if dead.is_empty() {
        return Ok(());
    }
    for id in &dead {
        // The strip kept the window alive; take it down with the harness.
        if let Some(strip) = st.sessions.get(&format!("_board-{id}")) {
            let _ = tmux_backend.run(&["kill-pane", "-t", &strip.pane]);
        }
        state::append(workspace, &Event::SessionClosed { id: id.clone() })?;
        state::append(
            workspace,
            &Event::SessionClosed {
                id: format!("_board-{id}"),
            },
        )?;
    }
    let survivors: Vec<(&String, &state::Session)> = st
        .sessions
        .iter()
        .filter(|(id, s)| !id.starts_with('_') && live.contains(&s.pane) && !dead.contains(id))
        .collect();
    if let Some((id, session)) = survivors.first() {
        let _ = tmux::select(tmux_backend, &session.pane);
        let _ = tmux_backend.run(&[
            "display-message",
            &format!("{} exited — {id} foregrounded", dead.join(", ")),
        ]);
    } else {
        // The last harness left: close the room it lived in — recorded
        // at registration, so tests and future multi-room setups reap
        // the RIGHT session, never a guess.
        let room = dead
            .iter()
            .find_map(|id| st.sessions.get(id).and_then(|s| s.room.clone()))
            .unwrap_or_else(|| crate::up::SESSION.to_owned());
        let _ = tmux_backend.run(&["kill-session", "-t", &room]);
    }
    Ok(())
}
