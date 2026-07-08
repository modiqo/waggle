//! Channel artifacts: one [`SharePackage`] in, one string per channel out.
//! Every renderer is pure — same package, byte-identical artifact, always
//! (the CP-8 purity gate) — and each respects its channel's conventions
//! instead of pasting one blob everywhere (design doc `05 §3`).

use crate::package::SharePackage;

/// A Slack/Discord message line: bold title, link, one-line context.
#[must_use]
pub fn slack_message(p: &SharePackage) -> String {
    let mut out = format!("*{}*\n{}", p.title, p.url);
    if !p.description.is_empty() {
        out.push('\n');
        out.push_str(&p.description);
    }
    out
}

/// An X/Twitter post: title + link, description while it fits 280 chars
/// (links count ~23 — we budget conservatively on the full URL length).
#[must_use]
pub fn x_post(p: &SharePackage) -> String {
    let base = format!("{} {}", p.title, p.url);
    if p.description.is_empty() {
        return base;
    }
    let budget = 280usize.saturating_sub(base.chars().count() + 2);
    if budget < 12 {
        return base;
    }
    let mut desc: String = p.description.chars().take(budget).collect();
    if desc.chars().count() < p.description.chars().count() {
        // Reserve one char for the ellipsis so the budget holds exactly.
        let mut trimmed: String = desc.chars().take(budget - 1).collect();
        trimmed.truncate(trimmed.trim_end().len());
        trimmed.push('…');
        desc = trimmed;
    }
    format!("{base}\n\n{desc}")
}

/// An email: `(subject, body)`. The body says what the link is before
/// asking anyone to click it.
#[must_use]
pub fn email(p: &SharePackage) -> (String, String) {
    let subject = p.title.clone();
    let mut body = String::new();
    if !p.description.is_empty() {
        body.push_str(&p.description);
        body.push_str("\n\n");
    }
    body.push_str(&p.url);
    body.push('\n');
    (subject, body)
}

/// A Markdown link with optional trailing context — for READMEs, issues,
/// agent-to-human handoffs in chat UIs.
#[must_use]
pub fn markdown(p: &SharePackage) -> String {
    if p.description.is_empty() {
        format!("[{}]({})", p.title, p.url)
    } else {
        format!("[{}]({}) — {}", p.title, p.url, p.description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package() -> SharePackage {
        SharePackage {
            url: "https://wgl.example/7Kp2mQ9x".into(),
            title: "Q3 Market Report".into(),
            description: "Findings from the agent swarm.".into(),
            image_url: None,
            token: "7Kp2mQ9x".into(),
        }
    }

    #[test]
    fn slack_artifact_snapshot() {
        assert_eq!(
            slack_message(&package()),
            "*Q3 Market Report*\nhttps://wgl.example/7Kp2mQ9x\nFindings from the agent swarm."
        );
    }

    #[test]
    fn x_post_snapshot_and_280_budget() {
        assert_eq!(
            x_post(&package()),
            "Q3 Market Report https://wgl.example/7Kp2mQ9x\n\nFindings from the agent swarm."
        );
        let mut long = package();
        long.description = "d".repeat(400);
        let post = x_post(&long);
        assert!(post.chars().count() <= 280, "{}", post.chars().count());
        assert!(post.ends_with('…'));
    }

    #[test]
    fn email_snapshot() {
        let (subject, body) = email(&package());
        assert_eq!(subject, "Q3 Market Report");
        assert_eq!(
            body,
            "Findings from the agent swarm.\n\nhttps://wgl.example/7Kp2mQ9x\n"
        );
    }

    #[test]
    fn markdown_snapshot() {
        assert_eq!(
            markdown(&package()),
            "[Q3 Market Report](https://wgl.example/7Kp2mQ9x) — Findings from the agent swarm."
        );
    }

    #[test]
    fn purity_same_package_byte_identical_artifacts() {
        let p = package();
        for _ in 0..3 {
            assert_eq!(slack_message(&p), slack_message(&p));
            assert_eq!(x_post(&p), x_post(&p));
            assert_eq!(email(&p), email(&p));
            assert_eq!(markdown(&p), markdown(&p));
        }
    }
}
