//! `waggle` — the clap projection of the operations catalog.
//!
//! Every subcommand's `about` text is wired to the canonical description in
//! [`waggle_ops`]: the CLI, the MCP tools, the map, and the docs speak with
//! one voice (design doc `09 §2`). The `parity` test module holds this
//! binary to the catalog in both directions — an undeclared subcommand or a
//! drifted description fails the build, and the guard is itself tested to
//! fail (CP-0 gate: the lint proves it lints).
//!
//! Verbs dispatch through the same [`waggle_mcp::Handler`] the MCP wire
//! uses; `waggle serve --stdio` IS the MCP server harnesses spawn.

use clap::{Parser, Subcommand};
use serde_json::json;

mod run;

#[derive(Parser)]
#[command(
    name = "waggle",
    version,
    about = "Attributed, resolvable artifact references for agent handoffs — a ~30-byte token instead of pasted context.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(about = waggle_ops::MINT.description)]
    Mint {
        /// Canonical URI of the artifact (file path, workspace URI, or URL).
        #[arg(long)]
        target: String,
        /// Who is distributing this; defaults to the session identity.
        #[arg(long)]
        sharer: Option<String>,
        /// Where this share lives (e.g. subagent/researcher); defaults to subagent/general.
        #[arg(long)]
        channel: Option<String>,
    },
    #[command(about = waggle_ops::RESOLVE.description)]
    Resolve {
        /// The waggle token to resolve.
        #[arg(long)]
        token: String,
        /// Resolver context (harness metadata, A2A agent card, or explicit JSON); defaults to negotiated.
        #[arg(long)]
        context: Option<String>,
    },
    #[command(about = waggle_ops::RECORD.description)]
    Record {
        /// The waggle token the stage applies to.
        #[arg(long)]
        token: String,
        /// Well-known stage (run, repeat, assess, ...) or a custom kebab-case slug.
        #[arg(long)]
        stage: String,
    },
    #[command(about = waggle_ops::MUTATE.description)]
    Mutate {
        /// The waggle token to change.
        #[arg(long)]
        token: String,
        /// The change: revoke, supersede=<token>, expire=<ts>, or label k=v.
        #[arg(long)]
        change: String,
        /// Required for lifecycle changes: the manifest version this change was decided against (CAS).
        #[arg(long)]
        expected_version: Option<u32>,
    },
    #[command(about = waggle_ops::FUNNEL.description)]
    Funnel {
        /// The waggle token whose funnel to report.
        #[arg(long)]
        token: String,
    },
    #[command(about = waggle_ops::MAP.description)]
    Map {
        /// Token to orient around; omit for the global map.
        #[arg(long)]
        token: Option<String>,
    },
    #[command(about = waggle_ops::SERVE.description)]
    Serve {
        /// Run as a stdio proxy shim instead of the HTTP daemon.
        #[arg(long)]
        stdio: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    // Every verb maps to the same dispatcher the MCP wire uses (09 §2):
    // build the args object the tool expects, call, print the envelope.
    let code = match cli.cmd {
        Cmd::Mint {
            target,
            sharer,
            channel,
        } => run::tool_call(
            "mint",
            strip_nulls(json!({ "target": target, "sharer": sharer, "channel": channel })),
        ),
        Cmd::Resolve { token, context } => {
            let ctx =
                context.map(|c| serde_json::from_str::<serde_json::Value>(&c).unwrap_or(json!(c)));
            run::tool_call(
                "resolve",
                strip_nulls(json!({ "token": token, "context": ctx })),
            )
        }
        Cmd::Record { token, stage } => {
            run::tool_call("record", json!({ "token": token, "stage": stage }))
        }
        Cmd::Mutate {
            token,
            change,
            expected_version,
        } => run::tool_call(
            "mutate",
            strip_nulls(json!({
                "token": token,
                "change": change,
                "expected-version": expected_version,
            })),
        ),
        Cmd::Funnel { token } => run::tool_call("funnel", json!({ "token": token })),
        Cmd::Map { token } => run::tool_call("map", strip_nulls(json!({ "token": token }))),
        Cmd::Serve { stdio } => {
            if stdio {
                run::serve_stdio()
            } else {
                eprintln!(
                    "waggle serve: the HTTP daemon lands later in CP-6; \
                     use --stdio (works with every MCP harness today)."
                );
                2
            }
        }
    };
    std::process::exit(code);
}

/// Drop null members so handlers see 'absent', not 'null'.
fn strip_nulls(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            serde_json::Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
        }
        other => other,
    }
}

/// Catalog↔CLI parity (design doc `09 §2`): returns every disagreement
/// between the built clap tree and [`waggle_ops::OPERATIONS`].
#[cfg(test)]
fn check_parity(cmd: &clap::Command) -> Vec<String> {
    let mut errors = Vec::new();
    let normalize = |s: &str| s.replace('_', "-");

    // Direction 1: every catalog op has a faithful subcommand.
    for op in waggle_ops::OPERATIONS {
        let Some(sub) = cmd.get_subcommands().find(|s| s.get_name() == op.name) else {
            errors.push(format!("catalog op `{}` has no CLI subcommand", op.name));
            continue;
        };
        let about = sub.get_about().map(ToString::to_string).unwrap_or_default();
        if about != op.description {
            errors.push(format!(
                "`{}` about text differs from catalog description",
                op.name
            ));
        }
        for arg_spec in op.args {
            let found = sub
                .get_arguments()
                .find(|a| normalize(a.get_id().as_str()) == arg_spec.name);
            match found {
                None => errors.push(format!("`{}` is missing arg `{}`", op.name, arg_spec.name)),
                Some(a) => {
                    if a.is_required_set() != arg_spec.required {
                        errors.push(format!(
                            "`{}` arg `{}` required={} but catalog says {}",
                            op.name,
                            arg_spec.name,
                            a.is_required_set(),
                            arg_spec.required
                        ));
                    }
                }
            }
        }
        // No undeclared args beyond the catalog (help/version are clap's).
        for a in sub.get_arguments() {
            let id = normalize(a.get_id().as_str());
            if id != "help" && id != "version" && !op.args.iter().any(|s| s.name == id) {
                errors.push(format!("`{}` has undeclared arg `{}`", op.name, id));
            }
        }
    }

    // Direction 2: every subcommand is a catalog op.
    for sub in cmd.get_subcommands() {
        if sub.get_name() != "help" && waggle_ops::find(sub.get_name()).is_none() {
            errors.push(format!(
                "CLI subcommand `{}` is not in the catalog",
                sub.get_name()
            ));
        }
    }
    errors
}

#[cfg(test)]
mod parity {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn ops_inventory_parity() {
        let cmd = Cli::command();
        let errors = check_parity(&cmd);
        assert!(
            errors.is_empty(),
            "catalog/CLI drift:\n{}",
            errors.join("\n")
        );
    }

    #[test]
    fn the_guard_itself_fails_on_a_rogue_subcommand() {
        // CP-0 gate: prove the lint lints before trusting it.
        let cmd = Cli::command().subcommand(clap::Command::new("rogue"));
        let errors = check_parity(&cmd);
        assert!(
            errors.iter().any(|e| e.contains("`rogue`")),
            "parity failed to flag an undeclared subcommand"
        );
    }

    #[test]
    fn the_guard_flags_a_missing_subcommand() {
        // Remove one catalog op's subcommand: parity must notice.
        let cmd = Cli::command();
        let stripped: Vec<_> = cmd
            .get_subcommands()
            .filter(|s| s.get_name() != "mint")
            .cloned()
            .collect();
        let mut rebuilt = clap::Command::new("waggle");
        for s in stripped {
            rebuilt = rebuilt.subcommand(s);
        }
        let errors = check_parity(&rebuilt);
        assert!(errors.iter().any(|e| e.contains("`mint`")));
    }

    #[test]
    fn cli_self_check_is_well_formed() {
        Cli::command().debug_assert();
    }
}
