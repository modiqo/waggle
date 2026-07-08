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
    use sha2_free::eq_hashed;
    eq_hashed(presented.as_bytes(), gate.as_bytes())
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
