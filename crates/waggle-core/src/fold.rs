//! The fold engine: folds are the **only** read model (design docs `04`,
//! `13 §4`). Every counter, view, and report is `Fold` state after a replay
//! — no side-channel state, ever. Tuple composition runs N folds in **one
//! pass** over the log; adding an analytic is a new `Fold` impl, never a
//! new scan.

use std::collections::{BTreeMap, BTreeSet};

use crate::log::LogRecord;
use crate::manifest::AttributionManifest;
use crate::slug::Stage;
use crate::token::Token;

/// A pure accumulator over log records. `apply` performs no I/O and asks no
/// clock; unknown record kinds must be ignored (additive schema growth —
/// doc `13 §4`).
pub trait Fold {
    /// Fold one record into the accumulator.
    fn apply(&mut self, record: &LogRecord);
}

/// One-pass composition: `(A, B)` applies both. Nest tuples for more.
impl<A: Fold, B: Fold> Fold for (A, B) {
    fn apply(&mut self, record: &LogRecord) {
        self.0.apply(record);
        self.1.apply(record);
    }
}

/// Run `records` through `fold` (one pass) and hand the fold back.
pub fn replay<F: Fold>(records: impl IntoIterator<Item = LogRecord>, mut fold: F) -> F {
    for rec in &records.into_iter().collect::<Vec<_>>() {
        fold.apply(rec);
    }
    fold
}

/// Materializes manifests: `Minted` inserts; mutations apply in arrival
/// order (reconstruct orders per-token by seq, so arrival order *is* commit
/// order — doc `04 §2`). Lifecycle changes bump `version` (the CAS
/// baseline C-9); cosmetic changes don't.
#[derive(Debug, Default)]
pub struct ManifestFold {
    /// Token → latest manifest state at the folded prefix.
    pub manifests: BTreeMap<Token, AttributionManifest>,
}

impl Fold for ManifestFold {
    fn apply(&mut self, record: &LogRecord) {
        match record {
            LogRecord::Minted { manifest } => {
                // First write wins: a duplicate Minted (same token) is a
                // replayed record, not a new token (C-8's fold-side face).
                self.manifests
                    .entry(manifest.token)
                    .or_insert_with(|| (**manifest).clone());
            }
            LogRecord::Mutation {
                token, at, change, ..
            } => {
                let Some(m) = self.manifests.get_mut(token) else {
                    return;
                };
                crate::manifest::apply_change(m, change, *at);
            }
            LogRecord::Event(_) => {}
        }
    }
}

/// Counts stages per token. Commutative across tokens by construction —
/// counts don't care about cross-token order (doc `04 §2`).
#[derive(Debug, Default)]
pub struct FunnelFold {
    /// Token → stage → count.
    pub per_token: BTreeMap<Token, BTreeMap<Stage, u64>>,
}

impl Fold for FunnelFold {
    fn apply(&mut self, record: &LogRecord) {
        if let LogRecord::Event(e) = record {
            *self
                .per_token
                .entry(e.token)
                .or_default()
                .entry(e.stage.clone())
                .or_insert(0) += 1;
        }
    }
}

/// A token's judged outcome, derived from the `accepted`/`rejected`
/// stage counts (doc `19 §4.1`): the verdict IS the stage, so the log
/// stays payload-free (I-1) and this derivation is a pure function of
/// counts — order-free, replay-stable (R-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// No judge has recorded a verdict.
    Pending,
    /// At least one `accepted`, no `rejected`.
    Accepted,
    /// At least one `rejected`, no `accepted`.
    Rejected,
    /// Both were recorded — surfaced honestly for the orchestrator to
    /// resolve (re-judge, or supersede and re-delegate).
    Contested,
}

/// Derive the [`Outcome`] from a token's stage counts (the shape
/// [`FunnelFold`] and every store's `funnel` view produce).
#[must_use]
pub fn outcome_of(counts: &BTreeMap<Stage, u64>) -> Outcome {
    let n = |s: &Stage| counts.get(s).copied().unwrap_or(0);
    match (n(&Stage::accepted()) > 0, n(&Stage::rejected()) > 0) {
        (false, false) => Outcome::Pending,
        (true, false) => Outcome::Accepted,
        (false, true) => Outcome::Rejected,
        (true, true) => Outcome::Contested,
    }
}

/// Accumulates contract region touches per token: the OR of every
/// event's `regions` bitmask (doc `19 §4.2`). Commutative and
/// duplicate-tolerant by construction — OR is both — so R-1/R-3 hold
/// without ceremony. `coverage` evaluates this against the manifest's
/// declared [`crate::Contract`].
#[derive(Debug, Default)]
pub struct RegionTouchFold {
    /// Token → OR of region-touch bits observed so far.
    pub per_token: BTreeMap<Token, u8>,
}

impl Fold for RegionTouchFold {
    fn apply(&mut self, record: &LogRecord) {
        if let LogRecord::Event(e) = record {
            if let Some(bits) = e.regions {
                *self.per_token.entry(e.token).or_insert(0) |= bits;
            }
        }
    }
}

/// Accumulates the set of tree-node file ordinals a token's reads have
/// touched: the UNION of every event's [`crate::Event::entry`]. Union is
/// commutative and idempotent — like the OR in [`RegionTouchFold`] — so
/// R-1/R-3 (order- and duplicate-tolerance) and C-8 hold for free. Tree
/// coverage evaluates `|touched ∩ [0, node.files)|` against each node's
/// signed directory index for a true per-file receipt.
#[derive(Debug, Default)]
pub struct EntryTouchFold {
    /// Token → the set of `DirIndex` file ordinals its reads have reached.
    pub per_token: BTreeMap<Token, BTreeSet<u32>>,
}

impl Fold for EntryTouchFold {
    fn apply(&mut self, record: &LogRecord) {
        if let LogRecord::Event(e) = record {
            if let Some(ordinal) = e.entry {
                self.per_token.entry(e.token).or_default().insert(ordinal);
            }
        }
    }
}

/// The delegation forest: parent → children, from `Minted` records
/// (doc `06 §4` — coordination trace, attribution roll-up, cascade path).
#[derive(Debug, Default)]
pub struct LineageFold {
    /// Parent token → children in mint order.
    pub children: BTreeMap<Token, Vec<Token>>,
}

impl Fold for LineageFold {
    fn apply(&mut self, record: &LogRecord) {
        if let LogRecord::Minted { manifest } = record {
            if let Some(parent) = manifest.parent {
                let siblings = self.children.entry(parent).or_default();
                if !siblings.contains(&manifest.token) {
                    siblings.push(manifest.token);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ActorClass, Event, Seq};
    use crate::log::Change;
    use crate::{CanonicalUrl, Channel, MintOptions, MintSpec, ResolverContext, Sharer, Timestamp};

    fn minted(tag: u8, parent: Option<Token>) -> AttributionManifest {
        let mut entropy = move |buf: &mut [u8]| {
            buf.fill(tag);
            Ok(())
        };
        let mut spec = MintSpec::new(
            CanonicalUrl::new("ws://a/b").unwrap(),
            Sharer::new("lead").unwrap(),
            Channel::subagent_general(),
        );
        if let Some(p) = parent {
            spec = spec.child_of(p);
        }
        crate::mint(
            spec,
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1),
        )
        .unwrap()
    }

    fn event(token: Token, stage: Stage, seq: u32) -> LogRecord {
        LogRecord::Event(Event {
            token,
            stage,
            actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
            at: Timestamp::from_unix_ms(2),
            seq: Seq(seq),
            variant: None,
            regions: None,
            entry: None,
        })
    }

    #[test]
    fn one_pass_multi_fold_agrees_with_separate_passes() {
        // CP-3 gate: single scan, N folds (doc 13 §3).
        let parent = minted(1, None);
        let child = minted(2, Some(parent.token));
        let records = vec![
            LogRecord::Minted {
                manifest: Box::new(parent.clone()),
            },
            LogRecord::Minted {
                manifest: Box::new(child.clone()),
            },
            event(parent.token, Stage::resolve(), 1),
            event(parent.token, Stage::run(), 2),
            LogRecord::Mutation {
                token: parent.token,
                at: Timestamp::from_unix_ms(3),
                seq: Seq(3),
                change: Change::Revoked,
            },
        ];

        let (manifests, (funnel, lineage)) = replay(
            records.clone(),
            (
                ManifestFold::default(),
                (FunnelFold::default(), LineageFold::default()),
            ),
        );
        let solo_m = replay(records.clone(), ManifestFold::default());
        let solo_f = replay(records.clone(), FunnelFold::default());
        let solo_l = replay(records, LineageFold::default());

        assert_eq!(manifests.manifests, solo_m.manifests);
        assert_eq!(funnel.per_token, solo_f.per_token);
        assert_eq!(lineage.children, solo_l.children);

        // And the folds say what happened.
        assert!(manifests.manifests[&parent.token].revoked_at.is_some());
        assert_eq!(
            manifests.manifests[&parent.token].version, 2,
            "lifecycle bumps version"
        );
        assert_eq!(funnel.per_token[&parent.token][&Stage::run()], 1);
        assert_eq!(lineage.children[&parent.token], vec![child.token]);
    }

    #[test]
    fn cosmetic_changes_do_not_bump_version() {
        let m = minted(5, None);
        let records = vec![
            LogRecord::Minted {
                manifest: Box::new(m.clone()),
            },
            LogRecord::Mutation {
                token: m.token,
                at: Timestamp::from_unix_ms(2),
                seq: Seq(1),
                change: Change::LabelSet {
                    key: "campaign".into(),
                    value: "q3".into(),
                },
            },
        ];
        let folded = replay(records, ManifestFold::default());
        let out = &folded.manifests[&m.token];
        assert_eq!(out.version, 1);
        assert_eq!(out.labels["campaign"], "q3");
    }

    #[test]
    fn outcome_derivation_is_a_pure_function_of_counts() {
        let m = minted(7, None);
        let judged = |stages: &[Stage]| {
            let records: Vec<LogRecord> = stages
                .iter()
                .enumerate()
                .map(|(i, s)| event(m.token, s.clone(), u32::try_from(i).unwrap() + 1))
                .collect();
            let funnel = replay(records, FunnelFold::default());
            outcome_of(funnel.per_token.get(&m.token).unwrap_or(&BTreeMap::new()))
        };
        assert_eq!(judged(&[]), Outcome::Pending);
        assert_eq!(judged(&[Stage::resolve(), Stage::run()]), Outcome::Pending);
        assert_eq!(judged(&[Stage::accepted()]), Outcome::Accepted);
        assert_eq!(judged(&[Stage::rejected()]), Outcome::Rejected);
        assert_eq!(
            judged(&[Stage::accepted(), Stage::rejected()]),
            Outcome::Contested,
            "both verdicts surface honestly, never a silent overwrite"
        );
    }

    #[test]
    fn region_touches_or_together_shuffle_and_duplicate_immune() {
        let m = minted(8, None);
        let touch = |bits: u8, seq: u32| {
            LogRecord::Event(Event {
                token: m.token,
                stage: Stage::read(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                at: Timestamp::from_unix_ms(2),
                seq: Seq(seq),
                variant: None,
                regions: Some(bits),
                entry: None,
            })
        };
        let records = vec![
            touch(0b001, 1),
            event(m.token, Stage::read(), 2), // no regions: contract-free access
            touch(0b100, 3),
            touch(0b001, 1), // duplicate record — OR absorbs it (R-3)
        ];
        let mut shuffled = records.clone();
        shuffled.reverse();
        let a = replay(records, RegionTouchFold::default());
        let b = replay(shuffled, RegionTouchFold::default());
        assert_eq!(a.per_token[&m.token], 0b101);
        assert_eq!(a.per_token, b.per_token, "OR is commutative — R-1");
    }

    #[test]
    fn entry_touches_union_together_shuffle_and_duplicate_immune() {
        let m = minted(9, None);
        let touch = |ordinal: u32, seq: u32| {
            LogRecord::Event(Event {
                token: m.token,
                stage: Stage::read(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                at: Timestamp::from_unix_ms(2),
                seq: Seq(seq),
                variant: None,
                regions: None,
                entry: Some(ordinal),
            })
        };
        let records = vec![
            touch(2, 1),
            event(m.token, Stage::read(), 2), // no entry: whole-node access
            touch(0, 3),
            touch(2, 1), // duplicate record — the set absorbs it (R-3)
        ];
        let mut shuffled = records.clone();
        shuffled.reverse();
        let a = replay(records, EntryTouchFold::default());
        let b = replay(shuffled, EntryTouchFold::default());
        assert_eq!(a.per_token[&m.token], BTreeSet::from([0, 2]));
        assert_eq!(a.per_token, b.per_token, "union is commutative — R-1");
    }

    #[test]
    fn mutation_before_mint_is_ignored_not_a_panic() {
        // Hostile/misordered input: folds are total (reconstruct orders
        // per-token, but folds must survive anything).
        let m = minted(6, None);
        let records = vec![LogRecord::Mutation {
            token: m.token,
            at: Timestamp::from_unix_ms(1),
            seq: Seq(1),
            change: Change::Revoked,
        }];
        let folded = replay(records, ManifestFold::default());
        assert!(folded.manifests.is_empty());
    }
}
