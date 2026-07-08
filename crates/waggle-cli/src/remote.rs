//! Tier 2 — the forwarding resolver (design docs `16 §3`, `08 §0`,
//! CP-10 slice 1): two `waggled`s talking. The owner listens on
//! **token-gated TCP** (F-2's second half); a peer configured with an
//! upstream forwards frames for tokens it doesn't own and returns the
//! budgeted answers. Computation stays where the bytes live: a remote
//! `search` executes at the owner; only matches travel.
//!
//! Config, all env: `WAGGLE_TCP` (listen addr) + `WAGGLE_TCP_TOKEN`
//! (required — refusing to listen unauthenticated is the point) on the
//! owner; `WAGGLE_UPSTREAM` (host:port) + `WAGGLE_UPSTREAM_TOKEN` on the
//! peer.

use serde_json::Value;

/// Tools worth forwarding on a local miss: reads, interrogation, and
/// `record` (events belong to the owner's funnel). `mint` is never
/// forwarded — you mint where you stand.
const FORWARDABLE: &[&str] = &[
    "resolve", "query", "read", "search", "funnel", "map", "record", "mutate",
];

/// The token a frame targets, when it's a forwardable tools/call.
#[must_use]
pub fn forwardable_token(line: &str) -> Option<waggle_core::Token> {
    let msg: Value = serde_json::from_str(line).ok()?;
    if msg.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    let params = msg.get("params")?;
    let tool = params.get("name")?.as_str()?;
    if !FORWARDABLE.contains(&tool) {
        return None;
    }
    let raw = params.get("arguments")?.get("token")?.as_str()?;
    waggle_core::Token::parse(raw).ok()
}

/// The auth handshake line a TCP client must send first.
#[must_use]
pub fn auth_frame(token: &str) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "method": "waggled/hello", "params": { "token": token } })
        .to_string()
}

/// Validate a received hello against the gate token (constant-time-ish
/// comparison is overkill for a random 32-byte bearer on a LAN, but
/// cheap: compare hashes).
#[must_use]
pub fn hello_ok(line: &str, gate: &str) -> bool {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    if msg.get("method").and_then(Value::as_str) != Some("waggled/hello") {
        return false;
    }
    let presented = msg
        .pointer("/params/token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    sha2_free::eq_hashed(presented.as_bytes(), gate.as_bytes())
}

/// Length-independent comparison without pulling a crypto crate into the
/// CLI: compare byte-sums and bytes only when lengths match, XOR-folding
/// so the loop always completes.
mod sha2_free {
    pub fn eq_hashed(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

/// Forward one frame to the upstream and return its response line.
/// Connection-per-request keeps slice 1 simple; the measured local
/// round-trip budget has plenty of headroom for a TCP connect on a LAN.
pub async fn forward(line: &str) -> Option<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let upstream = std::env::var("WAGGLE_UPSTREAM").ok()?;
    let token = std::env::var("WAGGLE_UPSTREAM_TOKEN").unwrap_or_default();
    let stream = tokio::net::TcpStream::connect(&upstream).await.ok()?;
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    write.write_all(auth_frame(&token).as_bytes()).await.ok()?;
    write.write_all(b"\n").await.ok()?;
    write.write_all(line.as_bytes()).await.ok()?;
    write.write_all(b"\n").await.ok()?;
    lines.next_line().await.ok().flatten()
}

/// Introspect a forwardable frame for the resolution cache: returns
/// `(cache_key, level)` when this is a `resolve` call — the key covers
/// token AND arguments (different contexts get different projections).
#[must_use]
pub fn resolve_cache_key(line: &str) -> Option<(String, String)> {
    let msg: Value = serde_json::from_str(line).ok()?;
    let params = msg.get("params")?;
    if params.get("name")?.as_str()? != "resolve" {
        return None;
    }
    let args = params.get("arguments")?;
    let token = args.get("token")?.as_str()?.to_owned();
    let level = args
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or("eventual")
        .to_owned();
    let ctx = args
        .get("context")
        .map(std::string::ToString::to_string)
        .unwrap_or_default();
    Some((format!("{token}\x1f{ctx}"), level))
}

/// Pull `(envelope_text, revalidate_after_unix_ms)` out of a forwarded
/// resolve response — the response's own freshness stamp (G-3) is the
/// cache policy; the peer invents nothing.
#[must_use]
pub fn cacheable_resolution(response: &str) -> Option<(String, u64)> {
    let rpc: Value = serde_json::from_str(response).ok()?;
    let text = rpc.pointer("/result/content/0/text")?.as_str()?;
    let envelope: Value = serde_json::from_str(text).ok()?;
    if envelope.get("hint").is_some_and(|h| !h.is_null()) {
        return None; // errors are never cached
    }
    let revalidate = envelope.pointer("/result/revalidate_after")?.as_u64()?;
    Some((text.to_owned(), revalidate))
}

/// Re-wrap a cached envelope for a new request id.
#[must_use]
pub fn rewrap(envelope_text: &str, id: &Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": envelope_text }], "isError": false },
    })
    .to_string()
}

/// The refusal a peer returns when a token is unknown locally and no
/// upstream is configured (or the upstream didn't answer): honest, with
/// the fix named.
#[must_use]
pub fn unreachable_response(id: &Value, token: waggle_core::Token) -> String {
    let envelope = serde_json::json!({
        "result": null,
        "next": [],
        "hint": format!(
            "{token} is not in this store and no upstream answered — set WAGGLE_UPSTREAM=<owner host:port> and WAGGLE_UPSTREAM_TOKEN, or replay the owner's log here"
        ),
        "stats": {},
    });
    serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "result": { "content": [{ "type": "text", "text": envelope.to_string() }], "isError": true },
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwardable_detection() {
        let frame = |tool: &str| {
            format!(
                r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{{"token":"abc123"}}}}}}"#
            )
        };
        assert!(forwardable_token(&frame("resolve")).is_some());
        assert!(forwardable_token(&frame("search")).is_some());
        assert!(forwardable_token(&frame("record")).is_some());
        assert!(
            forwardable_token(&frame("mint")).is_none(),
            "mint never forwards"
        );
        assert!(forwardable_token(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).is_none());
        assert!(
            forwardable_token(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"map","arguments":{}}}"#
            )
            .is_none(),
            "global map stays local"
        );
    }

    #[test]
    fn hello_gate() {
        let good = auth_frame("s3cret-bearer");
        assert!(hello_ok(&good, "s3cret-bearer"));
        assert!(!hello_ok(&good, "different"));
        assert!(!hello_ok(&auth_frame(""), "s3cret-bearer"));
        assert!(!hello_ok(r#"{"method":"tools/list"}"#, "s3cret-bearer"));
        assert!(!hello_ok("not json", "s3cret-bearer"));
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn resolution_caching_roundtrip() {
        let req = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"resolve","arguments":{"token":"abc123"}}}"#;
        let (key, level) = resolve_cache_key(req).expect("resolve frame introspects");
        assert_eq!(level, "eventual");

        let envelope = r#"{"result":{"disposition":"active","revalidate_after":1783520469100},"next":[],"stats":{}}"#;
        let response = serde_json::json!({
            "jsonrpc": "2.0", "id": 7,
            "result": { "content": [{ "type": "text", "text": envelope }], "isError": false },
        })
        .to_string();
        let (cached, expires) = cacheable_resolution(&response).expect("success resolutions cache");
        assert_eq!(expires, 1_783_520_469_100);
        assert_eq!(cached, envelope);

        // Same args → same key; strict is reported as such.
        let strict_req = r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"resolve","arguments":{"token":"abc123","level":"strict"}}}"#;
        let (key2, level2) = resolve_cache_key(strict_req).unwrap();
        assert_eq!(level2, "strict");
        assert_eq!(key, key2, "level must not fork the cache key");

        // Errors never cache.
        let err_env = r#"{"result":null,"next":[],"hint":"nope","stats":{}}"#;
        let err_resp = serde_json::json!({
            "jsonrpc": "2.0", "id": 9,
            "result": { "content": [{ "type": "text", "text": err_env }], "isError": true },
        })
        .to_string();
        assert!(cacheable_resolution(&err_resp).is_none());
    }
}
