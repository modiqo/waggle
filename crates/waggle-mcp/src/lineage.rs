//! Lineage: the folder/mission pattern (doc `02`, guide 06). A parent
//! token is a LINEAGE ROOT: `mint --tree` snapshots a directory as its
//! children, resolution serves the root's index, funnels roll
//! descendants up, and a revoked ancestor tombstones everything
//! beneath it — one revocation for the whole tree.

use serde_json::{json, Value};
use waggle_core::Timestamp;
use waggle_store::{BlobSink, Store};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::Handler;
use crate::map::handoff_line;

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// `mint --tree`: the folder pattern as one call. The root token is
    /// already minted; every file inside (recursive, sorted, dotfiles
    /// skipped, capped) becomes a snapshot-pinned CHILD — one revocation
    /// covers the tree, and the root's funnel rolls the children up.
    pub(crate) async fn mint_tree<E>(
        &self,
        root: &waggle_store::ManifestView,
        now: Timestamp,
        entropy: &mut E,
    ) -> Envelope
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        const CAP: usize = 200;
        let token = root.manifest.token;
        let target = root.manifest.target.as_str();
        let Some(dir) = crate::content_handlers::local_path(target) else {
            return Envelope::err(
                format!("tree: `{target}` is not a local directory — --tree walks the filesystem"),
                vec![],
            );
        };
        let mut files = Vec::new();
        collect_files(std::path::Path::new(&dir), &mut files);
        files.sort();
        if files.len() > CAP {
            return Envelope::err(
                format!(
                    "tree: {} files exceeds the {CAP}-file cap — mint subfolders as their own trees",
                    files.len()
                ),
                vec![],
            );
        }
        let children = match self.mint_children(root, files, now, entropy).await {
            Ok(c) => c,
            Err(e) => return e,
        };
        let child_count = children.len();
        Envelope::ok(
            json!({
                "token": token.as_str(),
                "handoff": handoff_line(token.as_str()),
                "children": children,
                "tree": { "files": child_count },
            }),
            vec![
                NextCall {
                    tool: "funnel".into(),
                    args: json!({ "token": token.as_str() }),
                    why: "the root's funnel rolls up every child".into(),
                },
                NextCall {
                    tool: "mutate".into(),
                    args: json!({ "token": token.as_str(), "change": "revoke", "expected-version": 1 }),
                    why: "one revocation tombstones the whole tree".into(),
                },
            ],
        )
        .with_stats(Stats {
            records: Some(1 + child_count as u64),
            seq: Some(0),
        })
    }

    /// The tree's children: each file snapshot-minted under the root.
    async fn mint_children<E>(
        &self,
        root: &waggle_store::ManifestView,
        files: Vec<std::path::PathBuf>,
        now: Timestamp,
        entropy: &mut E,
    ) -> Result<Vec<Value>, Envelope>
    where
        E: FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError>,
    {
        let token = root.manifest.token;
        let mut children = Vec::new();
        for file in files {
            let child_args = json!({
                "target": format!("file://{}", file.display()),
                "parent": token.as_str(),
                "snapshot": true,
                "sharer": root.manifest.sharer.as_str(),
                "channel": root.manifest.channel.as_str(),
            });
            let child = Box::pin(self.mint(
                child_args.as_object().expect("literal object"),
                now,
                entropy,
            ))
            .await;
            match child.hint {
                None => children.push(json!({
                    "token": child.result["token"],
                    "target": format!("file://{}", file.display()),
                })),
                Some(hint) => {
                    return Err(Envelope::err(
                        format!("tree: {}: {hint}", file.display()),
                        vec![],
                    ))
                }
            }
        }
        Ok(children)
    }

    /// Walk the parent chain: the earliest revocation timestamp among
    /// ancestors, if any (lineage cascade — revoking a folder/mission
    /// token tombstones everything minted under it). Depth-capped.
    pub(crate) async fn ancestor_revoked_at(
        &self,
        manifest: &waggle_core::AttributionManifest,
    ) -> Option<waggle_core::Timestamp> {
        let mut parent = manifest.parent;
        for _ in 0..32 {
            let token = parent?;
            let view = self.store.manifest(token).await.ok().flatten()?;
            if let Some(at) = view.manifest.revoked_at {
                return Some(at);
            }
            parent = view.manifest.parent;
        }
        None
    }
}

/// The mint envelope's forward paths; a directory target is a fine
/// LOCATOR but read/search need files — teach the lineage pattern (or
/// `--tree`) up front.
pub(crate) fn mint_next(token: &str, target: &str) -> Vec<NextCall> {
    let mut next = vec![
        NextCall {
            tool: "resolve".into(),
            args: json!({ "token": token }),
            why: "self-check the projection consumers will receive".into(),
        },
        NextCall {
            tool: "map".into(),
            args: json!({ "token": token }),
            why: "orient around the new token".into(),
        },
    ];
    if crate::content_handlers::local_path(target)
        .is_some_and(|p| std::path::Path::new(&p).is_dir())
    {
        next.insert(
            0,
            NextCall {
                tool: "mint".into(),
                args: json!({ "target": format!("{target}/<file>"), "parent": token }),
                why: "the target is a folder — re-mint with tree=true to snapshot every file as a child, or mint files individually with parent=<this-token>".into(),
            },
        );
    }
    next
}

/// Recursive file collection for `mint --tree`: dotfiles and dot-dirs
/// skipped, symlinks not followed (walk what IS the folder).
fn collect_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, out);
        } else if meta.is_file() {
            out.push(path);
        }
    }
}
