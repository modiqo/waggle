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
    let dir = std::env::temp_dir().join(format!("waggle-html-extract-{}", std::process::id()));
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
    let dir = std::env::temp_dir().join(format!("waggle-pdf-extract-{}", std::process::id()));
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

/// The indexed tree, end to end (design doc: tree-scale): --tree mints a
/// hierarchy of directory NODES; the root projects its table of contents;
/// revoking the root tombstones every subtree node through lineage.
#[test]
fn indexed_tree_projection_and_cascade() {
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
                &serde_json::json!({ "target": format!("file://{}", docs.display()), "tree": true }),
                waggle_core::Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await;
        assert!(minted.hint.is_none(), "{minted:?}");
        let root = minted.result["token"].as_str().unwrap().to_owned();
        assert_eq!(
            minted.result["tree"]["files"], 2,
            "dotfile skipped: {minted:?}"
        );

        // The projection is the directory's table of contents: one local file,
        // one subdirectory (its own node token).
        let toc = handler
            .dispatch(
                "read",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert!(toc.hint.is_none(), "{toc:?}");
        assert_eq!(toc.result["files"], 1, "a.md is a local file");
        assert_eq!(toc.result["subdirs"], 1, "nested is a subtree");
        assert_eq!(toc.result["total_files"], 2);
        let sub = toc.result["dirs"][0]["token"].as_str().unwrap().to_owned();

        // Read one file back by name — its bytes travel with the tree.
        let file = handler
            .dispatch(
                "read",
                &serde_json::json!({ "token": root, "file": "a.md" }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert!(
            file.result["text"].as_str().unwrap().contains("bespoke"),
            "{file:?}"
        );

        // One revocation on the root tombstones every subtree node through lineage.
        let revoked = handler
            .dispatch(
                "mutate",
                &serde_json::json!({ "token": root, "change": "revoke", "expected-version": 1 }),
                waggle_core::Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(revoked.hint.is_none(), "{revoked:?}");
        let resolved = handler
            .dispatch(
                "resolve",
                &serde_json::json!({ "token": sub }),
                waggle_core::Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        assert!(
            resolved.result["disposition"].get("revoked").is_some(),
            "subtree node tombstoned through lineage: {resolved:?}"
        );
    });
}

/// One search over the root spans the whole nested tree, ranked, with paths —
/// and an absent pattern prunes to zero visited nodes.
#[test]
fn indexed_tree_search_spans_the_lineage() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("corpus");
        std::fs::create_dir_all(docs.join("deep")).unwrap();
        std::fs::write(docs.join("plan.md"), "the needle sits here\n").unwrap();
        std::fs::write(
            docs.join("deep/notes.md"),
            "needle needle nested\nno match\n",
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
        assert_eq!(
            found.result["total_matches"], 2,
            "two files match: {found:?}"
        );
        let matches = found.result["matches"].as_array().unwrap();
        // Ranked: the file with more matches (deep/notes.md, x2) comes first.
        assert!(
            matches[0]["path"].as_str().unwrap().contains("notes.md"),
            "{matches:?}"
        );
        assert!(
            matches[0]["path"].as_str().unwrap().contains("deep/"),
            "nested path carried"
        );
        assert_eq!(found.next[0].tool, "read");

        // An absent literal prunes the whole tree from the root Bloom — 0 visited.
        let none = handler
            .dispatch(
                "search",
                &serde_json::json!({ "token": root, "pattern": "wholly_absent_zzz" }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        assert_eq!(none.result["total_matches"], 0);
        assert_eq!(
            none.result["nodes_visited"], 0,
            "pruned at the root: {none:?}"
        );
    });
}

/// Tags at mint on a tree root + find: discovery by what humans remember.
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

        // Find by TAG (the tree root carries it).
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

        // Find by BASENAME; newest-first; executable next.
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

        // A revoked candidate is VISIBLY dead.
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
            "{after:?}"
        );
    });
}

/// Per-file coverage on a tree: a node with two files, only one of which is read,
/// reports `1/3` — not "the folder was touched" — and NAMES the file nobody
/// opened. A search that serves every file then closes it to `3/3`.
#[test]
fn coverage_is_per_file_over_a_tree() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("review_me");
        std::fs::create_dir_all(docs.join("x")).unwrap();
        std::fs::create_dir_all(docs.join("y")).unwrap();
        // Node x holds TWO files; node y holds one. All share "content" so a
        // search matches every file; each has a unique word too.
        std::fs::write(docs.join("x/a.md"), "content alpha\n").unwrap();
        std::fs::write(docs.join("x/c.md"), "content gamma\n").unwrap();
        std::fs::write(docs.join("y/b.md"), "content beta\n").unwrap();
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
        let toc = handler
            .dispatch(
                "read",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        let x = toc.result["dirs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["name"] == "x")
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .to_owned();

        // Read ONE of x's two files. Per-file coverage: 1 of 3 read, incomplete,
        // and the miss is named (c.md in node x, plus y/b.md untouched).
        handler
            .dispatch(
                "read",
                &serde_json::json!({ "token": x, "file": "a.md" }),
                waggle_core::Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        let audit = handler
            .dispatch(
                "coverage",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert!(audit.hint.is_none(), "{audit:?}");
        assert_eq!(audit.result["kind"], "tree");
        assert_eq!(audit.result["total_files"], 3);
        assert_eq!(audit.result["files"], "1/3", "one file read: {audit:?}");
        assert_eq!(audit.result["complete"], false);
        // Node x is partially read: one of its two files is still missing, named.
        let unread = audit.result["unread"].as_array().unwrap();
        let x_gap = unread
            .iter()
            .find(|u| u["token"] == x)
            .expect("x is partially read");
        assert_eq!(x_gap["unread_files"], 1, "c.md still missing: {audit:?}");
        assert!(x_gap["first_missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n == "c.md"));

        // A search across the tree serves every file → 3/3, complete.
        handler
            .dispatch(
                "search",
                &serde_json::json!({ "token": root, "pattern": "content" }),
                waggle_core::Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        let after = handler
            .dispatch(
                "coverage",
                &serde_json::json!({ "token": root }),
                waggle_core::Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert_eq!(after.result["files"], "3/3", "search served all: {after:?}");
        assert_eq!(after.result["complete"], true);
    });
}

/// `mint --tree --require files:all` puts a completeness GATE on the tree: coverage
/// reports `met` (a verdict an orchestrator can refuse on), flipping false→true as
/// the last file is read. A plain `--tree` carries no such verdict.
#[test]
fn files_all_gate_on_a_tree_flips_met_when_the_last_file_is_read() {
    pollster::block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("review_me");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("a.md"), "content alpha\n").unwrap();
        std::fs::write(docs.join("b.md"), "content beta\n").unwrap();
        let handler = handler_with_blobs(dir.path());
        let mut e = entropy();
        let root = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", docs.display()),
                         "tree": true, "require": "files:all" }),
                Timestamp::from_unix_ms(1),
                &mut e,
            )
            .await
            .result["token"]
            .as_str()
            .unwrap()
            .to_owned();

        // Nothing read: the contract is declared and unmet.
        let before = handler
            .dispatch(
                "coverage",
                &json!({ "token": root }),
                Timestamp::from_unix_ms(2),
                &mut e,
            )
            .await;
        assert_eq!(before.result["requires"], "files:all", "{before:?}");
        assert_eq!(
            before.result["met"], false,
            "unread tree cannot be met: {before:?}"
        );

        // Read one file — still short, still unmet.
        handler
            .dispatch(
                "read",
                &json!({ "token": root, "file": "a.md" }),
                Timestamp::from_unix_ms(3),
                &mut e,
            )
            .await;
        let mid = handler
            .dispatch(
                "coverage",
                &json!({ "token": root }),
                Timestamp::from_unix_ms(4),
                &mut e,
            )
            .await;
        assert_eq!(mid.result["met"], false, "one of two read: {mid:?}");

        // Read the second — the gate flips to met.
        handler
            .dispatch(
                "read",
                &json!({ "token": root, "file": "b.md" }),
                Timestamp::from_unix_ms(5),
                &mut e,
            )
            .await;
        let done = handler
            .dispatch(
                "coverage",
                &json!({ "token": root }),
                Timestamp::from_unix_ms(6),
                &mut e,
            )
            .await;
        assert_eq!(done.result["met"], true, "whole tree read: {done:?}");

        // A plain --tree (no contract) reports completeness as a fact, never a verdict.
        let plain = handler
            .dispatch(
                "mint",
                &json!({ "target": format!("file://{}", docs.display()), "tree": true }),
                Timestamp::from_unix_ms(7),
                &mut e,
            )
            .await
            .result["token"]
            .as_str()
            .unwrap()
            .to_owned();
        let cov = handler
            .dispatch(
                "coverage",
                &json!({ "token": plain }),
                Timestamp::from_unix_ms(8),
                &mut e,
            )
            .await;
        assert!(
            cov.result.get("met").is_none(),
            "no contract, no verdict: {cov:?}"
        );
    });
}
