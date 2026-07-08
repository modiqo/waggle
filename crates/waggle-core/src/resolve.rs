//! Resolution: manifest + context + now → [`Resolution`] (design doc `02`).
//!
//! Invariant I-4 *by signature*: this function takes a manifest it was
//! handed, a context, and a time value — it cannot read a store, cannot
//! write an event, cannot block a redirect. Recording is the host's
//! separate, asynchronous act.

use serde::Serialize;

use crate::context::ResolverContext;
use crate::manifest::{AttributionManifest, Disposition};
use crate::matcher::{select_variant, Selected};
use crate::time::Timestamp;

/// Default advisory freshness window when a variant declares none: 15
/// minutes. A resolution is knowledge, not a lease (G-3) — this is the
/// "re-resolve before acting" hint, not an enforcement mechanism.
pub const DEFAULT_REVALIDATE_MS: u64 = 15 * 60 * 1000;

/// What a consumer holds after resolving (doc `02 §2`, rev 2.1 G-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resolution<'m> {
    /// Lifecycle disposition at `as_of`.
    pub disposition: Disposition,
    /// The selected variant, borrowed from the manifest (zero-copy).
    /// `None` when the disposition withholds content (revoked) or —
    /// impossible for minted manifests — nothing matched.
    #[serde(skip)]
    pub variant: Option<Selected<'m>>,
    /// The instant this resolution reflects. Always present: resolutions
    /// are point-in-time and say so.
    pub as_of: Timestamp,
    /// Advisory: re-resolve before acting after this instant (G-3).
    pub revalidate_after: Timestamp,
}

/// Resolve `manifest` for `ctx` at `now`. Pure, total, deterministic —
/// the sealed matcher does the selection; disposition gates what is served.
///
/// Serving rules: `Active` and `Expired` serve content (expiry policy —
/// redirect-with-warning vs tombstone — belongs to hosts, doc `02`);
/// `Superseded` serves content *and* the pointer (late resolvers follow
/// it); `Revoked` serves nothing.
#[must_use]
pub fn resolve<'m>(
    manifest: &'m AttributionManifest,
    ctx: &ResolverContext,
    now: Timestamp,
) -> Resolution<'m> {
    let disposition = manifest.disposition(now);
    let variant = match disposition {
        Disposition::Revoked { .. } => None,
        _ => select_variant(&manifest.variants, ctx),
    };
    let window = variant
        .and_then(|s| s.variant.revalidate_after_ms)
        .unwrap_or(DEFAULT_REVALIDATE_MS);
    Resolution {
        disposition,
        variant,
        as_of: now,
        revalidate_after: now.plus_ms(window),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalUrl, Channel, MintOptions, MintSpec, Sharer};

    fn minted() -> AttributionManifest {
        let mut entropy = |buf: &mut [u8]| {
            buf.fill(11);
            Ok(())
        };
        crate::mint(
            MintSpec::new(
                CanonicalUrl::new("ws://a/report.md").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            Timestamp::from_unix_ms(1_000),
        )
        .unwrap()
    }

    #[test]
    fn g3_resolution_carries_freshness() {
        // 15 §5.1 `g3_resolution_carries_freshness`.
        let m = minted();
        let now = Timestamp::from_unix_ms(2_000);
        let r = resolve(&m, &ResolverContext::anonymous_agent(), now);
        assert_eq!(r.as_of, now);
        assert_eq!(r.revalidate_after, now.plus_ms(DEFAULT_REVALIDATE_MS));
        assert!(
            r.variant.is_some(),
            "minted manifests always serve (catch-all)"
        );
    }

    #[test]
    fn variant_declared_window_overrides_default() {
        let mut m = minted();
        m.variants[0].revalidate_after_ms = Some(1_000);
        let now = Timestamp::from_unix_ms(0);
        let r = resolve(&m, &ResolverContext::human(), now);
        assert_eq!(r.revalidate_after, Timestamp::from_unix_ms(1_000));
    }

    #[test]
    fn revoked_serves_nothing_superseded_serves_with_pointer() {
        let mut m = minted();
        let now = Timestamp::from_unix_ms(5_000);

        let other = crate::Token::parse("next1").unwrap();
        m.superseded_by = Some(other);
        let r = resolve(&m, &ResolverContext::human(), now);
        assert_eq!(r.disposition, Disposition::Superseded { by: other });
        assert!(
            r.variant.is_some(),
            "superseded still serves; the pointer travels with it"
        );

        m.revoked_at = Some(now);
        let r = resolve(&m, &ResolverContext::human(), now);
        assert_eq!(r.disposition, Disposition::Revoked { at: now });
        assert!(r.variant.is_none(), "revoked serves nothing");
    }

    #[test]
    fn resolve_is_pure_same_inputs_same_output() {
        let m = minted();
        let ctx = ResolverContext::anonymous_agent();
        let now = Timestamp::from_unix_ms(9);
        assert_eq!(resolve(&m, &ctx, now), resolve(&m, &ctx, now));
    }
}
