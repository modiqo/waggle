//! Lineage: the folder/mission pattern (doc `02`, guide 06). A parent
//! token is a LINEAGE ROOT: `mint --tree` snapshots a directory as its
//! children, resolution serves the root's index, funnels roll
//! descendants up, and a revoked ancestor tombstones everything
//! beneath it — one revocation for the whole tree.

use serde_json::{json, Map, Value};
use waggle_store::{BlobSink, Store};

use crate::envelope::{Envelope, NextCall, Stats};
use crate::handlers::{parse_token_arg, store_err, Handler};

/// Was this folder minted `--require files:all`? Then the delegation needs the
/// WHOLE tree, and `coverage` answers with a verdict, not merely a fact.
fn requires_all_files(view: Option<&waggle_store::ManifestView>) -> bool {
    view.and_then(|v| v.manifest.contract.as_ref())
        .is_some_and(|c| {
            c.regions()
                .iter()
                .any(|r| r.label() == Some(crate::contract_args::TREE_ALL))
        })
}

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// Did the consumer actually receive this file's content, and did it run on
    /// it? Read off the child's funnel — the receipt, not a projection.
    async fn child_consumption(&self, child: waggle_core::Token) -> (bool, bool) {
        let funnel = self.store.funnel(child).await.unwrap_or_default();
        let count = |stage: &str| {
            funnel
                .iter()
                .find(|(s, _)| s.as_str() == stage)
                .map_or(0, |(_, n)| *n)
        };
        (count("read") + count("resolve") > 0, count("run") > 0)
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

impl<S: Store, B: BlobSink> Handler<S, B> {
    /// `coverage`: the folder handoff's proof of reading (see the
    /// catalog description). BFS over descendants; per file, the
    /// funnel says unread / read / run — and misses are NAMED.
    pub(crate) async fn coverage(&self, args: &Map<String, Value>) -> Envelope {
        let root = match parse_token_arg(args) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let view = match self.store.manifest(root).await {
            Ok(v) => v,
            Err(e) => return store_err(&e),
        };
        // An indexed tree (design doc: tree-scale) is a hierarchy of directory
        // NODES, not per-file tokens, so its coverage is node-granular: how many
        // directory nodes were touched, out of the total, plus the true file count
        // from the root's index. (Per-file `files:all` coverage needs a served-set
        // the payload-free log cannot express in an 8-bit region mask — a
        // documented follow-up.)
        if view.as_ref().is_some_and(|v| v.manifest.tree.is_some()) {
            return self.tree_node_coverage(root, view.as_ref().unwrap()).await;
        }
        let mut queue: std::collections::VecDeque<_> = match self.store.children(root).await {
            Ok(c) => c.into(),
            Err(e) => return store_err(&e),
        };
        // A childless token with a contract audits ITSELF (19 §4.2):
        // which declared regions did the served bytes actually reach?
        if queue.is_empty() {
            if let Some(contract) = view.as_ref().and_then(|v| v.manifest.contract.clone()) {
                return self.contract_coverage(root, &contract).await;
            }
        }
        let mut files = 0u64;
        let mut read = 0u64;
        let mut run = 0u64;
        let mut unread: Vec<Value> = Vec::new();
        let mut unread_total = 0u64;
        let mut visited = 0;
        while let Some(child) = queue.pop_front() {
            visited += 1;
            if visited > 1000 {
                break; // runaway-lineage backstop, counts stay honest per-file
            }
            if let Ok(more) = self.store.children(child).await {
                queue.extend(more);
            }
            let Ok(Some(view)) = self.store.manifest(child).await else {
                continue;
            };
            files += 1;
            let (was_read, was_run) = self.child_consumption(child).await;
            if was_run {
                run += 1;
                read += 1;
            } else if was_read {
                read += 1;
            } else {
                unread_total += 1;
                if unread.len() < 20 {
                    unread.push(json!({
                        "token": child.as_str(),
                        "target": view.manifest.target.as_str(),
                    }));
                }
            }
        }
        if files == 0 {
            return Envelope::err(
                format!(
                    "{root} has no children and no contract — coverage audits lineage roots \
                     (mint --tree, bundles) and contract-bearing tokens (mint --require)"
                ),
                vec![],
            );
        }
        let complete = unread_total == 0;
        // A folder minted `--require files:all` carries a COMPLETENESS
        // contract: this delegation needs the whole tree, and the receipt says
        // so. Without it, `complete` is a fact the orchestrator may consult;
        // with it, `met` is a verdict it can refuse an answer on.
        let requires_all = requires_all_files(view.as_ref());
        let next = tree_coverage_next(root, unread.first(), requires_all);
        let mut result = json!({
            "token": root.as_str(),
            "files": files,
            "read": format!("{read}/{files}"),
            "run": format!("{run}/{files}"),
            "complete": complete,
            "unread": unread,
            "unread_total": unread_total,
        });
        if requires_all {
            // The folder was minted `--require files:all`: this delegation needs
            // the WHOLE tree. `complete` was a fact an orchestrator could
            // consult; `met` is a verdict it can refuse an answer on.
            result["met"] = json!(complete);
            result["requires"] = json!(crate::contract_args::TREE_ALL);
        }
        Envelope::ok(result, next).with_stats(Stats {
            records: Some(files),
            seq: None,
        })
    }

    /// Node-granular coverage for an indexed tree: walk the directory-node
    /// lineage and report how many nodes have been read, the total node count, and
    /// the true file/byte totals from the root's index. A node is "read" once any
    /// of its files was served (search hit or `read --file`).
    async fn tree_node_coverage(
        &self,
        root: waggle_core::Token,
        view: &waggle_store::ManifestView,
    ) -> Envelope {
        let (total_files, total_bytes) = view
            .manifest
            .tree
            .as_ref()
            .map_or((0, 0), |t| (t.files, t.bytes));
        let mut queue: std::collections::VecDeque<_> = [root]
            .into_iter()
            .collect::<std::collections::VecDeque<_>>();
        let (mut nodes, mut read, mut visited) = (0u64, 0u64, 0);
        let mut unread: Vec<Value> = Vec::new();
        while let Some(node) = queue.pop_front() {
            visited += 1;
            if visited > 5000 {
                break; // runaway-lineage backstop
            }
            if let Ok(more) = self.store.children(node).await {
                queue.extend(more);
            }
            let Ok(Some(v)) = self.store.manifest(node).await else {
                continue;
            };
            let Some(tn) = v.manifest.tree.as_ref() else {
                continue;
            };
            // Only file-bearing nodes count: a pure container (subdirs only) has
            // nothing of its own to read, so it can neither be read nor block
            // completeness — it is covered when its children are.
            if !self.node_has_local_files(tn).await {
                continue;
            }
            nodes += 1;
            let (was_read, _) = self.child_consumption(node).await;
            if was_read {
                read += 1;
            } else if unread.len() < 20 {
                unread.push(json!({
                    "token": node.as_str(),
                    "target": v.manifest.target.as_str(),
                }));
            }
        }
        Envelope::ok(
            json!({
                "token": root.as_str(),
                "kind": "tree",
                "nodes": format!("{read}/{nodes}"),
                "total_files": total_files,
                "total_bytes": total_bytes,
                "complete": unread.is_empty(),
                "unread_nodes": unread,
            }),
            vec![NextCall {
                tool: "search".into(),
                args: json!({ "token": root.as_str(), "pattern": "<regex>" }),
                why: "reading through search stamps the nodes it serves".into(),
            }],
        )
        .with_stats(Stats {
            records: Some(nodes),
            seq: None,
        })
    }

    /// Does this node have files of its own (not just subdirectories)? Reads the
    /// node's directory index; a fetch failure conservatively counts as "yes" so a
    /// node is never silently dropped from coverage.
    async fn node_has_local_files(&self, node: &waggle_core::TreeNode) -> bool {
        match self.blobs.get(&node.index).await {
            Ok(bytes) => serde_json::from_slice::<waggle_tree::DirIndex>(&bytes)
                .map_or(true, |idx| idx.files().next().is_some()),
            Err(_) => true,
        }
    }

    /// Single-token contract coverage (19 §4.2): fold the region-touch
    /// bits out of the token's own records, evaluate against the
    /// declared contract, and NAME the misses — label and line range,
    /// with the read that would close the first gap as the next step.
    async fn contract_coverage(
        &self,
        token: waggle_core::Token,
        contract: &waggle_core::Contract,
    ) -> Envelope {
        let records = match self.store.scan_token(token, waggle_core::Seq(0)).await {
            Ok(r) => r,
            Err(e) => return store_err(&e),
        };
        let record_count = records.len() as u64;
        let touched_bits = waggle_core::replay(records, waggle_core::RegionTouchFold::default())
            .per_token
            .get(&token)
            .copied()
            .unwrap_or(0);
        let verdict = contract.evaluate(touched_bits);
        let describe = |i: &usize| {
            let r = &contract.regions()[*i];
            json!({
                "region": i,
                "label": r.label(),
                "lines": format!("{}-{}", r.start(), r.end()),
            })
        };
        let missed: Vec<Value> = verdict.missed.iter().map(describe).collect();
        let funnel = self.store.funnel(token).await.unwrap_or_default();
        let next = if let Some(first) = verdict.missed.first() {
            let r = &contract.regions()[*first];
            vec![NextCall {
                tool: "read".into(),
                args: json!({ "token": token.as_str(), "lines": format!("{}-{}", r.start(), r.end()) }),
                why: "close the gap: the first required region nobody reached".into(),
            }]
        } else {
            vec![NextCall {
                tool: "funnel".into(),
                args: json!({ "token": token.as_str() }),
                why: "contract met — the funnel has the stage story".into(),
            }]
        };
        Envelope::ok(
            json!({
                "token": token.as_str(),
                "contract": {
                    "required": verdict.required,
                    "touched": verdict.touched,
                    "permille": verdict.permille,
                    "min_permille": contract.min_permille(),
                },
                "met": verdict.met,
                "missed": missed,
                "outcome": waggle_core::outcome_of(&funnel),
            }),
            next,
        )
        .with_stats(Stats {
            records: Some(record_count),
            seq: None,
        })
    }
}

/// What to offer a consumer whose tree coverage is short.
///
/// Do not send someone to close an ELEVEN-file gap one file at a time. It cannot
/// be done inside a turn budget, and we measured what happens when you try: a
/// model told to open "the first file nobody has opened" fetched them singly,
/// exhausted its turns, and never answered — while the ungated arm, holding the
/// SAME correct answer, was simply allowed to give it. The contract demanded the
/// whole tree; the guidance offered a footpath.
///
/// The fan-out is the move that satisfies `files:all`: one call, one lens, every
/// file served. Offer THAT first. A refusal is only fair if the way to satisfy it
/// is on the table.
fn tree_coverage_next(
    root: waggle_core::Token,
    first_unread: Option<&Value>,
    requires_all: bool,
) -> Vec<NextCall> {
    let Some(first) = first_unread else {
        return vec![NextCall {
            tool: "funnel".into(),
            args: json!({ "token": root.as_str() }),
            why: "full coverage — the rollup has the totals".into(),
        }];
    };
    let mut n = Vec::new();
    if requires_all {
        n.push(NextCall {
            tool: "read".into(),
            args: json!({
                "token": root.as_str(),
                "section": "<a heading common to these files>",
            }),
            why: "THE CHEAP WAY TO CLOSE THIS: fans the lens across EVERY file in the tree \
                  at once. Check `complete` on the result — if it is false, page on with \
                  `from`. Files with no such section are not served and stay unread; read \
                  those directly. Fetching children one at a time will exhaust your turns \
                  before it closes the gap."
                .into(),
        });
    }
    n.push(NextCall {
        tool: "read".into(),
        args: json!({ "token": first["token"] }),
        why: if requires_all {
            "or close the gap one file at a time — slow, and the tree may outlast you"
        } else {
            "close the gap: the first file nobody has opened"
        }
        .into(),
    });
    n
}
