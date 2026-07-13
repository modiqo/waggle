//! Indexed tree minting (design doc: tree-scale).
//!
//! A `mint --tree` records a directory as a **Merkle hierarchy of directory
//! nodes**, one signed manifest per directory (not per file). Each node carries
//! its [`DirIndex`], a [`TrigramIndex`] over its own files, and a [`Bloom`]
//! summary of every trigram beneath it — the structures `waggle-tree` defines.
//!
//! Two properties this module is responsible for upholding:
//!
//! * **Content eager, tokens lazy (C-1 preserved).** Every file's bytes are pinned
//!   into the blob store here, so a deleted file still reads. But a file is *not*
//!   given its own token/manifest — it is an entry in its directory node's index.
//!   The cap that used to bound files now bounds only directory nodes (few), so a
//!   corpus of thousands of files mints in one call.
//! * **Bloom composes up, tokens flow down.** A node's Bloom is the union of its
//!   children's, which needs the children first; a child's `parent` link needs the
//!   node's token first. The cycle is broken by **pre-generating** each node's
//!   token ([`MintSpec::with_token`]) before descending.

use serde_json::{json, Map, Value};
use waggle_core::{MintOptions, MintSpec, Timestamp, Token};
use waggle_store::{AppendIntent, Appended, BlobSink, MintNonce, Store};
use waggle_tree::{Bloom, DirIndex, Entry, FileEntry, SubdirEntry, TrigramIndex};

use crate::content_handlers::{local_path, read_capped};
use crate::envelope::Envelope;
use crate::handlers::{infer_content_type, Handler};

/// Content types for the two index blobs a node stores.
const DIRINDEX_CT: &str = "application/waggle-dirindex+json";
const TRIGRAM_CT: &str = "application/waggle-trigram+json";

/// Default ceiling on total bytes eagerly snapshotted by one tree mint, so nobody
/// accidentally pins a huge media folder. Override with `max-bytes` on the mint.
const DEFAULT_BUDGET: u64 = 256 * 1024 * 1024;

/// A fully computed directory node, ready to mint. Built bottom-up in phase one
/// (blobs pinned, indexes built, token pre-generated) so that phase two can mint
/// manifests **top-down** — a child's `parent` link needs the parent's manifest to
/// already exist in the store, which post-order minting cannot provide.
struct NodeData {
    token: Token,
    /// `file://` URL of this directory.
    dir_url: String,
    /// The `tree` field this node's manifest will carry.
    tree_node: waggle_core::TreeNode,
    /// Subtree Bloom — union of children's plus local files (for the parent).
    bloom: Bloom,
    files: u64,
    bytes: u64,
    /// Child directory nodes, in name order.
    children: Vec<NodeData>,
}

/// One file gathered during a directory walk, before the node is assembled.
struct FileRec {
    name: String,
    sha256: String,
    size: u64,
    content_type: String,
    /// Text fed to the trigram index and Bloom — the file's own bytes if text, a
    /// deterministic extraction for PDF/HTML, empty for opaque media.
    index_text: String,
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// Mint a directory as an indexed Merkle tree. Returns the root token's
    /// envelope. Called by the `mint --tree` path in place of the old flat
    /// per-file minting.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn mint_tree_indexed<E>(
        &self,
        dir: &str,
        sharer: &str,
        channel: &str,
        parent: Option<Token>,
        budget: u64,
        args: &Map<String, Value>,
        now: Timestamp,
        entropy: &mut E,
    ) -> Envelope
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let Some(path) = local_path(dir) else {
            return Envelope::err(
                format!("tree: `{dir}` is not a local directory — --tree walks the filesystem"),
                vec![],
            );
        };
        // Phase one: compute the whole tree bottom-up — pin blobs, build indexes,
        // pre-generate tokens. No manifests yet.
        let mut spent = 0u64;
        let root = match Box::pin(self.build_node(
            std::path::Path::new(&path),
            budget,
            &mut spent,
            entropy,
        ))
        .await
        {
            Ok(n) => n,
            Err(e) => return e,
        };
        let (files, bytes, token) = (root.files, root.bytes, root.token);
        // Phase two: mint manifests top-down, so each parent exists before its
        // children link to it.
        if let Err(e) =
            Box::pin(self.mint_nodes(&root, sharer, channel, parent, true, args, now, entropy))
                .await
        {
            return e;
        }
        Envelope::ok(
            json!({
                "token": token.as_str(),
                "handoff": crate::map::handoff_line(token.as_str()),
                "tree": { "files": files, "bytes": bytes },
            }),
            vec![
                crate::envelope::NextCall {
                    tool: "search".into(),
                    args: json!({ "token": token.as_str(), "pattern": "<regex>" }),
                    why: "one search spans the whole tree — pruned and ranked".into(),
                },
                crate::envelope::NextCall {
                    tool: "read".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "the directory's table of contents".into(),
                },
            ],
        )
    }

    /// Phase one — compute one directory node and its whole subtree, bottom-up:
    /// pin every file's bytes, build the node's index/trigram/Bloom, pre-generate
    /// its token, and recurse. Mints nothing; returns a [`NodeData`] tree.
    async fn build_node<E>(
        &self,
        dir: &std::path::Path,
        budget: u64,
        spent: &mut u64,
        entropy: &mut E,
    ) -> Result<NodeData, Envelope>
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let token =
            Token::generate(8, entropy).map_err(|e| Envelope::err(e.to_string(), vec![]))?;

        let mut files: Vec<FileRec> = Vec::new();
        let mut children: Vec<NodeData> = Vec::new();
        let mut bloom = Bloom::new();
        let (mut total_files, mut total_bytes) = (0u64, 0u64);

        for entry in read_sorted(dir)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_ignored(&name) {
                continue;
            }
            if entry.path().is_dir() {
                let sub = Box::pin(self.build_node(&entry.path(), budget, spent, entropy)).await?;
                bloom.union(&sub.bloom);
                total_files += sub.files;
                total_bytes += sub.bytes;
                children.push(sub);
            } else if entry.path().is_file() {
                let rec = self
                    .snapshot_file(&entry.path(), &name, budget, spent)
                    .await?;
                if !rec.index_text.is_empty() {
                    bloom.insert_text(&rec.index_text);
                }
                total_files += 1;
                total_bytes += rec.size;
                files.push(rec);
            }
        }

        // A name-sorted index; the trigram index's doc ids align with its file
        // order, so a search candidate maps straight back to a file entry.
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let mut trigram = TrigramIndex::builder();
        let mut any_text = false;
        for f in &files {
            trigram.add(&f.index_text);
            any_text |= !f.index_text.is_empty();
        }

        let mut entries: Vec<Entry> = files
            .iter()
            .map(|f| {
                Entry::File(FileEntry {
                    name: f.name.clone(),
                    sha256: f.sha256.clone(),
                    size: f.size,
                    content_type: f.content_type.clone(),
                })
            })
            .collect();
        for sub in &children {
            entries.push(Entry::Dir(SubdirEntry {
                name: dir_name(&sub.dir_url),
                token: sub.token.as_str().to_owned(),
                files: sub.files,
                bytes: sub.bytes,
            }));
        }

        let index_ref = self
            .put_json(&DirIndex::from_entries(entries), DIRINDEX_CT)
            .await?;
        let trigram_ref = if any_text {
            Some(self.put_json(&trigram.build(), TRIGRAM_CT).await?)
        } else {
            None
        };

        Ok(NodeData {
            token,
            dir_url: format!("file://{}", dir.display()),
            tree_node: waggle_core::TreeNode {
                index: index_ref,
                trigram: trigram_ref,
                bloom: bloom.to_hex(),
                files: total_files,
                bytes: total_bytes,
            },
            bloom,
            files: total_files,
            bytes: total_bytes,
            children,
        })
    }

    /// Phase two — mint manifests top-down, so a parent is in the store before its
    /// children link to it.
    #[allow(clippy::too_many_arguments)]
    async fn mint_nodes<E>(
        &self,
        node: &NodeData,
        sharer: &str,
        channel: &str,
        parent: Option<Token>,
        is_root: bool,
        args: &Map<String, Value>,
        now: Timestamp,
        entropy: &mut E,
    ) -> Result<(), Envelope>
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        // Tags (and any root-only mint options) land on the root node, so `find`
        // recovers the tree by the name a human remembers.
        let root_args = is_root.then_some(args);
        self.mint_node_manifest(node, sharer, channel, parent, root_args, now, entropy)
            .await?;
        for child in &node.children {
            Box::pin(self.mint_nodes(
                child,
                sharer,
                channel,
                Some(node.token),
                false,
                args,
                now,
                entropy,
            ))
            .await?;
        }
        Ok(())
    }

    /// Pin one file's bytes eagerly and gather its index record. Enforces the byte
    /// budget as it goes so a runaway corpus fails fast, not after gigabytes.
    async fn snapshot_file(
        &self,
        path: &std::path::Path,
        name: &str,
        budget: u64,
        spent: &mut u64,
    ) -> Result<FileRec, Envelope> {
        let bytes = read_capped(&path.to_string_lossy())?;
        *spent += bytes.len() as u64;
        if *spent > budget {
            return Err(Envelope::err(
                format!(
                    "tree: snapshot exceeds the {} MB budget at `{}` — mint a subfolder, or raise max-bytes",
                    budget / (1024 * 1024),
                    path.display()
                ),
                vec![],
            ));
        }
        let content_type = infer_content_type(&path.to_string_lossy()).to_owned();
        let media = self
            .blobs
            .put(&bytes, &content_type)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
        let index_text = index_text(&content_type, &bytes);
        Ok(FileRec {
            name: name.to_owned(),
            sha256: media.sha256.as_str().to_owned(),
            size: bytes.len() as u64,
            content_type,
            index_text,
        })
    }

    /// Mint (and sign, and persist) one directory node's manifest under its
    /// pre-generated token, carrying its `tree` field.
    #[allow(clippy::too_many_arguments)]
    async fn mint_node_manifest<E>(
        &self,
        node: &NodeData,
        sharer: &str,
        channel: &str,
        parent: Option<Token>,
        root_args: Option<&Map<String, Value>>,
        now: Timestamp,
        entropy: &mut E,
    ) -> Result<(), Envelope>
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let target = waggle_core::CanonicalUrl::new(&node.dir_url)
            .map_err(|e| Envelope::err(format!("tree target: {e}"), vec![]))?;
        let sharer = waggle_core::Sharer::new(sharer)
            .map_err(|e| Envelope::err(format!("sharer: {e}"), vec![]))?;
        let channel = waggle_core::Channel::new(channel)
            .map_err(|e| Envelope::err(format!("channel: {e}"), vec![]))?;
        let mut spec = MintSpec::new(target, sharer, channel)
            .with_token(node.token)
            .tree(node.tree_node.clone());
        if let Some(p) = parent {
            spec = spec.child_of(p);
        }
        if let Some(args) = root_args {
            spec = crate::discovery::apply_tags(spec, args);
        }
        let mut manifest = waggle_core::mint(spec, &MintOptions::default(), &mut *entropy, now)
            .map_err(|e| Envelope::err(e.to_string(), vec![]))?;
        if let Some(signer) = &self.signer {
            manifest.signature = Some(waggle_core::trust::sign_manifest(&manifest, signer));
        }
        let mut nonce = [0u8; 8];
        entropy(&mut nonce).map_err(|e| Envelope::err(format!("entropy: {e}"), vec![]))?;
        match self
            .store
            .append(AppendIntent::Mint {
                manifest: Box::new(manifest),
                nonce: MintNonce(u64::from_le_bytes(nonce)),
            })
            .await
        {
            Ok(Appended::Minted { .. }) => Ok(()),
            Ok(_) => Err(Envelope::err("tree: non-mint receipt for a node", vec![])),
            Err(e) => Err(crate::handlers::store_err(&e)),
        }
    }

    /// Serialize a value to JSON and pin it as a content-addressed blob.
    async fn put_json<T: serde::Serialize>(
        &self,
        value: &T,
        content_type: &str,
    ) -> Result<waggle_core::MediaRef, Envelope> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| Envelope::err(format!("tree index encode: {e}"), vec![]))?;
        self.blobs
            .put(&bytes, content_type)
            .await
            .map_err(|e| Envelope::err(e.to_string(), vec![]))
    }
}

/// The text a file contributes to the trigram index and Bloom: its own bytes when
/// text, a deterministic extraction for PDF/HTML, and nothing for opaque media
/// (which carries no searchable text — search simply never matches it, correctly).
fn index_text(content_type: &str, bytes: &[u8]) -> String {
    if crate::content::is_text(content_type) || crate::content::sniff_is_text(bytes) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    crate::extract::deterministic_extract(content_type, bytes)
        .map(|e| e.text)
        .unwrap_or_default()
}

/// The final path component of a `file://…/dir` URL — a subdir's name within its
/// parent.
fn dir_name(dir_url: &str) -> String {
    dir_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(dir_url)
        .to_owned()
}

/// Directory entries, sorted by name for deterministic node bytes.
fn read_sorted(dir: &std::path::Path) -> Result<Vec<std::fs::DirEntry>, Envelope> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| Envelope::err(format!("tree: read `{}`: {e}", dir.display()), vec![]))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries)
}

/// Names a tree mint skips: VCS internals, dependency caches, and dotfiles — the
/// generated/vendored bulk that would bloat storage without being the artifact.
fn is_ignored(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".hg" | ".svn" | "node_modules" | "target" | ".venv" | "__pycache__" | ".DS_Store"
    ) || name.starts_with('.')
}

/// Did the caller ask for a tree mint?
pub(crate) fn wants_tree(args: &Map<String, Value>) -> bool {
    args.get("tree").and_then(Value::as_bool).unwrap_or(false)
        || args.get("tree").and_then(Value::as_str) == Some("true")
}

/// The byte budget for a tree mint: `max-bytes` if the caller set one, else
/// [`DEFAULT_BUDGET`].
pub(crate) fn tree_budget(args: &Map<String, Value>) -> u64 {
    args.get("max-bytes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_BUDGET)
}
