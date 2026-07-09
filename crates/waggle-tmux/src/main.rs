//! `waggle-tmux` — the seamless switchboard (design/tmux/seamless-mode.md).
//! Choose harnesses once; outcomes move between them as waggle tokens,
//! minted in one gesture, resolved by the act of switching.

use clap::Parser;

mod actions;
mod board;
mod error;
mod profile;
mod state;
mod tmux;
mod up;
mod waggle;
mod watch;

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
    /// Mint outcomes into the pending queue: one path mints directly
    /// (folders as trees); several paths become ONE lineage bundle.
    Mint {
        /// Paths inside the workspace; omit with --pick-git to list candidates.
        paths: Vec<String>,
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
    /// The automation loop: agents mint to tmux/<session>; the watcher
    /// jumps there and delivers. Run it in a spare pane (up does).
    Watch {
        /// One scan instead of the loop (scripting, tests).
        #[arg(long)]
        once: bool,
    },
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
        Cmd::Mint {
            paths,
            to,
            pick_git,
        } => match (paths.len(), pick_git) {
            (1, _) => actions::mint(&waggle, &workspace, &paths[0], to.as_deref()).map(|_| ()),
            (n, _) if n > 1 => {
                actions::mint_bundle(&waggle, &workspace, &paths, to.as_deref()).map(|_| ())
            }
            (_, true) => actions::pick_git(&workspace).map(|files| {
                if files.is_empty() {
                    println!("nothing changed — outcomes come from work");
                } else {
                    println!("candidates (mint: waggle-tmux mint <paths…>):");
                    for f in files {
                        println!("  {f}");
                    }
                }
            }),
            _ => Err(error::Error::NotFound(
                "what outcome? waggle-tmux mint <paths…>, or --pick-git to list candidates".into(),
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
        Cmd::Watch { once } => watch::run(&tmux, &waggle, &workspace, once),
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
