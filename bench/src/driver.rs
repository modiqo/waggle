//! Tier 2 driver seam (design doc `22 §3`).
//!
//! An [`AgentDriver`] turns a trial into the region-touch bitmask an agent's
//! interrogation produced *through the substrate*. The harness feeds that
//! mask through the real coverage machinery — [`waggle_core::RegionTouchFold`]
//! folded and judged by [`waggle_core::Contract`] — to derive the receipt
//! signal. This is the seam a live model plugs into. A `ScriptedDriver`
//! models behaviour deterministically today; an `ApiDriver` would derive the
//! mask from a real model's actual substrate reads when keys are supplied.

use crate::rng::Lcg;

/// Which kind of subagent a trial simulates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentKind {
    /// Actually consumes the required content.
    Genuine,
    /// Reports completion without reading — the adversary.
    Bluffer,
}

/// The access condition under test.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Condition {
    /// Source in a vault: the token is the only path to the bytes.
    Sealed,
    /// Source directly readable on a shared filesystem — the "side door."
    SideDoor,
}

/// One trial: an agent of a kind, under a condition, against a contract of
/// `regions` required regions.
pub(crate) struct Trial {
    /// The (ground-truth) kind of agent.
    pub kind: AgentKind,
    /// The access condition.
    pub condition: Condition,
    /// Number of declared contract regions.
    pub regions: usize,
}

/// The model seam: given a trial, return the region-touch bitmask the
/// agent's interrogation produced through the substrate.
pub(crate) trait AgentDriver {
    /// Bit `i` set ⇔ the agent touched region `i` *via the substrate*.
    fn interrogate(&mut self, trial: &Trial) -> u8;
}

/// Deterministic touch model (design doc `22 §3`). Genuine readers touch
/// each required region with high probability; under the side door they may
/// bypass the substrate entirely (reading the file directly — a false
/// negative in the receipts); bluffers touch only incidentally.
pub(crate) struct ScriptedDriver {
    rng: Lcg,
    /// Per-region probability a genuine reader touches it via the substrate.
    p_read: f64,
    /// Per-region probability a bluffer incidentally touches it.
    p_bluff: f64,
    /// Side-door probability a genuine reader bypasses the substrate.
    bypass: f64,
}

impl ScriptedDriver {
    /// Build a scripted driver with the touch/bypass probabilities.
    pub(crate) fn new(seed: u64, p_read: f64, p_bluff: f64, bypass: f64) -> Self {
        Self {
            rng: Lcg::new(seed),
            p_read,
            p_bluff,
            bypass,
        }
    }
}

impl AgentDriver for ScriptedDriver {
    fn interrogate(&mut self, trial: &Trial) -> u8 {
        let mut bits = 0u8;
        match trial.kind {
            AgentKind::Genuine => {
                // Under the side door a genuine reader may read the file
                // directly, leaving no substrate record: a false negative.
                if trial.condition == Condition::SideDoor && self.rng.chance(self.bypass) {
                    return 0;
                }
                for i in 0..trial.regions {
                    if self.rng.chance(self.p_read) {
                        bits |= 1u8 << i;
                    }
                }
            }
            AgentKind::Bluffer => {
                for i in 0..trial.regions {
                    if self.rng.chance(self.p_bluff) {
                        bits |= 1u8 << i;
                    }
                }
            }
        }
        bits
    }
}
