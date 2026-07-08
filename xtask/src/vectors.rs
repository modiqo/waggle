//! `cargo xtask gen-vectors` — emit the portable conformance vectors
//! (spec §3/§6) from the reference implementation itself, so the JSON
//! can never drift from the code that defines it.

use waggle_core::{
    mint, resolve, select_variant, CanonicalUrl, Channel, Constraint, MatchExpr, MintOptions,
    MintSpec, ModalitySet, Posture, ResolverContext, Sharer, Timestamp, Variant, VariantBody,
};

fn seeded(fill: u8) -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    move |b: &mut [u8]| {
        b.fill(fill);
        Ok(())
    }
}

fn inline(tag: &str) -> VariantBody {
    VariantBody::Inline {
        content_type: "text/plain".into(),
        data: tag.into(),
    }
}

fn ctx(
    family: Option<&str>,
    harness: Option<&str>,
    modalities: u8,
    posture: Posture,
) -> ResolverContext {
    ResolverContext {
        kind: waggle_core::ConsumerKind::Agent,
        model_family: family.map(str::to_owned),
        harness: harness.map(str::to_owned),
        modalities: ModalitySet::from_bits_truncate(modalities),
        posture,
    }
}

/// The doc-06 variant set — the same one the selection-vector tests pin.
fn variants() -> Vec<Variant> {
    let one = |s: &str| Constraint::OneOf(vec![s.to_owned()]);
    vec![
        Variant {
            match_expr: MatchExpr {
                model_family: one("claude"),
                harness: one("claude-code"),
                ..MatchExpr::default()
            },
            body: inline("claude-code-guidance"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                model_family: one("gpt"),
                ..MatchExpr::default()
            },
            body: inline("gpt-mapping"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                modalities: Some(ModalitySet::BROWSER),
                ..MatchExpr::default()
            },
            body: inline("browser-flow"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                posture: Some(vec![Posture::Headless, Posture::Ci]),
                ..MatchExpr::default()
            },
            body: inline("fail-closed"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr::any(),
            body: inline("generic"),
            revalidate_after_ms: None,
        },
    ]
}

/// Write `spec/vectors/*.json`.
pub fn generate(root: &std::path::Path) -> std::io::Result<()> {
    let dir = root.join("spec/vectors");
    std::fs::create_dir_all(&dir)?;

    // ── selection.json: (context → selected index) over the doc-06 set.
    let vs = variants();
    let cases = [
        (
            "claude-in-claude-code",
            ctx(Some("claude"), Some("claude-code"), 1, Posture::Attended),
        ),
        (
            "claude-with-browser",
            ctx(Some("claude"), Some("other"), 3, Posture::Attended),
        ),
        (
            "gpt-headless-tie",
            ctx(Some("gpt"), None, 1, Posture::Headless),
        ),
        ("anonymous-ci", ctx(None, None, 1, Posture::Ci)),
        ("anonymous-attended", ctx(None, None, 1, Posture::Attended)),
        (
            "vision-not-browser",
            ctx(Some("gemini"), None, 8, Posture::Attended),
        ),
    ];
    let selection: Vec<serde_json::Value> = cases
        .iter()
        .map(|(name, c)| {
            let selected = select_variant(&vs, c).expect("total over catch-all sets");
            serde_json::json!({
                "name": name,
                "context": c,
                "expect_index": selected.index,
            })
        })
        .collect();
    std::fs::write(
        dir.join("selection.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "variants": vs,
            "cases": selection,
            "note": "sealed matcher, spec §3: match → specificity → declaration order → catch-all",
        }))?,
    )?;

    // ── signature.json: fixed-seed manifest + its exact signature block.
    let mut entropy = seeded(42);
    let manifest = mint(
        MintSpec::new(
            CanonicalUrl::new("ws://spec/vector-artifact").unwrap(),
            Sharer::new("spec").unwrap(),
            Channel::subagent_general(),
        ),
        &MintOptions::default(),
        &mut entropy,
        Timestamp::from_unix_ms(1_700_000_000_000),
    )
    .expect("vector mint");
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let block = waggle_core::trust::sign_manifest(&manifest, &key);
    let resolution = resolve(
        &manifest,
        &ResolverContext::anonymous_agent(),
        Timestamp::from_unix_ms(1_700_000_000_500),
    );
    std::fs::write(
        dir.join("signature.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "seed": "0707…07 (32 bytes of 0x07)",
            "manifest": manifest,
            "canonical_core_hex_len": waggle_core::trust::canonical_core_bytes(&manifest).len(),
            "signature": block,
            "resolution_disposition": resolution.disposition,
            "note": "spec §6: Ed25519 over the immutable core; a mismatch here is a canonical-encoding break",
        }))?,
    )?;
    Ok(())
}
