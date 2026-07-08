//! The crash-point matrix (design docs `15 §5.1`, 14 CP-5 —
//! `it_retry_storm`-class): SIGKILL a writer mid-burst, at varied
//! points, across repeated rounds on ONE store — then hold recovery to
//! the contract: every ACKED append survives (C-1), the sequence is
//! dense (C-3), views equal the fold (R-4), and the store keeps
//! accepting writes after each kill.
//!
//! Mechanism: this test re-invokes its own binary with `CRASH_CHILD=1`
//! to run the writer workload in a separate process the parent can kill.

#![cfg(unix)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use waggle_store::{AppendIntent, AppendStore, Appended, MintNonce, ReadStore};
use waggle_store_sqlite::SqliteStore;

fn mint_into(store: &SqliteStore, nonce: u64) -> waggle_core::Token {
    let mut entropy = move |b: &mut [u8]| {
        for (i, x) in b.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *x = (nonce as u8).wrapping_mul(97).wrapping_add(i as u8);
            }
        }
        Ok(())
    };
    let m = waggle_core::mint(
        waggle_core::MintSpec::new(
            waggle_core::CanonicalUrl::new("ws://crash/artifact").unwrap(),
            waggle_core::Sharer::new("lead").unwrap(),
            waggle_core::Channel::subagent_general(),
        ),
        &waggle_core::MintOptions::default(),
        &mut entropy,
        waggle_core::Timestamp::from_unix_ms(1),
    )
    .unwrap();
    let token = m.token;
    pollster::block_on(store.append(AppendIntent::Mint {
        manifest: Box::new(m),
        nonce: MintNonce(nonce),
    }))
    .unwrap();
    token
}

/// The child workload: append events as fast as possible, printing each
/// ACKED seq. The parent kills us whenever it pleases.
#[test]
fn crash_child_workload() {
    let Ok(db) = std::env::var("CRASH_CHILD_DB") else {
        return; // not a child invocation — nothing to do
    };
    let token = waggle_core::Token::parse(&std::env::var("CRASH_CHILD_TOKEN").unwrap()).unwrap();
    let store = SqliteStore::open(db.as_ref()).unwrap();
    let actor =
        waggle_core::ActorClass::from_context(&waggle_core::ResolverContext::anonymous_agent());
    for _ in 0..100_000 {
        let receipt = pollster::block_on(store.append(AppendIntent::Event {
            token,
            stage: waggle_core::Stage::run(),
            actor,
            variant: None,
            at: waggle_core::Timestamp::from_unix_ms(2),
        }))
        .unwrap();
        if let Appended::Event { seq } = receipt {
            println!("ACK {}", seq.0); // stdout is line-buffered per println
        }
    }
}

#[test]
fn killed_writers_never_lose_an_acked_write() {
    let dir = std::env::temp_dir().join(format!("waggle-crash-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crash.db");

    let token = {
        let store = SqliteStore::open(&db).unwrap();
        mint_into(&store, 1)
    };

    // Varied kill points: from "barely started" to "mid-flood".
    let delays_ms = [10u64, 120, 35, 80, 15, 60, 100, 25];
    let mut max_acked = 0u32;
    for (round, delay) in delays_ms.iter().enumerate() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["crash_child_workload", "--exact", "--nocapture"])
            .env("CRASH_CHILD_DB", &db)
            .env("CRASH_CHILD_TOKEN", token.as_str())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(*delay));
        let _ = Command::new("kill")
            .args(["-KILL", &child.id().to_string()])
            .status();
        let stdout = child.stdout.take().unwrap();
        let _ = child.wait();

        // Every seq the child managed to print was ACKED before the kill.
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(seq) = line
                .strip_prefix("ACK ")
                .and_then(|s| s.parse::<u32>().ok())
            {
                max_acked = max_acked.max(seq);
            }
        }

        // Recovery: reopen and hold the contract.
        let store = SqliteStore::open(&db).unwrap();
        let records = pollster::block_on(store.scan_token(token, waggle_core::Seq(0))).unwrap();
        let seqs: Vec<u32> = records.iter().map(|r| r.seq().0).collect();
        let max_stored = *seqs.iter().max().unwrap_or(&0);
        assert!(
            max_stored >= max_acked,
            "round {round}: acked seq {max_acked} lost after SIGKILL (stored max {max_stored}) — C-1 violated"
        );
        let expected: Vec<u32> = (0..=max_stored).collect();
        assert_eq!(
            seqs, expected,
            "round {round}: sequence not dense after crash — C-3 violated"
        );

        // R-4 after crash: materialized funnel ≡ fold over the log.
        let world = waggle_core::reconstruct(records);
        let funnel = pollster::block_on(store.funnel(token)).unwrap();
        // A kill can land before ANY event acked (only the mint) — the
        // fold then has no funnel entry, and the store must agree: empty.
        assert_eq!(
            funnel,
            world.funnels.get(&token).cloned().unwrap_or_default(),
            "round {round}: views diverged from the log after crash"
        );

        // And the store still accepts writes (no lingering lock/journal).
        let receipt = pollster::block_on(store.append(AppendIntent::Event {
            token,
            stage: waggle_core::Stage::repeat(),
            actor: waggle_core::ActorClass::from_context(
                &waggle_core::ResolverContext::anonymous_agent(),
            ),
            variant: None,
            at: waggle_core::Timestamp::from_unix_ms(3),
        }))
        .unwrap();
        assert!(matches!(receipt, Appended::Event { .. }));
    }
    assert!(
        max_acked > 0,
        "the matrix must actually exercise acked writes"
    );
    std::fs::remove_dir_all(&dir).ok();
}
