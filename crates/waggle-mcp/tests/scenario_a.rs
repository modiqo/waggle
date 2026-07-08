//! CP-6 gate: scenario A (design doc `06 §7`) as an executable test —
//! an orchestrator mints an attributed reference with variants, hands the
//! ~30-byte token to subagents, each resolves its own projection, work is
//! recorded, the funnel answers attribution, the map guides throughout.
//! Driven through the real MCP wire (`tools/call` JSON-RPC frames), over
//! the real `SQLite` store.

use serde_json::{json, Value};
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::{handle_message, validate_next, Handler, NextCall};
use waggle_store_sqlite::SqliteStore;

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0xC0FF_EE11_u32;
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

/// Send one tools/call frame; return the parsed envelope. Every envelope
/// is `envelope_next_valid`-checked on the way out (17 §5).
async fn call<E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>>(
    handler: &Handler<SqliteStore>,
    e: &mut E,
    now: u64,
    tool: &str,
    args: Value,
) -> Value {
    let frame = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": args },
    })
    .to_string();
    let response = handle_message(handler, &frame, Timestamp::from_unix_ms(now), e)
        .await
        .expect("tools/call always answers");
    let rpc: Value = serde_json::from_str(&response).unwrap();
    let text = rpc["result"]["content"][0]["text"].as_str().unwrap();
    let envelope: Value = serde_json::from_str(text).unwrap();
    for next in envelope["next"].as_array().unwrap() {
        let call = NextCall {
            tool: next["tool"].as_str().unwrap().to_owned(),
            args: next["args"].clone(),
            why: next["why"].as_str().unwrap_or("").to_owned(),
        };
        validate_next(&call).expect("envelope_next_valid: every next is executable");
    }
    envelope
}

#[test]
#[allow(clippy::too_many_lines)] // the scenario IS the narrative; splitting it would hide the flow
fn scenario_a_orchestrator_to_subagents_over_the_wire() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("orchestrator").unwrap(),
        );
        let mut e = entropy();

        // Handshake: initialize + tools/list come from the catalog.
        let init = handle_message(
            &handler,
            &json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}).to_string(),
            Timestamp::from_unix_ms(0),
            &mut e,
        )
        .await
        .unwrap();
        assert!(init.contains("waggled"));
        let list = handle_message(
            &handler,
            &json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}).to_string(),
            Timestamp::from_unix_ms(0),
            &mut e,
        )
        .await
        .unwrap();
        for tool in ["mint", "resolve", "record", "mutate", "funnel", "map"] {
            assert!(list.contains(&format!("\"{tool}\"")), "{tool} on the wire");
        }
        assert!(
            !list.contains("\"serve\""),
            "CLI-only ops stay off the wire"
        );

        // 1. The orchestrator mints ONE call with two variants (17 §5
        //    one_call_mint: only target is required; variants optional).
        // The variant travels in the manifest's own serde schema — built
        // from the real types so this test IS the wire documentation.
        let claude_variant = waggle_core::Variant {
            match_expr: waggle_core::MatchExpr {
                model_family: waggle_core::Constraint::OneOf(vec!["claude".into()]),
                ..waggle_core::MatchExpr::default()
            },
            body: waggle_core::VariantBody::Inline {
                content_type: "text/markdown".into(),
                data: "# Claude-tuned guidance".into(),
            },
            revalidate_after_ms: None,
        };
        let minted = call(
            &handler,
            &mut e,
            1_000,
            "mint",
            json!({
                "target": "ws://swarm/findings/report.md",
                "variants": [serde_json::to_value(&claude_variant).unwrap()],
            }),
        )
        .await;
        assert!(minted["hint"].is_null(), "{minted}");
        let token = minted["result"]["token"].as_str().unwrap().to_owned();
        assert!(
            token.len() <= 12,
            "the handoff IS ~30 bytes, not a context dump"
        );
        assert!(minted["result"]["handoff"]
            .as_str()
            .unwrap()
            .contains(&token));
        assert_eq!(
            minted["result"]["variants"], 2,
            "declared + synthesized catch-all"
        );

        // 2. The map, before any resolve: handoff guidance leads.
        let map0 = call(&handler, &mut e, 1_500, "map", json!({ "token": token })).await;
        assert!(map0["result"]["guidance"]
            .as_str()
            .unwrap()
            .contains("hand off"));

        // 3. A Claude subagent resolves → the claude variant (index 0).
        let claude = call(
            &handler,
            &mut e,
            2_000,
            "resolve",
            json!({ "token": token, "context": {
                "kind": "agent", "model_family": "claude",
                "modalities": 1, "posture": "headless" } }),
        )
        .await;
        assert_eq!(claude["result"]["variant"], 0);
        assert!(claude["result"]["body"]["inline"]["data"]
            .as_str()
            .unwrap()
            .contains("Claude-tuned"));

        // 4. An anonymous subagent resolves → the catch-all (index 1),
        //    same token, different projection: adaptive by construction.
        let anon = call(
            &handler,
            &mut e,
            2_100,
            "resolve",
            json!({ "token": token }),
        )
        .await;
        assert_eq!(anon["result"]["variant"], 1);

        // 5. Work happens; the subagent reports it.
        let rec = call(
            &handler,
            &mut e,
            3_000,
            "record",
            json!({ "token": token, "stage": "run" }),
        )
        .await;
        assert!(rec["hint"].is_null());

        // 6. The funnel answers attribution: 2 resolves, 1 run.
        let funnel = call(&handler, &mut e, 4_000, "funnel", json!({ "token": token })).await;
        assert_eq!(funnel["result"]["stages"]["resolve"], 2);
        assert_eq!(funnel["result"]["stages"]["run"], 1);

        // 7. The report is corrected: supersede, CAS-guarded. First with a
        //    stale version — the hint names the fix (hint_on_every_error).
        let stale = call(
            &handler,
            &mut e,
            5_000,
            "mutate",
            json!({ "token": token, "change": "revoke", "expected-version": 41 }),
        )
        .await;
        let hint = stale["hint"].as_str().unwrap();
        assert!(
            hint.contains("re-read"),
            "conflict hint names the fix: {hint}"
        );
        assert_eq!(
            stale["next"][0]["tool"], "resolve",
            "recovery step is executable"
        );

        let revoked = call(
            &handler,
            &mut e,
            5_100,
            "mutate",
            json!({ "token": token, "change": "revoke", "expected-version": 1 }),
        )
        .await;
        assert!(revoked["hint"].is_null());

        // 8. A late subagent resolves the tombstone: disposition says so,
        //    nothing is served.
        let late = call(
            &handler,
            &mut e,
            6_000,
            "resolve",
            json!({ "token": token }),
        )
        .await;
        assert!(late["result"]["disposition"]
            .to_string()
            .contains("revoked"));
        assert!(late["result"]["body"].is_null(), "revoked serves nothing");

        // 9. The map now leads with re-mint guidance (map_state_table).
        let map1 = call(&handler, &mut e, 7_000, "map", json!({ "token": token })).await;
        assert!(map1["result"]["here"].as_str().unwrap().contains("revoked"));
        assert_eq!(
            map1["next"][0]["tool"], "mint",
            "tombstone map leads to re-mint"
        );
    });
}

#[test]
fn one_call_mint_and_global_map() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("session").unwrap(),
        );
        let mut e = entropy();

        // The global map on an empty store: oriented from nothing.
        let map = call(&handler, &mut e, 1, "map", json!({})).await;
        assert!(map["result"]["here"].as_str().unwrap().contains("empty"));
        assert_eq!(map["next"][0]["tool"], "mint", "empty store leads to mint");

        // one_call_mint (17 §5): target alone suffices; everything else
        // defaults (session sharer, subagent/general, catch-all variant).
        let minted = call(
            &handler,
            &mut e,
            2,
            "mint",
            json!({ "target": "file:///tmp/notes.md" }),
        )
        .await;
        assert!(minted["hint"].is_null());
        assert_eq!(minted["result"]["variants"], 1, "catch-all synthesized");

        // Unknown token: hint + executable recovery.
        let missing = call(&handler, &mut e, 3, "resolve", json!({ "token": "zzzzz" })).await;
        assert!(missing["hint"].as_str().unwrap().contains("unknown token"));
        assert_eq!(missing["next"][0]["tool"], "map");
    });
}

#[test]
fn envelope_error_shape_is_uniform() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("session").unwrap(),
        );
        let mut e = entropy();
        // Every error path yields hint + null result (hint_on_every_error).
        for (tool, args) in [
            ("mint", json!({})),                                        // missing target
            ("resolve", json!({})),                                     // missing token
            ("record", json!({ "token": "abc" })),                      // missing stage
            ("mutate", json!({ "token": "abc", "change": "explode" })), // bad change
            ("nonsense", json!({})),                                    // unknown tool
        ] {
            let env = call(&handler, &mut e, 1, tool, args).await;
            assert!(env["hint"].is_string(), "{tool} must hint");
            assert!(env["result"].is_null(), "{tool} error result is null");
        }
    });
}

/// Regression (found via the G-8 federation hunt): a variant's declared
/// `revalidate_after_ms` must survive the mint tool path — the two-arg
/// builder was silently dropping it, turning every declared freshness
/// window into the 15-minute default.
#[test]
fn declared_revalidate_window_survives_mint() {
    pollster::block_on(async {
        let handler = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("fresh").unwrap(),
        );
        let mut e = entropy();
        let minted = handler
            .dispatch(
                "mint",
                &json!({
                    "target": "ws://fresh/x",
                    "variants": [{
                        "match": {},
                        "body": { "inline": { "content_type": "text/plain", "data": "v" } },
                        "revalidate_after_ms": 800,
                    }],
                }),
                Timestamp::from_unix_ms(1_000),
                &mut e,
            )
            .await;
        let token = minted.result["token"].as_str().unwrap().to_owned();
        let resolved = handler
            .dispatch(
                "resolve",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(2_000),
                &mut e,
            )
            .await;
        assert_eq!(
            resolved.result["revalidate_after"], 2_800,
            "the declared 800ms window applies — not the 15-minute default"
        );
    });
}
