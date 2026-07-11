//! Trust (design doc `14 CP-11`): Ed25519 signatures over the manifest's
//! **immutable core** — the three-zone design's payoff: lifecycle and
//! cosmetic mutations never invalidate a signature, because they never
//! touch what was signed.
//!
//! Sans-I/O as always: keys are parameters (hosts load them; the CLI
//! keeps a seed at `~/.waggle/identity`), and verification is a pure
//! function of the manifest. Canonical bytes are the serde encoding of
//! the immutable-core fields in declaration order — deterministic
//! because every map in the core is a `BTreeMap` and field order is
//! fixed by the struct.

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde::Serialize;

use crate::manifest::{AttributionManifest, SignatureBlock, Variant};
use crate::TargetMeta;
use crate::{CanonicalUrl, Channel, MediaRef, Sharer, Timestamp, Token};

/// The immutable core, borrowed — exactly the zone a signature covers.
/// Adding a mutable field here would break the "mutations never
/// invalidate" property; the compiler makes that a conscious act.
#[derive(Serialize)]
struct ImmutableCore<'m> {
    schema: u16,
    token: Token,
    target: &'m CanonicalUrl,
    sharer: &'m Sharer,
    channel: &'m Channel,
    minted_at: Timestamp,
    meta: &'m TargetMeta,
    parent: Option<Token>,
    content: &'m Option<MediaRef>,
    variants: &'m [Variant],
    private: bool,
    /// Skipped when absent so every pre-contract manifest keeps its
    /// exact canonical bytes — existing signatures stay valid (19 §4.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    contract: &'m Option<crate::Contract>,
}

/// The bytes a signature covers.
///
/// # Panics
/// Never in practice: the immutable core is plain data with no
/// fallible serialization path; the `expect` documents the invariant.
#[must_use]
pub fn canonical_core_bytes(m: &AttributionManifest) -> Vec<u8> {
    let core = ImmutableCore {
        schema: m.schema,
        token: m.token,
        target: &m.target,
        sharer: &m.sharer,
        channel: &m.channel,
        minted_at: m.minted_at,
        meta: &m.meta,
        parent: m.parent,
        content: &m.content,
        variants: &m.variants,
        private: m.private,
        contract: &m.contract,
    };
    serde_json::to_vec(&core).expect("core fields always serialize")
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Sign a manifest's immutable core. The host sets the result on
/// `manifest.signature` before appending.
#[must_use]
pub fn sign_manifest(manifest: &AttributionManifest, key: &SigningKey) -> SignatureBlock {
    let sig = key.sign(&canonical_core_bytes(manifest));
    SignatureBlock {
        alg: "ed25519".to_owned(),
        key: hex(key.verifying_key().as_bytes()),
        sig: hex(&sig.to_bytes()),
    }
}

/// What verification concluded — three-valued on purpose: absent is not
/// invalid, and consumers choose their own policy per trust context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    /// No signature block present.
    Unsigned,
    /// Present and correct; carries the signer's public key (hex).
    Valid {
        /// The verifying key, hex-encoded.
        key: String,
    },
    /// Present and WRONG — tampered core, wrong key, or malformed block.
    Invalid,
}

/// Verify a manifest's signature over its immutable core.
#[must_use]
pub fn verify_manifest(manifest: &AttributionManifest) -> SignatureStatus {
    let Some(block) = &manifest.signature else {
        return SignatureStatus::Unsigned;
    };
    if block.alg != "ed25519" {
        return SignatureStatus::Invalid;
    }
    let (Some(key_bytes), Some(sig_bytes)) = (unhex(&block.key), unhex(&block.sig)) else {
        return SignatureStatus::Invalid;
    };
    let (Ok(key_arr), Ok(sig_arr)) = (
        <[u8; 32]>::try_from(key_bytes.as_slice()),
        <[u8; 64]>::try_from(sig_bytes.as_slice()),
    ) else {
        return SignatureStatus::Invalid;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key_arr) else {
        return SignatureStatus::Invalid;
    };
    let sig = Signature::from_bytes(&sig_arr);
    if key.verify(&canonical_core_bytes(manifest), &sig).is_ok() {
        SignatureStatus::Valid {
            key: block.key.clone(),
        }
    } else {
        SignatureStatus::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MintOptions, MintSpec};

    fn minted() -> AttributionManifest {
        let mut entropy = |b: &mut [u8]| {
            b.fill(42);
            Ok(())
        };
        crate::mint(
            MintSpec::new(
                CanonicalUrl::new("ws://trust/artifact").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1),
        )
        .unwrap()
    }

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32]) // fixed seed: these are VECTORS
    }

    #[test]
    fn signature_round_trip_vector() {
        let mut m = minted();
        let block = sign_manifest(&m, &key());
        assert_eq!(block.alg, "ed25519");
        // The vector: fixed seed + fixed entropy + fixed clock ⇒ exact
        // signature, forever. A change here is a canonical-bytes break.
        assert_eq!(
            block.key,
            "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
        );
        m.signature = Some(block);
        assert_eq!(
            verify_manifest(&m),
            SignatureStatus::Valid {
                key: "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c".into()
            }
        );
    }

    #[test]
    fn mutations_never_invalidate_the_signature() {
        let mut m = minted();
        m.signature = Some(sign_manifest(&m, &key()));
        // Lifecycle + cosmetic churn — the mutable zones.
        crate::apply_change(&mut m, &crate::Change::Revoked, Timestamp::from_unix_ms(9));
        crate::apply_change(
            &mut m,
            &crate::Change::LabelSet {
                key: "team".into(),
                value: "research".into(),
            },
            Timestamp::from_unix_ms(10),
        );
        assert!(
            matches!(verify_manifest(&m), SignatureStatus::Valid { .. }),
            "the three-zone design: mutations don't touch what was signed"
        );
    }

    #[test]
    fn contract_free_canonical_bytes_never_mention_the_field() {
        // The 19 §4.2 compatibility rule, checked at the byte level: a
        // manifest without a contract serializes exactly as it did before
        // the field existed (the pinned vector in
        // `signature_round_trip_vector` proves the same end to end).
        let m = minted();
        let bytes = canonical_core_bytes(&m);
        assert!(
            !String::from_utf8(bytes).unwrap().contains("contract"),
            "absent contract must leave canonical bytes untouched"
        );
    }

    #[test]
    fn contract_bearing_manifests_sign_and_tamper_detect() {
        let mut entropy = |b: &mut [u8]| {
            b.fill(42);
            Ok(())
        };
        let contract = crate::Contract::new(
            vec![crate::Region::new(Some("Pricing".into()), 10, 40, 0).unwrap()],
            1000,
        )
        .unwrap();
        let mut m = crate::mint(
            MintSpec::new(
                CanonicalUrl::new("ws://trust/contracted").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            )
            .contract(contract),
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1),
        )
        .unwrap();
        m.signature = Some(sign_manifest(&m, &key()));
        assert!(matches!(verify_manifest(&m), SignatureStatus::Valid { .. }));
        // The contract is signed: tampering with it invalidates.
        m.contract = None;
        assert_eq!(verify_manifest(&m), SignatureStatus::Invalid);
    }

    #[test]
    fn tampered_core_is_invalid_absent_is_unsigned() {
        let mut m = minted();
        assert_eq!(verify_manifest(&m), SignatureStatus::Unsigned);
        m.signature = Some(sign_manifest(&m, &key()));
        // Tamper with the IMMUTABLE core after signing.
        m.sharer = Sharer::new("impostor").unwrap();
        assert_eq!(verify_manifest(&m), SignatureStatus::Invalid);
    }
}
