//! The [`SharePackage`]: everything a channel artifact renders from,
//! assembled **exclusively** from the manifest's mint-time snapshot
//! (invariant I-3 — waggle never scrapes a target at share or unfurl
//! time; what you approved at mint is what the world sees).

use serde::Serialize;
use waggle_core::AttributionManifest;

/// The render inputs for one share. Pure data — building one performs no
/// I/O, and rendering from one is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharePackage {
    /// The short link consumers follow: `{base}/{token}`.
    pub url: String,
    /// Title from the mint-time snapshot; the token when absent.
    pub title: String,
    /// Description from the snapshot; empty when absent.
    pub description: String,
    /// Preview image URL from the snapshot, when one was declared.
    pub image_url: Option<String>,
    /// The bare token, for artifacts that print it (QR captions).
    pub token: String,
}

impl SharePackage {
    /// Assemble from a manifest and the host's resolver base URL (no
    /// trailing slash needed — one is trimmed).
    /// `None` for private tokens: a capability URL must never be
    /// rendered onto a public surface (CP-11).
    #[must_use]
    pub fn from_manifest_public(manifest: &AttributionManifest, base_url: &str) -> Option<Self> {
        (!manifest.private).then(|| Self::from_manifest(manifest, base_url))
    }

    /// Assemble from a manifest and the host's resolver base URL (no
    /// trailing slash needed — one is trimmed). Prefer
    /// [`Self::from_manifest_public`] on public surfaces.
    #[must_use]
    pub fn from_manifest(manifest: &AttributionManifest, base_url: &str) -> Self {
        let token = manifest.token.as_str().to_owned();
        let url = format!("{}/{token}", base_url.trim_end_matches('/'));
        let meta = &manifest.meta;
        Self {
            url,
            title: meta.title.clone().unwrap_or_else(|| token.clone()),
            description: meta.description.clone().unwrap_or_default(),
            image_url: meta.image_url.as_ref().map(|u| u.as_str().to_owned()),
            token,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waggle_core::{CanonicalUrl, Channel, MintOptions, MintSpec, Sharer, Timestamp};

    fn minted(with_meta: bool) -> AttributionManifest {
        let mut entropy = |b: &mut [u8]| {
            b.fill(17);
            Ok(())
        };
        let mut spec = MintSpec::new(
            CanonicalUrl::new("https://example.com/report").unwrap(),
            Sharer::new("lead").unwrap(),
            Channel::new("slack/eng").unwrap(),
        );
        if with_meta {
            spec = spec.meta(waggle_core::TargetMeta {
                title: Some("Q3 Market Report".into()),
                description: Some("Findings from the agent swarm.".into()),
                image_url: Some(CanonicalUrl::new("https://example.com/og.png").unwrap()),
                labels: std::collections::BTreeMap::new(),
            });
        }
        waggle_core::mint(
            spec,
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1),
        )
        .unwrap()
    }

    #[test]
    fn package_uses_snapshot_never_scrapes() {
        let m = minted(true);
        let p = SharePackage::from_manifest(&m, "https://wgl.example/");
        assert_eq!(p.title, "Q3 Market Report");
        assert_eq!(p.url, format!("https://wgl.example/{}", m.token));
        assert_eq!(p.image_url.as_deref(), Some("https://example.com/og.png"));
    }

    #[test]
    fn bare_manifest_falls_back_to_the_token() {
        let m = minted(false);
        let p = SharePackage::from_manifest(&m, "https://wgl.example");
        assert_eq!(p.title, m.token.as_str());
        assert!(p.description.is_empty());
        assert!(p.image_url.is_none());
    }
}
