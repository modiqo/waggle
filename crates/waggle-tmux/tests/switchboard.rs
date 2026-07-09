//! The seamless loop against REAL tmux and the REAL waggle binary —
//! gated (`WAGGLE_TMUX_TESTS=1`) because it needs both on the machine.
//! No harnesses required: a `cat` pane plays the destination, and
//! capture-pane proves the instruction landed in its prompt — the
//! delivery half of "resolve upon switch" verified byte-for-byte;
//! the consumption half verified by resolving as the destination and
//! watching status flip via the funnel baseline.

use std::process::Command;

fn gated() -> bool {
    std::env::var("WAGGLE_TMUX_TESTS").is_ok_and(|v| v == "1")
}

/// The waggle binary for destination-side simulation — CI points this
/// at the workspace build; locally it's the installed one.
fn waggle_bin() -> String {
    std::env::var("WAGGLE_TMUX_BIN").unwrap_or_else(|_| "waggle".into())
}

struct TmuxGuard(String);
impl Drop for TmuxGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.0])
            .output();
    }
}

fn run(bin: &str, dir: &std::path::Path, envs: &[(&str, &str)], args: &[&str]) -> (bool, String) {
    let mut cmd = Command::new(bin);
    cmd.current_dir(dir).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

#[test]
fn the_seamless_loop_delivers_and_detects_consumption() {
    if !gated() {
        eprintln!("skipped: set WAGGLE_TMUX_TESTS=1 (needs tmux + waggle on PATH)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let session = format!("waggle-test-{}", std::process::id());
    let _guard = TmuxGuard(session.clone());

    // An isolated waggle world for the whole test.
    let store = ws.join("waggle.db").display().to_string();
    let sock = ws.join("no-daemon.sock").display().to_string();
    let envs: Vec<(&str, &str)> = vec![("WAGGLE_STORE", &store), ("WAGGLE_SOCK", &sock)];
    let me = env!("CARGO_BIN_EXE_waggle-tmux");

    // The destination pane: `cat` echoes what it receives — a perfect
    // witness for injection.
    assert!(Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-n", "w", "cat"])
        .status()
        .unwrap()
        .success());
    let pane = String::from_utf8(
        Command::new("tmux")
            .args(["list-panes", "-t", &session, "-F", "#{pane_id}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    // Register it, then FORCE owned=true (the up-created case) by
    // appending the state event the way `up` would.
    let (ok, out) = run(
        me,
        ws,
        &envs,
        &["register", "codex", "--profile", "codex", "--pane", &pane],
    );
    assert!(ok, "{out}");
    let events = ws.join(".waggle/tmux/events.jsonl");
    let external = std::fs::read_to_string(&events).unwrap();
    let owned = external.replace("\"owned\":false", "\"owned\":true");
    std::fs::write(&events, owned).unwrap();

    // An outcome exists because work happened.
    std::fs::write(ws.join("plan.md"), "# Plan\ndo the thing\n").unwrap();
    let (ok, out) = run(me, ws, &envs, &["mint", "plan.md", "--to", "codex"]);
    assert!(ok, "{out}");
    let token = out.split_whitespace().nth(1).unwrap().to_owned();

    // THE SWITCH: preview + deliver + baseline.
    let (ok, out) = run(me, ws, &envs, &["switch", "codex"]);
    assert!(ok, "{out}");
    assert!(
        out.contains("codex will receive: active"),
        "preview ran with dest context: {out}"
    );
    assert!(out.contains("delivered into codex's prompt"), "{out}");

    // The instruction is IN the destination pane (cat echoed it).
    std::thread::sleep(std::time::Duration::from_millis(400));
    let captured = String::from_utf8(
        Command::new("tmux")
            .args(["capture-pane", "-t", &pane, "-p"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(
        captured.contains(&format!("Resolve {token} via waggle")),
        "injection landed: {captured}"
    );

    // Before the destination acts: not consumed.
    let (ok, out) = run(me, ws, &envs, &["status"]);
    assert!(ok, "{out}");
    assert!(out.contains("not yet"), "{out}");

    // The destination resolves (as itself) — consumption flips.
    let (ok, out) = run(&waggle_bin(), ws, &envs, &["resolve", "--token", &token]);
    assert!(ok, "{out}");
    let (ok, out) = run(me, ws, &envs, &["status"]);
    assert!(ok, "{out}");
    assert!(
        out.contains("yes — 1 resolve(s)"),
        "funnel-derived consumption: {out}"
    );
}

#[test]
fn tree_outcomes_mint_directories() {
    if !gated() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let store = ws.join("waggle.db").display().to_string();
    let sock = ws.join("no-daemon.sock").display().to_string();
    let envs: Vec<(&str, &str)> = vec![("WAGGLE_STORE", &store), ("WAGGLE_SOCK", &sock)];
    let me = env!("CARGO_BIN_EXE_waggle-tmux");

    std::fs::create_dir_all(ws.join("handoff")).unwrap();
    std::fs::write(ws.join("handoff/diff.patch"), "+ fix\n").unwrap();
    std::fs::write(ws.join("handoff/test.log"), "1 failed: bespoke case\n").unwrap();
    let (ok, out) = run(me, ws, &envs, &["mint", "handoff"]);
    assert!(ok, "{out}");
    assert!(out.contains("(tree: directory + children)"), "{out}");
    let token = out.split_whitespace().nth(1).unwrap().to_owned();

    // Deep search through the ROOT proves children snapshot-minted.
    let (ok, out) = run(
        &waggle_bin(),
        ws,
        &envs,
        &["search", "--token", &token, "--pattern", "bespoke"],
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("\"total_matches\": 1"),
        "deep search over the tree: {out}"
    );
}

/// Phase 4: full automation — an AGENT mints an outcome addressed by
/// channel (tmux/<dest>); `watch --once` sees it in the shared store
/// and performs the jump: the destination pane receives the resolve
/// instruction with no human courier.
#[test]
fn watch_auto_delivers_channel_addressed_outcomes() {
    if !gated() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let session = format!("waggle-watch-{}", std::process::id());
    let _guard = TmuxGuard(session.clone());
    let store = ws.join("waggle.db").display().to_string();
    let sock = ws.join("no-daemon.sock").display().to_string();
    let envs: Vec<(&str, &str)> = vec![("WAGGLE_STORE", &store), ("WAGGLE_SOCK", &sock)];
    let me = env!("CARGO_BIN_EXE_waggle-tmux");

    assert!(Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-n", "w", "cat"])
        .status()
        .unwrap()
        .success());
    let pane = String::from_utf8(
        Command::new("tmux")
            .args(["list-panes", "-t", &session, "-F", "#{pane_id}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let (ok, out) = run(
        me,
        ws,
        &envs,
        &["register", "codex", "--profile", "codex", "--pane", &pane],
    );
    assert!(ok, "{out}");
    let events = ws.join(".waggle/tmux/events.jsonl");
    let owned = std::fs::read_to_string(&events)
        .unwrap()
        .replace("\"owned\":false", "\"owned\":true");
    std::fs::write(&events, owned).unwrap();

    // THE AGENT mints, addressed by channel — no waggle-tmux involved.
    std::fs::write(ws.join("review.md"), "# Review\nship it\n").unwrap();
    let (ok, out) = run(
        &waggle_bin(),
        ws,
        &envs,
        &[
            "mint",
            "--target",
            &format!("file://{}/review.md", ws.display()),
            "--snapshot",
            "--channel",
            "tmux/codex",
        ],
    );
    assert!(ok, "{out}");

    // The watcher notices and jumps.
    let (ok, out) = run(me, ws, &envs, &["watch", "--once"]);
    assert!(ok, "{out}");
    assert!(out.contains("→ codex"), "watch narrated the jump: {out}");
    std::thread::sleep(std::time::Duration::from_millis(400));
    let captured = String::from_utf8(
        Command::new("tmux")
            .args(["capture-pane", "-t", &pane, "-p"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(
        captured.contains("via waggle for your working context"),
        "auto-delivered without a human courier: {captured}"
    );
}

/// Multiple outcomes: several paths mint as ONE lineage bundle (a note
/// root + every piece as a child — folders as trees), and a switch
/// delivers EVERYTHING queued for the destination; nothing is orphaned
/// by a later mint.
#[test]
fn bundles_and_the_pending_queue() {
    if !gated() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let session = format!("waggle-bundle-{}", std::process::id());
    let _guard = TmuxGuard(session.clone());
    let store = ws.join("waggle.db").display().to_string();
    let sock = ws.join("no-daemon.sock").display().to_string();
    let envs: Vec<(&str, &str)> = vec![("WAGGLE_STORE", &store), ("WAGGLE_SOCK", &sock)];
    let me = env!("CARGO_BIN_EXE_waggle-tmux");

    assert!(Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-n", "w", "cat"])
        .status()
        .unwrap()
        .success());
    let pane = String::from_utf8(
        Command::new("tmux")
            .args(["list-panes", "-t", &session, "-F", "#{pane_id}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let (ok, out) = run(
        me,
        ws,
        &envs,
        &["register", "codex", "--profile", "codex", "--pane", &pane],
    );
    assert!(ok, "{out}");
    let events = ws.join(".waggle/tmux/events.jsonl");
    let owned = std::fs::read_to_string(&events)
        .unwrap()
        .replace("\"owned\":false", "\"owned\":true");
    std::fs::write(&events, owned).unwrap();

    // The user's exact scenario: an images folder AND three files.
    std::fs::create_dir_all(ws.join("images")).unwrap();
    std::fs::write(ws.join("images/a.txt"), "img-a\n").unwrap();
    std::fs::write(ws.join("one.md"), "one\n").unwrap();
    std::fs::write(ws.join("two.md"), "two\n").unwrap();
    std::fs::write(ws.join("three.md"), "three\n").unwrap();

    let (ok, out) = run(
        me,
        ws,
        &envs,
        &[
            "mint", "images", "one.md", "two.md", "three.md", "--to", "codex",
        ],
    );
    assert!(ok, "{out}");
    assert!(out.contains("4 piece(s) as children"), "{out}");
    let root = out.split_whitespace().nth(2).unwrap().to_owned();

    // The root resolves to its index: 4 children, the folder among them.
    let (ok, out) = run(&waggle_bin(), ws, &envs, &["resolve", "--token", &root]);
    assert!(ok, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["result"]["children"].as_array().unwrap().len(),
        4,
        "{out}"
    );

    // A second, independent outcome QUEUES (old single-slot dropped it).
    std::fs::write(ws.join("extra.md"), "extra\n").unwrap();
    let (ok, out) = run(me, ws, &envs, &["mint", "extra.md", "--to", "codex"]);
    assert!(ok, "{out}");
    let extra = out.split_whitespace().nth(1).unwrap().to_owned();

    // One switch delivers BOTH queued handoffs.
    let (ok, out) = run(me, ws, &envs, &["switch", "codex"]);
    assert!(ok, "{out}");
    assert_eq!(
        out.matches("delivered into codex's prompt").count(),
        2,
        "both queued tokens travel on one switch: {out}"
    );
    std::thread::sleep(std::time::Duration::from_millis(600));
    let captured = String::from_utf8(
        Command::new("tmux")
            .args(["capture-pane", "-t", &pane, "-p", "-S", "-200", "-J"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    // Each instruction appears twice in a cat pane (tty echo + output) —
    // assert BOTH tokens were delivered rather than counting echoes.
    assert!(
        captured.contains(&format!("Resolve {root} via waggle")),
        "bundle root delivered: {captured}"
    );
    assert!(
        captured.contains(&format!("Resolve {extra} via waggle")),
        "queued extra delivered too: {captured}"
    );
}

/// Exits are first-class: when one harness dies, reap closes its
/// window (strip included) and FOREGROUNDS the survivor; when the last
/// one dies, the whole session closes gracefully.
#[test]
fn reap_foregrounds_the_survivor_then_closes_the_room() {
    if !gated() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path();
    let session = format!("waggle-reap-{}", std::process::id());
    let _guard = TmuxGuard(session.clone());
    let envs: Vec<(&str, &str)> = vec![];
    let me = env!("CARGO_BIN_EXE_waggle-tmux");

    // Two "harness" panes (cat) in separate windows + a strip each,
    // registered the way `up` would register them.
    assert!(Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session,
            "-n",
            "claude-code",
            "cat"
        ])
        .status()
        .unwrap()
        .success());
    let claude_pane = String::from_utf8(
        Command::new("tmux")
            .args(["list-panes", "-t", &session, "-F", "#{pane_id}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let codex_pane = String::from_utf8(
        Command::new("tmux")
            .args([
                "new-window",
                "-t",
                &session,
                "-n",
                "codex",
                "-P",
                "-F",
                "#{pane_id}",
                "cat",
            ])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    for (id, pane) in [("claude-code", &claude_pane), ("codex", &codex_pane)] {
        let (ok, out) = run(
            me,
            ws,
            &envs,
            &["register", id, "--profile", id, "--pane", pane],
        );
        assert!(ok, "{out}");
    }

    // Kill the claude "harness" (the /exit) and reap.
    assert!(Command::new("tmux")
        .args(["kill-pane", "-t", &claude_pane])
        .status()
        .unwrap()
        .success());
    let (ok, out) = run(me, ws, &envs, &["reap"]);
    assert!(ok, "{out}");

    // Survivor foregrounded: codex's window is now the active one.
    let active = String::from_utf8(
        Command::new("tmux")
            .args(["display-message", "-t", &session, "-p", "#{window_name}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(active.trim(), "codex", "the survivor is foregrounded");
    let (ok, out) = run(me, ws, &envs, &["status"]);
    assert!(
        ok && !out.contains("claude-code"),
        "closed session leaves status: {out}"
    );

    // The last harness dies -> the room closes.
    assert!(Command::new("tmux")
        .args(["kill-pane", "-t", &codex_pane])
        .status()
        .unwrap()
        .success());
    let (_ok, _out) = run(me, ws, &envs, &["reap"]);
    let alive = Command::new("tmux")
        .args(["has-session", "-t", &session])
        .status()
        .unwrap()
        .success();
    assert!(!alive, "last exit closes the whole session");
}
