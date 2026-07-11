//! The MCP wire layer: JSON-RPC 2.0, newline-delimited (the MCP stdio
//! transport). Deliberately minimal — `initialize`, `tools/list`,
//! `tools/call`, `ping` — and transport-agnostic: [`handle_message`] maps
//! one request string to one response string; stdio loops and HTTP
//! handlers both call it. The shim can add no semantics because there are
//! none here to add (16 §3).

use serde_json::{json, Value};
use waggle_core::{Timestamp, Token};
use waggle_ops::Surface;
use waggle_store::Store;

use crate::handlers::Handler;
use crate::resources::{self, Session};

/// MCP protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Generate the MCP tool list from the operations catalog — the single
/// source (09 §2): every `Surface::Both` operation, schema from its args.
#[must_use]
pub fn tool_list() -> Value {
    let tools: Vec<Value> = waggle_ops::OPERATIONS
        .iter()
        .filter(|op| matches!(op.surface, Surface::Both))
        .map(|op| {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for arg in op.args {
                properties.insert(
                    arg.name.to_owned(),
                    json!({ "type": "string", "description": arg.doc }),
                );
                if arg.required {
                    required.push(arg.name);
                }
            }
            json!({
                "name": op.name,
                "description": op.description,
                "inputSchema": { "type": "object", "properties": properties, "required": required },
            })
        })
        .collect();
    json!({ "tools": tools })
}

#[allow(clippy::needless_pass_by_value)] // json! literals read better by value
fn rpc_ok(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn rpc_err(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// What one message produced for a stateful connection (doc `21 §3`):
/// the reply, any notification frames due to THIS connection's
/// subscriptions, and the lifecycle-mutated token for the transport's
/// hub to fan out to OTHER connections.
#[derive(Debug, Default)]
pub struct SessionOutput {
    /// The JSON-RPC reply, if the message wanted one.
    pub reply: Option<String>,
    /// Notification frames to write after the reply (own subscriptions).
    pub notifications: Vec<String>,
    /// A lifecycle mutation this message committed — hub fan-out.
    pub lifecycle: Option<Token>,
}

/// Handle one JSON-RPC message, statelessly. Returns `None` for
/// notifications (no response on the wire). Subscriptions refuse here —
/// they need a connection ([`handle_session`]).
pub async fn handle_message<S: Store, B: waggle_store::BlobSink, E>(
    handler: &Handler<S, B>,
    raw: &str,
    now: Timestamp,
    entropy: &mut E,
) -> Option<String>
where
    E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
{
    handle_inner(handler, None, raw, now, entropy).await.reply
}

/// Handle one JSON-RPC message for a stateful connection: identical
/// semantics plus working `resources/subscribe`/`unsubscribe` and the
/// notification bookkeeping of [`SessionOutput`].
pub async fn handle_session<S: Store, B: waggle_store::BlobSink, E>(
    handler: &Handler<S, B>,
    session: &mut Session,
    raw: &str,
    now: Timestamp,
    entropy: &mut E,
) -> SessionOutput
where
    E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
{
    handle_inner(handler, Some(session), raw, now, entropy).await
}

async fn handle_inner<S: Store, B: waggle_store::BlobSink, E>(
    handler: &Handler<S, B>,
    mut session: Option<&mut Session>,
    raw: &str,
    now: Timestamp,
    entropy: &mut E,
) -> SessionOutput
where
    E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
{
    let mut out = SessionOutput::default();
    let msg: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            out.reply = Some(rpc_err(&Value::Null, -32700, &format!("parse error: {e}")));
            return out;
        }
    };
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // Notifications (no id) get no response.
    let Some(id) = msg.get("id").cloned() else {
        return out;
    };
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let uri = || params.get("uri").and_then(Value::as_str).unwrap_or("");

    let response = match method {
        "initialize" => rpc_ok(
            &id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    // The resource projection (doc 21): the passive faces
                    // of a token; the verbs stay tools by design.
                    "resources": { "subscribe": true, "listChanged": false },
                },
                "serverInfo": { "name": "waggled", "version": env!("CARGO_PKG_VERSION") },
            }),
        ),
        "ping" => rpc_ok(&id, json!({})),
        "tools/list" => rpc_ok(&id, tool_list()),
        "tools/call" => {
            let tool = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let envelope = handler.dispatch(tool, &args, now, entropy).await;
            let is_error = envelope.hint.is_some();
            if !is_error {
                // A committed lifecycle mutation notifies subscribers:
                // this connection's directly, every other's via the hub.
                if let Some(token) = resources::lifecycle_mutation(&params) {
                    if session.as_ref().is_some_and(|s| s.contains(token)) {
                        out.notifications.push(resources::updated_notification(token));
                    }
                    out.lifecycle = Some(token);
                }
            }
            let text = serde_json::to_string(&envelope)
                .unwrap_or_else(|e| format!("{{\"hint\":\"encode failure: {e}\"}}"));
            rpc_ok(
                &id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": is_error,
                }),
            )
        }
        "resources/templates/list" => rpc_ok(&id, resources::templates()),
        "resources/list" => rpc_ok(&id, handler.resources_list(now).await),
        "resources/read" => match handler.resources_read(uri(), now, entropy).await {
            Ok(contents) => rpc_ok(&id, contents),
            Err(msg) => rpc_err(&id, -32002, &msg),
        },
        "resources/subscribe" | "resources/unsubscribe" => match session.as_mut() {
            None => rpc_err(
                &id,
                -32002,
                "subscriptions need a stateful connection — subscribe at the owner's daemon, not over one-shot HTTP",
            ),
            Some(sess) => match resources::parse_uri(uri()) {
                None => rpc_err(&id, -32002, "the scheme is waggle://<token>"),
                Some(token) => {
                    if method == "resources/subscribe" {
                        sess.subscribe(token);
                    } else {
                        sess.unsubscribe(token);
                    }
                    rpc_ok(&id, json!({}))
                }
            },
        },
        other => rpc_err(&id, -32601, &format!("method `{other}` not found")),
    };
    out.reply = Some(response);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_covers_every_both_surface_operation() {
        let list = tool_list();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for op in waggle_ops::OPERATIONS {
            match op.surface {
                Surface::Both => assert!(names.contains(&op.name), "{} missing from MCP", op.name),
                Surface::CliOnly => {
                    assert!(
                        !names.contains(&op.name),
                        "{} must not be an MCP tool",
                        op.name
                    );
                }
            }
        }
    }

    #[test]
    fn tool_schemas_mark_required_args() {
        let list = tool_list();
        let mint = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "mint")
            .unwrap();
        let required = mint["inputSchema"]["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r == "target"));
    }
}
