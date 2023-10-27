// ─────────────────────────────────────────────────────────────────────────────
//  Karpovian Rust — Texel Parameter Tuner
//
//  Minimises mean-squared error between sigmoid(eval) and game result (WDL)
//  using coordinate descent over the integer fields of EvalParams.
//
//  Data format — one position per line in tuning_data.txt:
//    <FEN>\t<result>
//  where result is  1.0 (White wins) | 0.5 (draw) | 0.0 (Black wins)
//  Tab-separated so the FEN (which contains spaces) is unambiguous.
//
//  Usage:
//    cargo run --release --bin tuner -- --data tuning_data.txt
//    cargo run --release --bin tuner -- --data tuning_data.txt --passes 150
//    cargo run --release --bin tuner -- --data tuning_data.txt --delta 1 --passes 50
//
//  Workflow:
//    1. python generate_data.py --games 1000   # ~30 k positions, ~5 min
//    2. cargo run --release --bin tuner -- --data tuning_data.txt
//    3. Copy the printed EvalParams into eval.rs::karpov_style()
// ─────────────────────────────────────────────────────────────────────────────

#![allow(dead_code)]

use std::str::FromStr;
use std::time::Instant;

use chess::{Board, Color};

// Pull in the three modules needed from the main engine.
// They share no state with the search — pure evaluation logic.
#[path = "tables.rs"]      mod tables;
#[path = "time_model.rs"]  mod time_model;
#[path = "personality.rs"] mod personality;
// Stub for nnue — the tuner uses HCE directly, not NNUE
mod nnue { pub fn nnue_eval(_: &chess::Board) -> Option<i32> { None } }
#[path = "eval.rs"]        mod eval;

use eval::{evaluate_fast, EvalParams};

// ── Sigmoid ───────────────────────────────────────────────────────────────────
// Standard Texel formula: sigmoid(score, K) = 1 / (1 + 10^(-K·score/400))
// score is in centipawns from White's perspective.

#[inline]
fn sigmoid(score: f64, k: f64) -> f64 {
    1.0 / (1.0 + 10_f64.powf(-k * score / 400.0))
}

// ── Error computation ─────────────────────────────────────────────────────────
// Mean squared error over the whole corpus.

fn mse(positions: &[(Board, f64)], ep: &EvalParams, k: f64) -> f64 {
    let mut sum = 0.0f64;
    for (board, result) in positions {
        let stm_score  = evaluate_fast(board, ep) as f64;
        // evaluate_fast returns score from side-to-move perspective; convert to White
        let white_score = if board.side_to_move() == Color::White {
            stm_score
        } else {
            -stm_score
        };
        let pred = sigmoid(white_score, k);
        let diff = result - pred;
        sum += diff * diff;
    }
    sum / positions.len() as f64
}

// ── Find optimal K ────────────────────────────────────────────────────────────
// Ternary search over [0.1, 3.0].  Runs ~60 MSE evaluations — cheap.

fn find_k(positions: &[(Board, f64)], ep: &EvalParams) -> f64 {
    let (mut lo, mut hi) = (0.1_f64, 3.0_f64);
    for _ in 0..60 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        if mse(positions, ep, m1) < mse(positions, ep, m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    (lo + hi) / 2.0
}

// ── Argument parsing (no extra crates) ───────────────────────────────────────

fn arg<T: FromStr>(args: &[String], flag: &str, default: T) -> T {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if flag(&args, "--help") || flag(&args, "-h") {
        println!("Usage: tuner [--data FILE] [--passes N] [--delta D]");
        println!("  --data   tuning_data.txt (FEN<TAB>result, one per line)");
        println!("  --passes 100             (coordinate-descent passes)");
        println!("  --delta  1               (cp step per trial)");
        return;
    }

    let data_file: String = arg(&args, "--data",   "tuning_data.txt".to_string());
    let max_passes: usize = arg(&args, "--passes", 150usize);
    let delta:      i32   = arg(&args, "--delta",  1i32).max(1);

    // ── Load positions ────────────────────────────────────────────────────────

    println!("Loading positions from {} …", data_file);
    let content = match std::fs::read_to_string(&data_file) {
        Ok(c)  => c,
        Err(e) => {
            eprintln!("ERROR: cannot read {}: {}", data_file, e);
            eprintln!("Generate data first:  python generate_data.py --games 1000");
            std::process::exit(1);
        }
    };

    let mut positions: Vec<(Board, f64)> = Vec::new();
    let mut skipped = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        // Split on last tab (FEN contains spaces; result is after the tab)
        let (fen_str, result_str) = if let Some(p) = line.rfind('\t') {
            (&line[..p], &line[p+1..])
        } else {
            // Fallback: last space-delimited token is the result
            match line.rfind(' ') {
                Some(p) => (&line[..p], &line[p+1..]),
                None    => { skipped += 1; continue; }
            }
        };

        let result = match result_str.trim() {
            "1.0" | "1"   | "1-0"       => 1.0_f64,
            "0.5"         | "1/2-1/2"   => 0.5_f64,
            "0.0" | "0"   | "0-1"       => 0.0_f64,
            _                           => { skipped += 1; continue; }
        };

        match Board::from_str(fen_str.trim()) {
            Ok(board) => positions.push((board, result)),
            Err(_)    => { skipped += 1; }
        }
    }

    if skipped > 0 {
        println!("  (skipped {} malformed lines)", skipped);
    }
    println!("Loaded {} positions\n", positions.len());

    if positions.is_empty() {
        eprintln!("No valid positions found.  Expected format:  FEN<TAB>result");
        std::process::exit(1);
    }

    // ── Initial state ─────────────────────────────────────────────────────────

    let mut ep = EvalParams::karpov_style();

    print!("Finding optimal K …");
    let t = Instant::now();
    let k = find_k(&positions, &ep);
    let initial_err = mse(&positions, &ep, k);
    println!("  K = {:.4}  ({:.2}s)", k, t.elapsed().as_secs_f64());
    println!("Initial MSE = {:.8}\n", initial_err);

    let names = EvalParams::param_names();
    let n     = names.len();

    // ── Coordinate descent ────────────────────────────────────────────────────
    // For each parameter in turn, trial ±delta and keep whichever reduces error.
    // Repeat until a full pass produces zero improvements.

    let mut best_err = initial_err;
    let total_start  = Instant::now();

    for pass in 1..=max_passes {
        let mut improved = 0usize;
        let pass_start   = Instant::now();

        for pi in 0..n {
            let orig = ep.to_tunable()[pi];

            // Try + delta
            let mut params = ep.to_tunable();
            params[pi] = orig + delta;
            let mut candidate = ep.clone();
            candidate.set_from_tunable(&params);
            let err_plus = mse(&positions, &candidate, k);

            if err_plus < best_err {
                ep       = candidate;
                best_err = err_plus;
                improved += 1;
                continue;   // no need to try minus
            }

            // Try - delta
            let mut params = ep.to_tunable();
            params[pi] = orig - delta;
            let mut candidate = ep.clone();
            candidate.set_from_tunable(&params);
            let err_minus = mse(&positions, &candidate, k);

            if err_minus < best_err {
                ep       = candidate;
                best_err = err_minus;
                improved += 1;
            }
        }

        println!(
            "Pass {:3}  MSE = {:.8}  improved = {:2}  ({:.1}s)",
            pass, best_err, improved,
            pass_start.elapsed().as_secs_f64(),
        );

        if improved == 0 { break; }
    }

    let total_s = total_start.elapsed().as_secs_f64();
    let gain_pct = (initial_err - best_err) / initial_err * 100.0;
    println!("\nTotal tuning time: {:.1}s", total_s);
    println!("MSE improvement:  {:.2}%  ({:.8} → {:.8})", gain_pct, initial_err, best_err);

    // ── Print tuned parameters as Rust source ─────────────────────────────────

    let orig  = EvalParams::karpov_style().to_tunable();
    let tuned = ep.to_tunable();

    println!("\n\n// ═══════════════════════════════════════════════════════════════");
    println!("// Tuned EvalParams::karpov_style()  —  paste into eval.rs");
    println!("// (parameters unchanged from baseline are shown as comments)");
    println!("// ═══════════════════════════════════════════════════════════════");

    // Style multipliers are unchanged — print them first
    println!("EvalParams {{");
    println!("    material_weight:     1.0,");
    println!("    king_attack_weight:  1.0,");
    println!("    pawn_storm_weight:   1.0,");
    println!("    sac_bonus:           0,");
    println!("    sac_threshold:       150,");
    println!("    sac_uncastled_bonus: 60,");
    println!("    tropism_scale:       0.4,");

    for (i, name) in names.iter().enumerate() {
        let o = orig[i];
        let t = tuned[i];
        if o == t {
            println!("    {:<28}: {}, // unchanged", name, t);
        } else {
            println!("    {:<28}: {}, // was {}", name, t, o);
        }
    }
    println!("}}");
}
