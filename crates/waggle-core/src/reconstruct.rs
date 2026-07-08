//! Reconstruct: the normative replay (design doc `04 §6`). Dedup by
//! `(token, seq)` (R-3), order per-token by seq (the weakest ordering that
//! keeps every query exact — doc `04 §2`), fold everything in one pass, and
//! serialize deterministically (R-1).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::fold::{replay, FunnelFold, LineageFold, ManifestFold};
use crate::log::LogRecord;
use crate::manifest::AttributionManifest;
use crate::slug::Stage;
use crate::token::Token;

/// The world at a log prefix: manifests, funnels, lineage. Serialization is
/// deterministic (`BTreeMap` everywhere) — R-1's "byte-identical" is
/// checked against the serialized form.
#[derive(Debug, Default, Serialize)]
pub struct WorldState {
    /// Token → manifest state.
    pub manifests: BTreeMap<Token, AttributionManifest>,
    /// Token → stage → count.
    pub funnels: BTreeMap<Token, BTreeMap<Stage, u64>>,
    /// Parent → children (mint order).
    pub lineage: BTreeMap<Token, Vec<Token>>,
}

/// Rebuild the world from records, in any arrival order, with duplicates.
///
/// Properties (tested in `tests/event_sourcing.rs`):
/// - **R-1**: any arrival order respecting nothing at all — reconstruct
///   sorts per-token by seq — yields byte-identical serialized state;
/// - **R-3**: duplicates of `(token, seq, kind-discriminant)` are dropped;
/// - **R-2**: reconstruct(prefix) + replay(suffix) ≡ reconstruct(all) —
///   `WorldState` is a snapshot, and [`apply_suffix`] is the replay.
#[must_use]
pub fn reconstruct(records: impl IntoIterator<Item = LogRecord>) -> WorldState {
    // Group per token, dedup by (seq, kind), order by seq. Kind joins the
    // key because Minted is seq 0 by definition while an event could carry
    // seq 0 from a permissive producer — never conflate birth with traffic.
    let mut per_token: BTreeMap<Token, BTreeMap<(u32, u8), LogRecord>> = BTreeMap::new();
    for rec in records {
        let kind = match &rec {
            LogRecord::Minted { .. } => 0u8,
            LogRecord::Mutation { .. } => 1,
            LogRecord::Event(_) => 2,
        };
        per_token
            .entry(rec.token())
            .or_default()
            .entry((rec.seq().0, kind))
            .or_insert(rec);
    }

    let ordered = per_token.into_values().flat_map(BTreeMap::into_values);
    let (manifests, (funnels, lineage)) = replay(
        ordered,
        (
            ManifestFold::default(),
            (FunnelFold::default(), LineageFold::default()),
        ),
    );
    WorldState {
        manifests: manifests.manifests,
        funnels: funnels.per_token,
        lineage: lineage.children,
    }
}

/// Continue a snapshot with later records (R-2's replay half). The suffix
/// must already be per-token seq-ordered relative to the snapshot — stores
/// guarantee this by construction (C-3).
#[must_use]
pub fn apply_suffix(
    snapshot: WorldState,
    suffix: impl IntoIterator<Item = LogRecord>,
) -> WorldState {
    let mut manifests = ManifestFold {
        manifests: snapshot.manifests,
    };
    let mut funnels = FunnelFold {
        per_token: snapshot.funnels,
    };
    let mut lineage = LineageFold {
        children: snapshot.lineage,
    };
    for rec in suffix {
        use crate::fold::Fold as _;
        manifests.apply(&rec);
        funnels.apply(&rec);
        lineage.apply(&rec);
    }
    WorldState {
        manifests: manifests.manifests,
        funnels: funnels.per_token,
        lineage: lineage.children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ActorClass, Event, Seq};
    use crate::{CanonicalUrl, Channel, MintOptions, MintSpec, ResolverContext, Sharer, Timestamp};

    #[test]
    fn seq_zero_event_never_conflates_with_minted() {
        let mut entropy = |buf: &mut [u8]| {
            buf.fill(9);
            Ok(())
        };
        let m = crate::mint(
            MintSpec::new(
                CanonicalUrl::new("ws://a/b").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1),
        )
        .unwrap();
        let token = m.token;
        let records = vec![
            LogRecord::Event(Event {
                token,
                stage: crate::Stage::impression(),
                actor: ActorClass::from_context(&ResolverContext::human()),
                at: Timestamp::from_unix_ms(2),
                seq: Seq(0), // permissive producer
                variant: None,
            }),
            LogRecord::Minted {
                manifest: Box::new(m),
            },
        ];
        let world = reconstruct(records);
        assert!(world.manifests.contains_key(&token));
        assert_eq!(world.funnels[&token][&crate::Stage::impression()], 1);
    }
}
