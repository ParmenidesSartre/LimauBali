"""
Pantheon Tournament Runner
===========================
Plays engine vs engine over UCI, reports W/D/L and ELO estimate.

Usage:
  python tournament.py                        # Pantheon vs Stockfish auto-ELO search
  python tournament.py --games 40             # 40-game match
  python tournament.py --sf-elo 1800          # fix Stockfish ELO (no auto-search)
  python tournament.py --engine2 path.exe     # use a different opponent
  python tournament.py --style Tal            # Karpov|Tal|Petrosian|Fischer

Time control: --tc  seconds per side (default 5)
"""

import subprocess, threading, queue, time, math, argparse, sys, os, random
from dataclasses import dataclass, field
from typing import Optional

# -- Paths ---------------------------------------------------------------------

HERE    = os.path.dirname(os.path.abspath(__file__))
ROOT    = os.path.dirname(HERE)   # project root (one level up from tools/)
PANTHEON = None
for _name in ("pantheon.exe", "Karpovian_rust.exe"):
    _p = os.path.join(ROOT, "target", "release", _name)
    if os.path.exists(_p):
        PANTHEON = _p; break
STOCKFISH  = "stockfish"          # on PATH

# -- ELO math ------------------------------------------------------------------

def expected_score(elo_diff: float) -> float:
    return 1.0 / (1.0 + 10 ** (-elo_diff / 400))

def elo_diff_from_score(score: float) -> float:
    """score ∈ (0,1): fraction of points scored by engine1."""
    score = max(0.001, min(0.999, score))
    return -400 * math.log10(1.0 / score - 1.0)

def error_margin(wins: int, draws: int, losses: int) -> float:
    """95% confidence interval on ELO diff (Elo's formula)."""
    n = wins + draws + losses
    if n == 0: return 0.0
    w = wins / n; d = draws / n; l = losses / n
    s  = w + 0.5 * d
    s2 = w * (1 - s)**2 + d * (0.5 - s)**2 + l * (0 - s)**2
    stdev = math.sqrt(s2 / n)
    # delta ELO per unit score ≈ 400/ln(10) at s≈0.5
    scale  = 400 / math.log(10) / (s * (1 - s) + 1e-9)
    return 1.96 * stdev * scale

# -- UCI Engine wrapper --------------------------------------------------------

class Engine:
    def __init__(self, cmd: list[str], name: str):
        self.name = name
        self.proc = subprocess.Popen(
            cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, bufsize=1,
        )
        self._q: queue.Queue[str] = queue.Queue()
        self._reader = threading.Thread(target=self._read, daemon=True)
        self._reader.start()

    def _read(self):
        for line in self.proc.stdout:
            self._q.put(line.rstrip())

    def send(self, cmd: str):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def recv_until(self, token: str, timeout: float = 10.0) -> list[str]:
        lines = []
        deadline = time.time() + timeout
        while True:
            remaining = deadline - time.time()
            if remaining <= 0:
                break
            try:
                line = self._q.get(timeout=min(remaining, 0.1))
                lines.append(line)
                if line.startswith(token):
                    break
            except queue.Empty:
                pass
        return lines

    def uci_init(self):
        self.send("uci")
        self.recv_until("uciok", timeout=5)
        self.send("isready")
        self.recv_until("readyok", timeout=5)

    def new_game(self):
        self.send("ucinewgame")
        self.send("isready")
        self.recv_until("readyok", timeout=5)

    def set_option(self, name: str, value):
        self.send(f"setoption name {name} value {value}")

    def get_move(self, fen: str, moves: list[str], wtime: int, btime: int,
                 winc: int = 0, binc: int = 0) -> Optional[str]:
        pos = f"position fen {fen}"
        if moves:
            pos += " moves " + " ".join(moves)
        self.send(pos)
        self.send(f"go wtime {wtime} btime {btime} winc {winc} binc {binc}")
        lines = self.recv_until("bestmove", timeout=60)
        for line in reversed(lines):
            if line.startswith("bestmove"):
                parts = line.split()
                mv = parts[1] if len(parts) > 1 else None
                return None if mv in (None, "0000", "(none)") else mv
        return None

    def quit(self):
        try:
            self.send("quit")
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()

# -- Game result ---------------------------------------------------------------

@dataclass
class GameResult:
    winner: Optional[str]   # "engine1", "engine2", or None (draw)
    reason: str
    moves:  int
    board:  object = None   # final chess.Board (for PGN export)

# -- Play one game -------------------------------------------------------------

import chess

def play_game(e1: Engine, e2: Engine, e1_is_white: bool,
              tc_ms: int, inc_ms: int = 0) -> GameResult:
    """
    e1 plays white if e1_is_white, else black.
    Returns GameResult from e1's perspective.
    """
    board     = chess.Board()
    moves_uci: list[str] = []
    clocks    = [tc_ms, tc_ms]   # [white, black]
    start_fen = board.fen()

    white_engine = e1 if e1_is_white else e2
    black_engine = e2 if e1_is_white else e1

    for move_num in range(250):
        if board.is_game_over():
            break

        side   = 0 if board.turn == chess.WHITE else 1
        engine = white_engine if board.turn == chess.WHITE else black_engine

        t0  = time.time()
        mv  = engine.get_move(start_fen, moves_uci,
                               wtime=clocks[0], btime=clocks[1],
                               winc=inc_ms, binc=inc_ms)
        elapsed_ms = int((time.time() - t0) * 1000)

        clocks[side] -= elapsed_ms - inc_ms
        clocks[side] = max(clocks[side], 0)

        if mv is None:
            # Engine resigned / couldn't move
            winner = "engine2" if engine is e1 else "engine1"
            return GameResult(winner, "no move / resign", move_num, board)

        try:
            move = chess.Move.from_uci(mv)
            if move not in board.legal_moves:
                winner = "engine2" if engine is e1 else "engine1"
                return GameResult(winner, f"illegal move {mv}", move_num, board)
            board.push(move)
            moves_uci.append(mv)
        except Exception:
            winner = "engine2" if engine is e1 else "engine1"
            return GameResult(winner, f"bad move {mv}", move_num, board)

        # Flag check
        if clocks[side] <= 0:
            winner = "engine2" if engine is e1 else "engine1"
            return GameResult(winner, "timeout", move_num, board)

    # Determine result
    if board.is_checkmate():
        loser_is_white = board.turn == chess.WHITE
        if loser_is_white:
            w = "engine2" if e1_is_white else "engine1"
        else:
            w = "engine1" if e1_is_white else "engine2"
        return GameResult(w, "checkmate", len(moves_uci), board)

    return GameResult(None, board.result(), len(moves_uci), board)

# -- Run match -----------------------------------------------------------------

@dataclass
class MatchResult:
    wins: int   = 0
    draws: int  = 0
    losses: int = 0

    def score(self) -> float:
        n = self.total()
        return (self.wins + 0.5 * self.draws) / n if n else 0.5

    def total(self) -> int:
        return self.wins + self.draws + self.losses

    def elo_diff(self) -> float:
        return elo_diff_from_score(self.score())

    def margin(self) -> float:
        return error_margin(self.wins, self.draws, self.losses)

    def __str__(self):
        s   = self.score()
        d   = self.elo_diff()
        m   = self.margin()
        pct = s * 100
        return (f"  Score   : {self.wins}W / {self.draws}D / {self.losses}L  "
                f"({pct:.1f}%)\n"
                f"  ELO diff: {d:+.0f}  (±{m:.0f} at 95% confidence)")


def run_match(e1: Engine, e2: Engine, games: int,
              tc_ms: int, inc_ms: int = 0,
              verbose: bool = True,
              pgn_path: str = None) -> MatchResult:
    import chess.pgn, datetime
    result = MatchResult()
    pgn_file = open(pgn_path, "w") if pgn_path else None

    for g in range(games):
        e1_white = (g % 2 == 0)
        e1.new_game(); e2.new_game()
        gr = play_game(e1, e2, e1_white, tc_ms, inc_ms)
        if gr.winner == "engine1":   result.wins   += 1; sym = "1-0" if e1_white else "0-1"
        elif gr.winner == "engine2": result.losses += 1; sym = "0-1" if e1_white else "1-0"
        else:                        result.draws  += 1; sym = "1/2"

        # Save PGN
        if pgn_file and gr.board is not None:
            pgn_result = "1-0" if gr.winner == ("engine1" if e1_white else "engine2") \
                         else ("0-1" if gr.winner else "1/2-1/2")
            game = chess.pgn.Game.from_board(gr.board)
            game.headers["Event"]  = "Karpovian Tournament"
            game.headers["Date"]   = datetime.date.today().isoformat()
            game.headers["Round"]  = str(g + 1)
            game.headers["White"]  = e1.name if e1_white else e2.name
            game.headers["Black"]  = e2.name if e1_white else e1.name
            game.headers["Result"] = pgn_result
            print(game, file=pgn_file)
            print(file=pgn_file)
            pgn_file.flush()

        if verbose:
            color = "W" if e1_white else "B"
            print(f"  Game {g+1:3d}/{games}  [{color}] {sym:3s}  "
                  f"({gr.reason}, {gr.moves} plies)  "
                  f"running: {result.wins}W/{result.draws}D/{result.losses}L")
            sys.stdout.flush()

    if pgn_file:
        pgn_file.close()
    return result

# -- ELO binary search ---------------------------------------------------------

def make_sf(sf_elo: Optional[int]) -> Engine:
    sf = Engine([STOCKFISH], "Stockfish")
    sf.uci_init()
    # Disable tablebases for fair comparison — Pantheon has no tablebase support.
    # Without this, Stockfish plays perfect endgames even at limited ELO settings.
    sf.set_option("SyzygyPath", "")
    if sf_elo is not None:
        sf.set_option("UCI_LimitStrength", "true")
        sf.set_option("UCI_Elo", str(sf_elo))
        sf.set_option("Threads", "1")
        sf.set_option("Hash", "16")
    return sf

def elo_search(karp_path: str, tc_ms: int, inc_ms: int,
               games_per_bracket: int, style: str = None) -> int:
    """Binary-search Stockfish ELO where Karpovian scores ~50%."""
    print("\n-- ELO calibration (binary search) -----------------------------")

    lo, hi = 1000, 3000
    best_elo = (lo + hi) // 2

    for iteration in range(5):
        mid = (lo + hi) // 2
        print(f"\n  Bracket {iteration+1}: testing vs SF {mid} ELO "
              f"({games_per_bracket} games, {tc_ms/1000:.1f}s+{inc_ms/1000:.2f}s)")

        karp = Engine([karp_path], "Pantheon")
        karp.uci_init()
        if style:
            karp.set_option("Style", style)
        sf   = make_sf(mid)

        r = run_match(karp, sf, games_per_bracket, tc_ms, inc_ms)
        karp.quit(); sf.quit()

        s = r.score()
        print(f"  Result: {r.wins}W/{r.draws}D/{r.losses}L  score={s:.3f}  diff={r.elo_diff():+.0f}")

        if s > 0.55:
            lo = mid; best_elo = mid
        elif s < 0.45:
            hi = mid
        else:
            best_elo = mid
            break

    return best_elo

# -- Main ----------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Karpovian tournament runner")
    parser.add_argument("--engine1",    default=PANTHEON,   help="Path to engine 1 (default: Pantheon)")
    parser.add_argument("--engine2",    default=None,        help="Path to engine 2 (default: Stockfish)")
    parser.add_argument("--games",      type=int, default=40, help="Number of games (default: 40)")
    parser.add_argument("--tc",         type=float, default=5.0, help="Seconds per side (default: 5)")
    parser.add_argument("--inc",        type=float, default=0.05, help="Increment in seconds (default: 0.05)")
    parser.add_argument("--sf-elo",     type=int, default=None, help="Fix Stockfish ELO (skip auto-search)")
    parser.add_argument("--search-elo", action="store_true",    help="Run ELO binary search first")
    parser.add_argument("--style",      default=None,           help="Karpovian style: Default, Petrosian, Tal")
    parser.add_argument("--threads",    type=int, default=1,    help="Threads for engine1 (Lazy SMP, default: 1)")
    parser.add_argument("--pgn",        default=None,           help="Save games to this PGN file")
    parser.add_argument("--name1",      default=None)
    parser.add_argument("--name2",      default=None)
    args = parser.parse_args()

    tc_ms  = int(args.tc  * 1000)
    inc_ms = int(args.inc * 1000)

    # -- Engine 1 --------------------------------------------------------------
    e1_path = args.engine1
    e1_name = args.name1 or os.path.splitext(os.path.basename(e1_path))[0]

    # -- Engine 2 --------------------------------------------------------------
    use_sf = args.engine2 is None
    if use_sf:
        e2_name = f"Stockfish{' ' + str(args.sf_elo) if args.sf_elo else ''}"
    else:
        e2_name = args.name2 or os.path.splitext(os.path.basename(args.engine2))[0]

    print("=" * 60)
    print(f"  {e1_name}  vs  {e2_name}")
    print(f"  Time control : {args.tc:.1f}s + {args.inc:.2f}s inc")
    print(f"  Games        : {args.games}")
    print("=" * 60)

    # -- Optional ELO search first ---------------------------------------------
    sf_elo = args.sf_elo
    if use_sf and (args.search_elo or sf_elo is None):
        sf_elo = elo_search(e1_path, tc_ms, inc_ms, games_per_bracket=10, style=args.style)
        print(f"\n  Calibration complete. Closest Stockfish bracket: ~{sf_elo} ELO")

    # -- Main match ------------------------------------------------------------
    print(f"\n-- Main match ({args.games} games) ------------------------------")

    karp = Engine([e1_path], e1_name)
    karp.uci_init()
    if args.style:
        karp.set_option("Style", args.style)
        print(f"  Style        : {args.style}")
    if args.threads > 1:
        karp.set_option("Threads", str(args.threads))
        print(f"  Threads      : {args.threads}")

    if use_sf:
        opp = make_sf(sf_elo)
        opp_name = f"Stockfish {sf_elo}"
    else:
        opp = Engine([args.engine2], e2_name)
        opp.uci_init()
        opp_name = e2_name

    style_tag  = f"_{args.style}" if args.style else ""
    pgn_path   = args.pgn or os.path.join(ROOT, "games", f"games{style_tag}.pgn")
    result = run_match(karp, opp, args.games, tc_ms, inc_ms, pgn_path=pgn_path)
    print(f"  PGN saved  : {pgn_path}")

    karp.quit(); opp.quit()

    # -- Report ----------------------------------------------------------------
    print("\n" + "=" * 60)
    print(f"  FINAL RESULT:  {e1_name}  vs  {opp_name}")
    print(result)

    if use_sf and sf_elo is not None:
        est_elo = sf_elo + result.elo_diff()
        margin  = result.margin()
        print(f"\n  Estimated ELO: {est_elo:.0f}  (±{margin:.0f})")
        print(f"  (based on Stockfish bracket {sf_elo}, "
              f"diff {result.elo_diff():+.0f})")

    print("=" * 60)


if __name__ == "__main__":
    main()
