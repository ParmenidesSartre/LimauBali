#!/usr/bin/env python3
"""
download_book_pgns.py — Download all PGN Mentor player ZIPs and merge into
                         a single master PGN for opening book construction.

Usage:
    python download_book_pgns.py                  # download all + merge
    python download_book_pgns.py --workers 8      # parallel downloads
    python download_book_pgns.py --skip-download  # only merge existing files

Output:
    pgn_downloads/         folder with all individual PGNs
    master_games.pgn       merged file ready for build_book.py
"""

import os
import sys
import zipfile
import argparse
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

# ── All 240 PGN Mentor player URLs ────────────────────────────────────────────

PLAYER_URLS = [
    "https://www.pgnmentor.com/players/Abdusattorov.zip",
    "https://www.pgnmentor.com/players/Adams.zip",
    "https://www.pgnmentor.com/players/Akobian.zip",
    "https://www.pgnmentor.com/players/Akopian.zip",
    "https://www.pgnmentor.com/players/Alburt.zip",
    "https://www.pgnmentor.com/players/Alekhine.zip",
    "https://www.pgnmentor.com/players/Alekseev.zip",
    "https://www.pgnmentor.com/players/Almasi.zip",
    "https://www.pgnmentor.com/players/Anand.zip",
    "https://www.pgnmentor.com/players/Anderssen.zip",
    "https://www.pgnmentor.com/players/Andersson.zip",
    "https://www.pgnmentor.com/players/Andreikin.zip",
    "https://www.pgnmentor.com/players/Aronian.zip",
    "https://www.pgnmentor.com/players/Ashley.zip",
    "https://www.pgnmentor.com/players/Averbakh.zip",
    "https://www.pgnmentor.com/players/Azmaiparashvili.zip",
    "https://www.pgnmentor.com/players/Bacrot.zip",
    "https://www.pgnmentor.com/players/Bareev.zip",
    "https://www.pgnmentor.com/players/BecerraRivero.zip",
    "https://www.pgnmentor.com/players/Beliavsky.zip",
    "https://www.pgnmentor.com/players/Benjamin.zip",
    "https://www.pgnmentor.com/players/Benko.zip",
    "https://www.pgnmentor.com/players/Berliner.zip",
    "https://www.pgnmentor.com/players/Bernstein.zip",
    "https://www.pgnmentor.com/players/Bird.zip",
    "https://www.pgnmentor.com/players/Bisguier.zip",
    "https://www.pgnmentor.com/players/Blackburne.zip",
    "https://www.pgnmentor.com/players/Blatny.zip",
    "https://www.pgnmentor.com/players/Bogoljubow.zip",
    "https://www.pgnmentor.com/players/Boleslavsky.zip",
    "https://www.pgnmentor.com/players/Bologan.zip",
    "https://www.pgnmentor.com/players/Botvinnik.zip",
    "https://www.pgnmentor.com/players/Breyer.zip",
    "https://www.pgnmentor.com/players/Bronstein.zip",
    "https://www.pgnmentor.com/players/Browne.zip",
    "https://www.pgnmentor.com/players/Bruzon.zip",
    "https://www.pgnmentor.com/players/Bu.zip",
    "https://www.pgnmentor.com/players/Byrne.zip",
    "https://www.pgnmentor.com/players/Capablanca.zip",
    "https://www.pgnmentor.com/players/Carlsen.zip",
    "https://www.pgnmentor.com/players/Caruana.zip",
    "https://www.pgnmentor.com/players/Chiburdanidze.zip",
    "https://www.pgnmentor.com/players/Chigorin.zip",
    "https://www.pgnmentor.com/players/Christiansen.zip",
    "https://www.pgnmentor.com/players/DeFirmian.zip",
    "https://www.pgnmentor.com/players/DeLaBourdonnais.zip",
    "https://www.pgnmentor.com/players/Denker.zip",
    "https://www.pgnmentor.com/players/Ding.zip",
    "https://www.pgnmentor.com/players/DominguezPerez.zip",
    "https://www.pgnmentor.com/players/Dreev.zip",
    "https://www.pgnmentor.com/players/Duda.zip",
    "https://www.pgnmentor.com/players/Dzindzichashvili.zip",
    "https://www.pgnmentor.com/players/Ehlvest.zip",
    "https://www.pgnmentor.com/players/Eljanov.zip",
    "https://www.pgnmentor.com/players/Erigaisi.zip",
    "https://www.pgnmentor.com/players/Euwe.zip",
    "https://www.pgnmentor.com/players/Evans.zip",
    "https://www.pgnmentor.com/players/Fedorowicz.zip",
    "https://www.pgnmentor.com/players/Fine.zip",
    "https://www.pgnmentor.com/players/Finegold.zip",
    "https://www.pgnmentor.com/players/Firouzja.zip",
    "https://www.pgnmentor.com/players/Fischer.zip",
    "https://www.pgnmentor.com/players/Fishbein.zip",
    "https://www.pgnmentor.com/players/Flohr.zip",
    "https://www.pgnmentor.com/players/Gaprindashvili.zip",
    "https://www.pgnmentor.com/players/Gashimov.zip",
    "https://www.pgnmentor.com/players/Gelfand.zip",
    "https://www.pgnmentor.com/players/Geller.zip",
    "https://www.pgnmentor.com/players/Georgiev.zip",
    "https://www.pgnmentor.com/players/Giri.zip",
    "https://www.pgnmentor.com/players/Gligoric.zip",
    "https://www.pgnmentor.com/players/Goldin.zip",
    "https://www.pgnmentor.com/players/GrandaZuniga.zip",
    "https://www.pgnmentor.com/players/Grischuk.zip",
    "https://www.pgnmentor.com/players/Gukesh.zip",
    "https://www.pgnmentor.com/players/Gulko.zip",
    "https://www.pgnmentor.com/players/Gunsberg.zip",
    "https://www.pgnmentor.com/players/GurevichD.zip",
    "https://www.pgnmentor.com/players/GurevichM.zip",
    "https://www.pgnmentor.com/players/Harikrishna.zip",
    "https://www.pgnmentor.com/players/Hort.zip",
    "https://www.pgnmentor.com/players/Horwitz.zip",
    "https://www.pgnmentor.com/players/Hou.zip",
    "https://www.pgnmentor.com/players/Huebner.zip",
    "https://www.pgnmentor.com/players/Ibragimov.zip",
    "https://www.pgnmentor.com/players/IllescasCordoba.zip",
    "https://www.pgnmentor.com/players/Inarkiev.zip",
    "https://www.pgnmentor.com/players/Ivanchuk.zip",
    "https://www.pgnmentor.com/players/IvanovA.zip",
    "https://www.pgnmentor.com/players/IvanovI.zip",
    "https://www.pgnmentor.com/players/Ivkov.zip",
    "https://www.pgnmentor.com/players/Jakovenko.zip",
    "https://www.pgnmentor.com/players/Janowski.zip",
    "https://www.pgnmentor.com/players/Jobava.zip",
    "https://www.pgnmentor.com/players/Jussupow.zip",
    "https://www.pgnmentor.com/players/Kaidanov.zip",
    "https://www.pgnmentor.com/players/Kamsky.zip",
    "https://www.pgnmentor.com/players/Karjakin.zip",
    "https://www.pgnmentor.com/players/Karpov.zip",
    "https://www.pgnmentor.com/players/Kasimdzhanov.zip",
    "https://www.pgnmentor.com/players/Kasparov.zip",
    "https://www.pgnmentor.com/players/Kavalek.zip",
    "https://www.pgnmentor.com/players/Keres.zip",
    "https://www.pgnmentor.com/players/Keymer.zip",
    "https://www.pgnmentor.com/players/Khalifman.zip",
    "https://www.pgnmentor.com/players/Kholmov.zip",
    "https://www.pgnmentor.com/players/Koneru.zip",
    "https://www.pgnmentor.com/players/Korchnoi.zip",
    "https://www.pgnmentor.com/players/Korobov.zip",
    "https://www.pgnmentor.com/players/Kosteniuk.zip",
    "https://www.pgnmentor.com/players/Kotov.zip",
    "https://www.pgnmentor.com/players/Kramnik.zip",
    "https://www.pgnmentor.com/players/Krasenkow.zip",
    "https://www.pgnmentor.com/players/Krush.zip",
    "https://www.pgnmentor.com/players/Kudrin.zip",
    "https://www.pgnmentor.com/players/Lahno.zip",
    "https://www.pgnmentor.com/players/Larsen.zip",
    "https://www.pgnmentor.com/players/Lasker.zip",
    "https://www.pgnmentor.com/players/Lautier.zip",
    "https://www.pgnmentor.com/players/Le.zip",
    "https://www.pgnmentor.com/players/Leko.zip",
    "https://www.pgnmentor.com/players/Levenfish.zip",
    "https://www.pgnmentor.com/players/Li.zip",
    "https://www.pgnmentor.com/players/Lilienthal.zip",
    "https://www.pgnmentor.com/players/Ljubojevic.zip",
    "https://www.pgnmentor.com/players/Lputian.zip",
    "https://www.pgnmentor.com/players/MacKenzie.zip",
    "https://www.pgnmentor.com/players/Malakhov.zip",
    "https://www.pgnmentor.com/players/Mamedyarov.zip",
    "https://www.pgnmentor.com/players/Maroczy.zip",
    "https://www.pgnmentor.com/players/Marshall.zip",
    "https://www.pgnmentor.com/players/McDonnell.zip",
    "https://www.pgnmentor.com/players/McShane.zip",
    "https://www.pgnmentor.com/players/Mecking.zip",
    "https://www.pgnmentor.com/players/Mikenas.zip",
    "https://www.pgnmentor.com/players/Miles.zip",
    "https://www.pgnmentor.com/players/Milov.zip",
    "https://www.pgnmentor.com/players/Morozevich.zip",
    "https://www.pgnmentor.com/players/Morphy.zip",
    "https://www.pgnmentor.com/players/Motylev.zip",
    "https://www.pgnmentor.com/players/Movsesian.zip",
    "https://www.pgnmentor.com/players/Muzychuk.zip",
    "https://www.pgnmentor.com/players/Najdorf.zip",
    "https://www.pgnmentor.com/players/Najer.zip",
    "https://www.pgnmentor.com/players/Nakamura.zip",
    "https://www.pgnmentor.com/players/Navara.zip",
    "https://www.pgnmentor.com/players/Negi.zip",
    "https://www.pgnmentor.com/players/Nepomniachtchi.zip",
    "https://www.pgnmentor.com/players/Ni.zip",
    "https://www.pgnmentor.com/players/Nielsen.zip",
    "https://www.pgnmentor.com/players/Nikolic.zip",
    "https://www.pgnmentor.com/players/Nimzowitsch.zip",
    "https://www.pgnmentor.com/players/Nisipeanu.zip",
    "https://www.pgnmentor.com/players/Novikov.zip",
    "https://www.pgnmentor.com/players/Nunn.zip",
    "https://www.pgnmentor.com/players/Olafsson.zip",
    "https://www.pgnmentor.com/players/Oll.zip",
    "https://www.pgnmentor.com/players/Onischuk.zip",
    "https://www.pgnmentor.com/players/Pachman.zip",
    "https://www.pgnmentor.com/players/Paehtz.zip",
    "https://www.pgnmentor.com/players/Panno.zip",
    "https://www.pgnmentor.com/players/Paulsen.zip",
    "https://www.pgnmentor.com/players/Petrosian.zip",
    "https://www.pgnmentor.com/players/Philidor.zip",
    "https://www.pgnmentor.com/players/Pillsbury.zip",
    "https://www.pgnmentor.com/players/Pilnik.zip",
    "https://www.pgnmentor.com/players/PolgarJ.zip",
    "https://www.pgnmentor.com/players/PolgarS.zip",
    "https://www.pgnmentor.com/players/PolgarZ.zip",
    "https://www.pgnmentor.com/players/Polugaevsky.zip",
    "https://www.pgnmentor.com/players/Ponomariov.zip",
    "https://www.pgnmentor.com/players/Portisch.zip",
    "https://www.pgnmentor.com/players/Praggnanandhaa.zip",
    "https://www.pgnmentor.com/players/Psakhis.zip",
    "https://www.pgnmentor.com/players/Quinteros.zip",
    "https://www.pgnmentor.com/players/Radjabov.zip",
    "https://www.pgnmentor.com/players/Rapport.zip",
    "https://www.pgnmentor.com/players/Reshevsky.zip",
    "https://www.pgnmentor.com/players/Reti.zip",
    "https://www.pgnmentor.com/players/Ribli.zip",
    "https://www.pgnmentor.com/players/Rohde.zip",
    "https://www.pgnmentor.com/players/Rubinstein.zip",
    "https://www.pgnmentor.com/players/Rublevsky.zip",
    "https://www.pgnmentor.com/players/Saemisch.zip",
    "https://www.pgnmentor.com/players/Sakaev.zip",
    "https://www.pgnmentor.com/players/Salov.zip",
    "https://www.pgnmentor.com/players/Sasikiran.zip",
    "https://www.pgnmentor.com/players/Schlechter.zip",
    "https://www.pgnmentor.com/players/Seirawan.zip",
    "https://www.pgnmentor.com/players/Serper.zip",
    "https://www.pgnmentor.com/players/Shabalov.zip",
    "https://www.pgnmentor.com/players/Shamkovich.zip",
    "https://www.pgnmentor.com/players/Shirov.zip",
    "https://www.pgnmentor.com/players/Short.zip",
    "https://www.pgnmentor.com/players/Shulman.zip",
    "https://www.pgnmentor.com/players/Smirin.zip",
    "https://www.pgnmentor.com/players/Smyslov.zip",
    "https://www.pgnmentor.com/players/So.zip",
    "https://www.pgnmentor.com/players/Sokolov.zip",
    "https://www.pgnmentor.com/players/Soltis.zip",
    "https://www.pgnmentor.com/players/Spassky.zip",
    "https://www.pgnmentor.com/players/Speelman.zip",
    "https://www.pgnmentor.com/players/Spielmann.zip",
    "https://www.pgnmentor.com/players/Stahlberg.zip",
    "https://www.pgnmentor.com/players/Staunton.zip",
    "https://www.pgnmentor.com/players/Stefanova.zip",
    "https://www.pgnmentor.com/players/Stein.zip",
    "https://www.pgnmentor.com/players/Steinitz.zip",
    "https://www.pgnmentor.com/players/Suetin.zip",
    "https://www.pgnmentor.com/players/SultanKhan.zip",
    "https://www.pgnmentor.com/players/Sutovsky.zip",
    "https://www.pgnmentor.com/players/Svidler.zip",
    "https://www.pgnmentor.com/players/Szabo.zip",
    "https://www.pgnmentor.com/players/Taimanov.zip",
    "https://www.pgnmentor.com/players/Tal.zip",
    "https://www.pgnmentor.com/players/Tarrasch.zip",
    "https://www.pgnmentor.com/players/Tartakower.zip",
    "https://www.pgnmentor.com/players/Teichmann.zip",
    "https://www.pgnmentor.com/players/Timman.zip",
    "https://www.pgnmentor.com/players/Tiviakov.zip",
    "https://www.pgnmentor.com/players/Tkachiev.zip",
    "https://www.pgnmentor.com/players/Tomashevsky.zip",
    "https://www.pgnmentor.com/players/Topalov.zip",
    "https://www.pgnmentor.com/players/TorreRepetto.zip",
    "https://www.pgnmentor.com/players/Uhlmann.zip",
    "https://www.pgnmentor.com/players/Unzicker.zip",
    "https://www.pgnmentor.com/players/Ushenina.zip",
    "https://www.pgnmentor.com/players/VachierLagrave.zip",
    "https://www.pgnmentor.com/players/Vaganian.zip",
    "https://www.pgnmentor.com/players/VallejoPons.zip",
    "https://www.pgnmentor.com/players/VanWely.zip",
    "https://www.pgnmentor.com/players/Vitiugov.zip",
    "https://www.pgnmentor.com/players/Volokitin.zip",
    "https://www.pgnmentor.com/players/Waitzkin.zip",
    "https://www.pgnmentor.com/players/Wang.zip",
    "https://www.pgnmentor.com/players/WangH.zip",
    "https://www.pgnmentor.com/players/Wei.zip",
    "https://www.pgnmentor.com/players/Winawer.zip",
    "https://www.pgnmentor.com/players/Wojtaszek.zip",
    "https://www.pgnmentor.com/players/Wojtkiewicz.zip",
    "https://www.pgnmentor.com/players/Wolff.zip",
    "https://www.pgnmentor.com/players/Xie.zip",
    "https://www.pgnmentor.com/players/Xu.zip",
    "https://www.pgnmentor.com/players/Ye.zip",
    "https://www.pgnmentor.com/players/Yermolinsky.zip",
    "https://www.pgnmentor.com/players/Yu.zip",
    "https://www.pgnmentor.com/players/Yudasin.zip",
    "https://www.pgnmentor.com/players/Zhu.zip",
    "https://www.pgnmentor.com/players/Zukertort.zip",
    "https://www.pgnmentor.com/players/Zvjaginsev.zip",
]

# ── CLI ───────────────────────────────────────────────────────────────────────

ap = argparse.ArgumentParser()
ap.add_argument("--workers",       type=int, default=6,
                help="Parallel download threads (default 6)")
ap.add_argument("--skip-download", action="store_true",
                help="Skip downloading, only merge existing files")
ap.add_argument("--out-dir",       default="pgn_downloads",
                help="Folder to store downloaded PGNs")
ap.add_argument("--master-pgn",    default="master_games.pgn",
                help="Output merged PGN file")
ap.add_argument("--no-merge",      action="store_true",
                help="Download only, do not merge")
args = ap.parse_args()

OUT_DIR = Path(args.out_dir)
OUT_DIR.mkdir(exist_ok=True)

# ── Download ──────────────────────────────────────────────────────────────────

def download_one(url: str) -> tuple[str, bool, str]:
    name     = url.split("/")[-1].replace(".zip", "")
    zip_path = OUT_DIR / f"{name}.zip"
    pgn_path = OUT_DIR / f"{name}.pgn"

    if pgn_path.exists():
        return name, True, "cached"

    try:
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = resp.read()

        with open(zip_path, "wb") as f:
            f.write(data)

        with zipfile.ZipFile(zip_path) as z:
            pgn_files = [n for n in z.namelist() if n.lower().endswith(".pgn")]
            if not pgn_files:
                return name, False, "no PGN in zip"
            with z.open(pgn_files[0]) as src, open(pgn_path, "wb") as dst:
                dst.write(src.read())

        zip_path.unlink(missing_ok=True)
        return name, True, "downloaded"

    except Exception as e:
        return name, False, str(e)

if not args.skip_download:
    print(f"Downloading {len(PLAYER_URLS)} player PGNs -> {OUT_DIR}/")
    print(f"Using {args.workers} parallel workers ...\n")

    ok = fail = cached = 0
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = {pool.submit(download_one, url): url for url in PLAYER_URLS}
        for i, fut in enumerate(as_completed(futures), 1):
            name, success, status = fut.result()
            if success:
                if status == "cached":
                    cached += 1
                else:
                    ok += 1
            else:
                fail += 1
                print(f"  FAIL  {name}: {status}")
            if i % 20 == 0 or i == len(PLAYER_URLS):
                print(f"  [{i}/{len(PLAYER_URLS)}]  ok={ok}  cached={cached}  fail={fail}")

    print(f"\nDownload complete: {ok} new, {cached} cached, {fail} failed.\n")

# ── Merge ─────────────────────────────────────────────────────────────────────

if args.no_merge:
    print("--no-merge set, skipping merge step.")
    sys.exit(0)

pgn_files = sorted(OUT_DIR.glob("*.pgn"))
print(f"Merging {len(pgn_files)} PGN files -> {args.master_pgn} ...")

total_games = 0
with open(args.master_pgn, "w", encoding="utf-8", errors="ignore") as out:
    for pgn_path in pgn_files:
        try:
            text = pgn_path.read_text(encoding="utf-8", errors="ignore")
            # Count games by [Event tags
            count = text.count("\n[Event ") + (1 if text.startswith("[Event ") else 0)
            out.write(text)
            if not text.endswith("\n"):
                out.write("\n")
            total_games += count
        except Exception as e:
            print(f"  SKIP {pgn_path.name}: {e}")

print(f"Done. {total_games:,} games written to {args.master_pgn}")
print(f"\nNext step:")
print(f"  python build_book.py --pgn Petrosian.pgn --database {args.master_pgn} --freq 5 --seed-ply 20 --db-ply 40")
