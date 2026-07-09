//! The waggle seam: one trait method (`call`), the daemon-backed binary
//! behind it. Scaffolding per the standards doc §12 — the native shape
//! (socket line protocol) replaces the internals without changing
//! callers, because everything downstream only sees envelopes.

use crate::error::{Error, Result};

/// The only thing a client must do: run one waggle verb, return the
/// parsed envelope.
pub trait WaggleClient {
    /// Run `waggle <args>`, parsing stdout as the JSON envelope.
    fn call(&self, args: &[&str]) -> Result<serde_json::Value>;
}

/// The `waggle` binary on PATH (override with `WAGGLE_TMUX_BIN` — the
/// integration tests point this at a fresh build).
pub struct BinWaggle;

impl WaggleClient for BinWaggle {
    fn call(&self, args: &[&str]) -> Result<serde_json::Value> {
        let bin = std::env::var("WAGGLE_TMUX_BIN").unwrap_or_else(|_| "waggle".into());
        let out = std::process::Command::new(&bin)
            .args(args)
            .output()
            .map_err(|e| {
                Error::Waggle(format!(
                    "is `{bin}` installed? cargo install waggle-cli ({e})"
                ))
            })?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        let body: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|_| {
            Error::Waggle(format!(
                "`waggle {}` answered non-JSON: {}",
                args.join(" "),
                if stdout.trim().is_empty() {
                    String::from_utf8_lossy(&out.stderr).trim().to_owned()
                } else {
                    stdout.trim().to_owned()
                }
            ))
        })?;
        if let Some(hint) = body.get("hint").and_then(serde_json::Value::as_str) {
            return Err(Error::Waggle(hint.to_owned()));
        }
        Ok(body)
    }
}

/// Mint a snapshot (or a whole tree) on a channel, optionally chained
/// to a parent — the outcome primitive (seamless §4).
pub fn mint<W: WaggleClient>(
    waggle: &W,
    target: &str,
    tree: bool,
    parent: Option<&str>,
) -> Result<String> {
    // A directory root can't snapshot ITSELF — --tree snapshots the
    // children; files snapshot directly.
    let mut args = vec!["mint", "--target", target, "--channel", "tmux/outcome"];
    args.push(if tree { "--tree" } else { "--snapshot" });
    if let Some(p) = parent {
        args.extend(["--parent", p]);
    }
    let body = waggle.call(&args)?;
    body["result"]["token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Waggle(format!("mint returned no token: {body}")))
}

/// Record a lifecycle stage.
pub fn record<W: WaggleClient>(waggle: &W, token: &str, stage: &str) -> Result<()> {
    waggle.call(&["record", "--token", token, "--stage", stage])?;
    Ok(())
}

/// The token's resolve-stage count (the consumption baseline/signal).
pub fn resolve_count<W: WaggleClient>(waggle: &W, token: &str) -> Result<u64> {
    let body = waggle.call(&["funnel", "--token", token])?;
    Ok(body["result"]["stages"]["resolve"].as_u64().unwrap_or(0))
}

/// Resolve WITH a destination context — the switch-time preview
/// (seamless §5.2). Returns a one-line human summary.
pub fn preview<W: WaggleClient>(
    waggle: &W,
    token: &str,
    context: &serde_json::Value,
) -> Result<String> {
    let ctx = context.to_string();
    let body = waggle.call(&["resolve", "--token", token, "--context", &ctx])?;
    let result = &body["result"];
    let disposition = result["disposition"].to_string().replace('"', "");
    let variant = result["variant"]
        .as_u64()
        .map_or(String::new(), |v| format!(" · variant {v}"));
    let head = result["body"]["inline"]["data"]
        .as_str()
        .map(|d| d.chars().take(60).collect::<String>())
        .unwrap_or_default();
    Ok(format!("{disposition}{variant} · {head}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(serde_json::Value);
    impl WaggleClient for Fake {
        fn call(&self, _args: &[&str]) -> Result<serde_json::Value> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn mint_extracts_the_token() {
        let fake = Fake(serde_json::json!({ "result": { "token": "b2uQyZUC" } }));
        assert_eq!(mint(&fake, "file:///x", false, None).unwrap(), "b2uQyZUC");
    }

    #[test]
    fn hint_envelopes_become_errors() {
        struct Hinted;
        impl WaggleClient for Hinted {
            fn call(&self, _: &[&str]) -> Result<serde_json::Value> {
                let body = serde_json::json!({ "hint": "unknown token x" });
                if let Some(h) = body.get("hint").and_then(serde_json::Value::as_str) {
                    return Err(Error::Waggle(h.to_owned()));
                }
                Ok(body)
            }
        }
        assert!(record(&Hinted, "x", "run").is_err());
    }

    #[test]
    fn preview_summarizes_disposition_variant_head() {
        let fake = Fake(serde_json::json!({ "result": {
            "disposition": "active", "variant": 0,
            "body": { "inline": { "data": "Fetch the artifact at ws://x and use it." } },
        }}));
        let line = preview(&fake, "t", &serde_json::json!({})).unwrap();
        assert!(line.starts_with("active · variant 0 · Fetch the artifact"));
    }
}
