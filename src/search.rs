// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — Search
//
//  Negamax alpha-beta with:
//    Iterative deepening + aspiration windows (±50cp, depth ≥ 4)
//    Quiescence: captures + queen promotions, correct delta pruning
//    Check extension (+1 ply on moves that give check)
//    Null-move pruning (R = 2, skip in check / endgame / after null)
//    Futility pruning (depth 1-2: skip quiet non-checking moves)
//    Late Move Reduction (log(d)×log(m)/2, precomputed table)
//    Principal Variation Search (null-window probe on non-PV moves)
//    Killer heuristic (2 per ply)
//    History heuristic (capped at 16 384)
//    Transposition table
//
//  Personality affects: eval (Tal bonus + weights), contempt, time mgmt, book.
//  Search itself is style-neutral — no ordering hacks based on style.
// ─────────────────────────────────────────────────────────────────────────────

use chess::{BitBoard, Board, BoardStatus, ChessMove, Color, MoveGen, Piece};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::book::petrosian_book;
use crate::eval::{evaluate_fast, EvalParams};
use crate::personality::{Personality, Style};
use crate::san::pv_to_book;
use crate::tables::{CONTEMPT, INFINITY, MATE_SCORE, PIECE_VALUES};
use crate::tt::{TranspositionTable, EXACT, LOWER_BOUND, UPPER_BOUND};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_PLY:         usize = 64;
const DELTA_MARGIN:    i32   = 300;  // quiescence delta pruning (Python uses 300)
const NULL_R:          i32   = 2;    // null-move reduction
const NULL_MIN_DEPTH:  i32   = 3;
const LMR_MIN_DEPTH:   i32   = 3;
const LMR_MIN_MOVES:   usize = 3;
const FUTILITY_PER_D:  i32   = 200;  // futility margin per depth
const CONTEMPT_TAL:       i32 = 60;   // avoids draws — always plays for the win
const CONTEMPT_PETROSIAN: i32 = -50;  // draws are desirable — 40-50% draw rate
const CONTEMPT_FISCHER:   i32 = 10;   // slightly avoids draws — Fischer played to win
const HIST_MAX:        i32   = 16_384;
const ASP_WINDOW:      i32   = 50;   // aspiration half-window (±50cp)

// ── LMR table ─────────────────────────────────────────────────────────────────
// reduction[depth][move_index] = floor(ln(depth) * ln(move_index) / 2)
// Computed once at startup; avoids f64 in hot path.

struct LmrTable([[i32; 64]; 32]);

impl LmrTable {
    fn build() -> Self {
        let mut t = [[0i32; 64]; 32];
        for d in 1usize..32 {
            for m in 1usize..64 {
                t[d][m] = ((d as f64).ln() * (m as f64).ln() / 2.0) as i32;
            }
        }
        LmrTable(t)
    }
    #[inline]
    fn get(&self, depth: i32, moves: usize) -> i32 {
        self.0[(depth as usize).min(31)][moves.min(63)].max(0)
    }
}

// ── Search State ──────────────────────────────────────────────────────────────

pub struct SearchState {
    pub tt:            Arc<TranspositionTable>,
    pub killers:       [[Option<ChessMove>; 2]; MAX_PLY],
    pub history:       [[[i32; 64]; 64]; 2],
    pub nodes:         u64,
    pub hard_deadline: Option<Instant>,
    pub soft_deadline: Option<Instant>,
    pub start_time:    Instant,
    pub stopped:       bool,
    pub seldepth:      usize,
    pub personality:   Personality,
    pub hash_history:  Vec<u64>,
    pub ply_hashes:    [u64; MAX_PLY],
    pub root_scores:   Vec<(i32, ChessMove)>,
    /// When true, suppresses per-depth "info" output (used by bench mode).
    pub silent:        bool,
    lmr:               LmrTable,
    /// Cached EvalParams — rebuilt once per search when style/options change.
    pub ep:            EvalParams,
    /// Profiling counters (only meaningful during bench).
    pub qnodes:        u64,   // quiescence nodes
    pub eval_calls:    u64,   // times evaluate_with() was called (from qsearch)
    pub tt_hits:       u64,   // TT cutoffs in negamax
    /// Number of search threads (Lazy SMP).
    pub num_threads:   usize,
    /// Shared stop flag — set by main thread to stop helpers.
    pub shared_stop:   Option<Arc<AtomicBool>>,
}

impl SearchState {
    pub fn new(tt_mb: usize) -> Self {
        SearchState {
            tt:            Arc::new(TranspositionTable::new(tt_mb)),
            killers:       [[None; 2]; MAX_PLY],
            history:       [[[0i32; 64]; 64]; 2],
            nodes:         0,
            hard_deadline: None,
            soft_deadline: None,
            start_time:    Instant::now(),
            stopped:       false,
            seldepth:      0,
            personality:   Personality::new(),
            hash_history:  Vec::new(),
            ply_hashes:    [0u64; MAX_PLY],
            root_scores:   Vec::new(),
            silent:        false,
            lmr:           LmrTable::build(),
            ep:            EvalParams::karpov_style(),
            qnodes:        0,
            eval_calls:    0,
            tt_hits:       0,
            num_threads:   1,
            shared_stop:   None,
        }
    }

    /// Construct a helper thread's SearchState sharing the given TT and stop flag.
    pub fn new_helper(tt: Arc<TranspositionTable>, stop: Arc<AtomicBool>) -> Self {
        SearchState {
            tt,
            killers:       [[None; 2]; MAX_PLY],
            history:       [[[0i32; 64]; 64]; 2],
            nodes:         0,
            hard_deadline: None,
            soft_deadline: None,
            start_time:    Instant::now(),
            stopped:       false,
            seldepth:      0,
            personality:   Personality::new(),
            hash_history:  Vec::new(),
            ply_hashes:    [0u64; MAX_PLY],
            root_scores:   Vec::new(),
            silent:        true,
            lmr:           LmrTable::build(),
            ep:            EvalParams::karpov_style(),
            qnodes:        0,
            eval_calls:    0,
            tt_hits:       0,
            num_threads:   1,
            shared_stop:   Some(stop),
        }
    }

    pub fn clear_for_search(&mut self) {
        self.killers      = [[None; 2]; MAX_PLY];
        // Age history instead of clearing (avoids losing good heuristics between depths)
        for side in &mut self.history {
            for row in side.iter_mut() {
                for v in row.iter_mut() { *v >>= 1; }
            }
        }
        self.nodes       = 0;
        self.seldepth    = 0;
        self.stopped     = false;
        self.root_scores = Vec::new();
        self.ply_hashes  = [0u64; MAX_PLY];
        self.qnodes      = 0;
        self.eval_calls  = 0;
        self.tt_hits     = 0;
    }

    pub fn elapsed_ms(&self) -> u64 { self.start_time.elapsed().as_millis() as u64 }
    pub fn nps(&self) -> u64 { self.nodes * 1000 / self.elapsed_ms().max(1) }

    fn time_up(&mut self) -> bool {
        if self.stopped { return true; }
        // Check shared stop flag (set by main thread or time expiry)
        if let Some(ref flag) = self.shared_stop {
            if flag.load(Ordering::Relaxed) { self.stopped = true; return true; }
        }
        if let Some(dl) = self.hard_deadline {
            if Instant::now() >= dl {
                self.stopped = true;
                // Signal helpers to stop
                if let Some(ref flag) = self.shared_stop {
                    flag.store(true, Ordering::Relaxed);
                }
                return true;
            }
        }
        false
    }

    fn contempt(&self) -> i32 {
        match self.personality.style {
            Style::Tal       => CONTEMPT_TAL,
            Style::Petrosian => CONTEMPT_PETROSIAN,
            Style::Fischer   => CONTEMPT_FISCHER,
            Style::Karpov    => CONTEMPT,  // neutral — wins by technique, not by avoiding draws
        }
    }

    fn is_repetition(&self, key: u64, ply: usize) -> bool {
        let mut count = 0usize;
        for &h in &self.hash_history {
            if h == key { count += 1; if count >= 2 { return true; } }
        }
        for &h in &self.ply_hashes[..ply] {
            if h == key { count += 1; if count >= 2 { return true; } }
        }
        false
    }
}

// ── Move scoring ──────────────────────────────────────────────────────────────
//
// Priority (highest first):
//   2 000 000  TT / hash move
//   1 000 000+ Captures: MVV-LVA  (victim*10 - attacker)
//     900 000  Queen promotion (non-capture)
//     800 000  Killer move slot 0
//     700 000  Killer move slot 1
//   0 – 16383  Quiet move: history score (never exceeds HIST_MAX)

fn is_capture(board: &Board, mv: ChessMove) -> bool {
    // Normal capture
    if board.piece_on(mv.get_dest()).is_some() { return true; }
    // En passant: pawn moves to the ep target square (which is empty)
    if let Some(ep_sq) = board.en_passant() {
        if mv.get_dest() == ep_sq && board.piece_on(mv.get_source()) == Some(Piece::Pawn) {
            return true;
        }
    }
    false
}

fn score_move(
    board: &Board,
    mv: ChessMove,
    tt_move: Option<ChessMove>,
    killers: &[Option<ChessMove>; 2],
    history: &[[i32; 64]; 64],
) -> i32 {
    if Some(mv) == tt_move { return 2_000_000; }

    if is_capture(board, mv) {
        let victim_val = board.piece_on(mv.get_dest())
            .map(|p| PIECE_VALUES[p.to_index()])
            .unwrap_or(PIECE_VALUES[0]); // en passant → pawn value
        let attacker_val = board.piece_on(mv.get_source())
            .map(|p| PIECE_VALUES[p.to_index()])
            .unwrap_or(100);
        return 1_000_000 + victim_val * 10 - attacker_val;
    }

    // Non-capture queen promotion
    if mv.get_promotion() == Some(Piece::Queen) { return 900_000; }
    // Other promotions treated as quiet moves (rare)

    if killers[0] == Some(mv) { return 800_000; }
    if killers[1] == Some(mv) { return 700_000; }

    let from = mv.get_source().to_index();
    let to   = mv.get_dest().to_index();
    history[from][to]
}

fn order_moves(
    board: &Board,
    moves: Vec<ChessMove>,
    tt_move: Option<ChessMove>,
    killers: &[Option<ChessMove>; 2],
    history: &[[i32; 64]; 64],
) -> Vec<ChessMove> {
    let mut scored: Vec<(i32, ChessMove)> = moves.into_iter()
        .map(|mv| (score_move(board, mv, tt_move, killers, history), mv))
        .collect();
    scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, mv)| mv).collect()
}

// ── Quiescence Search ─────────────────────────────────────────────────────────
//
// Searches captures (+ queen promotions) until the position is quiet.
// Delta pruning: skip captures that cannot possibly raise alpha even if won.

fn quiesce(board: &Board, mut alpha: i32, beta: i32, state: &mut SearchState) -> i32 {
    if state.nodes & 2047 == 0 && state.time_up() { return 0; }
    state.nodes  += 1;
    state.qnodes += 1;

    match board.status() {
        BoardStatus::Checkmate => return -(MATE_SCORE),
        BoardStatus::Stalemate => return 0,
        BoardStatus::Ongoing   => {}
    }

    state.eval_calls += 1;
    let stand_pat = evaluate_fast(board, &state.ep);

    if stand_pat >= beta { return beta; }
    if stand_pat > alpha { alpha = stand_pat; }

    // Collect captures + queen promotions, ordered by MVV-LVA
    let mut moves: Vec<(i32, ChessMove)> = MoveGen::new_legal(board)
        .filter(|&mv| {
            is_capture(board, mv)
                || mv.get_promotion() == Some(Piece::Queen)
        })
        .map(|mv| {
            let victim_val = board.piece_on(mv.get_dest())
                .map(|p| PIECE_VALUES[p.to_index()])
                .unwrap_or(PIECE_VALUES[0]); // pawn for en passant / promo
            let atk_val = board.piece_on(mv.get_source())
                .map(|p| PIECE_VALUES[p.to_index()])
                .unwrap_or(100);
            (victim_val * 10 - atk_val, mv)
        })
        .collect();
    moves.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    for (_, mv) in moves {
        // Delta pruning: skip if even winning this piece cannot raise alpha
        let gain = board.piece_on(mv.get_dest())
            .map(|p| PIECE_VALUES[p.to_index()])
            .unwrap_or(PIECE_VALUES[0]);
        if stand_pat + gain + DELTA_MARGIN < alpha { continue; }

        let nb    = board.make_move_new(mv);
        let score = -quiesce(&nb, -beta, -alpha, state);

        if score >= beta { return beta; }
        if score > alpha { alpha = score; }
    }

    alpha
}

// ── Negamax alpha-beta ────────────────────────────────────────────────────────

pub fn negamax(
    board:    &Board,
    depth:    i32,
    ply:      usize,
    mut alpha: i32,
    beta:     i32,
    in_null:  bool,
    state:    &mut SearchState,
) -> i32 {
    // Time check every 2048 nodes
    if state.nodes & 2047 == 0 && state.time_up() { return 0; }
    state.nodes += 1;
    if ply > state.seldepth { state.seldepth = ply; }

    // ── Terminal states ──────────────────────────────────────────────────────
    match board.status() {
        BoardStatus::Checkmate => return -(MATE_SCORE - ply as i32),
        BoardStatus::Stalemate => return 0,
        BoardStatus::Ongoing   => {}
    }

    // ── Repetition → contempt ────────────────────────────────────────────────
    let key = board.get_hash();
    if ply > 0 {
        if state.is_repetition(key, ply) { return -state.contempt(); }
        if ply < MAX_PLY { state.ply_hashes[ply] = key; }
    }

    // ── Leaf → quiescence ────────────────────────────────────────────────────
    if depth <= 0 { return quiesce(board, alpha, beta, state); }

    let pv_node = beta > alpha + 1;

    // ── Transposition table lookup ───────────────────────────────────────────
    let tt_entry = state.tt.get(key);
    if let Some(entry) = tt_entry {
        if !pv_node && entry.depth >= depth {
            match entry.flag {
                EXACT       => { state.tt_hits += 1; return entry.score; }
                LOWER_BOUND => { if entry.score >= beta  { state.tt_hits += 1; return entry.score; } }
                UPPER_BOUND => { if entry.score <= alpha { state.tt_hits += 1; return entry.score; } }
                _           => {}
            }
        }
    }
    let tt_move = tt_entry.and_then(|e| e.best_move);

    let in_check = *board.checkers() != BitBoard(0);

    // ── Null-move pruning ────────────────────────────────────────────────────
    // Skip: in check, after a null move, PV nodes, endgame (no non-pawn pieces)
    if !in_null && !in_check && !pv_node && depth >= NULL_MIN_DEPTH {
        let non_pawns = (*board.pieces(Piece::Knight)
            | *board.pieces(Piece::Bishop)
            | *board.pieces(Piece::Rook)
            | *board.pieces(Piece::Queen))
            & *board.color_combined(board.side_to_move());
        if non_pawns != BitBoard(0) {
            if let Some(nb) = board.null_move() {
                let ns = -negamax(&nb, depth - 1 - NULL_R, ply + 1,
                                  -beta, -beta + 1, true, state);
                if state.stopped { return 0; }
                if ns >= beta { return beta; }
            }
        }
    }

    // ── Futility pruning (depth 1-2) ─────────────────────────────────────────
    // At shallow depth, skip quiet non-checking moves that can't raise alpha.
    let futility_base: Option<i32> = if !in_check && !pv_node && depth <= 2 {
        state.eval_calls += 1;
        Some(evaluate_fast(board, &state.ep) + FUTILITY_PER_D * depth)
    } else {
        None
    };

    // ── Generate and order moves ─────────────────────────────────────────────
    let all_moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();
    let killers   = state.killers[ply.min(MAX_PLY - 1)];
    let color_idx = if board.side_to_move() == Color::White { 0 } else { 1 };
    let ordered   = order_moves(board, all_moves, tt_move, &killers,
                                &state.history[color_idx]);

    let mut best_score     = -INFINITY;
    let mut best_move      = None;
    let mut moves_searched = 0usize;
    let orig_alpha         = alpha;

    for mv in &ordered {
        let cap = is_capture(board, *mv);
        let nb  = board.make_move_new(*mv);
        let gives_check = *nb.checkers() != BitBoard(0);

        // ── Futility pruning ─────────────────────────────────────────────────
        if let Some(fb) = futility_base {
            if moves_searched > 0
                && !cap
                && !gives_check
                && mv.get_promotion().is_none()
                && fb <= alpha
            {
                moves_searched += 1;
                continue;
            }
        }

        // Check extension: +1 ply when the move gives check
        let extension = if gives_check { 1 } else { 0 };

        // ── Late Move Reduction ──────────────────────────────────────────────
        // Reduce quiet, non-checking late moves that are unlikely to be best.
        // Never reduce: first move, captures, checks, in-check positions, killers.
        let reduction = if depth >= LMR_MIN_DEPTH
            && moves_searched >= LMR_MIN_MOVES
            && !cap
            && !gives_check
            && !in_check
            && extension == 0
        {
            state.lmr.get(depth, moves_searched)
        } else {
            0
        };

        // ── Recursive search ─────────────────────────────────────────────────
        let score = if moves_searched == 0 {
            // First move: always search full window
            -negamax(&nb, depth - 1 + extension, ply + 1, -beta, -alpha, false, state)

        } else if reduction > 0 {
            // LMR: reduced null-window probe; re-search at full depth if it beats alpha
            let s = -negamax(&nb, depth - 1 - reduction + extension, ply + 1,
                             -alpha - 1, -alpha, false, state);
            if s > alpha && !state.stopped {
                -negamax(&nb, depth - 1 + extension, ply + 1, -beta, -alpha, false, state)
            } else { s }

        } else {
            // PVS: null-window probe; full re-search only if it falls inside (alpha, beta)
            let s = -negamax(&nb, depth - 1 + extension, ply + 1,
                             -alpha - 1, -alpha, false, state);
            if s > alpha && s < beta && !state.stopped {
                -negamax(&nb, depth - 1 + extension, ply + 1, -beta, -alpha, false, state)
            } else { s }
        };

        if state.stopped { return 0; }
        moves_searched += 1;

        // Collect root move scores for personality-based selection
        if ply == 0 {
            state.root_scores.push((score, *mv));
        }

        if score > best_score {
            best_score = score;
            best_move  = Some(*mv);
        }
        if score > alpha { alpha = score; }
        if alpha >= beta {
            // Beta cutoff: update killers and history for quiet moves
            if !cap {
                let k = &mut state.killers[ply.min(MAX_PLY - 1)];
                if k[0] != Some(*mv) { k[1] = k[0]; k[0] = Some(*mv); }
                let h = &mut state.history[color_idx]
                    [mv.get_source().to_index()][mv.get_dest().to_index()];
                *h = (*h + depth * depth).min(HIST_MAX);
            }
            break;
        }
    }

    // ── Store in transposition table ─────────────────────────────────────────
    if !state.stopped {
        let flag = if best_score <= orig_alpha { UPPER_BOUND }
                   else if best_score >= beta  { LOWER_BOUND }
                   else                        { EXACT };
        state.tt.put(key, depth, flag, best_score, best_move);
    }

    best_score
}

// ── PV extraction ─────────────────────────────────────────────────────────────

fn extract_pv(board: &Board, tt: &TranspositionTable, max_len: usize) -> Vec<ChessMove> {
    let mut pv   = Vec::with_capacity(max_len);
    let mut b    = board.clone();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..max_len {
        let key = b.get_hash();
        if !seen.insert(key) { break; }
        match tt.get(key).and_then(|e| e.best_move) {
            Some(mv) => { pv.push(mv); b = b.make_move_new(mv); }
            None     => break,
        }
    }
    pv
}

fn score_string(score: i32) -> String {
    if score.abs() > MATE_SCORE - 200 {
        let moves = (MATE_SCORE - score.abs() + 1) / 2;
        let sign  = if score > 0 { "" } else { "-" };
        format!("mate {}{}", sign, moves)
    } else {
        format!("cp {}", score)
    }
}

fn print_info(board: &Board, depth: i32, score: i32, state: &SearchState) {
    if state.silent { return; }
    let pv      = extract_pv(board, &state.tt, depth as usize + 4);
    let pv_uci  = pv.iter().map(|m| m.to_string()).collect::<Vec<_>>();
    let pv_book = pv_to_book(board, &pv);
    println!(
        "info depth {} seldepth {} score {} nodes {} nps {} time {} hashfull {} pv {}",
        depth, state.seldepth, score_string(score),
        state.nodes, state.nps(), state.elapsed_ms(),
        state.tt.hashfull(), pv_uci.join(" "),
    );
    if !pv_book.is_empty() {
        println!("info string {}", pv_book);
    }
    std::io::stdout().flush().ok();
}

// ── Iterative Deepening ───────────────────────────────────────────────────────

pub struct SearchResult {
    pub best_move: Option<ChessMove>,
    pub score:     i32,
    pub depth:     i32,
    pub nodes:     u64,
}

/// Inner iterative-deepening loop shared by main thread and Lazy SMP helpers.
/// Returns (best_move, best_score, best_depth).
fn iterative_deepening(
    board:     &Board,
    max_depth: i32,
    state:     &mut SearchState,
) -> (Option<ChessMove>, i32, i32) {
    let mut best_move      = None;
    let mut best_score     = 0i32;
    let mut best_depth     = 1i32;
    let mut prev_score     = state.personality.prev_score;
    let mut prev_score_asp = 0i32;

    for depth in 1..=max_depth {
        if state.stopped { break; }

        state.root_scores.clear();
        state.seldepth = 0;

        let (asp_alpha, asp_beta) = if depth >= 4 {
            (prev_score_asp - ASP_WINDOW, prev_score_asp + ASP_WINDOW)
        } else {
            (-INFINITY, INFINITY)
        };

        let raw = negamax(board, depth, 0, asp_alpha, asp_beta, false, state);
        if state.stopped { break; }

        let completed = if depth >= 4 && (raw <= asp_alpha || raw >= asp_beta) {
            negamax(board, depth, 0, -INFINITY, INFINITY, false, state)
        } else {
            raw
        };
        if state.stopped { break; }
        prev_score_asp = completed;

        best_score = completed;
        best_depth = depth;

        if let Some(entry) = state.tt.get(board.get_hash()) {
            best_move = entry.best_move;
        }

        state.root_scores.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        print_info(board, depth, best_score, state);

        if best_score.abs() > MATE_SCORE - 100 { break; }

        if let Some(soft) = state.soft_deadline {
            let change = (best_score - prev_score).abs();
            if Instant::now() > soft && change <= 40 { break; }
        }
        prev_score = best_score;
    }

    (best_move, best_score, best_depth)
}

pub fn find_best_move(
    board:          &Board,
    max_depth:      i32,
    _ply_from_start: usize,
    state:          &mut SearchState,
) -> SearchResult {
    state.clear_for_search();
    state.ep         = EvalParams::from_personality(&state.personality);
    state.start_time = Instant::now();

    // ── Opening book probe ────────────────────────────────────────────────────
    let book_mv = petrosian_book().probe(board.get_hash());
    if let Some(book_mv) = book_mv {
        if !state.silent {
            println!("info depth 0 score cp 0 nodes 1 nps 0 time 0 hashfull 0 pv {}", book_mv);
            println!("info string Book: {}", book_mv);
            std::io::stdout().flush().ok();
        }
        return SearchResult { best_move: Some(book_mv), score: 0, depth: 0, nodes: 1 };
    }

    // ── Lazy SMP: spawn helper threads ───────────────────────────────────────
    let num_threads = state.num_threads.max(1);
    let shared_stop = Arc::new(AtomicBool::new(false));

    let helpers: Vec<_> = if num_threads > 1 {
        let board_copy      = *board;
        let tt              = Arc::clone(&state.tt);
        let ep              = state.ep.clone();
        let style           = state.personality.style;
        let hash_history    = state.hash_history.clone();
        let hard_deadline   = state.hard_deadline;
        let shared_stop_ref = Arc::clone(&shared_stop);

        (1..num_threads).map(|i| {
            let b    = board_copy;
            let tt2  = Arc::clone(&tt);
            let stop = Arc::clone(&shared_stop_ref);
            let ep2  = ep.clone();
            let hh   = hash_history.clone();

            std::thread::spawn(move || {
                let mut hs          = SearchState::new_helper(tt2, stop);
                hs.ep               = ep2;
                hs.personality.style = style;
                hs.hash_history      = hh;
                hs.hard_deadline     = hard_deadline;
                // Stagger starting depth to diversify search
                let start_depth = (i as i32).min(max_depth);
                for depth in start_depth..=max_depth {
                    if hs.stopped { break; }
                    negamax(&b, depth, 0, -INFINITY, INFINITY, false, &mut hs);
                }
            })
        }).collect()
    } else {
        vec![]
    };

    // Wire shared stop into main thread's state
    state.shared_stop = Some(Arc::clone(&shared_stop));

    // ── Main thread search ───────────────────────────────────────────────────
    let (best_move, best_score, best_depth) = iterative_deepening(board, max_depth, state);

    // Signal all helpers to stop and wait
    shared_stop.store(true, Ordering::Relaxed);
    for h in helpers { let _ = h.join(); }

    // Record this move for time model learning
    let time_spent   = state.elapsed_ms();
    let score_gain   = (best_score - state.personality.prev_score).abs();
    let was_unstable = score_gain > 30;
    state.personality.record_move_time(board, time_spent, score_gain, was_unstable);

    // Persist move records after every move so data is never lost if the
    // GUI closes without sending ucinewgame or result.
    state.personality.time_model.save();

    state.personality.prev_score = best_score;
    SearchResult { best_move, score: best_score, depth: best_depth, nodes: state.nodes }
}
