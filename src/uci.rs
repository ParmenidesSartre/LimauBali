// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — UCI Protocol Handler
// ─────────────────────────────────────────────────────────────────────────────

use chess::{Board, ChessMove, Color};
use std::io::{self, BufRead, Write};
use std::str::FromStr;
use std::time::{Duration, Instant};

use crate::bench::{print_bench_result, run_bench};
use crate::eval::evaluate_trace;
use crate::personality::{Style, ENGINE_ELO};
use crate::search::{find_best_move, SearchState};
use crate::tt::TranspositionTable;
use std::sync::Arc;

pub struct UciEngine {
    pub state:         SearchState,
    pub board:         Board,
    pub ply_from_start: usize,   // how many half-moves from starting position
}

impl UciEngine {
    pub fn new() -> Self {
        UciEngine {
            state:          SearchState::new(128),
            board:          Board::default(),
            ply_from_start: 0,
        }
    }

    pub fn run(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            self.handle_command(&line);
        }
    }

    /// Called by main.rs to replay the first line that was already consumed
    /// for protocol detection.
    pub fn handle_first(&mut self, line: String) {
        self.handle_command(&line);
    }

    fn handle_command(&mut self, line: &str) {
        let cmd = match line.split_whitespace().next() { Some(c) => c, None => return };
        match cmd {
            "uci"        => self.cmd_uci(),
            "isready"    => { println!("readyok"); }
            "ucinewgame" => self.cmd_newgame(),
            "setoption"  => self.cmd_setoption(line),
            "position"   => self.cmd_position(line),
            "go"         => self.cmd_go(line),
            "stop"       => { self.state.stopped = true; }
            "result"     => self.cmd_result(line),
            "eval"       => self.cmd_eval(),
            "bench"      => self.cmd_bench(line),
            "quit"       => std::process::exit(0),
            _            => {}
        }
        io::stdout().flush().ok();
    }

    fn cmd_uci(&self) {
        let style_tag = match self.state.personality.style {
            crate::personality::Style::Karpov    => " (Karpov)",
            crate::personality::Style::Tal       => " (Tal)",
            crate::personality::Style::Petrosian => " (Petrosian)",
            crate::personality::Style::Fischer   => " (Fischer)",
        };
        println!("id name LimauBali{}", style_tag);
        println!("id author Faizal Azman (Malaysia)");
        println!("option name Hash type spin default 128 min 1 max 4096");
        println!("option name Threads type spin default 1 min 1 max 64");
        println!("option name Style type combo default Karpov var Karpov var Tal var Petrosian var Fischer");
        println!("option name UCI_LimitStrength type check default false");
        println!("option name UCI_Elo type spin default 2000 min 1000 max 2200");
        println!("uciok");
    }

    fn cmd_newgame(&mut self) {
        self.board          = Board::default();
        self.ply_from_start = 0;
        self.state.tt.clear();
        self.state.hash_history.clear();
        self.state.personality.reset_game();
    }

    fn cmd_setoption(&mut self, line: &str) {
        // setoption name <name> value <value>
        let tokens: Vec<&str> = line.split_whitespace().collect();
        // Find "name" and "value" positions
        let name_pos  = tokens.iter().position(|&t| t == "name");
        let value_pos = tokens.iter().position(|&t| t == "value");
        if name_pos.is_none() { return; }
        let np = name_pos.unwrap() + 1;
        let vp = value_pos.map(|p| p + 1);

        let opt_name = match value_pos {
            Some(v) => tokens[np..v.saturating_sub(1)+1-1+1].join(" "),
            None    => tokens[np..].join(" "),
        };
        let opt_val = vp.and_then(|p| tokens.get(p)).map(|s| *s).unwrap_or("");

        match opt_name.as_str() {
            "Hash" => {
                if let Ok(mb) = opt_val.parse::<usize>() {
                    self.state.tt = Arc::new(TranspositionTable::new(mb));
                }
            }
            "Threads" => {
                if let Ok(n) = opt_val.parse::<usize>() {
                    self.state.num_threads = n.clamp(1, 64);
                }
            }
            "Style" => {
                self.state.personality.style = match opt_val {
                    "Tal"       => Style::Tal,
                    "Petrosian" => Style::Petrosian,
                    "Fischer"   => Style::Fischer,
                    _           => Style::Karpov,
                };
            }
            "UCI_LimitStrength" => {
                self.state.personality.limit_strength = opt_val == "true";
            }
            "UCI_Elo" => {
                if let Ok(elo) = opt_val.parse::<i32>() {
                    self.state.personality.target_elo = elo.clamp(1000, ENGINE_ELO);
                }
            }
            _ => {}
        }
    }

    fn cmd_position(&mut self, line: &str) {
        let mut tokens = line.split_whitespace().peekable();
        tokens.next(); // "position"

        let kind = match tokens.next() { Some(k) => k, None => return };

        let mut board = if kind == "startpos" {
            Board::default()
        } else if kind == "fen" {
            let mut parts = Vec::new();
            while let Some(&t) = tokens.peek() {
                if t == "moves" { break; }
                parts.push(t);
                tokens.next();
            }
            Board::from_str(&parts.join(" ")).unwrap_or_default()
        } else {
            Board::default()
        };

        // Skip "moves" keyword
        if tokens.peek() == Some(&"moves") { tokens.next(); }

        // Apply moves and build hash history
        self.state.hash_history.clear();
        self.state.hash_history.push(board.get_hash());
        let mut ply = 0usize;

        for mv_str in tokens {
            match ChessMove::from_str(mv_str) {
                Ok(mv) => {
                    board = board.make_move_new(mv);
                    self.state.hash_history.push(board.get_hash());
                    ply += 1;
                }
                Err(_) => break,
            }
        }

        self.board          = board;
        self.ply_from_start = ply;
    }

    fn cmd_go(&mut self, line: &str) {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        let side_is_white = self.board.side_to_move() == Color::White;
        let mut wtime:     Option<u64> = None;
        let mut btime:     Option<u64> = None;
        let mut winc:      u64 = 0;
        let mut binc:      u64 = 0;
        let mut movestogo: Option<u64> = None;
        let mut movetime:  Option<u64> = None;
        let mut max_depth: i32 = 64;
        let mut infinite         = false;

        let mut i = 1;
        while i < tokens.len() {
            match tokens[i] {
                "wtime"     => { wtime     = tokens.get(i+1).and_then(|v| v.parse().ok()); i += 2; }
                "btime"     => { btime     = tokens.get(i+1).and_then(|v| v.parse().ok()); i += 2; }
                "winc"      => { winc      = tokens.get(i+1).and_then(|v| v.parse().ok()).unwrap_or(0); i += 2; }
                "binc"      => { binc      = tokens.get(i+1).and_then(|v| v.parse().ok()).unwrap_or(0); i += 2; }
                "movestogo" => { movestogo = tokens.get(i+1).and_then(|v| v.parse().ok()); i += 2; }
                "movetime"  => { movetime  = tokens.get(i+1).and_then(|v| v.parse().ok()); i += 2; }
                "depth"     => { max_depth = tokens.get(i+1).and_then(|v| v.parse().ok()).unwrap_or(64); i += 2; }
                "infinite"  => { infinite  = true; i += 1; }
                _ => { i += 1; }
            }
        }

        // ── Time management ───────────────────────────────────────────────────
        let (soft_ms, hard_ms) = if infinite {
            (u64::MAX, u64::MAX)
        } else if let Some(mt) = movetime {
            (mt, mt)
        } else {
            let our_time = if side_is_white { wtime } else { btime }.unwrap_or(10_000);
            let our_inc  = if side_is_white { winc } else { binc };
            let last_score_change = 0i32; // first move of game: no history
            self.state.personality.compute_time(
                &self.board, our_time, our_inc, movestogo, last_score_change,
            )
        };

        let now = Instant::now();
        self.state.soft_deadline = if soft_ms == u64::MAX { None }
                                   else { Some(now + Duration::from_millis(soft_ms)) };
        self.state.hard_deadline = if hard_ms == u64::MAX { None }
                                   else { Some(now + Duration::from_millis(hard_ms)) };

        let result = find_best_move(
            &self.board, max_depth, self.ply_from_start, &mut self.state,
        );

        let best = result.best_move
            .map(|m| m.to_string())
            .unwrap_or_else(|| "0000".to_string());

        println!("bestmove {}", best);
        io::stdout().flush().ok();
    }

    fn cmd_eval(&mut self) {
        let t = evaluate_trace(&self.board, Some(&self.state.personality));
        let stm = if t.side_to_move == Color::White { "White" } else { "Black" };
        fn fmt(v: i32) -> String { format!("{:+}", v) }

        println!("info string ┌──────────────────────────────────────────────────────┐");
        println!("info string │  Karpovian Eval  (side to move: {:<6})  Phase: {}%mg │", stm, t.phase_pct);
        println!("info string ├──────────────────────┬──────────┬──────────┬─────────┤");
        println!("info string │  Term                │  White   │  Black   │   Net   │");
        println!("info string ├──────────────────────┼──────────┼──────────┼─────────┤");

        let rows: &[(&str, i32, i32)] = &[
            ("Material + PST",   t.material_w,     t.material_b),
            ("King Safety",      t.king_safety_w,  t.king_safety_b),
            ("King Tropism",     t.king_tropism_w, t.king_tropism_b),
            ("Piece Activity",   t.activity_w,     t.activity_b),
            ("Pawn Structure",   t.pawn_w,         t.pawn_b),
            ("Sac Compensation", t.sac_comp_w,     t.sac_comp_b),
        ];
        for (label, w, b) in rows {
            println!("info string │  {:<20}│  {:>6}  │  {:>6}  │  {:>6} │",
                label, fmt(*w), fmt(*b), fmt(*w - *b));
        }
        println!("info string ├──────────────────────┴──────────┴──────────┼─────────┤");
        if t.mopup != 0 {
            println!("info string │  Mopup (endgame net)                        │  {:>6} │", fmt(t.mopup));
        }
        if t.ocb_shave != 0 {
            println!("info string │  OCB Draw Scaling                           │  {:>6} │", fmt(t.ocb_shave));
        }
        println!("info string │  Tempo                                      │  {:>6} │", fmt(t.tempo));
        println!("info string ├─────────────────────────────────────────────┼─────────┤");
        println!("info string │  FINAL (side to move)                       │  {:>6} │", fmt(t.total));
        println!("info string └─────────────────────────────────────────────┴─────────┘");
        io::stdout().flush().ok();
    }

    fn cmd_result(&mut self, line: &str) {
        // UCI result format: "result 1-0", "result 0-1", "result 1/2-1/2"
        let result_str = line.split_whitespace().nth(1).unwrap_or("*");
        let engine_is_white = self.board.side_to_move() == chess::Color::Black;
        self.state.personality.finish_game(result_str, engine_is_white);
    }

    fn cmd_bench(&mut self, line: &str) {
        // bench [depth]   — default depth 12
        let depth = line.split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(12);
        let result = run_bench(&mut self.state, depth);
        print_bench_result(&result);
        io::stdout().flush().ok();
    }
}
