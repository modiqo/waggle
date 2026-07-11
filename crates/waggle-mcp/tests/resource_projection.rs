//! Doc-21 gates over the JSON-RPC wire: capabilities advertise the
//! resource projection, reads ARE resolves (the funnel stays honest),
//! private/tombstoned tokens never enumerate, and subscriptions push
//! `notifications/resources/updated` on lifecycle mutations — never on
//! cosmetic churn, and never over a stateless transport.

use serde_json::{json, Value};
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::{handle_message, handle_session, Handler, Session};
use waggle_store_sqlite::SqliteStore;

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0x2E50_u32;
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

fn handler() -> Handler<SqliteStore> {
    Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
}

fn frame(id: u64, method: &str, params: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

async fn call(
    h: &Handler<SqliteStore>,
    s: &mut Session,
    e: &mut impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    at: u64,
    method: &str,
    params: Value,
) -> (Value, Vec<String>) {
    let out = handle_session(
        h,
        s,
        &frame(at, method, &params),
        Timestamp::from_unix_ms(at),
        e,
    )
    .await;
    let reply: Value = serde_json::from_str(&out.reply.expect("a reply")).unwrap();
    (reply, out.notifications)
}

/// Mint through the tool surface; returns the token.
async fn mint(
    h: &Handler<SqliteStore>,
    s: &mut Session,
    e: &mut impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    at: u64,
    extra: Value,
) -> String {
    let mut args = json!({ "target": format!("ws://res/artifact-{at}") });
    for (k, v) in extra.as_object().unwrap() {
        args[k] = v.clone();
    }
    let (reply, _) = call(
        h,
        s,
        e,
        at,
        "tools/call",
        json!({ "name": "mint", "arguments": args }),
    )
    .await;
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    serde_json::from_str::<Value>(text).unwrap()["result"]["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn capabilities_templates_and_honest_stateless_refusal() {
    let h = handler();
    let mut s = Session::default();
    let mut e = entropy();
    pollster::block_on(async {
        let (init, _) = call(&h, &mut s, &mut e, 1, "initialize", json!({})).await;
        let caps = &init["result"]["capabilities"];
        assert_eq!(caps["resources"]["subscribe"], true);
        assert!(caps["tools"].is_object(), "the verbs stay tools (21 §1)");

        let (tpl, _) = call(&h, &mut s, &mut e, 2, "resources/templates/list", json!({})).await;
        assert_eq!(
            tpl["result"]["resourceTemplates"][0]["uriTemplate"],
            "waggle://{token}"
        );

        // Stateless transports refuse subscriptions with the fix named.
        let refused = handle_message(
            &h,
            &frame(
                3,
                "resources/subscribe",
                &json!({ "uri": "waggle://b2uQyZUC" }),
            ),
            Timestamp::from_unix_ms(3),
            &mut e,
        )
        .await
        .unwrap();
        assert!(refused.contains("stateful connection"), "{refused}");
    });
}

#[test]
fn list_excludes_private_and_tombstoned_and_read_records_the_resolve() {
    let h = handler();
    let mut s = Session::default();
    let mut e = entropy();
    pollster::block_on(async {
        let public = mint(&h, &mut s, &mut e, 10, json!({})).await;
        let private = mint(&h, &mut s, &mut e, 11, json!({ "private": true })).await;
        let doomed = mint(&h, &mut s, &mut e, 12, json!({})).await;
        call(
            &h, &mut s, &mut e, 13, "tools/call",
            json!({ "name": "mutate", "arguments": { "token": doomed, "change": "revoke", "expected-version": 1 } }),
        )
        .await;

        let (list, _) = call(&h, &mut s, &mut e, 14, "resources/list", json!({})).await;
        let uris: Vec<&str> = list["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["uri"].as_str())
            .collect();
        assert!(uris.contains(&format!("waggle://{public}").as_str()));
        assert!(
            !uris
                .iter()
                .any(|u| u.contains(&private) || u.contains(&doomed)),
            "private tokens never enumerate; tombstones serve nothing: {uris:?}"
        );

        // A resource read IS a resolve: contents come back and the
        // funnel moved — the receipt trail survives the projection.
        let (read, _) = call(
            &h,
            &mut s,
            &mut e,
            15,
            "resources/read",
            json!({ "uri": format!("waggle://{public}") }),
        )
        .await;
        let contents = &read["result"]["contents"][0];
        assert_eq!(contents["uri"], format!("waggle://{public}"));
        assert!(contents["text"].as_str().is_some());
        let (funnel, _) = call(
            &h,
            &mut s,
            &mut e,
            16,
            "tools/call",
            json!({ "name": "funnel", "arguments": { "token": public } }),
        )
        .await;
        let text = funnel["result"]["content"][0]["text"].as_str().unwrap();
        let env: Value = serde_json::from_str(text).unwrap();
        assert_eq!(env["result"]["stages"]["resolve"], 1, "{env}");

        // Reading a tombstone refuses.
        let (gone, _) = call(
            &h,
            &mut s,
            &mut e,
            17,
            "resources/read",
            json!({ "uri": format!("waggle://{doomed}") }),
        )
        .await;
        assert!(gone["error"]["message"].as_str().is_some(), "{gone}");
    });
}

#[test]
fn subscriptions_push_on_lifecycle_never_on_cosmetics() {
    let h = handler();
    let mut s = Session::default();
    let mut e = entropy();
    pollster::block_on(async {
        let token = mint(&h, &mut s, &mut e, 20, json!({})).await;
        let uri = format!("waggle://{token}");
        call(
            &h,
            &mut s,
            &mut e,
            21,
            "resources/subscribe",
            json!({ "uri": uri }),
        )
        .await;

        // Cosmetic churn: no notification.
        let (_, quiet) = call(
            &h, &mut s, &mut e, 22, "tools/call",
            json!({ "name": "mutate", "arguments": { "token": token, "change": "label team=research" } }),
        )
        .await;
        assert!(quiet.is_empty(), "cosmetics never notify (21 §3)");

        // Lifecycle: the notification frame arrives, and the lifecycle
        // token is surfaced for the daemon hub's fan-out.
        let out = handle_session(
            &h,
            &mut s,
            &frame(23, "tools/call", &json!({ "name": "mutate", "arguments": { "token": token, "change": "revoke", "expected-version": 1 } })),
            Timestamp::from_unix_ms(23),
            &mut e,
        )
        .await;
        assert_eq!(
            out.lifecycle.map(|t| t.as_str().to_owned()),
            Some(token.clone())
        );
        assert_eq!(out.notifications.len(), 1);
        let n: Value = serde_json::from_str(&out.notifications[0]).unwrap();
        assert_eq!(n["method"], "notifications/resources/updated");
        assert_eq!(n["params"]["uri"], format!("waggle://{token}"));

        // After unsubscribe, silence — but the hub token still surfaces
        // (OTHER connections may care).
        let token2 = mint(&h, &mut s, &mut e, 24, json!({})).await;
        call(
            &h,
            &mut s,
            &mut e,
            25,
            "resources/subscribe",
            json!({ "uri": format!("waggle://{token2}") }),
        )
        .await;
        call(
            &h,
            &mut s,
            &mut e,
            26,
            "resources/unsubscribe",
            json!({ "uri": format!("waggle://{token2}") }),
        )
        .await;
        let (_, none) = call(
            &h, &mut s, &mut e, 27, "tools/call",
            json!({ "name": "mutate", "arguments": { "token": token2, "change": "revoke", "expected-version": 1 } }),
        )
        .await;
        assert!(none.is_empty());
    });
}
