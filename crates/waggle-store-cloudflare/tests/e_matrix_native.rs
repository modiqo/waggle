//! CP-10e, the native rows (design doc `08 §9`): the edge engine
//! certified before any worker exists. E1-native runs THE conformance
//! suite; E6-native is the differential oracle — the same operations
//! against the edge engine and `SQLite` must produce byte-identical
//! reconstructions and answers, because the local tier IS the
//! specification.

use waggle_core::{reconstruct, Stage};
use waggle_store::conformance::{run_all, Harness};
use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
use waggle_store_cloudflare::{EdgeStore, MemoryEdgeStorage};

/// E1 (native): the edge engine is a waggle backend.
#[test]
fn e1_native_conformance() {
    run_all(&Harness::new(|| {
        EdgeStore::new(MemoryEdgeStorage::default())
    }));
}

/// E6 (native): the differential oracle over a 200-step scripted-random
/// op sequence.
#[test]
#[allow(clippy::too_many_lines)] // the oracle script IS the test
fn e6_native_edge_equals_local() {
    pollster::block_on(async {
        let edge = EdgeStore::new(MemoryEdgeStorage::default());
        let local = waggle_store_sqlite::SqliteStore::open_in_memory().unwrap();

        let mut rng_state = 0xE6E6_u32;
        let mut rnd = move || {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 17;
            rng_state ^= rng_state << 5;
            rng_state
        };
        let mut tokens: Vec<waggle_core::Token> = Vec::new();
        let stages = [
            Stage::resolve(),
            Stage::read(),
            Stage::run(),
            Stage::repeat(),
        ];

        for step in 0..200u32 {
            let roll = rnd() % 100;
            if roll < 25 || tokens.is_empty() {
                let parent = (roll < 8 && !tokens.is_empty())
                    .then(|| tokens[(rnd() as usize) % tokens.len()]);
                let mut entropy = {
                    let mut s = step.wrapping_mul(0x9E37_79B9) | 1;
                    move |b: &mut [u8]| {
                        for x in b.iter_mut() {
                            s ^= s << 13;
                            s ^= s >> 17;
                            s ^= s << 5;
                            *x = (s & 0xFF) as u8;
                        }
                        Ok(())
                    }
                };
                let mut spec = waggle_core::MintSpec::new(
                    waggle_core::CanonicalUrl::new(&format!("ws://oracle/{}", step % 7)).unwrap(),
                    waggle_core::Sharer::new("oracle").unwrap(),
                    waggle_core::Channel::subagent_general(),
                );
                if let Some(p) = parent {
                    spec = spec.child_of(p);
                }
                let manifest = waggle_core::mint(
                    spec,
                    &waggle_core::MintOptions::default(),
                    &mut entropy,
                    waggle_core::Timestamp::from_unix_ms(u64::from(step)),
                )
                .unwrap();
                let mk = |m: waggle_core::AttributionManifest| AppendIntent::Mint {
                    manifest: Box::new(m),
                    nonce: MintNonce(u64::from(step)),
                };
                let a = edge.append(mk(manifest.clone())).await;
                let b = local.append(mk(manifest.clone())).await;
                assert_eq!(a.is_ok(), b.is_ok(), "step {step}: mint diverged");
                if a.is_ok() {
                    tokens.push(manifest.token);
                }
            } else if roll < 75 {
                let token = tokens[(rnd() as usize) % tokens.len()];
                let stage = stages[(rnd() as usize) % stages.len()].clone();
                let mk = || AppendIntent::Event {
                    token,
                    stage: stage.clone(),
                    actor: waggle_core::ActorClass::from_context(
                        &waggle_core::ResolverContext::anonymous_agent(),
                    ),
                    variant: None,
                    regions: None,
                    entry: None,
                    at: waggle_core::Timestamp::from_unix_ms(u64::from(step)),
                };
                let a = edge.append(mk()).await;
                let b = local.append(mk()).await;
                assert_eq!(a.is_ok(), b.is_ok(), "step {step}: event diverged");
            } else {
                let token = tokens[(rnd() as usize) % tokens.len()];
                let version = edge
                    .manifest(token)
                    .await
                    .unwrap()
                    .map_or(1, |v| v.version());
                let change = match rnd() % 3 {
                    0 => waggle_core::Change::Revoked,
                    1 => waggle_core::Change::ExpirySet {
                        expires_at: Some(waggle_core::Timestamp::from_unix_ms(9_999_999)),
                    },
                    _ => waggle_core::Change::LabelSet {
                        key: "step".into(),
                        value: step.to_string(),
                    },
                };
                let mk = || AppendIntent::Mutate {
                    token,
                    change: change.clone(),
                    expected_version: change.is_lifecycle().then_some(version),
                    at: waggle_core::Timestamp::from_unix_ms(u64::from(step)),
                };
                let a = edge.append(mk()).await;
                let b = local.append(mk()).await;
                assert_eq!(
                    a.is_ok(),
                    b.is_ok(),
                    "step {step}: mutate diverged ({a:?} vs {b:?})"
                );
            }
        }

        // THE ORACLE: byte-identical worlds.
        let edge_world =
            serde_json::to_string(&reconstruct(edge.scan_all().await.unwrap())).unwrap();
        let local_world =
            serde_json::to_string(&reconstruct(local.scan_all().await.unwrap())).unwrap();
        assert_eq!(edge_world, local_world, "the worlds diverged");

        for token in &tokens {
            let em = edge
                .manifest(*token)
                .await
                .unwrap()
                .map(|v| (*v.manifest).clone());
            let lm = local
                .manifest(*token)
                .await
                .unwrap()
                .map(|v| (*v.manifest).clone());
            assert_eq!(em, lm, "manifest diverged for {token}");
            assert_eq!(
                edge.funnel(*token).await.unwrap(),
                local.funnel(*token).await.unwrap(),
                "funnel diverged for {token}"
            );
            assert_eq!(
                edge.children(*token).await.unwrap(),
                local.children(*token).await.unwrap(),
                "children diverged for {token}"
            );
        }

        // E10 (native half): stored events carry ONLY the fixed fields.
        for rec in edge.scan_all().await.unwrap() {
            if let waggle_core::LogRecord::Event(e) = rec {
                let v = serde_json::to_value(&e).unwrap();
                for k in v.as_object().unwrap().keys() {
                    assert!(
                        ["token", "stage", "actor", "at", "seq", "variant"].contains(&k.as_str()),
                        "unexpected event field `{k}` — I-1 breach"
                    );
                }
            }
        }
    });
}

/// E4 (native rehearsal): `SQLite` → edge migration over the wire format,
/// every line sent twice (the injected retry).
#[test]
fn e4_native_replay_migration_rehearsal() {
    pollster::block_on(async {
        let local = waggle_store_sqlite::SqliteStore::open_in_memory().unwrap();
        let mut entropy = |b: &mut [u8]| {
            b.fill(77);
            Ok(())
        };
        let manifest = waggle_core::mint(
            waggle_core::MintSpec::new(
                waggle_core::CanonicalUrl::new("ws://migrate/edge").unwrap(),
                waggle_core::Sharer::new("lead").unwrap(),
                waggle_core::Channel::subagent_general(),
            ),
            &waggle_core::MintOptions::default(),
            &mut entropy,
            waggle_core::Timestamp::from_unix_ms(1),
        )
        .unwrap();
        let token = manifest.token;
        local
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(1),
            })
            .await
            .unwrap();
        for stage in [Stage::resolve(), Stage::run()] {
            local
                .append(AppendIntent::Event {
                    token,
                    stage,
                    actor: waggle_core::ActorClass::from_context(
                        &waggle_core::ResolverContext::anonymous_agent(),
                    ),
                    variant: None,
                    regions: None,
                    entry: None,
                    at: waggle_core::Timestamp::from_unix_ms(2),
                })
                .await
                .unwrap();
        }

        let edge = EdgeStore::new(MemoryEdgeStorage::default());
        let records = local.scan_all().await.unwrap();
        for rec in records.iter().chain(records.iter()) {
            edge.ingest(rec.clone()).await.unwrap();
        }
        let a = serde_json::to_string(&reconstruct(records)).unwrap();
        let b = serde_json::to_string(&reconstruct(edge.scan_all().await.unwrap())).unwrap();
        assert_eq!(a, b, "migration is a stream: destination ≡ source");
        assert_eq!(edge.funnel(token).await.unwrap()[&Stage::run()], 1);
    });
}
