#!/usr/bin/env python3
"""
build_book.py — Generate opening book for Karpovian Rust
=========================================================

UNIVERSAL MODE (build one book from all master games):
  python build_book.py --universal --pgn master_games.pgn --freq 10 --ply 30

TWO-PHASE MODE (player-specific book):
  Phase 1 — seed file: extract every position the player personally reached.
  Phase 2 — database files: scan large opening PGNs; for any position that
             matches a seed, record every move played there.

  python build_book.py --pgn Petrosian.pgn --database master_games.pgn

SINGLE-FILE MODE (original behaviour):
  python build_book.py --pgn Petrosian.pgn

Common options:
  --player   Last name to match in seed PGN headers     (default: Petrosian)
  --out      Output Rust source file                    (default: src/book.rs)
  --freq     Min times a move must appear to be kept    (default: 3)
  --ply      Max half-moves recorded per game           (default: 40)
  --seed-ply How deep to extract seed positions         (default: 20)
  --db-ply   How deep to scan database games            (default: 40)
  --target   Which book to write: petrosian | tal        (default: petrosian)
  --universal Record ALL moves from ALL games (no player filter)
"""

import argparse
import chess
import chess.pgn
from collections import defaultdict, Counter

# -- CLI ----------------------------------------------------------------------

ap = argparse.ArgumentParser()
ap.add_argument("--pgn",       default="../books/master_games.pgn",
                help="Seed PGN: player's personal games (or master_games.pgn in --universal mode)")
ap.add_argument("--database",  nargs="*", default=[],
                help="Large opening PGNs to mine for moves at seed positions")
ap.add_argument("--player",    default="Petrosian",
                help="Last name to match in seed PGN headers")
ap.add_argument("--out",       default="../src/book.rs",
                help="Output Rust source file")
ap.add_argument("--freq",      type=int, default=3,
                help="Minimum frequency to keep a move")
ap.add_argument("--ply",       type=int, default=40,
                help="Max half-moves per game (single-file / universal mode)")
ap.add_argument("--seed-ply",  type=int, default=20,
                help="Depth of seed position extraction (half-moves)")
ap.add_argument("--db-ply",    type=int, default=40,
                help="Depth to scan database games (half-moves)")
ap.add_argument("--target",    default="petrosian",
                choices=["petrosian", "tal"],
                help="Which book to write (affects Rust symbol names)")
ap.add_argument("--universal", action="store_true",
                help="Record every move from every game (no player filter)")
args = ap.parse_args()

# -- Helpers ------------------------------------------------------------------

def iter_games(path):
    """Yield parsed games from a PGN file, skipping corrupt entries."""
    with open(path, encoding="utf-8", errors="ignore") as f:
        while True:
            try:
                g = chess.pgn.read_game(f)
            except Exception:
                continue
            if g is None:
                break
            yield g

# book[epd][uci_move] = count
book: dict[str, Counter] = defaultdict(Counter)

# =========================================================================
# UNIVERSAL MODE — record every move from every game up to --ply depth
# =========================================================================
if args.universal:
    print(f"Universal mode — scanning {args.pgn} (all games, all moves) ...", flush=True)
    total_games = 0
    for game in iter_games(args.pgn):
        board = game.board()
        for ply, move in enumerate(game.mainline_moves()):
            if ply >= args.ply:
                break
            book[board.epd()][move.uci()] += 1
            board.push(move)
        total_games += 1
        if total_games % 10_000 == 0:
            print(f"  ... {total_games:,} games", flush=True)

    print(f"  Scanned {total_games:,} games")
    seed_games = total_games   # used in header comment

# =========================================================================
# TWO-PHASE / SINGLE-FILE MODE
# =========================================================================
else:
    # -- Phase 1: extract seed positions from player's personal games ------

    print(f"Phase 1 - reading seed file: {args.pgn} ...", flush=True)
    seed_epds: set[str] = set()
    seed_games = 0
    skipped    = 0

    for game in iter_games(args.pgn):
        white   = game.headers.get("White", "")
        black   = game.headers.get("Black", "")
        p_white = args.player in white
        p_black = args.player in black
        if not p_white and not p_black:
            skipped += 1
            continue

        board = game.board()
        for ply, move in enumerate(game.mainline_moves()):
            if ply >= args.seed_ply:
                break
            epd = board.epd()
            seed_epds.add(epd)

            petrosian_to_move = (p_white and board.turn == chess.WHITE) or \
                                (p_black and board.turn == chess.BLACK)
            if petrosian_to_move:
                book[epd][move.uci()] += 1

            board.push(move)

        seed_games += 1
        if seed_games % 200 == 0:
            print(f"  ... {seed_games} seed games", flush=True)

    print(f"  Seed: {seed_games} games, {len(seed_epds):,} unique positions")

    # -- Phase 2: mine database PGNs for moves at seed positions -----------

    if args.database:
        for db_path in args.database:
            print(f"\nPhase 2 - scanning database: {db_path} ...", flush=True)
            db_games  = 0
            db_hits   = 0

            for game in iter_games(db_path):
                board = game.board()
                hit   = False
                for ply, move in enumerate(game.mainline_moves()):
                    if ply >= args.db_ply:
                        break
                    epd = board.epd()
                    if epd in seed_epds:
                        book[epd][move.uci()] += 1
                        hit = True
                    board.push(move)
                db_games += 1
                if hit:
                    db_hits += 1
                if db_games % 5000 == 0:
                    print(f"  ... {db_games:,} games scanned, {db_hits:,} matched seed positions",
                          flush=True)

            print(f"  Done: {db_games:,} games, {db_hits:,} matched seed positions")

    else:
        # Single-file mode: extend depth using the seed file itself
        print("\nNo --database supplied - using seed file only (single-file mode).")
        if args.ply > args.seed_ply:
            print(f"Phase 1b - extending depth to ply {args.ply} ...", flush=True)
            for game in iter_games(args.pgn):
                white   = game.headers.get("White", "")
                black   = game.headers.get("Black", "")
                p_white = args.player in white
                p_black = args.player in black
                if not p_white and not p_black:
                    continue
                board = game.board()
                for ply, move in enumerate(game.mainline_moves()):
                    if ply < args.seed_ply or ply >= args.ply:
                        if ply >= args.ply:
                            break
                        board.push(move)
                        continue
                    petrosian_to_move = (p_white and board.turn == chess.WHITE) or \
                                        (p_black and board.turn == chess.BLACK)
                    if petrosian_to_move:
                        book[board.epd()][move.uci()] += 1
                    board.push(move)

# -- Filter by frequency ------------------------------------------------------

entries: list[tuple[str, str, int]] = []
for epd, moves in book.items():
    for mv, cnt in moves.items():
        if cnt >= args.freq:
            entries.append((epd, mv, cnt))

entries.sort(key=lambda x: (x[0], -x[2]))

total_entries   = len(entries)
total_positions = len({e[0] for e in entries})

print(f"\nBook: {total_positions:,} positions, {total_entries:,} entries "
      f"(freq >= {args.freq})")

# -- Rust symbol names based on target ----------------------------------------

if args.target == "tal":
    rust_fn   = "tal_book"
    rust_cell = "TAL_BOOK_CELL"
    rust_type = "Tal Opening Book"
    db_note   = "Tal.pgn"
else:
    rust_fn   = "petrosian_book"
    rust_cell = "BOOK_CELL"
    rust_type = "Universal Opening Book" if args.universal else "Petrosian Opening Book"
    db_note   = args.pgn

db_list = ", ".join(args.database) if args.database else "(none)"
mode_str = "universal (all games)" if args.universal else "two-phase" if args.database else "single-file"

# -- Emit Rust source ---------------------------------------------------------

RUST_HEADER = f"""\
// -----------------------------------------------------------------------------
//  Karpovian Rust -- {rust_type}  (AUTO-GENERATED)
//
//  Mode      : {mode_str}
//  Source    : {args.pgn}  ({seed_games} games)
//  Databases : {db_list}
//  Min freq  : {args.freq}  (move must appear in >= {args.freq} games)
//  Ply depth : {args.ply}
//  Entries   : {total_entries} moves across {total_positions} positions
//
//  DO NOT EDIT -- regenerate with:
//    python build_book.py --universal --pgn master_games.pgn --freq 10 --ply 30
// -----------------------------------------------------------------------------

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;

use chess::{{Board, ChessMove, MoveGen}};

// -- Book data ----------------------------------------------------------------

static BOOK_ENTRIES: &[(&str, &str)] = &[
"""

RUST_FOOTER = f"""\
];

// -- Book implementation ------------------------------------------------------

pub struct OpeningBook {{
    entries: HashMap<u64, Vec<ChessMove>>,
    size:    usize,
}}

impl OpeningBook {{
    fn build(raw: &[(&str, &str)]) -> Self {{
        let mut entries: HashMap<u64, Vec<ChessMove>> = HashMap::new();
        let mut size    = 0usize;
        let mut bad     = 0usize;

        for &(epd, mv_str) in raw {{
            let fen = format!("{{}} 0 1", epd);
            let board = match Board::from_str(&fen) {{
                Ok(b)  => b,
                Err(_) => {{ bad += 1; continue; }}
            }};

            let legal: Vec<ChessMove> = MoveGen::new_legal(&board).collect();
            let mv = match ChessMove::from_str(mv_str) {{
                Ok(m) if legal.contains(&m) => m,
                _ => {{ bad += 1; continue; }}
            }};

            let entry = entries.entry(board.get_hash()).or_default();
            if !entry.contains(&mv) {{
                entry.push(mv);
                size += 1;
            }}
        }}

        if bad > 0 {{
            eprintln!("info string Book: {{}} invalid entries skipped", bad);
        }}

        OpeningBook {{ entries, size }}
    }}

    /// Returns a book move for the given position hash, chosen with
    /// light variety when multiple moves exist.
    pub fn probe(&self, hash: u64) -> Option<ChessMove> {{
        let moves = self.entries.get(&hash)?;
        if moves.is_empty() {{ return None; }}
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0);
        Some(moves[tick % moves.len()])
    }}

    pub fn len(&self) -> usize {{ self.size }}
}}

// -- Global instance ----------------------------------------------------------

static {rust_cell}: OnceLock<OpeningBook> = OnceLock::new();

pub fn {rust_fn}() -> &'static OpeningBook {{
    {rust_cell}.get_or_init(|| OpeningBook::build(BOOK_ENTRIES))
}}
"""

with open(args.out, "w", encoding="utf-8") as out:
    out.write(RUST_HEADER)
    for epd, mv, cnt in entries:
        safe_epd = epd.replace('"', '\\"')
        out.write(f'    ("{safe_epd}", "{mv}"),  // x{cnt}\n')
    out.write(RUST_FOOTER)

print(f"Written to {args.out}")
print(f"Next step:  cargo build --release")
