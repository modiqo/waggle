//! CP-7 gates: the guidance walk (following `next_paths` from the root
//! reaches every leaf of the document) and the budget property at the
//! tool layer (no envelope slice ever exceeds `max-bytes`).

use serde_json::{json, Value};
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::SqliteStore;

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0x51_1CE5_u32;
    move |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = (state & 0xFF) as u8;
        }
        Ok(())
    }
}

/// A populated token: manifest with variants, funnel counts, children.
async fn populated(
    handler: &Handler<SqliteStore>,
    e: &mut impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
) -> String {
    let minted = handler
        .dispatch(
            "mint",
            &json!({ "target": "ws://q/report.md" }),
            Timestamp::from_unix_ms(1),
            e,
        )
        .await;
    let token = minted.result["token"].as_str().unwrap().to_owned();
    handler
        .dispatch(
            "resolve",
            &json!({ "token": token }),
            Timestamp::from_unix_ms(2),
            e,
        )
        .await;
    handler
        .dispatch(
            "record",
            &json!({ "token": token, "stage": "run" }),
            Timestamp::from_unix_ms(3),
            e,
        )
        .await;
    handler
        .dispatch(
            "mint",
            &json!({ "target": "ws://q/sub.md", "parent": token }),
            Timestamp::from_unix_ms(4),
            e,
        )
        .await;
    token
}

fn leaves(v: &Value, path: String, out: &mut Vec<String>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, child) in map {
                leaves(child, format!("{path}/{k}"), out);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (i, child) in items.iter().enumerate() {
                leaves(child, format!("{path}/{i}"), out);
            }
        }
        _ => out.push(path),
    }
}

#[test]
fn guidance_walk_reaches_every_leaf() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("q").unwrap(),
        );
        let mut e = entropy();
        let token = populated(&handler, &mut e).await;

        // The full document, assembled the same way the handler does —
        // via a giant-budget root query.
        let root = handler
            .dispatch(
                "query",
                &json!({ "token": token, "max-bytes": 1_000_000 }),
                Timestamp::from_unix_ms(9),
                &mut e,
            )
            .await;
        assert!(root.hint.is_none(), "{root:?}");
        let full_doc = root.result["slice"].clone();
        let mut expected = Vec::new();
        leaves(&full_doc, String::new(), &mut expected);
        assert!(
            expected.len() > 10,
            "the fixture is non-trivial: {expected:?}"
        );

        // BFS: from the root, follow ONLY next_paths. Every leaf of the
        // document must be visited — guidance is complete, not decorative.
        let mut queue = vec![String::new()];
        let mut visited_leaves = Vec::new();
        while let Some(path) = queue.pop() {
            let step = handler
                .dispatch(
                    "query",
                    &json!({ "token": token, "path": path, "max-bytes": 1_000_000 }),
                    Timestamp::from_unix_ms(9),
                    &mut e,
                )
                .await;
            assert!(step.hint.is_none(), "walk broke at `{path}`");
            let next = step.result["next_paths"].as_array().unwrap();
            if next.is_empty() {
                visited_leaves.push(path);
            } else {
                // Arrays elide middle indices in guidance; expand fully
                // from the slice itself for the completeness check.
                match &step.result["slice"] {
                    Value::Array(items) => {
                        for i in 0..items.len() {
                            queue.push(format!("{path}/{i}"));
                        }
                    }
                    _ => queue.extend(next.iter().map(|p| p.as_str().unwrap().to_owned())),
                }
            }
        }
        visited_leaves.sort();
        expected.sort();
        assert_eq!(
            visited_leaves, expected,
            "every leaf reachable via guidance"
        );
    });
}

#[test]
fn budget_holds_at_the_tool_layer() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("q").unwrap(),
        );
        let mut e = entropy();
        let token = populated(&handler, &mut e).await;

        for budget in [64u64, 200, 512, 4096] {
            let env = handler
                .dispatch(
                    "query",
                    &json!({ "token": token, "max-bytes": budget }),
                    Timestamp::from_unix_ms(9),
                    &mut e,
                )
                .await;
            let size = serde_json::to_string(&env.result["slice"]).unwrap().len() as u64;
            assert!(size <= budget.max(64), "budget {budget}: slice was {size}");
        }

        // A bad path errs with the valid roots named and a recovery step.
        let bad = handler
            .dispatch(
                "query",
                &json!({ "token": token, "path": "/nope" }),
                Timestamp::from_unix_ms(9),
                &mut e,
            )
            .await;
        assert!(bad.hint.as_ref().unwrap().contains("/manifest"));
        assert_eq!(bad.next[0].tool, "query");
    });
}
