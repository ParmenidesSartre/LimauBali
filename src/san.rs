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
