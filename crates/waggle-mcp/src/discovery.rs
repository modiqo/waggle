//! Discovery: `find` — names as LOOKUP, tokens as IDENTITY (the user's
//! design ruling). Ranked candidates matched on target basename, tags,
//! channel, and sharer; disposition always visible so a dead name is
//! visibly dead; the newest candidate rides as an executable next.

use serde_json::{json, Map, Value};
use waggle_store::{BlobSink, Store};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{arg_str, store_err, Handler};

/// Tags at mint: repeatable "k=v" (a bare word becomes `name=<word>`),
/// an array over MCP or repeated flags from the CLI.
pub(crate) fn apply_tags(
    mut spec: waggle_core::MintSpec,
    args: &Map<String, Value>,
) -> waggle_core::MintSpec {
    if let Some(tags) = args.get("tag") {
        let list: Vec<String> = match tags {
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            Value::String(s) => vec![s.clone()],
            _ => vec![],
        };
        for tag in list {
            let (k, v) = tag
                .split_once('=')
                .map_or(("name", tag.as_str()), |(k, v)| (k, v));
            spec = spec.label(k, v);
        }
    }
    spec
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// `find`: ranked candidates matching basename/tags/channel/sharer.
    pub(crate) async fn find(&self, args: &Map<String, Value>) -> Envelope {
        let Some(query) = arg_str(args, "query") else {
            return Envelope::err("missing `query` — what should I look for?", vec![]);
        };
        let needle = query.to_lowercase();
        let records = match self.store.scan_all().await {
            Ok(r) => r,
            Err(e) => return store_err(&e),
        };
        let world = waggle_core::reconstruct(records);
        let mut hits: Vec<&waggle_core::AttributionManifest> = world
            .manifests
            .values()
            .filter(|m| {
                let basename = m.target.as_str().rsplit('/').next().unwrap_or_default();
                basename.to_lowercase().contains(&needle)
                    || m.channel.as_str().to_lowercase().contains(&needle)
                    || m.sharer.as_str().to_lowercase().contains(&needle)
                    || m.labels.iter().any(|(k, v)| {
                        k.to_lowercase().contains(&needle) || v.to_lowercase().contains(&needle)
                    })
            })
            .collect();
        hits.sort_by_key(|m| std::cmp::Reverse(m.minted_at.as_unix_ms()));
        let total = hits.len();
        hits.truncate(10);
        let candidates: Vec<Value> = hits
            .iter()
            .map(|m| {
                let disposition = if m.revoked_at.is_some() {
                    json!("revoked")
                } else if let Some(by) = m.superseded_by {
                    json!({ "superseded_by": by.as_str() })
                } else {
                    json!("active")
                };
                json!({
                    "token": m.token.as_str(),
                    "target": m.target.as_str(),
                    "disposition": disposition,
                    "sharer": m.sharer.as_str(),
                    "minted_at": m.minted_at,
                    "tags": m.labels,
                })
            })
            .collect();
        let next = hits
            .first()
            .map(|m| {
                vec![NextCall {
                    tool: "resolve".into(),
                    args: json!({ "token": m.token.as_str() }),
                    why: "the newest candidate — resolve it if it's the one you meant".into(),
                }]
            })
            .unwrap_or_default();
        Envelope::ok(
            json!({
                "query": query,
                "total": total,
                "candidates": candidates,
            }),
            next,
        )
        .with_stats(Stats {
            records: Some(total as u64),
            seq: None,
        })
    }
}
