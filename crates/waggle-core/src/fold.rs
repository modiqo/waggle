//! The fold engine: folds are the **only** read model (design docs `04`,
//! `13 §4`). Every counter, view, and report is `Fold` state after a replay
//! — no side-channel state, ever. Tuple composition runs N folds in **one
//! pass** over the log; adding an analytic is a new `Fold` impl, never a
//! new scan.

use std::collections::BTreeMap;

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
