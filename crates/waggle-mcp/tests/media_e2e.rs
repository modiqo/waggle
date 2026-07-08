//! CP-6 gate `media_variant_by_modality` (rev 2.3, design doc `06 §2`):
//! mint with an attached image; the vision agent resolves the `MediaRef`,
//! fetches the bytes out-of-band, and verifies the hash; the text-only
//! agent gets the catch-all. Corrupted media is refused at fetch.

use serde_json::json;
use waggle_core::{MediaRef, Sharer, Timestamp};
use waggle_mcp::Handler;
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0xFACE_D00D_u32;
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

#[test]
fn media_variant_by_modality_end_to_end() {
    let dir = std::env::temp_dir().join(format!("waggle-media-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    // The whiteboard photo (fake PNG bytes; the pipeline doesn't decode).
    let image_path = dir.join("whiteboard.png");
    let image_bytes = b"\x89PNG-the-architecture-sketch".to_vec();
    std::fs::write(&image_path, &image_bytes).unwrap();

    let blobs_dir = dir.join("blobs");
    let handler = Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
    .with_blobs(Box::new(BlobStore::open(&blobs_dir).unwrap()));
    let mut e = entropy();

    pollster::block_on(async {
        // Mint with the attachment: one call, content-addressed storage,
        // the vision variant shaped automatically.
        let minted = handler
            .dispatch(
                "mint",
                &json!({
                    "target": "ws://standup/whiteboard-discussion",
                    "attach": image_path.to_str().unwrap(),
                }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();
        assert_eq!(minted.result["variants"], 2, "media variant + catch-all");

        // The vision agent: resolves to the MediaRef, fetches, verifies.
        let vision = handler
            .dispatch(
                "resolve",
                &json!({ "token": token, "context": {
                    "kind": "agent", "modalities": 9, "posture": "headless" } }), // TEXT|VISION
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert_eq!(
            vision.result["variant"], 0,
            "vision consumer gets the media variant"
        );
        let media: MediaRef =
            serde_json::from_value(vision.result["body"]["media"].clone()).unwrap();
        assert!(media.uri.as_str().starts_with("blob://"));
        assert_eq!(
            media.content_type, "image/png",
            "inferred from the extension"
        );
        assert_eq!(media.size, image_bytes.len() as u64);

        // Out-of-band fetch + hash verification (the resolver's duty).
        let blobs = BlobStore::open(&blobs_dir).unwrap();
        let fetched = blobs.get(&media).unwrap();
        assert_eq!(fetched, image_bytes, "bytes round-trip content-addressed");

        // The text-only agent: same token, the catch-all instead.
        let text_only = handler
            .dispatch(
                "resolve",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(text_only.result["variant"], 1, "no vision ⇒ catch-all");
        assert!(text_only.result["body"]["inline"].is_object());

        // Tampered media is refused at fetch — integrity is the contract.
        let sha = media.sha256.as_str();
        std::fs::write(blobs_dir.join(&sha[..2]).join(sha), b"tampered").unwrap();
        let err = blobs.get(&media).unwrap_err();
        assert!(err.to_string().contains("failed integrity"));

        // A host without a blob store refuses attach with a hint.
        let bare = Handler::new(
            SqliteStore::open_in_memory().unwrap(),
            Sharer::new("bare").unwrap(),
        );
        let refused = bare
            .dispatch(
                "mint",
                &json!({ "target": "ws://x", "attach": image_path.to_str().unwrap() }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(refused.hint.unwrap().contains("blob store"));
    });
    std::fs::remove_dir_all(&dir).ok();
}
