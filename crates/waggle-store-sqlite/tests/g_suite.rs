//! The G-gap verification suite (design doc `15 §4–§5`): the concurrency
//! gaps found in the adversarial scenario review, each pinned by a test
//! against the real store. G-3/G-4/G-5 live in core and the conformance
//! suite; G-7/G-8 are edge-tier (CP-10). Here: the multi-writer physics.

use std::sync::Arc;
use std::thread;

use waggle_core::Stage;
use waggle_store::{AppendIntent, AppendStore, Appended, MintNonce, ReadStore};
use waggle_store_sqlite::SqliteStore;

fn minted_into(store: &SqliteStore, nonce: u64) -> waggle_core::Token {
    let mut entropy = move |b: &mut [u8]| {
        for (i, x) in b.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *x = (nonce as u8).wrapping_mul(53).wrapping_add(i as u8);
            }
        }
        Ok(())
    };
    let manifest = waggle_core::mint(
        waggle_core::MintSpec::new(
            waggle_core::CanonicalUrl::new("ws://g/artifact").unwrap(),
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
        nonce: MintNonce(nonce),
    }))
    .unwrap();
    token
}

fn event(token: waggle_core::Token) -> AppendIntent {
    AppendIntent::Event {
        token,
        stage: Stage::run(),
        actor: waggle_core::ActorClass::from_context(
            &waggle_core::ResolverContext::anonymous_agent(),
        ),
        variant: None,
        at: waggle_core::Timestamp::from_unix_ms(2),
    }
}

/// G-1/G-2: eight concurrent writers, one store — every ack is real, the
/// per-token sequence comes out dense and gapless (the single commit
/// point orders; nobody's write is lost or torn).
#[test]
fn g1_g2_concurrent_writers_dense_seqs() {
    const WRITERS: usize = 8;
    const PER_WRITER: usize = 50;
    let dir = std::env::temp_dir().join(format!("waggle-g12-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(SqliteStore::open(&dir.join("g.db")).unwrap());
    let token = minted_into(&store, 1);

    let mut handles = Vec::new();
    for _ in 0..WRITERS {
        let store = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            let mut seqs = Vec::with_capacity(PER_WRITER);
            for _ in 0..PER_WRITER {
                let Appended::Event { seq } =
                    pollster::block_on(store.append(event(token))).unwrap()
                else {
                    panic!("event receipt expected")
                };
                seqs.push(seq.0);
            }
            seqs
        }));
    }
    let mut all: Vec<u32> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    all.sort_unstable();
    let expected: Vec<u32> = (1..=u32::try_from(WRITERS * PER_WRITER).unwrap()).collect();
    assert_eq!(
        all, expected,
        "dense, gapless, no duplicate seq under 8-way contention"
    );

    let funnel = pollster::block_on(store.funnel(token)).unwrap();
    assert_eq!(funnel[&Stage::run()], (WRITERS * PER_WRITER) as u64);
    std::fs::remove_dir_all(&dir).ok();
}

/// G-2 (reader side): a reader hammering manifest + funnel while writers
/// flood never sees a decode error or a torn state — WAL snapshot reads.
#[test]
fn g2_readers_never_tear_under_write_load() {
    let dir = std::env::temp_dir().join(format!("waggle-g2r-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(SqliteStore::open(&dir.join("g.db")).unwrap());
    let token = minted_into(&store, 2);

    let writer = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            for _ in 0..300 {
                pollster::block_on(store.append(event(token))).unwrap();
            }
        })
    };
    let reader = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            let mut last = 0u64;
            for _ in 0..300 {
                let view = pollster::block_on(store.manifest(token)).unwrap();
                assert!(
                    view.is_some(),
                    "the manifest never flickers out of existence"
                );
                let funnel = pollster::block_on(store.funnel(token)).unwrap();
                let count = funnel.get(&Stage::run()).copied().unwrap_or(0);
                assert!(count >= last, "counters never run backwards");
                last = count;
            }
        })
    };
    writer.join().unwrap();
    reader.join().unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

/// G-6 (analytics flood): a 10k-event burst all lands, and a read taken
/// immediately afterward reflects every event — ingestion cannot silently
/// shed load.
#[test]
fn g6_analytics_flood_all_lands() {
    let store = SqliteStore::open_in_memory().unwrap();
    let token = minted_into(&store, 3);
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        pollster::block_on(store.append(event(token))).unwrap();
    }
    let elapsed = start.elapsed();
    let funnel = pollster::block_on(store.funnel(token)).unwrap();
    assert_eq!(
        funnel[&Stage::run()],
        10_000,
        "every event of the burst landed"
    );
    println!(
        "g6 flood: 10k events in {elapsed:?} ({:.0}/s)",
        10_000.0 / elapsed.as_secs_f64()
    );
}
