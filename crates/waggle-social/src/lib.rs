//! # waggle-social — the human face of the same token
//!
//! Pure renderers from a [`SharePackage`] (assembled exclusively from the
//! manifest's **mint-time snapshot** — invariant I-3, the system never
//! scrapes targets) to channel artifacts: Slack/Discord lines, X posts
//! under budget, email, Markdown, an Open Graph meta block for unfurl
//! bots, and SVG QR codes for the qr-event channel (design doc `05`).
//!
//! Everything here is a pure function: same inputs, byte-identical
//! artifact, always — the CP-8 purity gate. No clock, no network, no
//! store; hosts fetch the manifest, this crate only renders.
//!
//! Social minting is a capability of the primitive, not a battlefront
//! (design doc `01 §3`): the same token that hands a subagent its variant
//! unfurls truthfully in Slack.

mod channels;
mod og;
mod package;
#[cfg(feature = "qr")]
mod qr;

pub use channels::{email, markdown, slack_message, x_post};
pub use og::og_meta;
pub use package::SharePackage;
#[cfg(feature = "qr")]
pub use qr::{qr_svg, QrError};
