//! `waggle init` — the moment a repo becomes waggle-fluent (doc 17).
//!
//! Installs the agent stub into every harness convention file
//! present (`CLAUDE.md`, `AGENTS.md`, `.cursorrules`), creating the two
//! dominant ones when none exist. Idempotent: the stub lives between
//! markers and re-running replaces the block in place, so `init` is also
//! `update`.

use std::path::{Path, PathBuf};

const BEGIN: &str =
    "<!-- waggle:stub:begin (managed by `waggle init`; edits inside are overwritten) -->";
const END: &str = "<!-- waggle:stub:end -->";

/// The stub — the ENTIRE out-of-band instruction (design doc 17): one
/// installed behavior, everything else taught in-band by the tools.
const STUB: &str = "\
## Artifact handoffs (waggle)
When passing work products between agents or subagents, do not paste file
contents. Call waggle's `mint` with the artifact's path and hand over the
`handoff` line from the result. Consumers call `resolve` with the token.
For SOURCE CODE, mint with `snapshot` (structure is extracted: consumers
get a symbol outline and `read --symbol NAME`), and declare what a
consumer must reach — `--require symbol:NAME` — so `coverage` can prove
the review; judge returned work with `record --stage accepted|rejected`.
When minting a binary artifact (PDF, image, audio), extract its text with
your own abilities first and pass it via `content`.
If unsure what to do with a token, call `map`.";

/// Convention files, in the order harnesses made them famous.
const CONVENTION_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md", ".cursorrules"];

fn block() -> String {
    format!("{BEGIN}\n{STUB}\n{END}\n")
}

/// Install or refresh the stub in one file. Returns what happened.
fn install_into(path: &Path) -> std::io::Result<&'static str> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if let (Some(start), Some(end)) = (existing.find(BEGIN), existing.find(END)) {
        let mut updated = existing.clone();
        updated.replace_range(start..=(end + END.len()), &block());
        if updated == existing {
            return Ok("current");
        }
        std::fs::write(path, updated)?;
        return Ok("updated");
    }
    let mut appended = existing;
    if !appended.is_empty() && !appended.ends_with("\n\n") {
        appended.push_str(if appended.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        });
    }
    appended.push_str(&block());
    std::fs::write(path, appended)?;
    Ok("installed")
}

/// Run `waggle init`. With `file`, target exactly that path; otherwise
/// every convention file present in the current directory — creating
/// `AGENTS.md` and `CLAUDE.md` when none exist at all.
pub fn run(file: Option<&str>) -> i32 {
    let targets: Vec<PathBuf> = if let Some(f) = file {
        vec![PathBuf::from(f)]
    } else {
        let present: Vec<PathBuf> = CONVENTION_FILES
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        if present.is_empty() {
            vec![PathBuf::from("AGENTS.md"), PathBuf::from("CLAUDE.md")]
        } else {
            present
        }
    };

    let mut results = serde_json::Map::new();
    for target in &targets {
        match install_into(target) {
            Ok(what) => {
                results.insert(target.display().to_string(), serde_json::json!(what));
            }
            Err(e) => {
                eprintln!("waggle init: {}: {e}", target.display());
                return 1;
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "result": results,
            "next": [
                { "tool": "map", "args": {},
                  "why": "orient: the tools teach everything past this stub" }
            ],
            "hint": "pair with: claude mcp add waggle -- waggle serve --stdio",
        })
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("waggle-init-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn appends_to_existing_and_is_idempotent() {
        let dir = tmp("append");
        let f = dir.join("CLAUDE.md");
        std::fs::write(&f, "# My project\n\nExisting instructions.\n").unwrap();

        assert_eq!(install_into(&f).unwrap(), "installed");
        let after = std::fs::read_to_string(&f).unwrap();
        assert!(
            after.starts_with("# My project"),
            "existing content preserved"
        );
        assert!(after.contains("Artifact handoffs"));

        // Re-run: byte-identical, reported as current.
        assert_eq!(install_into(&f).unwrap(), "current");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), after);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn updates_a_stale_stub_in_place() {
        let dir = tmp("update");
        let f = dir.join("AGENTS.md");
        std::fs::write(
            &f,
            format!("intro\n\n{BEGIN}\nOLD STUB TEXT\n{END}\n\ntrailing docs\n"),
        )
        .unwrap();

        assert_eq!(install_into(&f).unwrap(), "updated");
        let after = std::fs::read_to_string(&f).unwrap();
        assert!(!after.contains("OLD STUB TEXT"), "stale block replaced");
        assert!(after.contains("Artifact handoffs"));
        assert!(
            after.starts_with("intro"),
            "content before the block preserved"
        );
        assert!(
            after.trim_end().ends_with("trailing docs"),
            "content after preserved"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
