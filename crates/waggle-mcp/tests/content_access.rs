//! CP-7.5 gates (design doc `18 §7`): surgical content access end to end
//! over the dispatcher and the real `SQLite` store — the grep→open loop,
//! snapshot immortality, binary refusal, and `read` in the funnel.

use serde_json::json;
use waggle_core::{Sharer, Timestamp};
use waggle_mcp::{validate_next, Handler, NextCall};
use waggle_store_sqlite::{BlobStore, SqliteStore};

fn entropy() -> impl FnMut(&mut [u8]) -> Result<(), waggle_core::EntropyError> {
    let mut state = 0xC0DE_u32;
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

const REPORT: &str = "\
# Market Report

## Methodology
we swept 40 sources
and verified claims

## Competitor Pricing
pricing clusters at $49-79/mo
enterprise pricing is bespoke

## Risks
open-source pricing pressure
";

fn handler_with_blobs(dir: &std::path::Path) -> Handler<SqliteStore, BlobStore> {
    Handler::new(
        SqliteStore::open_in_memory().unwrap(),
        Sharer::new("lead").unwrap(),
    )
    .with_blobs(BlobStore::open(&dir.join("blobs")).unwrap())
}

fn check_next(env: &waggle_mcp::Envelope) {
    for n in &env.next {
        validate_next(&NextCall {
            tool: n.tool.clone(),
            args: n.args.clone(),
            why: String::new(),
        })
        .expect("envelope_next_valid");
    }
}

#[test]
fn grep_open_loop_on_a_live_file() {
    let dir = std::env::temp_dir().join(format!("waggle-content-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("report.md");
    std::fs::write(&file, REPORT).unwrap();

    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", file.display()) }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // The overview: lenses + outline, no address needed.
        let over = handler
            .dispatch(
                "read",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(over.hint.is_none(), "{over:?}");
        check_next(&over);
        assert_eq!(over.result["content_type"], "text/markdown");
        let lenses = over.result["lenses"].as_array().unwrap();
        assert!(lenses.iter().any(|l| l == "section"));
        assert!(over.result["outline"].as_array().unwrap().len() >= 4);

        // grep → the matches carry line numbers and honest totals.
        let found = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "(?i)pricing", "max-matches": 2 }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(found.hint.is_none());
        check_next(&found);
        assert_eq!(found.result["total_matches"], 4);
        assert_eq!(found.result["returned"], 2);
        assert_eq!(found.result["truncated"], true);
        assert_eq!(found.next[0].tool, "read", "grep chains into open");

        // …open the neighborhood the guidance points at.
        let window = handler
            .dispatch(
                "read",
                &found.next[0].args.clone(),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(window.result["text"].as_str().unwrap().contains("$49-79"));

        // The section lens.
        let section = handler
            .dispatch(
                "read",
                &json!({ "token": token, "section": "competitor pricing" }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(section.result["text"].as_str().unwrap().contains("bespoke"));

        // Attribution: reads and searches recorded as counts (I-1).
        let funnel = handler
            .dispatch(
                "funnel",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert_eq!(
            funnel.result["stages"]["read"], 4,
            "overview+search+window+section"
        );
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn snapshot_immortality_content_outlives_the_file() {
    let dir = std::env::temp_dir().join(format!("waggle-snap-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("findings.md");
    std::fs::write(&file, REPORT).unwrap();

    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", file.display()), "snapshot": true }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // Delete the source file. The token's content must survive.
        std::fs::remove_file(&file).unwrap();

        let found = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "bespoke" }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(
            found.hint.is_none(),
            "snapshot outlives the file: {found:?}"
        );
        assert_eq!(found.result["total_matches"], 1);

        // And mutating the file wouldn't matter either: what you grep is
        // what was minted — immutable by hash.
        std::fs::write(&file, "totally different now").unwrap();
        let again = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "bespoke" }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(
            again.result["total_matches"], 1,
            "snapshot, not the live file"
        );
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn json_pointer_lens_and_refusals() {
    let dir = std::env::temp_dir().join(format!("waggle-jsonlens-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let lock = dir.join("deps.json");
    std::fs::write(
        &lock,
        r#"{"dependencies":{"react":{"version":"18.3.1"},"vite":{"version":"6.0.1"}}}"#,
    )
    .unwrap();

    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", lock.display()) }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // The pointer lens: one value out of the lockfile, ~40 bytes.
        let version = handler
            .dispatch(
                "read",
                &json!({ "token": token, "path": "/dependencies/react/version" }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert_eq!(version.result["slice"], "18.3.1");

        // A bad pointer names the valid roots.
        let bad = handler
            .dispatch(
                "read",
                &json!({ "token": token, "path": "/nope" }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(bad.hint.as_ref().unwrap().contains("/dependencies"));

        // Binary content refuses with the extract-at-mint hint.
        let img = dir.join("photo.png");
        std::fs::write(&img, b"\x89PNG...").unwrap();
        let bin_minted = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", img.display()) }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        let bin_token = bin_minted.result["token"].as_str().unwrap().to_owned();
        let refused = handler
            .dispatch(
                "search",
                &json!({ "token": bin_token, "pattern": "x" }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(refused.hint.as_ref().unwrap().contains("binary"));

        // Content that exists nowhere: the hint names the snapshot fix.
        let ghost = handler
            .dispatch(
                "mint",
                &json!({ "target": "ws://remote/unreachable.md" }),
                Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        let ghost_token = ghost.result["token"].as_str().unwrap().to_owned();
        let missing = handler
            .dispatch(
                "read",
                &json!({ "token": ghost_token }),
                Timestamp::from_unix_ms(7),
                &mut e,
            )
            .await;
        assert!(missing.hint.as_ref().unwrap().contains("snapshot"));
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn binary_target_with_extracted_content_the_pdf_story() {
    // Doc 18 §7: the harness extracted the PDF once at mint; the token
    // serves surgical access to the extraction forever, while the target
    // stays the original binary.
    let dir = std::env::temp_dir().join(format!("waggle-extract-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let pdf = dir.join("q3-report.pdf");
    std::fs::write(&pdf, b"%PDF-1.7 ...binary noise...").unwrap();
    let extracted = dir.join("q3-report.extracted.md");
    std::fs::write(&extracted, REPORT).unwrap();

    let handler = handler_with_blobs(&dir);
    let mut e = entropy();
    pollster::block_on(async {
        let minted = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", pdf.display()),
                    "content": extracted.to_str().unwrap(),
                }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let token = minted.result["token"].as_str().unwrap().to_owned();

        // Search hits the EXTRACTION even though the target is binary.
        let found = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "bespoke" }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(found.hint.is_none(), "{found:?}");
        assert_eq!(found.result["total_matches"], 1);

        // The overview reports the extraction's type and lenses.
        let over = handler
            .dispatch(
                "read",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(over.result["content_type"], "text/markdown");

        // snapshot + content together: refused with the distinction named.
        let both = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", pdf.display()),
                    "snapshot": true,
                    "content": extracted.to_str().unwrap(),
                }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(both
            .hint
            .as_ref()
            .unwrap()
            .contains("one of snapshot/content"));

        // Passing a binary as the "extraction": refused with the fix.
        let bad = handler
            .dispatch(
                "mint",
                &json!({
                    "target": format!("file://{}", pdf.display()),
                    "content": pdf.to_str().unwrap(),
                }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(bad.hint.as_ref().unwrap().contains("extracted TEXT"));
    });
    std::fs::remove_dir_all(&dir).ok();
}

/// Folder targets: minting one is legal (it's a locator and a lineage
/// root), the envelope teaches the children pattern up front, and the
/// content verbs refuse with the same lesson — never a raw OS error.
#[test]
fn folder_targets_teach_the_lineage_pattern() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let reports = dir.path().join("reports");
        std::fs::create_dir_all(&reports).unwrap();
        std::fs::write(reports.join("a.md"), "alpha\n").unwrap();
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();

        let minted = handler
            .dispatch(
                "mint",
                &serde_json::json!({ "target": format!("file://{}", reports.display()) }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none());
        let token = minted.result["token"].as_str().unwrap().to_owned();
        // The first next step teaches children-of-this-folder.
        assert_eq!(minted.next[0].tool, "mint", "{:?}", minted.next);
        assert_eq!(minted.next[0].args["parent"], token);

        // read/search refuse with the fix named — not "os error 21".
        for (op, args) in [
            ("read", serde_json::json!({ "token": token })),
            (
                "search",
                serde_json::json!({ "token": token, "pattern": "alpha" }),
            ),
        ] {
            let envelope = handler
                .dispatch(op, &args, waggle_core::Timestamp::from_unix_ms(2), &mut e)
                .await;
            let hint = envelope.hint.expect("refusal expected");
            assert!(
                hint.contains("directory") && hint.contains("parent"),
                "`{op}` hint must teach the lineage pattern: {hint}"
            );
        }
    });
}
