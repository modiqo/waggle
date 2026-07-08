//! The waggle edge worker (design doc `08`): the same engine the native
//! tier certified, running inside a **Durable Object per tenant** — the
//! single-writer commit point — behind two authenticated routes:
//!
//! - `POST /mcp` — MCP JSON-RPC frames (the same wire as `waggled`);
//! - `POST /store` — the store RPC (`ingest` for replay-migration,
//!   `scan` for export) used by certification and `waggle export | replay`;
//! - `GET /t/:token` — the human/unfurl route: OG meta from the mint
//!   snapshot (I-3), then the target;
//! - `GET /health` — liveness.
//!
//! Auth: a bearer secret (`TENANT_TOKEN`) gates every route except
//! `/health` and `/t/` (public unfurls are the point of short links).
//! Everything here is glue — semantics live in the certified crates.

#![cfg(target_arch = "wasm32")]

use worker::*;

#[allow(missing_docs)] // the durable_object macro generates undocumented glue
mod hive;
mod routes;

pub use hive::Hive;

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path().to_owned();

    if path == "/health" {
        return Response::ok("waggle-edge");
    }
    if let Some(token) = path.strip_prefix("/t/") {
        return routes::unfurl(&env, token).await;
    }

    // Everything else is bearer-gated.
    if !routes::authorized(&req, &env) {
        return Response::error("unauthorized", 401);
    }
    match (req.method(), path.as_str()) {
        (Method::Post, "/mcp") => routes::mcp(req, &env).await,
        (Method::Post, "/store") => routes::store_rpc(req, &env).await,
        _ => Response::error("not found — routes: /mcp /store /t/:token /health", 404),
    }
}
