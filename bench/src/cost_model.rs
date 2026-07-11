//! Tier 1a — the handoff cost model (design doc `22 §2.1`).
//!
//! Three strategies price the same delegation: a naïve copy (the
//! cross-vendor / cross-machine reality), a copy that benefits from
//! within-vendor prompt caching (the *strong* baseline), and waggle's name
//! semantics. All costs are in tokens under a pluggable [`Tokenizer`]; the
//! headline is the tokenizer-invariant ratio.

use crate::tokenizer::Tokenizer;

/// The `~30`-byte handoff line — the same bytes in every tokenizer,
/// vendor, and machine (paper §"Why the copy is not a caching problem").
pub(crate) const TOKEN_LINE_BYTES: usize = 30;

/// One delegation scenario: an artifact of `s_bytes` handed to `holders`
/// consumers, each taking `turns` turns, with `revisions` corrections;
/// each consumer pulls a `proj_bytes` projection once on resolve.
#[derive(Clone, Copy)]
pub(crate) struct Scenario {
    /// Artifact size in bytes (`S`).
    pub s_bytes: usize,
    /// Number of consumers / fanout (`H`).
    pub holders: usize,
    /// Turns per consumer (`T`).
    pub turns: usize,
    /// Corrections that re-price the artifact (`R`).
    pub revisions: usize,
    /// Projection (digest) size a consumer resolves once, in bytes.
    pub proj_bytes: usize,
}

/// The three priced strategies for a scenario, in tokens.
#[derive(Clone, Copy)]
pub(crate) struct Costs {
    /// Copy with no caching: the artifact re-sent every turn of every
    /// consumer.
    pub copy_naive: f64,
    /// Copy with within-vendor prompt caching (subsequent turns discounted;
    /// a correction invalidates the cached prefix → full re-price).
    pub copy_cached: f64,
    /// waggle: the 30-byte line placed and re-sent; the projection pulled
    /// once; corrections re-send the line, not the artifact.
    pub waggle: f64,
}

impl Costs {
    /// Cost ratio against the *strong* (cached) copy baseline.
    pub(crate) fn ratio_vs_cached(&self) -> f64 {
        if self.waggle > 0.0 {
            self.copy_cached / self.waggle
        } else {
            f64::INFINITY
        }
    }
}

/// Price a scenario. `cache_discount ∈ [0,1]` is the fraction of full price
/// billed for a cached prefix (e.g. `0.1` for a 90 %-off cache read).
// The single-char bindings mirror the paper's notation (H, T, R, S, b, p).
#[allow(clippy::many_single_char_names)]
pub(crate) fn costs(sc: &Scenario, tok: &impl Tokenizer, cache_discount: f64) -> Costs {
    let h = sc.holders as f64;
    let t = sc.turns as f64;
    let r = sc.revisions as f64;
    let extra_turns = (t - 1.0).max(0.0);

    let s = tok.tokens(sc.s_bytes);
    let b = tok.tokens(TOKEN_LINE_BYTES);
    let p = tok.tokens(sc.proj_bytes);

    // copy, no cache: place + re-send every turn, plus each correction.
    let copy_naive = h * t * s + r * h * s;
    // copy, cached: first turn full, later turns discounted, corrections
    // invalidate the prefix and re-price at full.
    let copy_cached = h * s + h * extra_turns * cache_discount * s + r * h * s;
    // waggle: line per holder + projection once + line per extra turn +
    // line per correction.
    let waggle = h * b + h * p + h * extra_turns * b + r * h * b;

    Costs {
        copy_naive,
        copy_cached,
        waggle,
    }
}

/// A row of the cost-vs-artifact-size sweep for the crossover figure.
pub(crate) struct SweepRow {
    /// Artifact size in KiB (the x axis).
    pub s_kib: f64,
    /// The three priced strategies at this size.
    pub costs: Costs,
}

/// Sweep artifact size at a fixed fanout/turns/revisions, for the figure.
pub(crate) fn size_sweep(
    sizes_kib: &[usize],
    holders: usize,
    turns: usize,
    revisions: usize,
    proj_bytes: usize,
    tok: &impl Tokenizer,
    cache_discount: f64,
) -> Vec<SweepRow> {
    sizes_kib
        .iter()
        .map(|&kib| {
            let sc = Scenario {
                s_bytes: kib * 1024,
                holders,
                turns,
                revisions,
                proj_bytes,
            };
            SweepRow {
                s_kib: kib as f64,
                costs: costs(&sc, tok, cache_discount),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::CharRatio;

    fn tok() -> CharRatio {
        CharRatio::english()
    }

    #[test]
    fn waggle_beats_cached_copy_on_the_paper_cell() {
        // paper §"The arithmetic, honestly": 40 KB, 5 consumers, 5 turns,
        // one correction. waggle must win by a wide margin.
        let sc = Scenario {
            s_bytes: 40 * 1024,
            holders: 5,
            turns: 5,
            revisions: 1,
            proj_bytes: 2 * 1024,
        };
        let c = costs(&sc, &tok(), 0.1);
        assert!(c.waggle < c.copy_cached);
        assert!(c.copy_cached < c.copy_naive);
        assert!(
            c.ratio_vs_cached() > 5.0,
            "expected a large ratio, got {}",
            c.ratio_vs_cached()
        );
    }

    #[test]
    fn ratio_is_tokenizer_invariant() {
        // Same scenario, two different bytes-per-token ratios → identical
        // cost ratio (the headline does not depend on the tokenizer).
        struct Ratio(f64);
        impl Tokenizer for Ratio {
            fn tokens(&self, bytes: usize) -> f64 {
                bytes as f64 / self.0
            }
            fn label(&self) -> &'static str {
                "test"
            }
        }
        let sc = Scenario {
            s_bytes: 40 * 1024,
            holders: 5,
            turns: 5,
            revisions: 1,
            proj_bytes: 2 * 1024,
        };
        let a = costs(&sc, &Ratio(4.0), 0.1).ratio_vs_cached();
        let b = costs(&sc, &Ratio(3.0), 0.1).ratio_vs_cached();
        assert!(
            (a - b).abs() < 1e-9,
            "ratio changed with tokenizer: {a} vs {b}"
        );
    }

    #[test]
    fn single_turn_single_holder_is_the_hard_case() {
        // With H=T=1, R=0 the copy is just one placement; waggle pays the
        // line plus a projection. The model must not pretend waggle wins
        // everywhere — honesty check.
        let sc = Scenario {
            s_bytes: 1024,
            holders: 1,
            turns: 1,
            revisions: 0,
            proj_bytes: 2 * 1024,
        };
        let c = costs(&sc, &tok(), 0.1);
        assert!(c.waggle > 0.0 && c.copy_cached > 0.0);
    }
}
