//! CP-3 gates: the reconstruct guarantees R-1..R-3 (design doc `04 §6`) as
//! property tests, and the 1M-event funnel-fold measurement (13 §6 budget:
//! < 10 ms release-mode; asserted here with generous debug-mode headroom so
//! CI catches regressions in *shape* — the precise criterion baseline lands
//! with the CP-9 bench harness).

use proptest::prelude::*;
use waggle_core::{
    mint, reconstruct, ActorClass, CanonicalUrl, Channel, Event, EventLog, InternTables, LogRecord,
    MintOptions, MintSpec, ResolverContext, Seq, Sharer, Stage, Timestamp, Token,
};

fn seeded_entropy(seed: u32) -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = seed | 1;
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

/// A small synthetic world: `n_tokens` minted tokens, each with a run of
/// events and one revocation on the last token.
fn world(n_tokens: u32, events_per_token: u32) -> Vec<LogRecord> {
    let mut records = Vec::new();
    let stages = [Stage::resolve(), Stage::run(), Stage::repeat()];
    for t in 0..n_tokens {
        let mut entropy = seeded_entropy(t.wrapping_mul(0x9E37_79B9) | 1);
        let m = mint(
            MintSpec::new(
                CanonicalUrl::new("ws://swarm/artifact").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(u64::from(t)),
        )
        .unwrap();
        let token = m.token;
        records.push(LogRecord::Minted {
            manifest: Box::new(m),
        });
        for i in 0..events_per_token {
            records.push(LogRecord::Event(Event {
                token,
                stage: stages[(i as usize) % stages.len()].clone(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                at: Timestamp::from_unix_ms(u64::from(i)),
                seq: Seq(i + 1),
                variant: Some(u8::try_from(i % 3).unwrap()),
            }));
        }
    }
    records
}

fn serialized(records: Vec<LogRecord>) -> String {
    serde_json::to_string(&reconstruct(records)).expect("worldstate serializes")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// R-1 — determinism under interleaving: any shuffle of the record
    /// stream reconstructs to a byte-identical serialized world.
    #[test]
    fn r1_reconstruct_is_order_insensitive(seed: u64) {
        let records = world(5, 8);
        let baseline = serialized(records.clone());

        // Fisher–Yates with a seeded xorshift — deterministic per case.
        let mut shuffled = records;
        let mut state = seed | 1;
        let mut rnd = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for i in (1..shuffled.len()).rev() {
            #[allow(clippy::cast_possible_truncation)] // modulo bounds it
            let j = (rnd() as usize) % (i + 1);
            shuffled.swap(i, j);
        }
        prop_assert_eq!(serialized(shuffled), baseline);
    }

    /// R-3 — duplicate immunity: log ∪ duplicates ≡ log.
    #[test]
    fn r3_duplicates_change_nothing(dup_count in 1usize..4) {
        let records = world(4, 6);
        let baseline = serialized(records.clone());
        let mut with_dups = records.clone();
        for (i, rec) in records.iter().enumerate() {
            if i % 3 == 0 {
                for _ in 0..dup_count {
                    with_dups.push(rec.clone());
                }
            }
        }
        prop_assert_eq!(serialized(with_dups), baseline);
    }

    /// R-2 — snapshot equivalence: reconstruct(prefix) + replay(suffix) ≡
    /// reconstruct(all), at every split point (sampled).
    #[test]
    fn r2_snapshot_plus_suffix_equals_full(split_pct in 0u32..=100) {
        // Suffix replay assumes per-token seq order (C-3), so split the
        // *ordered* stream the way a store would.
        let mut records = world(3, 6);
        records.sort_by_key(|r| (r.token(), r.seq().0));
        let split = (records.len() * split_pct as usize) / 100;
        let (prefix, suffix) = records.split_at(split);

        let full = serialized(records.clone());
        let snap = reconstruct(prefix.to_vec());
        let resumed = waggle_core::apply_suffix(snap, suffix.to_vec());
        prop_assert_eq!(serde_json::to_string(&resumed).unwrap(), full);
    }
}

/// The 1M-event funnel fold over the `SoA` log — the shape check for 13 §6's
/// budget (< 10 ms release). Debug-mode ceiling is generous; the point is
/// catching accidental O(n·m) or allocation-per-row regressions in CI.
#[test]
fn fold_funnel_1m_shape() {
    let mut tables = InternTables::default();
    let mut log = EventLog::default();
    let tokens: Vec<Token> = (0..100u32)
        .map(|i| {
            // (i|1) would collide adjacent seeds; a multiplied odd seed
            // keeps every stream distinct.
            let mut e = seeded_entropy((i + 1).wrapping_mul(0x9E37_79B9) | 1);
            Token::generate(8, &mut e).unwrap()
        })
        .collect();
    let distinct: std::collections::BTreeSet<_> = tokens.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        tokens.len(),
        "fixture tokens must be distinct"
    );
    let stages = [Stage::resolve(), Stage::run(), Stage::repeat()];
    let actor = ActorClass::from_context(&ResolverContext::anonymous_agent());
    for i in 0..1_000_000u32 {
        log.push(
            &Event {
                token: tokens[(i % 100) as usize],
                stage: stages[(i % 3) as usize].clone(),
                actor,
                at: Timestamp::from_unix_ms(u64::from(i)),
                seq: Seq(i),
                variant: None,
            },
            &mut tables,
        );
    }
    assert_eq!(log.len(), 1_000_000);

    let target = tables.token_id(tokens[7]).unwrap();
    let start = std::time::Instant::now();
    let counts = log.stage_counts(target, &tables);
    let elapsed = start.elapsed();
    let total: u64 = counts.iter().map(|(_, c)| c).sum();
    assert_eq!(total, 10_000, "100 tokens × 1M events ⇒ 10k rows each");
    assert!(
        elapsed.as_millis() < 250,
        "1M-row scan took {elapsed:?} — shape regression (budget is <10ms release)"
    );
    println!("fold_funnel_1m: {elapsed:?} (debug mode)");
}
