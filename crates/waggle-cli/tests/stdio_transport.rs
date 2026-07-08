//! CP-6 gate (partial): the stdio transport, end to end — spawn the real
//! `waggle serve --stdio` binary, speak MCP JSON-RPC over its pipes, and
//! confirm the store on disk saw it all. This is the exact process a
//! harness config launches (design doc `16 §3`).

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn temp_store(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-stdio-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("waggle.db")
}

#[test]
fn serve_stdio_speaks_mcp_and_persists() {
    let store = temp_store("mcp");
    let mut child = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["serve", "--stdio"])
        .env("WAGGLE_STORE", &store)
        .env("WAGGLE_SHARER", "harness-test")
        .env("WAGGLE_DIRECT", "1") // the direct server; the shim path has its own test
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn waggle serve --stdio");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let mut lines = stdout.lines();
    let mut send = |frame: &str| {
        writeln!(stdin, "{frame}").unwrap();
        stdin.flush().unwrap();
    };
    let mut recv = || -> serde_json::Value {
        let line = lines.next().expect("a response line").unwrap();
        serde_json::from_str(&line).expect("valid JSON-RPC")
    };

    // Handshake.
    send(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    let init = recv();
    assert_eq!(init["result"]["serverInfo"]["name"], "waggled");

    // A notification gets NO response line (next response must be id 2).
    send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    send(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    let list = recv();
    assert_eq!(list["id"], 2, "notifications are silent");
    assert!(list["result"]["tools"].as_array().unwrap().len() >= 6);

    // Mint over the wire; capture the token from the envelope.
    send(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"mint","arguments":{"target":"file:///tmp/findings.md"}}}"#,
    );
    let minted = recv();
    let envelope: serde_json::Value =
        serde_json::from_str(minted["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let token = envelope["result"]["token"].as_str().unwrap().to_owned();

    // Resolve the token in the same session.
    send(&format!(
        r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"resolve","arguments":{{"token":"{token}"}}}}}}"#
    ));
    let resolved = recv();
    let envelope: serde_json::Value =
        serde_json::from_str(resolved["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(envelope["result"]["target"], "file:///tmp/findings.md");

    drop(stdin); // EOF ends the session cleanly
    let status = child.wait().unwrap();
    assert!(status.success());

    // A SECOND process sees the first one's writes: the store is the
    // durable thing; the server is just a door (16 §2).
    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["funnel", "--token", &token])
        .env("WAGGLE_STORE", &store)
        .output()
        .unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
    assert_eq!(
        envelope["result"]["stages"]["resolve"], 1,
        "the resolve was recorded durably"
    );
    std::fs::remove_dir_all(store.parent().unwrap()).ok();
}

#[test]
fn cli_verbs_share_the_mcp_dispatcher() {
    // waggle mint → waggle resolve → waggle map, three processes, one
    // store: CLI and MCP are projections of the same handler (09 §2).
    let store = temp_store("verbs");
    let run = |args: &[&str]| -> (serde_json::Value, bool) {
        let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
            .args(args)
            .env("WAGGLE_STORE", &store)
            .output()
            .unwrap();
        let v = serde_json::from_str(std::str::from_utf8(&out.stdout).unwrap()).unwrap();
        (v, out.status.success())
    };

    let (minted, ok) = run(&["mint", "--target", "ws://cli/report.md"]);
    assert!(ok, "{minted}");
    let token = minted["result"]["token"].as_str().unwrap().to_owned();
    assert!(minted["result"]["handoff"]
        .as_str()
        .unwrap()
        .contains(&token));

    let (resolved, ok) = run(&["resolve", "--token", &token]);
    assert!(ok);
    assert!(resolved["result"]["body"].is_object());

    let (map, ok) = run(&["map", "--token", &token]);
    assert!(ok);
    assert!(map["result"]["here"].as_str().unwrap().contains(&token));

    // And the error contract holds at the process boundary: exit 1 + hint.
    let (err, ok) = run(&["resolve", "--token", "zzzzz"]);
    assert!(!ok, "unknown token exits nonzero");
    assert!(err["hint"].as_str().unwrap().contains("unknown token"));
    std::fs::remove_dir_all(store.parent().unwrap()).ok();
}
