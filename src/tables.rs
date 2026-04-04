// ─────────────────────────────────────────────────────────────────────────────
//  Karpovian Rust — Piece Values, PST Tables, Constants
//
//  PST layout: index 0 = a1 (rank 1), index 63 = h8 (rank 8)
//  For White: use sq.to_index() directly
//  For Black: use mirror(sq.to_index()) which flips the rank
// ─────────────────────────────────────────────────────────────────────────────

pub const INFINITY: i32 = 1_000_000;
pub const MATE_SCORE: i32 = 100_000;
pub const CONTEMPT: i32 = 20;


// Indexed by chess::Piece::to_index(): Pawn=0, Knight=1, Bishop=2, Rook=3, Queen=4, King=5
pub const PIECE_VALUES: [i32; 6] = [100, 320, 330, 500, 900, 20000];

// Phase weights per piece (for tapered evaluation)
// Knight=1, Bishop=1, Rook=2, Queen=4  (pawns & king = 0)
pub const PHASE_WEIGHT: [i32; 6] = [0, 1, 1, 2, 4, 0];
pub const MAX_PHASE: i32 = 24; // 4*1 + 4*1 + 4*2 + 2*4 = 24

/// Flip a square index vertically (swap rank): a1↔a8, h1↔h8
#[inline(always)]
pub fn mirror(sq: usize) -> usize {
    (7 - sq / 8) * 8 + (sq % 8)
}

/// Get PST index for a square, mirroring for Black
#[inline(always)]
pub fn pst_idx(sq_index: usize, is_white: bool) -> usize {
    if is_white { sq_index } else { mirror(sq_index) }
}

// ── Pawn PST (rank 1 = indices 0-7, rank 8 = indices 56-63) ──────────────────
pub const PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,   // rank 1 (can't have pawns here)
     5, 10, 10,-30,-30, 10, 10,  5,   // rank 2 (starting squares, slight adjustments)
     5, -5,-10,  0,  0,-10, -5,  5,   // rank 3
     0,  0,  5, 30, 30,  5,  0,  0,   // rank 4 (central advance)
     5,  5, 15, 35, 35, 15,  5,  5,   // rank 5 (strong centre)
    10, 10, 20, 35, 35, 20, 10, 10,   // rank 6 (advanced)
    50, 50, 50, 50, 50, 50, 50, 50,   // rank 7 (near promotion!)
     0,  0,  0,  0,  0,  0,  0,  0,   // rank 8 (promoted)
];

// ── Knight PST ────────────────────────────────────────────────────────────────
pub const PST_KNIGHT: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,   // rank 1
    -40,-20,  0,  5,  5,  0,-20,-40,   // rank 2
    -30,  5, 10, 15, 15, 10,  5,-30,   // rank 3
    -30,  0, 15, 20, 20, 15,  0,-30,   // rank 4
    -30,  5, 15, 20, 20, 15,  5,-30,   // rank 5
    -30,  0, 10, 15, 15, 10,  0,-30,   // rank 6
    -40,-20,  0,  0,  0,  0,-20,-40,   // rank 7
    -50,-40,-30,-30,-30,-30,-40,-50,   // rank 8
];

// ── Bishop PST ────────────────────────────────────────────────────────────────
pub const PST_BISHOP: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,   // rank 1
    -10,  5,  0,  0,  0,  0,  5,-10,   // rank 2
    -10, 10, 10, 10, 10, 10, 10,-10,   // rank 3
    -10,  0, 10, 10, 10, 10,  0,-10,   // rank 4
    -10,  5,  5, 10, 10,  5,  5,-10,   // rank 5
    -10,  0,  5, 10, 10,  5,  0,-10,   // rank 6
    -10,  0,  0,  0,  0,  0,  0,-10,   // rank 7
    -20,-10,-10,-10,-10,-10,-10,-20,   // rank 8
];

// ── Rook PST ──────────────────────────────────────────────────────────────────
pub const PST_ROOK: [i32; 64] = [
     0,  0,  0,  5,  5,  0,  0,  0,   // rank 1
    -5,  0,  0,  0,  0,  0,  0, -5,   // rank 2
    -5,  0,  0,  0,  0,  0,  0, -5,   // rank 3
    -5,  0,  0,  0,  0,  0,  0, -5,   // rank 4
    -5,  0,  0,  0,  0,  0,  0, -5,   // rank 5
    -5,  0,  0,  0,  0,  0,  0, -5,   // rank 6
     5, 10, 10, 10, 10, 10, 10,  5,   // rank 7 (7th rank is powerful!)
     0,  0,  0,  0,  0,  0,  0,  0,   // rank 8
];

// ── Queen PST ─────────────────────────────────────────────────────────────────
pub const PST_QUEEN: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,   // rank 1
    -10,  0,  5,  0,  0,  0,  0,-10,   // rank 2
    -10,  5,  5,  5,  5,  5,  0,-10,   // rank 3
      0,  0,  5,  5,  5,  5,  0, -5,   // rank 4
     -5,  0,  5,  5,  5,  5,  0, -5,   // rank 5
    -10,  0,  5,  5,  5,  5,  0,-10,   // rank 6
    -10,  0,  0,  0,  0,  0,  0,-10,   // rank 7
    -20,-10,-10, -5, -5,-10,-10,-20,   // rank 8
];

// ── King middlegame PST ───────────────────────────────────────────────────────
pub const PST_KING_MG: [i32; 64] = [
     25, 35, 15,  0,  0, 15, 35, 25,   // rank 1 (castled positions rewarded)
     25, 25,  0,  0,  0,  0, 25, 25,   // rank 2
    -10,-20,-20,-20,-20,-20,-20,-10,   // rank 3
    -20,-30,-30,-40,-40,-30,-30,-20,   // rank 4
    -30,-40,-40,-50,-50,-40,-40,-30,   // rank 5
    -30,-40,-40,-50,-50,-40,-40,-30,   // rank 6
    -30,-40,-40,-50,-50,-40,-40,-30,   // rank 7
    -30,-40,-40,-50,-50,-40,-40,-30,   // rank 8
];

// ── King endgame PST ──────────────────────────────────────────────────────────
pub const PST_KING_EG: [i32; 64] = [
    -50,-30,-30,-30,-30,-30,-30,-50,   // rank 1
    -30,-20,-10,  0,  0,-10,-20,-30,   // rank 2
    -30,-10, 20, 30, 30, 20,-10,-30,   // rank 3
    -30,-10, 30, 40, 40, 30,-10,-30,   // rank 4
    -30,-10, 30, 40, 40, 30,-10,-30,   // rank 5
    -30,-10, 20, 30, 30, 20,-10,-30,   // rank 6
    -30,-30,  0,  0,  0,  0,-30,-30,   // rank 7
    -50,-40,-30,-20,-20,-30,-40,-50,   // rank 8
];

// Passed pawn bonus by rank (0-indexed, 0=rank1, 7=rank8) — tapered MG/EG
// MG: passers less critical while pieces are on the board
// EG: passers become dominant threats without pieces to stop them
pub const PASSED_PAWN_MG: [i32; 8] = [0,  5, 12, 22, 38,  60,  95, 0];
pub const PASSED_PAWN_EG: [i32; 8] = [0, 15, 30, 55, 85, 130, 170, 0];

// ── Precomputed bitboard masks ─────────────────────────────────────────────────
//  Indexed by file (0=a, 7=h) or rank (0=1st, 7=8th).
//  Using these avoids repeated shift computations inside the hot eval path.

/// All squares on file f.  FILE_MASKS[0] = a-file, FILE_MASKS[7] = h-file.
pub const FILE_MASKS: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut f = 0usize;
    while f < 8 { m[f] = 0x0101_0101_0101_0101u64 << f; f += 1; }
    m
};

/// All squares on rank r.  RANK_MASKS[0] = rank-1, RANK_MASKS[7] = rank-8.
pub const RANK_MASKS: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut r = 0usize;
    while r < 8 { m[r] = 0xFFu64 << (r * 8); r += 1; }
    m
};

/// All squares strictly ahead of rank r for White (ranks r+1 … 7).
/// WHITE_AHEAD[7] = 0 (nothing ahead of the back rank from the other side).
pub const WHITE_AHEAD: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut r = 0usize;
    while r < 7 { m[r] = (!0u64) << (8 * (r + 1)); r += 1; }
    // m[7] stays 0
    m
};

/// All squares strictly ahead of rank r for Black (ranks 0 … r-1).
/// BLACK_AHEAD[0] = 0 (nothing ahead of rank-1 for black).
pub const BLACK_AHEAD: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut r = 1usize;
    while r < 8 { m[r] = (1u64 << (r * 8)).wrapping_sub(1); r += 1; }
    // m[0] stays 0
    m
};

/// All squares on ranks 0 ..= r (inclusive).  Used for pawn support spans.
pub const RANKS_UPTO: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut r = 0usize;
    while r < 7 { m[r] = (1u64 << (8 * (r + 1))).wrapping_sub(1); r += 1; }
    m[7] = !0u64;
    m
};

/// All squares on ranks r ..= 7 (inclusive).  Used for pawn support spans.
pub const RANKS_FROM: [u64; 8] = {
    let mut m = [0u64; 8];
    let mut r = 0usize;
    while r < 8 { m[r] = (!0u64) << (8 * r); r += 1; }
    m
};

/// The a-file mask — used to prevent bitboard shift wrap from h→a.
pub const FILE_A_MASK: u64 = 0x0101_0101_0101_0101u64;
/// The h-file mask — used to prevent bitboard shift wrap from a→h.
pub const FILE_H_MASK: u64 = 0x8080_8080_8080_8080u64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_is_involution() {
        for i in 0..64 { assert_eq!(mirror(mirror(i)), i, "mirror(mirror({})) != {}", i, i); }
    }

    #[test]
    fn mirror_corners() {
        assert_eq!(mirror(0),  56); // a1 → a8
        assert_eq!(mirror(7),  63); // h1 → h8
        assert_eq!(mirror(56), 0);  // a8 → a1
        assert_eq!(mirror(63), 7);  // h8 → h1
    }

    #[test]
    fn pst_idx_white_identity() {
        for i in 0..64 { assert_eq!(pst_idx(i, true), i); }
    }

    #[test]
    fn pst_idx_black_mirrors() {
        for i in 0..64 { assert_eq!(pst_idx(i, false), mirror(i)); }
    }

    #[test]
    fn all_pst_arrays_are_64_entries() {
        assert_eq!(PST_PAWN.len(),    64);
        assert_eq!(PST_KNIGHT.len(),  64);
        assert_eq!(PST_BISHOP.len(),  64);
        assert_eq!(PST_ROOK.len(),    64);
        assert_eq!(PST_QUEEN.len(),   64);
        assert_eq!(PST_KING_MG.len(), 64);
        assert_eq!(PST_KING_EG.len(), 64);
    }

    #[test]
    fn pst_values_in_reasonable_range() {
        for &v in PST_PAWN.iter()    { assert!(v >= -50  && v <= 200, "PST_PAWN value {} out of range", v); }
        for &v in PST_KNIGHT.iter()  { assert!(v >= -60  && v <= 40,  "PST_KNIGHT value {} out of range", v); }
        for &v in PST_BISHOP.iter()  { assert!(v >= -30  && v <= 30,  "PST_BISHOP value {} out of range", v); }
        for &v in PST_ROOK.iter()    { assert!(v >= -20  && v <= 30,  "PST_ROOK value {} out of range", v); }
        for &v in PST_QUEEN.iter()   { assert!(v >= -30  && v <= 20,  "PST_QUEEN value {} out of range", v); }
        for &v in PST_KING_MG.iter() { assert!(v >= -60  && v <= 40,  "PST_KING_MG value {} out of range", v); }
        for &v in PST_KING_EG.iter() { assert!(v >= -60  && v <= 50,  "PST_KING_EG value {} out of range", v); }
    }

    #[test]
    fn file_masks_cover_8_squares_each() {
        for (f, &m) in FILE_MASKS.iter().enumerate() {
            assert_eq!(m.count_ones(), 8, "FILE_MASKS[{}] should have 8 bits", f);
        }
    }

    #[test]
    fn rank_masks_cover_8_squares_each() {
        for (r, &m) in RANK_MASKS.iter().enumerate() {
            assert_eq!(m.count_ones(), 8, "RANK_MASKS[{}] should have 8 bits", r);
        }
    }

    #[test]
    fn file_masks_cover_all_64_squares() {
        let all = FILE_MASKS.iter().fold(0u64, |acc, &m| acc | m);
        assert_eq!(all, !0u64);
    }

    #[test]
    fn rank_masks_cover_all_64_squares() {
        let all = RANK_MASKS.iter().fold(0u64, |acc, &m| acc | m);
        assert_eq!(all, !0u64);
    }

    #[test]
    fn white_ahead_decreasing_towards_rank8() {
        for r in 0..7 {
            assert!(WHITE_AHEAD[r].count_ones() > WHITE_AHEAD[r + 1].count_ones());
        }
        assert_eq!(WHITE_AHEAD[7], 0, "Nothing ahead of rank 8 for white");
    }

    #[test]
    fn black_ahead_increasing_towards_rank1() {
        for r in 1..8 {
            assert!(BLACK_AHEAD[r].count_ones() > BLACK_AHEAD[r - 1].count_ones());
        }
        assert_eq!(BLACK_AHEAD[0], 0, "Nothing ahead of rank 1 for black");
    }

    #[test]
    fn passed_pawn_bonuses_increase_by_rank() {
        for r in 1..6 {
            assert!(PASSED_PAWN_MG[r + 1] >= PASSED_PAWN_MG[r],
                "PASSED_PAWN_MG[{}]={} should be >= [{}]={}",
                r+1, PASSED_PAWN_MG[r+1], r, PASSED_PAWN_MG[r]);
            assert!(PASSED_PAWN_EG[r + 1] >= PASSED_PAWN_EG[r],
                "PASSED_PAWN_EG[{}]={} should be >= [{}]={}",
                r+1, PASSED_PAWN_EG[r+1], r, PASSED_PAWN_EG[r]);
        }
    }

    #[test]
    fn piece_values_ordered_correctly() {
        // Pawn < Knight <= Bishop < Rook < Queen < King
        assert!(PIECE_VALUES[0] < PIECE_VALUES[1], "pawn < knight");
        assert!(PIECE_VALUES[1] <= PIECE_VALUES[2], "knight <= bishop");
        assert!(PIECE_VALUES[2] < PIECE_VALUES[3], "bishop < rook");
        assert!(PIECE_VALUES[3] < PIECE_VALUES[4], "rook < queen");
        assert!(PIECE_VALUES[4] < PIECE_VALUES[5], "queen < king");
    }

    #[test]
    fn phase_weights_sum_to_max_phase() {
        // Starting position: 4 knights + 4 bishops + 4 rooks + 2 queens
        // = 4*1 + 4*1 + 4*2 + 2*4 = 4+4+8+8 = 24 = MAX_PHASE
        let sum = 4 * PHASE_WEIGHT[1] + 4 * PHASE_WEIGHT[2]
                + 4 * PHASE_WEIGHT[3] + 2 * PHASE_WEIGHT[4];
        assert_eq!(sum, MAX_PHASE);
    }
}
