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
mod driver;
mod emit;
mod rng;
mod tier2;
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
        "tier2" => run_tier2(&out),
        "all" => {
            let codes = [run_cost(&out), run_determinism(&out), run_tier2(&out)];
            if codes.iter().all(|c| *c == ExitCode::SUCCESS) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        other => {
            eprintln!(
                "unknown subcommand: {other}\n\
                 usage: waggle-bench [cost-model|determinism|tier2|all] [out-dir]"
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

// Tier-2 behaviour model (design doc 22 §3). Pre-registered here.
const T2_REGIONS: usize = 3;
const T2_TRIALS: usize = 400;
const T2_BLUFFER_RATE: f64 = 0.25;
const T2_P_READ: f64 = 0.98; // genuine per-region thoroughness
const T2_P_BLUFF: f64 = 0.04; // bluffer incidental touch
const T2_BYPASS: f64 = 0.35; // side-door bypass probability
const T2_SEED: u64 = 0x5EA1_C0DE;

fn run_tier2(out: &Path) -> ExitCode {
    ensure_dir(out);
    let r = tier2::run(
        T2_REGIONS,
        T2_TRIALS,
        T2_BLUFFER_RATE,
        T2_P_READ,
        T2_P_BLUFF,
        T2_BYPASS,
        T2_SEED,
    );
    let tex = out.join("tier2.tex");
    if let Err(e) = emit::write_tier2(&tex, &r) {
        eprintln!("write {}: {e}", tex.display());
        return ExitCode::FAILURE;
    }
    let dat = out.join("tier2_roc.dat");
    if let Err(e) = emit::write_roc_dat(&dat, &r.roc) {
        eprintln!("write {}: {e}", dat.display());
        return ExitCode::FAILURE;
    }
    println!(
        "tier2: sealed(P={:.2} R={:.2} F1={:.2}) side-door(P={:.2} R={:.2} F1={:.2}) · FNR {:.1}%→{:.1}% · bluffers caught {:.1}% · AUC={:.3}",
        r.sealed.precision(),
        r.sealed.recall(),
        r.sealed.f1(),
        r.side_door.precision(),
        r.side_door.recall(),
        r.side_door.f1(),
        r.sealed.false_negative_rate() * 100.0,
        r.side_door.false_negative_rate() * 100.0,
        r.sealed.bluffer_detection() * 100.0,
        r.auc,
    );
    println!("  wrote {} and {}", tex.display(), dat.display());
    ExitCode::SUCCESS
}
