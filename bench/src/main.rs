//! `waggle-bench` — the reproducible benchmark harness (design doc `22`).
//!
//! Tier 1 is deterministic and needs no model calls: it prices the handoff
//! cost model (§2.1) and checks reconstruction determinism (§2.2), emitting
//! the paper's data files and tables under `paper/generated/`. Tiers 2 and
//! 3 (receipt reliability under seal; the cost-vs-quality frontier) are
//! model-driven and specified in doc 22 §3–4; their harness plugs into the
//! same accounting via an `AgentDriver` seam and runs when keys and public
//! datasets are supplied.
//!
//! Usage: `waggle-bench [cost-model|determinism|all] [out-dir]`
//! (default out-dir: `paper/generated`).

// A tooling binary: these numeric conversions are intentional and exact
// enough for token accounting and seeded shuffling.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

mod cost_model;
mod determinism;
mod emit;
mod tokenizer;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cost_model::{costs, size_sweep, Scenario};
use tokenizer::{CharRatio, Tokenizer};

const SIZES_KIB: &[usize] = &[4, 16, 40, 160, 640];
const CACHE_DISCOUNT: f64 = 0.1;
const PROJ_BYTES: usize = 2 * 1024;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("all", String::as_str);
    let out = PathBuf::from(
        args.get(2)
            .cloned()
            .unwrap_or_else(|| "paper/generated".to_owned()),
    );

    match cmd {
        "cost-model" => run_cost(&out),
        "determinism" => run_determinism(&out),
        "all" => {
            let a = run_cost(&out);
            let b = run_determinism(&out);
            if a == ExitCode::SUCCESS && b == ExitCode::SUCCESS {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        other => {
            eprintln!(
                "unknown subcommand: {other}\n\
                 usage: waggle-bench [cost-model|determinism|all] [out-dir]"
            );
            ExitCode::FAILURE
        }
    }
}

fn ensure_dir(out: &Path) {
    if let Err(e) = fs::create_dir_all(out) {
        eprintln!("could not create {}: {e}", out.display());
    }
}

fn run_cost(out: &Path) -> ExitCode {
    ensure_dir(out);
    let tok = CharRatio::english();

    // The crossover figure: vary artifact size at the paper's fanout/turns.
    let rows = size_sweep(SIZES_KIB, 5, 5, 1, PROJ_BYTES, &tok, CACHE_DISCOUNT);
    let dat = out.join("cost_sweep.dat");
    if let Err(e) = emit::write_cost_dat(&dat, &rows) {
        eprintln!("write {}: {e}", dat.display());
        return ExitCode::FAILURE;
    }

    // Representative table, including the paper's exact cell.
    let scenarios = [
        (
            "single, one turn",
            Scenario {
                s_bytes: 4 * 1024,
                holders: 1,
                turns: 1,
                revisions: 0,
                proj_bytes: PROJ_BYTES,
            },
        ),
        (
            "fan-out, few turns",
            Scenario {
                s_bytes: 40 * 1024,
                holders: 3,
                turns: 3,
                revisions: 0,
                proj_bytes: PROJ_BYTES,
            },
        ),
        (
            "paper cell",
            Scenario {
                s_bytes: 40 * 1024,
                holders: 5,
                turns: 5,
                revisions: 1,
                proj_bytes: PROJ_BYTES,
            },
        ),
        (
            "deep delegation",
            Scenario {
                s_bytes: 160 * 1024,
                holders: 10,
                turns: 10,
                revisions: 3,
                proj_bytes: PROJ_BYTES,
            },
        ),
    ];
    let table_rows: Vec<_> = scenarios
        .iter()
        .map(|(label, sc)| ((*label).to_owned(), *sc, costs(sc, &tok, CACHE_DISCOUNT)))
        .collect();
    let tex = out.join("cost_table.tex");
    if let Err(e) = emit::write_cost_table(&tex, &table_rows, tok.label()) {
        eprintln!("write {}: {e}", tex.display());
        return ExitCode::FAILURE;
    }

    let paper = &table_rows[2].2;
    println!(
        "cost-model: tokenizer={} · paper cell (40KB,H5,T5,R1): copy(cached)={:.0} tok, waggle={:.0} tok, ratio={:.1}x",
        tok.label(),
        paper.copy_cached,
        paper.waggle,
        paper.ratio_vs_cached(),
    );
    println!("  wrote {} and {}", dat.display(), tex.display());
    ExitCode::SUCCESS
}

fn run_determinism(out: &Path) -> ExitCode {
    ensure_dir(out);
    // A log large enough to exercise the fold; every ordering must agree.
    let report = determinism::run(64, 6, 200, 0xB0BA_CAFE);
    let tex = out.join("determinism.tex");
    if let Err(e) = emit::write_determinism(&tex, &report) {
        eprintln!("write {}: {e}", tex.display());
        return ExitCode::FAILURE;
    }
    println!(
        "determinism: {} tokens, {} events, {} orderings → {} (fold {}µs)",
        report.tokens,
        report.events,
        report.permutations,
        if report.all_identical {
            "byte-identical"
        } else {
            "DIVERGED"
        },
        report.fold_micros,
    );
    println!("  wrote {}", tex.display());
    if report.all_identical {
        ExitCode::SUCCESS
    } else {
        // A real gate: non-determinism fails the benchmark.
        ExitCode::FAILURE
    }
}
