//! `waggle edge` — the deployed edge from the command line (doc 08).
//!
//! Pure HTTPS client work: `status` reads health and the tool surface;
//! `push` replicates this store's records (and the snapshot blobs they
//! reference) so its tokens resolve — and grep — at the edge; `smoke`
//! proves the loop end to end. Deploying the worker itself is wrangler's
//! job (guide 09); this command never needs the repo.
//!
//! HTTPS via `ureq` (rustls) — the CLI's only HTTP client, deliberately
//! not shared with the daemon's federation path (which stays a
//! transport-thin line protocol).

use base64::Engine as _;
use serde_json::{json, Value};
use waggle_store::ReadStore as _;

struct Edge {
    url: String,
    bearer: String,
}

impl Edge {
    fn from(url: Option<&str>, bearer: Option<&str>) -> Result<Self, String> {
        let url = url
            .map(str::to_owned)
            .or_else(|| std::env::var("WAGGLE_EDGE_URL").ok())
            .ok_or("no edge configured — pass --url or set WAGGLE_EDGE_URL")?;
        let bearer = bearer
            .map(str::to_owned)
            .or_else(|| std::env::var("WAGGLE_EDGE_BEARER").ok())
            .ok_or("no bearer — pass --bearer or set WAGGLE_EDGE_BEARER")?;
        Ok(Self {
            url: url.trim_end_matches('/').to_owned(),
            bearer,
        })
    }

    fn post(&self, path: &str, body: &str) -> Result<String, String> {
        let resp = ureq::post(&format!("{}{path}", self.url))
            .set("authorization", &format!("Bearer {}", self.bearer))
            .set("content-type", "application/json")
            .send_string(body);
        match resp {
            Ok(r) => r.into_string().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(code, r)) => Err(format!(
                "{path} → {code}: {}",
                r.into_string().unwrap_or_default()
            )),
            Err(e) => Err(e.to_string()),
        }
    }

    fn store_op(&self, body: &Value) -> Result<Value, String> {
        let text = self.post("/store", &body.to_string())?;
        let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if let Some(err) = v.get("err").and_then(Value::as_str) {
            return Err(err.to_owned());
        }
        Ok(v["ok"].clone())
    }

    fn tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        let frame = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args },
        });
        let text = self.post("/mcp", &frame.to_string())?;
        let rpc: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let envelope = rpc
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .ok_or("malformed rpc response")?;
        serde_json::from_str(envelope).map_err(|e| e.to_string())
    }
}

fn fail(msg: &str) -> i32 {
    eprintln!("waggle edge: {msg}");
    1
}

/// `waggle edge <status|push|smoke>`.
pub fn run(action: &str, url: Option<&str>, bearer: Option<&str>) -> i32 {
    let edge = match Edge::from(url, bearer) {
        Ok(e) => e,
        Err(msg) => return fail(&msg),
    };
    match action {
        "status" => status(&edge),
        "push" => push(&edge),
        "smoke" => smoke(&edge),
        other => fail(&format!("`{other}` — actions are status | push | smoke")),
    }
}

fn status(edge: &Edge) -> i32 {
    let health = ureq::get(&format!("{}/health", edge.url))
        .call()
        .map_or(0, |r| r.status());
    let tools = edge
        .post("/mcp", r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v.pointer("/result/tools")
                .and_then(Value::as_array)
                .map(Vec::len)
        });
    match (health, tools) {
        (200, Some(n)) => {
            println!("{}", json!({ "url": edge.url, "health": "ok", "tools": n }));
            0
        }
        (200, None) => fail("healthy but /mcp refused — check the bearer"),
        _ => fail("unreachable — is the worker deployed? (npx wrangler deploy, guide 09)"),
    }
}

/// Replicate this machine's store to the edge: every record via ingest
/// (idempotent — rerun anytime), plus the snapshot blobs manifests
/// reference, so read/search work where the files never existed.
fn push(edge: &Edge) -> i32 {
    let handler = match crate::run::open_handler() {
        Ok(h) => h,
        Err(e) => return fail(&e),
    };
    pollster::block_on(async {
        let records = match handler.store().scan_all().await {
            Ok(r) => r,
            Err(e) => return fail(&e.to_string()),
        };
        let mut pushed = 0u64;
        let mut blobs = 0u64;
        for record in &records {
            // Blobs first: a manifest that lands before its content
            // would serve read/search hints instead of answers.
            if let waggle_core::LogRecord::Minted { manifest } = record {
                if let Some(media) = &manifest.content {
                    match handler.blobs().get(media) {
                        Ok(bytes) => {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            if let Err(e) = edge.store_op(&json!({
                                "op": "put-blob",
                                "content_type": media.content_type,
                                "b64": b64,
                            })) {
                                return fail(&format!("blob {}: {e}", media.sha256.as_str()));
                            }
                            blobs += 1;
                        }
                        Err(e) => eprintln!(
                            "waggle edge push: skipping blob {}: {e}",
                            media.sha256.as_str()
                        ),
                    }
                }
            }
            match edge.store_op(&json!({ "op": "ingest", "record": record })) {
                Ok(ok) => {
                    if ok["fresh"].as_bool().unwrap_or(false) {
                        pushed += 1;
                    }
                }
                Err(e) => return fail(&format!("ingest: {e}")),
            }
        }
        println!(
            "{}",
            json!({
                "records_scanned": records.len(),
                "records_new": pushed,
                "blobs_pushed": blobs,
                "hint": "rerun anytime — ingest is idempotent (C-4)",
            })
        );
        0
    })
}

fn smoke(edge: &Edge) -> i32 {
    let minted = match edge.tool("mint", &json!({ "target": "ws://edge-smoke/probe.md" })) {
        Ok(e) => e,
        Err(e) => return fail(&format!("mint: {e}")),
    };
    let Some(token) = minted.pointer("/result/token").and_then(Value::as_str) else {
        return fail(&format!("mint refused: {minted}"));
    };
    let token = token.to_owned();
    let resolved = match edge.tool("resolve", &json!({ "token": token })) {
        Ok(e) => e,
        Err(e) => return fail(&format!("resolve: {e}")),
    };
    if resolved.pointer("/result/disposition") != Some(&json!("active")) {
        return fail(&format!("resolve not active: {resolved}"));
    }
    let funnel = match edge.tool("funnel", &json!({ "token": token })) {
        Ok(e) => e,
        Err(e) => return fail(&format!("funnel: {e}")),
    };
    println!(
        "{}",
        json!({
            "smoke": "ok",
            "token": token,
            "funnel": funnel.pointer("/result/stages"),
        })
    );
    0
}
