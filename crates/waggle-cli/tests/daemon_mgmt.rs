//! Daemon lifecycle (16 appendix): status/start/stop/restart, orphan
//! diagnosis, and idle exit — no lingering daemons, by construction.

#![cfg(unix)]

use std::process::{Command, Stdio};

fn env_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-dmgmt-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn waggle(dir: &std::path::Path, args: &[&str]) -> (serde_json::Value, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(args)
        .env("WAGGLE_STORE", dir.join("waggle.db"))
        .env("WAGGLE_SOCK", dir.join("waggled.sock"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Compound actions (restart) print a receipt per step; the last line
    // is the final state.
    let last = stdout.trim().lines().last().unwrap_or("");
    let v = serde_json::from_str(last).unwrap_or(serde_json::Value::Null);
    (v, out.status.success())
}

#[test]
fn lifecycle_status_start_stop_restart() {
    let dir = env_dir("cycle");

    // Nothing running: status says so, exit 1.
    let (_, ok) = waggle(&dir, &["daemon", "status"]);
    assert!(!ok, "status on nothing exits 1");

    // Start: comes up, reports pid + store.
    let (status, ok) = waggle(&dir, &["daemon", "start"]);
    assert!(ok, "{status}");
    let pid1 = status["pid"].as_u64().expect("pid in status");
    assert!(status["store"].as_str().unwrap().contains("waggle.db"));
    assert!(dir.join("waggled.pid").exists(), "pidfile written");

    // Idempotent start.
    let (again, ok) = waggle(&dir, &["daemon", "start"]);
    assert!(ok);
    assert_eq!(again["already_running"], true);

    // Restart: a NEW pid.
    let (restarted, ok) = waggle(&dir, &["daemon", "restart"]);
    assert!(ok, "{restarted}");
    let pid2 = restarted["pid"].as_u64().unwrap();
    assert_ne!(pid1, pid2, "restart yields a fresh daemon");

    // Stop: clean exit, files swept, status finds nothing.
    let (stopped, ok) = waggle(&dir, &["daemon", "stop"]);
    assert!(ok, "{stopped}");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!dir.join("waggled.sock").exists(), "socket cleaned");
    assert!(!dir.join("waggled.pid").exists(), "pidfile cleaned");
    let (_, ok) = waggle(&dir, &["daemon", "status"]);
    assert!(!ok);

    // Stop when nothing runs: honest exit 1.
    let (_, ok) = waggle(&dir, &["daemon", "stop"]);
    assert!(!ok);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn idle_daemon_exits_and_cleans_up() {
    let dir = env_dir("idle");
    let mut child = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["serve", "--daemon"])
        .env("WAGGLE_STORE", dir.join("waggle.db"))
        .env("WAGGLE_SOCK", dir.join("waggled.sock"))
        .env("WAGGLE_IDLE_SECS", "2")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Wait past the idle window (checks run every idle/4 ≥ 1s).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "idle daemon must exit"
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    assert!(
        !dir.join("waggled.sock").exists(),
        "idle exit sweeps the socket"
    );
    assert!(
        !dir.join("waggled.pid").exists(),
        "idle exit sweeps the pidfile"
    );
    std::fs::remove_dir_all(&dir).ok();
}
