//! Tier 1b — reconstruction determinism (design doc `22 §2.2`).
//!
//! Backs the paper's "replays identically on any machine" claim as a
//! *property*, not a vibe: a log is reconstructed from many shuffled and
//! duplicated orderings, and every resulting [`waggle_core::WorldState`]
//! must serialize byte-for-byte identically (C-8 replay tolerance). Any
//! divergence is a hard failure that fails the benchmark gate.

use std::time::Instant;
use waggle_core::{
    mint, reconstruct, ActorClass, CanonicalUrl, Channel, Event, LogRecord, MintOptions, MintSpec,
    ResolverContext, Seq, Sharer, Stage, Timestamp,
};

/// Outcome of the determinism check.
pub(crate) struct Report {
    /// Distinct tokens minted.
    pub tokens: usize,
    /// Total events appended.
    pub events: usize,
    /// Shuffled + duplicated orderings reconstructed.
    pub permutations: usize,
    /// Whether every ordering serialized identically.
    pub all_identical: bool,
    /// Time for one clean reconstruction, microseconds.
    pub fold_micros: u128,
}

/// A tiny deterministic PRNG (LCG). No external dependency; seed-stable so
/// the check reproduces exactly on any machine.
struct Lcg(u64);

impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() >> 33) as usize % n
    }
}

fn stage_cycle(i: usize) -> Stage {
    match i % 3 {
        0 => Stage::resolve(),
        1 => Stage::read(),
        _ => Stage::run(),
    }
}

/// Build a log of `k` tokens, each followed by `events_per` funnel events.
fn build_log(k: usize, events_per: usize) -> Vec<LogRecord> {
    // A persistent counter is entropy enough for distinct tokens and keeps
    // the harness sans-I/O and reproducible (mirrors the core's test source).
    let mut n: u8 = 0;
    let mut ent = |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            n = n.wrapping_add(13);
            *b = n;
        }
        Ok(())
    };

    let mut records = Vec::with_capacity(k * (1 + events_per));
    for _ in 0..k {
        let spec = MintSpec::new(
            CanonicalUrl::new("ws://bench/artifact.md").unwrap(),
            Sharer::new("bench").unwrap(),
            Channel::subagent_general(),
        );
        let m = mint(
            spec,
            &MintOptions::default(),
            &mut ent,
            Timestamp::from_unix_ms(0),
        )
        .unwrap();
        let token = m.token;
        records.push(LogRecord::Minted {
            manifest: Box::new(m),
        });
        for e in 0..events_per {
            records.push(LogRecord::Event(Event {
                token,
                stage: stage_cycle(e),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                at: Timestamp::from_unix_ms((e + 1) as u64),
                seq: Seq((e + 1) as u32),
                variant: None,
                regions: None,
            }));
        }
    }
    records
}

fn serialized(records: &[LogRecord]) -> String {
    let ws = reconstruct(records.iter().cloned());
    serde_json::to_string(&ws).expect("WorldState serializes")
}

/// Reconstruct a baseline log, then `permutations` shuffled + duplicated
/// orderings, asserting every reconstruction serializes identically.
pub(crate) fn run(k: usize, events_per: usize, permutations: usize, seed: u64) -> Report {
    let base_records = build_log(k, events_per);
    let events = k * events_per;

    let t0 = Instant::now();
    let baseline = serialized(&base_records);
    let fold_micros = t0.elapsed().as_micros();

    let mut rng = Lcg(seed);
    let mut all_identical = true;
    for _ in 0..permutations {
        let mut recs = base_records.clone();
        // Fisher–Yates shuffle.
        for i in (1..recs.len()).rev() {
            let j = rng.below(i + 1);
            recs.swap(i, j);
        }
        // Duplicate a random handful — replays must dedup to the same state.
        let dup = 1 + rng.below(recs.len());
        for _ in 0..dup {
            let idx = rng.below(recs.len());
            let clone = recs[idx].clone();
            recs.push(clone);
        }
        if serialized(&recs) != baseline {
            all_identical = false;
        }
    }

    Report {
        tokens: k,
        events,
        permutations,
        all_identical,
        fold_micros,
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn reconstruction_is_order_and_duplication_stable() {
        let r = run(16, 4, 24, 0xB0BA);
        assert!(r.all_identical, "reconstruct() was not order/dup-invariant");
        assert_eq!(r.tokens, 16);
        assert_eq!(r.events, 64);
    }
}
