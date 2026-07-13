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

/// Doc 18 §7: an opaque artifact the substrate can read deterministically (here
/// HTML — no feature needed) is extracted at mint, so `read`/`search` work over
/// its text, and the served projection carries the extraction's provenance. This
/// is the difference between a capability claim and a binding one: the token
/// carries the searchable text, not the harness.
#[test]
fn opaque_html_is_extracted_and_searchable_with_provenance() {
    let dir = std::env::temp_dir().join(format!("waggle-extract-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("briefing.html");
    std::fs::write(
        &file,
        "<html><head><style>b{x}</style></head><body>\
         <h1>Ops Briefing</h1><p>The retry budget is 3 attempts.</p>\
         <p>AUDIT CODE: Q2R-9583</p></body></html>",
    )
    .unwrap();

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

        // read serves the extracted text — content_type is text/plain, and the
        // projection declares HOW the text was recovered.
        let over = handler
            .dispatch(
                "read",
                &json!({ "token": token }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(
            over.hint.is_none(),
            "extraction should serve, not refuse: {over:?}"
        );
        assert_eq!(over.result["content_type"], "text/plain");
        assert_eq!(over.result["source"]["extracted_by"], "html-strip");
        assert_eq!(over.result["source"]["deterministic"], true);

        // search finds the needle in the extracted text — the tags are gone.
        let found = handler
            .dispatch(
                "search",
                &json!({ "token": token, "pattern": "AUDIT CODE" }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(found.hint.is_none(), "{found:?}");
        assert_eq!(found.result["total_matches"], 1);
        let hit = found.result["matches"][0]["text"].as_str().unwrap();
        assert!(hit.contains("Q2R-9583"), "got {hit}");
        assert!(
            !hit.contains('<'),
            "extracted text must carry no tags: {hit}"
        );
    });
    std::fs::remove_dir_all(&dir).ok();
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
        let h = refused.hint.as_ref().unwrap();
        assert!(h.contains("image/png"), "hint names the type: {h}");
        assert!(
            h.contains("does not read") && h.contains("your own"),
            "hint tells the consumer to perceive it with its own model: {h}"
        );

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

/// The folder story, end to end: --tree mints the files as snapshot
/// children; the root's funnel ROLLS UP child stages; revoking the root
/// tombstones every child (resolve AND content refuse through lineage).
#[test]
fn folder_tree_rollup_and_cascade() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        std::fs::create_dir_all(docs.join("nested")).unwrap();
        std::fs::write(docs.join("a.md"), "alpha bespoke\n").unwrap();
        std::fs::write(docs.join("nested/b.md"), "beta\n").unwrap();
        std::fs::write(docs.join(".hidden"), "skip me\n").unwrap();
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();

        let minted = handler
            .dispatch(
                "mint",
                &serde_json::json!({
                    "target": format!("file://{}", docs.display()),
                    "tree": true,
                }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let root = minted.result["token"].as_str().unwrap().to_owned();
        let children = minted.result["children"].as_array().unwrap();
        assert_eq!(
            children.len(),
            2,
            "recursive, dotfiles skipped: {children:?}"
        );
        let child = children[0]["token"].as_str().unwrap().to_owned();

        // Resolving the ROOT serves the index — how a folder token
        // works on a machine where the folder never existed.
        let root_resolved = handler
            .dispatch(
                "resolve",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        let index = root_resolved.result["children"].as_array().unwrap();
        assert_eq!(index.len(), 2, "the folder's projection is its index");
        assert!(index[0]["target"].as_str().unwrap().starts_with("file://"));

        // Children are real snapshot tokens: grep works.
        let hit = handler
            .dispatch(
                "search",
                &serde_json::json!({ "token": child, "pattern": "bespoke" }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(hit.hint.is_none());
        assert_eq!(hit.result["total_matches"], 1);

        // Rollup: the root's funnel includes the child's read.
        let funnel = handler
            .dispatch(
                "funnel",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(
            funnel.result["rollup"]["read"], 1,
            "the folder answers for its tree: {funnel:?}"
        );

        // One revocation, whole tree.
        let revoked = handler
            .dispatch(
                "mutate",
                &serde_json::json!({ "token": root, "change": "revoke", "expected-version": 1 }),
                waggle_core::Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(revoked.hint.is_none());
        let resolved = handler
            .dispatch(
                "resolve",
                &serde_json::json!({ "token": child }),
                waggle_core::Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(
            resolved.result["disposition"].get("revoked").is_some(),
            "child tombstoned through lineage: {resolved:?}"
        );
        let read = handler
            .dispatch(
                "read",
                &serde_json::json!({ "token": child }),
                waggle_core::Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert!(
            read.hint.expect("refusal").contains("revoked"),
            "content serves nothing through a revoked lineage"
        );
    });
}

/// Deep search: grep the ROOT token and hit every file in the tree —
/// matches grouped per file, nested files included.
#[test]
fn deep_search_over_the_root_token() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("corpus");
        std::fs::create_dir_all(docs.join("deep")).unwrap();
        std::fs::write(docs.join("plan.md"), "the needle sits here\n").unwrap();
        std::fs::write(
            docs.join("deep/notes.md"),
            "another needle, nested\nno match line\n",
        )
        .unwrap();
        std::fs::write(docs.join("blank.md"), "nothing relevant\n").unwrap();
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();

        let minted = handler
            .dispatch(
                "mint",
                &serde_json::json!({ "target": format!("file://{}", docs.display()), "tree": true }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let root = minted.result["token"].as_str().unwrap().to_owned();

        let found = handler
            .dispatch(
                "search",
                &serde_json::json!({ "token": root, "pattern": "needle" }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(found.hint.is_none(), "{found:?}");
        assert_eq!(found.result["total_matches"], 2, "{found:?}");
        assert_eq!(found.result["tree"]["files_searched"], 3);
        let files = found.result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2, "only matching files listed");
        assert!(
            files
                .iter()
                .any(|f| f["target"].as_str().unwrap().contains("notes.md")), // sep-agnostic (Windows)
            "nested files are in the tree: {files:?}"
        );
        // The grep→open chain points into the first matching file.
        assert_eq!(found.next[0].tool, "read");
    });
}

/// Tags at mint + find: discovery by what humans remember — ranked
/// candidates with disposition, never name-as-identity.
#[test]
fn tags_and_find_discovery() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();

        std::fs::create_dir_all(dir.path().join("design_docs")).unwrap();
        std::fs::write(dir.path().join("design_docs/a.md"), "arch\n").unwrap();
        std::fs::write(dir.path().join("plan.md"), "plan\n").unwrap();

        let folder = handler
            .dispatch(
                "mint",
                &serde_json::json!({
                    "target": format!("file://{}/design_docs", dir.path().display()),
                    "tree": true,
                    "tag": ["design_docs", "kind=reference"],
                }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(folder.hint.is_none(), "{folder:?}");
        let plan = handler
            .dispatch(
                "mint",
                &serde_json::json!({
                    "target": format!("file://{}/plan.md", dir.path().display()),
                    "snapshot": true,
                }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        let plan_token = plan.result["token"].as_str().unwrap().to_owned();

        // Find by TAG (bare tag became name=design_docs).
        let by_tag = handler
            .dispatch(
                "find",
                &serde_json::json!({ "query": "design_docs" }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(by_tag.result["total"].as_u64().unwrap() >= 1, "{by_tag:?}");
        assert_eq!(
            by_tag.result["candidates"][0]["tags"]["name"],
            "design_docs"
        );

        // Find by BASENAME; newest-first ranking; executable next.
        let by_name = handler
            .dispatch(
                "find",
                &serde_json::json!({ "query": "plan.md" }),
                waggle_core::Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert_eq!(by_name.result["candidates"][0]["token"], plan_token);
        assert_eq!(by_name.next[0].tool, "resolve");

        // Disposition is visible: a revoked candidate says so.
        handler
            .dispatch(
                "mutate",
                &serde_json::json!({ "token": plan_token, "change": "revoke", "expected-version": 1 }),
                waggle_core::Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        let after = handler
            .dispatch(
                "find",
                &serde_json::json!({ "query": "plan.md" }),
                waggle_core::Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert_eq!(
            after.result["candidates"][0]["disposition"], "revoked",
            "a dead name is VISIBLY dead: {after:?}"
        );
    });
}

/// The folder-review proof: coverage on a tree names exactly which
/// files a review MISSED — and a root deep-search honestly counts as
/// reading every file it scanned.
#[test]
fn coverage_proves_the_folder_was_read_or_names_the_gaps() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("review_me");
        std::fs::create_dir_all(&docs).unwrap();
        for name in ["a.md", "b.md", "c.md"] {
            std::fs::write(docs.join(name), format!("content of {name}\n")).unwrap();
        }
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();
        let minted = handler
            .dispatch(
                "mint",
                &serde_json::json!({ "target": format!("file://{}", docs.display()), "tree": true }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        let root = minted.result["token"].as_str().unwrap().to_owned();
        let children = minted.result["children"].as_array().unwrap().clone();

        // A lazy review: reads a.md and b.md, never opens c.md.
        for child in children.iter().take(2) {
            let token = child["token"].as_str().unwrap();
            handler
                .dispatch(
                    "read",
                    &serde_json::json!({ "token": token }),
                    waggle_core::Timestamp::from_unix_ms(2),
                    &mut e,
                )
                .await;
        }
        let audit = handler
            .dispatch(
                "coverage",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(audit.hint.is_none(), "{audit:?}");
        assert_eq!(audit.result["read"], "2/3", "{audit:?}");
        assert_eq!(audit.result["complete"], false);
        assert!(
            audit.result["unread"][0]["target"]
                .as_str()
                .unwrap()
                .ends_with("c.md"),
            "the miss is NAMED: {audit:?}"
        );
        assert_eq!(audit.next[0].tool, "read", "next closes the gap");

        // A root deep-search touches every file — honest per-child reads.
        handler
            .dispatch(
                "search",
                &serde_json::json!({ "token": root, "pattern": "content" }),
                waggle_core::Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        let after = handler
            .dispatch(
                "coverage",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert_eq!(after.result["read"], "3/3", "{after:?}");
        assert_eq!(after.result["complete"], true);

        // The STRONG bar: run stays honest — nothing recorded use yet.
        assert_eq!(after.result["run"], "0/3");
        let child0 = children[0]["token"].as_str().unwrap();
        handler
            .dispatch(
                "record",
                &serde_json::json!({ "token": child0, "stage": "run" }),
                waggle_core::Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        let strong = handler
            .dispatch(
                "coverage",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(7),
                &mut e,
            )
            .await;
        assert_eq!(strong.result["run"], "1/3");
    });
}
