//! The sealed variant matcher (design docs `03 §3`, `06 §2`).
//!
//! **Sealed by construction**: selection is a private algorithm behind one
//! free function — there is no trait to implement, no hook to override, no
//! configuration that alters ranking. "Same context → same projection" is
//! the trust claim agents depend on; determinism must not be forkable.
//! Expressiveness grows by adding *dimensions* to [`MatchExpr`] — a
//! visible, versioned act — never by swapping algorithms.
//!
//! The algorithm, normative (`06 §2`):
//! 1. a variant **matches** iff every constrained dimension accepts the
//!    context;
//! 2. **specificity** = number of constrained dimensions (0–4);
//! 3. highest specificity wins; ties break by **declaration order**;
//! 4. mint guarantees a catch-all, so over minted manifests selection is
//!    total (hostile inputs yield `None`, never a panic).

use crate::context::ResolverContext;
use crate::manifest::{Constraint, MatchExpr, Variant};

/// The outcome of selection: which variant, and where it sat. The index is
/// what `Event.variant` records (doc `02` — manifest-referencing, so
/// I-1-compatible) and what the authoring feedback loop keys on (`06 §6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selected<'m> {
    /// Position in the manifest's declaration order.
    pub index: u8,
    /// The winning variant.
    pub variant: &'m Variant,
}

/// Select the variant for `ctx` from `variants` (declaration order).
///
/// Returns `None` only when nothing matches — impossible for manifests
/// produced by [`crate::mint`], which guarantees a catch-all.
#[must_use]
pub fn select_variant<'m>(variants: &'m [Variant], ctx: &ResolverContext) -> Option<Selected<'m>> {
    let mut best: Option<(u8, Selected<'m>)> = None;
    for (i, variant) in variants.iter().enumerate() {
        if !matches(&variant.match_expr, ctx) {
            continue;
        }
        let spec = variant.match_expr.specificity();
        let candidate_is_better = match &best {
            None => true,
            // Strictly greater: on ties, the earlier declaration stands.
            Some((best_spec, _)) => spec > *best_spec,
        };
        if candidate_is_better {
            #[allow(clippy::cast_possible_truncation)] // manifests are small; index fits u8
            let selected = Selected {
                index: i as u8,
                variant,
            };
            best = Some((spec, selected));
        }
    }
    best.map(|(_, s)| s)
}

/// Does `expr` accept `ctx`? Private — part of the sealed algorithm.
fn matches(expr: &MatchExpr, ctx: &ResolverContext) -> bool {
    constraint_accepts(&expr.model_family, ctx.model_family.as_deref())
        && constraint_accepts(&expr.harness, ctx.harness.as_deref())
        && expr
            .modalities
            .is_none_or(|required| ctx.modalities.contains(required))
        && expr
            .posture
            .as_ref()
            .is_none_or(|allowed| allowed.contains(&ctx.posture))
}

/// `Any` accepts everything; `OneOf` requires a declared value in the set.
/// An *undeclared* context value fails a constrained dimension — a variant
/// asking for `claude` must not serve an anonymous consumer.
fn constraint_accepts(c: &Constraint, value: Option<&str>) -> bool {
    match c {
        Constraint::Any => true,
        Constraint::OneOf(allowed) => {
            value.is_some_and(|v| allowed.iter().any(|a| a.eq_ignore_ascii_case(v)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ConsumerKind;
    use crate::manifest::{ModalitySet, Posture, VariantBody};

    fn inline(tag: &str) -> VariantBody {
        VariantBody::Inline {
            content_type: "text/plain".into(),
            data: tag.into(),
        }
    }

    fn tag(v: &Variant) -> &str {
        match &v.body {
            VariantBody::Inline { data, .. } => data,
            VariantBody::Media(_) => "media",
        }
    }

    fn agent(family: Option<&str>, modalities: ModalitySet, posture: Posture) -> ResolverContext {
        ResolverContext {
            kind: ConsumerKind::Agent,
            model_family: family.map(str::to_owned),
            harness: None,
            modalities,
            posture,
        }
    }

    #[test]
    fn empty_variant_list_yields_none_never_panics() {
        assert!(select_variant(&[], &ResolverContext::human()).is_none());
    }

    #[test]
    fn undeclared_context_value_fails_a_constrained_dimension() {
        let variants = vec![Variant {
            match_expr: MatchExpr {
                model_family: Constraint::OneOf(vec!["claude".into()]),
                ..MatchExpr::default()
            },
            body: inline("claude-only"),
            revalidate_after_ms: None,
        }];
        let anon = agent(None, ModalitySet::TEXT, Posture::Headless);
        assert!(
            select_variant(&variants, &anon).is_none(),
            "a variant asking for claude must not serve an anonymous consumer"
        );
    }

    #[test]
    fn family_match_is_case_insensitive() {
        let variants = vec![Variant {
            match_expr: MatchExpr {
                model_family: Constraint::OneOf(vec!["claude".into()]),
                ..MatchExpr::default()
            },
            body: inline("c"),
            revalidate_after_ms: None,
        }];
        let ctx = agent(Some("Claude"), ModalitySet::TEXT, Posture::Headless);
        assert_eq!(select_variant(&variants, &ctx).unwrap().index, 0);
    }

    #[test]
    fn modalities_are_superset_matched() {
        let variants = vec![Variant {
            match_expr: MatchExpr {
                modalities: Some(ModalitySet::VISION),
                ..MatchExpr::default()
            },
            body: inline("needs-eyes"),
            revalidate_after_ms: None,
        }];
        let with_eyes = agent(
            None,
            ModalitySet::TEXT.with(ModalitySet::VISION),
            Posture::Attended,
        );
        let without = agent(None, ModalitySet::TEXT, Posture::Attended);
        assert!(select_variant(&variants, &with_eyes).is_some());
        assert!(select_variant(&variants, &without).is_none());
    }

    #[test]
    fn specificity_beats_declaration_order_but_ties_do_not() {
        let variants = vec![
            Variant {
                match_expr: MatchExpr::any(),
                body: inline("catch-all"),
                revalidate_after_ms: None,
            },
            Variant {
                match_expr: MatchExpr {
                    model_family: Constraint::OneOf(vec!["gpt".into()]),
                    harness: Constraint::OneOf(vec!["codex".into()]),
                    ..MatchExpr::default()
                },
                body: inline("gpt+codex"),
                revalidate_after_ms: None,
            },
            Variant {
                match_expr: MatchExpr {
                    model_family: Constraint::OneOf(vec!["gpt".into()]),
                    ..MatchExpr::default()
                },
                body: inline("gpt-early"),
                revalidate_after_ms: None,
            },
            Variant {
                match_expr: MatchExpr {
                    harness: Constraint::OneOf(vec!["codex".into()]),
                    ..MatchExpr::default()
                },
                body: inline("codex-late"),
                revalidate_after_ms: None,
            },
        ];
        let mut ctx = agent(Some("gpt"), ModalitySet::TEXT, Posture::Headless);
        ctx.harness = Some("codex".into());
        // Specificity 2 beats both specificity-1 variants and the catch-all.
        let sel = select_variant(&variants, &ctx).unwrap();
        assert_eq!(tag(sel.variant), "gpt+codex");

        // Remove the specificity-2 winner: the two specificity-1 variants
        // tie; declaration order (gpt-early, index 1 of the remaining list)
        // must win.
        let tied: Vec<Variant> = variants
            .iter()
            .filter(|v| tag(v) != "gpt+codex")
            .cloned()
            .collect();
        let sel = select_variant(&tied, &ctx).unwrap();
        assert_eq!(
            tag(sel.variant),
            "gpt-early",
            "ties break by declaration order"
        );
    }
}
