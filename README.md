# LimauBali Chess Engine

> *Named after the legendary Tambun pomelo of Ipoh — sweet, sharp, and unmistakably Malaysian.*

LimauBali is a UCI/WinBoard chess engine written in Rust by **Faizal Azman**, built in Ipoh, Perak, Malaysia.

---

## The Name

**Limau Bali** (also known as *Limau Tambun*) is a variety of pomelo that has been cultivated in the Tambun Valley of Ipoh since the 19th century. First introduced by British naturalist Sir Hugh Low in 1884, it quickly became a symbol of Perak's identity — prized for its striking sweet-tangy flavour and golden-pink flesh.

Today the Hentian Limau Bali Tambun has grown into a landmark marketplace that draws visitors from over 90 countries. During the Mid-Autumn Festival, Taoists hang the fruit beside family shrines as an offering for prosperity. Limau Bali is not just a fruit — it is a piece of Ipoh's soul.

Naming a chess engine after it felt right. Chess demands the same qualities the fruit embodies: a sharp edge wrapped in elegance, depth beneath a deceptively simple surface.

## Ipoh & Chess

Ipoh has a quiet but enduring chess culture. The Perak Chess Association has been running local tournaments and youth development programmes for decades. Recent events like the Dato Dr. Ramanathan Cup drew 180 players from across the region. While Malaysia's national chess scene is anchored in Kuala Lumpur, Ipoh's community punches above its weight — producing competitive club players and fostering a love for the game that runs deep in the city's coffee-shop culture.

LimauBali is this community's engine.

---

## Features

- **UCI & WinBoard / XBoard** protocol support — works with Arena, Cutechess, ChessMaster, and Lichess-bot
- **Four playing personalities** modelled on chess legends:
  - `Karpov` — deep positional, prophylactic
  - `Tal` — sacrificial, sharp, aggressive
  - `Petrosian` — ultra-solid, fortress-like defence
  - `Fischer` — universal, precise, relentless
- **Alpha-beta search** with iterative deepening, aspiration windows, null-move pruning, LMR, and quiescence search
- **Transposition table** — lock-free, configurable size (default 128 MB)
- **Opening book** — master-games database + dedicated Tal gambit book
- **Self-teaching time model** — adapts time allocation based on position complexity across sessions
- **Tapered evaluation** — smooth blend of middlegame and endgame scores via phase counter
- **Texel tuner** — standalone binary for optimising evaluation weights

---

## Building

Requires [Rust](https://rustup.rs/) 1.65 or later.

```bash
# Engine binary
cargo build --release

# Tuner binary
cargo build --release --bin tuner
```

The release binary will be at `target/release/limaubali` (or `limaubali.exe` on Windows).

---

## Usage

### UCI (Arena, Cutechess, Lichess-bot)

Point your GUI at the `limaubali` binary. It auto-detects UCI on startup.

**UCI options**

| Option | Default | Description |
|---|---|---|
| `Hash` | 128 MB | Transposition table size |
| `Threads` | 1 | Search threads |
| `Style` | Karpov | Playing personality |
| `UCI_LimitStrength` | false | Enable ELO cap |
| `UCI_Elo` | — | Target ELO (1000–2800) |

### WinBoard / XBoard

Set the engine command to `limaubali` and the protocol to *WinBoard 2* (CECP v2). The engine auto-detects the `xboard` handshake.

### Command-Line Flags

```
limaubali --style Tal          # pre-select personality
limaubali --hash 256           # hash table size in MB
limaubali --elo 1500           # enable strength limiting
```

---

## Tools

| Script | Purpose |
|---|---|
| `tools/build_book.py` | Compile PGN collections into binary opening book |
| `tools/download_book_pgns.py` | Bulk-download GM game PGNs |
| `tools/tournament.py` | Round-robin engine tournament via cutechess-cli |
| `tools/aggression.py` | Analyse game logs and report style metrics |

---

## Benchmarking

```bash
cargo run --release -- bench
```

Runs the built-in WAC (Win At Chess) test suite and reports nodes/second.

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

*Built in Ipoh, Malaysia. Powered by pomelo.*
