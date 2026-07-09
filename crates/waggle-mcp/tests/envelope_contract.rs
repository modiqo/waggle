//! The envelope contract, swept mechanically: EVERY MCP operation's
//! success envelope must report what the call cost (`stats`) — the
//! field exists so agents can reason about what they touched, and
//! "this handler's author forgot" is not an acceptable state. Born
//! from a live gap: `map` shipped without stats and no test could
//! object, because no test stated the requirement. This one states it.

use serde_json::{json, Value};
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    // Varies per call: constant entropy would make every later mint an
    // idempotent REPLAY of the first (same sharer+nonce, C-8).
    let mut n = 0u8;
    move |b: &mut [u8]| {
        n = n.wrapping_add(1);
        b.fill(99u8.wrapping_add(n));
        Ok(())
    }
}

#[test]
fn every_operation_reports_its_stats() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let handler = Handler::new(
            SqliteStore::open(&dir.path().join("w.db")).unwrap(),
            Sharer::new("sweeper").unwrap(),
        )
        .with_blobs(BlobStore::open(&dir.path().join("blobs")).unwrap());
        let mut e = entropy();

        // Seed: a snapshot-backed token so content ops have substance.
        let artifact = dir.path().join("artifact.md");
        std::fs::write(&artifact, "# Title\n\nbody line with a needle\n").unwrap();
        let minted = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", artifact.display()),
                    "snapshot": true,
                }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "seed mint failed: {minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();
        // A child under the seed token, so lineage ops (coverage) have
        // a tree to audit.
        let child = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", artifact.display()),
                    "snapshot": true,
                    "parent": token,
                }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(child.hint.is_none(), "seed child failed: {child:?}");

        // Every MCP-facing operation, with working args for that token.
        let calls: Vec<(&str, Value)> = vec![
            ("resolve", json!({ "token": token })),
            ("record", json!({ "token": token, "stage": "run" })),
            ("funnel", json!({ "token": token })),
            ("read", json!({ "token": token, "lines": "1-2" })),
            ("search", json!({ "token": token, "pattern": "needle" })),
            ("query", json!({ "token": token })),
            ("find", json!({ "query": "artifact" })),
            ("coverage", json!({ "token": token })),
            ("map", json!({ "token": token })),
            ("map", json!({})),
            (
                "mutate",
                json!({ "token": token, "change": "label team=research" }),
            ),
        ];

        // The sweep covers the full MCP surface — if the catalog grows an
        // op this list doesn't exercise, fail loudly instead of silently
        // narrowing the contract.
        let mcp_ops: Vec<&str> = waggle_ops::OPERATIONS
            .iter()
            .filter(|op| !matches!(op.surface, waggle_ops::Surface::CliOnly))
            .map(|op| op.name)
            .collect();
        for op in &mcp_ops {
            assert!(
                *op == "mint" || calls.iter().any(|(name, _)| name == op),
                "operation `{op}` joined the catalog but not this sweep — add a call for it"
            );
        }

        for (name, call_args) in calls {
            let envelope = handler
                .dispatch(name, &call_args, Timestamp::from_unix_ms(50), &mut e)
                .await;
            assert!(
                envelope.hint.is_none(),
                "`{name}` {call_args} errored: {:?}",
                envelope.hint
            );
            assert!(
                !envelope.stats.is_empty(),
                "`{name}` touched the store but reported empty stats — every call says what it cost"
            );
        }
    });
}
