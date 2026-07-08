//! CP-10 slice 1: two waggleds, one handoff (16 §3). The OWNER daemon
//! listens on token-gated TCP; the PEER daemon (a different machine in
//! spirit: different store, different socket) forwards frames for tokens
//! it doesn't own. The computation runs at the owner — a remote `search`
//! greps the owner's snapshot; only matches travel (08 §0).

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Shim {
    child: Child,
    stdin: ChildStdin,
    lines: std::io::Lines<BufReader<ChildStdout>>,
}

impl Shim {
    fn spawn(dir: &std::path::Path, extra_env: &[(&str, String)]) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_waggle"));
        cmd.args(["serve", "--stdio"])
            .env("WAGGLE_STORE", dir.join("waggle.db"))
            .env("WAGGLE_SOCK", dir.join("waggled.sock"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            stdin,
            lines,
        }
    }

    fn tool(&mut self, name: &str, args: serde_json::Value) -> serde_json::Value {
        let frame = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args },
        });
        writeln!(self.stdin, "{frame}").unwrap();
        self.stdin.flush().unwrap();
        let line = self.lines.next().unwrap().unwrap();
        let rpc: serde_json::Value = serde_json::from_str(&line).unwrap();
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn close(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

fn stop_daemon(dir: &std::path::Path) {
    let _ = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["daemon", "stop"])
        .env("WAGGLE_STORE", dir.join("waggle.db"))
        .env("WAGGLE_SOCK", dir.join("waggled.sock"))
        .output();
}

#[test]
fn two_waggleds_one_handoff_computation_at_the_owner() {
    let base = std::env::temp_dir().join(format!("waggle-fed-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    let owner_dir = base.join("owner");
    let peer_dir = base.join("peer");
    std::fs::create_dir_all(&owner_dir).unwrap();
    std::fs::create_dir_all(&peer_dir).unwrap();
    let gate = "test-gate-token-0123456789abcdef".to_owned();
    let tcp = "127.0.0.1:47611".to_owned();

    // The artifact lives ONLY on the owner's machine.
    let report = owner_dir.join("findings.md");
    std::fs::write(
        &report,
        "# Findings\n\n## Pricing\nenterprise pricing is bespoke\n",
    )
    .unwrap();

    // OWNER: unix socket + token-gated TCP.
    let mut owner = Shim::spawn(
        &owner_dir,
        &[
            ("WAGGLE_TCP", tcp.clone()),
            ("WAGGLE_TCP_TOKEN", gate.clone()),
            ("WAGGLE_SHARER", "owner".into()),
        ],
    );
    let minted = owner.tool(
        "mint",
        serde_json::json!({ "target": format!("file://{}", report.display()), "snapshot": true }),
    );
    assert!(minted["hint"].is_null(), "{minted}");
    let token = minted["result"]["token"].as_str().unwrap().to_owned();

    // PEER: a different store entirely, upstream pointed at the owner.
    let mut peer = Shim::spawn(
        &peer_dir,
        &[
            ("WAGGLE_UPSTREAM", tcp.clone()),
            ("WAGGLE_UPSTREAM_TOKEN", gate.clone()),
            ("WAGGLE_SHARER", "peer".into()),
        ],
    );

    // The peer resolves the OWNER's token — forwarded, answered.
    let resolved = peer.tool("resolve", serde_json::json!({ "token": token }));
    assert!(resolved["hint"].is_null(), "{resolved}");
    assert!(resolved["result"]["target"]
        .as_str()
        .unwrap()
        .contains("findings.md"));

    // The peer GREPS the owner's content: the file never left the owner's
    // disk; the search ran there; only the match traveled (08 §0).
    let found = peer.tool(
        "search",
        serde_json::json!({ "token": token, "pattern": "bespoke" }),
    );
    assert!(found["hint"].is_null(), "{found}");
    assert_eq!(found["result"]["total_matches"], 1);

    // The peer reports work; the OWNER's funnel shows the whole story.
    let rec = peer.tool(
        "record",
        serde_json::json!({ "token": token, "stage": "run" }),
    );
    assert!(rec["hint"].is_null());
    let funnel = owner.tool("funnel", serde_json::json!({ "token": token }));
    assert_eq!(
        funnel["result"]["stages"]["resolve"], 1,
        "the remote resolve landed at the owner"
    );
    assert_eq!(funnel["result"]["stages"]["run"], 1);
    assert_eq!(
        funnel["result"]["stages"]["read"], 1,
        "the remote search landed too"
    );

    // A peer WITHOUT upstream config gets the honest refusal.
    let lost_dir = base.join("lost");
    std::fs::create_dir_all(&lost_dir).unwrap();
    let mut lost = Shim::spawn(&lost_dir, &[]);
    let refused = lost.tool("resolve", serde_json::json!({ "token": token }));
    assert!(refused["hint"].as_str().unwrap().contains("unknown token"));

    owner.close();
    peer.close();
    lost.close();
    for d in [&owner_dir, &peer_dir, &lost_dir] {
        stop_daemon(d);
    }
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn tcp_gate_rejects_bad_bearers() {
    use std::io::Read as _;
    let base = std::env::temp_dir().join(format!("waggle-gate-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    let dir = base.join("owner");
    std::fs::create_dir_all(&dir).unwrap();
    let tcp = "127.0.0.1:47612";

    let mut owner = Shim::spawn(
        &dir,
        &[
            ("WAGGLE_TCP", tcp.into()),
            ("WAGGLE_TCP_TOKEN", "correct-horse-battery-staple".into()),
        ],
    );
    owner.tool("map", serde_json::json!({})); // ensure daemon is up

    // Wrong bearer: the connection is dropped without a byte served.
    let mut bad = std::net::TcpStream::connect(tcp).unwrap();
    writeln!(
        bad,
        r#"{{"jsonrpc":"2.0","method":"waggled/hello","params":{{"token":"WRONG"}}}}"#
    )
    .unwrap();
    writeln!(bad, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).unwrap();
    bad.set_read_timeout(Some(std::time::Duration::from_millis(800)))
        .unwrap();
    let mut buf = Vec::new();
    let _ = bad.read_to_end(&mut buf);
    assert!(
        buf.is_empty(),
        "rejected connections receive nothing, got: {buf:?}"
    );

    owner.close();
    stop_daemon(&dir);
    std::fs::remove_dir_all(&base).ok();
}
