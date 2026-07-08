//! The content-addressed blob sidecar (design docs `07 §4`, rev 2.3):
//! bytes never ride the log — they live here, named by their SHA-256, and
//! manifests carry `MediaRef`s pointing at them. Dedupe is free (same hash
//! ⇒ same path), writes are atomic (tmp → rename), reads verify what they
//! fetched, and GC is mark-and-sweep against live references.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use waggle_core::{CanonicalUrl, MediaRef, Sha256Hex};
use waggle_store::{BlobSink, StoreError};

/// The blob store rooted at `<store-root>/blobs/`.
pub struct BlobStore {
    root: PathBuf,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

impl BlobStore {
    /// Open (or create) a blob store rooted at `root`.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        std::fs::create_dir_all(root)
            .map_err(|e| StoreError::Backend(format!("blob root: {e}")))?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    fn path_for(&self, sha: &str) -> PathBuf {
        self.root.join(&sha[..2]).join(sha)
    }

    /// Store bytes, returning the `MediaRef` a manifest carries. Identical
    /// bytes land at the identical path — dedupe by construction.
    pub fn put(&self, bytes: &[u8], content_type: &str) -> Result<MediaRef, StoreError> {
        let sha = hex(&Sha256::digest(bytes));
        let dest = self.path_for(&sha);
        if !dest.exists() {
            let Some(dir) = dest.parent() else {
                return Err(StoreError::Backend("blob path has no parent".into()));
            };
            std::fs::create_dir_all(dir)
                .map_err(|e| StoreError::Backend(format!("blob dir: {e}")))?;
            // Atomic: write beside, rename into place. A crash leaves only
            // a .tmp file GC sweeps; a reader never sees partial bytes.
            let tmp = dir.join(format!(".tmp-{sha}"));
            std::fs::write(&tmp, bytes)
                .map_err(|e| StoreError::Backend(format!("blob write: {e}")))?;
            std::fs::rename(&tmp, &dest)
                .map_err(|e| StoreError::Backend(format!("blob rename: {e}")))?;
        }
        Ok(MediaRef {
            uri: CanonicalUrl::new(&format!("blob://{sha}"))
                .map_err(|e| StoreError::Codec(format!("blob uri: {e}")))?,
            content_type: content_type.to_owned(),
            size: bytes.len() as u64,
            sha256: Sha256Hex::new(&sha).map_err(|e| StoreError::Codec(format!("sha: {e}")))?,
        })
    }

    /// Fetch and **verify** bytes for a `MediaRef` (integrity is the
    /// contract: a resolver never trusts what it fetched until the hash
    /// agrees — rev 2.3).
    pub fn get(&self, media: &MediaRef) -> Result<Vec<u8>, StoreError> {
        let sha = media.sha256.as_str();
        let bytes = std::fs::read(self.path_for(sha))
            .map_err(|e| StoreError::Backend(format!("blob read {sha}: {e}")))?;
        let actual = hex(&Sha256::digest(&bytes));
        if actual != sha {
            return Err(StoreError::Codec(format!(
                "blob {sha} failed integrity: stored bytes hash to {actual} — the blob is corrupt; re-fetch from the source"
            )));
        }
        Ok(bytes)
    }

    /// Mark-and-sweep: remove every blob whose sha is not in `live`
    /// (collected from live manifests' `MediaRef`s). Also sweeps orphaned
    /// `.tmp-` files from interrupted writes. Returns removed count.
    pub fn gc(&self, live: &HashSet<String>) -> Result<usize, StoreError> {
        let mut removed = 0;
        let dirs = std::fs::read_dir(&self.root)
            .map_err(|e| StoreError::Backend(format!("blob gc: {e}")))?;
        for dir in dirs.flatten() {
            let Ok(entries) = std::fs::read_dir(dir.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let dead = name.starts_with(".tmp-") || !live.contains(&name);
                if dead && std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

impl BlobSink for BlobStore {
    async fn put(&self, bytes: &[u8], content_type: &str) -> Result<MediaRef, StoreError> {
        Self::put(self, bytes, content_type)
    }
    async fn get(&self, media: &MediaRef) -> Result<Vec<u8>, StoreError> {
        Self::get(self, media)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("waggle-blobs-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&p).ok();
        p
    }

    #[test]
    fn blob_roundtrip_and_corruption_detected() {
        let root = temp_root("roundtrip");
        let store = BlobStore::open(&root).unwrap();
        let media = store
            .put(b"the whiteboard photo bytes", "image/png")
            .unwrap();
        assert!(media.uri.as_str().starts_with("blob://"));
        assert_eq!(media.size, 26);
        assert_eq!(store.get(&media).unwrap(), b"the whiteboard photo bytes");

        // Corrupt the stored file: get() must refuse it.
        let sha = media.sha256.as_str();
        std::fs::write(root.join(&sha[..2]).join(sha), b"tampered").unwrap();
        let err = store.get(&media).unwrap_err();
        assert!(err.to_string().contains("failed integrity"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cas_dedupe_same_bytes_one_file() {
        let root = temp_root("dedupe");
        let store = BlobStore::open(&root).unwrap();
        let a = store.put(b"identical bytes", "text/plain").unwrap();
        let b = store
            .put(b"identical bytes", "application/octet-stream")
            .unwrap();
        assert_eq!(a.sha256, b.sha256, "same bytes, same address");
        let sha = a.sha256.as_str();
        let files: Vec<_> = std::fs::read_dir(root.join(&sha[..2])).unwrap().collect();
        assert_eq!(files.len(), 1, "dedupe stores one file");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn gc_sweeps_dead_keeps_live() {
        let root = temp_root("gc");
        let store = BlobStore::open(&root).unwrap();
        let live_ref = store.put(b"still referenced", "text/plain").unwrap();
        let dead_ref = store.put(b"orphaned", "text/plain").unwrap();
        // An interrupted write leaves a tmp file behind.
        let sha = dead_ref.sha256.as_str();
        std::fs::write(root.join(&sha[..2]).join(".tmp-interrupted"), b"partial").unwrap();

        let live: HashSet<String> = [live_ref.sha256.as_str().to_owned()].into();
        let removed = store.gc(&live).unwrap();
        assert_eq!(removed, 2, "the orphan and the tmp file");
        assert!(store.get(&live_ref).is_ok(), "live blob survives");
        assert!(store.get(&dead_ref).is_err(), "dead blob is gone");
        std::fs::remove_dir_all(&root).ok();
    }
}
