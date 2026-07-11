//! The serve side of the symbol lens (doc `20 §5.5`): consume an
//! outline blob (`application/waggle-outline+json`, parallel arrays,
//! produced by `waggle-lens-code` at mint) and render or resolve it —
//! **pure data work**, always compiled, wasm-safe. No tree-sitter here;
//! the edge serves outlines without ever parsing source.

use serde_json::{json, Value};

/// The wire version this reader understands (pinned against
/// `waggle_lens_code::WIRE_VERSION` by the fixture test below).
const WIRE_VERSION: u64 = 1;

/// One symbol row, borrowed out of the parsed wire arrays.
struct Row<'a> {
    name: &'a str,
    kind: &'a str,
    start: u64,
    end: u64,
    depth: u64,
}

/// Parse the parallel arrays into rows. `None` on version mismatch or a
/// malformed blob — an unreadable outline is *absent*, never an error.
fn rows(wire: &Value) -> Option<Vec<Row<'_>>> {
    if wire["x"].as_u64()? != WIRE_VERSION {
        return None;
    }
    let names = wire["names"].as_array()?;
    let kinds_legend = wire["kinds"].as_array()?;
    let (kind, start, end, depth) = (
        wire["kind"].as_array()?,
        wire["start"].as_array()?,
        wire["end"].as_array()?,
        wire["depth"].as_array()?,
    );
    let mut out = Vec::with_capacity(names.len());
    for i in 0..names.len() {
        let kind_idx = usize::try_from(kind.get(i)?.as_u64()?).ok()?;
        out.push(Row {
            name: names.get(i)?.as_str()?,
            kind: kinds_legend.get(kind_idx).and_then(Value::as_str)?,
            start: start.get(i)?.as_u64()?,
            end: end.get(i)?.as_u64()?,
            depth: depth.get(i)?.as_u64()?,
        });
    }
    Some(out)
}

/// Budget-fitted symbols overview: shallow structure first, prefix sums
/// once, partition point for the fit, truncation NAMED (doc `20 §5.5`).
pub(crate) fn render(blob: &[u8], max_bytes: usize) -> Option<Value> {
    let wire: Value = serde_json::from_slice(blob).ok()?;
    let rows = rows(&wire)?;
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| (rows[i].depth, rows[i].start));

    let entry = |i: usize| {
        let r = &rows[i];
        json!({
            "name": r.name,
            "kind": r.kind,
            "lines": format!("{}-{}", r.start, r.end),
            "depth": r.depth,
        })
    };
    let mut cum = 0usize;
    let prefix: Vec<usize> = order
        .iter()
        .map(|&i| {
            cum += serde_json::to_vec(&entry(i)).map_or(0, |v| v.len() + 1);
            cum
        })
        .collect();
    let fit = prefix.partition_point(|&size| size <= max_bytes);
    let mut shown = order[..fit].to_vec();
    shown.sort_by_key(|&i| rows[i].start); // present in document order
    Some(json!({
        "symbols": shown.into_iter().map(entry).collect::<Vec<_>>(),
        "total_symbols": rows.len(),
        "omitted": rows.len() - fit,
    }))
}

/// What resolving a symbol name against an outline concluded.
pub(crate) enum SymbolHit {
    /// Exactly one definition: its 1-based inclusive line range.
    Found(u64, u64),
    /// Several definitions share the name — each shown as `kind@lines`.
    Ambiguous(Vec<String>),
    /// No such symbol; carries a sample of what exists (misses teach).
    Missing(Vec<String>),
}

/// Resolve `read --symbol NAME` against an outline blob.
pub(crate) fn find_symbol(blob: &[u8], name: &str) -> SymbolHit {
    let Some(wire) = serde_json::from_slice::<Value>(blob).ok() else {
        return SymbolHit::Missing(vec![]);
    };
    let Some(rows) = rows(&wire) else {
        return SymbolHit::Missing(vec![]);
    };
    let hits: Vec<&Row> = rows.iter().filter(|r| r.name == name).collect();
    match hits.as_slice() {
        [] => SymbolHit::Missing(rows.iter().take(12).map(|r| r.name.to_owned()).collect()),
        [one] => SymbolHit::Found(one.start, one.end),
        many => SymbolHit::Ambiguous(
            many.iter()
                .map(|r| format!("{} @ {}-{}", r.kind, r.start, r.end))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cross-crate pin: a blob PRODUCED by the extractor must be
    /// consumable here — the two halves of doc 20 §5.2's wire contract.
    #[test]
    fn consumes_what_the_extractor_produces() {
        let src = "pub struct A;\n\nimpl A {\n    pub fn m(&self) {}\n}\n\nfn f() {}\n";
        let outline = waggle_lens_code::extract(src, waggle_lens_code::Lang::Rust);
        let blob = outline.to_wire();

        let rendered = render(&blob, 64 * 1024).expect("wire parses");
        assert_eq!(rendered["omitted"], 0);
        let symbols = rendered["symbols"].as_array().unwrap();
        assert!(symbols.iter().any(|s| s["name"] == "m"), "{symbols:?}");

        match find_symbol(&blob, "f") {
            SymbolHit::Found(start, end) => assert!(start >= 6 && end >= start),
            _ => panic!("f is unique"),
        }
        assert!(
            matches!(find_symbol(&blob, "nope"), SymbolHit::Missing(names) if !names.is_empty())
        );
    }

    #[test]
    fn budget_truncation_is_named_and_future_versions_read_as_absent() {
        let mk = |n: usize| {
            json!({
                "x": 1,
                "kinds": ["function"],
                "names": (0..n).map(|i| format!("sym_{i}")).collect::<Vec<_>>(),
                "kind": vec![0; n],
                "start": (0..n).map(|i| i * 10 + 1).collect::<Vec<_>>(),
                "end": (0..n).map(|i| i * 10 + 5).collect::<Vec<_>>(),
                "depth": vec![0; n],
            })
        };
        let blob = serde_json::to_vec(&mk(80)).unwrap();
        let small = render(&blob, 512).unwrap();
        let shown = small["symbols"].as_array().unwrap().len();
        assert!(shown > 0 && shown < 80);
        assert_eq!(small["omitted"], 80 - shown);

        let mut future = mk(3);
        future["x"] = json!(999);
        assert!(render(&serde_json::to_vec(&future).unwrap(), 512).is_none());
    }
}
