//! CP-1 property suite (design doc `14 CP-1`: ≥6 properties; `13 §5`:
//! property tests are a named pyramid layer). Each property cites the
//! invariant or gate it defends.

use proptest::prelude::*;
use waggle_core::{
    mint, CanonicalUrl, Channel, Disposition, MatchExpr, MintOptions, MintSpec, Sharer, Stage,
    Timestamp, Token, VariantBody, TOKEN_ALPHABET,
};

/// A deterministic, seedable entropy source for property runs.
fn seeded_entropy(seed: u32) -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = seed | 1; // xorshift32 must not start at zero
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

fn arb_slug() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[a-z0-9][a-z0-9._/-]{0,30}").expect("valid regex")
}

proptest! {
    /// P1 — Token generation only ever emits alphabet characters, at the
    /// requested length, for any seed and any valid length (02 §2).
    #[test]
    fn p1_tokens_stay_inside_the_alphabet(seed: u32, len in 1usize..=23) {
        let mut entropy = seeded_entropy(seed);
        let t = Token::generate(len, &mut entropy).unwrap();
        prop_assert_eq!(t.as_str().len(), len);
        prop_assert!(t.as_str().bytes().all(|b| TOKEN_ALPHABET.contains(&b)));
    }

    /// P2 — Parse/display round-trips for every generated token
    /// (tokens are names: identity survives the string boundary).
    #[test]
    fn p2_token_string_roundtrip(seed: u32) {
        let mut entropy = seeded_entropy(seed);
        let t = Token::generate(8, &mut entropy).unwrap();
        prop_assert_eq!(Token::parse(&t.to_string()).unwrap(), t);
    }

    /// P3 — Slug normalization is idempotent: normalize(normalize(x)) ==
    /// normalize(x) (13 §5 slug property).
    #[test]
    fn p3_slug_normalization_idempotent(raw in arb_slug()) {
        if let Ok(once) = Channel::new(&raw) {
            let twice = Channel::new(once.as_str()).unwrap();
            prop_assert_eq!(once, twice);
        }
        if let Ok(once) = Stage::new(&raw) {
            let twice = Stage::new(once.as_str()).unwrap();
            prop_assert_eq!(once, twice);
        }
    }

    /// P4 — Slugs never accept whitespace, uppercase output, or empties —
    /// whatever unicode arrives (the negative space of P3).
    #[test]
    fn p4_slug_rejects_or_normalizes_everything(raw in ".{0,80}") {
        // Rejection is a fine outcome for arbitrary input; acceptance must
        // yield a normalized, charset-clean slug.
        if let Ok(s) = Sharer::new(&raw) {
            prop_assert!(!s.as_str().is_empty());
            prop_assert!(s.as_str().bytes().all(|b| b.is_ascii_lowercase()
                || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.' | b'/')));
        }
    }

    /// P5 — one_call_mint (17 §5): a bare mint always yields exactly one
    /// catch-all variant, version 1, and an Active disposition at mint time.
    #[test]
    fn p5_one_call_mint_is_total(seed: u32, now_ms: u64) {
        let mut entropy = seeded_entropy(seed);
        let now = Timestamp::from_unix_ms(now_ms);
        let m = mint(
            MintSpec::new(
                CanonicalUrl::new("ws://a/artifact").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            now,
        )
        .unwrap();
        let catch_alls = m.variants.iter().filter(|v| v.match_expr.is_catch_all()).count();
        prop_assert_eq!(catch_alls, 1);
        prop_assert_eq!(m.version, 1);
        prop_assert_eq!(m.disposition(now), Disposition::Active);
    }

    /// P6 — Manifest JSON round-trips exactly, for varied specs (the
    /// manifest is the retrievable contract — 02 §2).
    #[test]
    fn p6_manifest_serde_roundtrip(seed: u32, ttl in proptest::option::of(0u64..1_000_000), body in "[ -~]{0,200}") {
        let mut entropy = seeded_entropy(seed);
        let mut spec = MintSpec::new(
            CanonicalUrl::new("file:///tmp/report.md").unwrap(),
            Sharer::new("orchestrator").unwrap(),
            Channel::new("subagent/pricing").unwrap(),
        )
        .variant(
            MatchExpr::any(),
            VariantBody::Inline { content_type: "text/markdown".into(), data: body },
        );
        if let Some(t) = ttl { spec = spec.ttl_ms(t); }
        let m = mint(spec, &MintOptions::default(), &mut entropy, Timestamp::from_unix_ms(5)).unwrap();
        let json = serde_json::to_string(&m).unwrap();
        let back: waggle_core::AttributionManifest = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, m);
    }

    /// P7 — Expiry semantics: for any ttl, the manifest is Active strictly
    /// before `minted_at + ttl` and Expired at/after it (G-3's substrate).
    #[test]
    fn p7_ttl_expiry_boundary(seed: u32, start in 0u64..1_000_000, ttl in 1u64..1_000_000) {
        let mut entropy = seeded_entropy(seed);
        let now = Timestamp::from_unix_ms(start);
        let m = mint(
            MintSpec::new(
                CanonicalUrl::new("ws://a/b").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            )
            .ttl_ms(ttl),
            &MintOptions::default(),
            &mut entropy,
            now,
        )
        .unwrap();
        let boundary = Timestamp::from_unix_ms(start + ttl);
        prop_assert_eq!(m.disposition(Timestamp::from_unix_ms(start + ttl - 1)), Disposition::Active);
        prop_assert_eq!(m.disposition(boundary), Disposition::Expired);
    }
}

/// P8 (plain test) — distinct entropy streams produce distinct tokens at
/// realistic rates: 10k seeded mints, no collision (a birthday-bound sanity
/// check on the 58⁸ space, not a proof).
#[test]
fn p8_no_collisions_across_10k_seeds() {
    let mut seen = std::collections::HashSet::new();
    for seed in 1..=10_000u32 {
        let mut entropy = seeded_entropy(seed.wrapping_mul(0x9E37_79B9));
        let t = Token::generate(8, &mut entropy).unwrap();
        assert!(seen.insert(t.to_string()), "collision at seed {seed}");
    }
}
