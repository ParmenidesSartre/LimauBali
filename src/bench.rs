// ─────────────────────────────────────────────────────────────────────────────
//  Karpovian Rust — Built-in Benchmark
//
//  UCI command:  bench [depth]   (default depth = 12)
//
//  Runs 20 fixed positions at a fixed depth (no time limit).
//  Reports:
//    • Total node count  — should stay stable across non-search changes
//    • NPS               — measures raw search speed
//    • Signature         — XOR of all best-move bit patterns; changes if
//                          move selection changes, catches eval regressions
//
//  Typical use:
//    Before a change:  bench 12  → write down signature + NPS
//    After  a change:  bench 12  → compare
//      Same signature, higher NPS  → pure speed win
//      Different signature         → behaviour changed (good or bad)
// ─────────────────────────────────────────────────────────────────────────────

use std::io::Write;
use std::str::FromStr;
use std::time::Instant;

use chess::Board;

use crate::search::{find_best_move, SearchState};

// ── 20 benchmark positions ────────────────────────────────────────────────────
// Mix of opening, middlegame, and endgame — chosen to stress all eval terms.

const BENCH_FENS: &[&str] = &[
    // Opening / early middlegame
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r1bqkbnr/pp1ppppp/2n5/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
    "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 4 5",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    // Rich middlegame
    "r2q1rk1/pp2ppbp/2n3p1/2pp4/3P4/2NBPN2/PP3PPP/R2Q1RK1 w - - 0 10",
    "r1bq1rk1/2p2ppp/p1pb1n2/1p2p3/4P3/1BP2N2/PP1P1PPP/R1BQR1K1 w - - 0 12",
    "r3r1k1/pp3ppp/1qn2n2/3pp3/1bB1P3/2N2N2/PP1Q1PPP/R3R1K1 w - - 0 14",
    "2rq1rk1/pp3ppp/2n1pn2/3p4/1bpP4/2N1PN2/PPQ2PPP/R1BR2K1 b - - 0 12",
    "r4rk1/1pp1qppp/p1np1n2/2b1p3/2B1P3/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "rnbq1rk1/pp2ppbp/2pp1np1/8/3PPP2/2N2N2/PPP1B1PP/R1BQK2R b KQ - 0 8",
    // Tactical / complex
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp4PP/R2Q1RK1 w kq - 0 1",
    "r1bqkb1r/ppppnppp/8/3Np3/4P3/8/PPP2PPP/RNBQKB1R w KQkq - 0 5",
    "r1bq1rk1/pp3ppp/2nbpn2/3p4/3P4/2NBPN2/PP3PPP/R1BQ1RK1 w - - 0 9",
    // Endgame
    "6k1/5ppp/p3p3/1p6/1P6/P3PP2/6PP/6K1 w - - 0 30",
    "8/pp3pk1/2p2p1p/4r3/4P3/2P3R1/PP4KP/8 w - - 0 28",
    "8/8/4k3/4p3/4P3/4K3/8/8 w - - 0 50",
    "8/3k4/3p4/8/3P4/3K4/8/8 w - - 0 1",
    "8/8/1k6/8/8/1K6/1P6/8 w - - 0 1",
    "4k3/1R6/8/8/8/8/8/4K3 w - - 0 1",
];

pub struct BenchResult {
    pub depth:       i32,
    pub positions:   usize,
    pub total_nodes: u64,
    pub elapsed_ms:  u64,
    pub nps:         u64,
    /// XOR of (from_sq | dest_sq<<6) for every best move found.
    /// Stays constant while search behaviour is unchanged.
    pub signature:   u64,
    // Profiling counters
    pub qnodes:      u64,
    pub eval_calls:  u64,
    pub tt_hits:     u64,
}

pub fn run_bench(state: &mut SearchState, depth: i32) -> BenchResult {
    let was_silent  = state.silent;
    state.silent    = true;   // suppress per-depth "info" lines

    let mut total_nodes = 0u64;
    let mut total_qnodes    = 0u64;
    let mut total_eval      = 0u64;
    let mut total_tt_hits   = 0u64;
    let mut signature   = 0u64;
    let wall            = Instant::now();

    for (i, &fen) in BENCH_FENS.iter().enumerate() {
        let board = match Board::from_str(fen) {
            Ok(b)  => b,
            Err(_) => { eprintln!("bench: bad FEN #{}", i + 1); continue; }
        };

        // Clear per-game state but keep TT across positions (realistic)
        state.hash_history.clear();
        state.hard_deadline = None;
        state.soft_deadline = None;
        state.personality.reset_game();

        let result = find_best_move(&board, depth, 0, state);
        total_nodes    += state.nodes;
        total_qnodes   += state.qnodes;
        total_eval     += state.eval_calls;
        total_tt_hits  += state.tt_hits;

        if let Some(mv) = result.best_move {
            signature ^= mv.get_source().to_index() as u64
                      | ((mv.get_dest().to_index() as u64) << 6);
        }

        // Progress indicator on stderr so it doesn't pollute UCI stdout
        eprint!("\r  Bench: {}/{} positions…", i + 1, BENCH_FENS.len());
        std::io::stderr().flush().ok();
    }
    eprintln!(); // newline after progress

    state.silent = was_silent;

    let elapsed_ms = wall.elapsed().as_millis() as u64;
    let nps        = if elapsed_ms > 0 { total_nodes * 1000 / elapsed_ms } else { total_nodes };

    BenchResult {
        depth,
        positions:   BENCH_FENS.len(),
        total_nodes,
        elapsed_ms,
        nps,
        signature,
        qnodes:      total_qnodes,
        eval_calls:  total_eval,
        tt_hits:     total_tt_hits,
    }
}

pub fn print_bench_result(r: &BenchResult) {
    let abs_nodes    = r.total_nodes;
    let negamax_nodes = abs_nodes.saturating_sub(r.qnodes).max(1);
    let q_pct        = r.qnodes    * 100 / abs_nodes.max(1);
    let eval_pct     = r.eval_calls * 100 / abs_nodes.max(1);
    let tt_pct       = r.tt_hits   * 100 / abs_nodes.max(1);
    let tt_neg_pct   = r.tt_hits   * 100 / negamax_nodes;
    println!("info string === Bench (depth {}) ===", r.depth);
    println!("info string Positions   : {}", r.positions);
    println!("info string Nodes total : {}  ({}% qsearch)", abs_nodes, q_pct);
    println!("info string TT cuts     : {}  ({}% all / {}% negamax)", r.tt_hits, tt_pct, tt_neg_pct);
    println!("info string Eval calls  : {}  ({}% of nodes)", r.eval_calls, eval_pct);
    println!("info string Time        : {:.1}s", r.elapsed_ms as f64 / 1000.0);
    println!("info string NPS         : {}", r.nps);
    println!("info string Signature   : {:#018x}", r.signature);
}
