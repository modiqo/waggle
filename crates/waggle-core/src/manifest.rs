//! The attribution manifest: three zones with three mutability rules
//! (design doc `02 §2`) — the immutable core fixed at mint, variants fixed
//! at mint (v1), and versioned mutable sections whose every change is an
//! event first (doc `04 §4`).
//!
//! This module is the *data model*; the sealed selection algorithm over
//! variants lands in CP-2 and lives beside it, never inside it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::slug::{Channel, Sharer};
use crate::target::{CanonicalUrl, MediaRef, TargetMeta};
use crate::time::Timestamp;
use crate::token::Token;

/// Schema version of the manifest envelope — versioned independently of
/// crate semver; additive changes only (doc `09 §6`).
pub const MANIFEST_SCHEMA_VERSION: u16 = 1;

/// A set of consumer modalities, as a small bitset.
///
/// Match dimensions for variants (doc `06 §2`); `vision`/`audio` are what
/// make multimodal variants ride the ordinary matcher.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ModalitySet(u8);

impl<'de> Deserialize<'de> for ModalitySet {
    /// Accepts BOTH wire forms: the compact bits (`5`) and the humane
    /// names (`["text", "shell"]`) — hosts and switchboards write names;
    /// the log and the vectors keep bits.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = ModalitySet;
            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("modality bits (u8) or names ([\"text\", ...])")
            }
            fn visit_u64<E: serde::de::Error>(self, bits: u64) -> Result<ModalitySet, E> {
                Ok(ModalitySet::from_bits_truncate(
                    u8::try_from(bits).map_err(E::custom)?,
                ))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<ModalitySet, A::Error> {
                let mut set = ModalitySet::empty();
                while let Some(name) = seq.next_element::<String>()? {
                    set = set.with(ModalitySet::from_name(&name).ok_or_else(|| {
                        serde::de::Error::custom(format!(
                            "unknown modality `{name}` — text, browser, shell, vision, audio"
                        ))
                    })?);
                }
                Ok(set)
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl ModalitySet {
    /// Plain text I/O.
    pub const TEXT: Self = Self(1);
    /// Can drive a browser.
    pub const BROWSER: Self = Self(1 << 1);
    /// Can run shell commands.
    pub const SHELL: Self = Self(1 << 2);
    /// Can interpret images.
    pub const VISION: Self = Self(1 << 3);
    /// Can interpret audio.
    pub const AUDIO: Self = Self(1 << 4);

    /// The empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Set union.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Does `self` contain every modality in `required`?
    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// A single modality by its wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::TEXT),
            "browser" => Some(Self::BROWSER),
            "shell" => Some(Self::SHELL),
            "vision" => Some(Self::VISION),
            "audio" => Some(Self::AUDIO),
            _ => None,
        }
    }

    /// Build from raw bits, ignoring undefined ones — for hosts
    /// deserializing foreign context descriptors (and for exhaustive
    /// testing over the modality space).
    #[must_use]
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0b1_1111)
    }
}

/// The consumer's execution posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Posture {
    /// A human is present now.
    Attended,
    /// No human present (SSH box, background agent).
    Headless,
    /// Automation context (CI, cron) — fail closed on any would-be prompt.
    Ci,
}

/// One dimension's constraint in a [`MatchExpr`]: unconstrained, or an
/// allow-set. Kept as data — expressiveness grows by adding *dimensions*,
/// never by adding cleverness (doc `06 §2`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Constraint {
    /// Matches anything.
    #[default]
    Any,
    /// Matches when the context's value is one of these.
    OneOf(Vec<String>),
}

/// Which resolver contexts a variant serves. A conjunction over four
/// dimensions; specificity = number of constrained dimensions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchExpr {
    /// Model family constraint (`claude`, `gpt`, `gemini`, …).
    #[serde(default, skip_serializing_if = "constraint_is_any")]
    pub model_family: Constraint,
    /// Harness constraint (`claude-code`, `codex`, …).
    #[serde(default, skip_serializing_if = "constraint_is_any")]
    pub harness: Constraint,
    /// Required modalities (superset match), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modalities: Option<ModalitySet>,
    /// Posture constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<Vec<Posture>>,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde skip_serializing_if signature
fn constraint_is_any(c: &Constraint) -> bool {
    matches!(c, Constraint::Any)
}

impl MatchExpr {
    /// The catch-all: matches every context. Every manifest must carry one
    /// variant with this expression so selection is total (doc `06 §2`).
    #[must_use]
    pub fn any() -> Self {
        Self::default()
    }

    /// True when no dimension is constrained.
    #[must_use]
    pub fn is_catch_all(&self) -> bool {
        matches!(self.model_family, Constraint::Any)
            && matches!(self.harness, Constraint::Any)
            && self.modalities.is_none()
            && self.posture.is_none()
    }

    /// Specificity: how many dimensions are constrained (0–4). Selection
    /// ranks by this; the algorithm itself is CP-2's sealed matcher.
    #[must_use]
    pub fn specificity(&self) -> u8 {
        u8::from(!matches!(self.model_family, Constraint::Any))
            + u8::from(!matches!(self.harness, Constraint::Any))
            + u8::from(self.modalities.is_some())
            + u8::from(self.posture.is_some())
    }
}

/// A variant's body: small content inline, larger content by reference
/// (rev 2.3 — the threshold is automatic at mint, doc `02`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariantBody {
    /// Inline content (≤ [`crate::INLINE_THRESHOLD_BYTES`]).
    Inline {
        /// MIME type of `data`.
        content_type: String,
        /// The content itself.
        data: String,
    },
    /// Bytes by reference with integrity.
    Media(MediaRef),
}

/// One projection of the artifact for a class of consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    /// Which contexts this variant serves.
    #[serde(rename = "match")]
    pub match_expr: MatchExpr,
    /// What those consumers receive.
    pub body: VariantBody,
    /// Advisory freshness window in ms for resolutions served from this
    /// variant (G-3). `None` ⇒ [`crate::DEFAULT_REVALIDATE_MS`]. Short for
    /// sensitive artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revalidate_after_ms: Option<u64>,
}

/// A token's lifecycle disposition at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// Live and servable.
    Active,
    /// Past its `expires_at`.
    Expired,
    /// Withdrawn; tombstoned, never recycled (I-6).
    Revoked {
        /// When it was revoked.
        at: Timestamp,
    },
    /// Replaced by a corrected token; late resolvers follow the pointer.
    Superseded {
        /// The replacement token.
        by: Token,
    },
}

/// The stored, retrievable record behind a token (doc `02 §2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionManifest {
    /// Envelope schema version — pinned, additive-only.
    pub schema: u16,
    /// The token this manifest belongs to.
    pub token: Token,
    /// Permanent identity of the target artifact.
    pub target: CanonicalUrl,
    /// Who minted — attribution, independent of authorship.
    pub sharer: Sharer,
    /// Where this share lives.
    pub channel: Channel,
    /// When it was minted.
    pub minted_at: Timestamp,
    /// Mint-time snapshot of the target (never scraped — I-3).
    #[serde(default, skip_serializing_if = "meta_is_empty")]
    pub meta: TargetMeta,
    /// Parent token when this was minted as a delegation child (lineage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Token>,
    /// The artifact's bytes at mint, content-addressed (doc `18 §3`) —
    /// set by snapshot minting, immutable like the rest of the core.
    /// `read`/`search` prefer this over the live target: what you grep is
    /// what was minted, wherever the blobs replicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<MediaRef>,
    /// The projections. Always ≥1; exactly one catch-all guaranteed at mint.
    pub variants: Vec<Variant>,
    /// Capability-URL semantics (CP-11): a private token IS its own
    /// credential — minted long enough to be unguessable, and refused by
    /// public surfaces (unfurls, social renderers). Immutable + signed.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub private: bool,
    /// The consumption contract, if the author declared one at mint
    /// (doc `19 §4.2`): which regions a consumer must reach. Immutable
    /// core — signed; absent for the (default) contract-free mint, so
    /// pre-contract manifests keep their exact canonical bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<crate::Contract>,
    /// Author signature over the immutable core (CP-11); set at mint by
    /// hosts that hold an identity. NOT itself part of the signed bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureBlock>,
    /// Mutable-section version — CAS target for lifecycle mutations (C-9).
    pub version: u32,
    /// Cosmetic: campaign label (LWW).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    /// Cosmetic: labels (LWW).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    /// Lifecycle: expiry, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
    /// Lifecycle: revocation instant, if revoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<Timestamp>,
    /// Lifecycle: replacement pointer, if superseded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<Token>,
}

fn meta_is_empty(m: &TargetMeta) -> bool {
    *m == TargetMeta::default()
}

/// A detached signature over the manifest's IMMUTABLE core (doc 14
/// CP-11): mutations never touch what was signed, so lifecycle churn
/// never invalidates it. Set by the host at mint; verified by anyone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureBlock {
    /// Signature algorithm — `ed25519` today.
    pub alg: String,
    /// The signer's verifying key, hex-encoded (32 bytes).
    pub key: String,
    /// The signature, hex-encoded (64 bytes).
    pub sig: String,
}

/// Apply one mutable-section change to a manifest — THE mutation semantic,
/// shared by `ManifestFold`, every backend, and reconstruct so they can
/// never disagree (R-4's precondition). Lifecycle changes bump `version`
/// (the C-9 CAS baseline); cosmetic changes don't. CAS checking is the
/// store's job at its commit point; this function only applies.
pub fn apply_change(
    manifest: &mut AttributionManifest,
    change: &crate::log::Change,
    at: Timestamp,
) {
    use crate::log::Change;
    match change {
        Change::Revoked => {
            manifest.revoked_at = Some(at);
            manifest.version += 1;
        }
        Change::Superseded { by } => {
            manifest.superseded_by = Some(*by);
            manifest.version += 1;
        }
        Change::ExpirySet { expires_at } => {
            manifest.expires_at = *expires_at;
            manifest.version += 1;
        }
        Change::CampaignSet { campaign } => manifest.campaign.clone_from(campaign),
        Change::LabelSet { key, value } => {
            manifest.labels.insert(key.clone(), value.clone());
        }
        Change::LabelUnset { key } => {
            manifest.labels.remove(key);
        }
    }
}

impl AttributionManifest {
    /// Lifecycle disposition at `now`. Precedence: revoked > superseded >
    /// expired > active (a revoked token stays revoked even if also
    /// superseded — revocation is the stronger claim).
    #[must_use]
    pub fn disposition(&self, now: Timestamp) -> Disposition {
        if let Some(at) = self.revoked_at {
            return Disposition::Revoked { at };
        }
        if let Some(by) = self.superseded_by {
            return Disposition::Superseded { by };
        }
        match self.expires_at {
            Some(exp) if exp <= now => Disposition::Expired,
            _ => Disposition::Active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modality_set_algebra() {
        let ctx = ModalitySet::TEXT.with(ModalitySet::VISION);
        assert!(ctx.contains(ModalitySet::VISION));
        assert!(ctx.contains(ModalitySet::empty()));
        assert!(!ctx.contains(ModalitySet::BROWSER));
        assert!(!ctx.contains(ModalitySet::VISION.with(ModalitySet::AUDIO)));
    }

    #[test]
    fn catch_all_and_specificity() {
        assert!(MatchExpr::any().is_catch_all());
        assert_eq!(MatchExpr::any().specificity(), 0);
        let m = MatchExpr {
            model_family: Constraint::OneOf(vec!["claude".into()]),
            modalities: Some(ModalitySet::VISION),
            ..MatchExpr::default()
        };
        assert!(!m.is_catch_all());
        assert_eq!(m.specificity(), 2);
    }

    #[test]
    fn disposition_precedence() {
        let now = Timestamp::from_unix_ms(10_000);
        let mut entropy = |buf: &mut [u8]| {
            buf.fill(7);
            Ok(())
        };
        let m = crate::mint(
            crate::MintSpec::new(
                CanonicalUrl::new("file:///tmp/x.md").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &crate::MintOptions::default(),
            &mut entropy,
            now,
        )
        .unwrap();

        assert_eq!(m.disposition(now), Disposition::Active);

        let mut expired = m.clone();
        expired.expires_at = Some(now);
        assert_eq!(expired.disposition(now), Disposition::Expired);
        assert_eq!(
            expired.disposition(Timestamp::from_unix_ms(9_999)),
            Disposition::Active,
            "expiry is exclusive of earlier instants"
        );

        let other = Token::parse("zzz").unwrap();
        let mut superseded = expired.clone();
        superseded.superseded_by = Some(other);
        assert_eq!(
            superseded.disposition(now),
            Disposition::Superseded { by: other }
        );

        let mut revoked = superseded;
        revoked.revoked_at = Some(now);
        assert_eq!(
            revoked.disposition(now),
            Disposition::Revoked { at: now },
            "revocation outranks supersession and expiry"
        );
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let now = Timestamp::from_unix_ms(1);
        let mut entropy = |buf: &mut [u8]| {
            buf.fill(9);
            Ok(())
        };
        let m = crate::mint(
            crate::MintSpec::new(
                CanonicalUrl::new("ws://a/b").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::new("subagent/pricing").unwrap(),
            )
            .variant(
                MatchExpr {
                    modalities: Some(ModalitySet::VISION),
                    ..MatchExpr::default()
                },
                VariantBody::Inline {
                    content_type: "text/markdown".into(),
                    data: "see the chart".into(),
                },
            ),
            &crate::MintOptions::default(),
            &mut entropy,
            now,
        )
        .unwrap();
        let json = serde_json::to_string(&m).unwrap();
        let back: AttributionManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.schema, MANIFEST_SCHEMA_VERSION);
    }
}
