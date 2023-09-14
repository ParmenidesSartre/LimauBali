// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — Self-Teaching Time Management Model
//
//  The engine learns from its own games which time allocations are productive.
//  After every game it updates 4 parameters:
//    phase_scale[opening]   — multiplier for moves 1-15
//    phase_scale[midgame]   — multiplier for moves 16-35
//    phase_scale[endgame]   — multiplier for moves 36+
//    instability_bonus      — extra time when score fluctuates between iterations
//
//  Learning signal: score_gain / time_spent per phase per game.
//    High efficiency (cp gained per ms) → time is being used well → scale up
//    Low efficiency (cp gained per ms) → time is being wasted → scale down
//    Game result also nudges scales: winning games reinforce their time patterns.
//
//  Model is persisted to time_model.cfg next to the engine executable.
//  After ~50 games the model converges to stable values.
// ─────────────────────────────────────────────────────────────────────────────

use std::fs;
use std::path::PathBuf;

// ── Phase ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase { Opening, Midgame, Endgame }

impl Phase {
    pub fn from_piece_count(pieces: u32) -> Self {
        if pieces > 10 { Phase::Opening }
        else if pieces > 4 { Phase::Midgame }
        else { Phase::Endgame }
    }
    fn idx(self) -> usize {
        match self { Phase::Opening => 0, Phase::Midgame => 1, Phase::Endgame => 2 }
    }
}

// ── Per-move record ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MoveRecord {
    pub phase:        Phase,
    pub time_ms:      u64,   // actual time spent on this move
    pub score_gain:   i32,   // |score_final - score_at_depth_1| — how much deeper search helped
    pub was_unstable: bool,  // score changed >30cp between depth iterations
}

// ── Time model ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TimeModel {
    /// Learned multipliers applied on top of base time allocation.
    pub phase_scale:       [f32; 3],  // [opening, midgame, endgame]
    /// Extra fraction of base time added when score is unstable.
    pub instability_bonus: f32,
    /// Total games trained on.
    pub games_played:      u32,
    /// Move records for the current game (cleared after update).
    pub moves:             Vec<MoveRecord>,
}

impl TimeModel {
    pub fn new() -> Self {
        // Start at neutral — pure experience will shift these
        TimeModel {
            phase_scale:       [1.0, 1.0, 1.0],
            instability_bonus: 0.40,
            games_played:      0,
            moves:             Vec::new(),
        }
    }

    /// Multiplier to apply for the given piece count.
    pub fn phase_scale_for(&self, piece_count: u32) -> f32 {
        self.phase_scale[Phase::from_piece_count(piece_count).idx()].clamp(0.30, 2.50)
    }

    /// Extra fraction to add when the position is unstable.
    pub fn instability_scale(&self) -> f32 {
        self.instability_bonus.clamp(0.0, 1.5)
    }

    /// Called once per move during a game.
    pub fn record_move(&mut self, phase: Phase, time_ms: u64,
                       score_gain: i32, was_unstable: bool) {
        self.moves.push(MoveRecord { phase, time_ms, score_gain, was_unstable });
    }

    // ── Learning update ───────────────────────────────────────────────────────

    /// Update the model after a game ends.
    /// `result` — 1.0 = win, 0.5 = draw, 0.0 = loss (from engine's perspective).
    pub fn update(&mut self, result: f32) {
        if self.moves.is_empty() { return; }

        // Small learning rate — gentle, stable updates
        const LR: f32        = 0.015;
        // Target: ~5 centipawns gained per second of thinking
        const TARGET_CP_S: f32 = 5.0;

        // ── Phase scale updates ───────────────────────────────────────────────
        for idx in 0..3 {
            let phase_moves: Vec<&MoveRecord> = self.moves.iter()
                .filter(|m| m.phase.idx() == idx)
                .collect();
            if phase_moves.is_empty() { continue; }

            let total_ms:   f32 = phase_moves.iter().map(|m| m.time_ms as f32).sum::<f32>().max(1.0);
            let total_gain: f32 = phase_moves.iter().map(|m| m.score_gain.abs() as f32).sum();

            // Efficiency: cp gained per second of thinking
            let efficiency = total_gain / (total_ms / 1000.0);

            // Gradient: how far are we from the target efficiency?
            let gradient = (efficiency - TARGET_CP_S) / TARGET_CP_S.max(0.001);
            let delta = (gradient * LR).clamp(-0.05, 0.05);

            // Game result reinforces time patterns from winning games
            let result_bonus = (result - 0.5) * LR * 0.5;

            self.phase_scale[idx] = (self.phase_scale[idx] + delta + result_bonus)
                .clamp(0.30, 2.50);
        }

        // ── Instability bonus update ──────────────────────────────────────────
        // Compare average score_gain for unstable vs stable moves.
        // If unstable moves gain more → detecting real instability → increase bonus.
        // If no difference → we're over-extending on noise → decrease bonus.
        let unstable: Vec<_> = self.moves.iter().filter(|m|  m.was_unstable).collect();
        let stable:   Vec<_> = self.moves.iter().filter(|m| !m.was_unstable).collect();

        if !unstable.is_empty() && !stable.is_empty() {
            let avg_u = unstable.iter().map(|m| m.score_gain.abs() as f32).sum::<f32>()
                        / unstable.len() as f32;
            let avg_s = stable.iter().map(|m| m.score_gain.abs() as f32).sum::<f32>()
                        / stable.len() as f32;

            let ratio = avg_u / avg_s.max(0.001);
            if ratio > 1.2 {
                // Unstable moves genuinely needed more search → keep/grow bonus
                self.instability_bonus = (self.instability_bonus + LR).clamp(0.0, 1.5);
            } else if ratio < 0.8 {
                // Instability signal is noise → shrink bonus
                self.instability_bonus = (self.instability_bonus - LR).clamp(0.0, 1.5);
            }
        }

        self.games_played += 1;
        self.moves.clear();
        self.save();
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    fn config_path() -> Option<PathBuf> {
        std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|d| d.join("time_model.cfg")))
    }

    pub fn load() -> Self {
        let path = match Self::config_path() { Some(p) => p, None => return Self::new() };
        let text = match fs::read_to_string(&path) { Ok(t) => t, Err(_) => return Self::new() };

        let mut m = Self::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() { continue; }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim();
                match k.trim() {
                    "opening_scale"     => { if let Ok(f) = v.parse::<f32>() { m.phase_scale[0] = f.clamp(0.30, 2.50); } }
                    "midgame_scale"     => { if let Ok(f) = v.parse::<f32>() { m.phase_scale[1] = f.clamp(0.30, 2.50); } }
                    "endgame_scale"     => { if let Ok(f) = v.parse::<f32>() { m.phase_scale[2] = f.clamp(0.30, 2.50); } }
                    "instability_bonus" => { if let Ok(f) = v.parse::<f32>() { m.instability_bonus = f.clamp(0.0, 1.5); } }
                    "games_played"      => { if let Ok(n) = v.parse::<u32>()  { m.games_played = n; } }
                    _ => {}
                }
            }
        }
        m
    }

    pub fn save(&self) {
        let path = match Self::config_path() { Some(p) => p, None => return };
        let text = format!(
            "# Pantheon Self-Teaching Time Model\n\
             # Auto-generated after every game — do not edit manually\n\
             # Trained on {} games\n\
             #\n\
             # phase_scale: multiplier applied to base time per game phase\n\
             # instability_bonus: extra fraction when score fluctuates between depths\n\
             #\n\
             opening_scale     = {:.4}\n\
             midgame_scale     = {:.4}\n\
             endgame_scale     = {:.4}\n\
             instability_bonus = {:.4}\n\
             games_played      = {}\n",
            self.games_played,
            self.phase_scale[0],
            self.phase_scale[1],
            self.phase_scale[2],
            self.instability_bonus,
            self.games_played,
        );
        let _ = fs::write(&path, text);
    }
}
