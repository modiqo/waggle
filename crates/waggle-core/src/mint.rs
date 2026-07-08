//! Minting: `MintSpec` in, [`AttributionManifest`] out — a pure function of
//! (spec, options, entropy, now), per the sans-I/O law. Storage happens at
//! the host: minting *is* the `Minted` log record's payload (doc `04 §1`).

use thiserror::Error;

use crate::manifest::{
    AttributionManifest, MatchExpr, Variant, VariantBody, MANIFEST_SCHEMA_VERSION,
};
use crate::slug::{Channel, Sharer};
use crate::target::{CanonicalUrl, TargetMeta, MANIFEST_SIZE_CAP_BYTES};
use crate::time::Timestamp;
use crate::token::{Token, TokenError};
use crate::Entropy;

/// Why a mint was rejected.
#[derive(Debug, Error)]
pub enum MintError {
    /// More than one catch-all variant — selection order would be ambiguous
    /// to authors even though declaration order breaks the tie.
    #[error("manifest declares {0} catch-all variants; exactly one is required")]
    DuplicateCatchAll(usize),
    /// The serialized manifest exceeds the size cap; move bodies to media.
    #[error("manifest is {size} bytes, over the {cap}-byte cap — attach large bodies as media instead of inlining")]
    ManifestTooLarge {
        /// Serialized size observed.
        size: usize,
        /// The cap ([`MANIFEST_SIZE_CAP_BYTES`]).
        cap: usize,
    },
    /// Token generation failed (entropy or length configuration).
    #[error(transparent)]
    Token(#[from] TokenError),
}

/// Tuning for mint. Defaults are the product decisions from the design
/// docs; override only with a reason.
#[derive(Debug, Clone)]
pub struct MintOptions {
    /// Token length in characters (default 8 ⇒ 58⁸ ≈ 1.3 × 10¹⁴ names).
    pub token_len: usize,
}

impl Default for MintOptions {
    fn default() -> Self {
        Self { token_len: 8 }
    }
}

/// Everything a mint needs, gathered with a builder. The one-call form is
/// `MintSpec::new(target, sharer, channel)` — variants, meta, lineage, and
/// ttl are escalations, never prerequisites (doc `17 §1` rule 3).
#[derive(Debug, Clone)]
pub struct MintSpec {
    target: CanonicalUrl,
    sharer: Sharer,
    channel: Channel,
    meta: TargetMeta,
    variants: Vec<Variant>,
    parent: Option<Token>,
    content: Option<crate::MediaRef>,
    private: bool,
    ttl_ms: Option<u64>,
}

impl MintSpec {
    /// The minimum viable mint: an artifact, a sharer, a channel.
    #[must_use]
    pub fn new(target: CanonicalUrl, sharer: Sharer, channel: Channel) -> Self {
        Self {
            target,
            sharer,
            channel,
            meta: TargetMeta::default(),
            variants: Vec::new(),
            parent: None,
            content: None,
            private: false,
            ttl_ms: None,
        }
    }

    /// Attach the mint-time snapshot (title, description, image, labels).
    #[must_use]
    pub fn meta(mut self, meta: TargetMeta) -> Self {
        self.meta = meta;
        self
    }

    /// Add a variant. Declaration order is selection tie-break order.
    #[must_use]
    pub fn variant(mut self, match_expr: MatchExpr, body: VariantBody) -> Self {
        self.variants.push(Variant {
            match_expr,
            body,
            revalidate_after_ms: None,
        });
        self
    }

    /// Mark this token as a delegation child of `parent` (lineage).
    #[must_use]
    pub fn child_of(mut self, parent: Token) -> Self {
        self.parent = Some(parent);
        self
    }

    /// The target URI this spec will mint (hosts snapshot from it).
    #[must_use]
    pub fn target_str(&self) -> &str {
        self.target.as_str()
    }

    /// Declare a variant in full — including `revalidate_after_ms`.
    /// (`variant()` is the two-arg convenience; this preserves every
    /// field a caller authored.)
    #[must_use]
    pub fn with_variant(mut self, variant: Variant) -> Self {
        self.variants.push(variant);
        self
    }

    /// Mint as a capability URL (CP-11): the token is generated LONG
    /// (16 chars ≈ 94 bits — possession is the credential) and public
    /// surfaces refuse to render it.
    #[must_use]
    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    /// Pin the artifact's bytes: a content-addressed snapshot taken at
    /// mint (doc `18 §3`). Enables `read`/`search` anywhere the blobs
    /// replicate, immutable by hash.
    #[must_use]
    pub fn content(mut self, media: crate::MediaRef) -> Self {
        self.content = Some(media);
        self
    }

    /// Expire the token `ttl_ms` after mint.
    #[must_use]
    pub fn ttl_ms(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = Some(ttl_ms);
        self
    }
}

/// Mint an attribution manifest. Pure: same inputs (including the entropy
/// stream) ⇒ same manifest.
///
/// Guarantees on success:
/// - exactly one catch-all variant exists (synthesized from the target when
///   the caller declared none — the zero-ceremony path), positioned last so
///   declared variants always win ties;
/// - the serialized manifest is within [`MANIFEST_SIZE_CAP_BYTES`];
/// - `version` starts at 1 (the CAS baseline for lifecycle mutations, C-9).
pub fn mint(
    spec: MintSpec,
    opts: &MintOptions,
    entropy: &mut impl Entropy,
    now: Timestamp,
) -> Result<AttributionManifest, MintError> {
    let mut variants = spec.variants;
    let catch_alls = variants
        .iter()
        .filter(|v| v.match_expr.is_catch_all())
        .count();
    match catch_alls {
        0 => variants.push(synthesized_catch_all(&spec.target, &spec.meta)),
        1 => {}
        n => return Err(MintError::DuplicateCatchAll(n)),
    }

    let token_len = if spec.private { 16 } else { opts.token_len };
    let manifest = AttributionManifest {
        schema: MANIFEST_SCHEMA_VERSION,
        token: Token::generate(token_len, entropy)?,
        target: spec.target,
        sharer: spec.sharer,
        channel: spec.channel,
        minted_at: now,
        meta: spec.meta,
        parent: spec.parent,
        content: spec.content,
        private: spec.private,
        signature: None, // hosts with an identity sign after mint (trust)
        variants,
        version: 1,
        campaign: None,
        labels: std::collections::BTreeMap::new(),
        expires_at: spec.ttl_ms.map(|ttl| now.plus_ms(ttl)),
        revoked_at: None,
        superseded_by: None,
    };

    // Size cap: serde_json is in dev/host land elsewhere, but the cap is a
    // core guarantee, so measure with the same encoding hosts store.
    let size = serde_json::to_vec(&manifest).map_or(usize::MAX, |v| v.len());
    if size > MANIFEST_SIZE_CAP_BYTES {
        return Err(MintError::ManifestTooLarge {
            size,
            cap: MANIFEST_SIZE_CAP_BYTES,
        });
    }
    Ok(manifest)
}

/// The zero-ceremony catch-all: point every unmatched consumer at the
/// canonical target with the snapshot description.
fn synthesized_catch_all(target: &CanonicalUrl, meta: &TargetMeta) -> Variant {
    let description = meta.description.clone().unwrap_or_else(|| {
        format!("Fetch the artifact at {target} and use it as your working context.")
    });
    Variant {
        match_expr: MatchExpr::any(),
        body: VariantBody::Inline {
            content_type: "text/markdown".into(),
            data: description,
        },
        revalidate_after_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Constraint;

    fn fixed_entropy() -> impl FnMut(&mut [u8]) -> Result<(), crate::EntropyError> {
        let mut n = 0u8;
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                n = n.wrapping_add(13);
                *b = n;
            }
            Ok(())
        }
    }

    fn base_spec() -> MintSpec {
        MintSpec::new(
            CanonicalUrl::new("ws://analysis/report.md").unwrap(),
            Sharer::new("lead").unwrap(),
            Channel::subagent_general(),
        )
    }

    #[test]
    fn one_call_mint_synthesizes_the_catch_all() {
        // 17 §5 `one_call_mint`: no variants declared, mint still total.
        let m = mint(
            base_spec(),
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap();
        assert_eq!(m.variants.len(), 1);
        assert!(m.variants[0].match_expr.is_catch_all());
        assert_eq!(m.version, 1);
        assert_eq!(m.token.as_str().len(), 8);
    }

    #[test]
    fn declared_catch_all_is_respected_not_duplicated() {
        let spec = base_spec().variant(
            MatchExpr::any(),
            VariantBody::Inline {
                content_type: "text/plain".into(),
                data: "custom".into(),
            },
        );
        let m = mint(
            spec,
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap();
        assert_eq!(m.variants.len(), 1);
        match &m.variants[0].body {
            VariantBody::Inline { data, .. } => assert_eq!(data, "custom"),
            VariantBody::Media(_) => panic!("expected inline"),
        }
    }

    #[test]
    fn duplicate_catch_alls_are_rejected() {
        let spec = base_spec()
            .variant(
                MatchExpr::any(),
                VariantBody::Inline {
                    content_type: "a".into(),
                    data: "1".into(),
                },
            )
            .variant(
                MatchExpr::any(),
                VariantBody::Inline {
                    content_type: "a".into(),
                    data: "2".into(),
                },
            );
        let err = mint(
            spec,
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap_err();
        assert!(matches!(err, MintError::DuplicateCatchAll(2)));
    }

    #[test]
    fn synthesized_catch_all_sits_last_so_declared_variants_win_ties() {
        let spec = base_spec().variant(
            MatchExpr {
                model_family: Constraint::OneOf(vec!["claude".into()]),
                ..MatchExpr::default()
            },
            VariantBody::Inline {
                content_type: "text/plain".into(),
                data: "claude-shaped".into(),
            },
        );
        let m = mint(
            spec,
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap();
        assert_eq!(m.variants.len(), 2);
        assert!(!m.variants[0].match_expr.is_catch_all());
        assert!(m.variants[1].match_expr.is_catch_all());
    }

    #[test]
    fn ttl_becomes_expiry_relative_to_now() {
        let now = Timestamp::from_unix_ms(1_000);
        let m = mint(
            base_spec().ttl_ms(500),
            &MintOptions::default(),
            &mut fixed_entropy(),
            now,
        )
        .unwrap();
        assert_eq!(m.expires_at, Some(Timestamp::from_unix_ms(1_500)));
    }

    #[test]
    fn lineage_parent_is_recorded() {
        let parent = Token::parse("parent1").unwrap();
        let m = mint(
            base_spec().child_of(parent),
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap();
        assert_eq!(m.parent, Some(parent));
    }

    #[test]
    fn oversized_manifests_are_rejected_with_the_fix_named() {
        let big = "x".repeat(MANIFEST_SIZE_CAP_BYTES + 1);
        let spec = base_spec().variant(
            MatchExpr::any(),
            VariantBody::Inline {
                content_type: "text/plain".into(),
                data: big,
            },
        );
        let err = mint(
            spec,
            &MintOptions::default(),
            &mut fixed_entropy(),
            Timestamp::from_unix_ms(0),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("attach large bodies as media"),
            "error names the fix: {msg}"
        );
    }
}
