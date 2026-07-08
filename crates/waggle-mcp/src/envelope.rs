//! The fluency envelope (design doc `17 §2`): every tool response is
//! `{result, next, hint, stats}` where `next` entries are **executable,
//! schema-valid calls** — guidance an agent can act on, never prose that
//! rots. Errors carry a fix-naming `hint`.

use serde::Serialize;
use serde_json::Value;

/// One executable next step. `tool` must name a catalog operation and
/// `args` keys must be a subset of its declared args — machine-checked by
/// [`validate_next`] and the `envelope_next_valid` gate (17 §5).
#[derive(Debug, Clone, Serialize)]
pub struct NextCall {
    /// Catalog operation to call next.
    pub tool: String,
    /// Arguments, ready to send (templates like `<your-role>` where a
    /// value can't be known).
    pub args: Value,
    /// Why an agent would take this step — one calm sentence.
    pub why: String,
}

/// Measurability as a user feature (13 §6): every response says what it
/// cost and touched.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Stats {
    /// Records scanned/written by this call, when meaningful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<u64>,
    /// The store's per-token sequence assigned, when a write happened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u32>,
}

/// The response every tool returns (17 §2).
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    /// The tool's payload.
    pub result: Value,
    /// ≤3 ordered, executable next steps.
    pub next: Vec<NextCall>,
    /// Errors only: one calm, fix-naming sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// What this call cost/touched.
    pub stats: Stats,
}

impl Envelope {
    /// A success envelope.
    #[must_use]
    pub fn ok(result: Value, next: Vec<NextCall>) -> Self {
        Self {
            result,
            next,
            hint: None,
            stats: Stats::default(),
        }
    }

    /// An error envelope: the hint IS the payload (`hint_on_every_error`,
    /// 17 §5). `next` carries the recovery step where one exists.
    #[must_use]
    pub fn err(hint: impl Into<String>, next: Vec<NextCall>) -> Self {
        Self {
            result: Value::Null,
            next,
            hint: Some(hint.into()),
            stats: Stats::default(),
        }
    }

    /// Attach stats.
    #[must_use]
    pub fn with_stats(mut self, stats: Stats) -> Self {
        self.stats = stats;
        self
    }
}

/// Machine-check a `next` entry against the operations catalog: the tool
/// exists and every arg key is declared. The `envelope_next_valid` gate
/// runs this over every envelope the test suites see.
pub fn validate_next(call: &NextCall) -> Result<(), String> {
    let Some(op) = waggle_ops::find(&call.tool) else {
        return Err(format!(
            "next.tool `{}` is not a catalog operation",
            call.tool
        ));
    };
    if let Value::Object(map) = &call.args {
        for key in map.keys() {
            if !op.args.iter().any(|a| a.name == key.as_str()) {
                return Err(format!(
                    "next.args key `{key}` is not declared on `{}`",
                    call.tool
                ));
            }
        }
        Ok(())
    } else {
        Err(format!("next.args for `{}` must be an object", call.tool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_next_passes_invalid_fails() {
        let ok = NextCall {
            tool: "resolve".into(),
            args: json!({"token": "abc"}),
            why: "verify".into(),
        };
        assert!(validate_next(&ok).is_ok());

        let bad_tool = NextCall {
            tool: "explode".into(),
            args: json!({}),
            why: "no".into(),
        };
        assert!(validate_next(&bad_tool).is_err());

        let bad_arg = NextCall {
            tool: "mint".into(),
            args: json!({"payload": "x"}),
            why: "no".into(),
        };
        assert!(validate_next(&bad_arg).unwrap_err().contains("payload"));
    }

    #[test]
    fn error_envelopes_always_carry_hints() {
        let e = Envelope::err("read the manifest first", vec![]);
        assert!(e.hint.is_some());
        assert_eq!(e.result, Value::Null);
    }
}
