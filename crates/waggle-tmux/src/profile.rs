//! Harness profiles: data, not plugins (standards doc §5). The four
//! matcher-visible fields project into a `ResolverContext` — profiles
//! that never touch the matcher are dead configuration (seamless §2.2).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// What a harness IS, well enough to resolve as it and launch it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessProfile {
    /// Profile id, e.g. `claude-code` (from the TOML table key).
    #[serde(default)]
    pub id: String,
    /// Human display label.
    pub display_name: String,
    /// Coarse model family: claude | gpt | gemini | other.
    pub family: String,
    /// Harness slug: claude-code | codex | other.
    pub harness: String,
    /// Modalities the harness presents (text, shell, ...).
    pub modalities: Vec<String>,
    /// attended | headless | ci.
    pub posture: String,
    /// The command that launches an interactive pane.
    #[serde(default)]
    pub launch_command: Option<String>,
    /// Per-harness phrasing of the delivered instruction; `{token}`
    /// is substituted. None uses [`DEFAULT_INJECT`].
    #[serde(default)]
    pub inject_template: Option<String>,
}

/// The default delivered instruction (seamless §5.5).
pub const DEFAULT_INJECT: &str = "Resolve {token} via waggle for your working context. \
     Use waggle search/read for slices; record --stage run when you have used it.";

impl HarnessProfile {
    /// The matcher-visible projection (seamless §2.2): exactly the four
    /// coarse fields, as the JSON `waggle resolve --context` accepts.
    #[must_use]
    pub fn resolver_context(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "agent",
            "model_family": self.family,
            "harness": self.harness,
            "modalities": self.modalities,
            "posture": self.posture,
        })
    }

    /// The instruction to deliver for `token`.
    #[must_use]
    pub fn inject_line(&self, token: &str) -> String {
        self.inject_template
            .as_deref()
            .unwrap_or(DEFAULT_INJECT)
            .replace("{token}", token)
    }
}

/// The builtin registry: Claude Code, Codex, and a generic shell agent.
#[must_use]
pub fn builtins() -> Vec<HarnessProfile> {
    vec![
        HarnessProfile {
            id: "claude-code".into(),
            display_name: "Claude Code".into(),
            family: "claude".into(),
            harness: "claude-code".into(),
            modalities: vec!["text".into(), "shell".into()],
            posture: "attended".into(),
            launch_command: Some("claude".into()),
            inject_template: None,
        },
        HarnessProfile {
            id: "codex".into(),
            display_name: "Codex".into(),
            family: "gpt".into(),
            harness: "codex".into(),
            modalities: vec!["text".into(), "shell".into()],
            posture: "attended".into(),
            launch_command: Some("codex".into()),
            inject_template: None,
        },
        HarnessProfile {
            id: "generic".into(),
            display_name: "Generic Agent".into(),
            family: "other".into(),
            harness: "other".into(),
            modalities: vec!["text".into(), "shell".into()],
            posture: "attended".into(),
            launch_command: None,
            inject_template: None,
        },
    ]
}

/// Builtins merged with `[profiles.*]` from `.waggle/tmux/config.toml`
/// (user entries win by id).
pub fn load(config_path: &std::path::Path) -> Result<Vec<HarnessProfile>> {
    let mut profiles = builtins();
    if config_path.exists() {
        let raw = std::fs::read_to_string(config_path)?;
        let doc: toml::Value = raw
            .parse()
            .map_err(|e| Error::Config(format!("{}: {e}", config_path.display())))?;
        if let Some(table) = doc.get("profiles").and_then(|p| p.as_table()) {
            for (id, body) in table {
                let mut profile: HarnessProfile = body
                    .clone()
                    .try_into()
                    .map_err(|e| Error::Config(format!("profile `{id}`: {e}")))?;
                profile.id.clone_from(id);
                profiles.retain(|p| p.id != *id);
                profiles.push(profile);
            }
        }
    }
    Ok(profiles)
}

/// Find a profile by id, with a fix-naming error.
pub fn find<'p>(profiles: &'p [HarnessProfile], id: &str) -> Result<&'p HarnessProfile> {
    profiles.iter().find(|p| p.id == id).ok_or_else(|| {
        let known: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        Error::NotFound(format!(
            "profile `{id}` — known: {} (add more under [profiles.{id}] in .waggle/tmux/config.toml)",
            known.join(", ")
        ))
    })
}

/// Is the profile's launch command installed on this machine?
#[must_use]
pub fn detected(profile: &HarnessProfile) -> bool {
    profile.launch_command.as_deref().is_some_and(|cmd| {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {cmd}")])
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_projects_exactly_the_four_fields() {
        let ctx = builtins()[1].resolver_context();
        assert_eq!(ctx["model_family"], "gpt");
        assert_eq!(ctx["harness"], "codex");
        assert_eq!(ctx["posture"], "attended");
        assert_eq!(ctx["kind"], "agent");
    }

    #[test]
    fn config_overrides_builtin_and_adds_custom() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[profiles.codex]
display_name = "Codex Nightly"
family = "gpt"
harness = "codex"
modalities = ["text"]
posture = "headless"
launch_command = "codex-nightly"

[profiles.aider]
display_name = "Aider"
family = "other"
harness = "aider"
modalities = ["text", "shell"]
posture = "attended"
"#,
        )
        .unwrap();
        let profiles = load(&path).unwrap();
        let codex = find(&profiles, "codex").unwrap();
        assert_eq!(codex.display_name, "Codex Nightly");
        assert_eq!(codex.posture, "headless");
        assert!(find(&profiles, "aider").is_ok());
        assert!(find(&profiles, "claude-code").is_ok(), "builtins survive");
    }

    #[test]
    fn inject_line_substitutes_token() {
        let line = builtins()[0].inject_line("7Kp2xQ9f");
        assert!(line.starts_with("Resolve 7Kp2xQ9f via waggle"));
    }
}
