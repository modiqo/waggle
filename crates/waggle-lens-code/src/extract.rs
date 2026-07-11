//! Extraction (doc `20 §5.3`): one parse, one tags-query pass, an arena
//! out — the tree drops before this module returns. Pure CPU: no clock,
//! no I/O, no globals mutated beyond the once-per-process query cache.
//!
//! The `tags.scm` convention this consumes is the standard one shipped by
//! the official grammars: a `@name` capture (the identifier) inside a
//! pattern whose whole node carries `@definition.<kind>`. Reference
//! captures (`@reference.*`) are skipped — the outline is definitions
//! only (doc `20 §9`). Standard text predicates (`#eq?`, `#match?`, …)
//! are evaluated by the tree-sitter binding itself via the text provider.

use std::cell::RefCell;
use std::sync::OnceLock;

use streaming_iterator::StreamingIterator as _;
use tree_sitter::{Language, Parser, Query, QueryCursor};

use crate::outline::{Sym, SymbolOutline, ROOT};
use crate::Lang;

fn grammar(lang: Lang) -> Language {
    match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
        Lang::Python => tree_sitter_python::LANGUAGE.into(),
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Lang::Go => tree_sitter_go::LANGUAGE.into(),
    }
}

fn tags_source(lang: Lang) -> String {
    match lang {
        Lang::Rust => tree_sitter_rust::TAGS_QUERY.to_owned(),
        Lang::Python => tree_sitter_python::TAGS_QUERY.to_owned(),
        // The TypeScript tags query covers only TS-specific constructs
        // (interfaces, signatures, abstract classes); plain classes and
        // functions come from the JavaScript query it inherits — the TS
        // grammar is a superset, so both compile against it.
        Lang::TypeScript | Lang::Tsx => format!(
            "{}\n{}",
            tree_sitter_javascript::TAGS_QUERY,
            tree_sitter_typescript::TAGS_QUERY
        ),
        Lang::JavaScript => tree_sitter_javascript::TAGS_QUERY.to_owned(),
        Lang::Go => tree_sitter_go::TAGS_QUERY.to_owned(),
    }
}

/// Compiled queries, once per process per language (doc `20 §4`): query
/// compilation is the expensive setup; sharing it forever is the point.
fn query(lang: Lang) -> Option<&'static Query> {
    static QUERIES: [OnceLock<Option<Query>>; 6] = [const { OnceLock::new() }; 6];
    let slot = match lang {
        Lang::Rust => 0,
        Lang::Python => 1,
        Lang::TypeScript => 2,
        Lang::Tsx => 3,
        Lang::JavaScript => 4,
        Lang::Go => 5,
    };
    QUERIES[slot]
        .get_or_init(|| Query::new(&grammar(lang), &tags_source(lang)).ok())
        .as_ref()
}

thread_local! {
    /// One parser per thread, `set_language` per call (doc `20 §4`).
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Extract the symbol outline from source text. Pure: same inputs, same
/// outline, always. Returns an empty outline (callers store no blob) when
/// the text does not parse at all or yields no definitions — degradation
/// is to today's text loop, never below it.
#[must_use]
pub fn extract(text: &str, lang: Lang) -> SymbolOutline {
    let Some(q) = query(lang) else {
        return SymbolOutline::default();
    };
    PARSER.with(|p| {
        let mut parser = p.borrow_mut();
        if parser.set_language(&grammar(lang)).is_err() {
            return SymbolOutline::default();
        }
        let Some(tree) = parser.parse(text, None) else {
            return SymbolOutline::default();
        };

        // Capture indices for the @name / @definition.* convention.
        let names = q.capture_names();
        let mut raw: Vec<(usize, usize, u32, u32, &str, &str)> = Vec::new();
        //           (start_byte, end_byte, start_line, end_line, name, kind)
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(q, tree.root_node(), text.as_bytes());
        while let Some(m) = matches.next() {
            let mut def = None;
            let mut ident = None;
            for cap in m.captures {
                let cap_name = names[cap.index as usize];
                if let Some(kind) = cap_name.strip_prefix("definition.") {
                    def = Some((cap.node, kind));
                } else if cap_name == "name" {
                    ident = Some(cap.node);
                }
            }
            let (Some((node, kind)), Some(ident)) = (def, ident) else {
                continue; // reference.* matches and doc captures
            };
            let Ok(name) = ident.utf8_text(text.as_bytes()) else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation)] // files are <16 MB by the read cap
            raw.push((
                node.start_byte(),
                node.end_byte(),
                node.start_position().row as u32 + 1,
                node.end_position().row as u32 + 1,
                name,
                kind,
            ));
        }
        build_arena(raw)
        // `tree` drops HERE — never retained (20 §4).
    })
}

/// Assemble the flat arena: document order, parent links by containment
/// (a stack of enclosing definitions), kinds deduplicated into a legend.
fn build_arena(mut raw: Vec<(usize, usize, u32, u32, &str, &str)>) -> SymbolOutline {
    // Document order; outermost first at equal starts so parents precede
    // children on the containment stack.
    raw.sort_by_key(|&(start, end, ..)| (start, usize::MAX - end));
    raw.dedup_by_key(|&mut (start, end, _, _, name, _)| (start, end, name.to_owned()));

    let mut out = SymbolOutline::default();
    let mut stack: Vec<(usize, usize, u32)> = Vec::new(); // (start, end, arena idx)
    for (start, end, start_line, end_line, name, kind) in raw {
        while stack
            .last()
            .is_some_and(|&(s, e, _)| !(s <= start && end <= e))
        {
            stack.pop();
        }
        let parent = stack.last().map_or(ROOT, |&(_, _, i)| i);
        let depth = u8::try_from(stack.len().min(u8::MAX as usize)).unwrap_or(u8::MAX);
        let kind_id = out.kinds.iter().position(|k| k == kind).unwrap_or_else(|| {
            out.kinds.push(kind.to_owned());
            out.kinds.len() - 1
        });
        let name_off = u32::try_from(out.names.len()).unwrap_or(u32::MAX);
        let name_len = u16::try_from(name.len().min(u16::MAX as usize)).unwrap_or(u16::MAX);
        out.names.push_str(&name[..name_len as usize]);
        let idx = u32::try_from(out.syms.len()).unwrap_or(u32::MAX);
        out.syms.push(Sym {
            name: (name_off, name_len),
            kind: u8::try_from(kind_id.min(u8::MAX as usize)).unwrap_or(u8::MAX),
            start_line,
            end_line,
            parent,
            depth,
        });
        stack.push((start, end, idx));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_SRC: &str = r"
/// A thing.
pub struct Contract {
    regions: Vec<u32>,
}

impl Contract {
    /// Judge the bits.
    pub fn evaluate(&self, bits: u8) -> bool {
        bits != 0
    }
}

fn helper() -> u8 {
    7
}
";

    #[test]
    fn rust_definitions_with_nesting_and_ranges() {
        let o = extract(RUST_SRC, Lang::Rust);
        assert!(!o.is_empty());
        let eval = o.find("evaluate");
        assert_eq!(eval.len(), 1, "outline: {o:?}");
        let (start, end, kind) = o.lines_of(eval[0]).unwrap();
        assert!(start >= 8 && end > start, "def extent covers the body");
        // The rust tags query marks methods distinctly from functions
        // (impl blocks themselves are references, not definitions — so
        // rust nesting is kind-based, not depth-based).
        assert_eq!(kind, "method");
        let helper = o.find("helper")[0];
        assert_eq!(o.lines_of(helper).unwrap().2, "function");
        assert!(!o.find("Contract").is_empty());
    }

    #[test]
    fn python_nesting_is_containment_based() {
        let py = extract(
            "class A:\n    def m(self):\n        pass\n\ndef f():\n    pass\n",
            Lang::Python,
        );
        let (m, f) = (py.find("m")[0], py.find("f")[0]);
        assert!(py.syms[m].depth > py.syms[f].depth, "{py:?}");
        assert_eq!(py.syms[m].parent, 0, "m's parent is class A in the arena");
    }

    #[test]
    fn python_and_typescript_and_go_extract() {
        let py = extract(
            "class A:\n    def m(self):\n        pass\n\ndef f():\n    pass\n",
            Lang::Python,
        );
        assert!(py.find("m").len() == 1 && py.find("f").len() == 1, "{py:?}");

        let ts = extract(
            "export class Store {\n  get(k: string): string { return k; }\n}\nfunction main() {}\n",
            Lang::TypeScript,
        );
        assert!(!ts.find("main").is_empty(), "{ts:?}");

        let go = extract(
            "package p\n\nfunc Handle(x int) int {\n\treturn x\n}\n",
            Lang::Go,
        );
        assert!(!go.find("Handle").is_empty(), "{go:?}");
    }

    #[test]
    fn broken_code_still_yields_what_parses() {
        // Error tolerance is the reason tree-sitter was chosen: mid-edit
        // code keeps its outline for everything before the breakage.
        let o = extract("fn good() {}\n\nfn broken( {{{\n", Lang::Rust);
        assert!(!o.find("good").is_empty());
    }

    #[test]
    fn extraction_is_deterministic() {
        let a = extract(RUST_SRC, Lang::Rust).to_wire();
        let b = extract(RUST_SRC, Lang::Rust).to_wire();
        assert_eq!(a, b, "same bytes, same outline — CAS dedupes for free");
    }
}
