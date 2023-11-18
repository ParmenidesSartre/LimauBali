"""
Pantheon Style Analyser
========================
Measures playing style metrics from a PGN file.

Usage:
  python aggression.py games_Karpov.pgn games_Tal.pgn games_Petrosian.pgn games_Fischer.pgn
  python aggression.py --engine "Pantheon*" games.pgn

Metrics per engine side:
  Captures          — total captures made
  Sac rate          — moves where engine gave up more material than it took
  Checks given      — number of checks delivered
  Pawn storms       — pawns advanced to rank 5/6 in games
  King proximity    — avg distance of engine's most advanced attacker to opp king
  Piece exchanges   — symmetric captures (equal trade)
  Avg game length   — plies
  Attack score      — composite (sac*3 + checks + storms*2) / games
"""

import sys, os, re, math
import chess
import chess.pgn

PIECE_VALUE = {
    chess.PAWN:   1,
    chess.KNIGHT: 3,
    chess.BISHOP: 3,
    chess.ROOK:   5,
    chess.QUEEN:  9,
    chess.KING:   0,
}

def material_value(piece_type):
    return PIECE_VALUE.get(piece_type, 0)

def chebyshev(sq1, sq2):
    r1, f1 = divmod(sq1, 8)
    r2, f2 = divmod(sq2, 8)
    return max(abs(r1 - r2), abs(f1 - f2))

# ── Per-game analysis ──────────────────────────────────────────────────────────

SAC_LABELS = {
    # (giver_type, taker_type) -> label
    (chess.QUEEN,  chess.ROOK)   : "Q for R   (Q-sac)",
    (chess.QUEEN,  chess.BISHOP) : "Q for B   (Q-sac)",
    (chess.QUEEN,  chess.KNIGHT) : "Q for N   (Q-sac)",
    (chess.QUEEN,  chess.PAWN)   : "Q for P   (Q-sac)",
    (chess.ROOK,   chess.BISHOP) : "R for B   (exchange sac)",
    (chess.ROOK,   chess.KNIGHT) : "R for N   (exchange sac)",
    (chess.ROOK,   chess.PAWN)   : "R for P   (exchange sac)",
    (chess.BISHOP, chess.PAWN)   : "B for P   (piece sac)",
    (chess.KNIGHT, chess.PAWN)   : "N for P   (piece sac)",
    (chess.PAWN,   None)         : "P sac     (gambit)",
}

def sac_label(giver_type, taker_type):
    key = (giver_type, taker_type)
    return SAC_LABELS.get(key, f"{chess.piece_name(giver_type)} for {chess.piece_name(taker_type)}")

def analyse_game(game, engine_pattern):
    """Return metrics dict for the engine side in this game."""
    white = game.headers.get("White", "")
    black = game.headers.get("Black", "")

    import fnmatch
    if fnmatch.fnmatch(white, engine_pattern):
        engine_color = chess.WHITE
    elif fnmatch.fnmatch(black, engine_pattern):
        engine_color = chess.BLACK
    else:
        return None

    board = game.board()
    metrics = dict(captures=0, sacs=0, checks=0, pawn_storms=0,
                   king_proximity_sum=0, king_proximity_n=0,
                   equal_trades=0, plies=0,
                   sac_breakdown={})   # label -> count

    for node in game.mainline():
        move  = node.move
        board_before = node.parent.board()
        color = board_before.turn

        metrics["plies"] += 1

        if color != engine_color:
            board.push(move)
            continue

        moving_piece = board_before.piece_at(move.from_square)
        captured     = board_before.piece_at(move.to_square)

        ep_capture = (moving_piece and moving_piece.piece_type == chess.PAWN
                      and chess.square_file(move.from_square) != chess.square_file(move.to_square)
                      and captured is None)

        if captured or ep_capture:
            metrics["captures"] += 1
            giver_type = moving_piece.piece_type if moving_piece else chess.PAWN
            taker_type = captured.piece_type if captured else chess.PAWN
            giver = material_value(giver_type)
            taker = material_value(taker_type)

            board.push(move)

            if giver > taker:
                # Only a real sacrifice if capturing piece is now attacked by something cheaper
                is_real_sac = False
                opp = not engine_color
                if board.is_attacked_by(opp, move.to_square):
                    for pt in (chess.PAWN, chess.KNIGHT, chess.BISHOP,
                               chess.ROOK, chess.QUEEN, chess.KING):
                        if board.attackers(opp, move.to_square) & board.pieces(pt, opp):
                            if material_value(pt) < giver:
                                is_real_sac = True
                            break
                if is_real_sac:
                    metrics["sacs"] += 1
                    lbl = sac_label(giver_type, taker_type)
                    metrics["sac_breakdown"][lbl] = metrics["sac_breakdown"].get(lbl, 0) + 1
            elif giver == taker:
                metrics["equal_trades"] += 1
        else:
            board.push(move)

        if board.is_check():
            metrics["checks"] += 1

        for sq in chess.SquareSet(board.pieces(chess.PAWN, engine_color)):
            rank = chess.square_rank(sq)
            if engine_color == chess.WHITE and rank >= 4:
                metrics["pawn_storms"] += 1
            elif engine_color == chess.BLACK and rank <= 3:
                metrics["pawn_storms"] += 1

        opp_king_sq = board.king(not engine_color)
        if opp_king_sq is not None:
            min_dist = 8
            for piece_type in (chess.QUEEN, chess.ROOK, chess.BISHOP, chess.KNIGHT):
                for sq in chess.SquareSet(board.pieces(piece_type, engine_color)):
                    d = chebyshev(sq, opp_king_sq)
                    if d < min_dist:
                        min_dist = d
            if min_dist < 8:
                metrics["king_proximity_sum"] += min_dist
                metrics["king_proximity_n"]   += 1

    return metrics

# ── Aggregate across games ─────────────────────────────────────────────────────

def analyse_pgn(pgn_path, engine_pattern="*"):
    totals = dict(captures=0, sacs=0, checks=0, pawn_storms=0,
                  king_proximity_sum=0, king_proximity_n=0,
                  equal_trades=0, plies=0, games=0, sac_breakdown={})

    with open(pgn_path) as f:
        while True:
            game = chess.pgn.read_game(f)
            if game is None:
                break
            m = analyse_game(game, engine_pattern)
            if m is None:
                continue
            totals["games"] += 1
            for k in ("captures","sacs","checks","pawn_storms",
                      "king_proximity_sum","king_proximity_n","equal_trades","plies"):
                totals[k] += m[k]
            for lbl, cnt in m["sac_breakdown"].items():
                totals["sac_breakdown"][lbl] = totals["sac_breakdown"].get(lbl, 0) + cnt

    return totals

# ── Report ─────────────────────────────────────────────────────────────────────

def report(label, t):
    g = t["games"]
    if g == 0:
        print(f"  {label}: no games found")
        return
    sac_rate      = t["sacs"]        / max(t["captures"], 1) * 100
    check_per_game= t["checks"]      / g
    storm_per_game= t["pawn_storms"] / g
    avg_len       = t["plies"]       / g
    avg_prox      = t["king_proximity_sum"] / max(t["king_proximity_n"], 1)
    attack_score  = (t["sacs"] * 3 + t["checks"] + t["pawn_storms"] * 2) / g

    print(f"  {'-'*52}")
    print(f"  {label}  ({g} games)")
    print(f"  {'-'*52}")
    print(f"  Games analysed     : {g}")
    print(f"  Avg game length    : {avg_len:.1f} plies")
    print(f"  Captures / game    : {t['captures']/g:.1f}")
    print(f"  Sacrifices         : {t['sacs']}  ({sac_rate:.1f}% of captures)")
    print(f"  Equal trades       : {t['equal_trades']}")
    print(f"  Checks / game      : {check_per_game:.2f}")
    print(f"  Pawn advances / g  : {storm_per_game:.1f}  (rank 5/6)")
    print(f"  Avg piece-king dist: {avg_prox:.2f}  (lower = more attacking)")
    print(f"  Attack score / game: {attack_score:.2f}")

    # Sacrifice breakdown
    bd = t.get("sac_breakdown", {})
    if bd:
        print(f"  Sacrifice breakdown ({t['sacs']} total):")
        # Group by category
        exchange_sacs = {k: v for k, v in bd.items() if "exchange sac" in k}
        q_sacs        = {k: v for k, v in bd.items() if "Q-sac" in k}
        piece_sacs    = {k: v for k, v in bd.items() if "piece sac" in k}
        gambits       = {k: v for k, v in bd.items() if "gambit" in k}
        other         = {k: v for k, v in bd.items()
                         if k not in {**exchange_sacs,**q_sacs,**piece_sacs,**gambits}}

        def print_group(name, group):
            if group:
                total = sum(group.values())
                pct   = total / max(t["sacs"], 1) * 100
                print(f"    {name} ({total}, {pct:.0f}%):")
                for lbl, cnt in sorted(group.items(), key=lambda x: -x[1]):
                    print(f"      {lbl:<30} x{cnt}")

        print_group("Exchange sacs (R for B/N)", exchange_sacs)
        print_group("Queen sacs",                q_sacs)
        print_group("Piece sacs (B/N for P)",    piece_sacs)
        print_group("Pawn gambits",              gambits)
        if other:
            print_group("Other",                 other)
    print()

# ── Main ───────────────────────────────────────────────────────────────────────

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Karpovian aggression analyser")
    parser.add_argument("pgns",   nargs="*",    help="PGN files to analyse")
    parser.add_argument("--engine", default="Pantheon*", help="Engine name pattern (fnmatch)")
    args = parser.parse_args()

    # Auto-discover style PGNs if no files given
    if not args.pgns:
        here = os.path.dirname(os.path.abspath(__file__))
        args.pgns = sorted([
            os.path.join(here, f) for f in os.listdir(here)
            if f.startswith("games") and f.endswith(".pgn")
        ])
        if not args.pgns:
            print("No PGN files found. Run tournament.py first.")
            return

    print(f"\n{'='*54}")
    print(f"  Pantheon Style Report  (pattern: {args.engine})")
    print(f"{'='*54}\n")

    all_results = []
    for path in args.pgns:
        label = os.path.splitext(os.path.basename(path))[0]
        t = analyse_pgn(path, args.engine)
        all_results.append((label, t))
        report(label, t)

    # Comparative summary
    if len(all_results) > 1:
        print(f"  {'-'*52}")
        print(f"  Comparative Attack Score")
        print(f"  {'-'*52}")
        scores = [(lbl, (t["sacs"]*3 + t["checks"] + t["pawn_storms"]*2) / max(t["games"],1))
                  for lbl, t in all_results]
        scores.sort(key=lambda x: -x[1])
        for rank, (lbl, sc) in enumerate(scores, 1):
            bar = "#" * int(sc)
            print(f"  {rank}. {lbl:<22} {sc:6.2f}  {bar}")
        print()

if __name__ == "__main__":
    main()
