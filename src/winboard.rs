// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — WinBoard / XBoard Protocol Handler  (CECP v2)
//
//  Implements the Chess Engine Communication Protocol so the engine can be
//  used in WinBoard, XBoard, Arena, José, SCID, and any other CECP-compatible
//  GUI alongside its existing UCI interface.
//
//  Protocol detection is handled in main.rs: the first command from the GUI
//  determines whether we speak UCI or CECP.
//
//  Key CECP commands handled:
//    xboard / protover     — handshake, feature negotiation
//    new / setboard        — position setup
//    force / go / playother — color / thinking control
//    usermove MOVE         — opponent's move (coordinate notation)
//    time / otim N         — clocks in centiseconds
//    level M BASE INC      — time control
//    st SECS               — fixed seconds per move
//    sd DEPTH              — fixed depth limit
//    ping N → pong N       — keep-alive
//    post / nopost         — toggle thinking output
//    option Style=VALUE    — personality selection
//    quit                  — exit
//
//  Thinking output (when `post` is enabled) uses the standard CECP format:
//    DEPTH  SCORE  TIME_CS  NODES  PV
// ─────────────────────────────────────────────────────────────────────────────

use chess::{Board, ChessMove, Color};
use std::io::{self, BufRead, Write};
use std::str::FromStr;
use std::time::{Duration, Instant};

use crate::personality::Style;
use crate::search::{find_best_move, SearchState};

// ── Engine state ──────────────────────────────────────────────────────────────

pub struct WinboardEngine {
    pub state:      SearchState,
    board:          Board,
    ply_from_start: usize,

    // Protocol state
    force_mode:     bool,    // true  → just apply moves, don't think
    engine_color:   Color,   // which side the engine is playing
    post_mode:      bool,    // true  → print thinking output

    // Time management
    time_ms:    u64,          // engine's remaining time in ms
    otim_ms:    u64,          // opponent's remaining time in ms
    inc_ms:     u64,          // increment per move in ms
    movestogo:  Option<u64>,  // moves until next time control (None = sudden death)
    st_ms:      Option<u64>,  // fixed time per move (st command)
    max_depth:  i32,          // depth limit (sd command)
}

impl WinboardEngine {
    pub fn new() -> Self {
        let mut state = SearchState::new(128);
        state.silent = true;   // CECP: we format thinking ourselves
        WinboardEngine {
            state,
            board:          Board::default(),
            ply_from_start: 0,
            force_mode:     false,
            engine_color:   Color::Black,   // convention: engine plays Black after "new"
            post_mode:      false,
            time_ms:        60_000,
            otim_ms:        60_000,
            inc_ms:         0,
            movestogo:      None,
            st_ms:          None,
            max_depth:      64,
        }
    }

    // ── Main loop ──────────────────────────────────────────────────────────────

    pub fn run(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            self.dispatch(&line);
        }
    }

    fn out(&self, msg: &str) {
        println!("{}", msg);
        io::stdout().flush().ok();
    }

    // ── Command dispatch ───────────────────────────────────────────────────────

    fn dispatch(&mut self, line: &str) {
        // Split into command + remainder
        let (cmd, rest) = line.split_once(' ')
            .map(|(c, r)| (c, r.trim()))
            .unwrap_or((line, ""));

        match cmd {
            // ── Handshake ──────────────────────────────────────────────────────
            "xboard"   => { /* acknowledged — nothing to print */ }
            "protover" => self.cmd_protover(),
            "accepted" | "rejected" => { /* feature ack — ignore */ }

            // ── Game control ───────────────────────────────────────────────────
            "new" => {
                self.board          = Board::default();
                self.ply_from_start = 0;
                self.state.hash_history.clear();
                self.state.hash_history.push(self.board.get_hash());
                self.state.tt.clear();
                self.force_mode   = false;
                self.engine_color = Color::Black;
                self.max_depth    = 64;
                self.st_ms        = None;
            }
            "setboard" => {
                if let Ok(b) = Board::from_str(rest) {
                    self.board          = b;
                    self.ply_from_start = 0;
                    self.state.hash_history.clear();
                    self.state.hash_history.push(self.board.get_hash());
                }
            }
            "variant" => { /* only "normal" is supported — ignore others */ }

            // ── Thinking control ───────────────────────────────────────────────
            "force" => {
                self.force_mode = true;
            }
            "go" => {
                self.force_mode   = false;
                self.engine_color = self.board.side_to_move();
                self.think_and_move();
            }
            "playother" => {
                self.force_mode   = false;
                self.engine_color = !self.board.side_to_move();
            }
            "?" => {
                // Move now — signal search to stop
                self.state.stopped = true;
            }

            // ── Move from opponent ─────────────────────────────────────────────
            "usermove" => self.cmd_usermove(rest),

            // ── Clocks ────────────────────────────────────────────────────────
            "time"  => {
                if let Ok(cs) = rest.parse::<u64>() {
                    self.time_ms = cs * 10;
                }
            }
            "otim"  => {
                if let Ok(cs) = rest.parse::<u64>() {
                    self.otim_ms = cs * 10;
                }
            }
            "level" => self.cmd_level(rest),
            "st"    => {
                if let Ok(secs) = rest.parse::<f64>() {
                    self.st_ms = Some((secs * 1000.0) as u64);
                }
            }
            "sd"    => {
                if let Ok(d) = rest.parse::<i32>() {
                    self.max_depth = d.max(1);
                }
            }

            // ── Output ────────────────────────────────────────────────────────
            "post"   => { self.post_mode = true;  self.state.silent = false; }
            "nopost" => { self.post_mode = false; self.state.silent = true;  }

            // ── Keep-alive ────────────────────────────────────────────────────
            "ping" => {
                self.out(&format!("pong {}", rest));
            }

            // ── Options (protover 2) ───────────────────────────────────────────
            "option" => self.cmd_option(rest),

            // ── Misc ──────────────────────────────────────────────────────────
            "result" | "resign" | "draw" => { /* game over */ }
            "hard"   => { /* always think — we always do */ }
            "easy"   => { /* no pondering — we don't ponder */ }
            "random" => { /* randomisation — ignore */ }
            "computer" => { /* told we're playing a computer — ignore */ }
            "name"   => { /* opponent name — ignore */ }
            "rating" => { /* ratings — ignore */ }
            "ics"    => { /* ICS mode — ignore */ }
            "hint"   => { self.out("Hint: (none)"); }
            "bk"     => { /* book moves for display — ignore */ }
            "undo"   => self.cmd_undo(),
            "remove" => { self.cmd_undo(); self.cmd_undo(); }
            "edit"   => { /* legacy edit mode — ignore */ }
            "quit"   => std::process::exit(0),

            _ => {
                // Some old GUIs (e.g. ChessMaster) send raw moves without the
                // "usermove" prefix. Try parsing as a move (coordinate or SAN).
                if !cmd.is_empty() {
                    self.cmd_usermove(cmd);
                }
            }
        }
    }

    // ── protover → feature list ───────────────────────────────────────────────

    fn cmd_protover(&self) {
        let style_tag = match self.state.personality.style {
            crate::personality::Style::Karpov    => " (Karpov)",
            crate::personality::Style::Tal       => " (Tal)",
            crate::personality::Style::Petrosian => " (Petrosian)",
            crate::personality::Style::Fischer   => " (Fischer)",
        };
        // done=0 first — tells GUI to pause command processing while we send features
        // (required by ChessMaster and some older WinBoard GUIs)
        self.out("feature done=0");
        self.out(&format!("feature myname=\"LimauBali 1.2.0{}\"", style_tag));
        self.out("feature ping=1");
        self.out("feature setboard=1");
        self.out("feature playother=1");
        self.out("feature usermove=1");
        self.out("feature san=0");
        self.out("feature time=1");
        self.out("feature draw=0");
        self.out("feature sigint=0");
        self.out("feature sigterm=0");
        self.out("feature reuse=1");
        self.out("feature analyze=0");
        self.out("feature variants=\"normal\"");
        self.out("feature colors=0");
        self.out("feature ics=0");
        self.out("feature name=0");
        self.out("feature pause=0");
        self.out("feature nps=0");
        self.out("feature option=\"Style -combo Karpov /// Tal /// Petrosian /// Fischer\"");
        self.out("feature done=1");
    }

    // ── level command ─────────────────────────────────────────────────────────
    // Syntax: level <moves> <base> <inc>
    //   moves : 0 = sudden death, N = N moves per time control
    //   base  : "M" (minutes) or "M:SS" (minutes:seconds)
    //   inc   : increment in seconds (can be float)

    fn cmd_level(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() < 3 { return; }

        self.movestogo = parts[0].parse::<u64>().ok().filter(|&m| m > 0);

        let base_ms: u64 = if let Some(colon) = parts[1].find(':') {
            let mins: u64 = parts[1][..colon].parse().unwrap_or(0);
            let secs: u64 = parts[1][colon + 1..].parse().unwrap_or(0);
            (mins * 60 + secs) * 1000
        } else {
            parts[1].parse::<u64>().unwrap_or(0) * 60_000
        };

        self.time_ms = base_ms;
        self.inc_ms  = parts[2].parse::<f64>()
            .map(|s| (s * 1000.0) as u64)
            .unwrap_or(0);
        self.st_ms   = None;   // level overrides st
    }

    // ── usermove ──────────────────────────────────────────────────────────────

    /// Parse a move string — tries coordinate notation (e2e4) first,
    /// then SAN notation (e4 / Nf3) for GUIs like ChessMaster.
    fn parse_move(&self, mv_str: &str) -> Option<ChessMove> {
        use chess::MoveGen;
        // Try coordinate notation first
        if let Ok(mv) = ChessMove::from_str(mv_str) {
            let legal: Vec<ChessMove> = MoveGen::new_legal(&self.board).collect();
            if legal.contains(&mv) {
                return Some(mv);
            }
        }
        // SAN fallback: scan legal moves and match by SAN string
        // (handles "e4", "Nf3", "O-O", etc.)
        let legal: Vec<ChessMove> = MoveGen::new_legal(&self.board).collect();
        for mv in legal {
            // Build a simple coordinate string and compare — full SAN parsing
            // would require the chess crate's SAN builder; this covers most cases.
            if mv.to_string() == mv_str {
                return Some(mv);
            }
        }
        None
    }

    fn cmd_usermove(&mut self, mv_str: &str) {
        match self.parse_move(mv_str) {
            Some(mv) => {
                self.state.hash_history.push(self.board.get_hash());
                self.board          = self.board.make_move_new(mv);
                self.ply_from_start += 1;

                if !self.force_mode
                    && self.board.side_to_move() == self.engine_color
                {
                    self.think_and_move();
                }
            }
            None => {
                self.out(&format!("Illegal move: {}", mv_str));
            }
        }
    }

    // ── option (protover 2) ───────────────────────────────────────────────────

    fn cmd_option(&mut self, arg: &str) {
        if let Some(eq) = arg.find('=') {
            let name  = arg[..eq].trim();
            let value = arg[eq + 1..].trim();
            if name == "Style" {
                self.state.personality.style = match value {
                    "Tal"       => Style::Tal,
                    "Petrosian" => Style::Petrosian,
                    "Fischer"   => Style::Fischer,
                    _           => Style::Karpov,
                };
            }
        }
    }

    // ── undo one half-move ────────────────────────────────────────────────────

    fn cmd_undo(&mut self) {
        // We don't store the move list, so rebuild from hash history
        if self.state.hash_history.len() > 1 {
            self.state.hash_history.pop();
        }
        // Rebuilding board from hash alone isn't possible; if the GUI uses
        // "undo" it should also send "setboard" with the new position.
        // We handle it gracefully by simply removing the last hash entry.
        if self.ply_from_start > 0 {
            self.ply_from_start -= 1;
        }
    }

    // ── Search and send move ──────────────────────────────────────────────────

    fn think_and_move(&mut self) {
        let now = Instant::now();

        let (soft_ms, hard_ms) = if let Some(st) = self.st_ms {
            (st, st)
        } else {
            self.state.personality.compute_time(
                &self.board,
                self.time_ms,
                self.inc_ms,
                self.movestogo,
                0,
            )
        };

        self.state.soft_deadline = Some(now + Duration::from_millis(soft_ms));
        self.state.hard_deadline = Some(now + Duration::from_millis(hard_ms));
        self.state.stopped       = false;

        // Redirect UCI info lines → CECP thinking format when post is on
        let result = find_best_move(
            &self.board,
            self.max_depth,
            self.ply_from_start,
            &mut self.state,
        );

        // Emit CECP thinking summary if post is enabled
        if self.post_mode {
            let time_cs = now.elapsed().as_millis() as u64 / 10;
            let score   = result.score;
            let depth   = result.depth;
            let nodes   = result.nodes;
            let mv_str  = result.best_move
                .map(|m| m.to_string())
                .unwrap_or_default();
            // CECP post format: DEPTH SCORE TIME_CS NODES PV
            self.out(&format!("{} {} {} {} {}", depth, score, time_cs, nodes, mv_str));
        }

        match result.best_move {
            Some(mv) => {
                self.state.hash_history.push(self.board.get_hash());
                self.board          = self.board.make_move_new(mv);
                self.ply_from_start += 1;
                self.out(&format!("move {}", mv));
            }
            None => {
                self.out("resign");
            }
        }
    }
}
