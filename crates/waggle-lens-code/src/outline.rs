//! The symbol outline: a flat, document-ordered arena (doc `20 §5.2`) and
//! its struct-of-arrays wire form. ~tens of bytes per symbol, one shared
//! name buffer, parent links by index — nesting without pointers, binary
//! search by line, prefix-sliceable rendering under a byte budget.

use serde::{Deserialize, Serialize};

/// Wire version — pins the `(bytes, extractor) → outline` function so a
/// future extractor bump mints new outlines without ambiguity (doc 20 §3).
pub const WIRE_VERSION: u16 = 1;

/// Sentinel parent index for top-level symbols.
pub(crate) const ROOT: u32 = u32::MAX;

/// One definition: name by (offset, len) into the shared buffer, kind by
/// index into the legend, 1-based inclusive lines (the same shape contract
/// `Region`s use), parent by arena index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sym {
    pub name: (u32, u16),
    pub kind: u8,
    pub start_line: u32,
    pub end_line: u32,
    pub parent: u32,
    pub depth: u8,
}

/// The arena. Construction happens in [`crate::extract`]; consumers get
/// lookups and the wire form.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SymbolOutline {
    pub(crate) syms: Vec<Sym>,
    pub(crate) names: String,
    /// Kind legend, indexed by `Sym::kind` — the `tags.scm` capture
    /// suffixes ("function", "method", "class", …), deduplicated.
    pub(crate) kinds: Vec<String>,
}

impl SymbolOutline {
    /// Number of definitions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.syms.len()
    }

    /// True when nothing was extracted (callers store no blob then).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.syms.is_empty()
    }

    pub(crate) fn name_of(&self, s: &Sym) -> &str {
        let (off, len) = s.name;
        &self.names[off as usize..off as usize + len as usize]
    }

    /// All definitions with this exact name, as arena indices in
    /// document order — `symbol:` contract resolution walks this.
    #[must_use]
    pub fn find(&self, name: &str) -> Vec<usize> {
        self.syms
            .iter()
            .enumerate()
            .filter(|(_, s)| self.name_of(s) == name)
            .map(|(i, _)| i)
            .collect()
    }

    /// The name at an arena index.
    #[must_use]
    pub fn name_at(&self, idx: usize) -> Option<&str> {
        self.syms.get(idx).map(|s| self.name_of(s))
    }

    /// The `(start_line, end_line, kind)` of an arena index.
    #[must_use]
    pub fn lines_of(&self, idx: usize) -> Option<(u32, u32, &str)> {
        let s = self.syms.get(idx)?;
        Some((
            s.start_line,
            s.end_line,
            self.kinds.get(s.kind as usize).map_or("", String::as_str),
        ))
    }

    /// Serialize to the wire form (struct-of-arrays JSON).
    ///
    /// # Panics
    /// Never in practice: the wire is plain data with no fallible
    /// serialization path; the `expect` documents the invariant.
    #[must_use]
    pub fn to_wire(&self) -> Vec<u8> {
        let w = Wire {
            x: WIRE_VERSION,
            kinds: self.kinds.clone(),
            names: self
                .syms
                .iter()
                .map(|s| self.name_of(s).to_owned())
                .collect(),
            kind: self.syms.iter().map(|s| s.kind).collect(),
            start: self.syms.iter().map(|s| s.start_line).collect(),
            end: self.syms.iter().map(|s| s.end_line).collect(),
            depth: self.syms.iter().map(|s| s.depth).collect(),
        };
        serde_json::to_vec(&w).expect("plain data always serializes")
    }
}

/// The wire shape: parallel arrays so the serve side (which lives in
/// `waggle-mcp`, wasm-safe — this crate never reaches the edge) can
/// slice prefixes without touching entries it will drop (doc `20 §4`).
/// Parent links are not carried — depth suffices for rendering, and
/// contracts resolve to line ranges at mint.
#[derive(Serialize, Deserialize)]
struct Wire {
    x: u16,
    kinds: Vec<String>,
    names: Vec<String>,
    kind: Vec<u8>,
    start: Vec<u32>,
    end: Vec<u32>,
    depth: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline(n: usize) -> SymbolOutline {
        let mut o = SymbolOutline {
            kinds: vec!["function".into(), "class".into()],
            ..SymbolOutline::default()
        };
        for i in 0..n {
            let name = format!("sym_{i}");
            let off = u32::try_from(o.names.len()).unwrap();
            o.names.push_str(&name);
            o.syms.push(Sym {
                name: (off, u16::try_from(name.len()).unwrap()),
                kind: u8::try_from(i % 2).unwrap(),
                start_line: u32::try_from(i * 10 + 1).unwrap(),
                end_line: u32::try_from(i * 10 + 8).unwrap(),
                parent: ROOT,
                depth: u8::try_from(i % 3).unwrap(),
            });
        }
        o
    }

    #[test]
    fn wire_is_parallel_arrays_with_the_version_pin() {
        let o = outline(7);
        let v: serde_json::Value = serde_json::from_slice(&o.to_wire()).unwrap();
        assert_eq!(v["x"], WIRE_VERSION);
        for field in ["names", "kind", "start", "end", "depth"] {
            assert_eq!(v[field].as_array().unwrap().len(), 7, "{field}");
        }
        assert_eq!(v["names"][3], "sym_3");
        assert_eq!(v["start"][3], 31);
        assert_eq!(v["kinds"][0], "function");
    }

    #[test]
    fn arena_lookups() {
        let o = outline(7);
        assert_eq!(o.find("sym_3"), vec![3]);
        let (start, end, kind) = o.lines_of(3).unwrap();
        assert_eq!((start, end), (31, 38));
        assert_eq!(kind, "class");
        assert!(o.find("nope").is_empty());
    }
}
