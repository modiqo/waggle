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
