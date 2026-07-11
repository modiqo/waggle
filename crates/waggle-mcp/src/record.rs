//! The `record` handler: downstream stages reported against a token —
//! including the judge's verdict (`accepted` / `rejected`, doc `19 §4.1`),
//! whose rejection response teaches the escalation choreography (19 §4.6).
//! Split from `handlers.rs` so each file stays one concept (13 §1).

use serde_json::{json, Map, Value};
use waggle_core::{ActorClass, ResolverContext, Stage, Timestamp, Token};
use waggle_store::{AppendIntent, Appended, BlobSink, Store};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{arg_str, parse_token_arg, store_err, Handler};

impl<S: Store, B: BlobSink> Handler<S, B> {
    pub(crate) async fn record(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(stage_raw) = arg_str(args, "stage") else {
            return Envelope::err(
                "missing `stage` — run, repeat, assess, accepted, rejected, or a custom slug",
                vec![],
            );
        };
        let stage = match Stage::new(stage_raw) {
            Ok(s) => s,
            Err(e) => return Envelope::err(format!("stage: {e}"), vec![]),
        };
        let receipt = self
            .store
            .append(AppendIntent::Event {
                token,
                stage: stage.clone(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                variant: None,
                regions: None,
                at: now,
            })
            .await;
        match receipt {
            Ok(Appended::Event { seq }) => Envelope::ok(
                json!({ "recorded": stage.as_str(), "token": token.as_str() }),
                self.record_next(token, &stage).await,
            )
            .with_stats(Stats {
                records: Some(1),
                seq: Some(seq.0),
            }),
            Ok(_) => Envelope::err("store returned a non-event receipt for a record", vec![]),
            Err(e) => store_err(&e),
        }
    }

    /// Forward paths after a record. A `rejected` verdict teaches the
    /// escalation choreography (19 §4.6): re-mint the same target under
    /// the same parent for a stronger consumer, then supersede this
    /// child — the escalation becomes lineage, not lore.
    async fn record_next(&self, token: Token, stage: &Stage) -> Vec<NextCall> {
        let funnel_next = NextCall {
            tool: "funnel".into(),
            args: json!({ "token": token.as_str() }),
            why: "see the counts your report just moved".into(),
        };
        if *stage != Stage::rejected() {
            return vec![funnel_next];
        }
        let Ok(Some(view)) = self.store.manifest(token).await else {
            return vec![funnel_next];
        };
        let mut mint_args = json!({ "target": view.manifest.target.as_str() });
        if let Some(parent) = view.manifest.parent {
            mint_args["parent"] = json!(parent.as_str());
        }
        vec![
            NextCall {
                tool: "mint".into(),
                args: mint_args,
                why: "escalate: re-mint the same artifact (same parent) for a stronger consumer"
                    .into(),
            },
            NextCall {
                tool: "mutate".into(),
                args: json!({
                    "token": token.as_str(),
                    "change": "supersede=<new-token>",
                    "expected-version": view.manifest.version,
                }),
                why: "then supersede this rejected handoff so late readers follow the pointer"
                    .into(),
            },
            funnel_next,
        ]
    }
}
