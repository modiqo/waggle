//! Events: payload-free by construction (invariant I-1) and fixed-width by
//! consequence — which is what makes the `SoA` log's torn-read safety
//! provable (design docs `02 §2`, `15 §2`).

use serde::{Deserialize, Serialize};

use crate::context::{ConsumerKind, ResolverContext};
use crate::slug::Stage;
use crate::time::Timestamp;
use crate::token::Token;

/// Per-token monotonic sequence number, assigned by the store at append
/// (contract C-3). Identity of a record is `(token, seq)` — the dedup key
/// (C-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Seq(pub u32);

/// Coarse model-family class — the *maximum* granularity events may hold
/// (I-7). Families, never versions or instance identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FamilyClass {
    /// Not declared.
    None,
    /// Anthropic Claude family.
    Claude,
    /// `OpenAI` GPT family.
    Gpt,
    /// Google Gemini family.
    Gemini,
    /// Any other declared family.
    Other,
}

/// Coarse harness class (same discipline as [`FamilyClass`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessClass {
    /// Not declared.
    None,
    /// Claude Code.
    ClaudeCode,
    /// `OpenAI` Codex.
    Codex,
    /// Any other declared harness.
    Other,
}

/// The actor dimensions an event may carry — three coarse classes, packed
/// into one byte in the analytical log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorClass {
    /// Bot, human, terminal, or agent.
    pub kind: ConsumerKind,
    /// Coarse model family (agents only; `None` otherwise).
    pub family: FamilyClass,
    /// Coarse harness (agents only; `None` otherwise).
    pub harness: HarnessClass,
}

impl ActorClass {
    /// Classify a resolver context — the lossy, deliberate downgrade from
    /// context to analytics dimension (I-7 enforced at the boundary).
    #[must_use]
    pub fn from_context(ctx: &ResolverContext) -> Self {
        let family = match ctx
            .model_family
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None => FamilyClass::None,
            Some("claude") => FamilyClass::Claude,
            Some("gpt") => FamilyClass::Gpt,
            Some("gemini") => FamilyClass::Gemini,
            Some(_) => FamilyClass::Other,
        };
        let harness = match ctx
            .harness
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None => HarnessClass::None,
            Some("claude-code") => HarnessClass::ClaudeCode,
            Some("codex") => HarnessClass::Codex,
            Some(_) => HarnessClass::Other,
        };
        Self {
            kind: ctx.kind,
            family,
            harness,
        }
    }

    /// Pack into one byte: kind (2 bits) | family (3 bits) | harness (3
    /// bits) — the `SoA` column encoding (doc `03 §4`).
    #[must_use]
    pub fn code(self) -> u8 {
        let k = match self.kind {
            ConsumerKind::Bot => 0u8,
            ConsumerKind::Human => 1,
            ConsumerKind::Terminal => 2,
            ConsumerKind::Agent => 3,
        };
        let f = match self.family {
            FamilyClass::None => 0u8,
            FamilyClass::Claude => 1,
            FamilyClass::Gpt => 2,
            FamilyClass::Gemini => 3,
            FamilyClass::Other => 4,
        };
        let h = match self.harness {
            HarnessClass::None => 0u8,
            HarnessClass::ClaudeCode => 1,
            HarnessClass::Codex => 2,
            HarnessClass::Other => 3,
        };
        k | (f << 2) | (h << 5)
    }
}

/// One thing that happened to a token. **No payload field exists** — the
/// type system, not policy, keeps recipient data out of analytics (I-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// The token the event applies to.
    pub token: Token,
    /// The funnel stage.
    pub stage: Stage,
    /// Coarse actor dimensions.
    pub actor: ActorClass,
    /// When it happened.
    pub at: Timestamp,
    /// Per-token monotonic sequence (store-assigned).
    pub seq: Seq,
    /// Which manifest variant served a resolve, if this event is one —
    /// manifest-referencing, so I-1-compatible (doc `02`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<u8>,
    /// Which declared contract regions this access touched, as a bitmask
    /// indexing the manifest's [`crate::Contract`] (doc `19 §4.2`) —
    /// manifest-referencing exactly like `variant`, so I-1-compatible:
    /// positions into a signed declaration, never bytes. Absent on
    /// contract-free tokens and on pre-contract logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regions: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_codes_are_distinct_and_stable() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in [
            ConsumerKind::Bot,
            ConsumerKind::Human,
            ConsumerKind::Terminal,
            ConsumerKind::Agent,
        ] {
            for family in [
                FamilyClass::None,
                FamilyClass::Claude,
                FamilyClass::Gpt,
                FamilyClass::Gemini,
                FamilyClass::Other,
            ] {
                for harness in [
                    HarnessClass::None,
                    HarnessClass::ClaudeCode,
                    HarnessClass::Codex,
                    HarnessClass::Other,
                ] {
                    assert!(seen.insert(
                        ActorClass {
                            kind,
                            family,
                            harness
                        }
                        .code()
                    ));
                }
            }
        }
        assert_eq!(seen.len(), 4 * 5 * 4);
    }

    #[test]
    fn classification_is_coarse_by_construction() {
        let mut ctx = ResolverContext::anonymous_agent();
        ctx.model_family = Some("claude-fable-5.1-preview".into()); // a version string
        let actor = ActorClass::from_context(&ctx);
        assert_eq!(
            actor.family,
            FamilyClass::Other,
            "unknown strings bucket to Other — versions never survive"
        );
        ctx.model_family = Some("Claude".into());
        assert_eq!(ActorClass::from_context(&ctx).family, FamilyClass::Claude);
    }

    #[test]
    fn event_serde_roundtrip() {
        let e = Event {
            token: Token::parse("abc123").unwrap(),
            stage: Stage::resolve(),
            actor: ActorClass::from_context(&ResolverContext::human()),
            at: Timestamp::from_unix_ms(9),
            seq: Seq(4),
            variant: Some(2),
            regions: Some(0b101),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
    }
}
