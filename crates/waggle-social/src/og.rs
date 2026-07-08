//! Open Graph meta block — what an unfurl bot receives. Rendered from the
//! mint-time snapshot only (I-3): the preview is what the sharer approved,
//! never a live scrape, and revoked tokens simply stop resolving.

use std::fmt::Write as _;

use crate::package::SharePackage;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The `<meta>` block for a token's unfurl page.
#[must_use]
pub fn og_meta(p: &SharePackage) -> String {
    let mut out = format!(
        "<meta property=\"og:title\" content=\"{}\">\n<meta property=\"og:url\" content=\"{}\">\n",
        escape(&p.title),
        escape(&p.url),
    );
    if !p.description.is_empty() {
        let _ = writeln!(
            out,
            "<meta property=\"og:description\" content=\"{}\">",
            escape(&p.description)
        );
    }
    if let Some(image) = &p.image_url {
        let _ = writeln!(
            out,
            "<meta property=\"og:image\" content=\"{}\">",
            escape(image)
        );
    }
    out.push_str("<meta name=\"twitter:card\" content=\"summary_large_image\">\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn og_block_snapshot_and_escaping() {
        let p = SharePackage {
            url: "https://wgl.example/7Kp2mQ9x".into(),
            title: "Q3 <Market> \"Report\" & Co".into(),
            description: "Findings.".into(),
            image_url: Some("https://example.com/og.png".into()),
            token: "7Kp2mQ9x".into(),
        };
        let block = og_meta(&p);
        assert_eq!(
            block,
            "<meta property=\"og:title\" content=\"Q3 &lt;Market&gt; &quot;Report&quot; &amp; Co\">\n\
             <meta property=\"og:url\" content=\"https://wgl.example/7Kp2mQ9x\">\n\
             <meta property=\"og:description\" content=\"Findings.\">\n\
             <meta property=\"og:image\" content=\"https://example.com/og.png\">\n\
             <meta name=\"twitter:card\" content=\"summary_large_image\">\n"
        );
        // Purity: byte-identical on re-render.
        assert_eq!(block, og_meta(&p));
    }
}
