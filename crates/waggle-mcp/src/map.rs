//! The `map` engine (design doc `17 §3`): "I am here — what are my forward
//! and reverse paths?" `here` is **derived, never stored** — a pure
//! function of (manifest, funnel), so the map is always true at its
//! snapshot and can never be stale instruction. Forward paths are ranked
//! by state; reverse paths are honest about append-only.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use waggle_core::{AttributionManifest, Disposition, Stage, Timestamp};

use crate::envelope::{Envelope, NextCall};

/// The handoff line — `mint`'s `next[0]` and the map's lead suggestion for
/// an unresolved token (17 §2: it arrives at the moment the orchestrator
/// needs it).
pub fn handoff_line(token: &str) -> String {
    format!("resolve {token} via waggle for your working context")
}

/// The global map: where you stand with no token in hand.
pub fn global_map(token_count: u64) -> Envelope {
    let here = if token_count == 0 {
        "an empty store — nothing minted yet".to_owned()
    } else {
        format!("{token_count} token(s) in this store")
    };
    let forward = waggle_ops::MAP
        .forward
        .iter()
        .map(|e| NextCall {
            tool: e.to.to_owned(),
            args: json!({}),
            why: e.why.to_owned(),
        })
        .collect();
    Envelope::ok(json!({ "here": here }), forward)
}

/// The token map: state-ranked forward paths, honest reverse paths.
pub fn token_map(
    manifest: &AttributionManifest,
    funnel: &BTreeMap<Stage, u64>,
    child_count: usize,
    now: Timestamp,
) -> Envelope {
    let token = manifest.token.as_str();
    let resolves = funnel.get(&Stage::resolve()).copied().unwrap_or(0);
    let runs = funnel.get(&Stage::run()).copied().unwrap_or(0);
    let disposition = manifest.disposition(now);

    let here = format!(
        "{token} — {} · {} variant(s) · {resolves} resolve(s) · {runs} run(s) · {child_count} child(ren)",
        match &disposition {
            Disposition::Active => "active",
            Disposition::Expired => "expired",
            Disposition::Revoked { .. } => "revoked (tombstone)",
            Disposition::Superseded { .. } => "superseded",
        },
        manifest.variants.len(),
    );

    // Forward, ranked by state (17 §3): the map is the skill, computed.
    let mut forward: Vec<NextCall> = Vec::new();
    let mut guidance = String::new();
    match disposition {
        Disposition::Revoked { .. } => {
            guidance = "this token is a tombstone — mint a fresh one from the artifact".into();
            forward.push(NextCall {
                tool: "mint".into(),
                args: json!({ "target": manifest.target.as_str() }),
                why: "re-mint from the same artifact if it should live again".into(),
            });
        }
        Disposition::Superseded { by } => {
            guidance = format!("superseded — follow the pointer to {by}");
            forward.push(NextCall {
                tool: "resolve".into(),
                args: json!({ "token": by.as_str() }),
                why: "the corrected artifact lives at the replacement token".into(),
            });
        }
        Disposition::Active | Disposition::Expired => {
            if resolves == 0 {
                guidance = format!(
                    "no consumer has resolved this yet — hand off with: '{}'",
                    handoff_line(token)
                );
            }
            forward.push(NextCall {
                tool: "resolve".into(),
                args: json!({ "token": token }),
                why: if resolves == 0 {
                    "self-check the projection each consumer will receive".into()
                } else {
                    "fetch the projection for your context".into()
                },
            });
            forward.push(NextCall {
                tool: if resolves > 0 { "funnel" } else { "record" }.into(),
                args: json!({ "token": token }),
                why: if resolves > 0 {
                    "see which consumers resolved and which stalled".into()
                } else {
                    "report stages as your task progresses".into()
                },
            });
        }
    }
    forward.truncate(3);

    // Reverse: mutations reverse via CAS-guarded lifecycle; events do not
    // reverse — say so and offer the compensating move (17 §3).
    let reverse = vec![
        NextCall {
            tool: "mutate".into(),
            args: json!({ "token": token, "change": "revoke", "expected-version": manifest.version }),
            why: "withdraw — children tombstone with it".into(),
        },
        NextCall {
            tool: "mutate".into(),
            args: json!({ "token": token, "change": "supersede=<new-token>", "expected-version": manifest.version }),
            why: "replace with a corrected artifact; late resolvers follow the pointer".into(),
        },
    ];

    let mut result = json!({
        "here": here,
        "reverse": reverse,
        "irreversible": { "events": "history does not un-happen — record a correcting stage instead" },
    });
    if !guidance.is_empty() {
        result["guidance"] = Value::String(guidance);
    }
    Envelope::ok(result, forward)
}
