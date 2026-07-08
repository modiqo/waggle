//! Targets and their metadata: [`CanonicalUrl`], the mint-time
//! [`TargetMeta`] snapshot (invariant I-3: the system never scrapes
//! targets), and [`MediaRef`] — bytes by reference with integrity
//! (design doc `02`, rev 2.3).

use std::collections::BTreeMap;

use core::fmt;
use serde::{de, Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Bodies at or under this many bytes may inline in a manifest; above it
/// they become a [`MediaRef`] into the content-addressed store. Chosen at
/// the range where `SQLite`'s small-blob reads beat the filesystem (02).
pub const INLINE_THRESHOLD_BYTES: usize = 64 * 1024;

/// Total manifest size cap — mint rejects manifests that exceed it (02).
pub const MANIFEST_SIZE_CAP_BYTES: usize = 256 * 1024;

/// Why a target URI was rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TargetError {
    /// Empty or longer than 2048 bytes.
    #[error("target uri length {0} outside 1..=2048")]
    Length(usize),
    /// Contains whitespace or control characters.
    #[error("target uri contains whitespace or control characters")]
    Charset,
}

/// The permanent identity of an artifact — a file path, workspace URI, or
/// URL. Never mutated by sharing; tokens point at it (02 §1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CanonicalUrl(String);

impl CanonicalUrl {
    /// Validate a target URI: non-empty, ≤2048 bytes, no whitespace or
    /// control characters. Scheme semantics belong to hosts, not the core.
    pub fn new(raw: &str) -> Result<Self, TargetError> {
        if raw.is_empty() || raw.len() > 2048 {
            return Err(TargetError::Length(raw.len()));
        }
        if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(TargetError::Charset);
        }
        Ok(Self(raw.to_owned()))
    }

    /// The URI as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CanonicalUrl {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::new(&s).map_err(de::Error::custom)
    }
}

/// Mint-time snapshot of what the target *is* — supplied by the minter,
/// never scraped (I-3). This is what unfurls render and what agents read
/// before resolving.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetMeta {
    /// Short human/agent-readable title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One-paragraph description of the artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional preview image for unfurl cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<CanonicalUrl>,
    /// Public labels — mint-time only; mutable labels live on the manifest.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

/// Bytes by reference: where they live, what they are, and the integrity
/// hash a resolver verifies after fetching (rev 2.3). Bytes never ride the
/// log or a tool response — delivery is out-of-band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRef {
    /// Where the bytes live (content-addressed store or external URL).
    pub uri: CanonicalUrl,
    /// MIME type of the referenced bytes.
    pub content_type: String,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 of the bytes, hex-encoded — verify what you fetched.
    pub sha256: Sha256Hex,
}

/// A lowercase-hex SHA-256 digest (64 chars), validated on construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Sha256Hex(String);

/// Why a digest string was rejected.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("sha256 must be 64 lowercase hex characters")]
pub struct Sha256Error;

impl Sha256Hex {
    /// Validate 64 lowercase hex characters.
    pub fn new(raw: &str) -> Result<Self, Sha256Error> {
        let ok = raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if ok {
            Ok(Self(raw.to_owned()))
        } else {
            Err(Sha256Error)
        }
    }

    /// The digest as lowercase hex.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Sha256Hex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::new(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_url_rules() {
        assert!(CanonicalUrl::new("ws://analysis/report.md").is_ok());
        assert!(CanonicalUrl::new("/tmp/analysis.md").is_ok());
        assert_eq!(CanonicalUrl::new(""), Err(TargetError::Length(0)));
        assert_eq!(CanonicalUrl::new("has space"), Err(TargetError::Charset));
        assert_eq!(CanonicalUrl::new("tab\there"), Err(TargetError::Charset));
        let long = "x".repeat(2049);
        assert!(matches!(
            CanonicalUrl::new(&long),
            Err(TargetError::Length(2049))
        ));
    }

    #[test]
    fn sha256_hex_validation() {
        let ok = "a".repeat(64);
        assert!(Sha256Hex::new(&ok).is_ok());
        assert_eq!(Sha256Hex::new("short"), Err(Sha256Error));
        let upper = "A".repeat(64);
        assert_eq!(
            Sha256Hex::new(&upper),
            Err(Sha256Error),
            "uppercase rejected — one canonical form"
        );
        let nonhex = "g".repeat(64);
        assert_eq!(Sha256Hex::new(&nonhex), Err(Sha256Error));
    }

    #[test]
    fn media_ref_serde_roundtrip() {
        let m = MediaRef {
            uri: CanonicalUrl::new("blob://ab/abcd").unwrap(),
            content_type: "image/png".to_owned(),
            size: 1024,
            sha256: Sha256Hex::new(&"0".repeat(64)).unwrap(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: MediaRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn target_meta_omits_empty_fields() {
        let json = serde_json::to_string(&TargetMeta::default()).unwrap();
        assert_eq!(
            json, "{}",
            "empty snapshot serializes empty — manifests stay small"
        );
    }
}
