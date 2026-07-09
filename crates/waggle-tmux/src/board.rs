//! The live board: what the watcher pane SHOWS. Every tick renders the
//! workspace's outcome lineage as a tree — token, target, destination,
//! stage counts, consumption, age — so the humans see the rally the
//! way the store sees it. Pure rendering over plain rows: unit-tested
//! without tmux, a store, or a clock.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::state::State;
use crate::watch::CHANNEL_PREFIX;

/// One outcome, flattened for rendering.
#[derive(Debug, Clone)]
pub struct Row {
    /// The token.
    pub token: String,
    /// Basename of the target (what a human recognizes).
    pub name: String,
    /// Destination session (from the tmux/<dest> channel).
    pub dest: String,
    /// Tree depth (0 = root).
    pub depth: usize,
    /// Last-in-siblings marker (└ vs ├).
    pub last: bool,
    /// resolve / read / run counts from the funnel.
    pub stages: (u64, u64, u64),
    /// Delivered and then resolved past the delivery baseline.
    pub consumed: bool,
    /// This token is the pending (undelivered) handoff.
    pub pending: bool,
    /// Seconds since mint.
    pub age_secs: u64,
}

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// Flatten the world into render rows: tmux-channel outcomes, grouped
/// under their lineage roots, newest roots last (nearest the prompt).
#[must_use]
pub fn build_rows(world: &waggle_core::WorldState, state: &State, now_ms: u64) -> Vec<Row> {
    let ours: BTreeMap<&waggle_core::Token, &waggle_core::AttributionManifest> = world
        .manifests
        .iter()
        .filter(|(_, m)| m.channel.as_str().starts_with(CHANNEL_PREFIX))
        .collect();
    let baselines: BTreeMap<&str, u64> = state
        .sessions
        .values()
        .filter_map(|s| s.last_delivery.as_ref())
        .map(|(t, b)| (t.as_str(), *b))
        .collect();
    let pending = state.pending.as_ref().map(|(t, _, _)| t.clone());

    let mut roots: Vec<&waggle_core::Token> = ours
        .iter()
        .filter(|(_, m)| m.parent.is_none_or(|p| !ours.contains_key(&p)))
        .map(|(t, _)| *t)
        .collect();
    roots.sort_by_key(|t| ours[t].minted_at.as_unix_ms());

    let mut rows = Vec::new();
    for root in roots {
        push_row(
            root,
            0,
            true,
            &ours,
            world,
            &baselines,
            pending.as_deref(),
            now_ms,
            &mut rows,
        );
    }
    rows
}

#[allow(clippy::too_many_arguments)] // a recursive flattener's working set
fn push_row(
    token: &waggle_core::Token,
    depth: usize,
    last: bool,
    ours: &BTreeMap<&waggle_core::Token, &waggle_core::AttributionManifest>,
    world: &waggle_core::WorldState,
    baselines: &BTreeMap<&str, u64>,
    pending: Option<&str>,
    now_ms: u64,
    rows: &mut Vec<Row>,
) {
    let Some(manifest) = ours.get(token) else {
        return;
    };
    let funnel = world.funnels.get(token);
    let count = |stage: &str| {
        funnel
            .and_then(|f| f.iter().find(|(s, _)| s.as_str() == stage))
            .map_or(0, |(_, n)| *n)
    };
    let resolves = count("resolve");
    let consumed = baselines
        .get(token.as_str())
        .is_some_and(|baseline| resolves > *baseline);
    rows.push(Row {
        token: token.as_str().to_owned(),
        name: {
            let base = manifest
                .target
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or_default();
            // One row per outcome — long names truncate, never wrap.
            base.chars().take(18).collect::<String>()
                + if base.chars().count() > 18 { "…" } else { "" }
        },
        dest: manifest
            .channel
            .as_str()
            .strip_prefix(CHANNEL_PREFIX)
            .unwrap_or_default()
            .to_owned(),
        depth,
        last,
        stages: (resolves, count("read"), count("run")),
        consumed,
        pending: pending == Some(token.as_str()),
        age_secs: now_ms.saturating_sub(manifest.minted_at.as_unix_ms()) / 1000,
    });
    let children = world.lineage.get(token).cloned().unwrap_or_default();
    let known: Vec<_> = children.iter().filter(|c| ours.contains_key(c)).collect();
    for (i, child) in known.iter().enumerate() {
        push_row(
            child,
            depth + 1,
            i + 1 == known.len(),
            ours,
            world,
            baselines,
            pending,
            now_ms,
            rows,
        );
    }
}

fn age(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        _ => format!("{}h", secs / 3600),
    }
}

/// Render the board: header, the tree (tail-cropped to fit), sessions.
#[must_use]
pub fn render(rows: &[Row], state: &State, height: usize) -> String {
    let mut out = String::new();
    let pending_note = state.pending.as_ref().map_or_else(
        || "none".to_owned(),
        |(t, _, to)| {
            format!(
                "{t}{}",
                to.as_ref().map(|d| format!(" -> {d}")).unwrap_or_default()
            )
        },
    );
    let _ = writeln!(
        out,
        "{BOLD}waggle{RESET} {DIM}|{RESET} {} outcome(s) {DIM}|{RESET} pending: {YELLOW}{pending_note}{RESET}",
        rows.len()
    );

    let budget = height.saturating_sub(2).max(1);
    let start = rows.len().saturating_sub(budget);
    if start > 0 {
        let _ = writeln!(out, "{DIM}  … {start} earlier{RESET}");
    }
    for row in &rows[start..] {
        let glyph = if row.depth == 0 {
            String::new()
        } else {
            format!(
                "{}{} ",
                "  ".repeat(row.depth - 1),
                if row.last { "└─" } else { "├─" }
            )
        };
        let (r, rd, run) = row.stages;
        let mark = if row.pending {
            format!("{YELLOW}pending{RESET}")
        } else if row.consumed {
            format!("{GREEN}consumed{RESET}")
        } else if r > 0 {
            format!("{GREEN}r{r}{RESET} rd{rd} run{run}")
        } else {
            format!("{DIM}unread{RESET}")
        };
        let _ = writeln!(
            out,
            "{glyph}{BOLD}{}{RESET} {} {DIM}->{RESET}{} {mark} {DIM}{}{RESET}",
            row.token,
            row.name,
            row.dest,
            age(row.age_secs)
        );
    }
    let sessions: Vec<String> = state
        .sessions
        .iter()
        .filter(|(id, _)| !id.starts_with('_'))
        .map(|(id, s)| format!("{id} {}", s.pane))
        .collect();
    let _ = write!(out, "{DIM}sessions: {}{RESET}", sessions.join(" | "));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(token: &str, depth: usize, consumed: bool) -> Row {
        Row {
            token: token.into(),
            name: "plan.md".into(),
            dest: "codex".into(),
            depth,
            last: true,
            stages: (1, 2, 1),
            consumed,
            pending: false,
            age_secs: 65,
        }
    }

    #[test]
    fn render_shows_tree_consumption_and_age() {
        let rows = vec![row("aaaa1111", 0, true), row("bbbb2222", 1, true)];
        let out = render(&rows, &State::default(), 12);
        assert!(out.contains("aaaa1111"));
        assert!(out.contains("└─"), "children draw as tree branches: {out}");
        assert!(out.contains("consumed"));
        assert!(out.contains("1m"), "ages humanize: {out}");
    }

    #[test]
    fn render_crops_to_height_and_names_the_cropped() {
        let rows: Vec<Row> = (0..20)
            .map(|i| row(&format!("tok{i:05}"), 0, false))
            .collect();
        let out = render(&rows, &State::default(), 8);
        assert!(out.contains("earlier"), "cropped rows are counted: {out}");
        assert!(out.contains("tok00019"), "newest stays visible");
        assert!(!out.contains("tok00000"), "oldest crops first");
    }
}
