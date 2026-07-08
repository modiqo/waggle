//! Tool handlers: catalog operation → store call → envelope. Transport-
//! agnostic — the stdio loop, the daemon, and the CLI all dispatch here,
//! which is what makes "shim adds no semantics" testable (16 §3).
//!
//! Effects arrive as parameters (`now`, entropy via [`Handler::new`]) —
//! the sans-I/O discipline continued one layer up.

use serde_json::{json, Map, Value};
use waggle_core::{
    mint, negotiate, resolve, ActorClass, CanonicalUrl, Change, Channel, ConsumerHint, MintOptions,
    MintSpec, ResolverContext, Sharer, Stage, Timestamp, Token,
};
use waggle_store::{AppendIntent, Appended, MintNonce, Store, StoreError};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::map::{global_map, handoff_line, token_map};

/// The dispatcher: one per store, shared by every transport.
pub struct Handler<S> {
    store: S,
    /// Session identity used when `sharer` is omitted (one-call mint).
    default_sharer: Sharer,
}

fn arg_str<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn parse_token_arg(args: &Map<String, Value>) -> Result<Token, Envelope> {
    let Some(raw) = arg_str(args, "token") else {
        return Err(Envelope::err(
            "missing `token` — pass the waggle token you were handed",
            vec![NextCall {
                tool: "map".into(),
                args: json!({}),
                why: "orient from the global map".into(),
            }],
        ));
    };
    Token::parse(raw).map_err(|e| {
        Envelope::err(
            format!("`{raw}` is not a waggle token ({e}) — check for truncation"),
            vec![],
        )
    })
}

fn store_err(e: &StoreError) -> Envelope {
    // Every store error already names its fix (07 §5); the envelope adds
    // the recovery step where one exists.
    let next = match e {
        StoreError::Conflict { token, .. } | StoreError::LifecycleRequiresVersion(token) => {
            vec![NextCall {
                tool: "resolve".into(),
                args: json!({ "token": token.as_str() }),
                why: "re-read the manifest for the current version, then retry".into(),
            }]
        }
        StoreError::UnknownToken(_) => vec![NextCall {
            tool: "map".into(),
            args: json!({}),
            why: "list what this store actually holds".into(),
        }],
        _ => vec![],
    };
    Envelope::err(e.to_string(), next)
}

impl<S: Store> Handler<S> {
    /// Build a handler over a store with a session identity.
    pub fn new(store: S, default_sharer: Sharer) -> Self {
        Self {
            store,
            default_sharer,
        }
    }

    /// The store, for transports needing direct reads.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Dispatch one tool call. `now` and `entropy` are the transport's —
    /// handlers stay clock- and randomness-free.
    pub async fn dispatch<E>(
        &self,
        tool: &str,
        args: &Value,
        now: Timestamp,
        entropy: &mut E,
    ) -> Envelope
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let empty = Map::new();
        let args = args.as_object().unwrap_or(&empty);
        match tool {
            "mint" => self.mint(args, now, entropy).await,
            "resolve" => self.resolve(args, now).await,
            "record" => self.record(args, now).await,
            "mutate" => self.mutate(args, now).await,
            "funnel" => self.funnel(args).await,
            "map" => self.map(args, now).await,
            other => Envelope::err(
                format!("`{other}` is not a waggle tool — `map` lists what exists"),
                vec![NextCall {
                    tool: "map".into(),
                    args: json!({}),
                    why: "orient".into(),
                }],
            ),
        }
    }

    async fn mint<E>(&self, args: &Map<String, Value>, now: Timestamp, entropy: &mut E) -> Envelope
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let Some(target) = arg_str(args, "target") else {
            return Envelope::err("missing `target` — the artifact URI to mint from", vec![]);
        };
        let target = match CanonicalUrl::new(target) {
            Ok(t) => t,
            Err(e) => return Envelope::err(format!("target: {e}"), vec![]),
        };
        let sharer = match arg_str(args, "sharer") {
            Some(s) => match Sharer::new(s) {
                Ok(s) => s,
                Err(e) => return Envelope::err(format!("sharer: {e}"), vec![]),
            },
            None => self.default_sharer.clone(), // one-call mint (17 §5)
        };
        let channel = match arg_str(args, "channel") {
            Some(c) => match Channel::new(c) {
                Ok(c) => c,
                Err(e) => return Envelope::err(format!("channel: {e}"), vec![]),
            },
            None => Channel::subagent_general(),
        };
        let mut spec = MintSpec::new(target, sharer, channel);
        if let Some(parent) = arg_str(args, "parent") {
            match Token::parse(parent) {
                Ok(p) => spec = spec.child_of(p),
                Err(e) => return Envelope::err(format!("parent: {e}"), vec![]),
            }
        }
        if let Some(variants) = args.get("variants") {
            match serde_json::from_value::<Vec<waggle_core::Variant>>(variants.clone()) {
                Ok(vs) => {
                    for v in vs {
                        spec = spec.variant(v.match_expr, v.body);
                    }
                }
                Err(e) => {
                    return Envelope::err(
                        format!("variants: {e} — an array of {{match, body}} objects"),
                        vec![],
                    )
                }
            }
        }
        let manifest = match mint(spec, &MintOptions::default(), &mut *entropy, now) {
            Ok(m) => m,
            Err(e) => return Envelope::err(e.to_string(), vec![]),
        };
        // The idempotency nonce is transport-supplied entropy (C-8).
        let mut nonce_bytes = [0u8; 8];
        if let Err(e) = entropy(&mut nonce_bytes) {
            return Envelope::err(format!("entropy: {e}"), vec![]);
        }
        let nonce = MintNonce(u64::from_le_bytes(nonce_bytes));
        let receipt = self
            .store
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce,
            })
            .await;
        match receipt {
            Ok(Appended::Minted { view, replayed }) => {
                let token = view.manifest.token;
                Envelope::ok(
                    json!({
                        "token": token.as_str(),
                        "handoff": handoff_line(token.as_str()),
                        "replayed": replayed,
                        "variants": view.manifest.variants.len(),
                    }),
                    vec![
                        NextCall {
                            tool: "resolve".into(),
                            args: json!({ "token": token.as_str() }),
                            why: "self-check the projection consumers will receive".into(),
                        },
                        NextCall {
                            tool: "map".into(),
                            args: json!({ "token": token.as_str() }),
                            why: "orient around the new token".into(),
                        },
                    ],
                )
                .with_stats(Stats {
                    records: Some(1),
                    seq: Some(0),
                })
            }
            Ok(_) => Envelope::err("store returned a non-mint receipt for a mint", vec![]),
            Err(e) => store_err(&e),
        }
    }

    async fn resolve(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let view = match self.store.manifest(token).await {
            Ok(Some(v)) => v,
            Ok(None) => return store_err(&StoreError::UnknownToken(token)),
            Err(e) => return store_err(&e),
        };
        let ctx = match args.get("context") {
            Some(v) => match serde_json::from_value::<ResolverContext>(v.clone()) {
                Ok(c) => c,
                Err(e) => {
                    return Envelope::err(
                        format!("context: {e} — pass a resolver context object or omit it"),
                        vec![],
                    )
                }
            },
            None => negotiate(&ConsumerHint::UserAgent("waggle-mcp")),
        };
        let resolution = resolve(&view.manifest, &ctx, now);
        let variant_index = resolution.variant.as_ref().map(|s| s.index);
        let body = resolution.variant.as_ref().map(|s| &s.variant.body);

        // Record the resolve — the host's separate act (I-4), done here at
        // the transport layer, never inside core resolve().
        let recorded = self
            .store
            .append(AppendIntent::Event {
                token,
                stage: Stage::resolve(),
                actor: ActorClass::from_context(&ctx),
                variant: variant_index,
                at: now,
            })
            .await;
        let seq = match recorded {
            Ok(Appended::Event { seq }) => Some(seq.0),
            _ => None,
        };

        Envelope::ok(
            json!({
                "disposition": resolution.disposition,
                "variant": variant_index,
                "body": body,
                "target": view.manifest.target.as_str(),
                "as_of": resolution.as_of,
                "revalidate_after": resolution.revalidate_after,
            }),
            vec![
                NextCall {
                    tool: "record".into(),
                    args: json!({ "token": token.as_str(), "stage": "run" }),
                    why: "report execution so attribution reflects reality".into(),
                },
                NextCall {
                    tool: "map".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "orient: state, forward and reverse paths".into(),
                },
            ],
        )
        .with_stats(Stats {
            records: Some(1),
            seq,
        })
    }

    async fn record(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(stage_raw) = arg_str(args, "stage") else {
            return Envelope::err(
                "missing `stage` — run, repeat, assess, or a custom slug",
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
                at: now,
            })
            .await;
        match receipt {
            Ok(Appended::Event { seq }) => Envelope::ok(
                json!({ "recorded": stage.as_str(), "token": token.as_str() }),
                vec![NextCall {
                    tool: "funnel".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "see the counts your report just moved".into(),
                }],
            )
            .with_stats(Stats {
                records: Some(1),
                seq: Some(seq.0),
            }),
            Ok(_) => Envelope::err("store returned a non-event receipt for a record", vec![]),
            Err(e) => store_err(&e),
        }
    }

    async fn mutate(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(change_raw) = arg_str(args, "change") else {
            return Envelope::err(
                "missing `change` — revoke, supersede=<token>, expire=<unix-ms>, or label k=v",
                vec![],
            );
        };
        let change = match parse_change(change_raw) {
            Ok(c) => c,
            Err(e) => return Envelope::err(e, vec![]),
        };
        let expected_version = args
            .get("expected-version")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok());
        let receipt = self
            .store
            .append(AppendIntent::Mutate {
                token,
                change,
                expected_version,
                at: now,
            })
            .await;
        match receipt {
            Ok(Appended::Mutated { seq, version }) => Envelope::ok(
                json!({ "token": token.as_str(), "version": version }),
                vec![NextCall {
                    tool: "map".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "confirm the new disposition and remaining paths".into(),
                }],
            )
            .with_stats(Stats {
                records: Some(1),
                seq: Some(seq.0),
            }),
            Ok(_) => Envelope::err("store returned a non-mutate receipt for a mutate", vec![]),
            Err(e) => store_err(&e),
        }
    }

    async fn funnel(&self, args: &Map<String, Value>) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let counts = match self.store.funnel(token).await {
            Ok(c) => c,
            Err(e) => return store_err(&e),
        };
        let children = self.store.children(token).await.unwrap_or_default();
        let total: u64 = counts.values().sum();
        let counts_json: Value = counts
            .iter()
            .map(|(s, c)| (s.as_str().to_owned(), json!(c)))
            .collect();
        Envelope::ok(
            json!({
                "token": token.as_str(),
                "stages": counts_json,
                "children": children.iter().map(waggle_core::Token::as_str).collect::<Vec<_>>(),
            }),
            vec![NextCall {
                tool: "map".into(),
                args: json!({ "token": token.as_str() }),
                why: "orient: the funnel feeds the map's ranked suggestions".into(),
            }],
        )
        .with_stats(Stats {
            records: Some(total),
            seq: None,
        })
    }

    async fn map(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let Some(_) = args.get("token") else {
            let count = self.store.scan_all().await.map_or(0, |r| r.len() as u64);
            return global_map(count);
        };
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let view = match self.store.manifest(token).await {
            Ok(Some(v)) => v,
            Ok(None) => return store_err(&StoreError::UnknownToken(token)),
            Err(e) => return store_err(&e),
        };
        let funnel = self.store.funnel(token).await.unwrap_or_default();
        let children = self.store.children(token).await.unwrap_or_default();
        token_map(&view.manifest, &funnel, children.len(), now)
    }
}

fn parse_change(raw: &str) -> Result<Change, String> {
    if raw == "revoke" {
        return Ok(Change::Revoked);
    }
    if let Some(by) = raw.strip_prefix("supersede=") {
        let token = Token::parse(by).map_err(|e| format!("supersede target: {e}"))?;
        return Ok(Change::Superseded { by: token });
    }
    if let Some(ts) = raw.strip_prefix("expire=") {
        let ms: u64 = ts
            .parse()
            .map_err(|_| "expire= takes unix milliseconds".to_owned())?;
        return Ok(Change::ExpirySet {
            expires_at: Some(Timestamp::from_unix_ms(ms)),
        });
    }
    if let Some(kv) = raw.strip_prefix("label ") {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| "label takes key=value".to_owned())?;
        return Ok(Change::LabelSet {
            key: k.to_owned(),
            value: v.to_owned(),
        });
    }
    Err(format!(
        "`{raw}` is not a change — revoke, supersede=<token>, expire=<unix-ms>, or label k=v"
    ))
}
