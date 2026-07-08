//! The MCP wire layer: JSON-RPC 2.0, newline-delimited (the MCP stdio
//! transport). Deliberately minimal — `initialize`, `tools/list`,
//! `tools/call`, `ping` — and transport-agnostic: [`handle_message`] maps
//! one request string to one response string; stdio loops and HTTP
//! handlers both call it. The shim can add no semantics because there are
//! none here to add (16 §3).

use serde_json::{json, Value};
use waggle_core::Timestamp;
use waggle_ops::Surface;
use waggle_store::Store;

use crate::handlers::Handler;

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

/// Handle one JSON-RPC message. Returns `None` for notifications (no
/// response on the wire). `now`/`entropy` come from the transport.
pub async fn handle_message<S: Store>(
    handler: &Handler<S>,
    raw: &str,
    now: Timestamp,
    entropy: &mut dyn FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
) -> Option<String> {
    let msg: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => return Some(rpc_err(&Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // Notifications (no id) get no response.
    let id = msg.get("id").cloned()?;
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

    let response = match method {
        "initialize" => rpc_ok(
            &id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
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
        other => rpc_err(&id, -32601, &format!("method `{other}` not found")),
    };
    Some(response)
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
