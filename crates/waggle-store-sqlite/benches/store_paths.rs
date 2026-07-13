#![allow(missing_docs)] // criterion macros generate undocumented items

//! Store-path budgets (design doc `13 §6`): cache-hit resolve < 1 µs,
//! cold manifest read < 50 µs, durable event append measured honestly
//! (synchronous=FULL — the fsync IS the product).

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
use waggle_store_sqlite::SqliteStore;

fn minted_into(store: &SqliteStore) -> waggle_core::Token {
    let mut entropy = |b: &mut [u8]| {
        b.fill(29);
        Ok(())
    };
    let manifest = waggle_core::mint(
        waggle_core::MintSpec::new(
            waggle_core::CanonicalUrl::new("ws://bench/artifact").unwrap(),
            waggle_core::Sharer::new("lead").unwrap(),
            waggle_core::Channel::subagent_general(),
        ),
        &waggle_core::MintOptions::default(),
        &mut entropy,
        waggle_core::Timestamp::from_unix_ms(1),
    )
    .unwrap();
    let token = manifest.token;
    pollster::block_on(store.append(AppendIntent::Mint {
        manifest: Box::new(manifest),
        nonce: MintNonce(1),
    }))
    .unwrap();
    token
}

fn bench_reads(c: &mut Criterion) {
    let store = SqliteStore::open_in_memory().unwrap();
    let token = minted_into(&store);
    // Warm the cache once; budget: cache-hit < 1 µs.
    pollster::block_on(store.manifest(token)).unwrap();
    c.bench_function("manifest_cache_hit", |b| {
        b.iter(|| pollster::block_on(store.manifest(black_box(token))).unwrap());
    });
}

fn bench_append(c: &mut Criterion) {
    let dir = std::env::temp_dir().join(format!("waggle-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = SqliteStore::open(&dir.join("bench.db")).unwrap();
    let token = minted_into(&store);
    let actor =
        waggle_core::ActorClass::from_context(&waggle_core::ResolverContext::anonymous_agent());
    // Durable (fsync) appends — the honest number, not the cached one.
    c.bench_function("event_append_durable", |b| {
        b.iter(|| {
            pollster::block_on(store.append(AppendIntent::Event {
                token,
                stage: waggle_core::Stage::run(),
                actor,
                variant: None,
                regions: None,
                entry: None,
                at: waggle_core::Timestamp::from_unix_ms(2),
            }))
            .unwrap()
        });
    });
    std::fs::remove_dir_all(&dir).ok();
}

criterion_group!(benches, bench_reads, bench_append);
criterion_main!(benches);
