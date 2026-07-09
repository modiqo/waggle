//! `waggle-tmux` — the seamless switchboard (design/tmux/seamless-mode.md).
//! Choose harnesses once; outcomes move between them as waggle tokens,
//! minted in one gesture, resolved by the act of switching.

use clap::Parser;

mod actions;
mod error;
mod profile;
mod state;
mod tmux;
mod up;
mod waggle;

#[derive(Parser)]
#[command(
    name = "waggle-tmux",
    version,
    about = "The seamless switchboard: harness handoffs as waggle tokens"
)]
enum Cmd {
    /// Choose harnesses and get a wired tmux workspace (MCP + stub + daemon + owned panes).
    Up {
        /// Profile ids (e.g. claude-code codex); empty = detect.
        harnesses: Vec<String>,
    },
    /// Mint any outcome (file, or directory as a --tree) into the pending handoff.
    Mint {
        /// Path inside the workspace; omit with --pick-git to list candidates.
        path: Option<String>,
        /// Destination session this outcome is for.
        #[arg(long)]
        to: Option<String>,
        /// List git-modified files as candidates instead of minting.
        #[arg(long)]
        pick_git: bool,
    },
    /// Switch to a session — and deliver + resolve the pending token there.
    Switch {
        /// Destination session id.
        dest: String,
        /// Deliver a specific token instead of the pending one.
        #[arg(long)]
        token: Option<String>,
        /// Print the resolve line instead of injecting (this switch only).
        #[arg(long)]
        no_inject: bool,
    },
    /// Follow the pending outcome to its destination.
    Next,
    /// Sessions, deliveries, and funnel-derived consumption.
    Status,
    /// Register an existing pane (external — never injected).
    Register {
        /// Local session id.
        id: String,
        /// Harness profile id.
        #[arg(long)]
        profile: String,
        /// tmux pane id, e.g. %3.
        #[arg(long)]
        pane: String,
    },
}

fn main() -> std::process::ExitCode {
    let workspace = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let tmux = tmux::RealTmux;
    let waggle = waggle::BinWaggle;
    let result = match Cmd::parse() {
        Cmd::Up { harnesses } => up::run(&tmux, &workspace, &harnesses),
        Cmd::Mint { path, to, pick_git } => match (path, pick_git) {
            (Some(p), _) => actions::mint(&waggle, &workspace, &p, to.as_deref()).map(|_| ()),
            (None, true) => actions::pick_git(&workspace).map(|files| {
                if files.is_empty() {
                    println!("nothing changed — outcomes come from work");
                } else {
                    println!("candidates (mint one: waggle-tmux mint <path>):");
                    for f in files {
                        println!("  {f}");
                    }
                }
            }),
            (None, false) => Err(error::Error::NotFound(
                "what outcome? waggle-tmux mint <file-or-dir>, or --pick-git to list candidates"
                    .into(),
            )),
        },
        Cmd::Switch {
            dest,
            token,
            no_inject,
        } => actions::switch(
            &tmux,
            &waggle,
            &workspace,
            &dest,
            token.as_deref(),
            no_inject,
        ),
        Cmd::Next => actions::next(&tmux, &waggle, &workspace),
        Cmd::Status => {
            actions::status(&waggle, &workspace);
            Ok(())
        }
        Cmd::Register { id, profile, pane } => {
            actions::register(&tmux, &workspace, &id, &profile, &pane)
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("waggle-tmux: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
