//! Argument parsing for the consumption-contract half of `mint`
//! (doc `19 §4.2`) and the `mutate` change grammar — the string-to-domain
//! seam, kept out of `handlers.rs` so each stays one concept.
//!
//! `--require` grammar, repeatable:
//! - `lines:START-END` — a 1-based inclusive line range;
//! - `section:HEADING` — sugar resolved AT MINT against the target's
//!   markdown outline (the artifact is at hand exactly once — mint
//!   time), stored as the resolved range with the heading as its label;
//! - `symbol:NAME` — the code analog (doc `20 §5.6`): resolved at mint
//!   against the extracted symbol outline (`code-lens` feature).
//!
//! Contracts are plain line ranges in the manifest; nothing re-resolves
//! later. `--min-coverage` is a fraction in (0, 1] of required regions
//! (default 1.0 — every region).

use serde_json::{Map, Value};
use waggle_core::{Change, Contract, Region, Timestamp, Token, FULL_COVERAGE_PERMILLE};

use crate::envelope::Envelope;

/// Parse the contract args off a mint call, if any. `text` loads the
/// target's content lazily — only a `section:` requirement needs it.
pub(crate) fn parse_contract(
    args: &Map<String, Value>,
    mut text: impl FnMut() -> Result<String, Envelope>,
) -> Result<Option<Contract>, Envelope> {
    let specs = requirement_specs(args);
    if specs.is_empty() {
        if args.get("min-coverage").is_some() {
            return Err(Envelope::err(
                "min-coverage without require — declare the regions the threshold applies to",
                vec![],
            ));
        }
        return Ok(None);
    }
    let mut regions = Vec::new();
    let mut outline_text: Option<String> = None;
    for (i, spec) in specs.iter().enumerate() {
        let region = if let Some(range) = spec.strip_prefix("lines:") {
            let (start, end) =
                parse_range(range).ok_or_else(|| Envelope::err(bad_require(spec), vec![]))?;
            Region::new(None, start, end, i)
        } else if let Some(heading) = spec.strip_prefix("section:") {
            let t = match &outline_text {
                Some(t) => t,
                None => outline_text.insert(text()?),
            };
            let (start, end) = crate::content::section_range(t, heading).ok_or_else(|| {
                Envelope::err(
                    format!(
                        "require: no section `{heading}` in the target — the outline is: {}",
                        crate::content::outline(t)
                    ),
                    vec![],
                )
            })?;
            let clamp = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
            Region::new(Some(heading.trim().to_owned()), clamp(start), clamp(end), i)
        } else if let Some(name) = spec.strip_prefix("symbol:") {
            let name = name.trim();
            let (start, end) = resolve_symbol_requirement(args, name, &mut text)
                .map_err(|e| Envelope::err(format!("require: {e}"), vec![]))?;
            Region::new(Some(name.to_owned()), start, end, i)
        } else {
            return Err(Envelope::err(bad_require(spec), vec![]));
        };
        regions.push(region.map_err(|e| Envelope::err(format!("require: {e}"), vec![]))?);
    }
    let min_permille = match args.get("min-coverage") {
        None => FULL_COVERAGE_PERMILLE,
        Some(v) => {
            let f = v
                .as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                .filter(|f| *f > 0.0 && *f <= 1.0)
                .ok_or_else(|| {
                    Envelope::err(
                        "min-coverage: a fraction in (0, 1] of required regions (e.g. 0.9)",
                        vec![],
                    )
                })?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            // f ∈ (0, 1] so the product is in (0, 1000] — exact in f64.
            {
                ((f * f64::from(FULL_COVERAGE_PERMILLE)).round() as u16).max(1)
            }
        }
    };
    Contract::new(regions, min_permille)
        .map(Some)
        .map_err(|e| Envelope::err(format!("require: {e}"), vec![]))
}

/// The `require` argument as a list: a JSON array of strings (the CLI's
/// repeatable flag) or one bare string (a hand-written tool call).
fn requirement_specs(args: &Map<String, Value>) -> Vec<String> {
    match args.get("require") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn parse_range(range: &str) -> Option<(u32, u32)> {
    let (a, b) = range.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn bad_require(spec: &str) -> String {
    format!(
        "require `{spec}` — expected lines:START-END (1-based inclusive), section:HEADING, or symbol:NAME"
    )
}

/// Resolve a `symbol:NAME` requirement at mint (doc `20 §5.6`): extract
/// the outline from the target's text and pin the definition's range.
/// Every failure names its fix, including the build that cannot.
#[cfg(feature = "code-lens")]
fn resolve_symbol_requirement(
    args: &Map<String, Value>,
    name: &str,
    text: &mut impl FnMut() -> Result<String, Envelope>,
) -> Result<(u32, u32), String> {
    let target = args.get("target").and_then(Value::as_str).unwrap_or("");
    let lang = waggle_lens_code::detect(target)
        .ok_or("symbol: the target has no supported grammar — declare lines:START-END")?;
    let t = text().map_err(|_| "symbol: needs a locally readable target".to_owned())?;
    let outline = waggle_lens_code::extract(&t, lang);
    let hits = outline.find(name);
    match hits.as_slice() {
        [] => {
            let known: Vec<&str> = (0..outline.len().min(12))
                .filter_map(|i| outline.name_at(i))
                .collect();
            Err(format!(
                "no symbol `{name}` — the outline has: {}",
                known.join(", ")
            ))
        }
        [one] => {
            let (start, end, _) = outline.lines_of(*one).ok_or("outline index")?;
            Ok((start, end))
        }
        many => Err(format!(
            "symbol `{name}` is ambiguous — declare a range instead: {}",
            many.iter()
                .filter_map(|&i| outline.lines_of(i))
                .map(|(s, e, k)| format!("{k} @ {s}-{e}"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Without the `code-lens` feature the refusal teaches the alternative.
#[cfg(not(feature = "code-lens"))]
fn resolve_symbol_requirement(
    _args: &Map<String, Value>,
    _name: &str,
    _text: &mut impl FnMut() -> Result<String, Envelope>,
) -> Result<(u32, u32), String> {
    Err("this build lacks the code-lens feature — declare lines:START-END instead".to_owned())
}

/// The `mutate` change grammar (moved verbatim from `handlers.rs`).
pub(crate) fn parse_change(raw: &str) -> Result<Change, String> {
    if raw == "revoke" {
        return Ok(Change::Revoked);
    }
    if let Some(by) = raw.strip_prefix("supersede=") {
        let token = Token::parse(by).map_err(|e| format!("supersede target: {e}"))?;
        return Ok(Change::Superseded { by: token });
    }
    if let Some(ts) = raw.strip_prefix("expire=") {
        let ms: u64 = ts
            .parse()
            .map_err(|_| "expire= takes unix milliseconds".to_owned())?;
        return Ok(Change::ExpirySet {
            expires_at: Some(Timestamp::from_unix_ms(ms)),
        });
    }
    if let Some(kv) = raw.strip_prefix("label ") {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| "label takes key=value".to_owned())?;
        return Ok(Change::LabelSet {
            key: k.to_owned(),
            value: v.to_owned(),
        });
    }
    Err(format!(
        "`{raw}` is not a change — revoke, supersede=<token>, expire=<unix-ms>, or label k=v"
    ))
}

/// Region-touch stamping helpers (doc `19 §4.2`): which contract bits a
/// served response reached. `None` (never `Some(0)`) when nothing was
/// touched, so contract-free traffic writes no field at all.
pub(crate) fn nonzero(bits: u8) -> Option<u8> {
    (bits != 0).then_some(bits)
}

/// Bits for a served line window, read off the result's `lines: "A-B"`.
pub(crate) fn span_bits(contract: Option<&Contract>, result: &Value) -> Option<u8> {
    let c = contract?;
    let (from, to) = parse_range(result["lines"].as_str()?)?;
    nonzero(c.touched_by_span(from, to))
}

/// Bits for served search matches, one touch per matched line.
pub(crate) fn match_bits(contract: Option<&Contract>, result: &Value) -> Option<u8> {
    let c = contract?;
    let mut bits = 0u8;
    for m in result["matches"].as_array().into_iter().flatten() {
        if let Some(line) = m["line"].as_u64().and_then(|l| u32::try_from(l).ok()) {
            bits |= c.touched_by_line(line);
        }
    }
    nonzero(bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(v: &Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn no_text() -> Result<String, Envelope> {
        panic!("text loader must not be called without a section: requirement")
    }

    #[test]
    fn absent_require_is_no_contract_and_lazy_text_stays_unloaded() {
        let c = parse_contract(&args(&json!({ "target": "x" })), no_text).unwrap();
        assert!(c.is_none());
    }

    #[test]
    fn lines_and_threshold_parse_and_validate() {
        let c = parse_contract(
            &args(&json!({ "require": ["lines:10-40", "lines:80-90"], "min-coverage": 0.5 })),
            no_text,
        )
        .unwrap()
        .unwrap();
        assert_eq!(c.regions().len(), 2);
        assert_eq!(c.min_permille(), 500);
        // Bad shapes are refused with the grammar named.
        for bad in ["lines:40-10", "lines:x-y", "pages:1-2"] {
            assert!(parse_contract(&args(&json!({ "require": [bad] })), no_text).is_err());
        }
        assert!(
            parse_contract(&args(&json!({ "min-coverage": 0.5 })), no_text).is_err(),
            "a threshold with no regions is a mistake worth naming"
        );
    }

    #[test]
    fn section_sugar_resolves_against_the_outline_at_mint() {
        let text = "# Top\nintro\n## Pricing\nrow\nrow\n## Terms\nfine print\n";
        let c = parse_contract(&args(&json!({ "require": ["section:Pricing"] })), || {
            Ok(text.to_owned())
        })
        .unwrap()
        .unwrap();
        let r = &c.regions()[0];
        assert_eq!(r.label(), Some("Pricing"));
        assert_eq!((r.start(), r.end()), (3, 5));
        assert!(
            parse_contract(&args(&json!({ "require": ["section:Nope"] })), || Ok(
                text.to_owned()
            ))
            .is_err(),
            "a missing section fails AT MINT, naming the outline"
        );
    }

    #[test]
    fn stamping_reads_served_shapes() {
        let c = Contract::new(
            vec![
                Region::new(None, 10, 20, 0).unwrap(),
                Region::new(None, 50, 60, 1).unwrap(),
            ],
            1000,
        )
        .unwrap();
        let window = json!({ "lines": "15-55" });
        assert_eq!(span_bits(Some(&c), &window), Some(0b11));
        assert_eq!(span_bits(None, &window), None);
        assert_eq!(span_bits(Some(&c), &json!({ "lines": "1-5" })), None);
        let hits = json!({ "matches": [{ "line": 12 }, { "line": 99 }] });
        assert_eq!(match_bits(Some(&c), &hits), Some(0b01));
    }
}
