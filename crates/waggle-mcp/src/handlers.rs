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
use waggle_store::{AppendIntent, Appended, BlobSink, MintNonce, NoBlobs, Store, StoreError};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::map::{global_map, handoff_line, token_map};

/// The dispatcher: one per store, shared by every transport.
pub struct Handler<S, B = NoBlobs> {
    pub(crate) store: S,
    /// Session identity used when `sharer` is omitted (one-call mint).
    pub(crate) default_sharer: Sharer,
    /// Content-addressed media storage; the [`NoBlobs`] default refuses
    /// with a hint rather than degrading silently.
    pub(crate) blobs: B,
    /// The host's signing identity (CP-11): present → every mint is
    /// signed over its immutable core.
    pub(crate) signer: Option<ed25519_dalek::SigningKey>,
}

pub(crate) fn arg_str<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

pub(crate) fn parse_token_arg(args: &Map<String, Value>) -> Result<Token, Envelope> {
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

pub(crate) fn store_err(e: &StoreError) -> Envelope {
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

impl<S: Store> Handler<S, NoBlobs> {
    /// Build a handler over a store with a session identity.
    pub fn new(store: S, default_sharer: Sharer) -> Self {
        Self {
            store,
            default_sharer,
            blobs: NoBlobs,
            signer: None,
        }
    }
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// Attach a blob sidecar — enables `mint --attach` and media variants.
    #[must_use]
    pub fn with_blobs<B2: BlobSink>(self, blobs: B2) -> Handler<S, B2> {
        Handler {
            store: self.store,
            default_sharer: self.default_sharer,
            blobs,
            signer: self.signer,
        }
    }

    /// Give the host a signing identity: every mint from here on carries
    /// an Ed25519 signature over its immutable core (CP-11).
    #[must_use]
    pub fn with_signer(mut self, signer: ed25519_dalek::SigningKey) -> Self {
        self.signer = Some(signer);
        self
    }

    /// The store, for transports needing direct reads.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// The blob sink, for transports needing direct blob I/O (replication).
    pub fn blobs(&self) -> &B {
        &self.blobs
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
            "read" => self.read_content(args, now).await,
            "search" => self.search_content(args, now).await,
            "query" => self.query(args).await,
            "map" => self.map(args, now).await,
            "find" => self.find(args).await,
            "coverage" => self.coverage(args).await,
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

    pub(crate) async fn mint<E>(
        &self,
        args: &Map<String, Value>,
        now: Timestamp,
        entropy: &mut E,
    ) -> Envelope
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
        let spec = MintSpec::new(target, sharer, channel);
        let spec = match self.mint_extras(spec, args).await {
            Ok(s) => s,
            Err(e) => return e,
        };
        let mut manifest = match mint(spec, &MintOptions::default(), &mut *entropy, now) {
            Ok(m) => m,
            Err(e) => return Envelope::err(e.to_string(), vec![]),
        };
        if let Some(signer) = &self.signer {
            manifest.signature = Some(waggle_core::trust::sign_manifest(&manifest, signer));
        }
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
                let tree = args.get("tree").and_then(Value::as_bool).unwrap_or(false)
                    || arg_str(args, "tree") == Some("true");
                if tree {
                    return self.mint_tree(&view, now, entropy).await;
                }
                let next = crate::lineage::mint_next(token.as_str(), view.manifest.target.as_str());
                Envelope::ok(
                    json!({
                        "token": token.as_str(),
                        "handoff": handoff_line(token.as_str()),
                        "replayed": replayed,
                        "variants": view.manifest.variants.len(),
                    }),
                    next,
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

    /// Snapshot + declared variants: the optional halves of mint.
    async fn mint_extras(
        &self,
        mut spec: MintSpec,
        args: &Map<String, Value>,
    ) -> Result<MintSpec, Envelope> {
        if let Some(parent) = arg_str(args, "parent") {
            match Token::parse(parent) {
                Ok(p) => spec = spec.child_of(p),
                Err(e) => return Err(Envelope::err(format!("parent: {e}"), vec![])),
            }
        }
        if args
            .get("private")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || arg_str(args, "private") == Some("true")
        {
            spec = spec.private();
        }
        spec = crate::discovery::apply_tags(spec, args);
        if let Some(path) = arg_str(args, "attach") {
            match self
                .attach_variant(path, arg_str(args, "attach-type"))
                .await
            {
                Ok(v) => spec = spec.with_variant(v),
                Err(e) => return Err(e),
            }
        }
        let snapshot = args
            .get("snapshot")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || arg_str(args, "snapshot") == Some("true");
        let extracted = arg_str(args, "content");
        if snapshot && extracted.is_some() {
            return Err(Envelope::err(
                "pass one of snapshot/content: snapshot pins the TARGET's own bytes; \
                 content pins your EXTRACTION of a binary target",
                vec![],
            ));
        }
        if snapshot {
            let media = self.snapshot_target(spec.target_str()).await?;
            spec = spec.content(media);
        }
        if let Some(path) = extracted {
            // The format boundary (doc 18 §7): the harness extracted this
            // with its own abilities; waggle persists and serves it.
            let media = self.pin_extraction(path).await?;
            spec = spec.content(media);
        }
        if let Some(variants) = args.get("variants") {
            let vs = serde_json::from_value::<Vec<waggle_core::Variant>>(variants.clone())
                .map_err(|e| {
                    Envelope::err(
                        format!("variants: {e} — an array of {{match, body}} objects"),
                        vec![],
                    )
                })?;
            for v in vs {
                spec = spec.with_variant(v); // full fidelity — incl. revalidate_after_ms
            }
        }
        Ok(spec)
    }

    /// Read `path`, store it content-addressed, and shape the media
    /// variant: images serve vision consumers, audio serves listeners
    /// (rev 2.3) — everyone else falls through to the catch-all.
    async fn attach_variant(
        &self,
        path: &str,
        declared_type: Option<&str>,
    ) -> Result<waggle_core::Variant, Envelope> {
        let bytes = std::fs::read(path)
            .map_err(|e| Envelope::err(format!("attach {path}: {e}"), vec![]))?;
        let content_type = declared_type.map_or_else(
            || infer_content_type(path).to_owned(),
            std::borrow::ToOwned::to_owned,
        );
        let media = self
            .blobs
            .put(&bytes, &content_type)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
        let modality = if content_type.starts_with("audio/") {
            waggle_core::ModalitySet::AUDIO
        } else {
            waggle_core::ModalitySet::VISION
        };
        Ok(waggle_core::Variant {
            match_expr: waggle_core::MatchExpr {
                modalities: Some(modality),
                ..waggle_core::MatchExpr::default()
            },
            body: waggle_core::VariantBody::Media(media),
            revalidate_after_ms: None,
        })
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
        let ctx = match parse_context(args) {
            Ok(c) => c,
            Err(e) => return e,
        };
        // The cascade the catalog promises: a revoked ANCESTOR tombstones
        // this token too — checked at resolution time, where it bites.
        let effective = match self.ancestor_revoked_at(&view.manifest).await {
            Some(at) => {
                let mut tombstoned = (*view.manifest).clone();
                waggle_core::apply_change(&mut tombstoned, &waggle_core::Change::Revoked, at);
                std::borrow::Cow::Owned(tombstoned)
            }
            None => std::borrow::Cow::Borrowed(&*view.manifest),
        };
        let resolution = resolve(&effective, &ctx, now);
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

        let signature = match waggle_core::trust::verify_manifest(&view.manifest) {
            waggle_core::trust::SignatureStatus::Valid { key } => {
                json!({ "status": "valid", "key": key })
            }
            waggle_core::trust::SignatureStatus::Unsigned => json!({ "status": "unsigned" }),
            waggle_core::trust::SignatureStatus::Invalid => json!({ "status": "INVALID" }),
        };
        // A lineage root's projection includes its INDEX: the children
        // as tokens + targets. That's how a folder token resolves on a
        // machine where the folder never existed — the consumer picks
        // its files and reads/greps them per-token.
        let mut child_index = Vec::new();
        for child in self.store.children(token).await.unwrap_or_default() {
            if let Ok(Some(cv)) = self.store.manifest(child).await {
                child_index.push(json!({
                    "token": child.as_str(),
                    "target": cv.manifest.target.as_str(),
                }));
            }
        }
        let mut result = json!({
                "disposition": resolution.disposition,
                "variant": variant_index,
                "body": body,
                "target": view.manifest.target.as_str(),
                "as_of": resolution.as_of,
                "revalidate_after": resolution.revalidate_after,
                "signature": signature,
        });
        if !child_index.is_empty() {
            result["children"] = json!(child_index);
        }
        Envelope::ok(
            result,
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
        // The lineage roll-up the description promises: this token's
        // stages plus every descendant's, BFS with a runaway cap — the
        // folder/mission token answers for its whole tree.
        let mut result = json!({
            "token": token.as_str(),
            "stages": counts_json,
            "children": children.iter().map(waggle_core::Token::as_str).collect::<Vec<_>>(),
        });
        if !children.is_empty() {
            let mut rollup = counts;
            let mut queue: std::collections::VecDeque<_> = children.iter().copied().collect();
            let mut visited = 0;
            while let Some(child) = queue.pop_front() {
                visited += 1;
                if visited > 1000 {
                    break; // runaway-lineage backstop; counts stay honest per-token
                }
                if let Ok(child_counts) = self.store.funnel(child).await {
                    for (stage, n) in child_counts {
                        *rollup.entry(stage).or_insert(0) += n;
                    }
                }
                queue.extend(self.store.children(child).await.unwrap_or_default());
            }
            result["rollup"] = rollup
                .iter()
                .map(|(s, c)| (s.as_str().to_owned(), json!(c)))
                .collect();
        }
        Envelope::ok(
            result,
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

    /// The guided query (CP-7): the token's document, sliced under a
    /// byte budget, with executable paths deeper as `next`.
    async fn query(&self, args: &Map<String, Value>) -> Envelope {
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
        let doc = json!({
            "manifest": &*view.manifest,
            "funnel": funnel.iter().map(|(s, c)| (s.as_str().to_owned(), json!(c)))
                .collect::<Value>(),
            "children": children.iter().map(waggle_core::Token::as_str).collect::<Vec<_>>(),
        });
        let path = arg_str(args, "path").unwrap_or("");
        let max_bytes = args
            .get("max-bytes")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(crate::query::DEFAULT_MAX_BYTES);
        let Some(slice) = crate::query::slice_at(&doc, path, max_bytes) else {
            let siblings = crate::query::slice_at(&doc, "", max_bytes)
                .map(|s| s.next_paths)
                .unwrap_or_default();
            return Envelope::err(
                format!(
                    "path `{path}` names nothing on {token} — valid roots: {}",
                    siblings.join(", ")
                ),
                vec![NextCall {
                    tool: "query".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "start from the root shape".into(),
                }],
            );
        };
        let next = slice
            .next_paths
            .iter()
            .take(3)
            .map(|p| NextCall {
                tool: "query".into(),
                args: json!({ "token": token.as_str(), "path": p }),
                why: "one level deeper".into(),
            })
            .collect();
        #[allow(clippy::cast_possible_truncation)]
        Envelope::ok(
            json!({
                "path": path,
                "slice": slice.slice,
                "truncated": slice.truncated,
                "full_bytes": slice.full_bytes,
                "next_paths": slice.next_paths,
            }),
            next,
        )
        .with_stats(Stats {
            records: Some(slice.full_bytes as u64),
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

/// The resolver context from args: an object, a JSON STRING of one
/// (how CLIs deliver --context), or negotiated when absent.
fn parse_context(args: &Map<String, Value>) -> Result<ResolverContext, Envelope> {
    let Some(v) = args.get("context") else {
        return Ok(negotiate(&ConsumerHint::UserAgent("waggle-mcp")));
    };
    let value = match v.as_str() {
        Some(s) => serde_json::from_str::<Value>(s).unwrap_or_else(|_| v.clone()),
        None => v.clone(),
    };
    serde_json::from_value::<ResolverContext>(value).map_err(|e| {
        Envelope::err(
            format!("context: {e} — pass a resolver context object or omit it"),
            vec![],
        )
    })
}

/// Extension → content type, for the common media cases.
pub(crate) fn infer_content_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown") => "text/markdown",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("rs") => "text/x-rust",
        Some("py") => "text/x-python",
        Some("ts" | "tsx" | "js" | "jsx") => "text/x-script",
        Some("txt" | "csv" | "log" | "toml" | "ini" | "cfg") => "text/plain",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg" | "oga") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        _ => "application/octet-stream",
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
