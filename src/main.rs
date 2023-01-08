// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — Entry Point
//
//  Protocol auto-detection:
//    First command = "uci"     → UCI mode    (for Arena/Cutechess/Lichess-bot)
//    First command = "xboard"  → WinBoard/XBoard CECP v2 mode
//
//  Command-line options (set in Arena's "Command Line Parameters" field):
//    --style Karpov|Tal|Petrosian|Fischer   pre-select playing style
//    --hash  N                              hash table size in MB
//    --elo   N                              target ELO (enables strength limiting)
// ─────────────────────────────────────────────────────────────────────────────

mod bench;
mod book;
mod eval;
mod personality;
mod san;
mod search;
mod time_model;
mod tables;
mod tt;
mod uci;
mod winboard;

use std::io::{self, BufRead};
use personality::Style;

// ── Parse command-line arguments ──────────────────────────────────────────────

struct CliArgs {
    style: Option<Style>,
    hash_mb: Option<usize>,
    elo: Option<i32>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = CliArgs { style: None, hash_mb: None, elo: None };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--style" => {
                if let Some(v) = args.get(i + 1) {
                    out.style = match v.as_str() {
                        "Tal"       => Some(Style::Tal),
                        "Petrosian" => Some(Style::Petrosian),
                        "Fischer"   => Some(Style::Fischer),
                        _           => Some(Style::Karpov),
                    };
                    i += 2;
                } else { i += 1; }
            }
            "--hash" => {
                if let Some(v) = args.get(i + 1) {
                    out.hash_mb = v.parse().ok();
                    i += 2;
                } else { i += 1; }
            }
            "--elo" => {
                if let Some(v) = args.get(i + 1) {
                    out.elo = v.parse().ok();
                    i += 2;
                } else { i += 1; }
            }
            _ => { i += 1; }
        }
    }
    out
}

fn main() {
    let cli = parse_args();

    // Peek at the first non-empty line to determine protocol.
    // The lock is held inside a block so it is dropped before
    // the chosen engine's run() tries to acquire stdin again.
    let first = {
        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();
        loop {
            match lines.next() {
                None => return,
                Some(Ok(l)) => {
                    let l = l.trim().to_string();
                    if !l.is_empty() { break l; }
                }
                Some(Err(_)) => return,
            }
        }
        // `lines` and `stdin` drop here, releasing the lock
    };

    match first.split_whitespace().next().unwrap_or("") {
        "xboard" => {
            let mut engine = winboard::WinboardEngine::new();
            if let Some(s)  = cli.style   { engine.state.personality.style = s; }
            if let Some(mb) = cli.hash_mb { engine.state.tt = std::sync::Arc::new(crate::tt::TranspositionTable::new(mb)); }
            if let Some(elo) = cli.elo    {
                engine.state.personality.limit_strength = true;
                engine.state.personality.target_elo = elo.clamp(1000, personality::ENGINE_ELO);
            }
            engine.run();
        }
        _ => {
            // UCI (default) — feed the first line back through the handler
            let mut engine = uci::UciEngine::new();
            if let Some(s)  = cli.style   { engine.state.personality.style = s; }
            if let Some(mb) = cli.hash_mb { engine.state.tt = std::sync::Arc::new(crate::tt::TranspositionTable::new(mb)); }
            if let Some(elo) = cli.elo    {
                engine.state.personality.limit_strength = true;
                engine.state.personality.target_elo = elo.clamp(1000, personality::ENGINE_ELO);
            }
            engine.handle_first(first);
            engine.run();
        }
    }
}
