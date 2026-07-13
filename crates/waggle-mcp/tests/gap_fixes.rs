//! The doc-20 §1 gap fixes, feature-independent: extension-less text
//! files keep the full text loop (gap 1), and `mint --tree` refuses to
//! walk generated/vendored trees (gap 2).

use serde_json::json;
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0x6A95_u32;
    move |buf: &mut [u8]| {
        for b in buf.iter_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = (state & 0xFF) as u8;
        }
        Ok(())
    }
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("waggle-gaps-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn handler(dir: &std::path::Path) -> Handler<SqliteStore, BlobStore> {
    Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
    .with_blobs(BlobStore::open(&dir.join("blobs")).unwrap())
}

#[test]
fn extension_less_text_files_get_the_text_loop() {
    let dir = scratch("sniff");
    // A Makefile (basename table) and a bare script (byte sniff).
    std::fs::write(dir.join("Makefile"), "build:\n\tcargo build\n").unwrap();
    std::fs::write(dir.join("release-notes"), "v2: the pricing changed\n").unwrap();
    let h = handler(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        for name in ["Makefile", "release-notes"] {
            let minted = h
                .dispatch(
                    "mint",
                    &json!({ "target": format!("file://{}", dir.join(name).display()), "snapshot": true }),
                    Timestamp::from_unix_ms(1),
                    &mut e,
                )
                .await;
            let token = minted.result["token"].as_str().unwrap().to_owned();
            let found = h
                .dispatch(
                    "search",
                    &json!({ "token": token, "pattern": "cargo|pricing" }),
                    Timestamp::from_unix_ms(2),
                    &mut e,
                )
                .await;
            assert!(found.hint.is_none(), "{name} must grep as text: {found:?}");
            assert_eq!(found.result["total_matches"], 1, "{name}");
        }
    });
}

#[test]
fn tree_mint_denies_generated_and_vendored_dirs() {
    let dir = scratch("tree");
    let repo = dir.join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::create_dir_all(repo.join("node_modules/dep")).unwrap();
    std::fs::create_dir_all(repo.join("target/debug")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn real() {}\n").unwrap();
    std::fs::write(repo.join("node_modules/dep/index.js"), "junk\n").unwrap();
    std::fs::write(repo.join("target/debug/artifact.txt"), "junk\n").unwrap();
    let h = handler(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = h
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", repo.display()), "tree": true }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        // node_modules/ and target/ are denied — only src/lib.rs survives.
        assert_eq!(
            minted.result["tree"]["files"], 1,
            "only src/lib.rs: {minted:?}"
        );
        let root = minted.result["token"].as_str().unwrap().to_owned();
        let toc = h
            .dispatch(
                "read",
                &json!({ "token": root }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        // The one surviving file lives under src/, reachable through the projection.
        assert_eq!(toc.result["total_files"], 1);
        let src = toc.result["dirs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == "src");
        assert!(src.is_some(), "src/ is the only subdir: {toc:?}");
    });
}
