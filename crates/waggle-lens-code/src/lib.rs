//! # waggle-lens-code — the symbol lens (design doc `20`)
//!
//! Source-code structure, computed **once at mint** where the artifact is
//! at hand, stored content-addressed beside the snapshot, served
//! everywhere as plain data. This crate is the extraction half:
//! tree-sitter parses the snapshot, each grammar's `tags.scm` query marks
//! the definitions, and the result is a flat, compact [`SymbolOutline`] —
//! never an AST, never an index, never retained.
//!
//! The performance discipline (doc `20 §4`) is structural here:
//! compiled queries are cached once per process ([`std::sync::OnceLock`]),
//! parsers are reused per thread, the outline is an arena (one `Vec`, one
//! shared name buffer, parent links by index), and the parse tree drops
//! before extraction returns. `extract` is a pure function of its inputs —
//! no clock, no I/O — so the sans-I/O law holds even though this crate
//! lives outside `waggle-core` (it must: grammars are C, and this crate is
//! **never compiled to wasm**; the edge serves precomputed outlines).

mod extract;
mod outline;

pub use extract::extract;
pub use outline::{SymbolOutline, WIRE_VERSION};

/// The languages the v1 lens ships (doc `20 §9`): chosen by agent
/// traffic; each addition pays its binary-size cost consciously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Rust (`.rs`).
    Rust,
    /// Python (`.py`, `.pyi`).
    Python,
    /// TypeScript (`.ts`, `.mts`, `.cts`).
    TypeScript,
    /// TSX (`.tsx`).
    Tsx,
    /// JavaScript (`.js`, `.mjs`, `.cjs`, `.jsx`).
    JavaScript,
    /// Go (`.go`).
    Go,
}

/// Detect a supported language from a path — extension first, then the
/// well-known extension-less basenames have no grammar here (they are
/// text-sniffed upstream); a `None` simply means "no outline", never an
/// error: the lens degrades to the plain text loop (doc `20 §9`).
#[must_use]
pub fn detect(path: &str) -> Option<Lang> {
    let basename = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = basename.rsplit_once('.').map(|(_, e)| e)?;
    match ext.to_ascii_lowercase().as_str() {
        "rs" => Some(Lang::Rust),
        "py" | "pyi" => Some(Lang::Python),
        "ts" | "mts" | "cts" => Some(Lang::TypeScript),
        "tsx" => Some(Lang::Tsx),
        "js" | "mjs" | "cjs" | "jsx" => Some(Lang::JavaScript),
        "go" => Some(Lang::Go),
        _ => None,
    }
}

/// The outline blob's content type (doc `20 §3`).
pub const OUTLINE_CONTENT_TYPE: &str = "application/waggle-outline+json";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_is_extension_driven_and_total() {
        assert_eq!(detect("crates/waggle-core/src/mint.rs"), Some(Lang::Rust));
        assert_eq!(detect("a/b/app.test.TSX"), Some(Lang::Tsx));
        assert_eq!(detect("script.mjs"), Some(Lang::JavaScript));
        assert_eq!(detect("cmd/main.go"), Some(Lang::Go));
        assert_eq!(detect("README.md"), None, "no grammar is not an error");
        assert_eq!(detect("Makefile"), None, "extension-less: text loop only");
        assert_eq!(detect(".hidden"), None);
    }
}
