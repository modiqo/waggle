//! The guided query engine (design doc `14 CP-7`): slice, don't dump.
//! `query(token, path)` returns a **budgeted** slice of the token's
//! document (manifest + funnel + lineage) plus `next_paths` — executable
//! paths deeper — so an agent walks exactly as far as it needs and no
//! response ever exceeds `max_bytes`.
//!
//! Paths are a JSON-Pointer subset: `/`-separated keys and array indices,
//! no escape sequences (waggle keys are slugs — none contain `/` or `~`).

use serde_json::{json, Value};

/// Default response budget: 4 KB — a slice, not a payload.
pub const DEFAULT_MAX_BYTES: usize = 4096;

/// Floor for `max_bytes`: below this even the shape summary can't fit.
pub const MIN_MAX_BYTES: usize = 64;

/// The outcome of one guided query step.
#[derive(Debug)]
pub struct Slice {
    /// The slice (or its shape summary when the full value blew the
    /// budget — `truncated` says which).
    pub slice: Value,
    /// Executable paths deeper, in declaration order.
    pub next_paths: Vec<String>,
    /// True when `slice` is a shape summary, not the value itself.
    pub truncated: bool,
    /// Serialized size of the full value at this path (what you avoided).
    pub full_bytes: usize,
}

/// Descend `doc` by `path`. Returns `None` when the path names nothing —
/// callers turn that into a hint plus the valid siblings.
#[must_use]
pub fn descend<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    let mut node = doc;
    for part in path.split('/').filter(|p| !p.is_empty()) {
        node = match node {
            Value::Object(map) => map.get(part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

/// Slice `doc` at `path` under `max_bytes`. Total: always returns, and the
/// serialized `slice` never exceeds the budget (the budget property test
/// holds this over random documents, paths, and budgets).
#[must_use]
pub fn slice_at(doc: &Value, path: &str, max_bytes: usize) -> Option<Slice> {
    let budget = max_bytes.max(MIN_MAX_BYTES);
    let node = descend(doc, path)?;
    let full = serde_json::to_string(node).unwrap_or_default();
    let next_paths = child_paths(node, path);
    if full.len() <= budget {
        return Some(Slice {
            slice: node.clone(),
            next_paths,
            truncated: false,
            full_bytes: full.len(),
        });
    }
    // Over budget: shrink down a ladder — fewer keys, then no hint text,
    // then the bare {kind, bytes} which always fits the floor.
    let mut keys = shape_keys(node);
    let mut with_hint = true;
    loop {
        let summary = shape_summary(node, &keys, full.len(), with_hint);
        let size = serde_json::to_string(&summary).map_or(usize::MAX, |s| s.len());
        if size <= budget {
            return Some(Slice {
                slice: summary,
                next_paths,
                truncated: true,
                full_bytes: full.len(),
            });
        }
        if keys.is_empty() {
            if with_hint {
                with_hint = false;
                continue;
            }
            return Some(Slice {
                slice: json!({ "kind": kind_of(node), "bytes": full.len() }),
                next_paths,
                truncated: true,
                full_bytes: full.len(),
            });
        }
        keys.truncate(keys.len() / 2);
    }
}

fn kind_of(node: &Value) -> &'static str {
    match node {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "bool",
        Value::Null => "null",
    }
}

fn shape_keys(node: &Value) -> Vec<String> {
    match node {
        Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn shape_summary(node: &Value, keys: &[String], full_bytes: usize, with_hint: bool) -> Value {
    let mut summary = match node {
        Value::Object(_) => json!({ "kind": "object", "bytes": full_bytes, "keys": keys }),
        Value::Array(items) => json!({ "kind": "array", "bytes": full_bytes, "len": items.len() }),
        Value::String(s) => {
            let prefix: String = s.chars().take(48).collect();
            json!({ "kind": "string", "bytes": full_bytes, "prefix": prefix })
        }
        other => return other.clone(),
    };
    if with_hint {
        summary["hint"] = json!("over budget — follow a next path or raise max-bytes");
    }
    summary
}

/// Paths one level deeper — the guidance. Objects offer every key; arrays
/// offer the first three indices (and the last, when longer).
fn child_paths(node: &Value, path: &str) -> Vec<String> {
    let base = path.trim_end_matches('/');
    let join = |part: &str| {
        if base.is_empty() {
            format!("/{part}")
        } else {
            format!("{base}/{part}")
        }
    };
    match node {
        Value::Object(map) => map.keys().map(|k| join(k)).collect(),
        Value::Array(items) => {
            let mut paths: Vec<String> = (0..items.len().min(3))
                .map(|i| join(&i.to_string()))
                .collect();
            if items.len() > 3 {
                paths.push(join(&(items.len() - 1).to_string()));
            }
            paths
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Value {
        json!({
            "manifest": { "token": "abc123", "variants": [
                { "body": "x".repeat(6000) }, { "body": "catch-all" } ] },
            "funnel": { "resolve": 2, "run": 1 },
            "children": ["kid1", "kid2", "kid3", "kid4", "kid5"],
        })
    }

    #[test]
    fn descend_walks_objects_and_arrays() {
        let d = doc();
        assert_eq!(descend(&d, "/funnel/run"), Some(&json!(1)));
        assert_eq!(descend(&d, "/children/1"), Some(&json!("kid2")));
        assert_eq!(
            descend(&d, "/manifest/variants/1/body"),
            Some(&json!("catch-all"))
        );
        assert!(descend(&d, "/nope").is_none());
        assert!(descend(&d, "/children/9").is_none());
    }

    #[test]
    fn small_values_come_back_whole_with_guidance() {
        let s = slice_at(&doc(), "/funnel", 4096).unwrap();
        assert!(!s.truncated);
        assert_eq!(s.slice["run"], 1);
        assert!(s.next_paths.contains(&"/funnel/resolve".to_owned()));
    }

    #[test]
    fn oversized_values_come_back_as_shape_within_budget() {
        let s = slice_at(&doc(), "", 256).unwrap();
        assert!(s.truncated);
        assert!(serde_json::to_string(&s.slice).unwrap().len() <= 256);
        assert_eq!(s.slice["kind"], "object");
        assert!(s.full_bytes > 6000, "the avoided payload is reported");
    }

    #[test]
    fn arrays_guide_by_index_with_tail() {
        let s = slice_at(&doc(), "/children", 4096).unwrap();
        assert_eq!(
            s.next_paths,
            vec!["/children/0", "/children/1", "/children/2", "/children/4"]
        );
    }

    #[test]
    fn budget_property_random_docs_paths_budgets() {
        // No response exceeds max_bytes — over random shapes and budgets.
        let mut state = 0xBEEF_u32;
        let mut rnd = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let d = doc();
        let paths = [
            "",
            "/manifest",
            "/manifest/variants",
            "/manifest/variants/0",
            "/manifest/variants/0/body",
            "/funnel",
            "/children",
        ];
        for _ in 0..2_000 {
            let path = paths[(rnd() as usize) % paths.len()];
            let budget = MIN_MAX_BYTES + (rnd() as usize) % 8_192;
            let s = slice_at(&d, path, budget).unwrap();
            let size = serde_json::to_string(&s.slice).unwrap().len();
            assert!(size <= budget, "{path} @ {budget}: {size}");
        }
    }
}
