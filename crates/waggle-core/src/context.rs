//! The resolver's context: who is asking, and [`negotiate`] — the
//! generalized content negotiation that turns a consumer hint into a
//! [`ResolverContext`] (design docs `02 §2`, `06 §1`).
//!
//! Rich extraction (harness metadata, A2A Agent Cards) lives in
//! `waggle-agent`; the core knows only the neutral schema and the
//! user-agent classes every deployment needs.

use serde::{Deserialize, Serialize};

use crate::manifest::{ModalitySet, Posture};

/// The coarse consumer class — the *maximum* actor granularity the event
/// log may ever hold (invariant I-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsumerKind {
    /// An unfurl/preview bot.
    Bot,
    /// A human in a browser.
    Human,
    /// A terminal client (curl-class).
    Terminal,
    /// An AI agent presenting a context.
    Agent,
}

/// The neutral resolver-context schema — waggle's lingua franca. External
/// schemas (harness metadata, A2A cards) reach it through extractors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolverContext {
    /// The coarse consumer class.
    pub kind: ConsumerKind,
    /// Model family (`claude`, `gpt`, `gemini`, …), if declared. Families
    /// only — never versions or instance identifiers (I-7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_family: Option<String>,
    /// Harness (`claude-code`, `codex`, …), if declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// Modalities the consumer can exercise.
    #[serde(default)]
    pub modalities: ModalitySet,
    /// Execution posture.
    pub posture: Posture,
}

impl ResolverContext {
    /// A human in a browser — the 301 path.
    #[must_use]
    pub fn human() -> Self {
        Self {
            kind: ConsumerKind::Human,
            model_family: None,
            harness: None,
            modalities: ModalitySet::empty(),
            posture: Posture::Attended,
        }
    }

    /// An anonymous agent context with nothing declared — matches only
    /// catch-all and modality-free variants.
    #[must_use]
    pub fn anonymous_agent() -> Self {
        Self {
            kind: ConsumerKind::Agent,
            model_family: None,
            harness: None,
            modalities: ModalitySet::TEXT,
            posture: Posture::Headless,
        }
    }
}

/// What a consumer presented at the door.
#[derive(Debug, Clone)]
pub enum ConsumerHint<'a> {
    /// An HTTP `User-Agent` string (web and terminal consumers).
    UserAgent(&'a str),
    /// An explicit, already-built context (agents; extractor output).
    Explicit(ResolverContext),
}

/// Case-insensitive markers for unfurl/preview bots.
const BOT_MARKERS: &[&str] = &[
    "slackbot",
    "twitterbot",
    "facebookexternalhit",
    "discordbot",
    "linkedinbot",
    "whatsapp",
    "telegrambot",
    "googlebot",
    "bingbot",
    "embedly",
];

/// Case-insensitive markers for terminal clients.
const TERMINAL_MARKERS: &[&str] = &["curl", "wget", "httpie", "python-requests"];

/// Turn a hint into a context. Pure and total: unknown user agents are
/// humans (the safe default — disclosure, not projection).
#[must_use]
pub fn negotiate(hint: &ConsumerHint<'_>) -> ResolverContext {
    match hint {
        ConsumerHint::Explicit(ctx) => ctx.clone(),
        ConsumerHint::UserAgent(ua) => {
            let lower = ua.to_ascii_lowercase();
            let kind = if BOT_MARKERS.iter().any(|m| lower.contains(m)) {
                ConsumerKind::Bot
            } else if TERMINAL_MARKERS.iter().any(|m| lower.contains(m)) {
                ConsumerKind::Terminal
            } else {
                ConsumerKind::Human
            };
            ResolverContext {
                kind,
                model_family: None,
                harness: None,
                modalities: ModalitySet::empty(),
                posture: Posture::Attended,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_classes() {
        let bot = negotiate(&ConsumerHint::UserAgent("Slackbot-LinkExpanding 1.0"));
        assert_eq!(bot.kind, ConsumerKind::Bot);
        let term = negotiate(&ConsumerHint::UserAgent("curl/8.6.0"));
        assert_eq!(term.kind, ConsumerKind::Terminal);
        let human = negotiate(&ConsumerHint::UserAgent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari/605.1.15",
        ));
        assert_eq!(human.kind, ConsumerKind::Human);
        let unknown = negotiate(&ConsumerHint::UserAgent("SomethingNew/0.1"));
        assert_eq!(
            unknown.kind,
            ConsumerKind::Human,
            "unknown defaults to human"
        );
    }

    #[test]
    fn explicit_context_passes_through_unchanged() {
        let ctx = ResolverContext {
            kind: ConsumerKind::Agent,
            model_family: Some("claude".into()),
            harness: Some("claude-code".into()),
            modalities: ModalitySet::TEXT.with(ModalitySet::VISION),
            posture: Posture::Headless,
        };
        assert_eq!(negotiate(&ConsumerHint::Explicit(ctx.clone())), ctx);
    }

    #[test]
    fn context_serde_roundtrip() {
        let ctx = ResolverContext::anonymous_agent();
        let json = serde_json::to_string(&ctx).unwrap();
        let back: ResolverContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ctx);
    }
}
