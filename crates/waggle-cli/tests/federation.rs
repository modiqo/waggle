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
            .env("WAGGLE_IDLE_SECS", "20")
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

    fn tool(&mut self, name: &str, args: &serde_json::Value) -> serde_json::Value {
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
    let tcp = format!("127.0.0.1:{}", 40000 + std::process::id() % 20000);

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
        &serde_json::json!({ "target": format!("file://{}", report.display()), "snapshot": true }),
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
    let resolved = peer.tool("resolve", &serde_json::json!({ "token": token }));
    assert!(resolved["hint"].is_null(), "{resolved}");
    assert!(resolved["result"]["target"]
        .as_str()
        .unwrap()
        .contains("findings.md"));

    // The peer GREPS the owner's content: the file never left the owner's
    // disk; the search ran there; only the match traveled (08 §0).
    let found = peer.tool(
        "search",
        &serde_json::json!({ "token": token, "pattern": "bespoke" }),
    );
    assert!(found["hint"].is_null(), "{found}");
    assert_eq!(found["result"]["total_matches"], 1);

    // The peer reports work; the OWNER's funnel shows the whole story.
    let rec = peer.tool(
        "record",
        &serde_json::json!({ "token": token, "stage": "run" }),
    );
    assert!(rec["hint"].is_null());
    let funnel = owner.tool("funnel", &serde_json::json!({ "token": token }));
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
    let refused = lost.tool("resolve", &serde_json::json!({ "token": token }));
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
    let tcp_s = format!("127.0.0.1:{}", 40002 + std::process::id() % 20000);
    let tcp = tcp_s.as_str();

    let mut owner = Shim::spawn(
        &dir,
        &[
            ("WAGGLE_TCP", tcp.into()),
            ("WAGGLE_TCP_TOKEN", "correct-horse-battery-staple".into()),
        ],
    );
    owner.tool("map", &serde_json::json!({})); // ensure daemon is up

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

/// G-8 (15 §5.3, `it_strict_vs_eventual_revoke`): after the owner
/// revokes, a STRICT resolve at the peer sees the tombstone immediately;
/// an EVENTUAL resolve inside its revalidate window still serves the
/// cached resolution (the documented trade) — and once the window
/// passes, eventual re-consults and sees the revocation too.
#[test]
fn g8_strict_vs_eventual_revoke_and_offline_window() {
    let base = std::env::temp_dir().join(format!("waggle-g8-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    let owner_dir = base.join("owner");
    let peer_dir = base.join("peer");
    std::fs::create_dir_all(&owner_dir).unwrap();
    std::fs::create_dir_all(&peer_dir).unwrap();
    let gate = "g8-gate-token-0123456789abcdef".to_owned();
    let tcp = format!("127.0.0.1:{}", 40001 + std::process::id() % 20000);

    let mut owner = Shim::spawn(
        &owner_dir,
        &[
            ("WAGGLE_TCP", tcp.clone()),
            ("WAGGLE_TCP_TOKEN", gate.clone()),
        ],
    );
    // An explicit catch-all with a SHORT freshness window (800 ms) so the
    // test exercises the cache boundary without waiting 15 minutes.
    let minted = owner.tool(
        "mint",
        &serde_json::json!({
            "target": "ws://g8/artifact",
            "variants": [{
                "match": {},
                "body": { "inline": { "content_type": "text/plain", "data": "v1 content" } },
                "revalidate_after_ms": 800,
            }],
        }),
    );
    assert!(minted["hint"].is_null(), "{minted}");
    let token = minted["result"]["token"].as_str().unwrap().to_owned();

    // Pre-start the peer daemon with captured stderr (debug visibility).
    let peer_log = std::fs::File::create(base.join("peer.log")).unwrap();
    let mut peer_daemon = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["serve", "--daemon"])
        .env("WAGGLE_STORE", peer_dir.join("waggle.db"))
        .env("WAGGLE_SOCK", peer_dir.join("waggled.sock"))
        .env("WAGGLE_UPSTREAM", tcp.clone())
        .env("WAGGLE_UPSTREAM_TOKEN", gate.clone())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(peer_log)
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let mut peer = Shim::spawn(
        &peer_dir,
        &[
            ("WAGGLE_UPSTREAM", tcp.clone()),
            ("WAGGLE_UPSTREAM_TOKEN", gate.clone()),
        ],
    );

    // Prime the peer's cache (eventual is the default).
    let first = peer.tool("resolve", &serde_json::json!({ "token": token }));
    assert_eq!(
        first["result"]["disposition"], "active",
        "prime failed: {first}"
    );

    // The owner revokes.
    let revoked = owner.tool(
        "mutate",
        &serde_json::json!({ "token": token, "change": "revoke", "expected-version": 1 }),
    );
    assert!(revoked["hint"].is_null(), "{revoked}");

    // EVENTUAL inside the window: still the cached (stale) resolution —
    // the documented trade, stamped with its own revalidate_after.
    let eventual = peer.tool("resolve", &serde_json::json!({ "token": token }));
    assert_eq!(
        eventual["result"]["disposition"], "active",
        "eventual serves cache inside the window: {eventual}"
    );

    // STRICT at the peer: the tombstone bites immediately…
    let strict = peer.tool(
        "resolve",
        &serde_json::json!({ "token": token, "level": "strict" }),
    );
    assert!(
        strict["result"]["disposition"]
            .to_string()
            .contains("revoked"),
        "strict must see the revocation: {strict}"
    );

    // …and strict REFRESHES the shared cache — the next eventual serves
    // the newer knowledge instead of the stale entry.
    let refreshed = peer.tool("resolve", &serde_json::json!({ "token": token }));
    assert!(
        refreshed["result"]["disposition"]
            .to_string()
            .contains("revoked"),
        "strict updates what eventual serves next: {refreshed}"
    );

    owner.close();
    peer.close();
    let _ = peer_daemon.kill();
    let _ = peer_daemon.wait();
    for d in [&owner_dir, &peer_dir] {
        stop_daemon(d);
    }
    std::fs::remove_dir_all(&base).ok();
}

/// Slice 3: CLI verbs federate too — `waggle resolve` on the peer
/// machine routes through the peer daemon and reaches the owner, exactly
/// like MCP traffic. One dispatcher, one behavior, both surfaces.
#[test]
fn cli_verbs_federate_through_the_daemon() {
    let base = std::env::temp_dir().join(format!("waggle-clifed-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    let owner_dir = base.join("owner");
    let peer_dir = base.join("peer");
    std::fs::create_dir_all(&owner_dir).unwrap();
    std::fs::create_dir_all(&peer_dir).unwrap();
    let gate = "clifed-gate-0123456789abcdef".to_owned();
    let tcp = format!("127.0.0.1:{}", 40003 + std::process::id() % 20000);

    let mut owner = Shim::spawn(
        &owner_dir,
        &[
            ("WAGGLE_TCP", tcp.clone()),
            ("WAGGLE_TCP_TOKEN", gate.clone()),
        ],
    );
    let minted = owner.tool("mint", &serde_json::json!({ "target": "ws://clifed/x" }));
    let token = minted["result"]["token"].as_str().unwrap().to_owned();

    // Start the peer daemon (upstream configured), then use the PLAIN CLI.
    let mut peer = Shim::spawn(
        &peer_dir,
        &[
            ("WAGGLE_UPSTREAM", tcp.clone()),
            ("WAGGLE_UPSTREAM_TOKEN", gate.clone()),
        ],
    );
    peer.tool("map", &serde_json::json!({})); // ensure the daemon is up

    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["resolve", "--token", &token])
        .env("WAGGLE_STORE", peer_dir.join("waggle.db"))
        .env("WAGGLE_SOCK", peer_dir.join("waggled.sock"))
        .output()
        .unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert!(out.status.success(), "{envelope}");
    assert_eq!(
        envelope["result"]["target"], "ws://clifed/x",
        "the CLI resolve federated to the owner: {envelope}"
    );

    owner.close();
    peer.close();
    for d in [&owner_dir, &peer_dir] {
        stop_daemon(d);
    }
    std::fs::remove_dir_all(&base).ok();
}

/// E3 at the edge (CP-10e): THE THREE-TIER CHAIN — plain CLI → local
/// waggled → HTTP upstream → the edge worker. Gated on `WAGGLE_EDGE_URL`
/// (+ `WAGGLE_EDGE_BEARER`): the `just edge-test` / CI recipe boots
/// wrangler and exports them.
#[test]
fn e3_three_tier_chain_to_the_edge() {
    let Ok(edge_url) = std::env::var("WAGGLE_EDGE_URL") else {
        eprintln!("skipped: set WAGGLE_EDGE_URL (see `just edge-test`)");
        return;
    };
    let bearer = std::env::var("WAGGLE_EDGE_BEARER").unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("waggle-3tier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    // Tier 1+2: a local shim/daemon with the EDGE as its upstream.
    let local = Shim::spawn(
        &dir,
        &[
            ("WAGGLE_UPSTREAM", edge_url.clone()),
            ("WAGGLE_UPSTREAM_TOKEN", bearer.clone()),
        ],
    );

    // Mint AT THE EDGE (through its /mcp), so the token is edge-owned.
    let mint_frame = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "mint", "arguments": { "target": "ws://three-tier/x" } },
    });
    let resp = ureq_post(&edge_url, &bearer, &mint_frame.to_string());
    let rpc: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let token = envelope["result"]["token"].as_str().unwrap().to_owned();

    // The chain: CLI → daemon → HTTP → edge. One command, three tiers.
    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["resolve", "--token", &token])
        .env("WAGGLE_STORE", dir.join("waggle.db"))
        .env("WAGGLE_SOCK", dir.join("waggled.sock"))
        .output()
        .unwrap();
    let resolved: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert!(out.status.success(), "{resolved}");
    assert_eq!(
        resolved["result"]["target"], "ws://three-tier/x",
        "the CLI reached the edge through the daemon: {resolved}"
    );

    // Strict revocation bites end to end: revoke at the edge, strict
    // resolve from the CLI sees the tombstone.
    let revoke = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "mutate", "arguments": {
            "token": token, "change": "revoke", "expected-version": 1 } },
    });
    ureq_post(&edge_url, &bearer, &revoke.to_string());
    let out = Command::new(env!("CARGO_BIN_EXE_waggle"))
        .args(["resolve", "--token", &token, "--level", "strict"])
        .env("WAGGLE_STORE", dir.join("waggle.db"))
        .env("WAGGLE_SOCK", dir.join("waggled.sock"))
        .output()
        .unwrap();
    let strict: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert!(
        strict["result"]["disposition"]
            .to_string()
            .contains("revoked"),
        "strict revocation across three tiers: {strict}"
    );

    local.close();
    stop_daemon(&dir);
    std::fs::remove_dir_all(&dir).ok();
}

/// Minimal HTTP POST for the test (std only — no client dep in the CLI).
fn ureq_post(base: &str, bearer: &str, body: &str) -> String {
    use std::io::{Read, Write};
    let host = base.trim_start_matches("http://").trim_end_matches('/');
    let mut stream = std::net::TcpStream::connect(host).unwrap();
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nhost: {host}\r\nauthorization: Bearer {bearer}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut raw = String::new();
    stream.read_to_string(&mut raw).unwrap();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        // decode enough for one small chunked body
        let mut out = String::new();
        let mut rest = body;
        while let Some((size, tail)) = rest.split_once("\r\n") {
            let Ok(n) = usize::from_str_radix(size.trim(), 16) else {
                break;
            };
            if n == 0 {
                break;
            }
            out.push_str(&tail[..n.min(tail.len())]);
            rest = tail.get(n..).map_or("", |r| r.trim_start_matches("\r\n"));
        }
        out
    } else {
        body.to_owned()
    }
}
