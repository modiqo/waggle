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

/// The tenant's hive: one Durable Object per tenant (08 §8). The tenant
/// comes from the `x-waggle-tenant` header (sanitized slug, default
/// "hive") — authenticated callers may partition their state; unfurls
/// always read the default tenant.
fn hive(env: &Env, tenant: &str) -> Result<Stub> {
    let ns = env.durable_object("HIVE")?;
    ns.id_from_name(tenant)?.get_stub()
}

/// Sanitize a tenant header to a safe DO name.
fn tenant_of(req: &Request) -> String {
    req.headers()
        .get("x-waggle-tenant")
        .ok()
        .flatten()
        .filter(|t| {
            !t.is_empty()
                && t.len() <= 32
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        .unwrap_or_else(|| "hive".to_owned())
}

async fn delegate(env: &Env, tenant: &str, path: &str, body: String) -> Result<Response> {
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_body(Some(body.into()));
    let req = Request::new_with_init(&format!("https://hive{path}"), &init)?;
    hive(env, tenant)?.fetch_with_request(req).await
}

/// `/mcp`: one JSON-RPC frame per request body, answered by the DO. A
/// successful `mutate` invalidates the token's unfurl cache (E2's
/// write-path half).
pub async fn mcp(mut req: Request, env: &Env) -> Result<Response> {
    let tenant = tenant_of(&req);
    let body = req.text().await?;
    let invalidate = mutate_token(&body);
    let resp = delegate(env, &tenant, "/mcp", body).await;
    if let (Some(token), Ok(kv)) = (invalidate, env.kv("CACHE")) {
        let _ = kv.delete(&format!("unfurl:{token}")).await;
    }
    resp
}

/// The token a mutate frame targets (for cache invalidation).
fn mutate_token(body: &str) -> Option<String> {
    let msg: serde_json::Value = serde_json::from_str(body).ok()?;
    let params = msg.get("params")?;
    (params.get("name")?.as_str()? == "mutate").then(|| {
        params
            .pointer("/arguments/token")?
            .as_str()
            .map(str::to_owned)
    })?
}

/// `/store`: the certification/replay RPC, answered by the DO.
pub async fn store_rpc(mut req: Request, env: &Env) -> Result<Response> {
    let tenant = tenant_of(&req);
    let body = req.text().await?;
    // A lifecycle mutation can arrive by INGEST too (replication/push) —
    // it must invalidate the unfurl cache exactly like an /mcp mutate.
    // (Found live: a pushed revocation served a stale cached unfurl.)
    let invalidate = ingested_mutation_token(&body);
    let resp = delegate(env, &tenant, "/store", body).await;
    if let (Some(token), Ok(kv)) = (invalidate, env.kv("CACHE")) {
        let _ = kv.delete(&format!("unfurl:{token}")).await;
    }
    resp
}

/// The token of an ingested mutation record, for cache invalidation.
fn ingested_mutation_token(body: &str) -> Option<String> {
    let msg: serde_json::Value = serde_json::from_str(body).ok()?;
    if msg.get("op")?.as_str()? != "ingest" {
        return None;
    }
    let record = msg.get("record")?;
    (record.get("record")?.as_str()? == "mutation")
        .then(|| record.get("token")?.as_str().map(str::to_owned))?
}

/// `/t/:token` — the public face: OG meta from the mint snapshot (I-3),
/// a human-readable link onward, and an impression recorded. Rendering
/// comes from `waggle-social`, byte-identical to every other tier.
pub async fn unfurl(env: &Env, raw: &str) -> Result<Response> {
    let Ok(token) = waggle_core::Token::parse(raw) else {
        return Response::error("not a waggle token", 404);
    };
    // E2: the KV cache tier — read-through with a short TTL; mutations
    // invalidate; a missing binding just skips caching.
    let cache_key = format!("unfurl:{token}");
    if let Ok(kv) = env.kv("CACHE") {
        if let Ok(Some(hit)) = kv.get(&cache_key).text().await {
            record_impression(env, &token).await;
            return Response::from_html(hit);
        }
    }
    // Read the manifest through the DO's store RPC (one hop, cache later).
    let body = serde_json::json!({ "op": "scan-token", "token": token.as_str(), "from_seq": 0 })
        .to_string();
    let mut resp = delegate(env, "hive", "/store", body).await?;
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

    // Revoked tokens serve NOTHING — 410, and never from cache.
    if manifest.revoked_at.is_some() {
        if let Ok(kv) = env.kv("CACHE") {
            let _ = kv.delete(&cache_key).await;
        }
        return Response::error("this token was revoked by its owner", 410);
    }

    // I-3: the snapshot, never a scrape. The same renderer as every tier.
    let package = waggle_social::SharePackage::from_manifest(manifest, "https://example-edge");
    let meta = waggle_social::og_meta(&package);
    let target = manifest.target.as_str();
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">{meta}<title>{}</title></head>\
         <body><p>waggle token <code>{token}</code> → <a href=\"{target}\">{target}</a></p></body></html>",
        package.title,
    );

    if let Ok(kv) = env.kv("CACHE") {
        if let Ok(put) = kv.put(&cache_key, html.clone()) {
            let _ = put.expiration_ttl(60).execute().await;
        }
    }
    record_impression(env, &token).await;
    Response::from_html(html)
}

/// Every unfurl records an impression — payload-free (I-1), the funnel's
/// top stage, exactly what the conference-slide scenario counts (05 §4).
async fn record_impression(env: &Env, token: &waggle_core::Token) {
    let frame = serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "tools/call",
        "params": { "name": "record",
                    "arguments": { "token": token.as_str(), "stage": "impression" } },
    })
    .to_string();
    let _ = delegate(env, "hive", "/mcp", frame).await;
}
