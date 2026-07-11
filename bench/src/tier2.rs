//! Tier 2 — verification without trust (design doc `22 §3`).
//!
//! The decisive experiment the paper's Limitations name: how reliably does
//! the receipt signal (served bytes + coverage fold) detect that a subagent
//! *consumed the required region*, under a *seal* versus a *side door*, with
//! *bluffers* mixed in? Every trial routes through the real coverage
//! machinery — [`waggle_core::Event`] region-touch bits, combined by
//! [`waggle_core::RegionTouchFold`], judged by [`waggle_core::Contract`] —
//! so the signal is the substrate's own, not a mock. The agent behaviour is
//! the only modelled part, and it lives behind the [`AgentDriver`] seam.

use waggle_core::{
    replay, ActorClass, Contract, Event, LogRecord, Region, RegionTouchFold, ResolverContext, Seq,
    Stage, Timestamp, Token, FULL_COVERAGE_PERMILLE,
};

use crate::driver::{AgentDriver, AgentKind, Condition, ScriptedDriver, Trial};
use crate::rng::Lcg;

/// A confusion matrix over trials: does "coverage met" predict "genuinely
/// consumed"?
#[derive(Clone, Copy, Default)]
pub(crate) struct Metrics {
    /// Genuine and coverage-met.
    pub tp: u32,
    /// Bluffer but coverage-met (a missed bluff).
    pub fp: u32,
    /// Genuine but coverage-unmet (a false negative — the side door leaks
    /// these).
    pub fn_neg: u32,
    /// Bluffer and coverage-unmet (a caught bluff).
    pub tn: u32,
}

fn ratio(num: u32, den: u32) -> f64 {
    if den == 0 {
        0.0
    } else {
        f64::from(num) / f64::from(den)
    }
}

impl Metrics {
    pub(crate) fn precision(&self) -> f64 {
        ratio(self.tp, self.tp + self.fp)
    }
    pub(crate) fn recall(&self) -> f64 {
        ratio(self.tp, self.tp + self.fn_neg)
    }
    pub(crate) fn f1(&self) -> f64 {
        let (p, r) = (self.precision(), self.recall());
        if p + r > 0.0 {
            2.0 * p * r / (p + r)
        } else {
            0.0
        }
    }
    /// Fraction of genuine consumers the receipts *miss* (the leak).
    pub(crate) fn false_negative_rate(&self) -> f64 {
        ratio(self.fn_neg, self.tp + self.fn_neg)
    }
    /// Fraction of bluffers the receipts *catch*.
    pub(crate) fn bluffer_detection(&self) -> f64 {
        ratio(self.tn, self.tn + self.fp)
    }
}

/// The Tier-2 result: per-condition metrics and a coverage ROC.
pub(crate) struct Report {
    /// Trials per condition.
    pub trials_per_condition: usize,
    /// Declared contract regions.
    pub regions: usize,
    /// Fraction of trials that were bluffers.
    pub bluffer_rate: f64,
    /// Sealed condition.
    pub sealed: Metrics,
    /// Side-door condition.
    pub side_door: Metrics,
    /// ROC over the coverage threshold, `(fpr, tpr)`, sorted by `fpr`.
    pub roc: Vec<(f64, f64)>,
    /// Area under the ROC.
    pub auc: f64,
}

/// Build a contract of `regions` disjoint required regions, satisfied only
/// at full coverage.
fn build_contract(regions: usize) -> Contract {
    let rs = (0..regions)
        .map(|i| {
            let start = u32::try_from(i * 100 + 1).expect("small");
            Region::new(Some(format!("r{i}")), start, start + 49, i).expect("valid region")
        })
        .collect();
    Contract::new(rs, FULL_COVERAGE_PERMILLE).expect("valid contract")
}

/// Route a touch mask through the real coverage machinery and return the
/// achieved coverage in permille.
fn permille_for(contract: &Contract, bits: u8, token: Token) -> u16 {
    // One event per touched region — the OR-fold reassembles them, exactly
    // as separate reads would land on the log.
    let records: Vec<LogRecord> = (0..contract.regions().len())
        .filter(|&i| bits & (1u8 << i) != 0)
        .map(|i| {
            LogRecord::Event(Event {
                token,
                stage: Stage::read(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                at: Timestamp::from_unix_ms(1),
                seq: Seq(1),
                variant: None,
                regions: Some(1u8 << i),
            })
        })
        .collect();
    let fold = replay(records, RegionTouchFold::default());
    let folded = fold.per_token.get(&token).copied().unwrap_or(0);
    contract.evaluate(folded).permille
}

#[allow(clippy::too_many_arguments)]
fn eval_condition(
    contract: &Contract,
    condition: Condition,
    n: usize,
    bluffer_rate: f64,
    driver: &mut ScriptedDriver,
    kind_rng: &mut Lcg,
    token: Token,
    samples: &mut Vec<(bool, u16)>,
) -> Metrics {
    let mut m = Metrics::default();
    let regions = contract.regions().len();
    for _ in 0..n {
        let kind = if kind_rng.chance(bluffer_rate) {
            AgentKind::Bluffer
        } else {
            AgentKind::Genuine
        };
        let trial = Trial {
            kind,
            condition,
            regions,
        };
        let bits = driver.interrogate(&trial);
        let permille = permille_for(contract, bits, token);
        let genuine = kind == AgentKind::Genuine;
        let met = permille >= FULL_COVERAGE_PERMILLE;
        match (genuine, met) {
            (true, true) => m.tp += 1,
            (true, false) => m.fn_neg += 1,
            (false, true) => m.fp += 1,
            (false, false) => m.tn += 1,
        }
        samples.push((genuine, permille));
    }
    m
}

/// Sweep the coverage threshold to trace the ROC and its area.
fn roc_curve(samples: &[(bool, u16)]) -> (Vec<(f64, f64)>, f64) {
    let pos = samples.iter().filter(|(g, _)| *g).count();
    let neg = samples.len() - pos;
    let mut pts: Vec<(f64, f64)> = (0..=1001)
        .step_by(25)
        .map(|theta| {
            let t = u16::try_from(theta).unwrap_or(u16::MAX);
            let mut tp = 0u32;
            let mut fp = 0u32;
            for &(g, p) in samples {
                if p >= t {
                    if g {
                        tp += 1;
                    } else {
                        fp += 1;
                    }
                }
            }
            (
                ratio(fp, u32::try_from(neg).unwrap_or(u32::MAX)),
                ratio(tp, u32::try_from(pos).unwrap_or(u32::MAX)),
            )
        })
        .collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let auc = pts
        .windows(2)
        .map(|w| (w[1].0 - w[0].0) * (w[0].1 + w[1].1) / 2.0)
        .sum();
    (pts, auc)
}

/// Run the Tier-2 experiment with the given behaviour model.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    regions: usize,
    n: usize,
    bluffer_rate: f64,
    p_read: f64,
    p_bluff: f64,
    bypass: f64,
    seed: u64,
) -> Report {
    let contract = build_contract(regions);
    let mut driver = ScriptedDriver::new(seed, p_read, p_bluff, bypass);
    let mut kind_rng = Lcg::new(seed ^ 0x9E37_79B9_7F4A_7C15);

    // One fixed token suffices: each trial folds independently.
    let mut ctr: u8 = 0;
    let mut ent = |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            ctr = ctr.wrapping_add(13);
            *b = ctr;
        }
        Ok(())
    };
    let token = Token::generate(8, &mut ent).expect("token");

    let mut samples: Vec<(bool, u16)> = Vec::new();
    let sealed = eval_condition(
        &contract,
        Condition::Sealed,
        n,
        bluffer_rate,
        &mut driver,
        &mut kind_rng,
        token,
        &mut samples,
    );
    let side_door = eval_condition(
        &contract,
        Condition::SideDoor,
        n,
        bluffer_rate,
        &mut driver,
        &mut kind_rng,
        token,
        &mut samples,
    );
    let (roc, auc) = roc_curve(&samples);

    Report {
        trials_per_condition: n,
        regions,
        bluffer_rate,
        sealed,
        side_door,
        roc,
        auc,
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn sealed_is_more_reliable_than_the_side_door() {
        let r = run(3, 400, 0.25, 0.98, 0.04, 0.35, 0x5EA1);
        // Bluffers are caught either way: precision is high.
        assert!(
            r.sealed.precision() > 0.95,
            "precision {}",
            r.sealed.precision()
        );
        assert!(r.sealed.bluffer_detection() > 0.95);
        // The side door leaks genuine consumption → lower recall, higher FNR.
        assert!(
            r.sealed.recall() > r.side_door.recall() + 0.15,
            "sealed {} vs side-door {}",
            r.sealed.recall(),
            r.side_door.recall()
        );
        assert!(r.side_door.false_negative_rate() > r.sealed.false_negative_rate());
        // The graded signal separates genuine from bluffer well.
        assert!(r.auc > 0.9, "auc {}", r.auc);
    }
}
