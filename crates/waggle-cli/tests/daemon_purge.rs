//! `waggle daemon purge`: the zombie killer — reaches daemons whose
//! sockets and pidfiles are GONE (deleted temp dirs, crashed tests),
//! which `stop` structurally cannot. Own test binary: it must not run
//! beside tests that keep daemons alive on purpose.

#![cfg(unix)]

use std::process::{Command, Stdio};

#[test]
fn purge_reaps_zombies_with_deleted_sockets() {
    // Purge is deliberately nuclear: it reaps EVERY waggled this user
    // owns — including a real one on the default socket. Gated so
    // `cargo test` on a dev machine never murders the daemon you're
    // dogfooding with; CI (no real daemon) always sets the gate.
    if std::env::var("WAGGLE_PURGE_TEST").is_err() {
        eprintln!("skipped: set WAGGLE_PURGE_TEST=1 (kills ALL your waggled processes)");
        return;
    }
    // Two daemons whose state dirs we delete out from under them — the
    // exact zombie shape that poisoned earlier test runs.
    let mut zombies = Vec::new();
    for i in 0..2 {
        let dir = std::env::temp_dir().join(format!("waggle-purge-{i}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_waggle"))
            .args(["serve", "--daemon"])
            .env("WAGGLE_STORE", dir.join("waggle.db"))
            .env("WAGGLE_SOCK", dir.join("waggled.sock"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        std::fs::remove_dir_all(&dir).unwrap(); // socket + pidfile: gone
        zombies.push(child);
    }
    let pids: Vec<u32> = zombies.iter().map(std::process::Child::id).collect();

    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["daemon", "purge"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let report: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    let purged: Vec<u32> = report["purged"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(serde_json::Value::as_u64)
        .map(|p| u32::try_from(p).unwrap())
        .collect();
    for pid in &pids {
        assert!(
            purged.contains(pid),
            "zombie {pid} reaped; report: {report}"
        );
    }
    for mut z in zombies {
        let status = z.wait().unwrap(); // reaped by us — no lingering process
        assert!(!status.success(), "terminated by signal, not clean exit");
    }
}
