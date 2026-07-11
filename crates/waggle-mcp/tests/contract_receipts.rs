//! P0/P1 gates (design doc `19 §4.1–4.2`), end to end over the dispatcher
//! and the real `SQLite` store: the consumption contract declared at mint,
//! region touches stamped by served reads and search hits, single-token
//! `coverage` naming its misses, and the judged outcome (`accepted` /
//! `rejected`) joining the funnel — with a rejection teaching the
//! escalation choreography.

use serde_json::json;
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0xBEE5_u32;
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

const PLAN: &str = "\
# Plan

## Scope
lines here
and here

## Pricing
the load-bearing numbers
sit on these lines
exactly

## Rollout
ship in phases
";

fn handler_with_blobs(dir: &std::path::Path) -> Handler<SqliteStore, BlobStore> {
    Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
    .with_blobs(BlobStore::open(&dir.join("blobs")).unwrap())
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-contract-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn contract_coverage_counts_only_what_was_served_and_names_the_miss() {
    let dir = scratch("coverage");
    let file = dir.join("plan.md");
    std::fs::write(&file, PLAN).unwrap();
    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        // Mint with a two-region contract: the section sugar resolves
        // against the outline NOW, and the manifest stores plain ranges.
        let minted = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", file.display()),
                    "snapshot": true,
                    "require": ["section:Pricing", "section:Rollout"],
                }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // Resolving and reading OUTSIDE the required regions moves the
        // funnel but touches no contract region.
        handler
            .dispatch(
                "read",
                &json!({ "token": token, "lines": "1-5" }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        let unmet = handler
            .dispatch(
                "coverage",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(unmet.hint.is_none(), "{unmet:?}");
        assert_eq!(unmet.result["met"], false);
        assert_eq!(unmet.result["contract"]["required"], 2);
        assert_eq!(unmet.result["contract"]["touched"], 0);
        let missed = unmet.result["missed"].as_array().unwrap();
        assert_eq!(missed.len(), 2, "both misses NAMED: {missed:?}");
        assert_eq!(missed[0]["label"], "Pricing");
        assert_eq!(
            unmet.next[0].tool, "read",
            "the gap-closing read is the offered next step"
        );

        // A search hit inside §Pricing is a touch — the grep is the
        // evidence.
        let found = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "load-bearing" }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(found.hint.is_none());
        // A windowed read overlapping §Rollout closes the second region.
        let rollout_window = unmet.result["missed"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["label"] == "Rollout")
            .and_then(|m| m["lines"].as_str())
            .unwrap()
            .to_owned();
        handler
            .dispatch(
                "read",
                &json!({ "token": token, "lines": rollout_window }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;

        let met = handler
            .dispatch(
                "coverage",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert_eq!(met.result["met"], true, "{met:?}");
        assert_eq!(met.result["contract"]["permille"], 1000);
        assert!(met.result["missed"].as_array().unwrap().is_empty());
        assert_eq!(met.result["outcome"], "pending", "consumed ≠ judged");
    });
}

#[test]
fn outcome_stages_join_the_funnel_and_rejection_teaches_escalation() {
    let dir = scratch("outcome");
    let file = dir.join("plan.md");
    std::fs::write(&file, PLAN).unwrap();
    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let parent = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", file.display()) }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let parent_token = parent.result["token"].as_str().unwrap().to_owned();
        let child = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", file.display()),
                    "parent": parent_token,
                    "channel": "subagent/pricing",
                }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        let token = child.result["token"].as_str().unwrap().to_owned();

        // Pending until a judge speaks.
        let funnel = handler
            .dispatch(
                "funnel",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(funnel.result["outcome"], "pending");

        // The judge rejects: the envelope teaches the escalation — a
        // re-mint of the same target under the SAME parent, then the
        // supersede that makes the escalation lineage.
        let rejected = handler
            .dispatch(
                "record",
                &json!({ "token": token, "stage": "rejected" }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(rejected.hint.is_none(), "{rejected:?}");
        assert_eq!(rejected.next[0].tool, "mint");
        assert_eq!(
            rejected.next[0].args["parent"].as_str(),
            Some(parent_token.as_str()),
            "escalation re-mints under the same parent — lineage, not lore"
        );
        assert_eq!(rejected.next[1].tool, "mutate");

        let funnel = handler
            .dispatch(
                "funnel",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert_eq!(funnel.result["outcome"], "rejected");
        assert_eq!(funnel.result["stages"]["rejected"], 1);

        // A later accept makes it contested — surfaced, never silently
        // overwritten.
        handler
            .dispatch(
                "record",
                &json!({ "token": token, "stage": "accepted" }),
                Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        let funnel = handler
            .dispatch(
                "funnel",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(7),
                &mut e,
            )
            .await;
        assert_eq!(funnel.result["outcome"], "contested");
    });
}

#[test]
fn contract_mint_rejects_what_it_cannot_honor() {
    let dir = scratch("refusals");
    let file = dir.join("plan.md");
    std::fs::write(&file, PLAN).unwrap();
    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    let target = format!("file://{}", file.display());
    pollster::block_on(async {
        // A section the outline doesn't have fails AT MINT, not at audit.
        let bad = handler
            .dispatch(
                "mint",
                &json!({ "target": target, "require": ["section:Nonexistent"] }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let hint = bad.hint.expect("must refuse");
        assert!(hint.contains("no section"), "{hint}");

        // A threshold with no regions is a mistake worth naming.
        let orphan = handler
            .dispatch(
                "mint",
                &json!({ "target": target, "min-coverage": 0.5 }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(orphan
            .hint
            .expect("must refuse")
            .contains("without require"));

        // Coverage on a contract-free, childless token names both audits.
        let plain = handler
            .dispatch(
                "mint",
                &json!({ "target": target }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        let token = plain.result["token"].as_str().unwrap().to_owned();
        let cov = handler
            .dispatch(
                "coverage",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        let hint = cov.hint.expect("must refuse");
        assert!(hint.contains("no children and no contract"), "{hint}");
    });
}
