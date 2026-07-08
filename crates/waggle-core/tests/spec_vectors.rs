//! CP-12: the published conformance vectors are LOAD-BEARING — this test
//! reads `spec/vectors/*.json` (the files an independent implementation
//! would target) and holds the reference implementation to them. The CI
//! docs job additionally regenerates them and fails on drift.

use waggle_core::{select_variant, ResolverContext, Variant};

fn vectors_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors")
}

#[test]
fn selection_vectors_hold() {
    let raw = std::fs::read_to_string(vectors_dir().join("selection.json"))
        .expect("spec/vectors/selection.json — run `cargo xtask gen-vectors`");
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let variants: Vec<Variant> = serde_json::from_value(doc["variants"].clone()).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(
        cases.len() >= 6,
        "the vector set covers the doc-06 walkthrough"
    );
    for case in cases {
        let ctx: ResolverContext = serde_json::from_value(case["context"].clone()).unwrap();
        let expect = u8::try_from(case["expect_index"].as_u64().unwrap()).unwrap();
        let selected = select_variant(&variants, &ctx).expect("total");
        assert_eq!(
            selected.index, expect,
            "vector `{}` diverged — the sealed matcher changed",
            case["name"]
        );
    }
}

#[test]
fn signature_vector_holds() {
    let raw = std::fs::read_to_string(vectors_dir().join("signature.json"))
        .expect("spec/vectors/signature.json — run `cargo xtask gen-vectors`");
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let manifest: waggle_core::AttributionManifest =
        serde_json::from_value(doc["manifest"].clone()).unwrap();

    // Re-sign with the spec seed: the block must match byte for byte —
    // any divergence is a canonical-encoding break.
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let block = waggle_core::trust::sign_manifest(&manifest, &key);
    assert_eq!(
        serde_json::to_value(&block).unwrap(),
        doc["signature"],
        "canonical core encoding drifted from the published vector"
    );

    // And a manifest carrying the published block verifies.
    let mut signed = manifest;
    signed.signature = Some(serde_json::from_value(doc["signature"].clone()).unwrap());
    assert!(matches!(
        waggle_core::trust::verify_manifest(&signed),
        waggle_core::trust::SignatureStatus::Valid { .. }
    ));
}
