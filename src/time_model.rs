// ─────────────────────────────────────────────────────────────────────────────
//  LimauBali — Self-Teaching Time Management Model  (v2)
//
//  Improvements over v1:
//    • Four separate parameter sets — one per time-control bucket
//      (Bullet / Blitz / Rapid / Classical).  The engine auto-detects the
//      active bucket from the clock on every move.
//    • Dynamic ceiling/floor per bucket — bullet is capped at 1.2× so the
//      engine never flags; classical can stretch to 2.5×.
//    • Decaying learning rate — fast early convergence, stable later.
//      lr = 0.03 / (1 + games * 0.02)  →  ~0.030 at game 1, ~0.010 at game 100
//    • Flag penalty — if the clock fell below 3 s during a game, all active
//      phase scales are pulled back by 15% after the game ends.
//    • Per-bucket efficiency target — bullet should make faster decisions
//      (higher cp/s target) while classical can think more deeply (lower target).
//
//  Config format  (time_model.cfg, next to executable):
//    [bullet]
//    opening_scale     = 1.0000
//    midgame_scale     = 1.0000
//    endgame_scale     = 1.0000
//    instability_bonus = 0.4000
//    games_played      = 0
//    [blitz]
//    ...
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

// ── Time-control bucket ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TimeControl { Bullet, Blitz, Rapid, Classical }

impl TimeControl {
    /// Auto-detect from remaining clock at the start of compute_time.
    pub fn detect(our_time_ms: u64) -> Self {
        match our_time_ms {
            0..=179_999       => Self::Bullet,    // < 3 min
            180_000..=599_999  => Self::Blitz,    // 3–10 min
            600_000..=1_799_999 => Self::Rapid,   // 10–30 min
            _                 => Self::Classical, // > 30 min
        }
    }

    /// Maximum allowed phase_scale — tighter for faster time controls.
    pub fn ceiling(self) -> f32 {
        match self {
            Self::Bullet    => 1.20,
            Self::Blitz     => 1.80,
            Self::Rapid     => 2.20,
            Self::Classical => 2.50,
        }
    }

    pub fn floor(self) -> f32 { 0.30 }

    /// Maximum instability_bonus — bullet can't afford long extensions.
    pub fn instability_ceiling(self) -> f32 {
        match self {
            Self::Bullet    => 0.50,
            Self::Blitz     => 0.90,
            Self::Rapid     => 1.20,
            Self::Classical => 1.50,
        }
    }

    /// Target efficiency in centipawns gained per second of thinking.
    /// Bullet needs high cp/s (fast, accurate); classical can afford low cp/s (deep).
    pub fn target_cp_s(self) -> f32 {
        match self {
            Self::Bullet    => 15.0,
            Self::Blitz     =>  8.0,
            Self::Rapid     =>  5.0,
            Self::Classical =>  3.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bullet    => "bullet",
            Self::Blitz     => "blitz",
            Self::Rapid     => "rapid",
            Self::Classical => "classical",
        }
    }

    pub fn idx(self) -> usize {
        match self {
            Self::Bullet    => 0,
            Self::Blitz     => 1,
            Self::Rapid     => 2,
            Self::Classical => 3,
        }
    }

    fn from_idx(i: usize) -> Self {
        match i { 0 => Self::Bullet, 1 => Self::Blitz, 2 => Self::Rapid, _ => Self::Classical }
    }
}

// ── Per-bucket learned parameters ────────────────────────────────────────────

#[derive(Clone)]
pub struct BucketParams {
    pub phase_scale:       [f32; 3],  // [opening, midgame, endgame]
    pub instability_bonus: f32,
    pub games_played:      u32,
}

impl BucketParams {
    fn new() -> Self {
        BucketParams { phase_scale: [1.0, 1.0, 1.0], instability_bonus: 0.40, games_played: 0 }
    }
}

// ── Per-move record ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MoveRecord {
    pub phase:        Phase,
    pub time_ms:      u64,
    pub score_gain:   i32,
    pub was_unstable: bool,
}

// ── Time model ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TimeModel {
    pub buckets:   [BucketParams; 4],  // indexed by TimeControl::idx()
    pub active_tc: TimeControl,
    pub near_flag: bool,               // clock fell below danger threshold this game
    pub moves:     Vec<MoveRecord>,
}

impl TimeModel {
    pub fn new() -> Self {
        TimeModel {
            buckets:   [
                BucketParams::new(),
                BucketParams::new(),
                BucketParams::new(),
                BucketParams::new(),
            ],
            active_tc: TimeControl::Blitz,
            near_flag: false,
            moves:     Vec::new(),
        }
    }

    // ── Active-bucket helpers ─────────────────────────────────────────────────

    /// Called at the start of every compute_time so the model tracks which
    /// bucket is being played.  Safe to call every move — cheap comparison.
    pub fn set_tc(&mut self, our_time_ms: u64) {
        self.active_tc = TimeControl::detect(our_time_ms);
    }

    /// Mark that the clock dropped into danger territory this game.
    /// Triggers a 15% penalty on phase scales during update().
    pub fn mark_near_flag(&mut self) {
        self.near_flag = true;
    }

    fn active(&self) -> &BucketParams {
        &self.buckets[self.active_tc.idx()]
    }

    /// Phase multiplier clamped to the active bucket's ceiling.
    pub fn phase_scale_for(&self, piece_count: u32) -> f32 {
        let tc  = self.active_tc;
        let idx = Phase::from_piece_count(piece_count).idx();
        self.active().phase_scale[idx].clamp(tc.floor(), tc.ceiling())
    }

    /// Instability bonus clamped to the active bucket's ceiling.
    pub fn instability_scale(&self) -> f32 {
        self.active().instability_bonus.clamp(0.0, self.active_tc.instability_ceiling())
    }

    pub fn record_move(&mut self, phase: Phase, time_ms: u64,
                       score_gain: i32, was_unstable: bool) {
        self.moves.push(MoveRecord { phase, time_ms, score_gain, was_unstable });
    }

    // ── Learning update ───────────────────────────────────────────────────────

    /// Called after every game ends.  Updates only the active bucket.
    /// `result` — 1.0 = win, 0.5 = draw, 0.0 = loss (engine's perspective).
    pub fn update(&mut self, result: f32) {
        if self.moves.is_empty() { return; }

        let tc  = self.active_tc;
        let b   = &mut self.buckets[tc.idx()];

        // Decaying learning rate: aggressive early, conservative once converged.
        // 0.030 at game 0 → 0.015 at game 50 → 0.010 at game 100
        let lr: f32 = 0.03 / (1.0 + b.games_played as f32 * 0.02);

        let target_cp_s = tc.target_cp_s();
        let ceiling     = tc.ceiling();
        let floor       = tc.floor();
        let inst_ceil   = tc.instability_ceiling();

        // ── Phase scale updates ───────────────────────────────────────────────
        for idx in 0..3usize {
            let phase_moves: Vec<&MoveRecord> = self.moves.iter()
                .filter(|m| m.phase.idx() == idx)
                .collect();
            if phase_moves.is_empty() { continue; }

            let total_ms:   f32 = phase_moves.iter().map(|m| m.time_ms as f32).sum::<f32>().max(1.0);
            let total_gain: f32 = phase_moves.iter().map(|m| m.score_gain.abs() as f32).sum();

            // Efficiency: cp gained per second of thinking
            let efficiency   = total_gain / (total_ms / 1000.0);
            let gradient     = (efficiency - target_cp_s) / target_cp_s.max(0.001);
            let delta        = (gradient * lr).clamp(-0.08, 0.08);
            let result_nudge = (result - 0.5) * lr * 0.4;

            b.phase_scale[idx] = (b.phase_scale[idx] + delta + result_nudge)
                .clamp(floor, ceiling);
        }

        // ── Flag penalty ──────────────────────────────────────────────────────
        // If we nearly flagged, pull all scales and instability back by 15%.
        if self.near_flag {
            for s in b.phase_scale.iter_mut() {
                *s = (*s * 0.85).clamp(floor, ceiling);
            }
            b.instability_bonus = (b.instability_bonus * 0.85).clamp(0.0, inst_ceil);
        }

        // ── Instability bonus update ──────────────────────────────────────────
        let unstable: Vec<_> = self.moves.iter().filter(|m|  m.was_unstable).collect();
        let stable:   Vec<_> = self.moves.iter().filter(|m| !m.was_unstable).collect();

        if !unstable.is_empty() && !stable.is_empty() {
            let avg_u = unstable.iter().map(|m| m.score_gain.abs() as f32).sum::<f32>()
                        / unstable.len() as f32;
            let avg_s = stable.iter().map(|m| m.score_gain.abs() as f32).sum::<f32>()
                        / stable.len() as f32;

            let ratio = avg_u / avg_s.max(0.001);
            let delta = if ratio > 1.2 { lr } else if ratio < 0.8 { -lr } else { 0.0 };
            b.instability_bonus = (b.instability_bonus + delta).clamp(0.0, inst_ceil);
        }

        b.games_played += 1;
        self.moves.clear();
        self.near_flag = false;
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

        let mut model = Self::new();
        let mut current_idx: Option<usize> = None;
        let mut has_sections = false;

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() { continue; }

            // Section header: [bullet], [blitz], [rapid], [classical]
            if line.starts_with('[') && line.ends_with(']') {
                has_sections = true;
                current_idx = match &line[1..line.len()-1] {
                    "bullet"    => Some(0),
                    "blitz"     => Some(1),
                    "rapid"     => Some(2),
                    "classical" => Some(3),
                    _           => None,
                };
                continue;
            }

            // Fall back: old flat format — load into blitz bucket
            let idx = current_idx.unwrap_or(if has_sections { continue; } else { 1 });
            let b   = &mut model.buckets[idx];
            let tc  = TimeControl::from_idx(idx);

            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim();
                match k.trim() {
                    "opening_scale"     => { if let Ok(f) = v.parse::<f32>() { b.phase_scale[0] = f.clamp(tc.floor(), tc.ceiling()); } }
                    "midgame_scale"     => { if let Ok(f) = v.parse::<f32>() { b.phase_scale[1] = f.clamp(tc.floor(), tc.ceiling()); } }
                    "endgame_scale"     => { if let Ok(f) = v.parse::<f32>() { b.phase_scale[2] = f.clamp(tc.floor(), tc.ceiling()); } }
                    "instability_bonus" => { if let Ok(f) = v.parse::<f32>() { b.instability_bonus = f.clamp(0.0, tc.instability_ceiling()); } }
                    "games_played"      => { if let Ok(n) = v.parse::<u32>()  { b.games_played = n; } }
                    _ => {}
                }
            }
        }
        model
    }

    pub fn save(&self) {
        let path = match Self::config_path() { Some(p) => p, None => return };

        let mut text = String::from(
            "# LimauBali Self-Teaching Time Model  (v2)\n\
             # Auto-generated — do not edit manually\n\
             # Separate learned parameters per time-control bucket.\n\
             # Ceilings:  bullet=1.20  blitz=1.80  rapid=2.20  classical=2.50\n\n"
        );

        for i in 0..4usize {
            let tc = TimeControl::from_idx(i);
            let b  = &self.buckets[i];
            text.push_str(&format!(
                "[{}]  # trained on {} games | ceiling={:.2} | target={:.1} cp/s\n\
                 opening_scale     = {:.4}\n\
                 midgame_scale     = {:.4}\n\
                 endgame_scale     = {:.4}\n\
                 instability_bonus = {:.4}\n\
                 games_played      = {}\n\n",
                tc.name(), b.games_played, tc.ceiling(), tc.target_cp_s(),
                b.phase_scale[0], b.phase_scale[1], b.phase_scale[2],
                b.instability_bonus, b.games_played,
            ));
        }

        let _ = fs::write(&path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TimeControl detection ─────────────────────────────────────────────────

    #[test]
    fn detect_bullet_boundaries() {
        assert_eq!(TimeControl::detect(0),       TimeControl::Bullet);
        assert_eq!(TimeControl::detect(60_000),  TimeControl::Bullet); // 1 min
        assert_eq!(TimeControl::detect(179_999), TimeControl::Bullet); // just under 3 min
    }

    #[test]
    fn detect_blitz_boundaries() {
        assert_eq!(TimeControl::detect(180_000), TimeControl::Blitz); // 3 min exactly
        assert_eq!(TimeControl::detect(300_000), TimeControl::Blitz); // 5 min
        assert_eq!(TimeControl::detect(599_999), TimeControl::Blitz);
    }

    #[test]
    fn detect_rapid_boundaries() {
        assert_eq!(TimeControl::detect(600_000),   TimeControl::Rapid); // 10 min exactly
        assert_eq!(TimeControl::detect(900_000),   TimeControl::Rapid); // 15 min
        assert_eq!(TimeControl::detect(1_799_999), TimeControl::Rapid);
    }

    #[test]
    fn detect_classical_boundaries() {
        assert_eq!(TimeControl::detect(1_800_000), TimeControl::Classical); // 30 min exactly
        assert_eq!(TimeControl::detect(5_400_000), TimeControl::Classical); // 90 min
    }

    // ── Ceiling / floor ordering ──────────────────────────────────────────────

    #[test]
    fn ceilings_increase_with_time_control() {
        assert!(TimeControl::Bullet.ceiling()    < TimeControl::Blitz.ceiling());
        assert!(TimeControl::Blitz.ceiling()     < TimeControl::Rapid.ceiling());
        assert!(TimeControl::Rapid.ceiling()     < TimeControl::Classical.ceiling());
    }

    #[test]
    fn target_cp_s_decreases_with_time_control() {
        assert!(TimeControl::Bullet.target_cp_s()    > TimeControl::Blitz.target_cp_s());
        assert!(TimeControl::Blitz.target_cp_s()     > TimeControl::Rapid.target_cp_s());
        assert!(TimeControl::Rapid.target_cp_s()     > TimeControl::Classical.target_cp_s());
    }

    #[test]
    fn instability_ceilings_increase_with_time_control() {
        assert!(TimeControl::Bullet.instability_ceiling()    < TimeControl::Blitz.instability_ceiling());
        assert!(TimeControl::Blitz.instability_ceiling()     < TimeControl::Rapid.instability_ceiling());
        assert!(TimeControl::Rapid.instability_ceiling()     < TimeControl::Classical.instability_ceiling());
    }

    // ── BucketParams defaults ─────────────────────────────────────────────────

    #[test]
    fn bucket_params_default_values() {
        let b = BucketParams::new();
        assert_eq!(b.phase_scale,       [1.0, 1.0, 1.0]);
        assert_eq!(b.instability_bonus, 0.40);
        assert_eq!(b.games_played,      0);
    }

    // ── set_tc ────────────────────────────────────────────────────────────────

    #[test]
    fn set_tc_selects_correct_bucket() {
        let mut m = TimeModel::new();
        m.set_tc(60_000);      assert_eq!(m.active_tc, TimeControl::Bullet);
        m.set_tc(300_000);     assert_eq!(m.active_tc, TimeControl::Blitz);
        m.set_tc(900_000);     assert_eq!(m.active_tc, TimeControl::Rapid);
        m.set_tc(3_600_000);   assert_eq!(m.active_tc, TimeControl::Classical);
    }

    // ── Clamping ──────────────────────────────────────────────────────────────

    #[test]
    fn phase_scale_clamped_to_bucket_ceiling() {
        let mut m = TimeModel::new();
        m.set_tc(300_000); // blitz, ceiling = 1.80
        m.buckets[TimeControl::Blitz.idx()].phase_scale = [9.9, 9.9, 9.9];
        assert_eq!(m.phase_scale_for(15), TimeControl::Blitz.ceiling());
    }

    #[test]
    fn phase_scale_clamped_to_floor() {
        let mut m = TimeModel::new();
        m.set_tc(300_000);
        m.buckets[TimeControl::Blitz.idx()].phase_scale = [-9.9, -9.9, -9.9];
        assert_eq!(m.phase_scale_for(15), TimeControl::Blitz.floor());
    }

    #[test]
    fn instability_scale_clamped_to_bucket_ceiling() {
        let mut m = TimeModel::new();
        m.set_tc(60_000); // bullet, instability_ceiling = 0.50
        m.buckets[0].instability_bonus = 99.9;
        assert_eq!(m.instability_scale(), TimeControl::Bullet.instability_ceiling());
    }

    // ── Learning update ───────────────────────────────────────────────────────

    #[test]
    fn games_played_increments_after_update() {
        let mut m = TimeModel::new();
        m.set_tc(300_000);
        m.moves.push(MoveRecord { phase: Phase::Opening, time_ms: 1000, score_gain: 5, was_unstable: false });
        m.update(0.5);
        assert_eq!(m.buckets[TimeControl::Blitz.idx()].games_played, 1);
    }

    #[test]
    fn moves_cleared_after_update() {
        let mut m = TimeModel::new();
        m.set_tc(300_000);
        m.moves.push(MoveRecord { phase: Phase::Midgame, time_ms: 500, score_gain: 10, was_unstable: false });
        m.update(0.5);
        assert!(m.moves.is_empty());
    }

    #[test]
    fn update_noop_when_no_moves() {
        let mut m = TimeModel::new();
        m.set_tc(300_000);
        m.update(1.0);
        assert_eq!(m.buckets[TimeControl::Blitz.idx()].games_played, 0);
    }

    #[test]
    fn flag_penalty_reduces_scales() {
        let mut m = TimeModel::new();
        m.set_tc(300_000); // blitz
        let idx = TimeControl::Blitz.idx();
        m.buckets[idx].phase_scale       = [1.5, 1.5, 1.5];
        m.buckets[idx].instability_bonus = 0.80;
        m.near_flag = true;
        m.moves.push(MoveRecord { phase: Phase::Midgame, time_ms: 500, score_gain: 5, was_unstable: false });
        m.update(0.5);
        let b = &m.buckets[idx];
        // 1.5 * 0.85 = 1.275 — all scales should be pulled down
        assert!(b.phase_scale[0]     < 1.5, "opening scale should decrease after flag");
        assert!(b.phase_scale[1]     < 1.5, "midgame scale should decrease after flag");
        assert!(b.phase_scale[2]     < 1.5, "endgame scale should decrease after flag");
        assert!(b.instability_bonus  < 0.8, "instability bonus should decrease after flag");
    }

    #[test]
    fn near_flag_cleared_after_update() {
        let mut m = TimeModel::new();
        m.set_tc(300_000);
        m.near_flag = true;
        m.moves.push(MoveRecord { phase: Phase::Opening, time_ms: 500, score_gain: 1, was_unstable: false });
        m.update(0.5);
        assert!(!m.near_flag);
    }

    #[test]
    fn buckets_are_independent() {
        let mut m = TimeModel::new();
        m.set_tc(300_000); // update blitz
        m.moves.push(MoveRecord { phase: Phase::Midgame, time_ms: 2000, score_gain: 50, was_unstable: false });
        m.update(1.0);
        // Rapid and classical buckets should remain at 0 games
        assert_eq!(m.buckets[TimeControl::Rapid.idx()].games_played,     0);
        assert_eq!(m.buckets[TimeControl::Classical.idx()].games_played, 0);
        assert_eq!(m.buckets[TimeControl::Bullet.idx()].games_played,    0);
    }

    #[test]
    fn decaying_lr_slows_over_time() {
        // LR = 0.03 / (1 + games * 0.02)
        let lr_at_0   = 0.03f32 / (1.0 + 0.0   * 0.02);
        let lr_at_50  = 0.03f32 / (1.0 + 50.0  * 0.02);
        let lr_at_100 = 0.03f32 / (1.0 + 100.0 * 0.02);
        assert!(lr_at_0 > lr_at_50);
        assert!(lr_at_50 > lr_at_100);
        assert!(lr_at_100 > 0.0);
    }

    #[test]
    fn phase_from_piece_count_correct() {
        assert_eq!(Phase::from_piece_count(16), Phase::Opening);  // full board
        assert_eq!(Phase::from_piece_count(11), Phase::Opening);  // just above threshold
        assert_eq!(Phase::from_piece_count(10), Phase::Midgame);
        assert_eq!(Phase::from_piece_count(5),  Phase::Midgame);
        assert_eq!(Phase::from_piece_count(4),  Phase::Endgame);
        assert_eq!(Phase::from_piece_count(0),  Phase::Endgame);
    }
}
