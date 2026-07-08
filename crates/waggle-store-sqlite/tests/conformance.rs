//! The `SQLite` backend is a waggle backend because this run is green
//! (design doc `07 §5`) — plus reopen durability (C-1) and the wire-format
//! round-trip that makes replay-as-migration real (16 §4).

use waggle_core::{reconstruct, Stage};
use waggle_store::conformance::{run_all, Harness};
use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
use waggle_store_sqlite::SqliteStore;

fn minted_manifest(seed: u8, target: &str) -> waggle_core::AttributionManifest {
    let mut entropy = move |b: &mut [u8]| {
        for (i, x) in b.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *x = seed.wrapping_mul(37).wrapping_add(i as u8);
            }
        }
        Ok(())
    };
    waggle_core::mint(
        waggle_core::MintSpec::new(
            waggle_core::CanonicalUrl::new(target).unwrap(),
            waggle_core::Sharer::new("lead").unwrap(),
            waggle_core::Channel::subagent_general(),
        ),
        &waggle_core::MintOptions::default(),
        &mut entropy,
        waggle_core::Timestamp::from_unix_ms(1),
    )
    .unwrap()
}

fn event_intent(token: waggle_core::Token, stage: Stage) -> AppendIntent {
    AppendIntent::Event {
        token,
        stage,
        actor: waggle_core::ActorClass::from_context(
            &waggle_core::ResolverContext::anonymous_agent(),
        ),
        variant: None,
        at: waggle_core::Timestamp::from_unix_ms(4),
    }
}

#[test]
fn sqlite_backend_passes_conformance() {
    run_all(&Harness::new(|| {
        SqliteStore::open_in_memory().expect("in-memory sqlite")
    }));
}

#[test]
fn sqlite_survives_reopen_from_disk() {
    // C-1 in its plainest form: what was acked is there after reopen.
    let dir = std::env::temp_dir().join(format!("waggle-sqlite-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("store.db");

    let token = pollster::block_on(async {
        let store = SqliteStore::open(&path).unwrap();
        let manifest = minted_manifest(21, "ws://persist/artifact");
        let token = manifest.token;
        store
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(1),
            })
            .await
            .unwrap();
        store
            .append(event_intent(token, Stage::run()))
            .await
            .unwrap();
        token
    });

    pollster::block_on(async {
        let reopened = SqliteStore::open(&path).unwrap();
        let view = reopened
            .manifest(token)
            .await
            .unwrap()
            .expect("acked mint survives reopen");
        assert_eq!(view.manifest.token, token);
        let funnel = reopened.funnel(token).await.unwrap();
        assert_eq!(funnel[&Stage::run()], 1);
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn wire_export_replay_roundtrip_with_duplicates() {
    // 16 §4: JSONL is the migration format; C-4 dedupe makes replay
    // retry-safe; R-1 makes the destination equal the source.
    pollster::block_on(async {
        let source = SqliteStore::open_in_memory().unwrap();
        let manifest = minted_manifest(33, "ws://migrate/artifact");
        let token = manifest.token;
        source
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(9),
            })
            .await
            .unwrap();
        for stage in [Stage::resolve(), Stage::run(), Stage::repeat()] {
            source.append(event_intent(token, stage)).await.unwrap();
        }

        // Export: records → JSONL lines (the wire format).
        let records = source.scan_all().await.unwrap();
        let jsonl: Vec<String> = records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect();

        // Replay into a fresh store — every line TWICE (an injected
        // mid-replay retry). C-4 dedupe must absorb it.
        let target = SqliteStore::open_in_memory().unwrap();
        let mut fresh = 0;
        for line in jsonl.iter().chain(jsonl.iter()) {
            let record: waggle_core::LogRecord = serde_json::from_str(line).unwrap();
            if target.ingest(record).await.unwrap() {
                fresh += 1;
            }
        }
        assert_eq!(fresh, jsonl.len(), "each record applies exactly once");

        // Reconstruct-equality: the destination IS the source.
        let src_world = serde_json::to_string(&reconstruct(records)).unwrap();
        let dst_world =
            serde_json::to_string(&reconstruct(target.scan_all().await.unwrap())).unwrap();
        assert_eq!(src_world, dst_world, "export→replay→reconstruct ≡ (R-1)");

        // And the materialized views converged too (R-4 after ingest).
        let funnel = target.funnel(token).await.unwrap();
        assert_eq!(funnel[&Stage::run()], 1);
        assert!(target.manifest(token).await.unwrap().is_some());
    });
}

#[test]
fn cache_serves_hot_reads_and_invalidates_on_write() {
    pollster::block_on(async {
        let store = SqliteStore::open_in_memory().unwrap();
        let manifest = minted_manifest(44, "ws://cache/artifact");
        let token = manifest.token;
        let version_at_mint = manifest.version;
        store
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(4),
            })
            .await
            .unwrap();

        let first = store.manifest(token).await.unwrap().unwrap();
        assert_eq!(first.version(), version_at_mint);

        // A lifecycle write must be visible on the very next read — the
        // cache is an accelerator, never a staleness source (13 §8).
        store
            .append(AppendIntent::Mutate {
                token,
                change: waggle_core::Change::Revoked,
                expected_version: Some(version_at_mint),
                at: waggle_core::Timestamp::from_unix_ms(9),
            })
            .await
            .unwrap();
        let after = store.manifest(token).await.unwrap().unwrap();
        assert!(
            after.manifest.revoked_at.is_some(),
            "post-commit read sees the revocation"
        );
        assert_eq!(after.version(), version_at_mint + 1);
    });
}
