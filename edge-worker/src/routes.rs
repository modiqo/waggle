//! Worker-level routing: auth at the door, then delegate to the
//! tenant's Hive DO — the worker adds no semantics (the shim principle,
//! 16 §3, holding at the edge too).

use worker::*;

/// Bearer gate: `Authorization: Bearer <TENANT_TOKEN>`. The secret is a
/// wrangler secret (or `.dev.vars` under Miniflare). No secret set →
/// nothing authorizes (fail closed).
pub fn authorized(req: &Request, env: &Env) -> bool {
    let Ok(secret) = env.secret("TENANT_TOKEN") else {
        return false;
    };
    let expected = secret.to_string();
    if expected.len() < 16 {
        return false; // fail closed on weak configuration
    }
    let presented = req
        .headers()
        .get("authorization")
        .ok()
        .flatten()
        .and_then(|h| h.strip_prefix("Bearer ").map(str::to_owned))
        .unwrap_or_default();
    // Constant-time-ish comparison, same discipline as the daemon gate.
    presented.len() == expected.len()
        && presented
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// The tenant's hive. v1: one tenant per deployment ("hive"); the
/// per-tenant id becomes a header/key lookup when multi-tenancy lands.
fn hive(env: &Env) -> Result<Stub> {
    let ns = env.durable_object("HIVE")?;
    ns.id_from_name("hive")?.get_stub()
}

async fn delegate(env: &Env, path: &str, body: String) -> Result<Response> {
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_body(Some(body.into()));
    let req = Request::new_with_init(&format!("https://hive{path}"), &init)?;
    hive(env)?.fetch_with_request(req).await
}

/// `/mcp`: one JSON-RPC frame per request body, answered by the DO.
pub async fn mcp(mut req: Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    delegate(env, "/mcp", body).await
}

/// `/store`: the certification/replay RPC, answered by the DO.
pub async fn store_rpc(mut req: Request, env: &Env) -> Result<Response> {
    let body = req.text().await?;
    delegate(env, "/store", body).await
}

/// `/t/:token` — the public face: OG meta from the mint snapshot (I-3),
/// a human-readable link onward, and an impression recorded. Rendering
/// comes from `waggle-social`, byte-identical to every other tier.
pub async fn unfurl(env: &Env, raw: &str) -> Result<Response> {
    let Ok(token) = waggle_core::Token::parse(raw) else {
        return Response::error("not a waggle token", 404);
    };
    // Read the manifest through the DO's store RPC (one hop, cache later).
    let body = serde_json::json!({ "op": "scan-token", "token": token.as_str(), "from_seq": 0 })
        .to_string();
    let mut resp = delegate(env, "/store", body).await?;
    let text = resp.text().await?;
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let records: Vec<waggle_core::LogRecord> =
        serde_json::from_value(parsed["ok"].clone()).unwrap_or_default();
    if records.is_empty() {
        return Response::error("unknown token", 404);
    }
    let world = waggle_core::reconstruct(records);
    let Some(manifest) = world.manifests.get(&token) else {
        return Response::error("unknown token", 404);
    };

    // I-3: the snapshot, never a scrape. The same renderer as every tier.
    let package = waggle_social::SharePackage::from_manifest(manifest, "https://example-edge");
    let meta = waggle_social::og_meta(&package);
    let target = manifest.target.as_str();
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">{meta}<title>{}</title></head>\
         <body><p>waggle token <code>{token}</code> → <a href=\"{target}\">{target}</a></p></body></html>",
        package.title,
    );

    // The impression event lands with the KV-cache slice (deviation
    // noted in the matrix) — unfurls stay read-only in v1.
    Response::from_html(html)
}
