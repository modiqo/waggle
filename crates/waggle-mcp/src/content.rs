//! The lens engine (design doc `18`): surgical access to a token's
//! **content** — line windows, regex search, markdown sections, JSON
//! pointers — under the same discipline as `query`: hard byte budgets,
//! responses that name the bytes they spared you, guidance deeper.
//!
//! Pure functions over `(text, content_type, request)` — no I/O here;
//! the handler fetches bytes (snapshot blob or local target) and records
//! the `read` stage. Which lenses apply is decided by content type,
//! discoverable via the overview, never guessed.

use serde_json::{json, Value};

/// Content types the lens engine treats as text (doc `18 §3`).
#[must_use]
pub fn is_text(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || matches!(
            content_type,
            "application/json" | "application/yaml" | "application/x-yaml"
        )
}

/// The sniff fallback (doc `20 §5.1`): when the extension says nothing,
/// the bytes decide. First 8 KiB: no NUL and valid UTF-8 up to a char
/// boundary ⇒ text. Extension-less scripts and config files keep the
/// full text loop instead of a binary refusal.
#[must_use]
pub fn sniff_is_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8 * 1024)];
    if head.contains(&0) {
        return false;
    }
    match core::str::from_utf8(head) {
        Ok(_) => true,
        // A multi-byte char cut at the window edge is still text.
        Err(e) => e.valid_up_to() + 4 > head.len() && e.error_len().is_none(),
    }
}

/// The lenses available for a content type — advertised in the overview.
#[must_use]
pub fn lenses_for(content_type: &str) -> Vec<&'static str> {
    let mut lenses = vec!["lines", "search"];
    if content_type == "text/markdown" {
        lenses.push("outline");
        lenses.push("section");
    }
    if content_type == "application/json" {
        lenses.push("path");
    }
    lenses
}

/// The overview: what a `read` with no address returns. Size, type,
/// lenses, and the structure the type affords (markdown outline with
/// line numbers; JSON root shape via the CP-7 engine).
#[must_use]
pub fn overview(text: &str, content_type: &str, max_bytes: usize) -> Value {
    let mut out = json!({
        "content_type": content_type,
        "total_lines": text.lines().count(),
        "total_bytes": text.len(),
        "lenses": lenses_for(content_type),
    });
    if content_type == "text/markdown" {
        out["outline"] = outline(text);
    }
    if content_type == "application/json" {
        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
            if let Some(slice) = crate::query::slice_at(&parsed, "", max_bytes / 2) {
                out["shape"] = slice.slice;
            }
        }
    }
    out
}

/// The markdown outline: ATX headings with their 1-based line numbers.
#[must_use]
pub fn outline(text: &str) -> Value {
    let mut in_fence = false;
    let items: Vec<Value> = text
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                return None;
            }
            if in_fence || !line.starts_with('#') {
                return None;
            }
            let level = line.chars().take_while(|c| *c == '#').count();
            (level <= 6 && line.chars().nth(level) == Some(' '))
                .then(|| json!({ "line": i + 1, "level": level, "heading": line[level..].trim() }))
        })
        .collect();
    Value::Array(items)
}

/// A line window, 1-based inclusive, clamped to the budget. Reports the
/// range actually returned so callers can continue precisely.
#[must_use]
pub fn read_lines(text: &str, from: usize, to: usize, max_bytes: usize) -> Value {
    let budget = max_bytes.max(crate::query::MIN_MAX_BYTES);
    let total_lines = text.lines().count();
    let from = from.max(1);
    let mut returned = Vec::new();
    let mut bytes = 0usize;
    let mut truncated_by_budget = false;
    for (i, line) in text.lines().enumerate().skip(from - 1) {
        if i + 1 > to {
            break;
        }
        if bytes + line.len() + 1 > budget {
            truncated_by_budget = true;
            break;
        }
        bytes += line.len() + 1;
        returned.push(line);
    }
    let last = from + returned.len().saturating_sub(1);
    json!({
        "lines": format!("{from}-{last}"),
        "text": returned.join("\n"),
        "total_lines": total_lines,
        "truncated": truncated_by_budget || last < to.min(total_lines),
        "next_window": (last < total_lines).then(|| format!("{}-{}", last + 1, (last + (to - from + 1)).min(total_lines))),
    })
}

/// A markdown section's 1-based inclusive line range: from its heading
/// to the line before the next heading of the same or higher level.
/// Shared by `read --section` and mint-time `section:` contract sugar.
#[must_use]
pub fn section_range(text: &str, heading: &str) -> Option<(usize, usize)> {
    let want = heading.trim().to_lowercase();
    let Value::Array(items) = outline(text) else {
        return None;
    };
    let as_line = |v: &Value| usize::try_from(v.as_u64().unwrap_or(1)).unwrap_or(usize::MAX);
    let (start_line, level) = items.iter().find_map(|h| {
        (h["heading"].as_str()?.to_lowercase() == want)
            .then(|| (as_line(&h["line"]), h["level"].as_u64().unwrap_or(1)))
    })?;
    let end_line = items
        .iter()
        .filter_map(|h| {
            let line = usize::try_from(h["line"].as_u64()?).ok()?;
            (line > start_line && h["level"].as_u64()? <= level).then_some(line)
        })
        .min()
        .map_or(text.lines().count(), |l| l - 1);
    Some((start_line, end_line))
}

/// A markdown section by heading (case-insensitive): from the heading to
/// the next heading of the same or higher level.
#[must_use]
pub fn read_section(text: &str, heading: &str, max_bytes: usize) -> Option<Value> {
    let (start_line, end_line) = section_range(text, heading)?;
    Some(read_lines(text, start_line, end_line, max_bytes))
}

/// Grep: regex matches with context, capped, budgeted. `total_matches`
/// is counted in full even when the returned list is truncated — honesty
/// about what you didn't see is the contract.
pub fn search(
    text: &str,
    pattern: &str,
    context: usize,
    max_matches: usize,
    max_bytes: usize,
) -> Result<Value, String> {
    let re = regex::Regex::new(pattern).map_err(|e| {
        format!("pattern: {e} — Rust regex syntax; prefix (?i) for case-insensitive")
    })?;
    let budget = max_bytes.max(crate::query::MIN_MAX_BYTES);
    let max_matches = max_matches.clamp(1, 50);
    let lines: Vec<&str> = text.lines().collect();
    let mut matches = Vec::new();
    let mut total = 0usize;
    let mut bytes = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if !re.is_match(line) {
            continue;
        }
        total += 1;
        if matches.len() >= max_matches {
            continue; // keep counting, stop collecting
        }
        let before: Vec<&str> = lines[i.saturating_sub(context)..i].to_vec();
        let after_end = (i + 1 + context).min(lines.len());
        let after: Vec<&str> = lines[i + 1..after_end].to_vec();
        let entry_bytes = line.len()
            + before.iter().map(|l| l.len()).sum::<usize>()
            + after.iter().map(|l| l.len()).sum::<usize>()
            + 48;
        if bytes + entry_bytes > budget {
            continue; // budget full — keep counting totals
        }
        bytes += entry_bytes;
        matches.push(json!({
            "line": i + 1,
            "text": line,
            "before": before,
            "after": after,
        }));
    }
    let returned = matches.len();
    Ok(json!({
        "matches": matches,
        "total_matches": total,
        "returned": returned,
        "truncated": returned < total,
    }))
}

/// JSON pointer lens: parse and delegate to the CP-7 slice engine — one
/// budget discipline, one path syntax, already proven.
pub fn read_path(text: &str, path: &str, max_bytes: usize) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(text)
        .map_err(|e| format!("content is not valid JSON ({e}) — use lines/search on it instead"))?;
    let Some(slice) = crate::query::slice_at(&parsed, path, max_bytes) else {
        let roots = crate::query::slice_at(&parsed, "", max_bytes)
            .map(|s| s.next_paths.join(", "))
            .unwrap_or_default();
        return Err(format!(
            "path `{path}` names nothing — valid roots: {roots}"
        ));
    };
    Ok(json!({
        "path": path,
        "slice": slice.slice,
        "truncated": slice.truncated,
        "full_bytes": slice.full_bytes,
        "next_paths": slice.next_paths,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "# Report\nintro line\n\n## Methodology\nhow we did it\nmore method\n\n```\n# not a heading\n```\n\n## Pricing\ncompetitor pricing clusters at $49\nline after\n\n### Detail\nnested\n\n## Risks\nthe risks\n";

    #[test]
    fn outline_finds_headings_and_skips_fences() {
        let o = outline(MD);
        let heads: Vec<&str> = o
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["heading"].as_str().unwrap())
            .collect();
        assert_eq!(
            heads,
            vec!["Report", "Methodology", "Pricing", "Detail", "Risks"]
        );
    }

    #[test]
    fn line_windows_clamp_and_continue() {
        let w = read_lines(MD, 4, 6, 4096);
        assert_eq!(w["lines"], "4-6");
        assert!(w["text"].as_str().unwrap().starts_with("## Methodology"));
        assert_eq!(w["next_window"], "7-9");

        // Budget cuts mid-window and says so.
        let tight = read_lines(MD, 1, 100, 64);
        assert_eq!(tight["truncated"], true);
        assert!(tight["text"].as_str().unwrap().len() <= 64);
    }

    #[test]
    fn sections_span_to_the_next_same_level_heading() {
        let s = read_section(MD, "pricing", 4096).unwrap();
        let text = s["text"].as_str().unwrap();
        assert!(text.contains("competitor pricing"));
        assert!(
            text.contains("### Detail"),
            "subsections belong to the section"
        );
        assert!(!text.contains("## Risks"), "stops before the next sibling");
        assert!(read_section(MD, "nope", 4096).is_none());
    }

    #[test]
    fn search_counts_fully_even_when_truncated() {
        let text = (0..40)
            .map(|i| format!("line {i} pricing"))
            .collect::<Vec<_>>()
            .join("\n");
        let r = search(&text, "pricing", 1, 5, 4096).unwrap();
        assert_eq!(r["total_matches"], 40);
        assert_eq!(r["returned"], 5);
        assert_eq!(r["truncated"], true);
        assert_eq!(r["matches"][0]["line"], 1);
        assert_eq!(r["matches"][0]["after"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn search_respects_budget_and_rejects_bad_regex() {
        let text = "x\n".repeat(1000) + "needle wide line";
        let r = search(&text, "needle", 2, 50, 64).unwrap();
        assert_eq!(r["total_matches"], 1);
        assert!(search("x", "(unclosed", 2, 5, 4096).is_err());
    }

    #[test]
    fn json_pointer_delegates_to_the_query_engine() {
        let text = r#"{"dependencies":{"react":{"version":"18.3.1"}}}"#;
        let r = read_path(text, "/dependencies/react/version", 4096).unwrap();
        assert_eq!(r["slice"], "18.3.1");
        let err = read_path(text, "/nope", 4096).unwrap_err();
        assert!(
            err.contains("/dependencies"),
            "names the valid roots: {err}"
        );
        assert!(read_path("not json", "/a", 4096).is_err());
    }

    #[test]
    fn overview_advertises_lenses_by_type() {
        let o = overview(MD, "text/markdown", 4096);
        assert!(o["lenses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "section"));
        assert!(o["outline"].is_array());
        let j = overview("{\"a\":1}", "application/json", 4096);
        assert!(j["lenses"].as_array().unwrap().iter().any(|l| l == "path"));
        assert!(is_text("text/x-rust"));
        assert!(!is_text("image/png"));
    }

    #[test]
    fn budget_property_over_random_requests() {
        let mut state = 0xD1CE_u32;
        let mut rnd = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let long = (0..500)
            .map(|i| format!("line {i} with some pricing text"))
            .collect::<Vec<_>>()
            .join("\n");
        for _ in 0..500 {
            let budget = crate::query::MIN_MAX_BYTES + (rnd() as usize) % 4096;
            let from = 1 + (rnd() as usize) % 400;
            let w = read_lines(&long, from, from + (rnd() as usize) % 100, budget);
            assert!(w["text"].as_str().unwrap().len() <= budget);
            let s = search(&long, "pricing", 2, 1 + (rnd() as usize) % 50, budget).unwrap();
            let ser = serde_json::to_string(&s["matches"]).unwrap();
            // matches payload stays within budget + envelope slack
            assert!(ser.len() <= budget + 256, "{} > {budget}", ser.len());
        }
    }
}
