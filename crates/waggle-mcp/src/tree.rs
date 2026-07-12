//! The directory affordances (design doc `22 §4`): what a token means when it
//! names a **folder** rather than a file.
//!
//! Every one of these exists because the benchmark caught an agent needing it
//! and not having it:
//!
//! * `search` had always grepped a tree, but `read` answered null — so a folder
//!   could be searched and never *described*. The first move an agent makes with
//!   a shared directory is to ask what is in it, and a consumer that cannot see
//!   the vocabulary can only guess a regex.
//! * A tree could be grepped but not *lensed*. We watched an agent issue
//!   `section: "Retry Policy"` ten times, once per child token, hand-rolling a
//!   fan-out the substrate should have done in one call.
//! * A fan-out that ran out of budget truncated in silence. An agent read nine
//!   of ten runbooks — the missing one was the violator — and answered
//!   confidently and wrongly. Incompleteness must be loud, and resumable.

use serde_json::{json, Map, Value};
use waggle_core::Timestamp;
use waggle_store::{BlobSink, Store};

use crate::content_handlers::ContentView;
use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{arg_str, Handler};

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// If the token is a lineage root with no content of its own, it is a TREE:
    /// serve the directory instead of failing. With a lens, fan that lens out
    /// across every file — handed a folder, an agent asks the same question of
    /// all of it, and we watched one issue `section: "Retry Policy"` ten times,
    /// once per child, because a tree could be grepped but never *lensed*.
    pub(crate) async fn try_tree_read(
        &self,
        token: waggle_core::Token,
        args: &Map<String, Value>,
        max_bytes: usize,
        now: Timestamp,
    ) -> Option<Envelope> {
        let v = self.store.manifest(token).await.ok()??;
        if v.manifest.content.is_some() {
            return None;
        }
        if self
            .store
            .children(token)
            .await
            .unwrap_or_default()
            .is_empty()
        {
            return None;
        }
        let lens = ["section", "symbol", "lines"]
            .into_iter()
            .find_map(|k| arg_str(args, k).map(|val| (k, val.to_owned())));
        Some(match lens {
            Some((kind, value)) => {
                let from = args.get("from").and_then(Value::as_u64);
                self.read_tree_lens(token, kind, &value, max_bytes, from, now)
                    .await
            }
            None => self.read_tree(token, max_bytes).await,
        })
    }

    /// One child's row in the directory projection: its path relative to the
    /// folder, its own token, and the structure it affords — the SYMBOLS a
    /// source file already carries, or the headings of a document. A folder of
    /// code that lists no symbols presents as structureless, which is the one
    /// shape where knowing what is inside each file matters most.
    async fn tree_entry(
        &self,
        child: waggle_core::Token,
        v: &waggle_store::ManifestView,
        base: Option<&str>,
        max_bytes: usize,
        total_bytes: &mut u64,
    ) -> Value {
        let target = v.manifest.target.as_str().to_owned();
        // A basename is not an identity: a repository is full of `mod.py` and
        // `index.ts`, and a consumer handed six of them cannot tell which is
        // which, nor reason about where anything sits.
        let name = crate::content_handlers::local_path(&target).map_or_else(
            || target.clone(),
            |p| match base {
                Some(b) => p.strip_prefix(b).unwrap_or(&p).to_owned(),
                None => p.clone(),
            },
        );
        let mut entry = json!({ "name": name, "token": child.as_str() });

        let Ok(cv) = self.content_of(child).await else {
            return entry;
        };
        *total_bytes += cv.text.len() as u64;
        entry["bytes"] = json!(cv.text.len());
        entry["lines"] = json!(cv.text.lines().count());
        entry["content_type"] = json!(cv.content_type);

        if let Some(media) = &v.manifest.outline {
            if let Ok(blob) = self.blobs.get(media).await {
                if let Some(sym) = crate::outline_wire::render(&blob, max_bytes / 4) {
                    let names: Vec<Value> = sym
                        .get("symbols")
                        .and_then(Value::as_array)
                        .map(|rows| {
                            rows.iter()
                                .take(8)
                                .filter_map(|r| r.get("name").cloned())
                                .collect()
                        })
                        .unwrap_or_default();
                    if !names.is_empty() {
                        entry["symbols"] = Value::Array(names);
                        entry["lenses"] = json!(["symbol", "lines", "search"]);
                        return entry;
                    }
                }
            }
        }
        let o = if cv.content_type == "text/markdown" {
            crate::content::outline(&cv.text)
        } else {
            crate::content::outline_plain(&cv.text)
        };
        if let Value::Array(items) = o {
            let heads: Vec<Value> = items
                .into_iter()
                .take(6)
                .filter_map(|h| h.get("heading").cloned())
                .collect();
            if !heads.is_empty() {
                entry["outline"] = Value::Array(heads);
            }
        }
        entry
    }

    /// The **directory projection**: what `read` returns for a tree.
    ///
    /// Each child by name, with its own token (so the consumer can address it
    /// directly), its size, type, and — budget permitting — its outline. That
    /// is the folder's table of contents: the consumer learns the vocabulary
    /// before it has to guess a pattern, and it learns which file to open
    /// without opening all of them.
    ///
    /// Listing is *not* consumption: no `read` stage is stamped for the
    /// children here. A table of contents tells you what exists; it does not
    /// serve you the bytes, and the receipts must not pretend it did.
    async fn read_tree(&self, root: waggle_core::Token, max_bytes: usize) -> Envelope {
        // The folder's own directory, so children can be named by their path
        // RELATIVE to it. A basename is not an identity: a repository is full
        // of `mod.py` and `index.ts`, and a consumer handed six of them cannot
        // tell which is which, nor reason about where anything sits.
        let base = match self.store.manifest(root).await {
            Ok(Some(v)) => crate::content_handlers::local_path(v.manifest.target.as_str())
                .map(|p| format!("{}/", p.trim_end_matches('/'))),
            _ => None,
        };

        let mut queue: std::collections::VecDeque<_> =
            self.store.children(root).await.unwrap_or_default().into();
        let mut files = Vec::new();
        let mut total_bytes: u64 = 0;
        let mut visited = 0usize;
        let mut budget = 0usize;
        let mut truncated = false;

        while let Some(child) = queue.pop_front() {
            visited += 1;
            if visited > 200 || budget > max_bytes {
                truncated = true;
                break;
            }
            queue.extend(self.store.children(child).await.unwrap_or_default());
            let Ok(Some(v)) = self.store.manifest(child).await else {
                continue;
            };
            let entry = self
                .tree_entry(child, &v, base.as_deref(), max_bytes, &mut total_bytes)
                .await;
            budget += entry.to_string().len();
            files.push(entry);
        }

        let n = files.len();
        let result = json!({
            "kind": "tree",
            "files": n,
            "total_bytes": total_bytes,
            "truncated": truncated,
            "children": files,
        });
        let next = vec![
            NextCall {
                tool: "read".into(),
                args: json!({ "token": root.as_str(), "section": "<a heading from the outlines above>" }),
                why: "that section from EVERY file in ONE call — do not fetch them one at a time"
                    .into(),
            },
            NextCall {
                tool: "search".into(),
                args: json!({ "token": root.as_str(), "pattern": "<regex>" }),
                why: "grep every file in the tree at once; matches come back per file".into(),
            },
            NextCall {
                tool: "read".into(),
                args: json!({ "token": "<a child's token above>" }),
                why: "open a single file — the listing gave you its token".into(),
            },
        ];
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(n as u64),
            seq: None,
        })
    }
    /// Apply one lens to one file — the per-child half of a tree fan-out.
    async fn lens_one(
        &self,
        child: waggle_core::Token,
        cv: &ContentView,
        kind: &str,
        value: &str,
        per_file: usize,
    ) -> Option<Value> {
        match kind {
            "section" => crate::content::read_section(&cv.text, value, per_file),
            "symbol" => match self.resolve_symbol(child, cv.outline.as_ref(), value).await {
                Ok((from, to)) => Some(crate::content::read_lines(&cv.text, from, to, per_file)),
                Err(_) => None,
            },
            _ => value
                .split_once('-')
                .and_then(|(a, b)| {
                    Some((
                        a.trim().parse::<usize>().ok()?,
                        b.trim().parse::<usize>().ok()?,
                    ))
                })
                .map(|(f, t)| crate::content::read_lines(&cv.text, f, t, per_file)),
        }
    }
    /// **Lens the tree**: apply one lens to every file in it.
    ///
    /// `search` has always grepped a folder. This is the other half: ask the
    /// same *structural* question of every file — "the Retry Policy section
    /// from all twelve runbooks", "the `handle` symbol wherever it is defined".
    /// We added it because we watched agents hand-roll exactly this, issuing
    /// the identical command once per child token; a folder that can be grepped
    /// but not lensed makes the consumer do the fan-out itself.
    ///
    /// Unlike the listing, this SERVES bytes, so each file it answers for is
    /// stamped as read — the receipt records what the consumer actually got.
    async fn read_tree_lens(
        &self,
        root: waggle_core::Token,
        kind: &str,
        value: &str,
        max_bytes: usize,
        args_from: Option<u64>,
        now: Timestamp,
    ) -> Envelope {
        let all: Vec<_> = self.store.children(root).await.unwrap_or_default();
        let total_files = all.len() as u64;
        // `from` continues a fan-out that ran out of budget. A tree-lens that
        // truncates in silence is worse than a slow one: we watched an agent
        // reason over 9 of 10 runbooks — the missing one was the violator — and
        // answer confidently and wrongly. Completeness has to be resumable, and
        // incompleteness has to be loud.
        let from = usize::try_from(args_from.unwrap_or(0)).unwrap_or(0);
        let mut queue: std::collections::VecDeque<_> = all.into_iter().skip(from).collect();
        let mut files = Vec::new();
        let mut visited = 0usize;
        let mut budget = 0usize;
        let mut matched = 0u64;
        let mut skipped = 0u64;
        let mut truncated = false;
        let mut consumed = 0usize;

        while let Some(child) = queue.pop_front() {
            visited += 1;
            if visited > 200 || budget >= max_bytes {
                truncated = true;
                break;
            }
            consumed += 1;
            queue.extend(self.store.children(child).await.unwrap_or_default());
            let Ok(Some(v)) = self.store.manifest(child).await else {
                continue;
            };
            let Ok(cv) = self.content_of(child).await else {
                skipped += 1;
                continue;
            };
            let per_file = (max_bytes / 6).max(crate::query::MIN_MAX_BYTES);
            let served = self.lens_one(child, &cv, kind, value, per_file).await;
            let Some(slice) = served else {
                skipped += 1; // this file simply has no such section/symbol
                continue;
            };
            matched += 1;
            let target = v.manifest.target.as_str().to_owned();
            let name = target
                .rsplit('/')
                .next()
                .unwrap_or(target.as_str())
                .to_owned();
            let entry = json!({
                "name": name,
                "token": child.as_str(),
                "lines": slice["lines"],
                "text": slice["text"],
            });
            budget += entry.to_string().len();
            files.push(entry);
            // Bytes were served: the receipt says so.
            self.record_read(child, now, None).await;
        }

        let seen = from + consumed;
        let complete = !truncated && seen as u64 >= total_files;
        let result = json!({
            "kind": "tree-lens",
            "lens": kind,
            "of": value,
            "total_files": total_files,
            "examined": seen,
            "matched": matched,
            "skipped": skipped,
            "complete": complete,
            "truncated": truncated,
            "files": files,
        });
        let next = if truncated {
            // LOUD: the consumer has not seen the whole tree. Say so, and say
            // exactly how to finish — a partial fan-out that reads as a whole
            // one is how a confident wrong answer gets made.
            vec![NextCall {
                tool: "read".into(),
                args: json!({
                    "token": root.as_str(), kind: value, "from": seen,
                }),
                why: format!(
                    "INCOMPLETE: {seen} of {total_files} files examined. Continue from `from`={seen} before you conclude anything about the tree"
                ),
            }]
        } else if matched == 0 {
            vec![NextCall {
                tool: "read".into(),
                args: json!({ "token": root.as_str() }),
                why: "no file had that — the tree listing names every file's outline".into(),
            }]
        } else {
            vec![]
        };
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(matched),
            seq: None,
        })
    }
}
