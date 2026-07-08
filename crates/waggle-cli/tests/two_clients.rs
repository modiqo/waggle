//! CP-6 gate (16 §6): two clients, one daemon, one store. A Claude-Code-
//! like client and a Codex-like client each spawn `waggle serve --stdio`
//! (the shim); both sessions land on the same waggled process; what one
//! mints, the other resolves.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Client {
    child: Child,
    stdin: ChildStdin,
    lines: std::io::Lines<BufReader<ChildStdout>>,
}

impl Client {
    fn spawn(store: &std::path::Path, sock: &std::path::Path, sharer: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_waggle"))
            .args(["serve", "--stdio"])
            .env("WAGGLE_STORE", store)
            .env("WAGGLE_SOCK", sock)
            .env("WAGGLE_SHARER", sharer)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn shim");
        let stdin = child.stdin.take().unwrap();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            stdin,
            lines,
        }
    }

    fn call(&mut self, frame: &str) -> serde_json::Value {
        writeln!(self.stdin, "{frame}").unwrap();
        self.stdin.flush().unwrap();
        let line = self.lines.next().expect("a response").unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn tool(&mut self, id: u32, name: &str, args: &serde_json::Value) -> serde_json::Value {
        let frame = serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": args },
        });
        let rpc = self.call(&frame.to_string());
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    fn close(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

#[test]
fn two_clients_one_daemon_share_the_store() {
    let dir = std::env::temp_dir().join(format!("waggle-2c-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let store = dir.join("waggle.db");
    let sock = dir.join("waggled.sock");

    // Start waggled explicitly (the shim can auto-start it too; here we
    // want a handle to clean up).
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["serve", "--daemon"])
        .env("WAGGLE_STORE", &store)
        .env("WAGGLE_SOCK", &sock)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..50 {
        if sock.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Two harnesses, two shim processes, one daemon underneath (16 §6).
    let mut claude_like = Client::spawn(&store, &sock, "claude-session");
    let mut codex_like = Client::spawn(&store, &sock, "codex-session");

    let init = claude_like.call(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    assert_eq!(init["result"]["serverInfo"]["name"], "waggled");
    codex_like.call(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);

    // Client A mints; client B resolves A's token in ITS OWN session —
    // cross-client visibility through the shared owner process.
    let minted = claude_like.tool(
        2,
        "mint",
        &serde_json::json!({ "target": "ws://shared/findings.md" }),
    );
    assert!(minted["hint"].is_null(), "{minted}");
    let token = minted["result"]["token"].as_str().unwrap().to_owned();

    let resolved = codex_like.tool(2, "resolve", &serde_json::json!({ "token": token }));
    assert!(resolved["hint"].is_null(), "{resolved}");
    assert_eq!(resolved["result"]["target"], "ws://shared/findings.md");

    // And the funnel, asked by A, reflects B's resolve.
    let funnel = claude_like.tool(3, "funnel", &serde_json::json!({ "token": token }));
    assert_eq!(funnel["result"]["stages"]["resolve"], 1);

    claude_like.close();
    codex_like.close();
    let _ = daemon.kill();
    let _ = daemon.wait();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn shim_auto_starts_the_daemon() {
    let dir = std::env::temp_dir().join(format!("waggle-auto-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let store = dir.join("waggle.db");
    let sock = dir.join("waggled.sock");

    // No daemon running: the shim must arrange one and still answer.
    let mut client = Client::spawn(&store, &sock, "solo");
    let init = client.call(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    assert_eq!(init["result"]["serverInfo"]["name"], "waggled");
    let minted = client.tool(
        2,
        "mint",
        &serde_json::json!({ "target": "ws://auto/x.md" }),
    );
    assert!(minted["hint"].is_null());
    client.close();

    // Clean up the auto-started daemon by connecting and letting the
    // directory removal take the socket; the daemon is idle and harmless,
    // but kill it via its socket path being removed + test process exit.
    std::fs::remove_dir_all(&dir).ok();
}
