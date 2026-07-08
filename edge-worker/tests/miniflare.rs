//! CP-10e, the Miniflare rows (design doc `08 §9`): the worker under
//! workerd via `wrangler dev`, driven over real HTTP. Gated behind
//! `WAGGLE_EDGE_TESTS=1` (needs node + npx); the edge CI job sets it.
//!
//! Run locally: `just edge-test`.

#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

const BEARER: &str = "dev-tenant-token-0123456789abcdef";

fn gated() -> bool {
    std::env::var("WAGGLE_EDGE_TESTS").is_ok()
}

struct Wrangler {
    child: Child,
    port: u16,
}

impl Drop for Wrangler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

static PORT: OnceLock<u16> = OnceLock::new();

fn boot() -> Wrangler {
    if let Ok(p) = std::env::var("WAGGLE_EDGE_EXTERNAL_PORT") {
        // An orchestrator (just edge-test / CI) already booted wrangler.
        return Wrangler {
            child: Command::new("true").spawn().unwrap(),
            port: p.parse().unwrap(),
        };
    }
    let port = *PORT.get_or_init(|| 43700 + (std::process::id() % 1000) as u16);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let child = Command::new("npx")
        .args([
            "--yes",
            "wrangler",
            "dev",
            "--port",
            &port.to_string(),
            "--local-protocol",
            "http",
        ])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("npx wrangler dev");
    let mf = Wrangler { child, port };
    // First boot compiles the worker (worker-build) — allow minutes.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(420);
    loop {
        if let Ok(resp) = ureq::get(&format!("http://127.0.0.1:{port}/health")).call() {
            if resp.status() == 200 {
                break;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "wrangler dev never became healthy"
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    mf
}

fn post(port: u16, path: &str, bearer: Option<&str>, body: &str) -> (u16, String) {
    let mut req = ureq::post(&format!("http://127.0.0.1:{port}{path}"));
    if let Some(b) = bearer {
        req = req.set("authorization", &format!("Bearer {b}"));
    }
    match req.send_string(body) {
        Ok(resp) => {
            let status = resp.status();
            (status, resp.into_string().unwrap_or_default())
        }
        Err(ureq::Error::Status(code, resp)) => (code, resp.into_string().unwrap_or_default()),
        Err(e) => panic!("http: {e}"),
    }
}

fn store_op(port: u16, body: &serde_json::Value) -> serde_json::Value {
    let (status, text) = post(port, "/store", Some(BEARER), &body.to_string());
    assert_eq!(status, 200, "{text}");
    serde_json::from_str(&text).unwrap()
}

/// One boot, every row — wrangler startup is the expensive part.
#[test]
fn miniflare_matrix() {
    if !gated() {
        eprintln!("skipped: set WAGGLE_EDGE_TESTS=1 (needs node) — `just edge-test`");
        return;
    }
    let mf = boot();
    let port = mf.port;

    // ── E8: auth — no bearer and wrong bearer are refused; /health open.
    let (code, _) = post(
        port,
        "/mcp",
        None,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(code, 401, "E8: missing bearer refused");
    let (code, _) = post(
        port,
        "/mcp",
        Some("wrong-token-wrong-token"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(code, 401, "E8: wrong bearer refused");

    // ── E7: the tool surface on /mcp is the catalog, exactly.
    let (code, text) = post(
        port,
        "/mcp",
        Some(BEARER),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(code, 200);
    let rpc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let names: Vec<&str> = rpc["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for op in waggle_ops_names() {
        assert!(
            names.contains(&op),
            "E7: `{op}` missing from the edge tool list"
        );
    }

    // ── E4: replay migration — a local SQLite store streams to the edge
    //     over /store ingest, every record TWICE (the injected retry),
    //     with a mid-replay "kill" (stop + restart the stream).
    let (records, token) = seed_local_store();
    let half = records.len() / 2;
    for rec in &records[..half] {
        store_op(port, &serde_json::json!({ "op": "ingest", "record": rec }));
    }
    // the "kill": abandon; resume FROM ZERO with duplicates (C-4 absorbs)
    for rec in records.iter().chain(records.iter()) {
        store_op(port, &serde_json::json!({ "op": "ingest", "record": rec }));
    }
    let scanned = store_op(port, &serde_json::json!({ "op": "scan" }));
    let edge_records: Vec<waggle_core::LogRecord> =
        serde_json::from_value(scanned["ok"].clone()).unwrap();
    let a = serde_json::to_string(&waggle_core::reconstruct(records.clone())).unwrap();
    let b = serde_json::to_string(&waggle_core::reconstruct(edge_records)).unwrap();
    assert_eq!(a, b, "E4: destination ≡ source after kill+retry replay");

    // ── E6 (Miniflare half): resolve the migrated token via /mcp; the
    //     answer must match the local resolution byte-for-byte (envelope
    //     `result.target`/`disposition` agreement is the probe).
    let frame = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"resolve","arguments":{{"token":"{token}"}}}}}}"#
    );
    let (_, text) = post(port, "/mcp", Some(BEARER), &frame);
    let rpc: serde_json::Value = serde_json::from_str(&text).unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(
        envelope["hint"].is_null(),
        "E6: edge resolve failed: {envelope}"
    );
    assert_eq!(envelope["result"]["target"], "ws://edge-mig/artifact");

    // ── E11: resolve p50 through the full HTTP+DO path.
    let mut samples: Vec<u128> = (0..40)
        .map(|_| {
            let start = std::time::Instant::now();
            let _ = post(port, "/mcp", Some(BEARER), &frame);
            start.elapsed().as_micros()
        })
        .collect();
    samples.sort_unstable();
    let p50 = samples[samples.len() / 2];
    println!("E11: edge resolve p50 {p50} µs (budget 10 ms local-Miniflare)");
    assert!(p50 < 10_000, "E11: p50 {p50} µs exceeds the 10 ms budget");

    // ── E9: the public unfurl carries OG meta from the snapshot.
    let resp = ureq::get(&format!("http://127.0.0.1:{port}/t/{token}"))
        .call()
        .unwrap();
    let html = resp.into_string().unwrap();
    assert!(html.contains("og:title"), "E9: {html}");
    assert!(html.contains("ws://edge-mig/artifact"));

    // E3 (three-tier chain) lives in waggle-cli's federation tests,
    // gated on WAGGLE_EDGE_URL — the just/CI recipe boots wrangler once
    // and runs both suites against it.
}

fn waggle_ops_names() -> Vec<&'static str> {
    vec![
        "mint", "resolve", "record", "mutate", "funnel", "read", "search", "query", "map",
    ]
}

fn seed_local_store() -> (Vec<waggle_core::LogRecord>, String) {
    use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
    pollster::block_on(async {
        let local = waggle_store_sqlite::SqliteStore::open_in_memory().unwrap();
        let mut entropy = |b: &mut [u8]| {
            b.fill(91);
            Ok(())
        };
        let manifest = waggle_core::mint(
            waggle_core::MintSpec::new(
                waggle_core::CanonicalUrl::new("ws://edge-mig/artifact").unwrap(),
                waggle_core::Sharer::new("lead").unwrap(),
                waggle_core::Channel::subagent_general(),
            ),
            &waggle_core::MintOptions::default(),
            &mut entropy,
            waggle_core::Timestamp::from_unix_ms(1),
        )
        .unwrap();
        let token = manifest.token;
        local
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(1),
            })
            .await
            .unwrap();
        for stage in [waggle_core::Stage::resolve(), waggle_core::Stage::run()] {
            local
                .append(AppendIntent::Event {
                    token,
                    stage,
                    actor: waggle_core::ActorClass::from_context(
                        &waggle_core::ResolverContext::anonymous_agent(),
                    ),
                    variant: None,
                    at: waggle_core::Timestamp::from_unix_ms(2),
                })
                .await
                .unwrap();
        }
        (local.scan_all().await.unwrap(), token.as_str().to_owned())
    })
}
