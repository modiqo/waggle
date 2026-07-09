//! `waggle-tmux watch` — the automation loop (seamless-mode Phase 4).
//!
//! Agents already hold the waggle MCP tools. When one finishes, it
//! mints its own outcome ADDRESSED BY CHANNEL — `tmux/<destination>` —
//! and this watcher, polling the shared store read-only, performs the
//! jump: focus the destination pane, deliver the resolve instruction.
//! The humans watch the match; nobody plays courier.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;

use waggle_store::ReadStore as _;

use crate::error::{Error, Result};
use crate::tmux::TmuxBackend;
use crate::waggle::WaggleClient;
use crate::{actions, state};

/// The channel prefix that addresses a switchboard destination.
pub const CHANNEL_PREFIX: &str = "tmux/";

/// Where the shared store lives (the daemon's default, or the env).
fn store_path() -> String {
    std::env::var("WAGGLE_STORE").unwrap_or_else(|_| {
        format!(
            "{}/.waggle/waggle.db",
            std::env::var("HOME").unwrap_or_else(|_| ".".into())
        )
    })
}

/// One scan: any UNSEEN outcome minted on a `tmux/<dest>` channel gets
/// delivered. Returns how many were delivered.
pub fn tick<T: TmuxBackend, W: WaggleClient>(
    tmux: &T,
    waggle: &W,
    workspace: &Path,
    seen: &mut BTreeSet<String>,
) -> Result<u32> {
    let store = waggle_store_sqlite::SqliteStore::open(Path::new(&store_path()))
        .map_err(|e| Error::Waggle(format!("store: {e}")))?;
    let records =
        pollster::block_on(store.scan_all()).map_err(|e| Error::Waggle(format!("scan: {e}")))?;
    let mut delivered = 0;
    for record in records {
        let waggle_core::LogRecord::Minted { manifest } = record else {
            continue;
        };
        let Some(dest) = manifest.channel.as_str().strip_prefix(CHANNEL_PREFIX) else {
            continue;
        };
        let token = manifest.token.as_str().to_owned();
        if !seen.insert(token.clone()) {
            continue;
        }
        let st = state::load(workspace);
        if !st.sessions.contains_key(dest) {
            // Not ours (another workspace's outcome, or 'outcome' itself).
            continue;
        }
        println!("watch: {token} → {dest}");
        match actions::switch(tmux, waggle, workspace, dest, Some(&token), false) {
            Ok(()) => delivered += 1,
            Err(e) => eprintln!("watch: {token} → {dest}: {e}"),
        }
    }
    Ok(delivered)
}

/// What a watch process does: deliver, render, or both. Delivery MUST
/// be a single process per workspace (duplicates would double-deliver);
/// boards are pure readers and replicate freely — one strip per window.
#[derive(Clone, Copy)]
pub struct Mode {
    /// Perform deliveries (the single watcher).
    pub deliver: bool,
    /// Render the board each tick.
    pub board: bool,
}

/// The loop (or one pass with `once`). Priming marks everything already
/// in the store as seen — the watcher delivers the FUTURE, not history.
pub fn run<T: TmuxBackend, W: WaggleClient>(
    tmux: &T,
    waggle: &W,
    workspace: &Path,
    once: bool,
    mode: Mode,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    if mode.deliver && !once {
        prime(&mut seen)?;
    }
    loop {
        if mode.deliver {
            tick(tmux, waggle, workspace, &mut seen)?;
            // Exit backstop: hooks catch most deaths; the deliverer
            // sweeps for any they missed.
            let _ = crate::actions::reap(tmux, workspace);
        }
        if once {
            return Ok(());
        }
        if mode.board {
            draw_board(tmux, workspace);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// Redraw the pane as the live board (best effort — a broken store
/// read leaves the last frame standing rather than crashing the loop).
fn draw_board<T: TmuxBackend>(tmux: &T, workspace: &Path) {
    let Ok(store) = waggle_store_sqlite::SqliteStore::open(Path::new(&store_path())) else {
        return;
    };
    let Ok(records) = pollster::block_on(store.scan_all()) else {
        return;
    };
    let world = waggle_core::reconstruct(records);
    let st = state::load(workspace);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0));
    let rows = crate::board::build_rows(&world, &st, now_ms);
    // Live pane height — the board adapts the moment you resize it.
    let height = std::env::var("TMUX_PANE")
        .ok()
        .and_then(|pane| {
            tmux.run(&["display-message", "-p", "-t", &pane, "#{pane_height}"])
                .ok()
        })
        .and_then(|h| h.trim().parse().ok())
        .unwrap_or(10);
    // Clear + home, then the frame.
    print!("\x1b[2J\x1b[H{}", crate::board::render(&rows, &st, height));
    let _ = std::io::stdout().flush();
}

fn prime(seen: &mut BTreeSet<String>) -> Result<()> {
    let store = waggle_store_sqlite::SqliteStore::open(Path::new(&store_path()))
        .map_err(|e| Error::Waggle(format!("store: {e}")))?;
    let records =
        pollster::block_on(store.scan_all()).map_err(|e| Error::Waggle(format!("scan: {e}")))?;
    for record in records {
        if let waggle_core::LogRecord::Minted { manifest } = record {
            seen.insert(manifest.token.as_str().to_owned());
        }
    }
    Ok(())
}
