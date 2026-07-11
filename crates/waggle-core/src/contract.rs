//! Consumption contracts (design doc `19 §4.2`): the author's mint-time
//! declaration of which regions of the artifact a consumer must actually
//! reach. Declared in the **immutable core** (a contract you can
//! re-negotiate after delegation is not a contract), evaluated by the
//! coverage fold over region-touch bits stamped on `read` events.
//!
//! Regions are 1-based inclusive line ranges — at most
//! [`MAX_CONTRACT_REGIONS`], because touches travel on events as a bitmask
//! ([`crate::Event::regions`]) that must stay fixed-width (I-1's
//! fixed-width consequence, doc `03 §4`). The bitmask is
//! manifest-referencing — an index into *this* declared list — which is
//! the same I-1-compatibility argument the `variant` field makes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The most regions one contract may declare — the width of the
/// region-touch bitmask on events.
pub const MAX_CONTRACT_REGIONS: usize = 8;

/// The threshold meaning "every declared region must be touched".
pub const FULL_COVERAGE_PERMILLE: u16 = 1000;

/// Longest permitted region label (labels live in the signed core; caps
/// keep the manifest inside its size budget).
const MAX_LABEL_LEN: usize = 80;

/// Why a contract was rejected at construction.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    /// A contract with nothing required requires nothing.
    #[error("a contract needs at least one region")]
    NoRegions,
    /// More regions than the event bitmask can name.
    #[error("{0} regions exceeds the {MAX_CONTRACT_REGIONS}-region cap — merge adjacent ranges")]
    TooManyRegions(usize),
    /// Lines are 1-based and ranges are inclusive; `start` must be ≥ 1
    /// and ≤ `end`.
    #[error("region {index}: lines {start}-{end} is not a 1-based inclusive range")]
    BadRange {
        /// Which region (0-based, declaration order).
        index: usize,
        /// Declared start line.
        start: u32,
        /// Declared end line.
        end: u32,
    },
    /// The threshold is a permille of declared regions in `1..=1000`.
    #[error("min-permille {0} outside 1..=1000 (1000 = every region)")]
    BadThreshold(u16),
    /// Labels are short human names, not documents.
    #[error("region {0}: label exceeds {MAX_LABEL_LEN} characters")]
    LabelTooLong(usize),
}

/// One required region: a 1-based inclusive line range with an optional
/// human label (the section heading it came from, typically) so misses
/// can be *named*, not just numbered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RegionWire")]
pub struct Region {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    start: u32,
    end: u32,
}

/// The unvalidated wire shape — [`Region`] deserializes through it so an
/// invalid range can never enter the domain (the slug discipline, doc
/// `02 §2`).
#[derive(Deserialize)]
struct RegionWire {
    #[serde(default)]
    label: Option<String>,
    start: u32,
    end: u32,
}

impl TryFrom<RegionWire> for Region {
    type Error = ContractError;
    fn try_from(w: RegionWire) -> Result<Self, ContractError> {
        Region::new(w.label, w.start, w.end, 0)
    }
}

impl Region {
    /// Validate one region. `index` only shapes the error message.
    pub fn new(
        label: Option<String>,
        start: u32,
        end: u32,
        index: usize,
    ) -> Result<Self, ContractError> {
        if start == 0 || start > end {
            return Err(ContractError::BadRange { index, start, end });
        }
        if label.as_deref().is_some_and(|l| l.len() > MAX_LABEL_LEN) {
            return Err(ContractError::LabelTooLong(index));
        }
        Ok(Self { label, start, end })
    }

    /// The human name, if the author gave one.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// First required line (1-based).
    #[must_use]
    pub fn start(&self) -> u32 {
        self.start
    }

    /// Last required line (inclusive).
    #[must_use]
    pub fn end(&self) -> u32 {
        self.end
    }

    /// Does the served window `[from, to]` (1-based inclusive) overlap
    /// this region?
    #[must_use]
    pub fn overlaps(&self, from: u32, to: u32) -> bool {
        from <= self.end && to >= self.start
    }
}

/// The consumption contract: which regions must be reached, and what
/// fraction of them (permille) satisfies the author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContractWire", rename_all = "kebab-case")]
pub struct Contract {
    regions: Vec<Region>,
    min_permille: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ContractWire {
    regions: Vec<Region>,
    #[serde(default = "full")]
    min_permille: u16,
}

fn full() -> u16 {
    FULL_COVERAGE_PERMILLE
}

impl TryFrom<ContractWire> for Contract {
    type Error = ContractError;
    fn try_from(w: ContractWire) -> Result<Self, ContractError> {
        Contract::new(w.regions, w.min_permille)
    }
}

impl Contract {
    /// Validate a whole contract.
    pub fn new(regions: Vec<Region>, min_permille: u16) -> Result<Self, ContractError> {
        if regions.is_empty() {
            return Err(ContractError::NoRegions);
        }
        if regions.len() > MAX_CONTRACT_REGIONS {
            return Err(ContractError::TooManyRegions(regions.len()));
        }
        if min_permille == 0 || min_permille > FULL_COVERAGE_PERMILLE {
            return Err(ContractError::BadThreshold(min_permille));
        }
        Ok(Self {
            regions,
            min_permille,
        })
    }

    /// The declared regions, in declaration order (bit `i` of a touch
    /// mask names `regions()[i]`).
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// The satisfaction threshold, as a permille of declared regions.
    #[must_use]
    pub fn min_permille(&self) -> u16 {
        self.min_permille
    }

    /// Which regions a served line window `[from, to]` touches, as the
    /// event bitmask.
    #[must_use]
    pub fn touched_by_span(&self, from: u32, to: u32) -> u8 {
        let mut bits = 0u8;
        for (i, r) in self.regions.iter().enumerate() {
            if r.overlaps(from, to) {
                bits |= 1 << i;
            }
        }
        bits
    }

    /// Which regions a single served line touches (search hits).
    #[must_use]
    pub fn touched_by_line(&self, line: u32) -> u8 {
        self.touched_by_span(line, line)
    }

    /// Evaluate accumulated touch bits against the threshold.
    ///
    /// # Panics
    /// Never in practice: `touched ≤ required ≤ 8` keeps the permille
    /// within `u16`; the `expect` documents the invariant.
    #[must_use]
    pub fn evaluate(&self, bits: u8) -> Coverage {
        let required = self.regions.len();
        let touched = (0..required).filter(|i| bits & (1 << i) != 0).count();
        let missed = (0..required).filter(|i| bits & (1 << i) == 0).collect();
        let permille = u16::try_from(touched * usize::from(FULL_COVERAGE_PERMILLE) / required)
            .expect("touched ≤ required ≤ 8 keeps permille ≤ 1000");
        Coverage {
            required,
            touched,
            permille,
            met: permille >= self.min_permille,
            missed,
        }
    }
}

/// The verdict [`Contract::evaluate`] returns: how much of the contract
/// was reached, whether that satisfies the author, and — the honest
/// half — exactly which regions nobody touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Declared region count.
    pub required: usize,
    /// Regions with at least one touch.
    pub touched: usize,
    /// `touched / required`, in permille.
    pub permille: u16,
    /// `permille >= min_permille`.
    pub met: bool,
    /// Indexes (declaration order) of untouched regions — the misses,
    /// nameable via [`Region::label`].
    pub missed: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(start: u32, end: u32) -> Region {
        Region::new(None, start, end, 0).unwrap()
    }

    #[test]
    fn construction_rejects_what_the_spec_rejects() {
        assert_eq!(Contract::new(vec![], 1000), Err(ContractError::NoRegions));
        let nine = (1..=9).map(|i| region(i * 10, i * 10 + 5)).collect();
        assert_eq!(
            Contract::new(nine, 1000),
            Err(ContractError::TooManyRegions(9))
        );
        assert!(matches!(
            Region::new(None, 0, 5, 3),
            Err(ContractError::BadRange { index: 3, .. })
        ));
        assert!(matches!(
            Region::new(None, 9, 5, 0),
            Err(ContractError::BadRange { .. })
        ));
        assert_eq!(
            Contract::new(vec![region(1, 2)], 0),
            Err(ContractError::BadThreshold(0))
        );
        assert_eq!(
            Contract::new(vec![region(1, 2)], 1001),
            Err(ContractError::BadThreshold(1001))
        );
        assert!(matches!(
            Region::new(Some("x".repeat(81)), 1, 2, 5),
            Err(ContractError::LabelTooLong(5))
        ));
    }

    #[test]
    fn touch_bits_are_declaration_order_indexed() {
        let c = Contract::new(vec![region(10, 20), region(30, 40), region(50, 60)], 1000).unwrap();
        assert_eq!(c.touched_by_span(1, 9), 0b000);
        assert_eq!(c.touched_by_span(15, 35), 0b011);
        assert_eq!(c.touched_by_span(20, 30), 0b011, "inclusive at both edges");
        assert_eq!(c.touched_by_line(50), 0b100);
        assert_eq!(c.touched_by_span(1, 100), 0b111);
    }

    #[test]
    fn evaluate_thresholds_and_misses() {
        let c = Contract::new(vec![region(1, 10), region(20, 30), region(40, 50)], 667).unwrap();
        let none = c.evaluate(0);
        assert!(!none.met);
        assert_eq!(none.missed, vec![0, 1, 2]);
        let two = c.evaluate(0b101);
        assert_eq!(two.touched, 2);
        assert_eq!(two.permille, 666);
        assert!(!two.met, "666 < 667 — permille floors, honestly");
        let all = c.evaluate(0b111);
        assert!(all.met);
        assert!(all.missed.is_empty());
        assert_eq!(all.permille, 1000);
    }

    #[test]
    fn serde_validates_on_the_way_in_and_roundtrips() {
        let c = Contract::new(
            vec![Region::new(Some("Pricing".into()), 847, 920, 0).unwrap()],
            900,
        )
        .unwrap();
        let json = serde_json::to_string(&c).unwrap();
        let back: Contract = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        // min-permille defaults to full coverage when omitted.
        let d: Contract = serde_json::from_str(r#"{"regions":[{"start":1,"end":5}]}"#).unwrap();
        assert_eq!(d.min_permille(), FULL_COVERAGE_PERMILLE);
        // Invalid wire shapes never become domain values.
        assert!(serde_json::from_str::<Contract>(r#"{"regions":[]}"#).is_err());
        assert!(serde_json::from_str::<Contract>(r#"{"regions":[{"start":9,"end":5}]}"#).is_err());
    }
}
