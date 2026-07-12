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
        let from = args.get("from").and_then(Value::as_u64);
        Some(if let Some((kind, value)) = lens {
            self.read_tree_lens(token, kind, &value, max_bytes, from, now)
                .await
        } else {
            self.read_tree(token, max_bytes, from).await
        })
    }

    /// Every file token under a tree, in a stable order. `mint --tree` flattens,
    /// so this is usually one hop — but it walks, so a nested lineage yields the
    /// same list, and the projection and the fan-out can never disagree about
    /// what "all the files" means.
    pub(crate) async fn tree_files(&self, root: waggle_core::Token) -> Vec<waggle_core::Token> {
        let mut out = Vec::new();
        let mut queue: std::collections::VecDeque<_> =
            self.store.children(root).await.unwrap_or_default().into();
        while let Some(child) = queue.pop_front() {
            if out.len() >= 200 {
                break;
            }
            queue.extend(self.store.children(child).await.unwrap_or_default());
            out.push(child);
        }
        out
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
    ///
    /// A big tree does not fit in one projection, and a projection that stops
    /// at 25 of 180 files without saying so is the worst possible answer: the
    /// consumer reads a complete-looking table of contents and concludes the
    /// tree contains nothing else. So the count is taken BEFORE the budget is
    /// spent, `complete` is stated outright, and truncation hands back both
    /// ways out — resume at the cursor, or stop listing and `search`, which
    /// returns only the files that matched and never pays for the rest.
    async fn read_tree(
        &self,
        root: waggle_core::Token,
        max_bytes: usize,
        from: Option<u64>,
    ) -> Envelope {
        // The folder's own directory, so children can be named by their path
        // RELATIVE to it. A basename is not an identity: a repository is full
        // of `mod.py` and `index.ts`, and a consumer handed six of them cannot
        // tell which is which, nor reason about where anything sits.
        let base = match self.store.manifest(root).await {
            Ok(Some(v)) => crate::content_handlers::local_path(v.manifest.target.as_str())
                .map(|p| format!("{}/", p.trim_end_matches('/'))),
            _ => None,
        };

        // The denominator first. Knowing there are 180 files is what makes a
        // 25-file answer honest, and it costs one walk of the lineage — no
        // manifests, no blobs, no budget.
        let all = self.tree_files(root).await;
        let total_files = all.len() as u64;
        let start = usize::try_from(from.unwrap_or(0)).unwrap_or(0);

        let mut files = Vec::new();
        let mut total_bytes: u64 = 0;
        let mut budget = 0usize;
        let mut truncated = false;

        for child in all.iter().skip(start) {
            let Ok(Some(v)) = self.store.manifest(*child).await else {
                continue;
            };
            let entry = self
                .tree_entry(*child, &v, base.as_deref(), max_bytes, &mut total_bytes)
                .await;
            let cost = entry.to_string().len();
            if !files.is_empty() && budget + cost > max_bytes {
                truncated = true;
                break;
            }
            budget += cost;
            files.push(entry);
        }

        let n = files.len();
        let listed = start + n;
        let complete = !truncated && listed as u64 >= total_files;
        let mut result = json!({
            "kind": "tree",
            "files": n,
            "total_files": total_files,
            "listed": format!("{}/{}", listed, total_files),
            "complete": complete,
            "total_bytes": total_bytes,
            "truncated": truncated,
            "children": files,
        });

        let mut next = Vec::new();
        if !complete {
            // Loud. The consumer has seen a fraction of the tree, and the one
            // thing it must not do is reason as though it has seen all of it.
            result["hint"] = json!(format!(
                "INCOMPLETE LISTING: {listed} of {total_files} files. The rest are NOT \
                 shown and you have NOT seen them. Do not conclude anything about the \
                 tree from this page. Either narrow with `search` — which returns only \
                 the files that match, and never pays for the others — or page on with \
                 `from: {listed}`."
            ));
            next.push(NextCall {
                tool: "search".into(),
                args: json!({ "token": root.as_str(), "pattern": "<regex>" }),
                why: "PREFERRED at this size: grep the whole tree and get back ONLY the \
                      matching files, each with its own token — a filtered listing that \
                      costs a fraction of the full one"
                    .into(),
            });
            next.push(NextCall {
                tool: "read".into(),
                args: json!({ "token": root.as_str(), "from": listed }),
                why: "continue the listing from where it stopped".into(),
            });
        }
        next.push(NextCall {
            tool: "read".into(),
            args: json!({ "token": root.as_str(), "section": "<a heading from the outlines above>" }),
            why: "that section from EVERY file in ONE call — do not fetch them one at a time"
                .into(),
        });
        next.push(NextCall {
            tool: "search".into(),
            args: json!({ "token": root.as_str(), "pattern": "<regex>" }),
            why: "grep every file in the tree at once; matches come back per file".into(),
        });
        next.push(NextCall {
            tool: "read".into(),
            args: json!({ "token": "<a child's token above>" }),
            why: "open a single file — the listing gave you its token".into(),
        });
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
        let all = self.tree_files(root).await;
        let total_files = all.len() as u64;
        // `from` continues a fan-out that ran out of budget. A tree-lens that
        // truncates in silence is worse than a slow one: we watched an agent
        // reason over 9 of 10 runbooks — the missing one was the violator — and
        // answer confidently and wrongly. Completeness has to be resumable, and
        // incompleteness has to be loud.
        let from = usize::try_from(args_from.unwrap_or(0)).unwrap_or(0);
        // `max_bytes` is a PER-RESPONSE cap, and applying it to an N-file
        // response is a category error. It was being split `max_bytes / 6` — a
        // hardcoded six-file assumption — so a fan-out over eleven files served
        // nine and silently dropped two. `--require files:all` was therefore
        // unsatisfiable in one call BY CONSTRUCTION: the one move that closes a
        // completeness contract could not close it. Consumers fell back to
        // fetching children one at a time, and the weaker ones burned their turn
        // budget and never answered — while the gate, correctly, refused to
        // believe the ones that answered early.
        //
        // Nor is the answer to divide the budget by the file count: the load-
        // bearing sentence is usually at the END of a section, and slicing every
        // file to a thin prefix serves all of them while cutting the fact out of
        // each. Full coverage, zero information.
        //
        // Only four ways exist to fit eleven full sections into an eight-kilobyte
        // budget, and three of them are wrong:
        //
        //   thin every file  — the load-bearing sentence is at the END of a
        //                      section; a thin prefix of all of them serves
        //                      everything and cuts the fact out of each. Full
        //                      coverage, zero information.
        //   drop files       — the original bug: nine of eleven, silently.
        //   overrun the caller — we tried this, and it was the worst of the
        //                      three. `max_bytes` is a CONTRACT with the caller.
        //                      We returned 9,255 bytes against a budget of 8,000;
        //                      the client showed its model the first 4,500 and
        //                      dropped the rest — while our receipt certified all
        //                      ten files as read. The gate then believed a
        //                      consumer that had seen five runbooks out of eleven.
        //                      A receipt must never attest to bytes the consumer
        //                      did not get, and we cannot know what a client
        //                      truncates: so we must not exceed what it asked for.
        //
        //   PAGE             — serve as many WHOLE files as the budget holds, say
        //                      `complete: false`, hand back `from`. Honest about
        //                      depth, honest about coverage, honest about cost.
        //
        // So each served file gets a full single-file allowance, and the response
        // as a whole never exceeds the budget the caller set.
        let per_file = max_bytes;
        let ceiling = max_bytes;
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
            if visited > 200 {
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
            // Decide BEFORE committing. Appending an entry and only then noticing
            // we are over budget overshoots by up to one whole file — and an
            // overshoot is not a rounding error, it is a receipt that lies: the
            // client truncates what we overran, and we have already stamped the
            // file `read`. If it does not fit, stop here and let `from` carry it.
            let cost = entry.to_string().len();
            if !files.is_empty() && budget + cost > ceiling {
                truncated = true;
                consumed -= 1;
                break;
            }
            budget += cost;
            files.push(entry);
            // Bytes were served — and only now, once we know the consumer will
            // actually receive them, does the receipt say so.
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
