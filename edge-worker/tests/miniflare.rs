//! CP-10e, the Miniflare rows (design doc `08 §9`): the worker under
//! workerd via `wrangler dev`, driven over real HTTP. Gated behind
//! `WAGGLE_EDGE_TESTS=1` (needs node + npx); the edge CI job sets it.
//!
//! Run locally: `just edge-test`.

#![cfg(unix)]

use base64::Engine as _;
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
    post_t(port, path, bearer, None, body)
}

fn post_t(
    port: u16,
    path: &str,
    bearer: Option<&str>,
    tenant: Option<&str>,
    body: &str,
) -> (u16, String) {
    let mut req = ureq::post(&format!("http://127.0.0.1:{port}{path}"));
    if let Some(b) = bearer {
        req = req.set("authorization", &format!("Bearer {b}"));
    }
    if let Some(t) = tenant {
        req = req.set("x-waggle-tenant", t);
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
    store_op_t(port, None, body)
}

fn store_op_t(port: u16, tenant: Option<&str>, body: &serde_json::Value) -> serde_json::Value {
    let (status, text) = post_t(port, "/store", Some(BEARER), tenant, &body.to_string());
    assert_eq!(status, 200, "{text}");
    serde_json::from_str(&text).unwrap()
}

/// One boot, every row — wrangler startup is the expensive part.
#[test]
#[allow(clippy::too_many_lines)] // the matrix IS the narrative; splitting hides the flow
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

    // ── E5: computation at the data — push a snapshot blob, ingest a
    //     manifest whose content points at it, grep AT THE EDGE.
    let text = "# Edge Report\n\n## Findings\nenterprise pricing is bespoke\n";
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let put = store_op(
        port,
        &serde_json::json!({ "op": "put-blob", "content_type": "text/markdown", "b64": b64 }),
    );
    let media: serde_json::Value = put["ok"].clone();
    assert!(media["sha256"].is_string(), "E5: blob stored: {put}");

    let mut entropy = |b: &mut [u8]| {
        b.fill(123);
        Ok(())
    };
    let manifest = waggle_core::mint(
        waggle_core::MintSpec::new(
            waggle_core::CanonicalUrl::new("ws://edge-content/report.md").unwrap(),
            waggle_core::Sharer::new("lead").unwrap(),
            waggle_core::Channel::subagent_general(),
        )
        .content(serde_json::from_value(media).unwrap()),
        &waggle_core::MintOptions::default(),
        &mut entropy,
        waggle_core::Timestamp::from_unix_ms(5),
    )
    .unwrap();
    let content_token = manifest.token.as_str().to_owned();
    store_op(
        port,
        &serde_json::json!({ "op": "ingest",
            "record": waggle_core::LogRecord::Minted { manifest: Box::new(manifest) } }),
    );
    let search_frame = format!(
        r#"{{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{{"name":"search","arguments":{{"token":"{content_token}","pattern":"bespoke"}}}}}}"#
    );
    let (_, text_resp) = post(port, "/mcp", Some(BEARER), &search_frame);
    let rpc: serde_json::Value = serde_json::from_str(&text_resp).unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(envelope["hint"].is_null(), "E5: {envelope}");
    assert_eq!(
        envelope["result"]["total_matches"], 1,
        "E5: the grep ran at the edge"
    );

    // ── E2: the unfurl cache reads through and revocation invalidates —
    //     a revoked token 410s, never a stale cached page.
    let unfurl = |t: &str| match ureq::get(&format!("http://127.0.0.1:{port}/t/{t}")).call() {
        Ok(r) => r.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(e) => panic!("unfurl: {e}"),
    };
    assert_eq!(unfurl(&content_token), 200, "E2: first render");
    assert_eq!(unfurl(&content_token), 200, "E2: cached render");
    let revoke = format!(
        r#"{{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{{"name":"mutate","arguments":{{"token":"{content_token}","change":"revoke","expected-version":1}}}}}}"#
    );
    post(port, "/mcp", Some(BEARER), &revoke);
    assert_eq!(
        unfurl(&content_token),
        410,
        "E2: revoked → gone, never stale cache"
    );

    // ── impressions: the unfurls above recorded the funnel's top stage.
    let funnel_frame = format!(
        r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"funnel","arguments":{{"token":"{content_token}"}}}}}}"#
    );
    let (_, ftext) = post(port, "/mcp", Some(BEARER), &funnel_frame);
    let rpc: serde_json::Value = serde_json::from_str(&ftext).unwrap();
    let envelope: serde_json::Value =
        serde_json::from_str(rpc["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(
        envelope["result"]["stages"]["impression"]
            .as_u64()
            .unwrap_or(0)
            >= 2,
        "impressions recorded: {envelope}"
    );

    // E3 (three-tier chain) lives in waggle-cli's federation tests,
    // gated on WAGGLE_EDGE_URL — the just/CI recipe boots wrangler once
    // and runs both suites against it.
}

/// E1-mf: THE conformance suite, driven over the wire — a Store adapter
/// speaking `/store`. The edge is a waggle backend at the HTTP layer too.
#[test]
fn e1_miniflare_conformance() {
    if !gated() {
        eprintln!("skipped: set WAGGLE_EDGE_TESTS=1 (needs node) — `just edge-test`");
        return;
    }
    // Each conformance check needs a FRESH store; the hive persists. Use
    // a namespacing trick: the suite runs against one hive but every
    // check mints under distinct nonces/tokens — run_all's checks are
    // already independent by construction (fresh factory per check is
    // about isolation of INDICES; the edge hive's global indices are
    // per-token keyed, so sharing is safe for this suite).
    let port = *PORT.get_or_init(|| {
        std::env::var("WAGGLE_EDGE_EXTERNAL_PORT")
            .expect("external wrangler")
            .parse()
            .unwrap()
    });
    // Per-check tenants: each fresh factory call gets its own Durable
    // Object — the isolation the suite's contract requires, via the same
    // header real multi-tenancy uses.
    let counter = std::sync::atomic::AtomicU32::new(0);
    waggle_store::conformance::run_all(&waggle_store::conformance::Harness::new(move || {
        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        HttpEdgeStore {
            port,
            tenant: format!("conf-{n}"),
        }
    }));
}

/// The wire adapter: the full Store contract over `/store`.
struct HttpEdgeStore {
    port: u16,
    tenant: String,
}

impl HttpEdgeStore {
    fn op(&self, body: &serde_json::Value) -> serde_json::Value {
        store_op_t(self.port, Some(&self.tenant), body)
    }
    fn world(&self) -> waggle_core::WorldState {
        let all = self.op(&serde_json::json!({ "op": "scan" }));
        let records: Vec<waggle_core::LogRecord> =
            serde_json::from_value(all["ok"].clone()).unwrap();
        waggle_core::reconstruct(records)
    }
}

fn parse_store_err(msg: &str) -> waggle_store::StoreError {
    use waggle_store::StoreError;
    let token_after = |prefix: &str| {
        msg.strip_prefix(prefix)
            .and_then(|r| r.split_whitespace().next())
            .map(|t| t.trim_end_matches(':'))
            .and_then(|t| waggle_core::Token::parse(t).ok())
    };
    if let Some(t) = token_after("unknown token ") {
        return StoreError::UnknownToken(t);
    }
    if msg.starts_with("parent ") && msg.contains("is revoked") {
        if let Some(t) = token_after("parent ") {
            return StoreError::ParentRevoked(t);
        }
    }
    if msg.starts_with("parent ") && msg.contains("unknown") {
        if let Some(t) = token_after("parent ") {
            return StoreError::ParentUnknown(t);
        }
    }
    if msg.starts_with("lifecycle change on ") {
        if let Some(t) = token_after("lifecycle change on ") {
            return StoreError::LifecycleRequiresVersion(t);
        }
    }
    if msg.starts_with("version conflict on ") {
        let token = token_after("version conflict on ").unwrap();
        let num = |marker: &str| {
            msg.split(marker)
                .nth(1)
                .and_then(|r| r.split([',', ' ']).next())
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0)
        };
        return StoreError::Conflict {
            token,
            expected: num("expected "),
            actual: num("current "),
        };
    }
    StoreError::Backend(msg.to_owned())
}

impl waggle_store::AppendStore for HttpEdgeStore {
    async fn append(
        &self,
        intent: waggle_store::AppendIntent,
    ) -> Result<waggle_store::Appended, waggle_store::StoreError> {
        let resp = self.op(&serde_json::json!({ "op": "append",
            "intent": serde_json::to_value(&intent).unwrap() }));
        if let Some(err) = resp.get("err").and_then(serde_json::Value::as_str) {
            return Err(parse_store_err(err));
        }
        let ok = &resp["ok"];
        match ok["kind"].as_str().unwrap_or_default() {
            "minted" => {
                let token = waggle_core::Token::parse(ok["token"].as_str().unwrap()).unwrap();
                let manifest = self.world().manifests[&token].clone();
                Ok(waggle_store::Appended::Minted {
                    view: waggle_store::ManifestView {
                        manifest: std::sync::Arc::new(manifest),
                    },
                    replayed: ok["replayed"].as_bool().unwrap_or(false),
                })
            }
            "mutated" => Ok(waggle_store::Appended::Mutated {
                seq: waggle_core::Seq(u32::try_from(ok["seq"].as_u64().unwrap()).unwrap()),
                version: u32::try_from(ok["version"].as_u64().unwrap()).unwrap(),
            }),
            "event" => Ok(waggle_store::Appended::Event {
                seq: waggle_core::Seq(u32::try_from(ok["seq"].as_u64().unwrap()).unwrap()),
            }),
            other => Err(waggle_store::StoreError::Codec(format!(
                "receipt kind {other}"
            ))),
        }
    }

    async fn ingest(
        &self,
        record: waggle_core::LogRecord,
    ) -> Result<bool, waggle_store::StoreError> {
        let resp = self.op(&serde_json::json!({ "op": "ingest",
            "record": serde_json::to_value(&record).unwrap() }));
        if let Some(err) = resp.get("err").and_then(serde_json::Value::as_str) {
            return Err(parse_store_err(err));
        }
        Ok(resp["ok"]["fresh"].as_bool().unwrap_or(false))
    }
}

impl waggle_store::ReadStore for HttpEdgeStore {
    async fn manifest(
        &self,
        token: waggle_core::Token,
    ) -> Result<Option<waggle_store::ManifestView>, waggle_store::StoreError> {
        Ok(self
            .world()
            .manifests
            .get(&token)
            .cloned()
            .map(|m| waggle_store::ManifestView {
                manifest: std::sync::Arc::new(m),
            }))
    }
    async fn children(
        &self,
        token: waggle_core::Token,
    ) -> Result<Vec<waggle_core::Token>, waggle_store::StoreError> {
        Ok(self
            .world()
            .lineage
            .get(&token)
            .cloned()
            .unwrap_or_default())
    }
    async fn tokens_for_target(
        &self,
        target: &waggle_core::CanonicalUrl,
    ) -> Result<Vec<waggle_core::Token>, waggle_store::StoreError> {
        Ok(self
            .world()
            .manifests
            .values()
            .filter(|m| m.target.as_str() == target.as_str())
            .map(|m| m.token)
            .collect())
    }
    async fn scan_token(
        &self,
        token: waggle_core::Token,
        from_seq: waggle_core::Seq,
    ) -> Result<Vec<waggle_core::LogRecord>, waggle_store::StoreError> {
        let resp = self.op(&serde_json::json!({ "op": "scan-token",
            "token": token.as_str(), "from_seq": from_seq.0 }));
        serde_json::from_value(resp["ok"].clone())
            .map_err(|e| waggle_store::StoreError::Codec(e.to_string()))
    }
    async fn scan_all(&self) -> Result<Vec<waggle_core::LogRecord>, waggle_store::StoreError> {
        let resp = self.op(&serde_json::json!({ "op": "scan" }));
        serde_json::from_value(resp["ok"].clone())
            .map_err(|e| waggle_store::StoreError::Codec(e.to_string()))
    }
    async fn funnel(
        &self,
        token: waggle_core::Token,
    ) -> Result<std::collections::BTreeMap<waggle_core::Stage, u64>, waggle_store::StoreError> {
        Ok(self
            .world()
            .funnels
            .get(&token)
            .cloned()
            .unwrap_or_default())
    }
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
