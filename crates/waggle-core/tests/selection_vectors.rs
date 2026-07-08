//! CP-2 gate: the selection-vector table (design docs `11 §4`, `14 CP-2`).
//!
//! Data-driven rows pin the sealed matcher's behavior exactly — including
//! ties and near-misses, because that's where implementations rot. The
//! table includes the worked example from design doc `06 §2` and the
//! multimodal example from `06 §2` (rev 2.3). These vectors are the seed of
//! the public conformance vectors published at 1.0.

use waggle_core::{
    select_variant, Constraint, ConsumerKind, MatchExpr, ModalitySet, Posture, ResolverContext,
    Variant, VariantBody,
};

fn v(match_expr: MatchExpr, tag: &str) -> Variant {
    Variant {
        match_expr,
        body: VariantBody::Inline {
            content_type: "text/plain".into(),
            data: tag.into(),
        },
        revalidate_after_ms: None,
    }
}

fn tag(variant: &Variant) -> &str {
    match &variant.body {
        VariantBody::Inline { data, .. } => data,
        VariantBody::Media(_) => "media",
    }
}

fn ctx(
    family: Option<&str>,
    harness: Option<&str>,
    modalities: ModalitySet,
    posture: Posture,
) -> ResolverContext {
    ResolverContext {
        kind: ConsumerKind::Agent,
        model_family: family.map(str::to_owned),
        harness: harness.map(str::to_owned),
        modalities,
        posture,
    }
}

fn one_of(s: &str) -> Constraint {
    Constraint::OneOf(vec![s.to_owned()])
}

/// The play-style token from design doc `06 §2`, five variants.
fn doc06_variants() -> Vec<Variant> {
    vec![
        // 0: claude + terminal-harness guidance
        v(
            MatchExpr {
                model_family: one_of("claude"),
                harness: one_of("claude-code"),
                ..MatchExpr::default()
            },
            "skill-md-guidance",
        ),
        // 1: codex tool-mapping
        v(
            MatchExpr {
                model_family: one_of("gpt"),
                ..MatchExpr::default()
            },
            "codex-tool-mapping",
        ),
        // 2: browser-capable flow
        v(
            MatchExpr {
                modalities: Some(ModalitySet::BROWSER),
                ..MatchExpr::default()
            },
            "browser-flow",
        ),
        // 3: headless/ci fail-closed policy
        v(
            MatchExpr {
                posture: Some(vec![Posture::Headless, Posture::Ci]),
                ..MatchExpr::default()
            },
            "fail-closed",
        ),
        // 4: catch-all
        v(MatchExpr::any(), "generic"),
    ]
}

struct Row {
    name: &'static str,
    ctx: ResolverContext,
    expect: &'static str,
}

#[test]
fn doc06_walkthrough_table() {
    let variants = doc06_variants();
    let rows = [
        Row {
            name: "claude in claude-code (attended): specificity-2 wins",
            ctx: ctx(
                Some("claude"),
                Some("claude-code"),
                ModalitySet::TEXT,
                Posture::Attended,
            ),
            expect: "skill-md-guidance",
        },
        Row {
            name: "claude in cowork-class harness: family alone misses variant 0; \
                   browser modality carries variant 2",
            ctx: ctx(
                Some("claude"),
                Some("claude-cowork"),
                ModalitySet::TEXT.with(ModalitySet::BROWSER),
                Posture::Attended,
            ),
            expect: "browser-flow",
        },
        Row {
            name: "gpt headless: tie between codex-mapping and fail-closed (both \
                   specificity 1) — declaration order picks codex-mapping",
            ctx: ctx(Some("gpt"), None, ModalitySet::TEXT, Posture::Headless),
            expect: "codex-tool-mapping",
        },
        Row {
            name: "anonymous CI runner: only posture matches",
            ctx: ctx(None, None, ModalitySet::TEXT, Posture::Ci),
            expect: "fail-closed",
        },
        Row {
            name: "anonymous attended text-only agent: near-miss on every \
                   constrained variant → catch-all",
            ctx: ctx(None, None, ModalitySet::TEXT, Posture::Attended),
            expect: "generic",
        },
        Row {
            name: "gemini with vision but no browser: vision ⊉ browser → catch-all",
            ctx: ctx(Some("gemini"), None, ModalitySet::VISION, Posture::Attended),
            expect: "generic",
        },
    ];
    for row in rows {
        let sel = select_variant(&variants, &row.ctx)
            .unwrap_or_else(|| panic!("row `{}` selected nothing", row.name));
        assert_eq!(tag(sel.variant), row.expect, "row `{}`", row.name);
    }
}

/// The multimodal whiteboard-photo token (doc `06 §2`, rev 2.3):
/// image to the vision agent, transcript to everyone else.
#[test]
fn multimodal_by_modality_table() {
    let variants = vec![
        v(
            MatchExpr {
                modalities: Some(ModalitySet::VISION),
                ..MatchExpr::default()
            },
            "the-image",
        ),
        v(
            MatchExpr {
                modalities: Some(ModalitySet::AUDIO),
                ..MatchExpr::default()
            },
            "the-voice-note",
        ),
        v(MatchExpr::any(), "transcript-and-alt-text"),
    ];
    let eyes = ctx(
        None,
        None,
        ModalitySet::TEXT.with(ModalitySet::VISION),
        Posture::Attended,
    );
    let ears = ctx(
        None,
        None,
        ModalitySet::TEXT.with(ModalitySet::AUDIO),
        Posture::Attended,
    );
    let neither = ctx(None, None, ModalitySet::TEXT, Posture::Attended);
    assert_eq!(
        tag(select_variant(&variants, &eyes).unwrap().variant),
        "the-image"
    );
    assert_eq!(
        tag(select_variant(&variants, &ears).unwrap().variant),
        "the-voice-note"
    );
    assert_eq!(
        tag(select_variant(&variants, &neither).unwrap().variant),
        "transcript-and-alt-text"
    );
}

/// CP-2 determinism gate: same context ⇒ same index, over 10⁴ pseudo-random
/// contexts against the doc06 variant set — and stability across repeated
/// evaluation (the property agents' trust rests on, I-2).
#[test]
fn determinism_over_10k_random_contexts() {
    let variants = doc06_variants();
    let families = [
        None,
        Some("claude"),
        Some("gpt"),
        Some("gemini"),
        Some("other"),
    ];
    let harnesses = [None, Some("claude-code"), Some("codex"), Some("cursor")];
    let postures = [Posture::Attended, Posture::Headless, Posture::Ci];
    let mut state = 0x5EED_u32;
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for _ in 0..10_000 {
        let c = ctx(
            families[(rnd() as usize) % families.len()],
            harnesses[(rnd() as usize) % harnesses.len()],
            ModalitySet::from_bits_truncate((rnd() % 32) as u8),
            postures[(rnd() as usize) % postures.len()],
        );
        let a = select_variant(&variants, &c).map(|s| s.index);
        let b = select_variant(&variants, &c).map(|s| s.index);
        assert_eq!(a, b, "same context must select the same variant index");
        assert!(a.is_some(), "catch-all makes selection total");
    }
}
