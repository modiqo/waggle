//! The JSONL backend is a waggle backend (design doc `07 §5`) — and the
//! wire format is its native tongue: reopen IS a replay.

use waggle_core::Stage;
use waggle_store::conformance::{run_all, Harness};
use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
use waggle_store_fs_jsonl::FsJsonlStore;

fn temp_journal(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-jsonl-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("log.jsonl")
}

#[test]
fn fs_jsonl_backend_passes_conformance() {
    let base = temp_journal("conf");
    let counter = std::sync::atomic::AtomicU32::new(0);
    run_all(&Harness::new(move || {
        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        FsJsonlStore::open(&base.with_extension(format!("{n}.jsonl"))).unwrap()
    }));
}

#[test]
fn reopen_replays_the_journal() {
    let path = temp_journal("reopen");
    let token = pollster::block_on(async {
        let store = FsJsonlStore::open(&path).unwrap();
        let mut entropy = |b: &mut [u8]| {
            b.fill(55);
            Ok(())
        };
        let manifest = waggle_core::mint(
            waggle_core::MintSpec::new(
                waggle_core::CanonicalUrl::new("ws://journal/artifact").unwrap(),
                waggle_core::Sharer::new("lead").unwrap(),
                waggle_core::Channel::subagent_general(),
            ),
            &waggle_core::MintOptions::default(),
            &mut entropy,
            waggle_core::Timestamp::from_unix_ms(1),
        )
        .unwrap();
        let token = manifest.token;
        store
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(1),
            })
            .await
            .unwrap();
        for _ in 0..3 {
            store
                .append(AppendIntent::Event {
                    token,
                    stage: Stage::run(),
                    actor: waggle_core::ActorClass::from_context(
                        &waggle_core::ResolverContext::anonymous_agent(),
                    ),
                    variant: None,
                    at: waggle_core::Timestamp::from_unix_ms(2),
                })
                .await
                .unwrap();
        }
        token
    });

    pollster::block_on(async {
        let reopened = FsJsonlStore::open(&path).unwrap();
        assert!(
            reopened.manifest(token).await.unwrap().is_some(),
            "reopen IS a replay"
        );
        assert_eq!(reopened.funnel(token).await.unwrap()[&Stage::run()], 3);
        // And the journal file is human-greppable JSONL — the wire format.
        let text = std::fs::read_to_string(reopened.path()).unwrap();
        assert_eq!(text.lines().count(), 4, "one line per record");
        assert!(text.lines().all(|l| l.contains("\"record\":")));
    });
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}
