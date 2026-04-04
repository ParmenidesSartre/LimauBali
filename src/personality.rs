// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — Personality & Human Behaviour System
//
//  UCI options:
//    setoption name Style value Karpov|Tal|Petrosian|Fischer
//    setoption name UCI_LimitStrength value true|false
//    setoption name UCI_Elo value 1000-2200
//
//  Human time management:
//    Complexity-aware, clock-pressure-aware, score-instability-aware.
//    Each game feels different — not clock-perfectly-distributed.
//
//  Blunders:
//    Only triggered under pressure (score < -80), scaled by how bad it is.
//    Never gives away free material — just picks a suboptimal plan.
// ─────────────────────────────────────────────────────────────────────────────

use chess::{BitBoard, Board, ChessMove, Color, MoveGen, Piece};

use crate::time_model::{TimeControl, Phase, TimeModel};

pub const ENGINE_ELO: i32 = 2245; // calibrated from tournament

// ── Style ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Karpov,
    Tal,
    Petrosian,
    Fischer,
}

// ── Personality ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Personality {
    pub style:          Style,
    pub limit_strength: bool,
    pub target_elo:     i32,
    rng:                u64,

    // Time management state
    pub prev_score:     i32,
    pub score_stable:   bool,

    // Self-teaching time model
    pub time_model:     TimeModel,
}

impl Personality {
    pub fn new() -> Self {
        Personality {
            style:          Style::Karpov,
            limit_strength: false,
            target_elo:     2000,
            rng:            0x517c_c1b7_2722_0a95,
            prev_score:     0,
            score_stable:   true,
            time_model:     TimeModel::load(),
        }
    }

    pub fn reset_game(&mut self) {
        // If we have move data from a completed game but never got a result
        // command (Arena doesn't send it), save with neutral result so the
        // time model data isn't lost.
        if !self.time_model.moves.is_empty() {
            self.time_model.update(0.5);
        }
        self.prev_score   = 0;
        self.score_stable = true;
    }

    /// Record a move's time usage. Called from search after each move.
    pub fn record_move_time(&mut self, board: &Board, time_ms: u64,
                            score_gain: i32, was_unstable: bool) {
        let pieces = (board.pieces(Piece::Knight) | board.pieces(Piece::Bishop)
                    | board.pieces(Piece::Rook)   | board.pieces(Piece::Queen)).popcnt();
        let phase = Phase::from_piece_count(pieces);
        self.time_model.record_move(phase, time_ms, score_gain, was_unstable);
    }

    /// Called at game end with result string from UCI ("1-0", "0-1", "1/2-1/2").
    /// Updates and saves the time model.
    pub fn finish_game(&mut self, result: &str, engine_is_white: bool) {
        let score = match result {
            "1-0" => if engine_is_white { 1.0 } else { 0.0 },
            "0-1" => if engine_is_white { 0.0 } else { 1.0 },
            _     => 0.5,
        };
        self.time_model.update(score);
        let tc  = self.time_model.active_tc;
        let b   = &self.time_model.buckets[tc.idx()];
        eprintln!(
            "info string TimeModel [{}] game #{}: opening={:.3} midgame={:.3} endgame={:.3} instability={:.3}",
            tc.name(), b.games_played, b.phase_scale[0], b.phase_scale[1], b.phase_scale[2], b.instability_bonus
        );
    }

    // ── xorshift64 RNG ────────────────────────────────────────────────────────

    pub fn rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.rng = x; x
    }

    fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        if lo >= hi { return lo; }
        lo + (self.rand() % (hi - lo + 1) as u64) as i32
    }

    fn rand_f(&mut self) -> f64 {
        (self.rand() as f64) / (u64::MAX as f64)
    }

    /// Centipawn noise for UCI_LimitStrength — only used under pressure in search.rs.
    pub fn eval_noise(&mut self) -> i32 {
        if !self.limit_strength { return 0; }
        let range = ((ENGINE_ELO - self.target_elo) / 8).clamp(0, 150);
        if range == 0 { return 0; }
        self.rand_range(-range, range)
    }

    // ── Human-like time management ────────────────────────────────────────────
    //
    // Returns (soft_limit_ms, hard_limit_ms).
    //   soft_limit: engine prefers to stop here (after completing a depth)
    //   hard_limit: engine must stop immediately regardless
    //
    // Factors:
    //   1. Base allocation from clock
    //   2. Position complexity (many moves, lots of captures, in check)
    //   3. Score instability from last search (score dropped → think more)
    //   4. Clock pressure (low time → shrink)
    //   5. Human variation (±25% random jitter, makes each game feel unique)
    //
    pub fn compute_time(
        &mut self,
        board: &Board,
        our_time_ms: u64,
        inc_ms: u64,
        movestogo: Option<u64>,
        last_score_change: i32, // |score_now - score_prev|, 0 if first move
    ) -> (u64, u64) {
        // Auto-detect time-control bucket so the learned parameters are drawn
        // from the correct set (bullet / blitz / rapid / classical).
        self.time_model.set_tc(our_time_ms);

        // Flag detection: mark near-flag earlier for bullet (2s) than blitz (3s).
        let flag_threshold = match TimeControl::detect(our_time_ms) {
            TimeControl::Bullet => 2_000,
            _                   => 3_000,
        };
        if our_time_ms < flag_threshold {
            self.time_model.mark_near_flag();
        }

        // Estimate moves remaining: use GUI hint if given, otherwise derive from
        // piece count (fewer pieces → endgame → fewer moves left).
        // Always keep a safety reserve of 2s (or 10% of clock, whichever is bigger)
        // so we never flag even if every factor stacks to maximum.
        let pieces = (board.pieces(Piece::Knight) | board.pieces(Piece::Bishop)
                    | board.pieces(Piece::Rook)   | board.pieces(Piece::Queen)).popcnt();

        // Bullet/blitz games end faster — fewer moves remain on average.
        // Aggressive allocation: give each move more time by assuming fewer moves left.
        let tc = self.time_model.active_tc;
        let estimated_mtg = match tc {
            TimeControl::Bullet => {
                if pieces > 10 { 22.0 } else if pieces > 6 { 16.0 }
                else if pieces > 2 { 10.0 } else { 5.0 }
            }
            TimeControl::Blitz => {
                if pieces > 10 { 28.0 } else if pieces > 6 { 20.0 }
                else if pieces > 2 { 12.0 } else { 6.0 }
            }
            _ => {
                if pieces > 10 { 35.0 } else if pieces > 6 { 25.0 }
                else if pieces > 2 { 15.0 } else { 8.0 }
            }
        };
        let mtg = movestogo.map(|m| m as f64).unwrap_or(estimated_mtg).max(1.0);

        // Safety reserve: bullet uses 92%, others 90%
        let reserve = match tc {
            TimeControl::Bullet => 0.92,
            _                                      => 0.90,
        };
        let usable_ms = (our_time_ms as f64 * reserve).max(1.0);

        // ── 1. Base time ──────────────────────────────────────────────────────
        // Standard formula: remaining_time / moves_to_go + increment
        let base = usable_ms / mtg + inc_ms as f64 * 0.8;

        // ── 2. Complexity factor ──────────────────────────────────────────────
        let moves: Vec<ChessMove> = MoveGen::new_legal(board).collect();
        let n_moves    = moves.len() as f64;
        let n_captures = moves.iter()
            .filter(|m| board.piece_on(m.get_dest()).is_some())
            .count() as f64;
        let in_check   = *board.checkers() != BitBoard(0);

        // More legal moves = complex position
        let move_factor = if n_moves > 40.0 { 1.35 }
                          else if n_moves > 30.0 { 1.15 }
                          else if n_moves < 10.0 { 0.70 }  // few moves = forced/simple
                          else { 1.0 };

        // Lots of captures = tactical complexity
        let capture_factor = if n_captures > 5.0 { 1.25 }
                             else if n_captures > 2.0 { 1.10 }
                             else { 1.0 };

        // In check = critical, must think
        let check_factor = if in_check { 1.40 } else { 1.0 };

        // ── 3. Score instability ──────────────────────────────────────────────
        // Threshold values are fixed; the *magnitude* of the bonus is learned.
        let learned_bonus = self.time_model.instability_scale() as f64;
        let instability_factor: f64 = if last_score_change > 80 {
            1.0 + learned_bonus * 1.50
        } else if last_score_change > 40 {
            1.0 + learned_bonus * 0.75
        } else if last_score_change > 20 {
            1.0 + learned_bonus * 0.375
        } else {
            1.0
        };

        // ── 4. Pressure from losing position ─────────────────────────────────
        // Humans panic and rush when losing badly → use *less* time (paradoxically worse)
        // But also sometimes overthink and use more → model both
        let pressure_factor = if self.prev_score < -200 {
            // Losing badly: sometimes panic (quick), sometimes overthink
            if self.rand_f() < 0.35 { 0.65 } else { 1.20 }
        } else if self.prev_score < -80 {
            1.15 // slightly losing → think more
        } else if self.prev_score > 200 {
            0.85 // winning comfortably → play faster, confident
        } else {
            1.0
        };

        // ── 5. Phase factor — learned from experience ─────────────────────────
        let phase_factor = self.time_model.phase_scale_for(pieces) as f64;

        // ── 6. Human variation (random jitter) ───────────────────────────────
        // Bullet/blitz: tight ±10% — consistency matters more than variety.
        // Rapid/classical: ±25% — more natural variation in longer games.
        let jitter = match tc {
            TimeControl::Bullet => 0.90 + self.rand_f() * 0.20, // 0.90–1.10
            TimeControl::Blitz  => 0.85 + self.rand_f() * 0.30, // 0.85–1.15
            _                                      => 0.75 + self.rand_f() * 0.50, // 0.75–1.25
        };

        // ── 7. Style modifier ─────────────────────────────────────────────────
        let style_factor = match self.style {
            Style::Tal => {
                // Tal played fast and intuitively, especially in attack
                if n_captures > 3.0 { 0.80 } else { 0.90 }
            }
            Style::Petrosian => {
                // Petrosian thought deeply and prophylactically — always take more time
                1.45
            }
            Style::Karpov => {
                // Karpov was thorough and methodical, especially in complex positions
                if n_captures > 2.0 { 1.20 } else { 1.10 }
            }
            Style::Fischer => {
                // Fischer was confident and quick — knew theory, trusted his calculation
                if n_captures > 4.0 { 1.10 } else { 0.85 }
            }
        };

        // ── Combine all factors ───────────────────────────────────────────────
        let soft = base
            * move_factor
            * capture_factor
            * check_factor
            * instability_factor
            * pressure_factor
            * phase_factor
            * jitter
            * style_factor;

        // ── Clock pressure safety ─────────────────────────────────────────────
        // Move faster as clock drains — thresholds are tighter for bullet.
        // The max_fraction caps how much of the remaining clock one move can use.
        let max_fraction = match tc {
            TimeControl::Bullet => {
                if      our_time_ms < 3_000  { 0.12 }  // < 3s  → tiny moves only
                else if our_time_ms < 8_000  { 0.18 }  // < 8s  → very fast
                else if our_time_ms < 15_000 { 0.25 }  // < 15s → fast
                else                          { 0.38 }  // plenty of time
            }
            TimeControl::Blitz => {
                if      our_time_ms < 5_000  { 0.15 }
                else if our_time_ms < 15_000 { 0.22 }
                else if our_time_ms < 30_000 { 0.30 }
                else                          { 0.40 }
            }
            _ => {
                if our_time_ms < 5_000  { 0.20 }
                else if our_time_ms < 15_000 { 0.30 }
                else                          { 0.40 }
            }
        };
        let max_allowed = (our_time_ms as f64 * max_fraction) as u64;

        // Hard limit = 2× soft for bullet (less search extension room), 3× for others.
        let hard_mult = match tc {
            TimeControl::Bullet => 2,
            _                                      => 3,
        };
        let soft_ms = (soft as u64).min(max_allowed).max(1);
        let hard_ms = (soft_ms * hard_mult).min(max_allowed)
                          .min(our_time_ms.saturating_sub(200)).max(1);

        (soft_ms, hard_ms)
    }

    // ── Move temperature threshold (cp) ───────────────────────────────────────
    // Moves within this range of the best are candidates for varied selection.
    pub fn temperature_cp(&self, current_score: i32) -> i32 {
        let base = match self.style {
            Style::Tal      => 25,  // intuitive, willing to vary
            Style::Petrosian => 0,  // always plays the objectively best defensive move
            Style::Karpov   => 5,   // slight variation; usually plays the best move
            Style::Fischer  => 0,   // precise — plays the best move
        };

        // ELO-based extra threshold
        let elo_extra = if self.limit_strength {
            ((ENGINE_ELO - self.target_elo) / 20).clamp(0, 100)
        } else { 0 };

        // Under pressure → slightly more "desperate" moves
        let pressure_extra = if current_score < -150 { 25 }
                             else if current_score < -80 { 12 }
                             else { 0 };

        base + elo_extra + pressure_extra
    }

    // ── Pick move from root candidates ────────────────────────────────────────
    // candidates: sorted best-first (score, move)
    // current_score: the best score found
    pub fn pick_move(
        &mut self,
        board: &Board,
        candidates: &[(i32, ChessMove)],
        current_score: i32,
    ) -> Option<ChessMove> {
        if candidates.is_empty() { return None; }

        let best   = candidates[0].0;
        let thresh = self.temperature_cp(current_score);

        // Pool: moves within threshold of best
        let pool: Vec<&(i32, ChessMove)> = candidates.iter()
            .filter(|(s, _)| best - s <= thresh)
            .collect();

        // Only one option or no temperature → always best
        if pool.len() == 1 || thresh == 0 {
            return Some(candidates[0].1);
        }

        // Petrosian: no special selection — always picks the objectively best move.
        // The positional steering happens entirely in the evaluation function.
        if self.style == Style::Petrosian {
            return Some(candidates[0].1);
        }

        if self.style == Style::Tal {
            // Prefer sac+checks that the search judged as sound (within 25cp of best).
            const SAC_WINDOW: i32 = 25;
            let sac_checks: Vec<_> = pool.iter().filter(|(s, mv)| {
                if best - s > SAC_WINDOW { return false; }
                let nb = board.make_move_new(*mv);
                if *nb.checkers() == BitBoard(0) { return false; }
                let attacker_val = board.piece_on(mv.get_source())
                    .map(|p| crate::tables::PIECE_VALUES[p.to_index()]).unwrap_or(100);
                let victim_val = board.piece_on(mv.get_dest())
                    .map(|p| crate::tables::PIECE_VALUES[p.to_index()]).unwrap_or(0);
                victim_val < attacker_val
            }).collect();

            if !sac_checks.is_empty() {
                let best_sac = sac_checks.iter().max_by_key(|(s, _)| *s);
                if let Some(entry) = best_sac { return Some(entry.1); }
            }

            // Any check in pool
            let checks: Vec<_> = pool.iter().filter(|(_, mv)| {
                *board.make_move_new(*mv).checkers() != BitBoard(0)
            }).collect();
            if !checks.is_empty() {
                return Some(checks[self.rand() as usize % checks.len()].1);
            }

            // Captures
            let captures: Vec<_> = pool.iter().filter(|(_, mv)| {
                board.piece_on(mv.get_dest()).is_some() || mv.get_promotion().is_some()
            }).collect();
            if !captures.is_empty() {
                return Some(captures[self.rand() as usize % captures.len()].1);
            }
        }

        // Weighted random among pool: weight ∝ exp((score - worst) / 15)
        // Approximated as linear for speed
        let worst = pool.last().map(|(s, _)| *s).unwrap_or(best - thresh);
        let weights: Vec<u64> = pool.iter()
            .map(|(s, _)| ((s - worst + 1) as u64).max(1))
            .collect();
        let total: u64 = weights.iter().sum();
        let mut pick = self.rand() % total.max(1);
        for (w, entry) in weights.iter().zip(pool.iter()) {
            if pick < *w { return Some(entry.1); }
            pick -= w;
        }
        Some(pool[0].1)
    }

    // ── Tal static evaluation bonus (called once per eval leaf) ───────────────
    // Pure function — no RNG — so it's stable within search tree.
    // Mild attack/activity bonus — rewards piece proximity and open lines.
    pub fn tal_eval_bonus(board: &Board) -> i32 {
        let stm    = board.side_to_move();
        let our    = *board.color_combined(stm);
        let them   = *board.color_combined(!stm);
        let e_king = board.king_square(!stm);
        let ekf    = e_king.get_file().to_index() as i32;
        let ekr    = e_king.get_rank().to_index() as i32;
        let all_pawns = *board.pieces(Piece::Pawn);
        let mut bonus = 0i32;

        // ── Piece proximity to enemy king ─────────────────────────────────────
        for &piece in &[Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
            for sq in *board.pieces(piece) & our {
                let pf   = sq.get_file().to_index() as i32;
                let pr   = sq.get_rank().to_index() as i32;
                let dist = (pf - ekf).abs().max((pr - ekr).abs());
                let prox = (6 - dist).max(0);
                let w = match piece {
                    Piece::Queen  => 5,
                    Piece::Rook   => 3,
                    Piece::Bishop => 2,
                    Piece::Knight => 3,
                    _             => 0,
                };
                bonus += prox * w;
            }
        }

        // ── Open/semi-open files near enemy king ──────────────────────────────
        for df in [-1i32, 0, 1] {
            let f = ekf + df;
            if !(0..8).contains(&f) { continue; }
            let fmask = BitBoard::new(0x0101_0101_0101_0101u64 << f as usize);
            let our_pawns = *board.pieces(Piece::Pawn) & our;
            if (all_pawns & fmask) == BitBoard(0) {
                bonus += 10;
            } else if (our_pawns & fmask) == BitBoard(0) {
                bonus += 4;
            }
        }

        // ── Piece count = complexity/chaos ────────────────────────────────────
        let n_pieces = (board.pieces(Piece::Knight) | board.pieces(Piece::Bishop)
            | board.pieces(Piece::Rook) | board.pieces(Piece::Queen)).popcnt() as i32;
        bonus += n_pieces;

        // ── Pawn storm toward enemy king ──────────────────────────────────────
        let our_pawns = *board.pieces(Piece::Pawn) & our;
        for sq in our_pawns {
            let pf = sq.get_file().to_index() as i32;
            let pr = sq.get_rank().to_index() as i32;
            if (pf - ekf).abs() <= 2 {
                let advance = if stm == Color::White { pr } else { 7 - pr };
                if advance >= 4 {
                    bonus += (advance - 3) * 6;
                }
            }
        }

        // ── Enemy king defenders ──────────────────────────────────────────────
        let mut enemy_defenders = 0i32;
        for &piece in &[Piece::Rook, Piece::Bishop, Piece::Knight] {
            for sq in *board.pieces(piece) & them {
                let pf = sq.get_file().to_index() as i32;
                let pr = sq.get_rank().to_index() as i32;
                let dist = (pf - ekf).abs().max((pr - ekr).abs());
                if dist <= 2 { enemy_defenders += 1; }
            }
        }
        bonus += (4 - enemy_defenders).max(0) * 3;

        bonus
    }

}
