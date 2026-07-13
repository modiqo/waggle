//! Deterministic text extraction for opaque artifacts (design doc `18 §7`).
//!
//! The rule the substrate holds to: **it may read an artifact only when reading
//! is deterministic and provenance is recorded.** A PDF's embedded text layer and
//! an HTML document's text are pure functions of the bytes — the substrate
//! extracts them at mint, so `read`/`search`/`coverage` work over the artifact
//! itself and the searchable text travels *with* the token.
//!
//! What it will not do is guess. Audio, video, and scanned images carry no text
//! layer; recovering their content needs a model, and a model's output is an
//! opinion that drifts — non-deterministic, and worse than the consumer's own
//! perception. Those return `None` here: the raw bytes are attached, the
//! projection says so plainly, and the consumer's own vision or speech stack does
//! the reading. The substrate never *defaults* to a model, because the moment it
//! does, a coverage receipt could attest to a transcript nobody can reproduce.

/// A deterministic extraction: the recovered text and the extractor that made it.
pub struct Extracted {
    pub text: String,
    /// Provenance id recorded in the manifest — e.g. `pdf-textlayer`.
    pub extractor: &'static str,
}

/// Extract text from an opaque artifact, IF a deterministic extractor exists for
/// its type. `None` means "no deterministic reading is possible" — the caller
/// attaches the raw bytes and lets the consumer perceive them.
///
/// This is the whole MIME-directed policy in one place. It reads content, so it
/// is pure and side-effect-free; nothing here touches the log.
pub fn deterministic_extract(content_type: &str, bytes: &[u8]) -> Option<Extracted> {
    match base_type(content_type) {
        "application/pdf" => extract_pdf(bytes),
        "text/html" | "application/xhtml+xml" => extract_html(bytes),
        _ => None,
    }
}

fn base_type(content_type: &str) -> &str {
    content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim()
}

/// A PDF's embedded text layer. Deterministic: the same bytes yield the same
/// text, so a coverage receipt over the extraction is reproducible. Returns
/// `None` when the PDF carries no text layer at all (a pure scan) — that case is
/// an image, and belongs to the consumer's OCR, not ours.
#[cfg(feature = "doc-extract")]
fn extract_pdf(bytes: &[u8]) -> Option<Extracted> {
    let text = pdf_extract::extract_text_from_mem(bytes).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    Some(Extracted {
        text,
        extractor: "pdf-textlayer",
    })
}

#[cfg(not(feature = "doc-extract"))]
fn extract_pdf(_bytes: &[u8]) -> Option<Extracted> {
    None
}

/// The visible text of an HTML document, tags stripped. Deterministic and pure —
/// no crate needed, and it keeps the wasm/edge build lean. `script` and `style`
/// bodies are dropped; entities are left as written (a lens is over structure,
/// and an over-clever decode would only add ways to disagree with the source).
fn extract_html(bytes: &[u8]) -> Option<Extracted> {
    let src = core::str::from_utf8(bytes).ok()?;
    let mut out = String::with_capacity(src.len() / 2);
    let mut chars = src.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '<' {
            out.push(c);
            continue;
        }
        // A tag. Peek the name to drop script/style bodies whole.
        let rest = &src[i..];
        let lower = rest
            .get(..16.min(rest.len()))
            .unwrap_or(rest)
            .to_ascii_lowercase();
        let skip_to = if lower.starts_with("<script") {
            Some("</script>")
        } else if lower.starts_with("<style") {
            Some("</style>")
        } else {
            None
        };
        if let Some(close) = skip_to {
            if let Some(end) = rest.to_ascii_lowercase().find(close) {
                for _ in 0..(end + close.len()).saturating_sub(1) {
                    chars.next();
                }
                continue;
            }
        }
        // An ordinary tag: consume through its '>'.
        for (_, tc) in chars.by_ref() {
            if tc == '>' {
                break;
            }
        }
        // A tag boundary is whitespace, so words on either side don't fuse.
        out.push(' ');
    }
    // Collapse the runs of whitespace the tag stripping leaves behind.
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    Some(Extracted {
        text,
        extractor: "html-strip",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_strips_tags_and_drops_script() {
        let html = b"<html><head><style>p{color:red}</style></head>\
                     <body><h1>Runbook</h1><p>retry budget of 3</p>\
                     <script>alert('x')</script></body></html>";
        let e = deterministic_extract("text/html", html).expect("html extractable");
        assert_eq!(e.extractor, "html-strip");
        assert!(e.text.contains("Runbook"));
        assert!(e.text.contains("retry budget of 3"));
        assert!(!e.text.contains("color:red"), "style body must be dropped");
        assert!(!e.text.contains("alert"), "script body must be dropped");
    }

    #[test]
    fn html_does_not_fuse_words_across_tags() {
        let e = deterministic_extract("text/html", b"<p>one</p><p>two</p>").unwrap();
        assert!(e.text.contains("one two"), "got {:?}", e.text);
    }

    #[test]
    fn charset_parameter_is_tolerated() {
        assert!(deterministic_extract("text/html; charset=utf-8", b"<p>hi</p>").is_some());
    }

    #[test]
    fn opaque_media_is_not_extractable() {
        assert!(deterministic_extract("audio/mp4", b"\x00\x01\x02").is_none());
        assert!(deterministic_extract("image/png", b"\x89PNG").is_none());
        assert!(deterministic_extract("video/mp4", b"\x00\x01\x02").is_none());
    }

    #[test]
    fn html_extraction_is_deterministic() {
        let h = b"<div><span>alpha</span> <span>beta</span></div>";
        let a = deterministic_extract("text/html", h).unwrap().text;
        let b = deterministic_extract("text/html", h).unwrap().text;
        assert_eq!(a, b);
    }
}
