//! Pure pieces of tree search: a query plan, a hit, and ranking.
//!
//! The *traversal* — descend the lineage, prune with each node's Bloom, load a
//! node's trigram index, grep the survivors — is I/O and lives in `waggle-mcp`.
//! What lives here is everything that must be deterministic and testable in
//! isolation: how a query decides whether a subtree is worth entering, and how
//! the confirmed hits are ordered before they are returned.

use crate::bloom::Bloom;
use crate::trigram::TrigramIndex;

/// The decision a node makes about a query before spending any I/O on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prune {
    /// The node's Bloom proves the query cannot match anywhere beneath it. Skip
    /// the whole subtree — no index load, no descent.
    Skip,
    /// The query might match. The caller should load this node's trigram index,
    /// grep its candidate files, and recurse into its subdirectories.
    Enter,
}

/// Ask a node's Bloom summary whether a query is worth entering. This is the
/// prune gate that makes deep trees sublinear: a `Skip` costs one manifest read.
#[must_use]
pub fn prune(summary: &Bloom, query: &str) -> Prune {
    if summary.might_contain_all(query) {
        Prune::Enter
    } else {
        Prune::Skip
    }
}

/// The documents in a node worth grepping for a query: the trigram index narrows
/// a node's own files to those that carry every query trigram. The caller then
/// confirms with a real match (the trigram index can admit false positives; grep
/// removes them).
#[must_use]
pub fn candidates(index: &TrigramIndex, query: &str) -> Vec<u32> {
    index.candidates(query)
}

/// One confirmed match, with everything a consumer needs to drill in: the file's
/// path (relative to the searched root), the subtree token that owns it, the line
/// and its text, and a match count used for ranking.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Path from the searched root, e.g. `design/roadmap/retry.md`.
    pub path: String,
    /// The token of the subtree node that directly contains the file — the
    /// handle a consumer resolves to read more.
    pub token: String,
    /// The file's ordinal in its node's `DirIndex.files()` order — a stable
    /// position into the signed directory index. Carried so the caller can
    /// stamp a per-file read (I-1-safe like a region touch) on the owning node.
    pub entry: u32,
    /// 1-based line number of the first match in the file.
    pub line: u32,
    /// The matching line's text (already budget-trimmed by the caller).
    pub text: String,
    /// How many times the pattern matched in the file — the ranking signal.
    pub matches: u32,
}

/// Rank hits best-first, then truncate to `limit`. The ordering, in priority:
///
/// 1. **more matches first** — a file that mentions the pattern ten times is more
///    likely the one you want than a file that mentions it once;
/// 2. **shallower paths first** — a top-level file usually outranks something
///    buried deep, when match counts tie;
/// 3. **path, lexicographically** — a stable final tie-break so results are
///    deterministic run to run.
#[must_use]
pub fn rank(mut hits: Vec<Hit>, limit: usize) -> Vec<Hit> {
    hits.sort_by(|a, b| {
        b.matches
            .cmp(&a.matches)
            .then_with(|| depth(&a.path).cmp(&depth(&b.path)))
            .then_with(|| a.path.cmp(&b.path))
    });
    hits.truncate(limit);
    hits
}

/// Path depth = number of `/` separators. `retry.md` is 0, `a/b/retry.md` is 2.
fn depth(path: &str) -> usize {
    path.bytes().filter(|&b| b == b'/').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, matches: u32) -> Hit {
        Hit {
            path: path.into(),
            token: "t".into(),
            entry: 0,
            line: 1,
            text: "x".into(),
            matches,
        }
    }

    #[test]
    fn prune_skips_when_bloom_excludes_query() {
        let mut b = Bloom::new();
        b.insert_text("retry budget");
        assert_eq!(prune(&b, "retry"), Prune::Enter);
        assert_eq!(prune(&b, "wholly-absent-xyz"), Prune::Skip);
    }

    #[test]
    fn rank_orders_by_match_count_then_depth_then_path() {
        let ranked = rank(
            vec![hit("deep/a/b/c.md", 1), hit("top.md", 1), hit("hot.md", 9)],
            10,
        );
        assert_eq!(ranked[0].path, "hot.md"); // most matches
        assert_eq!(ranked[1].path, "top.md"); // tie on matches, shallower
        assert_eq!(ranked[2].path, "deep/a/b/c.md");
    }

    #[test]
    fn rank_truncates_to_limit() {
        let hits: Vec<Hit> = (0..20).map(|i| hit(&format!("f{i}.md"), 1)).collect();
        assert_eq!(rank(hits, 5).len(), 5);
    }

    #[test]
    fn rank_is_deterministic_on_full_ties() {
        let mk = || vec![hit("b.md", 1), hit("a.md", 1), hit("c.md", 1)];
        assert_eq!(rank(mk(), 10), rank(mk(), 10));
        assert_eq!(rank(mk(), 10)[0].path, "a.md");
    }
}
