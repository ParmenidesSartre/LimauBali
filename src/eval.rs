// ─────────────────────────────────────────────────────────────────────────────
//  Pantheon — Static Evaluation
//
//  All tunable numbers live in EvalParams.  Preset constructors per master:
//    EvalParams::karpov_style()    — prophylactic, positional squeeze
//    EvalParams::tal_style()       — aggressive, sacrificial
//    EvalParams::petrosian_style() — suffocating, exchange-averse
//    EvalParams::fischer_style()   — technical precision, bishop pair
//
//  To change a personality, edit the numbers in its constructor.
//  To add a new term, add a field here and wire it through eval_inner.
//
//  Evaluation terms:
//    1.  Material + PST (tapered middlegame↔endgame blend)
//    2.  King safety: shelter, open files, zone attack units, tropism
//    3.  Piece activity: mobility, bishop pair, rook files, 7th rank, dev lead
//    4.  Pawn structure: passed, isolated, doubled, backward, storm
//    4f. Knight outposts, rook behind passer, threats, space, blockade
//    4k. Connected pawns, bad bishop
//    5a. Uncastled king penalty (opening/middlegame)
//    5b. Knight centralization (middlegame)
//    5c. Weak square exploitation (middlegame)
//    5d. Passed pawn king proximity (endgame)
//    5e. Protected passed pawn
//    5f. King activity in endgame
//    5g. Rook cuts enemy king off from passer (Tarrasch rule)
//    6.  Sacrifice compensation (down material near enemy king)
//    7.  Endgame mopup (push losing king to corner)
//    8.  OCB draw scaling
//    9.  Tempo bonus
// ─────────────────────────────────────────────────────────────────────────────

use chess::{BitBoard, Board, BoardStatus, Color, Piece, Rank, Square};
use crate::personality::{Personality, Style};
use crate::tables::*;

// ── EvalParams: every tunable number in one place ─────────────────────────────

#[derive(Clone)]
pub struct EvalParams {
    // ── Style multipliers (personality) ──────────────────────────────────────
    /// Multiplier on raw material. < 1.0 = willing to sacrifice (Tal: 0.85)
    pub material_weight:        f32,
    /// Multiplier on all king-attack/tropism terms. > 1.0 = more aggressive (Tal: 2.0)
    pub king_attack_weight:     f32,
    /// Multiplier on pawn storm bonus. > 1.0 = more storming (Tal: 1.8)
    pub pawn_storm_weight:      f32,
    /// Sacrifice compensation: bonus per attacker near enemy king when we're down material.
    /// Set to 0 for Default (no compensation).
    pub sac_bonus:              i32,
    /// How far behind in material (cp) before sac compensation activates.
    pub sac_threshold:          i32,
    /// Extra bonus in sac compensation when enemy king is uncastled (files 2-5).
    pub sac_uncastled_bonus:    i32,

    // ── Tempo ─────────────────────────────────────────────────────────────────
    /// Flat bonus for the side to move.
    pub tempo:                  i32,

    // ── King safety penalties ─────────────────────────────────────────────────
    /// Penalty: no shelter pawn on king's own file (centre file of the 3-file zone).
    pub shelter_center:         i32,
    /// Penalty: no shelter pawn on a file adjacent to the king.
    pub shelter_adjacent:       i32,
    /// Penalty per fully open file (no pawns at all) adjacent to the king.
    pub open_file_king:         i32,
    /// Penalty per semi-open file (enemy pawn only) adjacent to the king.
    pub semi_open_file_king:    i32,
    /// Multiplier on tropism proximity score (raw tropism × this × king_attack_weight).
    pub tropism_scale:          f32,

    // ── Activity bonuses ─────────────────────────────────────────────────────
    /// Bishop pair bonus (both bishops present).
    pub bishop_pair:            i32,
    /// Rook on fully open file (no pawns) bonus.
    pub rook_open_file:         i32,
    /// Rook on semi-open file (no own pawns) bonus.
    pub rook_semi_open:         i32,
    /// Connected rooks (same rank/file, no pieces between) bonus.
    pub connected_rooks:        i32,
    /// Rook on 7th rank bonus (per rook).
    pub rook_seventh:           i32,
    /// Bonus per undeveloped enemy minor piece still on back rank (middlegame only).
    pub dev_lead_per_piece:     i32,
    /// Raw mobility count is divided by this before adding to score.
    pub mobility_divisor:       i32,

    // ── Pawn structure penalties ──────────────────────────────────────────────
    /// Penalty per isolated pawn (no friendly pawns on adjacent files).
    pub isolated_pawn:          i32,
    /// Penalty per extra pawn per file (doubled = 1 extra, tripled = 2 extra, etc.).
    pub doubled_pawn:           i32,
    /// Penalty per backward pawn (can't be supported, faces enemy pawn attacks).
    pub backward_pawn:          i32,

    // ── Outposts ──────────────────────────────────────────────────────────────
    /// Bonus for a knight on an outpost (unreachable by enemy pawns) in the opponent's half.
    pub outpost_knight:             i32,
    /// Extra bonus when that outpost knight is also defended by a friendly pawn.
    pub outpost_knight_supported:   i32,

    // ── Rook coordination ─────────────────────────────────────────────────────
    /// Bonus for a rook on the same file as, and behind, a friendly passed pawn.
    pub rook_behind_passer:         i32,

    // ── Threats ───────────────────────────────────────────────────────────────
    /// Bonus per undefended (hanging) enemy piece we are attacking.
    pub hanging_piece_bonus:        i32,

    // ── Space advantage ───────────────────────────────────────────────────────
    /// Bonus per rank of advancement for central pawns in opponent's territory.
    pub space_bonus:                i32,

    // ── Blockade ─────────────────────────────────────────────────────────────
    /// Bonus for our knight sitting on the stop-square of an opponent's passed pawn.
    /// Petrosian's signature: the knight both halts the passer and gains a permanent outpost.
    pub blockade_knight:            i32,
    /// Bonus for any other piece (not knight) blockading an opponent's passed pawn.
    pub blockade_piece:             i32,

    // ── Pawn chain quality ────────────────────────────────────────────────────
    /// Bonus per connected pawn (has a friendly pawn on an adjacent file, same or nearby rank).
    pub connected_pawn:             i32,
    /// Penalty per own pawn sitting on the same color as our sole bishop (bad bishop).
    pub bad_bishop:                 i32,

    // ── Piece preservation (Petrosian) ────────────────────────────────────────
    /// Bonus per own non-pawn piece still on the board.
    /// Rewards keeping the full army on the board; discourages exchanges/sacs.
    pub piece_count_bonus:          i32,
    /// Flat bonus when own queen is still on the board.
    /// Discourages queen trades — Petrosian almost never traded queens voluntarily.
    pub queen_on_board_bonus:       i32,

    // ── Opening/Middlegame: king safety ───────────────────────────────────────
    /// Penalty when own king is on central files (c–f) in the middlegame (hasn't castled).
    /// Tapered with game phase — most dangerous when many pieces are on the board.
    pub uncastled_king:             i32,

    // ── Middlegame: piece harmony ─────────────────────────────────────────────
    /// Bonus for a knight that is centralized (d/e file, ranks 4-5 for white).
    /// Knight in the center of the board controls many squares and is hard to dislodge.
    pub knight_centralization:      i32,
    /// Bonus per pawn of the opponent's that is on a weak square we permanently control.
    /// A weak square is one that can never be defended by an enemy pawn.
    pub weak_square_bonus:          i32,

    // ── Endgame: king and pawn technique ─────────────────────────────────────
    /// Bonus per square of closeness (7 - dist) of own king to own passed pawn (endgame).
    pub passer_king_prox:           i32,
    /// Bonus per square of distance of enemy king from our passed pawn (endgame).
    pub passer_enemy_king:          i32,
    /// Extra bonus for a passed pawn defended by a friendly pawn (protected passer).
    pub protected_passer:           i32,
    /// Bonus for king centralization in the endgame (per square of distance from corner).
    pub king_activity_eg:           i32,
    /// Bonus for a rook that cuts the enemy king off from our passed pawn (same rank, rook between king and pawn file).
    pub rook_cuts_king:             i32,

    // ── Strategic imbalances (Petrosian) ─────────────────────────────────────
    /// Bonus when we have a knight and opponent has a single bad bishop
    /// (blocked by own pawns on the same color squares).
    /// Petrosian's favourite structural advantage: N >> bad B.
    pub knight_vs_bad_bishop:       i32,
    /// Bonus per flank where we have a pawn majority (more pawns than opponent
    /// on that third of the board).  Flanks: QS=a–c, center=d–e, KS=f–h.
    pub pawn_majority:              i32,
    /// Bonus when our king has direct opposition against the enemy king in
    /// the endgame (kings separated by exactly one square, our turn).
    /// Key K+P endgame technique Petrosian was legendary for.
    pub king_opposition:            i32,

    // ── Tal attacking terms ───────────────────────────────────────────────────
    /// Bonus when our rook is on the same file as the enemy king, or one file
    /// away.  Rook lifts to the king's file are a hallmark of Tal's attacks.
    pub rook_king_file:             i32,
    /// Bonus when our queen and a bishop are on the same diagonal that passes
    /// through or adjacent to the enemy king zone.  Queen-bishop battery is
    /// Tal's most feared attacking formation.
    pub queen_bishop_battery:       i32,
    /// Extra bonus when 3+ of our pieces are in the enemy king zone (attack
    /// reaches critical mass — the position becomes very hard to defend).
    pub attack_critical_mass:       i32,
}

impl EvalParams {
    /// Prophylactic, positional squeeze. Anticipates opponent's plans,
    /// restricts piece activity, outplays in endgames. Karpov's style.
    pub fn karpov_style() -> Self {
        EvalParams {
            material_weight:        1.05,  // respects material — no unsound sacs
            king_attack_weight:     0.85,  // defensive — no speculative attacks
            pawn_storm_weight:      0.70,  // no pawn storms; control, not chaos
            sac_bonus:              0,
            sac_threshold:          150,
            sac_uncastled_bonus:    60,
            tempo:                  25,
            shelter_center:         -10,
            shelter_adjacent:       17,
            open_file_king:         46,
            semi_open_file_king:    59,
            tropism_scale:          0.25,  // pieces control key squares, NOT aimed at king
            bishop_pair:            150,   // Karpov valued the bishop pair
            rook_open_file:         25,
            rook_semi_open:         57,
            connected_rooks:        20,
            rook_seventh:           64,
            dev_lead_per_piece:     132,
            mobility_divisor:       4,     // activity and restriction matter
            isolated_pawn:          30,    // strong structural play
            doubled_pawn:           104,
            backward_pawn:          20,
            outpost_knight:             50,   // dominant, stable pieces
            outpost_knight_supported:   60,
            rook_behind_passer:         64,
            hanging_piece_bonus:        65,   // punishes loose pieces
            space_bonus:                20,   // the territorial squeeze — Karpov's trademark
            blockade_knight:            70,   // prophylaxis: knight on passer stop-sq
            blockade_piece:             93,
            connected_pawn:             -28,
            bad_bishop:                 -2,
            piece_count_bonus:          5,    // slight preference to keep pieces
            queen_on_board_bonus:       0,
            uncastled_king:             40,   // punish opponent's uncastled king
            knight_centralization:      15,   // active knights
            weak_square_bonus:          8,    // positional restraint
            passer_king_prox:           6,    // king supports passer in endgame
            passer_enemy_king:          5,    // king away from our passer = winning
            protected_passer:           30,   // protected passers are strong
            king_activity_eg:           8,    // king active in endgame
            rook_cuts_king:             20,   // Tarrasch rule: rook cuts enemy king
            knight_vs_bad_bishop:       20,   // positional advantage when it arises
            pawn_majority:              8,    // modest majority awareness
            king_opposition:            15,   // endgame technique
            rook_king_file:             10,   // rarely lifts rook to king file
            queen_bishop_battery:       5,    // no speculative batteries
            attack_critical_mass:       5,    // prophylaxis, not mass attack
        }
    }

    /// Aggressive, sacrificial play. Higher king attack, lower material value.
    pub fn tal_style() -> Self {
        EvalParams {
            material_weight:        0.80,  // very willing to sacrifice material for attack
            king_attack_weight:     2.5,   // maximum aggression toward the enemy king
            pawn_storm_weight:      1.8,
            sac_bonus:              100,   // big compensation when attacking with sacrifices
            sac_threshold:          150,
            sac_uncastled_bonus:    80,
            tempo:                  10,
            shelter_center:         50,
            shelter_adjacent:       30,
            open_file_king:         60,    // aggressively opens files toward the king
            semi_open_file_king:    25,
            tropism_scale:          0.70,  // pieces rewarded strongly for king proximity
            bishop_pair:            35,
            rook_open_file:         28,
            rook_semi_open:         14,
            connected_rooks:        22,
            rook_seventh:           55,    // 7th rank rooks are attacking weapons
            dev_lead_per_piece:     25,    // hyperfast development to launch the attack
            mobility_divisor:       5,
            isolated_pawn:          8,     // structure matters less than attack
            doubled_pawn:           5,
            backward_pawn:          8,
            outpost_knight:             18,
            outpost_knight_supported:   35,
            rook_behind_passer:         22,
            hanging_piece_bonus:        50,
            space_bonus:                0,   // prefers open chaos, not territory control
            blockade_knight:            10,  // Tal doesn't care about blockade
            blockade_piece:             5,
            connected_pawn:             2,
            bad_bishop:                 2,
            piece_count_bonus:          0,
            queen_on_board_bonus:       60,  // NEVER trade the queen — it's the primary attacker
            uncastled_king:             50,   // punish laggard castlers — open the center!
            knight_centralization:      5,    // knights get traded for attacks anyway
            weak_square_bonus:          3,    // Tal doesn't care about structure
            passer_king_prox:           2,    // Tal doesn't play endgames if he can help it
            passer_enemy_king:          2,
            protected_passer:           10,
            king_activity_eg:           3,
            rook_cuts_king:             10,
            knight_vs_bad_bishop:       5,    // Tal doesn't care — he trades everything
            pawn_majority:              3,
            king_opposition:            5,    // rarely reaches K+P endings
            rook_king_file:             40,   // rook lifts to the enemy king's file
            queen_bishop_battery:       45,   // queen+bishop battery on diagonal = Tal's trademark
            attack_critical_mass:       70,   // 3+ pieces in king zone = overwhelming attack
        }
    }

    /// Suffocating, prophylactic play. Chokes opponent's pieces, maximises king safety.
    pub fn petrosian_style() -> Self {
        EvalParams {
            material_weight:        1.15,  // material is sacred — never sacrifice without clear return
            king_attack_weight:     0.5,   // never launches king attacks
            pawn_storm_weight:      0.3,   // no pawn storms
            sac_bonus:              0,     // never sacrifices
            sac_threshold:          150,
            sac_uncastled_bonus:    0,
            tempo:                  10,
            shelter_center:         70,    // king shelter is paramount
            shelter_adjacent:       48,
            open_file_king:         65,
            semi_open_file_king:    32,
            tropism_scale:          0.2,   // own pieces not aiming at king
            bishop_pair:            30,    // knights preferred in closed positions
            rook_open_file:         22,
            rook_semi_open:         11,
            connected_rooks:        25,
            rook_seventh:           38,
            dev_lead_per_piece:     20,
            mobility_divisor:       3,     // mobility weighted more → rewards restricting opponent
            isolated_pawn:          22,    // severe structure penalties
            doubled_pawn:           16,
            backward_pawn:          20,
            outpost_knight:             35,  // Petrosian loved stable, dominant knights
            outpost_knight_supported:   62,
            rook_behind_passer:         20,
            hanging_piece_bonus:        28,
            space_bonus:                8,   // core of the bind — territory control is everything
            blockade_knight:            55,  // signature move: knight on passer stop-square
            blockade_piece:             22,
            connected_pawn:             8,   // pawn chains are structural fortresses
            bad_bishop:                 10,  // avoid creating bad bishops, inflict them on opponent
            piece_count_bonus:          18,  // reward keeping the full army — discourages exchanges/sacs
            queen_on_board_bonus:       30,  // queen preservation — Petrosian almost never traded queens
            uncastled_king:             80,  // Petrosian ALWAYS castled early; enemy castling delay = danger
            knight_centralization:      25,  // dominant centralized knights are the cornerstone of the bind
            weak_square_bonus:          18,  // weak squares are permanent weaknesses — exploit them
            passer_king_prox:           12,  // king escorts the passer to promotion
            passer_enemy_king:          10,  // keep enemy king far from our passers
            protected_passer:           45,  // a protected passer is almost always winning
            king_activity_eg:           15,  // king is a powerful piece in the endgame
            rook_cuts_king:             35,  // Tarrasch rule: rook cuts off the king (Petrosian's endgame technique)
            knight_vs_bad_bishop:       50,  // Petrosian's favourite structural advantage — N >> bad B
            pawn_majority:              18,  // QS/KS majority converted to passed pawn
            king_opposition:            30,  // precise K+P endgame technique — Petrosian was legendary at this
            rook_king_file:             8,
            queen_bishop_battery:       5,
            attack_critical_mass:       5,
        }
    }

    /// Technical precision. Open positions, punishes weak pawns, craves the
    /// bishop pair. Clean, calculated play — no gambits, no unsound sacs.
    pub fn fischer_style() -> Self {
        EvalParams {
            material_weight:        1.10,  // material is respected — no speculative sacs
            king_attack_weight:     0.90,  // attacks only when position is ripe
            pawn_storm_weight:      0.80,  // no storms; open files instead
            sac_bonus:              0,
            sac_threshold:          150,
            sac_uncastled_bonus:    60,
            tempo:                  30,    // Fischer played quickly and confidently
            shelter_center:         -10,
            shelter_adjacent:       17,
            open_file_king:         46,
            semi_open_file_king:    59,
            tropism_scale:          0.55,  // attacks when justified — calculated king pressure
            bishop_pair:            200,   // Fischer's trademark — bishop pair dominance
            rook_open_file:         42,    // Fischer's rooks dominated open files
            rook_semi_open:         70,
            connected_rooks:        30,
            rook_seventh:           75,    // dominating 7th rank
            dev_lead_per_piece:     120,
            mobility_divisor:       3,     // very active pieces — Fischer maximised mobility
            isolated_pawn:          35,    // Fischer punished isolated pawns severely
            doubled_pawn:           80,
            backward_pawn:          25,
            outpost_knight:             40,
            outpost_knight_supported:   55,
            rook_behind_passer:         70,
            hanging_piece_bonus:        70,   // spots and exploits loose pieces
            space_bonus:                -10,  // prefers open over cramped positions
            blockade_knight:            40,
            blockade_piece:             50,
            connected_pawn:             -20,
            bad_bishop:                 15,   // actively avoids or inflicts bad bishops
            piece_count_bonus:          0,
            queen_on_board_bonus:       0,
            uncastled_king:             55,   // Fischer always castled — punishes delays severely
            knight_centralization:      18,   // active centralized pieces
            weak_square_bonus:          12,   // technical exploitation of structural weaknesses
            passer_king_prox:           10,   // king drives the passer home
            passer_enemy_king:          8,    // enemy king must be kept at bay
            protected_passer:           40,   // Fischer converted passed pawn endgames clinically
            king_activity_eg:           12,   // king to center in endgame
            rook_cuts_king:             30,   // Fischer knew the Tarrasch rule deeply
            knight_vs_bad_bishop:       30,   // Fischer was excellent at exploiting structural imbalances
            pawn_majority:              14,   // converts pawn majorities clinically
            king_opposition:            25,   // precise endgame technique
            rook_king_file:             28,   // Fischer lifted rooks to the king file when it opened
            queen_bishop_battery:       22,   // calculated diagonal pressure — not speculative
            attack_critical_mass:       25,   // Fischer attacked decisively when the moment arrived
        }
    }

    pub fn from_personality(p: &Personality) -> Self {
        match p.style {
            Style::Karpov    => Self::karpov_style(),
            Style::Tal       => Self::tal_style(),
            Style::Petrosian => Self::petrosian_style(),
            Style::Fischer   => Self::fischer_style(),
        }
    }

    // ── Texel tuner interface ──────────────────────────────────────────────────
    // Only the integer structural parameters are tuned; style multipliers (f32)
    // define personality and are left fixed.

    pub fn param_names() -> &'static [&'static str] {
        &[
            "tempo",
            "shelter_center", "shelter_adjacent",
            "open_file_king", "semi_open_file_king",
            "bishop_pair",
            "rook_open_file", "rook_semi_open", "connected_rooks",
            "rook_seventh", "dev_lead_per_piece", "mobility_divisor",
            "isolated_pawn", "doubled_pawn", "backward_pawn",
            "outpost_knight", "outpost_knight_supported",
            "rook_behind_passer", "hanging_piece_bonus",
            "space_bonus",
            "blockade_knight", "blockade_piece",
            "connected_pawn", "bad_bishop",
            "uncastled_king", "knight_centralization", "weak_square_bonus",
            "passer_king_prox", "passer_enemy_king", "protected_passer",
            "king_activity_eg", "rook_cuts_king",
            "knight_vs_bad_bishop", "pawn_majority", "king_opposition",
            "rook_king_file", "queen_bishop_battery", "attack_critical_mass",
        ]
    }

    pub fn to_tunable(&self) -> Vec<i32> {
        vec![
            self.tempo,
            self.shelter_center, self.shelter_adjacent,
            self.open_file_king, self.semi_open_file_king,
            self.bishop_pair,
            self.rook_open_file, self.rook_semi_open, self.connected_rooks,
            self.rook_seventh, self.dev_lead_per_piece, self.mobility_divisor,
            self.isolated_pawn, self.doubled_pawn, self.backward_pawn,
            self.outpost_knight, self.outpost_knight_supported,
            self.rook_behind_passer, self.hanging_piece_bonus,
            self.space_bonus,
            self.blockade_knight, self.blockade_piece,
            self.connected_pawn, self.bad_bishop,
            self.uncastled_king, self.knight_centralization, self.weak_square_bonus,
            self.passer_king_prox, self.passer_enemy_king, self.protected_passer,
            self.king_activity_eg, self.rook_cuts_king,
            self.knight_vs_bad_bishop, self.pawn_majority, self.king_opposition,
            self.rook_king_file, self.queen_bishop_battery, self.attack_critical_mass,
        ]
    }

    pub fn set_from_tunable(&mut self, v: &[i32]) {
        self.tempo                    = v[0];
        self.shelter_center           = v[1];
        self.shelter_adjacent         = v[2];
        self.open_file_king           = v[3];
        self.semi_open_file_king      = v[4];
        self.bishop_pair              = v[5];
        self.rook_open_file           = v[6];
        self.rook_semi_open           = v[7];
        self.connected_rooks          = v[8];
        self.rook_seventh             = v[9];
        self.dev_lead_per_piece       = v[10];
        self.mobility_divisor         = v[11].max(1);
        self.isolated_pawn            = v[12];
        self.doubled_pawn             = v[13];
        self.backward_pawn            = v[14];
        self.outpost_knight           = v[15];
        self.outpost_knight_supported = v[16];
        self.rook_behind_passer       = v[17];
        self.hanging_piece_bonus      = v[18];
        self.space_bonus              = v[19];
        self.blockade_knight          = v[20];
        self.blockade_piece           = v[21];
        self.connected_pawn           = v[22];
        self.bad_bishop               = v[23];
        self.uncastled_king           = v[24];
        self.knight_centralization    = v[25];
        self.weak_square_bonus        = v[26];
        self.passer_king_prox         = v[27];
        self.passer_enemy_king        = v[28];
        self.protected_passer         = v[29];
        self.king_activity_eg         = v[30];
        self.rook_cuts_king           = v[31];
        self.knight_vs_bad_bishop     = v[32];
        self.pawn_majority            = v[33];
        self.king_opposition          = v[34];
        self.rook_king_file           = v[35];
        self.queen_bishop_battery     = v[36];
        self.attack_critical_mass     = v[37];
    }
}

// ── Phase ─────────────────────────────────────────────────────────────────────

fn game_phase(board: &Board) -> f32 {
    let mut phase = 0i32;
    for &p in &[Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
        phase += (*board.pieces(p)).popcnt() as i32 * PHASE_WEIGHT[p.to_index()];
    }
    (phase.min(MAX_PHASE) as f32) / (MAX_PHASE as f32)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline(always)]
fn file_mask(f: usize) -> BitBoard { BitBoard::new(FILE_MASKS[f]) }

#[inline(always)]
fn rank_mask(r: usize) -> BitBoard { BitBoard::new(RANK_MASKS[r]) }

fn board_all(board: &Board) -> BitBoard {
    *board.color_combined(Color::White) | *board.color_combined(Color::Black)
}

// ── 1. Material + PST (tapered) ───────────────────────────────────────────────

fn material_pst(board: &Board, color: Color, mg: f32) -> i32 {
    let is_white = color == Color::White;
    let ours = *board.color_combined(color);
    let mut score = 0i32;
    for &piece in &[Piece::Pawn, Piece::Knight, Piece::Bishop,
                    Piece::Rook,  Piece::Queen,  Piece::King] {
        for sq in *board.pieces(piece) & ours {
            let idx = pst_idx(sq.to_index(), is_white);
            let pst = match piece {
                Piece::King => {
                    (PST_KING_MG[idx] as f32 * mg + PST_KING_EG[idx] as f32 * (1.0 - mg)) as i32
                }
                Piece::Pawn   => PST_PAWN[idx],
                Piece::Knight => PST_KNIGHT[idx],
                Piece::Bishop => PST_BISHOP[idx],
                Piece::Rook   => PST_ROOK[idx],
                Piece::Queen  => PST_QUEEN[idx],
            };
            score += PIECE_VALUES[piece.to_index()] + pst;
        }
    }
    score
}

// ── 2a. Pawn shelter ──────────────────────────────────────────────────────────
// Penalty for missing pawns in the three files in front of the king.

fn pawn_shelter(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let king_sq = board.king_square(color);
    let kf = king_sq.get_file().to_index();
    let kr = king_sq.get_rank().to_index();
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let is_white  = color == Color::White;
    let mut penalty = 0i32;

    for df in [0i32, -1, 1] {
        let f = kf as i32 + df;
        if !(0..8).contains(&f) { continue; }
        let fmask = file_mask(f as usize);

        let mut found = false;
        for dr in 1usize..=2 {
            let r = if is_white { kr + dr } else { kr.wrapping_sub(dr) };
            if r >= 8 { break; }
            if (own_pawns & fmask & rank_mask(r)) != BitBoard(0) {
                found = true;
                break;
            }
        }
        if !found {
            penalty += if df == 0 { ep.shelter_center } else { ep.shelter_adjacent };
        }
    }
    -penalty
}

// ── 2b. Open / semi-open files near king ──────────────────────────────────────

fn open_files_near_king(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let king_sq  = board.king_square(color);
    let kf = king_sq.get_file().to_index();
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let all_pawns = *board.pieces(Piece::Pawn);
    let mut penalty = 0i32;

    for df in [0i32, -1, 1] {
        let f = kf as i32 + df;
        if !(0..8).contains(&f) { continue; }
        let fmask = file_mask(f as usize);
        if (all_pawns & fmask) == BitBoard(0) {
            penalty += ep.open_file_king;
        } else if (own_pawns & fmask) == BitBoard(0) {
            penalty += ep.semi_open_file_king;
        }
    }
    -penalty
}

// ── 2c. King-zone attack units (nonlinear multiplier) ─────────────────────────
//
// Attacker weights per piece type (cp per piece in zone):
//   Pawn: 5, Knight: 25, Bishop: 25, Rook: 45, Queen: 90
//
// Nonlinear multiplier by attacker count:
//   [0, 0, 60, 80, 92, 96, 98, 99, 100, 100, 100] / 100
//
// These constants are not in EvalParams because they're structural (change the
// shape of the curve), not just magnitudes. Add them to EvalParams if you want
// to tune individual piece weights.

const KING_ATTACKER_WEIGHT: [i32; 6] = [5, 25, 25, 45, 90, 0];
const KING_ATTACK_MULT: [i32; 11] = [0, 0, 60, 80, 92, 96, 98, 99, 100, 100, 100];

fn king_zone_attacks(board: &Board, attacking_color: Color, king_color: Color) -> (i32, usize) {
    let king_sq = board.king_square(king_color);
    let kf = king_sq.get_file().to_index() as i32;
    let kr = king_sq.get_rank().to_index() as i32;

    let our = *board.color_combined(attacking_color);
    let mut attack_sum = 0i32;
    let mut attacker_count = 0usize;

    for &piece in &[Piece::Pawn, Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
        for sq in *board.pieces(piece) & our {
            let pf = sq.get_file().to_index() as i32;
            let pr = sq.get_rank().to_index() as i32;
            let dist = (pf - kf).abs().max((pr - kr).abs());
            if dist <= 2 {
                attack_sum += KING_ATTACKER_WEIGHT[piece.to_index()];
                attacker_count += 1;
            }
        }
    }

    (attack_sum, attacker_count)
}

fn king_safety(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let shelter    = pawn_shelter(board, color, ep);
    let open_files = open_files_near_king(board, color, ep);

    let (raw_attack, n_attackers) = king_zone_attacks(board, !color, color);
    let mult = KING_ATTACK_MULT[n_attackers.min(10)] as f32 / 100.0;
    let attack_penalty = -(raw_attack as f32 * mult * ep.king_attack_weight) as i32;

    shelter + open_files + attack_penalty
}

// ── 2d. King tropism (piece proximity to enemy king) ─────────────────────────
// Bonus for having OUR pieces close to the ENEMY king.
// Piece tropism weights: Knight=2, Bishop=2, Rook=2, Queen=4

fn king_tropism(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let ek  = board.king_square(!color);
    let ekf = ek.get_file().to_index() as i32;
    let ekr = ek.get_rank().to_index() as i32;
    let our = *board.color_combined(color);
    let mut bonus = 0i32;

    const TROPISM_W: [i32; 6] = [0, 2, 2, 2, 4, 0]; // N/B/R/Q
    for &piece in &[Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
        for sq in *board.pieces(piece) & our {
            let pf   = sq.get_file().to_index() as i32;
            let pr   = sq.get_rank().to_index() as i32;
            let dist = (pf - ekf).abs().max((pr - ekr).abs());
            let prox = (7 - dist).max(0);
            bonus   += prox * TROPISM_W[piece.to_index()];
        }
    }
    (bonus as f32 * ep.king_attack_weight * ep.tropism_scale) as i32
}

// ── 3a. Mobility ──────────────────────────────────────────────────────────────

fn mobility(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let our = *board.color_combined(color);
    let mut mob = 0i32;

    for sq in *board.pieces(Piece::Knight) & our {
        mob += (chess::get_knight_moves(sq) & !our).popcnt() as i32;
    }
    for sq in *board.pieces(Piece::Bishop) & our {
        mob += (chess::get_bishop_rays(sq) & !our).popcnt() as i32;
    }
    for sq in *board.pieces(Piece::Rook) & our {
        mob += (chess::get_rook_rays(sq) & !our).popcnt() as i32;
    }
    for sq in *board.pieces(Piece::Queen) & our {
        mob += ((chess::get_bishop_rays(sq) | chess::get_rook_rays(sq)) & !our).popcnt() as i32;
    }
    mob / ep.mobility_divisor.max(1)
}

// ── 3b. Bishop pair ───────────────────────────────────────────────────────────

fn bishop_pair(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let bishops = (*board.pieces(Piece::Bishop) & *board.color_combined(color)).popcnt();
    if bishops >= 2 { ep.bishop_pair } else { 0 }
}

// ── 3c. Rook on open / semi-open file ────────────────────────────────────────

fn rook_open_files(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let all_pawns = *board.pieces(Piece::Pawn);
    let our_rooks = *board.pieces(Piece::Rook) & *board.color_combined(color);
    let mut score = 0i32;
    for sq in our_rooks {
        let fmask = file_mask(sq.get_file().to_index());
        if (all_pawns & fmask) == BitBoard(0) {
            score += ep.rook_open_file;
        } else if (own_pawns & fmask) == BitBoard(0) {
            score += ep.rook_semi_open;
        }
    }
    score
}

// ── 3d. Connected rooks ───────────────────────────────────────────────────────

fn connected_rooks(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let rooks: Vec<Square> = (*board.pieces(Piece::Rook) & *board.color_combined(color)).collect();
    if rooks.len() < 2 { return 0; }
    let r1 = rooks[0]; let r2 = rooks[1];
    if r1.get_file() == r2.get_file() || r1.get_rank() == r2.get_rank() {
        if (board_all(board) & chess::between(r1, r2)) == BitBoard(0) {
            return ep.connected_rooks;
        }
    }
    0
}

// ── 3e. Rook on 7th rank ──────────────────────────────────────────────────────

fn rook_on_seventh(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let seventh = if color == Color::White { rank_mask(6) } else { rank_mask(1) };
    let our_rooks = *board.pieces(Piece::Rook) & *board.color_combined(color);
    (our_rooks & seventh).popcnt() as i32 * ep.rook_seventh
}

// ── 3f. Development lead (middlegame only) ────────────────────────────────────
// Bonus for each enemy minor piece still on its starting rank.

fn development_lead(board: &Board, color: Color, mg: f32, ep: &EvalParams) -> i32 {
    if mg < 0.5 { return 0; }
    let enemy        = !color;
    let back_rank    = if enemy == Color::White { rank_mask(0) } else { rank_mask(7) };
    let enemy_minors = (*board.pieces(Piece::Knight) | *board.pieces(Piece::Bishop))
        & *board.color_combined(enemy);
    let undeveloped  = (enemy_minors & back_rank).popcnt() as i32;
    undeveloped * ep.dev_lead_per_piece
}

// ── 4a. Passed pawns ──────────────────────────────────────────────────────────

fn passed_pawns(board: &Board, color: Color, mg: f32) -> i32 {
    let is_white    = color == Color::White;
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(!color);
    let mut score   = 0i32;

    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();

        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }

        let ahead = if is_white { BitBoard::new(WHITE_AHEAD[ri]) }
                    else        { BitBoard::new(BLACK_AHEAD[ri]) };

        if (enemy_pawns & adj & ahead) == BitBoard(0) {
            let bonus_rank = if is_white { ri } else { 7 - ri };
            let mg_b = PASSED_PAWN_MG[bonus_rank] as f32;
            let eg_b = PASSED_PAWN_EG[bonus_rank] as f32;
            score += (mg_b * mg + eg_b * (1.0 - mg)) as i32;
        }
    }
    score
}

// ── 4b. Isolated pawns ────────────────────────────────────────────────────────

fn isolated_pawns(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let mut count = 0i32;
    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let mut adj = BitBoard(0);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        if (own_pawns & adj) == BitBoard(0) { count += 1; }
    }
    -(count * ep.isolated_pawn)
}

// ── 4c. Doubled pawns ────────────────────────────────────────────────────────

fn doubled_pawns(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let mut penalty = 0i32;
    for f in 0..8 {
        let count = (own_pawns & file_mask(f)).popcnt() as i32;
        if count > 1 { penalty += (count - 1) * ep.doubled_pawn; }
    }
    -penalty
}

// ── 4d. Backward pawns ───────────────────────────────────────────────────────

fn backward_pawns(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let is_white    = color == Color::White;
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(!color);
    let mut count   = 0i32;

    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();
        if (is_white && ri >= 7) || (!is_white && ri == 0) { continue; }

        let stop_rank = if is_white { ri + 1 } else { ri - 1 };
        let stop_sq   = Square::make_square(Rank::from_index(stop_rank), sq.get_file());
        let ep_attacks = chess::get_pawn_attacks(stop_sq, color, !BitBoard(0));
        if (enemy_pawns & ep_attacks) == BitBoard(0) { continue; }

        let mut adj = BitBoard(0);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let support = if is_white { BitBoard::new(RANKS_UPTO[ri]) }
                      else        { BitBoard::new(RANKS_FROM[ri]) };

        if (own_pawns & adj & support) == BitBoard(0) { count += 1; }
    }
    -(count * ep.backward_pawn)
}

// ── 4e. Pawn storm toward enemy king ─────────────────────────────────────────
// Storm table: bonus per pawn by advance rank (0-indexed from our back rank).

const PAWN_STORM: [i32; 8] = [0, 0, 5, 10, 20, 35, 55, 0];

fn pawn_storm(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let ek    = board.king_square(!color);
    let ekf   = ek.get_file().to_index() as i32;
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let is_white  = color == Color::White;
    let mut bonus = 0i32;

    for sq in own_pawns {
        let pf = sq.get_file().to_index() as i32;
        let pr = sq.get_rank().to_index() as i32;
        if (pf - ekf).abs() <= 2 {
            let advance = if is_white { pr } else { 7 - pr };
            if advance >= 4 {
                bonus += PAWN_STORM[advance.min(7) as usize];
            }
        }
    }
    (bonus as f32 * ep.pawn_storm_weight) as i32
}

// ── 4f. Knight outposts ───────────────────────────────────────────────────────
// A knight on an outpost (safe from enemy pawn attack, in the opponent's half)
// is one of the strongest positional advantages.
// get_pawn_attacks(sq, color, all) → squares a pawn of `color` on sq attacks,
// i.e. the squares from which an enemy pawn would attack sq.

fn knight_outposts(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let is_white    = color == Color::White;
    let our_knights = *board.pieces(Piece::Knight) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn)   & *board.color_combined(!color);
    let own_pawns   = *board.pieces(Piece::Pawn)   & *board.color_combined(color);
    let mut bonus   = 0i32;

    for sq in our_knights {
        let advance = if is_white { sq.get_rank().to_index() }
                      else        { 7 - sq.get_rank().to_index() };
        if advance < 3 { continue; } // must be in rank 4+ (opponent's territory)

        // Square attacked by enemy pawn?  get_pawn_attacks(sq, color, _) returns the
        // squares a pawn of our color on sq would attack — those are exactly the squares
        // from which an enemy pawn attacks sq.
        let safe = (chess::get_pawn_attacks(sq, color, !BitBoard(0)) & enemy_pawns) == BitBoard(0);
        if !safe { continue; }

        // Supported by own pawn? get_pawn_attacks(sq, !color, _) gives squares below sq
        // from our perspective — where a friendly pawn would stand to protect sq.
        let supported = (chess::get_pawn_attacks(sq, !color, !BitBoard(0)) & own_pawns) != BitBoard(0);
        bonus += if supported { ep.outpost_knight_supported } else { ep.outpost_knight };
    }
    bonus
}

// ── 4g. Rook behind passed pawn ───────────────────────────────────────────────
// A rook on the same file as and behind a friendly passed pawn actively supports
// its advance — one of the most concrete endgame coordination bonuses.

fn rook_behind_passer(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    let is_white    = color == Color::White;
    let own_rooks   = *board.pieces(Piece::Rook)   & *board.color_combined(color);
    let own_pawns   = *board.pieces(Piece::Pawn)   & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn)   & *board.color_combined(!color);
    let mut score   = 0i32;

    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();

        // Is it a passed pawn?
        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let ahead = if is_white { BitBoard::new(WHITE_AHEAD[ri]) }
                    else        { BitBoard::new(BLACK_AHEAD[ri]) };
        if (enemy_pawns & adj & ahead) != BitBoard(0) { continue; }

        // Any friendly rook on the same file and *behind* the pawn?
        let fmask = file_mask(fi);
        for rook_sq in own_rooks & fmask {
            let rr      = rook_sq.get_rank().to_index();
            let behind  = if is_white { rr < ri } else { rr > ri };
            if behind { score += ep.rook_behind_passer; }
        }
    }
    score
}

// ── 4h. Threats (hanging pieces) ─────────────────────────────────────────────
// Bonus for attacking an undefended enemy piece — the engine sees this in search
// too, but naming it explicitly in eval helps at low depths and guides time mgmt.

fn is_attacked_by(board: &Board, sq: Square, color: Color) -> bool {
    let our = *board.color_combined(color);
    let all = board_all(board);
    (chess::get_knight_moves(sq)  & *board.pieces(Piece::Knight) & our) != BitBoard(0)
    || (chess::get_pawn_attacks(sq, !color, !BitBoard(0)) & *board.pieces(Piece::Pawn) & our) != BitBoard(0)
    || (chess::get_bishop_moves(sq, all) & (*board.pieces(Piece::Bishop) | *board.pieces(Piece::Queen)) & our) != BitBoard(0)
    || (chess::get_rook_moves(sq, all)   & (*board.pieces(Piece::Rook)   | *board.pieces(Piece::Queen)) & our) != BitBoard(0)
    || (chess::get_king_moves(sq)        & *board.pieces(Piece::King)    & our) != BitBoard(0)
}

// ── 4i. Space advantage ───────────────────────────────────────────────────────
// Space is defined by where our pawns actually are — no pawn, no space.
// For each own pawn on a central file (b–g) that has crossed into the opponent's
// half (rank 5+ for white), award a bonus scaled by how far it has advanced.
// More advanced pawns = more space denied to the opponent.

fn space_advantage(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.space_bonus == 0 { return 0; }
    let is_white  = color == Color::White;
    let own_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let mut score = 0i32;

    for sq in own_pawns {
        let fi      = sq.get_file().to_index();
        let advance = if is_white { sq.get_rank().to_index() } else { 7 - sq.get_rank().to_index() };
        // Only central files (b–g = 1..=6) and advanced into opponent's territory (rank 5+ = advance >= 4)
        if fi >= 1 && fi <= 6 && advance >= 4 {
            // Weight by advancement: rank5=1, rank6=2, rank7=3
            score += (advance as i32 - 3) * ep.space_bonus;
        }
    }
    score
}

fn threats(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.hanging_piece_bonus == 0 { return 0; }
    let enemy = !color;
    let them  = *board.color_combined(enemy);
    let mut bonus = 0i32;
    for &piece in &[Piece::Knight, Piece::Bishop, Piece::Rook, Piece::Queen] {
        for sq in *board.pieces(piece) & them {
            if is_attacked_by(board, sq, color) && !is_attacked_by(board, sq, enemy) {
                bonus += ep.hanging_piece_bonus;
            }
        }
    }
    bonus
}

// ── 5. Sacrifice compensation ─────────────────────────────────────────────────
// When down material significantly, compensate with a bonus for attacking pieces
// near the enemy king (especially when the enemy king is uncastled).

fn sacrifice_compensation(board: &Board, color: Color, raw_material_diff: i32,
                          ep: &EvalParams) -> i32 {
    if ep.sac_bonus == 0 || raw_material_diff > -(ep.sac_threshold) { return 0; }

    let (attack_sum, n_attackers) = king_zone_attacks(board, color, !color);
    let mult = KING_ATTACK_MULT[n_attackers.min(10)] as f32 / 100.0;
    let attacker_bonus = (attack_sum as f32 * mult * 0.5) as i32;

    let ek_file   = board.king_square(!color).get_file().to_index() as i32;
    let uncastled = (ek_file >= 2 && ek_file <= 5) as i32 * ep.sac_uncastled_bonus;

    (attacker_bonus + uncastled) * ep.sac_bonus / 80 // scale with sac_bonus
}

// ── 6. Endgame mopup ─────────────────────────────────────────────────────────

fn mopup(board: &Board, color: Color, absolute: i32) -> i32 {
    if absolute.abs() < 300 { return 0; }
    let we_win = if color == Color::White { absolute > 0 } else { absolute < 0 };
    if !we_win { return 0; }

    let enemy  = !color;
    let eks    = board.king_square(enemy);
    let ks     = board.king_square(color);
    let eks_fi = eks.get_file().to_index() as i32;
    let eks_ri = eks.get_rank().to_index() as i32;
    let ks_fi  = ks.get_file().to_index() as i32;
    let ks_ri  = ks.get_rank().to_index() as i32;

    let centre_dist = (eks_fi - 3).abs().min((eks_fi - 4).abs())
        .max((eks_ri - 3).abs().min((eks_ri - 4).abs()));
    let corner_bonus = (7 - centre_dist) * 4;

    let king_dist  = (eks_fi - ks_fi).abs().max((eks_ri - ks_ri).abs());
    let prox_bonus = (14 - king_dist) * 2;

    corner_bonus + prox_bonus
}

// ── 7. OCB draw scaling ───────────────────────────────────────────────────────

fn ocb_scale(board: &Board, score: i32) -> i32 {
    if score == 0 { return 0; }
    let wb = (*board.pieces(Piece::Bishop) & *board.color_combined(Color::White)).popcnt();
    let bb = (*board.pieces(Piece::Bishop) & *board.color_combined(Color::Black)).popcnt();
    if wb != 1 || bb != 1 { return score; }
    let wsq = (*board.pieces(Piece::Bishop) & *board.color_combined(Color::White)).next().unwrap();
    let bsq = (*board.pieces(Piece::Bishop) & *board.color_combined(Color::Black)).next().unwrap();
    let wi = wsq.to_index(); let bi = bsq.to_index();
    if (wi + wi / 8) % 2 == (bi + bi / 8) % 2 { return score; } // same colour
    for &p in &[Piece::Knight, Piece::Rook, Piece::Queen] {
        if board.pieces(p).popcnt() > 0 { return score; }
    }
    score / 2
}

// ── 4j. Passed pawn blockade ─────────────────────────────────────────────────
// Bonus for having a piece (especially a knight) on the stop-square of an
// opponent's passed pawn.  The piece simultaneously halts the pawn and occupies
// a stable, high-value outpost — Petrosian's most famous technique.

fn passed_pawn_blockade(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.blockade_knight == 0 && ep.blockade_piece == 0 { return 0; }
    let enemy          = !color;
    let enemy_is_white = enemy == Color::White;
    let own_pieces     = *board.color_combined(color);
    let own_knights    = *board.pieces(Piece::Knight) & own_pieces;
    let enemy_pawns    = *board.pieces(Piece::Pawn)   & *board.color_combined(enemy);
    let own_pawns      = *board.pieces(Piece::Pawn)   & *board.color_combined(color);
    let mut bonus      = 0i32;

    for sq in enemy_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();

        // Is this enemy pawn passed (no friendly pawn can stop it)?
        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let ahead = if enemy_is_white { BitBoard::new(WHITE_AHEAD[ri]) }
                    else              { BitBoard::new(BLACK_AHEAD[ri]) };
        if (own_pawns & adj & ahead) != BitBoard(0) { continue; } // our pawn covers it

        // The stop-square: one step in front of the enemy pawn
        let block_rank = if enemy_is_white {
            if ri >= 7 { continue; }
            ri + 1
        } else {
            if ri == 0 { continue; }
            ri - 1
        };
        let block_bb = BitBoard(1u64 << (block_rank * 8 + fi));

        if (own_pieces & block_bb) != BitBoard(0) {
            bonus += if (own_knights & block_bb) != BitBoard(0) {
                ep.blockade_knight
            } else {
                ep.blockade_piece
            };
        }
    }
    bonus
}

// ── 4k. Connected pawns ───────────────────────────────────────────────────────
// A pawn is connected if it has a friendly pawn on an adjacent file at the same
// rank, one rank ahead, or one rank behind.  Connected chains are harder to break,
// control more space, and are a hallmark of Petrosian's structural play.

fn connected_pawns(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.connected_pawn == 0 { return 0; }
    let pawns = (*board.pieces(Piece::Pawn) & *board.color_combined(color)).0;
    if pawns == 0 { return 0; }
    // Shift the whole pawn set left/right by one file (masking wrap-arounds),
    // then also one rank above/below those adjacent-file squares.
    // Any own pawn that overlaps the resulting mask is "connected".
    let left  = (pawns >> 1) & !FILE_H_MASK;  // pawns shifted to file-1
    let right = (pawns << 1) & !FILE_A_MASK;  // pawns shifted to file+1
    let adj   = left | right;                  // same-rank, adjacent-file squares
    let connected = pawns & (adj | (adj << 8) | (adj >> 8));
    connected.count_ones() as i32 * ep.connected_pawn
}

// ── 4l. Bad bishop ────────────────────────────────────────────────────────────
// A bishop is "bad" when most of our own pawns sit on the same colour as it,
// blocking its diagonals from inside.  Penalise each such pawn to discourage
// creating this weakness and to accurately reflect the bishop's reduced value.

fn bad_bishop(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.bad_bishop == 0 { return 0; }
    let our_bishops = *board.pieces(Piece::Bishop) & *board.color_combined(color);
    if our_bishops.popcnt() != 1 { return 0; } // only relevant with a single bishop
    let bishop_sq  = our_bishops.into_iter().next().unwrap();
    let bix        = bishop_sq.to_index();
    let on_light   = (bix + bix / 8) % 2 == 0;  // true = light square bishop

    let own_pawns  = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let mut same   = 0i32;
    for sq in own_pawns {
        let ix = sq.to_index();
        if ((ix + ix / 8) % 2 == 0) == on_light { same += 1; }
    }
    -(same * ep.bad_bishop)
}

// ── 5a. Uncastled king penalty (opening/middlegame) ──────────────────────────
// King on files c–f (central) in the middlegame hasn't castled and is a target.
// Tapered: maximum penalty at full middlegame (mg=1.0), fades as pieces leave.

fn uncastled_king_penalty(board: &Board, color: Color, mg: f32, ep: &EvalParams) -> i32 {
    if ep.uncastled_king == 0 || mg < 0.4 { return 0; }
    let kf = board.king_square(color).get_file().to_index();
    if kf < 2 || kf > 5 { return 0; }  // files c–f = hasn't castled
    -((ep.uncastled_king as f32 * mg) as i32)
}

// ── 5b. Knight centralization bonus (middlegame) ─────────────────────────────
// A knight on d4/d5/e4/e5 (or adjacent central squares) controls the most
// squares. Petrosian's knights were always optimally placed.

fn knight_centralization(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.knight_centralization == 0 { return 0; }
    let is_white   = color == Color::White;
    let our_knights = *board.pieces(Piece::Knight) & *board.color_combined(color);
    let mut bonus  = 0i32;
    for sq in our_knights {
        let f = sq.get_file().to_index() as i32;  // 0=a … 7=h
        let r = sq.get_rank().to_index() as i32;
        let advance = if is_white { r } else { 7 - r };
        // Central files (c–f) AND at least rank 4 from our side
        let file_score  = (3 - (f - 3).abs().min((f - 4).abs())).max(0);  // 0–3
        let rank_score  = (advance as i32 - 2).max(0).min(3);              // 0–3
        bonus += ep.knight_centralization * file_score * rank_score / 6;
    }
    bonus
}

// ── 5c. Weak square exploitation (middlegame) ────────────────────────────────
// A square is "weak" for color C if no pawn of color C can ever defend it.
// We count enemy pieces sitting on squares we permanently control (weak for them).
// Petrosian's entire philosophy: create holes and occupy them forever.

fn weak_square_bonus(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.weak_square_bonus == 0 { return 0; }
    let enemy = !color;
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pieces = (*board.pieces(Piece::Knight) | *board.pieces(Piece::Bishop)
                      | *board.pieces(Piece::Rook)   | *board.pieces(Piece::Queen))
                      & *board.color_combined(enemy);
    let is_white   = color == Color::White;
    let mut bonus  = 0i32;

    for sq in enemy_pieces {
        let f = sq.get_file().to_index();
        let r = sq.get_rank().to_index();
        // In the enemy's half (advance from our view)
        let advance_from_our = if is_white { r } else { 7 - r };
        if advance_from_our < 4 { continue; } // must be in our half of the board

        // Can any of our pawns ever reach adjacent files to defend this square?
        // A square is weak if no pawn on adjacent files can advance to defend it.
        let mut pawn_can_cover = false;
        for df in [0i32, -1, 1] {
            let pf = f as i32 + df;
            if !(0..8).contains(&pf) { continue; }
            let fmask = file_mask(pf as usize);
            // Our pawn must be at or behind this rank to cover it
            for psq in own_pawns & fmask {
                let pr = psq.get_rank().to_index();
                let pawn_advance = if is_white { pr } else { 7 - pr };
                let sq_advance   = if is_white { r } else { 7 - r };
                if pawn_advance <= sq_advance { pawn_can_cover = true; break; }
            }
            if pawn_can_cover { break; }
        }
        if !pawn_can_cover {
            bonus += ep.weak_square_bonus;
        }
    }
    bonus
}

// ── 5d. Passed pawn king proximity (endgame) ──────────────────────────────────
// In the endgame, proximity of kings to passed pawns is critical:
//   - Own king close to own passer: good (escorts it to promotion)
//   - Enemy king close to our passer: bad (will capture it)
// Tapered: only applies in endgame (mg < 0.5).

fn passer_king_proximity(board: &Board, color: Color, mg: f32, ep: &EvalParams) -> i32 {
    if (ep.passer_king_prox == 0 && ep.passer_enemy_king == 0) || mg > 0.5 { return 0; }
    let eg_scale    = (1.0 - mg * 2.0).max(0.0);  // 1.0 at mg=0, 0.0 at mg=0.5
    let is_white    = color == Color::White;
    let own_king    = board.king_square(color);
    let enemy_king  = board.king_square(!color);
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(!color);
    let mut score   = 0i32;

    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();
        // Passed pawn check
        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let ahead = if is_white { BitBoard::new(WHITE_AHEAD[ri]) } else { BitBoard::new(BLACK_AHEAD[ri]) };
        if (enemy_pawns & adj & ahead) != BitBoard(0) { continue; }

        let pf = fi as i32; let pr = ri as i32;
        let okf = own_king.get_file().to_index() as i32;
        let okr = own_king.get_rank().to_index() as i32;
        let ekf = enemy_king.get_file().to_index() as i32;
        let ekr = enemy_king.get_rank().to_index() as i32;

        let own_dist   = (pf - okf).abs().max((pr - okr).abs());
        let enemy_dist = (pf - ekf).abs().max((pr - ekr).abs());

        score += ((7 - own_dist) as f32 * ep.passer_king_prox as f32 * eg_scale) as i32;
        score += (enemy_dist as f32 * ep.passer_enemy_king as f32 * eg_scale) as i32;
    }
    score
}

// ── 5e. Protected passed pawn bonus ──────────────────────────────────────────
// A passed pawn defended by another friendly pawn cannot simply be captured —
// it must be blockaded or attacked from in front, making it much harder to stop.

fn protected_passer(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.protected_passer == 0 { return 0; }
    let is_white    = color == Color::White;
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(!color);
    let mut score   = 0i32;

    for sq in own_pawns {
        let fi = sq.get_file().to_index();
        let ri = sq.get_rank().to_index();
        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let ahead = if is_white { BitBoard::new(WHITE_AHEAD[ri]) } else { BitBoard::new(BLACK_AHEAD[ri]) };
        if (enemy_pawns & adj & ahead) != BitBoard(0) { continue; }  // not passed

        // Defended by a friendly pawn?
        let defenders = chess::get_pawn_attacks(sq, !color, !BitBoard(0)) & own_pawns;
        if defenders != BitBoard(0) { score += ep.protected_passer; }
    }
    score
}

// ── 5f. King activity in the endgame ─────────────────────────────────────────
// The king is a fighting piece in the endgame. Distance from the corner
// measures how active the king is: center = best, corner = worst.
// Petrosian's endgame kings were always optimally centralized.

fn king_activity_endgame(board: &Board, color: Color, mg: f32, ep: &EvalParams) -> i32 {
    if ep.king_activity_eg == 0 || mg > 0.35 { return 0; }
    let eg_scale = (1.0 - mg / 0.35).max(0.0);  // 1.0 at mg=0, 0.0 at mg=0.35
    let ks = board.king_square(color);
    let kf = ks.get_file().to_index() as i32;
    let kr = ks.get_rank().to_index() as i32;
    // Distance from the nearest center square (d4=27, d5=35, e4=28, e5=36)
    let center_dist = ((kf - 3).abs().min((kf - 4).abs()))
        .max((kr - 3).abs().min((kr - 4).abs()));
    ((4 - center_dist).max(0) as f32 * ep.king_activity_eg as f32 * eg_scale) as i32
}

// ── 5g. Rook cuts enemy king off from passed pawn (Tarrasch rule) ────────────
// If our rook is on the same rank as the enemy king AND between the enemy king
// and our passed pawn file, the enemy king cannot approach the passer.
// This is one of the most decisive techniques in rook endgames.

fn rook_cuts_king(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.rook_cuts_king == 0 { return 0; }
    let is_white    = color == Color::White;
    let own_rooks   = *board.pieces(Piece::Rook)  & *board.color_combined(color);
    let own_pawns   = *board.pieces(Piece::Pawn)  & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn)  & *board.color_combined(!color);
    let enemy_king  = board.king_square(!color);
    let ekf = enemy_king.get_file().to_index() as i32;
    let ekr = enemy_king.get_rank().to_index() as i32;
    let mut bonus   = 0i32;

    // Find passed pawns
    for psq in own_pawns {
        let fi = psq.get_file().to_index();
        let ri = psq.get_rank().to_index();
        let mut adj = file_mask(fi);
        if fi > 0 { adj |= file_mask(fi - 1); }
        if fi < 7 { adj |= file_mask(fi + 1); }
        let ahead = if is_white { BitBoard::new(WHITE_AHEAD[ri]) } else { BitBoard::new(BLACK_AHEAD[ri]) };
        if (enemy_pawns & adj & ahead) != BitBoard(0) { continue; }

        let pf = fi as i32;
        // Check if any of our rooks is on the same rank as the enemy king
        // AND between the enemy king and the pawn's file (cutting the king off)
        for rsq in own_rooks {
            let rf = rsq.get_file().to_index() as i32;
            let rr = rsq.get_rank().to_index() as i32;
            if rr != ekr { continue; }  // rook must be on same rank as enemy king
            // Rook cuts off if it is between king and pawn file on that rank
            let min_f = rf.min(pf);
            let max_f = rf.max(pf);
            if ekf > min_f && ekf < max_f { bonus += ep.rook_cuts_king; }
        }
    }
    bonus
}

// ── 6a. Knight vs bad bishop ──────────────────────────────────────────────────
// Petrosian's favourite structural advantage: a knight that cannot be traded
// off vs a bishop locked behind its own pawns.
//
// Triggers when:
//   - We have at least one knight
//   - Opponent has exactly one bishop (no bishop pair to compensate)
//   - That bishop is "bad": ≥ 3 of the opponent's own pawns sit on the same
//     colour as the bishop, blocking its diagonals.
//
// The bonus is given once per own knight (encourages keeping knights).

fn knight_vs_bad_bishop(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.knight_vs_bad_bishop == 0 { return 0; }
    let enemy = !color;
    let our_knights   = *board.pieces(Piece::Knight) & *board.color_combined(color);
    if our_knights == BitBoard(0) { return 0; }

    let enemy_bishops = *board.pieces(Piece::Bishop) & *board.color_combined(enemy);
    if enemy_bishops.popcnt() != 1 { return 0; }  // need exactly one (bad) bishop

    let bishop_sq = enemy_bishops.into_iter().next().unwrap();
    let on_light  = (bishop_sq.to_index() + bishop_sq.to_index() / 8) % 2 == 0;

    // Count enemy pawns on the same colour as the bishop
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(enemy);
    let mut same = 0i32;
    for sq in enemy_pawns {
        let ix = sq.to_index();
        if ((ix + ix / 8) % 2 == 0) == on_light { same += 1; }
    }
    if same < 3 { return 0; }  // not blocked enough to call it bad

    our_knights.popcnt() as i32 * ep.knight_vs_bad_bishop
}

// ── 6b. Pawn majority per flank ───────────────────────────────────────────────
// A pawn majority on a flank can be converted into a protected passed pawn.
// We split the board into three regions and award a bonus for each flank where
// we outnumber the opponent.
//
//   QS: files a–c  (0–2)
//   Center: files d–e  (3–4)
//   KS: files f–h  (5–7)

fn pawn_majority(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.pawn_majority == 0 { return 0; }
    let enemy = !color;
    let own_pawns   = *board.pieces(Piece::Pawn) & *board.color_combined(color);
    let enemy_pawns = *board.pieces(Piece::Pawn) & *board.color_combined(enemy);

    let count_flank = |pawns: BitBoard, lo: usize, hi: usize| -> i32 {
        pawns.into_iter()
            .filter(|sq| { let f = sq.get_file().to_index(); f >= lo && f <= hi })
            .count() as i32
    };

    let mut bonus = 0i32;
    for (lo, hi) in [(0usize, 2usize), (3, 4), (5, 7)] {
        let ours   = count_flank(own_pawns,   lo, hi);
        let theirs = count_flank(enemy_pawns, lo, hi);
        if ours > theirs {
            bonus += ep.pawn_majority * (ours - theirs);
        }
    }
    bonus
}

// ── 6c. King opposition (endgame) ────────────────────────────────────────────
// In king-and-pawn endings, having the opposition means the opponent's king
// must yield ground.  Direct opposition: kings on same file OR rank exactly
// 2 squares apart (one square between them), side-to-move has the opposition
// when it is the OPPONENT'S turn.
//
// We only award the bonus when:
//   - Deep endgame (mg < 0.25)
//   - Kings are in direct opposition (distance == 2 on file or rank, same axis)
//   - It is the OPPONENT'S turn (we hold the opposition)
//
// A simpler but still useful proxy: award when our king is adjacent (distance
// 1 in the Chebyshev sense) to the enemy king.  This covers both direct
// opposition and close-king scenarios that matter in K+P endings.

fn king_opposition(board: &Board, color: Color, mg: f32, ep: &EvalParams) -> i32 {
    if ep.king_opposition == 0 || mg > 0.25 { return 0; }
    // Only meaningful when opponent is to move (we have the opposition)
    if board.side_to_move() == color { return 0; }

    let our_king   = board.king_square(color);
    let enemy_king = board.king_square(!color);

    let of = our_king.get_file().to_index() as i32;
    let or_ = our_king.get_rank().to_index() as i32;
    let ef = enemy_king.get_file().to_index() as i32;
    let er = enemy_king.get_rank().to_index() as i32;

    let df = (of - ef).abs();
    let dr = (or_ - er).abs();

    // Direct opposition: same file, 2 ranks apart  OR  same rank, 2 files apart
    let direct = (df == 0 && dr == 2) || (df == 2 && dr == 0);
    // Diagonal opposition: 2 squares diagonally
    let diagonal = df == 2 && dr == 2;

    if direct || diagonal {
        let eg_scale = (1.0 - mg / 0.25).max(0.0);
        ((ep.king_opposition as f32 * eg_scale) as i32)
    } else {
        0
    }
}

// ── 5h. Rook on king file (Tal attacking term) ────────────────────────────────
// Bonus when our rook is on the same file as the enemy king, or one file away.
// Rook lifts toward the enemy king's file are a hallmark of Tal's attacks.

fn rook_king_file(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.rook_king_file == 0 { return 0; }
    let ek_file = board.king_square(!color).get_file().to_index() as i32;
    let our = *board.color_combined(color) & *board.pieces(Piece::Rook);
    let mut bonus = 0i32;
    for sq in our {
        let rf = sq.get_file().to_index() as i32;
        let dist = (rf - ek_file).abs();
        if dist == 0 {
            bonus += ep.rook_king_file;         // rook on enemy king's file
        } else if dist == 1 {
            bonus += ep.rook_king_file / 2;     // one file away — still threatening
        }
    }
    bonus
}

// ── 5i. Queen-bishop battery on diagonal (Tal attacking term) ─────────────────
// Bonus when our queen and a bishop share a diagonal that passes through or
// adjacent to the enemy king zone (3×3 around enemy king).
// This is Tal's most feared attacking formation.

fn queen_bishop_battery(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.queen_bishop_battery == 0 { return 0; }
    let ek  = board.king_square(!color);
    let ekf = ek.get_file().to_index() as i32;
    let ekr = ek.get_rank().to_index() as i32;

    let our   = *board.color_combined(color);
    let queens  = *board.pieces(Piece::Queen)  & our;
    let bishops = *board.pieces(Piece::Bishop) & our;

    if queens == BitBoard(0) || bishops == BitBoard(0) { return 0; }

    let mut bonus = 0i32;
    for bsq in bishops {
        let bf = bsq.get_file().to_index() as i32;
        let br = bsq.get_rank().to_index() as i32;
        for qsq in queens {
            let qf = qsq.get_file().to_index() as i32;
            let qr = qsq.get_rank().to_index() as i32;
            // Same diagonal: |Δfile| == |Δrank|
            if (qf - bf).abs() != (qr - br).abs() { continue; }
            // Check that the diagonal passes through the enemy king zone (±2)
            // The diagonal passing through (qf,qr) with slope ±1:
            // for slope +1: sq on diag iff qf-qr == f-r
            // for slope -1: sq on diag iff qf+qr == f+r
            let diag1 = qf - qr; // for / diagonal
            let diag2 = qf + qr; // for \ diagonal
            let on_diag1 = (ekf - diag1 - ekr).abs() <= 1;  // near / diagonal
            let on_diag2 = (ekf + ekr - diag2).abs() <= 1;  // near \ diagonal
            if on_diag1 || on_diag2 {
                bonus += ep.queen_bishop_battery;
                break; // one queen is enough
            }
        }
    }
    bonus
}

// ── 5j. Attack critical mass (Tal attacking term) ─────────────────────────────
// Extra bonus when 3+ of our pieces are attacking the enemy king zone.
// At this point the attack is overwhelming and very hard to defend.

fn attack_critical_mass(board: &Board, color: Color, ep: &EvalParams) -> i32 {
    if ep.attack_critical_mass == 0 { return 0; }
    let (_, n_attackers) = king_zone_attacks(board, color, !color);
    if n_attackers >= 3 {
        ep.attack_critical_mass * (n_attackers as i32 - 2)  // scales: 3=1x, 4=2x, 5=3x
    } else {
        0
    }
}

// ── Core evaluation helper ────────────────────────────────────────────────────

fn eval_inner(board: &Board, ep: &EvalParams) -> (i32, f32) {
    let mg    = game_phase(board);
    let is_eg = mg < 0.4;

    let raw_mat_w = material_pst(board, Color::White, mg);
    let raw_mat_b = material_pst(board, Color::Black, mg);
    let mat_w = (raw_mat_w as f32 * ep.material_weight) as i32;
    let mat_b = (raw_mat_b as f32 * ep.material_weight) as i32;

    let ks_w = king_safety(board, Color::White, ep);
    let ks_b = king_safety(board, Color::Black, ep);

    // tropism(White) = White pieces near Black king → added to White's score
    // tropism(Black) = Black pieces near White king → added to Black's score
    let trop_w = king_tropism(board, Color::White, ep);
    let trop_b = king_tropism(board, Color::Black, ep);

    let act_w = mobility(board, Color::White, ep)
              + bishop_pair(board, Color::White, ep)
              + rook_open_files(board, Color::White, ep)
              + connected_rooks(board, Color::White, ep)
              + rook_on_seventh(board, Color::White, ep)
              + development_lead(board, Color::White, mg, ep)
              + knight_outposts(board, Color::White, ep)
              + rook_behind_passer(board, Color::White, ep)
              + space_advantage(board, Color::White, ep)
              + passed_pawn_blockade(board, Color::White, ep)
              + threats(board, Color::White, ep)
              + knight_centralization(board, Color::White, ep)
              + weak_square_bonus(board, Color::White, ep)
              + protected_passer(board, Color::White, ep)
              + rook_cuts_king(board, Color::White, ep)
              + knight_vs_bad_bishop(board, Color::White, ep)
              + pawn_majority(board, Color::White, ep);
    let act_b = mobility(board, Color::Black, ep)
              + bishop_pair(board, Color::Black, ep)
              + rook_open_files(board, Color::Black, ep)
              + connected_rooks(board, Color::Black, ep)
              + rook_on_seventh(board, Color::Black, ep)
              + development_lead(board, Color::Black, mg, ep)
              + knight_outposts(board, Color::Black, ep)
              + rook_behind_passer(board, Color::Black, ep)
              + space_advantage(board, Color::Black, ep)
              + passed_pawn_blockade(board, Color::Black, ep)
              + threats(board, Color::Black, ep)
              + knight_centralization(board, Color::Black, ep)
              + weak_square_bonus(board, Color::Black, ep)
              + protected_passer(board, Color::Black, ep)
              + rook_cuts_king(board, Color::Black, ep)
              + knight_vs_bad_bishop(board, Color::Black, ep)
              + pawn_majority(board, Color::Black, ep);

    let pawn_w = passed_pawns(board, Color::White, mg)
               + isolated_pawns(board, Color::White, ep)
               + doubled_pawns(board, Color::White, ep)
               + backward_pawns(board, Color::White, ep)
               + pawn_storm(board, Color::White, ep)
               + connected_pawns(board, Color::White, ep)
               + bad_bishop(board, Color::White, ep);
    let pawn_b = passed_pawns(board, Color::Black, mg)
               + isolated_pawns(board, Color::Black, ep)
               + doubled_pawns(board, Color::Black, ep)
               + backward_pawns(board, Color::Black, ep)
               + pawn_storm(board, Color::Black, ep)
               + connected_pawns(board, Color::Black, ep)
               + bad_bishop(board, Color::Black, ep);

    // ── New terms: opening safety, endgame technique, Tal attacking ──────────
    let new_w = uncastled_king_penalty(board, Color::White, mg, ep)
              + passer_king_proximity(board, Color::White, mg, ep)
              + king_activity_endgame(board, Color::White, mg, ep)
              + king_opposition(board, Color::White, mg, ep)
              + rook_king_file(board, Color::White, ep)
              + queen_bishop_battery(board, Color::White, ep)
              + attack_critical_mass(board, Color::White, ep);
    let new_b = uncastled_king_penalty(board, Color::Black, mg, ep)
              + passer_king_proximity(board, Color::Black, mg, ep)
              + king_activity_endgame(board, Color::Black, mg, ep)
              + king_opposition(board, Color::Black, mg, ep)
              + rook_king_file(board, Color::Black, ep)
              + queen_bishop_battery(board, Color::Black, ep)
              + attack_critical_mass(board, Color::Black, ep);

    // ── Piece preservation (Petrosian: rewards keeping full army on board) ────
    let piece_w = if ep.piece_count_bonus != 0 || ep.queen_on_board_bonus != 0 {
        let pieces = (board.pieces(chess::Piece::Knight) | board.pieces(chess::Piece::Bishop)
                    | board.pieces(chess::Piece::Rook)   | board.pieces(chess::Piece::Queen))
                    & board.color_combined(Color::White);
        let has_queen = (board.pieces(chess::Piece::Queen) & board.color_combined(Color::White)) != BitBoard(0);
        ep.piece_count_bonus * pieces.popcnt() as i32
            + if has_queen { ep.queen_on_board_bonus } else { 0 }
    } else { 0 };
    let piece_b = if ep.piece_count_bonus != 0 || ep.queen_on_board_bonus != 0 {
        let pieces = (board.pieces(chess::Piece::Knight) | board.pieces(chess::Piece::Bishop)
                    | board.pieces(chess::Piece::Rook)   | board.pieces(chess::Piece::Queen))
                    & board.color_combined(Color::Black);
        let has_queen = (board.pieces(chess::Piece::Queen) & board.color_combined(Color::Black)) != BitBoard(0);
        ep.piece_count_bonus * pieces.popcnt() as i32
            + if has_queen { ep.queen_on_board_bonus } else { 0 }
    } else { 0 };

    let sw = mat_w + ks_w + trop_w + act_w + pawn_w + piece_w + new_w;
    let sb = mat_b + ks_b + trop_b + act_b + pawn_b + piece_b + new_b;
    let mut absolute = sw - sb;

    let mat_diff_w = raw_mat_w - raw_mat_b;
    absolute += sacrifice_compensation(board, Color::White, mat_diff_w, ep);
    absolute -= sacrifice_compensation(board, Color::Black, -mat_diff_w, ep);

    if is_eg {
        absolute += mopup(board, Color::White, absolute);
        absolute -= mopup(board, Color::Black, absolute);
    }

    absolute = ocb_scale(board, absolute);

    (absolute, mg)
}

// ── Public evaluation entry points ───────────────────────────────────────────

pub fn evaluate(board: &Board) -> i32 {
    evaluate_with(board, None)
}

pub fn evaluate_with(board: &Board, personality: Option<&Personality>) -> i32 {
    match board.status() {
        BoardStatus::Checkmate => return -MATE_SCORE,
        BoardStatus::Stalemate => return 0,
        BoardStatus::Ongoing   => {}
    }

    let ep = personality
        .map(EvalParams::from_personality)
        .unwrap_or_else(EvalParams::karpov_style);

    let (absolute, _mg) = eval_inner(board, &ep);
    let stm = if board.side_to_move() == Color::White { absolute } else { -absolute };
    stm + ep.tempo
}

/// Fast path for the search hot loop.
/// Caller guarantees the position is ongoing (status already checked).
/// Takes a pre-built &EvalParams so no struct construction per call.
///
/// If NNUE weights are loaded, uses NNUE as the base and adds only the
/// personality-specific bonuses (sac compensation, piece preservation) on top.
/// Falls back to the full hand-crafted eval if no weights file is found.
#[inline]
pub fn evaluate_fast(board: &Board, ep: &EvalParams) -> i32 {
    let (absolute, _mg) = eval_inner(board, ep);
    let stm = if board.side_to_move() == Color::White { absolute } else { -absolute };
    stm + ep.tempo
}


// ── Eval trace (for `eval` UCI command) ──────────────────────────────────────

pub struct EvalTrace {
    pub material_w:      i32,
    pub material_b:      i32,
    pub king_safety_w:   i32,
    pub king_safety_b:   i32,
    pub king_tropism_w:  i32,
    pub king_tropism_b:  i32,
    pub activity_w:      i32,
    pub activity_b:      i32,
    pub pawn_w:          i32,
    pub pawn_b:          i32,
    pub sac_comp_w:      i32,
    pub sac_comp_b:      i32,
    pub mopup:           i32,
    pub ocb_shave:       i32,
    pub tempo:           i32,
    pub total:           i32,
    pub phase_pct:       u32,
    pub side_to_move:    Color,
}

pub fn evaluate_trace(board: &Board, personality: Option<&Personality>) -> EvalTrace {
    let ep = personality
        .map(EvalParams::from_personality)
        .unwrap_or_else(EvalParams::karpov_style);

    let mg    = game_phase(board);
    let is_eg = mg < 0.4;

    let raw_mat_w = material_pst(board, Color::White, mg);
    let raw_mat_b = material_pst(board, Color::Black, mg);
    let mat_w = (raw_mat_w as f32 * ep.material_weight) as i32;
    let mat_b = (raw_mat_b as f32 * ep.material_weight) as i32;

    let ks_w = king_safety(board, Color::White, &ep);
    let ks_b = king_safety(board, Color::Black, &ep);
    let kt_w = king_tropism(board, Color::White, &ep);
    let kt_b = king_tropism(board, Color::Black, &ep);

    let act_w = mobility(board, Color::White, &ep)
              + bishop_pair(board, Color::White, &ep)
              + rook_open_files(board, Color::White, &ep)
              + connected_rooks(board, Color::White, &ep)
              + rook_on_seventh(board, Color::White, &ep)
              + development_lead(board, Color::White, mg, &ep)
              + knight_outposts(board, Color::White, &ep)
              + rook_behind_passer(board, Color::White, &ep)
              + space_advantage(board, Color::White, &ep)
              + passed_pawn_blockade(board, Color::White, &ep)
              + threats(board, Color::White, &ep);
    let act_b = mobility(board, Color::Black, &ep)
              + bishop_pair(board, Color::Black, &ep)
              + rook_open_files(board, Color::Black, &ep)
              + connected_rooks(board, Color::Black, &ep)
              + rook_on_seventh(board, Color::Black, &ep)
              + development_lead(board, Color::Black, mg, &ep)
              + knight_outposts(board, Color::Black, &ep)
              + rook_behind_passer(board, Color::Black, &ep)
              + space_advantage(board, Color::Black, &ep)
              + passed_pawn_blockade(board, Color::Black, &ep)
              + threats(board, Color::Black, &ep);

    let pawn_w = passed_pawns(board, Color::White, mg)
               + isolated_pawns(board, Color::White, &ep)
               + doubled_pawns(board, Color::White, &ep)
               + backward_pawns(board, Color::White, &ep)
               + pawn_storm(board, Color::White, &ep)
               + connected_pawns(board, Color::White, &ep)
               + bad_bishop(board, Color::White, &ep);
    let pawn_b = passed_pawns(board, Color::Black, mg)
               + isolated_pawns(board, Color::Black, &ep)
               + doubled_pawns(board, Color::Black, &ep)
               + backward_pawns(board, Color::Black, &ep)
               + pawn_storm(board, Color::Black, &ep)
               + connected_pawns(board, Color::Black, &ep)
               + bad_bishop(board, Color::Black, &ep);

    let sw = mat_w + ks_w + kt_w + act_w + pawn_w;
    let sb = mat_b + ks_b + kt_b + act_b + pawn_b;
    let mut absolute = sw - sb;

    let mat_diff_w = raw_mat_w - raw_mat_b;
    let sc_w = sacrifice_compensation(board, Color::White, mat_diff_w, &ep);
    let sc_b = sacrifice_compensation(board, Color::Black, -mat_diff_w, &ep);
    absolute += sc_w - sc_b;

    let mop = if is_eg {
        let mw = mopup(board, Color::White, absolute);
        let mb = mopup(board, Color::Black, absolute);
        absolute += mw - mb;
        mw - mb
    } else { 0 };

    let pre_ocb = absolute;
    absolute    = ocb_scale(board, absolute);
    let ocb_shave = absolute - pre_ocb;

    let stm   = if board.side_to_move() == Color::White { absolute } else { -absolute };
    let total = stm + ep.tempo;

    EvalTrace {
        material_w: mat_w, material_b: mat_b,
        king_safety_w: ks_w, king_safety_b: ks_b,
        king_tropism_w: kt_w, king_tropism_b: kt_b,
        activity_w: act_w, activity_b: act_b,
        pawn_w, pawn_b,
        sac_comp_w: sc_w, sac_comp_b: sc_b,
        mopup: mop,
        ocb_shave,
        tempo: ep.tempo,
        total,
        phase_pct: (mg * 100.0) as u32,
        side_to_move: board.side_to_move(),
    }
}
