#![allow(missing_docs)] // criterion macros generate undocumented items

//! The hot paths against their budgets (design doc `13 §6`). Run
//! `just bench`; numbers are recorded in `benches/PERF.md` with the
//! machine and date — measurable is a feature, not a slogan.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use waggle_core::{
    mint, resolve, select_variant, ActorClass, CanonicalUrl, Channel, Constraint, Event, EventLog,
    InternTables, MatchExpr, MintOptions, MintSpec, ModalitySet, Posture, ResolverContext, Seq,
    Sharer, Stage, Timestamp, Token, Variant, VariantBody,
};

fn seeded_entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0x1234_5677_u32 | 1;
    move |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = (state & 0xFF) as u8;
        }
        Ok(())
    }
}

fn variants() -> Vec<Variant> {
    let inline = |tag: &str| VariantBody::Inline {
        content_type: "text/plain".into(),
        data: tag.into(),
    };
    vec![
        Variant {
            match_expr: MatchExpr {
                model_family: Constraint::OneOf(vec!["claude".into()]),
                harness: Constraint::OneOf(vec!["claude-code".into()]),
                ..MatchExpr::default()
            },
            body: inline("a"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                model_family: Constraint::OneOf(vec!["gpt".into()]),
                ..MatchExpr::default()
            },
            body: inline("b"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                modalities: Some(ModalitySet::BROWSER),
                ..MatchExpr::default()
            },
            body: inline("c"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr {
                posture: Some(vec![Posture::Headless, Posture::Ci]),
                ..MatchExpr::default()
            },
            body: inline("d"),
            revalidate_after_ms: None,
        },
        Variant {
            match_expr: MatchExpr::any(),
            body: inline("e"),
            revalidate_after_ms: None,
        },
    ]
}

fn bench_token(c: &mut Criterion) {
    let mut entropy = seeded_entropy();
    c.bench_function("token_generate_8", |b| {
        b.iter(|| Token::generate(8, &mut entropy).unwrap());
    });
    c.bench_function("token_parse", |b| {
        b.iter(|| Token::parse(black_box("7Kp2mQ9x")).unwrap());
    });
}

fn bench_matcher(c: &mut Criterion) {
    let vs = variants();
    let ctx = ResolverContext {
        kind: waggle_core::ConsumerKind::Agent,
        model_family: Some("gpt".into()),
        harness: Some("codex".into()),
        modalities: ModalitySet::TEXT,
        posture: Posture::Headless,
    };
    c.bench_function("select_variant_5", |b| {
        b.iter(|| select_variant(black_box(&vs), black_box(&ctx)));
    });
}

fn bench_mint_resolve(c: &mut Criterion) {
    let mut entropy = seeded_entropy();
    c.bench_function("mint_two_variants", |b| {
        b.iter(|| {
            let mut spec = MintSpec::new(
                CanonicalUrl::new("ws://bench/artifact").unwrap(),
                Sharer::new("lead").unwrap(),
                Channel::subagent_general(),
            );
            for v in variants().into_iter().take(2) {
                spec = spec.variant(v.match_expr, v.body);
            }
            mint(
                spec,
                &MintOptions::default(),
                &mut entropy,
                Timestamp::from_unix_ms(1),
            )
            .unwrap()
        });
    });

    let mut entropy = seeded_entropy();
    let mut spec = MintSpec::new(
        CanonicalUrl::new("ws://bench/artifact").unwrap(),
        Sharer::new("lead").unwrap(),
        Channel::subagent_general(),
    );
    for v in variants() {
        spec = spec.variant(v.match_expr, v.body);
    }
    let manifest = mint(
        spec,
        &MintOptions::default(),
        &mut entropy,
        Timestamp::from_unix_ms(1),
    )
    .unwrap();
    let ctx = ResolverContext::anonymous_agent();
    c.bench_function("resolve_pure", |b| {
        b.iter(|| {
            resolve(
                black_box(&manifest),
                black_box(&ctx),
                Timestamp::from_unix_ms(2),
            )
        });
    });
}

fn bench_fold_1m(c: &mut Criterion) {
    let mut tables = InternTables::default();
    let mut log = EventLog::default();
    let mut entropy = seeded_entropy();
    let tokens: Vec<Token> = (0..100)
        .map(|_| Token::generate(8, &mut entropy).unwrap())
        .collect();
    let stages = [Stage::resolve(), Stage::run(), Stage::repeat()];
    let actor = ActorClass::from_context(&ResolverContext::anonymous_agent());
    for i in 0..1_000_000u32 {
        log.push(
            &Event {
                token: tokens[(i % 100) as usize],
                stage: stages[(i % 3) as usize].clone(),
                actor,
                at: Timestamp::from_unix_ms(u64::from(i)),
                seq: Seq(i),
                variant: None,
                regions: None,
            },
            &mut tables,
        );
    }
    let target = tables.token_id(tokens[7]).unwrap();
    // Budget (13 §6): < 10 ms release-mode for the 1M-row funnel fold.
    c.bench_function("fold_funnel_1m", |b| {
        b.iter(|| log.stage_counts(black_box(target), &tables));
    });
}

criterion_group!(
    benches,
    bench_token,
    bench_matcher,
    bench_mint_resolve,
    bench_fold_1m
);
criterion_main!(benches);
