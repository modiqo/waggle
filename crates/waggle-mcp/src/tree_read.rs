//! Reading and searching an indexed tree (design doc: tree-scale).
//!
//! A token minted `--tree` carries a [`waggle_core::TreeNode`] instead of inline
//! content. This module serves it:
//!
//! * **projection** — `read` on the token returns the directory's table of
//!   contents from its [`DirIndex`]: files (name, size, type) and subdirectories
//!   (name, token, totals), with no descent;
//! * **file read** — `read … --file <name>` fetches one file's bytes back from the
//!   blob store by its content hash and serves them (with the ordinary lenses);
//! * **search** — `search` on the token descends the whole lineage in one call,
//!   pruning subtrees with each node's Bloom, narrowing to candidate files with
//!   each node's trigram index, confirming with a real match, and returning
//!   **ranked** hits — each with its path and owning token so a consumer can drill
//!   in.
//!
//! The traversal is the I/O half of `waggle-tree`: it fetches index blobs and file
//! bytes and applies the pure prune/candidate/rank decisions that crate defines.

use serde_json::{json, Map, Value};
use waggle_core::{CanonicalUrl, MediaRef, Sha256Hex, Timestamp, Token, TreeNode};
use waggle_store::{BlobSink, Store};
use waggle_tree::{search as tsearch, Bloom, DirIndex, FileEntry, Hit, TrigramIndex};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{arg_str, Handler};

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// If `token` is an indexed tree node, serve it and return `Some`. Non-tree
    /// tokens return `None` so the caller falls through to file handling.
    pub(crate) async fn try_indexed_tree_read(
        &self,
        token: Token,
        args: &Map<String, Value>,
        now: Timestamp,
    ) -> Option<Envelope> {
        let view = self.store.manifest(token).await.ok()??;
        let node = view.manifest.tree.clone()?;
        Some(match arg_str(args, "file") {
            Some(name) => self.read_tree_file(token, &node, name, now).await,
            None => self.read_tree_toc(token, &node).await,
        })
    }

    /// The directory's table of contents from its index blob.
    async fn read_tree_toc(&self, token: Token, node: &TreeNode) -> Envelope {
        let index = match self.load_index(node).await {
            Ok(i) => i,
            Err(e) => return e,
        };
        let files: Vec<Value> = index
            .files()
            .map(|f| json!({ "name": f.name, "bytes": f.size, "content_type": f.content_type }))
            .collect();
        let subdirs: Vec<Value> = index
            .subdirs()
            .map(
                |d| json!({ "name": d.name, "token": d.token, "files": d.files, "bytes": d.bytes }),
            )
            .collect();
        let next = vec![
            NextCall {
                tool: "search".into(),
                args: json!({ "token": token.as_str(), "pattern": "<regex>" }),
                why: "grep the WHOLE tree in one call — pruned and ranked".into(),
            },
            NextCall {
                tool: "read".into(),
                args: json!({ "token": token.as_str(), "file": "<a name above>" }),
                why: "read one file's content by name".into(),
            },
        ];
        Envelope::ok(
            json!({
                "kind": "tree",
                "files": files.len(),
                "subdirs": subdirs.len(),
                "total_files": node.files,
                "total_bytes": node.bytes,
                "children": files,
                "dirs": subdirs,
            }),
            next,
        )
    }

    /// Serve one file's bytes, fetched from the blob store by its content hash.
    async fn read_tree_file(
        &self,
        token: Token,
        node: &TreeNode,
        name: &str,
        now: Timestamp,
    ) -> Envelope {
        let index = match self.load_index(node).await {
            Ok(i) => i,
            Err(e) => return e,
        };
        let Some(entry) = index.files().find(|f| f.name == name) else {
            let names: Vec<&str> = index.files().map(|f| f.name.as_str()).take(20).collect();
            return Envelope::err(
                format!("tree: no file `{name}` in this directory — files: {names:?}"),
                vec![],
            );
        };
        let media = match file_media(entry) {
            Ok(m) => m,
            Err(e) => return e,
        };
        let bytes = match self.blobs.get(&media).await {
            Ok(b) => b,
            Err(e) => return Envelope::err(e.to_string(), vec![]),
        };
        // A file read is real consumption — stamp it against this node so a
        // files:all coverage can roll it up.
        self.record_read(token, now, None).await;
        let text = String::from_utf8_lossy(&bytes);
        Envelope::ok(
            json!({
                "name": name,
                "content_type": entry.content_type,
                "bytes": entry.size,
                "text": text,
            }),
            vec![],
        )
        .with_stats(Stats {
            records: Some(bytes.len() as u64),
            seq: None,
        })
    }

    /// Search the whole tree from `root` in one call: prune, narrow, confirm, rank.
    pub(crate) async fn search_indexed_tree(
        &self,
        root: Token,
        pattern: &str,
        args: &Map<String, Value>,
        now: Timestamp,
    ) -> Envelope {
        let limit = args
            .get("max-matches")
            .and_then(Value::as_u64)
            .unwrap_or(10)
            .min(50) as usize;
        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return Envelope::err(format!("search: bad pattern: {e}"), vec![]),
        };
        let mut hits: Vec<Hit> = Vec::new();
        let mut visited = 0usize;
        Box::pin(self.search_node(root, "", pattern, &re, &mut hits, &mut visited)).await;
        let ranked = tsearch::rank(hits, limit);
        // Each confirmed match served that file's bytes — record it once.
        for h in &ranked {
            if let Ok(t) = Token::parse(&h.token) {
                self.record_read(t, now, None).await;
            }
        }
        let matches: Vec<Value> = ranked
            .iter()
            .map(|h| {
                json!({ "path": h.path, "token": h.token, "line": h.line,
                        "text": h.text, "matches": h.matches })
            })
            .collect();
        Envelope::ok(
            json!({
                "kind": "tree-search",
                "total_matches": matches.len(),
                "nodes_visited": visited,
                "matches": matches,
            }),
            vec![NextCall {
                tool: "read".into(),
                args: json!({ "token": "<a match's token>", "file": "<its path's last segment>" }),
                why: "open a matching file by name on its owning node".into(),
            }],
        )
    }

    /// One node of the search descent. Prunes on this node's Bloom, greps its
    /// candidate files, then recurses into subdirectories.
    async fn search_node(
        &self,
        token: Token,
        prefix: &str,
        pattern: &str,
        re: &regex::Regex,
        hits: &mut Vec<Hit>,
        visited: &mut usize,
    ) {
        let Ok(Some(view)) = self.store.manifest(token).await else {
            return;
        };
        let Some(node) = view.manifest.tree.clone() else {
            return;
        };
        // Prune: if the Bloom proves the pattern's trigrams are absent, skip the
        // whole subtree. Only literal patterns prune; a regex enters and greps.
        if is_literal(pattern) {
            if let Ok(bloom) = Bloom::from_hex(&node.bloom) {
                if tsearch::prune(&bloom, pattern) == tsearch::Prune::Skip {
                    return;
                }
            }
        }
        *visited += 1;
        let Ok(index) = self.load_index(&node).await else {
            return;
        };
        let files: Vec<&FileEntry> = index.files().collect();

        // Narrow to candidate files with the trigram index (literal patterns);
        // otherwise every file is a candidate.
        let candidates: Vec<usize> = match (&node.trigram, is_literal(pattern)) {
            (Some(tref), true) => match self.load_trigram(tref).await {
                Ok(idx) => idx
                    .candidates(pattern)
                    .into_iter()
                    .map(|d| d as usize)
                    .collect(),
                Err(_) => (0..files.len()).collect(),
            },
            _ => (0..files.len()).collect(),
        };

        for i in candidates {
            let Some(entry) = files.get(i) else { continue };
            let Ok(media) = file_media(entry) else {
                continue;
            };
            let Ok(bytes) = self.blobs.get(&media).await else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let count = u32::try_from(re.find_iter(&text).count()).unwrap_or(u32::MAX);
            if count == 0 {
                continue;
            }
            let (line_no, line_text) = first_match_line(&text, re);
            hits.push(Hit {
                path: format!("{prefix}{}", entry.name),
                token: token.as_str().to_owned(),
                line: line_no,
                text: line_text,
                matches: count,
            });
        }

        // Recurse into subdirectories, extending the path prefix.
        for sub in index.subdirs() {
            if let Ok(child) = Token::parse(&sub.token) {
                let child_prefix = format!("{prefix}{}/", sub.name);
                Box::pin(self.search_node(child, &child_prefix, pattern, re, hits, visited)).await;
            }
        }
    }

    /// Fetch and decode a node's directory index blob.
    async fn load_index(&self, node: &TreeNode) -> Result<DirIndex, Envelope> {
        let bytes = self
            .blobs
            .get(&node.index)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Envelope::err(format!("tree index decode: {e}"), vec![]))
    }

    /// Fetch and decode a node's trigram index blob.
    async fn load_trigram(&self, media: &MediaRef) -> Result<TrigramIndex, Envelope> {
        let bytes = self
            .blobs
            .get(media)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Envelope::err(format!("trigram decode: {e}"), vec![]))
    }
}

/// Rebuild the blob reference for a file entry from its recorded hash, size, and
/// type — the blob store is content-addressed, so the hash is the whole key.
fn file_media(entry: &FileEntry) -> Result<MediaRef, Envelope> {
    Ok(MediaRef {
        uri: CanonicalUrl::new(&format!("blob://{}", entry.sha256))
            .map_err(|e| Envelope::err(format!("blob uri: {e}"), vec![]))?,
        content_type: entry.content_type.clone(),
        size: entry.size,
        sha256: Sha256Hex::new(&entry.sha256)
            .map_err(|e| Envelope::err(format!("sha: {e}"), vec![]))?,
    })
}

/// A pattern with no regex metacharacters — safe to feed to the trigram index and
/// Bloom directly. A metacharacter means the literal trigrams may not appear in
/// the content, so we fall back to grepping every candidate (never miss).
fn is_literal(pattern: &str) -> bool {
    !pattern.chars().any(|c| ".*+?()[]{}|^$\\".contains(c))
}

/// The 1-based line number and text of the first line matching `re`.
fn first_match_line(text: &str, re: &regex::Regex) -> (u32, String) {
    for (i, line) in text.lines().enumerate() {
        if re.is_match(line) {
            let trimmed: String = line.chars().take(200).collect();
            return (u32::try_from(i + 1).unwrap_or(u32::MAX), trimmed);
        }
    }
    (1, String::new())
}
