//! The MCP resource projection (design doc `21`): the passive,
//! application-controlled faces of a token — enumeration, plain reads,
//! and update push — projected from the SAME dispatcher the tools use.
//! No new operations exist here (09 §2): `resources/read` IS a resolve
//! (recorded, funnel-honest), and subscriptions are ephemeral
//! per-connection state pushing `notifications/resources/updated` when
//! a lifecycle mutation lands on a subscribed token.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use waggle_core::{replay, Disposition, ManifestFold, Timestamp, Token};
use waggle_store::{BlobSink, Store};

use crate::handlers::Handler;

/// The most resources one `resources/list` page returns — capped and
/// said so, like every other truncation in the system.
const LIST_CAP: usize = 100;

/// Per-connection subscription state (doc `21 §3`): owned by the
/// transport's connection loop, dropped on disconnect — nothing about a
/// subscriber ever persists (the I-7 posture for connection state).
#[derive(Debug, Default)]
pub struct Session {
    subs: BTreeSet<Token>,
}

impl Session {
    /// Subscribe this connection to a token's lifecycle.
    pub fn subscribe(&mut self, token: Token) {
        self.subs.insert(token);
    }

    /// Drop a subscription. Returns whether it existed.
    pub fn unsubscribe(&mut self, token: Token) -> bool {
        self.subs.remove(&token)
    }

    /// Is this connection subscribed to `token`?
    #[must_use]
    pub fn contains(&self, token: Token) -> bool {
        self.subs.contains(&token)
    }

    /// How many tokens this connection is subscribed to — transports
    /// aggregate this into their health surface (`waggled/status`).
    #[must_use]
    pub fn subscription_count(&self) -> u64 {
        self.subs.len() as u64
    }
}

/// `waggle://TOKEN` → the token. The URI scheme is the token, nothing
/// more (doc 21 §3).
pub(crate) fn parse_uri(uri: &str) -> Option<Token> {
    Token::parse(uri.strip_prefix("waggle://")?).ok()
}

fn uri_of(token: Token) -> String {
    format!("waggle://{}", token.as_str())
}

/// The `notifications/resources/updated` frame for a token.
#[must_use]
pub fn updated_notification(token: Token) -> String {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": { "uri": uri_of(token) },
    })
    .to_string()
}

/// If this `tools/call` request is a LIFECYCLE mutation (revoke,
/// supersede, expiry — never cosmetic churn), the token it targets.
/// Pure request inspection; the transport broadcasts only after the
/// dispatcher acknowledged success.
pub(crate) fn lifecycle_mutation(params: &Value) -> Option<Token> {
    if params.get("name").and_then(Value::as_str) != Some("mutate") {
        return None;
    }
    let args = params.get("arguments")?;
    let change = args.get("change").and_then(Value::as_str)?;
    let lifecycle =
        change == "revoke" || change.starts_with("supersede=") || change.starts_with("expire=");
    if !lifecycle {
        return None;
    }
    Token::parse(args.get("token").and_then(Value::as_str)?).ok()
}

/// `resources/templates/list` — one template; discovery of the scheme.
pub(crate) fn templates() -> Value {
    json!({
        "resourceTemplates": [{
            "uriTemplate": "waggle://{token}",
            "name": "waggle token",
            "description": "An attributed artifact reference; reading it resolves the token's projection and records the resolve (the funnel stays honest).",
            "mimeType": "application/json",
        }]
    })
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// `resources/list`: active, PUBLIC tokens, newest first, capped.
    /// Private tokens are capability URLs and never enumerate (spec §6);
    /// revoked/expired tokens serve nothing, so they do not list.
    pub(crate) async fn resources_list(&self, now: Timestamp) -> Value {
        let records = self.store().scan_all().await.unwrap_or_default();
        let fold = replay(records, ManifestFold::default());
        let mut live: Vec<_> = fold
            .manifests
            .values()
            .filter(|m| !m.private && matches!(m.disposition(now), Disposition::Active))
            .collect();
        live.sort_by_key(|m| std::cmp::Reverse(m.minted_at));
        let total = live.len();
        let resources: Vec<Value> = live
            .into_iter()
            .take(LIST_CAP)
            .map(|m| {
                let basename = m.target.as_str().rsplit('/').next().unwrap_or("artifact");
                json!({
                    "uri": uri_of(m.token),
                    "name": basename,
                    "description": format!("{} · minted by {}", m.channel.as_str(), m.sharer.as_str()),
                    "mimeType": m.content.as_ref().map_or("application/json", |c| c.content_type.as_str()),
                })
            })
            .collect();
        let mut out = json!({ "resources": resources });
        if total > LIST_CAP {
            out["_truncated"] = json!(format!(
                "{} of {total} — resolve by token via find",
                LIST_CAP
            ));
        }
        out
    }

    /// `resources/read`: the projection, through the SAME dispatcher the
    /// resolve tool uses — recorded, cascade-checked, funnel-honest.
    /// Inline bodies serve as their own content type; everything else
    /// serves the resolution as JSON.
    pub(crate) async fn resources_read<E>(
        &self,
        uri: &str,
        now: Timestamp,
        entropy: &mut E,
    ) -> Result<Value, String>
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let token = parse_uri(uri).ok_or_else(|| {
            format!("`{uri}` is not a waggle resource — the scheme is waggle://<token>")
        })?;
        let args = json!({ "token": token.as_str() });
        let envelope = self.dispatch("resolve", &args, now, entropy).await;
        if let Some(hint) = envelope.hint {
            return Err(hint);
        }
        // The 410 posture (essay §controls): a revoked resource has no
        // content to read — the tool surface serves the disposition as
        // knowledge; the resource surface answers gone.
        if envelope.result["disposition"].get("revoked").is_some() {
            return Err(format!("{token} is revoked — the resource is gone (410)"));
        }
        let body = &envelope.result["body"];
        let content = match (
            body.pointer("/inline/content_type").and_then(Value::as_str),
            body.pointer("/inline/data").and_then(Value::as_str),
        ) {
            (Some(mime), Some(data)) => json!({
                "uri": uri,
                "mimeType": mime,
                "text": data,
            }),
            _ => json!({
                "uri": uri,
                "mimeType": "application/json",
                "text": envelope.result.to_string(),
            }),
        };
        let mut map = Map::new();
        map.insert("contents".into(), json!([content]));
        Ok(Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_scheme_is_the_token_and_nothing_more() {
        let t = Token::parse("b2uQyZUC").unwrap();
        assert_eq!(parse_uri(&uri_of(t)), Some(t));
        assert_eq!(parse_uri("file:///tmp/x"), None);
        assert_eq!(parse_uri("waggle://not a token!"), None);
    }

    #[test]
    fn lifecycle_detection_is_exact() {
        let mk = |change: &str| json!({ "name": "mutate", "arguments": { "token": "b2uQyZUC", "change": change } });
        assert!(lifecycle_mutation(&mk("revoke")).is_some());
        assert!(lifecycle_mutation(&mk("supersede=9rTq3wXk")).is_some());
        assert!(lifecycle_mutation(&mk("expire=1700000000000")).is_some());
        assert!(
            lifecycle_mutation(&mk("label team=research")).is_none(),
            "cosmetic churn never notifies"
        );
        assert!(lifecycle_mutation(
            &json!({ "name": "record", "arguments": { "token": "b2uQyZUC" } })
        )
        .is_none());
    }

    #[test]
    fn sessions_are_plain_ephemeral_sets() {
        let (a, b) = (
            Token::parse("aaaaaaaa").unwrap(),
            Token::parse("bbbbbbbb").unwrap(),
        );
        let mut s = Session::default();
        s.subscribe(a);
        assert!(s.contains(a) && !s.contains(b));
        assert!(s.unsubscribe(a));
        assert!(!s.unsubscribe(a), "double-unsubscribe is a no-op");
    }
}
