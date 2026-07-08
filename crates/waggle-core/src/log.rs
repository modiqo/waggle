//! The log record: one append-only stream carries events **and** manifest
//! lifecycle (design doc `04 §1`). Minting itself is a record — the
//! manifest table anywhere in the system is a fold over this stream,
//! rebuildable, never the truth.

use serde::{Deserialize, Serialize};

use crate::event::{Event, Seq};
use crate::manifest::AttributionManifest;
use crate::time::Timestamp;
use crate::token::Token;

/// A change to a manifest's mutable sections. Lifecycle changes are CAS
/// (C-9, checked at the store's commit point); cosmetic changes are LWW by
/// commit order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Change {
    /// Lifecycle: withdraw the token (tombstone, cascades to children).
    Revoked,
    /// Lifecycle: replace with a corrected token.
    Superseded {
        /// The replacement token.
        by: Token,
    },
    /// Lifecycle: set or clear expiry.
    ExpirySet {
        /// New expiry (`None` clears).
        expires_at: Option<Timestamp>,
    },
    /// Cosmetic: set or clear the campaign label.
    CampaignSet {
        /// New campaign (`None` clears).
        campaign: Option<String>,
    },
    /// Cosmetic: set one label.
    LabelSet {
        /// Label key.
        key: String,
        /// Label value.
        value: String,
    },
    /// Cosmetic: remove one label.
    LabelUnset {
        /// Label key.
        key: String,
    },
}

impl Change {
    /// Lifecycle changes require CAS (`expected_version`); cosmetic ones
    /// are LWW. The split is normative (C-9, doc `02`).
    #[must_use]
    pub fn is_lifecycle(&self) -> bool {
        matches!(
            self,
            Self::Revoked | Self::Superseded { .. } | Self::ExpirySet { .. }
        )
    }
}

/// One record in the append-only log. A closed enum: new kinds are
/// additive; folds ignore kinds they don't know (doc `13 §4`), which is
/// what keeps the replay promise compatible across schema growth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "record")]
pub enum LogRecord {
    /// A token was born: the full manifest is the payload (doc `04 §1`).
    Minted {
        /// The manifest as minted (`version == 1`).
        manifest: Box<AttributionManifest>,
    },
    /// A manifest's mutable sections changed.
    Mutation {
        /// The token whose manifest changed.
        token: Token,
        /// When.
        at: Timestamp,
        /// Per-token sequence (store-assigned; dedup key with `token`).
        seq: Seq,
        /// What changed.
        change: Change,
    },
    /// A funnel event (payload-free — I-1).
    Event(Event),
}

impl LogRecord {
    /// The token this record belongs to.
    #[must_use]
    pub fn token(&self) -> Token {
        match self {
            Self::Minted { manifest } => manifest.token,
            Self::Mutation { token, .. } => *token,
            Self::Event(e) => e.token,
        }
    }

    /// The per-token sequence. `Minted` is always seq 0 — birth precedes
    /// everything (the store assigns 1.. to subsequent records).
    #[must_use]
    pub fn seq(&self) -> Seq {
        match self {
            Self::Minted { .. } => Seq(0),
            Self::Mutation { seq, .. } => *seq,
            Self::Event(e) => e.seq,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalUrl, Channel, MintOptions, MintSpec, Sharer};

    #[test]
    fn lifecycle_split_is_exact() {
        assert!(Change::Revoked.is_lifecycle());
        assert!(Change::Superseded {
            by: Token::parse("abc").unwrap()
        }
        .is_lifecycle());
        assert!(Change::ExpirySet { expires_at: None }.is_lifecycle());
        assert!(!Change::CampaignSet { campaign: None }.is_lifecycle());
        assert!(!Change::LabelSet {
            key: "k".into(),
            value: "v".into()
        }
        .is_lifecycle());
        assert!(!Change::LabelUnset { key: "k".into() }.is_lifecycle());
    }

    #[test]
    fn record_identity_and_serde() {
        let mut entropy = |buf: &mut [u8]| {
            buf.fill(3);
            Ok(())
        };
        let m = crate::mint(
            MintSpec::new(
                CanonicalUrl::new("ws://a/b").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            ),
            &MintOptions::default(),
            &mut entropy,
            crate::Timestamp::from_unix_ms(1),
        )
        .unwrap();
        let token = m.token;
        let rec = LogRecord::Minted {
            manifest: Box::new(m),
        };
        assert_eq!(rec.token(), token);
        assert_eq!(rec.seq(), Seq(0));

        let json = serde_json::to_string(&rec).unwrap();
        let back: LogRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rec);
        assert!(
            json.contains("\"record\":\"minted\""),
            "tagged for the JSONL wire format"
        );
    }
}
