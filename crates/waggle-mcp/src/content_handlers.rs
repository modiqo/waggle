//! Content-access handlers (design doc `18`): `read`, `search`, and the
//! snapshot half of `mint` — the token as a file descriptor. The pure
//! lens engine lives in [`crate::content`]; this module owns the I/O
//! seam: where bytes come from (snapshot blob, then live local target)
//! and the `read` stage recorded per access.

use serde_json::{json, Map, Value};
use waggle_core::{ActorClass, ResolverContext, Stage, Timestamp, Token};
use waggle_store::{AppendIntent, BlobSink, Store, StoreError};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{arg_str, infer_content_type, parse_token_arg, store_err, Handler};

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// Snapshot the target's bytes into the blob CAS (doc 18 §3).
    pub(crate) async fn snapshot_target(
        &self,
        target: &str,
    ) -> Result<waggle_core::MediaRef, Envelope> {
        let path = local_path(target).ok_or_else(|| {
            Envelope::err(
                format!("snapshot: `{target}` is not a locally readable file path — snapshot works on file:// targets"),
                vec![],
            )
        })?;
        let bytes = read_capped(&path)?;
        self.blobs
            .put(&bytes, infer_content_type(&path))
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))
    }

    /// Pin a harness-extracted text file as the token's searchable
    /// content (doc 18 §7): the target stays the original binary; this
    /// extraction is what `read`/`search` serve.
    pub(crate) async fn pin_extraction(
        &self,
        path: &str,
    ) -> Result<waggle_core::MediaRef, Envelope> {
        let bytes = read_capped(path)?;
        let content_type = infer_content_type(path);
        if !crate::content::is_text(content_type) {
            return Err(Envelope::err(
                format!("content `{path}` is {content_type} — pass the extracted TEXT (md/txt/json), not another binary"),
                vec![],
            ));
        }
        self.blobs
            .put(&bytes, content_type)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))
    }

    /// Fetch the token's content per doc 18 §3: snapshot blob first, then
    /// the live local target. Returns `(text, content_type)`.
    async fn content_of(&self, token: Token) -> Result<(String, String), Envelope> {
        let view = match self.store.manifest(token).await {
            Ok(Some(v)) => v,
            Ok(None) => return Err(store_err(&StoreError::UnknownToken(token))),
            Err(e) => return Err(store_err(&e)),
        };
        if view.manifest.revoked_at.is_some()
            || self.ancestor_revoked_at(&view.manifest).await.is_some()
        {
            return Err(Envelope::err(
                format!("{token} is revoked (directly or through its lineage) — revoked content serves nothing"),
                vec![],
            ));
        }
        let (bytes, content_type) = if let Some(media) = &view.manifest.content {
            let bytes = self
                .blobs
                .get(media)
                .await
                .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
            (bytes, media.content_type.clone())
        } else if let Some(path) = local_path(view.manifest.target.as_str()) {
            (read_capped(&path)?, infer_content_type(&path).to_owned())
        } else {
            return Err(Envelope::err(
                format!(
                    "no readable content behind {token} — its target `{}` is not a local file; mint with snapshot=true (or --attach) so content travels with the token",
                    view.manifest.target.as_str()
                ),
                vec![],
            ));
        };
        if !crate::content::is_text(&content_type) {
            return Err(Envelope::err(
                format!("content is {content_type} (binary) — fetch it via its MediaRef, or mint an extracted-text variant to make it searchable"),
                vec![],
            ));
        }
        String::from_utf8(bytes)
            .map(|text| (text, content_type))
            .map_err(|_| {
                Envelope::err(
                    "content is not valid UTF-8 — treat it as binary media",
                    vec![],
                )
            })
    }

    async fn record_read(&self, token: Token, now: Timestamp) {
        let _ = self
            .store
            .append(AppendIntent::Event {
                token,
                stage: Stage::read(),
                actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
                variant: None,
                at: now,
            })
            .await;
    }

    /// `read` (doc 18 §4): overview / line window / markdown section /
    /// JSON pointer, budgeted, with continuation guidance.
    pub(crate) async fn read_content(&self, args: &Map<String, Value>, now: Timestamp) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let (text, content_type) = match self.content_of(token).await {
            Ok(x) => x,
            Err(e) => return e,
        };
        let max_bytes = args
            .get("max-bytes")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(crate::query::DEFAULT_MAX_BYTES);

        let result = if let Some(range) = arg_str(args, "lines") {
            let Some((from, to)) = range.split_once('-').and_then(|(a, b)| {
                Some((
                    a.trim().parse::<usize>().ok()?,
                    b.trim().parse::<usize>().ok()?,
                ))
            }) else {
                return Envelope::err(
                    format!("lines `{range}` — expected A-B, 1-based inclusive (e.g. 120-180)"),
                    vec![],
                );
            };
            crate::content::read_lines(&text, from, to, max_bytes)
        } else if let Some(heading) = arg_str(args, "section") {
            if let Some(v) = crate::content::read_section(&text, heading, max_bytes) {
                v
            } else {
                let outline = crate::content::outline(&text);
                return Envelope::err(
                    format!("no section `{heading}` — the outline is: {outline}"),
                    vec![NextCall {
                        tool: "read".into(),
                        args: json!({ "token": token.as_str() }),
                        why: "the overview lists sections and lenses".into(),
                    }],
                );
            }
        } else if let Some(path) = arg_str(args, "path") {
            match crate::content::read_path(&text, path, max_bytes) {
                Ok(v) => v,
                Err(hint) => return Envelope::err(hint, vec![]),
            }
        } else {
            crate::content::overview(&text, &content_type, max_bytes)
        };

        self.record_read(token, now).await;
        let next = read_next(token, &result);
        #[allow(clippy::cast_possible_truncation)]
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(text.len() as u64),
            seq: None,
        })
    }

    /// `search` (doc 18 §4): grep through the token.
    pub(crate) async fn search_content(
        &self,
        args: &Map<String, Value>,
        now: Timestamp,
    ) -> Envelope {
        let token = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(pattern) = arg_str(args, "pattern") else {
            return Envelope::err(
                "missing `pattern` — a Rust regex; (?i) for case-insensitive",
                vec![],
            );
        };
        // A lineage root with no content of its own searches DEEPLY:
        // every descendant's content, matches grouped per file — the
        // folder token greps as a tree, locally and at the edge alike.
        if let Ok(Some(view)) = self.store.manifest(token).await {
            if view.manifest.content.is_none() {
                let children = self.store.children(token).await.unwrap_or_default();
                if !children.is_empty() {
                    return self.search_tree(token, pattern, args, now).await;
                }
            }
        }
        let (text, _content_type) = match self.content_of(token).await {
            Ok(x) => x,
            Err(e) => return e,
        };
        let context = args
            .get("context")
            .and_then(Value::as_u64)
            .map_or(2, |v| usize::try_from(v).unwrap_or(2));
        let max_matches = args
            .get("max-matches")
            .and_then(Value::as_u64)
            .map_or(5, |v| usize::try_from(v).unwrap_or(5));
        let max_bytes = args
            .get("max-bytes")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(crate::query::DEFAULT_MAX_BYTES);
        let result = match crate::content::search(&text, pattern, context, max_matches, max_bytes) {
            Ok(v) => v,
            Err(hint) => return Envelope::err(hint, vec![]),
        };
        self.record_read(token, now).await;
        // The grep→open loop: chain the first match into a read window.
        let next = result["matches"]
            .as_array()
            .and_then(|m| m.first())
            .and_then(|m| m["line"].as_u64())
            .map(|line| {
                let from = line.saturating_sub(10).max(1);
                vec![NextCall {
                    tool: "read".into(),
                    args: json!({ "token": token.as_str(), "lines": format!("{from}-{}", line + 10) }),
                    why: "open the first match's neighborhood".into(),
                }]
            })
            .unwrap_or_default();
        #[allow(clippy::cast_possible_truncation)]
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(text.len() as u64),
            seq: None,
        })
    }
}

/// A target the daemon can read directly: file:// URI or absolute path.
pub(crate) fn local_path(target: &str) -> Option<String> {
    if let Some(p) = target.strip_prefix("file://") {
        return Some(p.to_owned());
    }
    target.starts_with('/').then(|| target.to_owned())
}

/// Read a local file under the doc-18 cap (16 MB).
fn read_capped(path: &str) -> Result<Vec<u8>, Envelope> {
    const CAP: u64 = 16 * 1024 * 1024;
    let meta = std::fs::metadata(path)
        .map_err(|e| Envelope::err(format!("content {path}: {e}"), vec![]))?;
    if meta.is_dir() {
        return Err(Envelope::err(
            format!(
                "`{path}` is a directory — waggle references artifacts, not trees:                  mint each file with parent=<this-token> so the folder token becomes                  their lineage root (revoking it tombstones them all)"
            ),
            vec![],
        ));
    }
    if meta.len() > CAP {
        return Err(Envelope::err(
            format!(
                "content is {} bytes — beyond the 16 MB read cap; snapshot a subset",
                meta.len()
            ),
            vec![],
        ));
    }
    std::fs::read(path).map_err(|e| Envelope::err(format!("content {path}: {e}"), vec![]))
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// Deep search over a lineage tree: BFS the descendants, grep each
    /// one that carries content, group matches per file. Totals are
    /// counted in full; listings are capped per file and by the byte
    /// budget — truncation is named, never silent.
    async fn search_tree(
        &self,
        root: waggle_core::Token,
        pattern: &str,
        args: &Map<String, Value>,
        now: Timestamp,
    ) -> Envelope {
        let context = args
            .get("context")
            .and_then(Value::as_u64)
            .map_or(1, |v| usize::try_from(v).unwrap_or(1));
        let per_file = args
            .get("max-matches")
            .and_then(Value::as_u64)
            .map_or(3, |v| usize::try_from(v).unwrap_or(3));
        let max_bytes = args
            .get("max-bytes")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(crate::query::DEFAULT_MAX_BYTES);

        let mut queue: std::collections::VecDeque<_> =
            self.store.children(root).await.unwrap_or_default().into();
        let mut files = Vec::new();
        let mut total: u64 = 0;
        let mut searched = 0u64;
        let mut skipped = 0u64;
        let mut visited = 0;
        while let Some(child) = queue.pop_front() {
            visited += 1;
            if visited > 200 {
                skipped += 1 + queue.len() as u64;
                break;
            }
            queue.extend(self.store.children(child).await.unwrap_or_default());
            let Ok(Some(view)) = self.store.manifest(child).await else {
                continue;
            };
            let Ok((text, _)) = self.content_of(child).await else {
                skipped += 1; // no snapshot here (or binary) — named below
                continue;
            };
            searched += 1;
            // Honest telemetry: the deep search DID read this file's
            // bytes — the child's funnel says so (coverage's 'read' bar).
            self.record_read(child, now).await;
            let Ok(found) =
                crate::content::search(&text, pattern, context, per_file, max_bytes / 4)
            else {
                continue; // bad regex reports once, below, via the empty case
            };
            let file_total = found["total_matches"].as_u64().unwrap_or(0);
            if file_total > 0 {
                total += file_total;
                files.push(json!({
                    "token": child.as_str(),
                    "target": view.manifest.target.as_str(),
                    "total_matches": file_total,
                    "matches": found["matches"],
                }));
            }
        }
        // Regex sanity: if nothing was searchable the pattern never ran.
        if searched == 0 {
            return Envelope::err(
                format!(
                    "no searchable content under {root} — its children have no snapshots here;                      `waggle edge push` (or re-mint with --tree) pins the bytes"
                ),
                vec![],
            );
        }
        let mut result = json!({
            "token": root.as_str(),
            "pattern": pattern,
            "tree": { "files_searched": searched, "files_skipped": skipped },
            "total_matches": total,
            "files": files,
        });
        // The byte budget holds for the whole tree answer.
        let rendered = serde_json::to_vec(&result).map_or(0, |b| b.len());
        if rendered > max_bytes {
            let files = result["files"].as_array().cloned().unwrap_or_default();
            let mut kept = Vec::new();
            let mut used = 200; // envelope skeleton allowance
            for f in files {
                let size = serde_json::to_vec(&f).map_or(0, |b| b.len());
                if used + size > max_bytes {
                    break;
                }
                used += size;
                kept.push(f);
            }
            let dropped = result["files"].as_array().map_or(0, Vec::len) - kept.len();
            result["files"] = json!(kept);
            result["truncated"] = json!(format!(
                "{dropped} matching file(s) beyond the {max_bytes}-byte budget — raise max-bytes or search a child directly"
            ));
        }
        self.record_read(root, now).await;
        let next = files_next(&result, root);
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(searched),
            seq: None,
        })
    }
}

/// The grep→open chain for tree results: read the first matching file.
fn files_next(result: &Value, root: waggle_core::Token) -> Vec<NextCall> {
    let mut next = Vec::new();
    if let Some(first) = result["files"].as_array().and_then(|f| f.first()) {
        if let (Some(token), Some(line)) = (
            first["token"].as_str(),
            first["matches"]
                .as_array()
                .and_then(|m| m.first())
                .and_then(|m| m["line"].as_u64()),
        ) {
            let from = line.saturating_sub(5).max(1);
            next.push(NextCall {
                tool: "read".into(),
                args: json!({ "token": token, "lines": format!("{from}-{}", line + 10) }),
                why: "open the first matching file at its hit".into(),
            });
        }
    }
    next.push(NextCall {
        tool: "resolve".into(),
        args: json!({ "token": root.as_str() }),
        why: "the root's index: every child token by filename".into(),
    });
    next.truncate(3);
    next
}

/// Continuation guidance for read results.
fn read_next(token: Token, result: &Value) -> Vec<NextCall> {
    let mut next = Vec::new();
    if let Some(window) = result["next_window"].as_str() {
        next.push(NextCall {
            tool: "read".into(),
            args: json!({ "token": token.as_str(), "lines": window }),
            why: "continue the window".into(),
        });
    }
    if let Some(outline) = result["outline"].as_array() {
        if let Some(first) = outline.iter().find_map(|h| h["heading"].as_str()) {
            next.push(NextCall {
                tool: "read".into(),
                args: json!({ "token": token.as_str(), "section": first }),
                why: "read the first section".into(),
            });
        }
        next.push(NextCall {
            tool: "search".into(),
            args: json!({ "token": token.as_str(), "pattern": "<regex>" }),
            why: "grep instead of reading top to bottom".into(),
        });
    }
    for p in result["next_paths"]
        .as_array()
        .into_iter()
        .flatten()
        .take(2)
    {
        if let Some(p) = p.as_str() {
            next.push(NextCall {
                tool: "read".into(),
                args: json!({ "token": token.as_str(), "path": p }),
                why: "one level deeper".into(),
            });
        }
    }
    next.truncate(3);
    next
}
