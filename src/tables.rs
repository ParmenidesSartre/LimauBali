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
