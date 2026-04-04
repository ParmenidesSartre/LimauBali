// ─────────────────────────────────────────────────────────────────────────────
//  Karpovian Rust — SAN (Standard Algebraic Notation) formatter
//
//  Converts a sequence of ChessMoves + starting Board into a human-readable
//  PV string with move numbers, e.g.:
//    "1. d4 Nf6 2. Nf3 Nc6 3. e3 d5 4. Nc3 e6"
// ─────────────────────────────────────────────────────────────────────────────

use chess::{BitBoard, Board, BoardStatus, ChessMove, Color, MoveGen, Piece, Square};

// ── Character helpers ─────────────────────────────────────────────────────────

fn piece_letter(p: Piece) -> char {
    match p {
        Piece::Knight => 'N',
        Piece::Bishop => 'B',
        Piece::Rook   => 'R',
        Piece::Queen  => 'Q',
        Piece::King   => 'K',
        Piece::Pawn   => ' ', // not used directly
    }
}

fn file_char(f: usize) -> char {
    (b'a' + f as u8) as char
}

fn rank_char(r: usize) -> char {
    (b'1' + r as u8) as char
}

fn sq_name(sq: Square) -> String {
    format!("{}{}", file_char(sq.get_file().to_index()), rank_char(sq.get_rank().to_index()))
}

// ── Single-move SAN ───────────────────────────────────────────────────────────

pub fn move_to_san(board: &Board, mv: ChessMove) -> String {
    let from  = mv.get_source();
    let to    = mv.get_dest();
    let piece = match board.piece_on(from) { Some(p) => p, None => return mv.to_string() };

    // ── Castling ──────────────────────────────────────────────────────────────
    if piece == Piece::King {
        let df = to.get_file().to_index() as i32 - from.get_file().to_index() as i32;
        if df == 2  { return "O-O".to_string(); }
        if df == -2 { return "O-O-O".to_string(); }
    }

    // ── Make the move to detect check/mate ───────────────────────────────────
    let new_board = board.make_move_new(mv);
    let in_check  = *new_board.checkers() != BitBoard(0);
    let is_mate   = new_board.status() == BoardStatus::Checkmate;

    // ── Is it a capture? ─────────────────────────────────────────────────────
    let is_capture = board.piece_on(to).is_some()
        || (piece == Piece::Pawn && from.get_file() != to.get_file()); // en passant

    let mut san = String::with_capacity(8);

    if piece == Piece::Pawn {
        // Pawn moves: [file×]dest[=promo]
        if is_capture {
            san.push(file_char(from.get_file().to_index()));
            san.push('x');
        }
        san.push_str(&sq_name(to));
        if let Some(promo) = mv.get_promotion() {
            san.push('=');
            san.push(piece_letter(promo));
        }
    } else {
        // Piece moves: Piece[disambig][x]dest
        san.push(piece_letter(piece));

        // Disambiguation: find all legal moves of the same piece type to the same square
        let ambiguous: Vec<ChessMove> = MoveGen::new_legal(board)
            .filter(|&m| {
                m != mv
                    && board.piece_on(m.get_source()) == Some(piece)
                    && m.get_dest() == to
            })
            .collect();

        if !ambiguous.is_empty() {
            let same_file = ambiguous.iter().any(|m| m.get_source().get_file() == from.get_file());
            let same_rank = ambiguous.iter().any(|m| m.get_source().get_rank() == from.get_rank());
            if !same_file {
                // Disambiguate by file
                san.push(file_char(from.get_file().to_index()));
            } else if !same_rank {
                // Disambiguate by rank
                san.push(rank_char(from.get_rank().to_index()));
            } else {
                // Full square needed
                san.push_str(&sq_name(from));
            }
        }

        if is_capture { san.push('x'); }
        san.push_str(&sq_name(to));
    }

    // Check / mate suffix
    if is_mate       { san.push('#'); }
    else if in_check { san.push('+'); }

    san
}

// ── PV → book-style string ────────────────────────────────────────────────────

/// Format a list of moves starting from `board` as:
///   "1. d4 Nf6 2. Nf3 Nc6 ..."
/// If white is not to move, the first number gets "..." style:
///   "1... Nf6 2. Nf3 Nc6 ..."
pub fn pv_to_book(board: &Board, moves: &[ChessMove]) -> String {
    if moves.is_empty() { return String::new(); }

    let mut result   = String::with_capacity(moves.len() * 8);
    let mut b        = board.clone();
    let mut fullmove = 1usize;
    let mut first    = true;

    for mv in moves {
        let is_white = b.side_to_move() == Color::White;

        if is_white {
            // White move: prefix with move number, separated from previous
            if !result.is_empty() { result.push(' '); }
            result.push_str(&format!("{}. ", fullmove));
        } else if first {
            // Black to move first (mid-game PV): "1... Nc6"
            result.push_str(&format!("{}... ", fullmove));
        } else {
            // Black reply after white: just a space before the SAN
            result.push(' ');
        }
        first = false;

        result.push_str(&move_to_san(&b, *mv));

        b = b.make_move_new(*mv);
        if b.side_to_move() == Color::White {
            fullmove += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess::{Board, ChessMove, Piece, Square};
    use std::str::FromStr;

    fn b(fen: &str) -> Board { Board::from_str(fen).unwrap() }
    fn mv(from: Square, to: Square) -> ChessMove { ChessMove::new(from, to, None) }
    fn mv_promo(from: Square, to: Square, p: Piece) -> ChessMove { ChessMove::new(from, to, Some(p)) }

    // ── Pawn moves ────────────────────────────────────────────────────────────

    #[test]
    fn pawn_double_push() {
        assert_eq!(move_to_san(&Board::default(), mv(Square::E2, Square::E4)), "e4");
    }

    #[test]
    fn pawn_single_push() {
        assert_eq!(move_to_san(&Board::default(), mv(Square::D2, Square::D3)), "d3");
    }

    #[test]
    fn pawn_capture() {
        // 1.e4 d5 — white exd5
        let board = b("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2");
        assert_eq!(move_to_san(&board, mv(Square::E4, Square::D5)), "exd5");
    }

    #[test]
    fn pawn_promotion_to_queen() {
        let board = b("8/P7/8/8/8/8/8/4K1k1 w - - 0 1");
        assert_eq!(move_to_san(&board, mv_promo(Square::A7, Square::A8, Piece::Queen)), "a8=Q");
    }

    #[test]
    fn pawn_promotion_to_knight() {
        let board = b("8/P7/8/8/8/8/8/4K1k1 w - - 0 1");
        assert_eq!(move_to_san(&board, mv_promo(Square::A7, Square::A8, Piece::Knight)), "a8=N");
    }

    // ── Piece moves ───────────────────────────────────────────────────────────

    #[test]
    fn knight_development() {
        assert_eq!(move_to_san(&Board::default(), mv(Square::G1, Square::F3)), "Nf3");
    }

    #[test]
    fn bishop_development() {
        let board = b("rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2");
        assert_eq!(move_to_san(&board, mv(Square::F1, Square::C4)), "Bc4");
    }

    #[test]
    fn rook_move() {
        // Open rook on a1 moving to a4
        let board = b("4k3/8/8/8/8/8/8/R3K3 w Q - 0 1");
        assert_eq!(move_to_san(&board, mv(Square::A1, Square::A4)), "Ra4");
    }

    // ── Castling ──────────────────────────────────────────────────────────────

    #[test]
    fn kingside_castling() {
        let board = b("r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4");
        assert_eq!(move_to_san(&board, mv(Square::E1, Square::G1)), "O-O");
    }

    #[test]
    fn queenside_castling() {
        let board = b("r3kbnr/pppqpppp/2np4/1B2P3/3P4/2N2N2/PPP2PPP/R1BQK2R b KQkq - 0 6");
        assert_eq!(move_to_san(&board, mv(Square::E8, Square::C8)), "O-O-O");
    }

    // ── Check and checkmate ───────────────────────────────────────────────────

    #[test]
    fn check_suffix() {
        // 1.e4 e5 2.Bc4 Nc6 3.Qh5 — Qxf7+ gives check (or mate)
        let board = b("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4");
        let san = move_to_san(&board, mv(Square::H5, Square::F7));
        assert!(san.ends_with('+') || san.ends_with('#'),
            "Expected + or # suffix, got '{}'", san);
    }

    #[test]
    fn checkmate_suffix() {
        // Back-rank mate: white Rd1→d8 is checkmate (black king on g8, pawns on f7/g7/h7)
        let board = b("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1");
        let san = move_to_san(&board, mv(Square::D1, Square::D8));
        assert!(san.ends_with('#'), "Expected '#' suffix, got '{}'", san);
    }

    // ── PV formatting ─────────────────────────────────────────────────────────

    #[test]
    fn pv_empty_returns_empty_string() {
        assert_eq!(pv_to_book(&Board::default(), &[]), "");
    }

    #[test]
    fn pv_single_white_move() {
        let moves = vec![ChessMove::new(Square::E2, Square::E4, None)];
        assert_eq!(pv_to_book(&Board::default(), &moves), "1. e4");
    }

    #[test]
    fn pv_two_moves() {
        let moves = vec![
            ChessMove::new(Square::E2, Square::E4, None),
            ChessMove::new(Square::E7, Square::E5, None),
        ];
        assert_eq!(pv_to_book(&Board::default(), &moves), "1. e4 e5");
    }

    #[test]
    fn pv_four_moves_increments_fullmove() {
        let moves = vec![
            ChessMove::new(Square::E2, Square::E4, None),
            ChessMove::new(Square::E7, Square::E5, None),
            ChessMove::new(Square::G1, Square::F3, None),
            ChessMove::new(Square::B8, Square::C6, None),
        ];
        let pv = pv_to_book(&Board::default(), &moves);
        assert!(pv.contains("2."), "Should show move number 2, got '{}'", pv);
        assert!(pv.starts_with("1. e4"), "Should start with '1. e4', got '{}'", pv);
    }
}
