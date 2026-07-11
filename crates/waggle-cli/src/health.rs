//! Health facts for `waggled/status` (docs `07` retention, `21 §3`):
//! disk weight — the store files and the blob CAS — so growth is
//! visible long before it is a problem, and the daemon's status answer
//! stays the one place to look.

use std::path::Path;

/// Size of one file, 0 when absent.
fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |m| m.len())
}

/// Disk stats for the store at `db`: the `SQLite` files (db + WAL + shm)
/// and the sibling blob CAS (`blobs/<aa>/<hash>` — two fixed levels,
/// doc 07 §4). One shallow walk, no recursion beyond the CAS shape.
pub(crate) fn disk_stats(db: &Path) -> serde_json::Value {
    let store_bytes: u64 = ["", "-wal", "-shm"]
        .iter()
        .map(|suffix| {
            let mut p = db.as_os_str().to_owned();
            p.push(suffix);
            file_bytes(Path::new(&p))
        })
        .sum();
    let blobs_dir = db.parent().map(|d| d.join("blobs"));
    let (mut blob_count, mut blob_bytes) = (0u64, 0u64);
    if let Some(dir) = blobs_dir {
        for shard in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if !shard.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            for blob in std::fs::read_dir(shard.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                if let Ok(meta) = blob.metadata() {
                    if meta.is_file() && !blob.file_name().to_string_lossy().ends_with(".tmp") {
                        blob_count += 1;
                        blob_bytes += meta.len();
                    }
                }
            }
        }
    }
    serde_json::json!({
        "store_bytes": store_bytes,
        "blobs": { "count": blob_count, "bytes": blob_bytes },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_stats_count_the_cas_and_skip_tmp() {
        let root = std::env::temp_dir().join(format!("waggle-health-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let shard = root.join("blobs").join("ab");
        std::fs::create_dir_all(&shard).unwrap();
        let db = root.join("waggle.db");
        std::fs::write(&db, vec![0u8; 100]).unwrap();
        std::fs::write(root.join("waggle.db-wal"), vec![0u8; 50]).unwrap();
        std::fs::write(shard.join("abcd"), vec![0u8; 7]).unwrap();
        std::fs::write(shard.join("efgh"), vec![0u8; 5]).unwrap();
        std::fs::write(shard.join("partial.tmp"), vec![0u8; 999]).unwrap();

        let stats = disk_stats(&db);
        assert_eq!(stats["store_bytes"], 150);
        assert_eq!(stats["blobs"]["count"], 2, "GC-sweepable .tmp never counts");
        assert_eq!(stats["blobs"]["bytes"], 12);

        // A store with no blobs dir reports zeros, never errors.
        let bare = root.join("bare").join("waggle.db");
        std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
        let empty = disk_stats(&bare);
        assert_eq!(empty["store_bytes"], 0);
        assert_eq!(empty["blobs"]["count"], 0);
    }
}
