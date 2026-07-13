//! The directory index — one content-addressed node of the Merkle tree.
//!
//! A tree mint records a directory not as a flat list of individually-minted
//! files but as a hierarchy of *nodes*. Each node is a [`DirIndex`]: the ordered
//! entries directly inside one directory. An entry is either a [`FileEntry`] (a
//! file, pinned by content hash — the bytes travel with the tree, so a deleted
//! file still reads) or a [`SubdirEntry`] (a child directory, addressed by its
//! own subtree token).
//!
//! Two things fall out of this shape:
//!
//! * **Merkle identity.** A node's content is a pure function of its entries, and
//!   an entry names a file by `sha256`. Identical files dedupe; a changed file
//!   changes only its node and that node's ancestors — cheap change detection and
//!   cheap re-mint.
//! * **Sizing without materialising.** Each subdir entry carries its subtree's
//!   `files` and `bytes` totals, so a node knows the weight of everything beneath
//!   it from the index alone — no walk, no token minting, to answer "how big."
//!
//! Sans-I/O: this module builds and reads the index structure. The filesystem
//! walk that produces entries, and the blob store that holds file bytes, live in
//! `waggle-mcp`.

use serde::{Deserialize, Serialize};

/// A file inside a directory node, pinned by content hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Name within this directory (not a full path).
    pub name: String,
    /// Lowercase-hex SHA-256 of the file's bytes — the blob key, and the Merkle
    /// leaf hash.
    pub sha256: String,
    /// Size in bytes.
    pub size: u64,
    /// MIME type inferred at mint (`text/markdown`, `application/pdf`, …).
    pub content_type: String,
}

/// A child directory, addressed by its own subtree token, with the totals a
/// parent needs to report size and to plan a search without descending.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubdirEntry {
    /// Name within this directory.
    pub name: String,
    /// The subtree's own token — the handle to recurse into.
    pub token: String,
    /// Files anywhere beneath this subdirectory (recursive).
    pub files: u64,
    /// Bytes anywhere beneath this subdirectory (recursive).
    pub bytes: u64,
}

/// One entry of a directory node: a file or a subdirectory. Serialised as a
/// tagged union so the wire form is self-describing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Entry {
    /// A file, pinned by hash.
    File(FileEntry),
    /// A subdirectory, addressed by token.
    Dir(SubdirEntry),
}

impl Entry {
    /// The entry's name within its directory.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Entry::File(f) => &f.name,
            Entry::Dir(d) => &d.name,
        }
    }
}

/// The entries directly inside one directory, kept in a stable order so the
/// serialised node is byte-identical for identical input (Merkle stability).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirIndex {
    /// Sorted by name (files and subdirs interleaved by name) — see
    /// [`DirIndex::from_entries`].
    pub entries: Vec<Entry>,
}

impl DirIndex {
    /// Build from arbitrary entries, sorting by name so the result is
    /// deterministic regardless of the order the caller discovered them in.
    #[must_use]
    pub fn from_entries(mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|a, b| a.name().cmp(b.name()));
        Self { entries }
    }

    /// Files directly in this directory (not recursive).
    pub fn files(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter().filter_map(|e| match e {
            Entry::File(f) => Some(f),
            Entry::Dir(_) => None,
        })
    }

    /// Subdirectories directly in this directory.
    pub fn subdirs(&self) -> impl Iterator<Item = &SubdirEntry> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Dir(d) => Some(d),
            Entry::File(_) => None,
        })
    }

    /// Total files beneath this node, recursive: local files plus every subdir's
    /// recorded subtree total. O(entries), no descent — the subdir totals were
    /// computed when those subtrees were minted.
    #[must_use]
    pub fn total_files(&self) -> u64 {
        let local = self.files().count() as u64;
        let sub: u64 = self.subdirs().map(|d| d.files).sum();
        local + sub
    }

    /// Total bytes beneath this node, recursive (same accounting as
    /// [`DirIndex::total_files`]).
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        let local: u64 = self.files().map(|f| f.size).sum();
        let sub: u64 = self.subdirs().map(|d| d.bytes).sum();
        local + sub
    }

    /// Look up a directly-contained entry by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> Entry {
        Entry::File(FileEntry {
            name: name.into(),
            sha256: format!("{name}-hash"),
            size,
            content_type: "text/plain".into(),
        })
    }

    fn dir(name: &str, files: u64, bytes: u64) -> Entry {
        Entry::Dir(SubdirEntry {
            name: name.into(),
            token: format!("{name}-tok"),
            files,
            bytes,
        })
    }

    #[test]
    fn entries_sort_by_name_for_stable_bytes() {
        let a = DirIndex::from_entries(vec![file("z.md", 1), file("a.md", 1)]);
        let b = DirIndex::from_entries(vec![file("a.md", 1), file("z.md", 1)]);
        assert_eq!(a, b);
        assert_eq!(a.entries[0].name(), "a.md");
    }

    #[test]
    fn totals_roll_up_subdir_recorded_counts() {
        let idx = DirIndex::from_entries(vec![file("readme.md", 100), dir("sub", 40, 4_000)]);
        assert_eq!(idx.total_files(), 1 + 40);
        assert_eq!(idx.total_bytes(), 100 + 4_000);
    }

    #[test]
    fn files_and_subdirs_partition_entries() {
        let idx = DirIndex::from_entries(vec![file("a", 1), dir("b", 1, 1), file("c", 1)]);
        assert_eq!(idx.files().count(), 2);
        assert_eq!(idx.subdirs().count(), 1);
    }

    #[test]
    fn serde_round_trip() {
        let idx = DirIndex::from_entries(vec![file("a.md", 10), dir("sub", 3, 30)]);
        let json = serde_json::to_string(&idx).unwrap();
        let back: DirIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(idx, back);
    }
}
