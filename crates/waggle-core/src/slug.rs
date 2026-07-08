//! Domain slugs: [`Sharer`], [`Channel`], and [`Stage`] — validated,
//! normalized newtypes so a bare `String` never carries domain meaning
//! (design docs `02 §2`, `13 §3`: a bare string in a public signature is a
//! review rejection).

use core::fmt;
use serde::{de, Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Why a slug was rejected.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlugError {
    /// Empty after trimming, or longer than the field's cap.
    #[error("{field} length {len} outside 1..={max}")]
    Length {
        /// Which field rejected the input.
        field: &'static str,
        /// Observed length after trimming.
        len: usize,
        /// The field's maximum length.
        max: usize,
    },
    /// A character outside `[a-z0-9._/-]` (after lowercasing).
    #[error("{field} contains a character outside [a-z0-9._/-]")]
    Charset {
        /// Which field rejected the input.
        field: &'static str,
    },
}

/// Trim, lowercase, and validate a slug against the shared charset.
fn normalize(field: &'static str, raw: &str, max: usize) -> Result<String, SlugError> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() || s.len() > max {
        return Err(SlugError::Length {
            field,
            len: s.len(),
            max,
        });
    }
    let ok = s.bytes().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.' | b'/')
    });
    if ok {
        Ok(s)
    } else {
        Err(SlugError::Charset { field })
    }
}

macro_rules! slug_newtype {
    ($(#[$doc:meta])* $name:ident, $field:literal, $max:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Normalize (trim, lowercase) and validate.
            pub fn new(raw: &str) -> Result<Self, SlugError> {
                normalize($field, raw, $max).map(Self)
            }

            /// The normalized slug.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Self::new(&s).map_err(de::Error::custom)
            }
        }
    };
}

slug_newtype!(
    /// Who performed a distribution act. Deliberately *not* an authenticated
    /// identity in the core — hosts bind sharers to their own auth (02 §2).
    Sharer,
    "sharer",
    64
);

slug_newtype!(
    /// Where a share lives: `x-twitter`, `slack`, `qr-event`,
    /// `subagent/researcher`, … One token per channel is the design's rule;
    /// the API enforces it by taking exactly one.
    Channel,
    "channel",
    64
);

slug_newtype!(
    /// A funnel stage. Well-known constructors below; custom stages are any
    /// valid slug — the open vocabulary is what extends the funnel past the
    /// click into the consuming product's own lifecycle.
    Stage,
    "stage",
    32
);

impl Channel {
    /// The default channel for agent handoffs when none is given
    /// (one-call mint, design doc `17 §1` rule 3).
    #[must_use]
    pub fn subagent_general() -> Self {
        Self("subagent/general".to_owned())
    }
}

macro_rules! well_known_stages {
    ($(($fn_name:ident, $lit:literal, $doc:literal)),+ $(,)?) => {
        impl Stage {
            $(
                #[doc = $doc]
                #[must_use]
                pub fn $fn_name() -> Self { Self($lit.to_owned()) }
            )+

            /// Every well-known stage, in funnel order.
            #[must_use]
            pub fn well_known() -> Vec<Self> {
                vec![$(Self($lit.to_owned())),+]
            }
        }
    };
}

well_known_stages![
    (impression, "impression", "An unfurl bot fetched the link."),
    (click, "click", "A human followed the link."),
    (resolve, "resolve", "A consumer fetched a projection."),
    (
        assess,
        "assess",
        "The consumer inspected before committing."
    ),
    (consent, "consent", "A human approved the action."),
    (install, "install", "A delta was installed."),
    (signin, "signin", "Identity was established."),
    (credential, "credential", "A credential was activated."),
    (run, "run", "The referenced work was executed."),
    (
        repeat,
        "repeat",
        "A repeat execution — the retention signal."
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_trims_and_lowercases() {
        let c = Channel::new("  X-Twitter ").unwrap();
        assert_eq!(c.as_str(), "x-twitter");
        // Idempotent: normalizing the normalized form changes nothing.
        assert_eq!(Channel::new(c.as_str()).unwrap(), c);
    }

    #[test]
    fn charset_and_length_are_enforced() {
        assert!(matches!(Sharer::new(""), Err(SlugError::Length { .. })));
        assert!(matches!(
            Sharer::new("has space"),
            Err(SlugError::Charset { .. })
        ));
        assert!(matches!(
            Stage::new("Ünicode"),
            Err(SlugError::Charset { .. })
        ));
        let long = "a".repeat(65);
        assert!(matches!(Channel::new(&long), Err(SlugError::Length { .. })));
        // Slash is allowed: subagent role channels depend on it.
        assert!(Channel::new("subagent/data-check").is_ok());
    }

    #[test]
    fn well_known_stages_are_valid_slugs() {
        for s in Stage::well_known() {
            assert_eq!(Stage::new(s.as_str()).unwrap(), s);
        }
        assert_eq!(Stage::run().as_str(), "run");
    }

    #[test]
    fn serde_validates_on_the_way_in() {
        let ok: Channel = serde_json::from_str("\"slack\"").unwrap();
        assert_eq!(ok.as_str(), "slack");
        assert!(serde_json::from_str::<Channel>("\"bad channel!\"").is_err());
    }
}
