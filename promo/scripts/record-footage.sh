#!/usr/bin/env bash
# Record the real dashboard footage the promo cuts from (headless, no window):
# `nerv tv --record` clips at 1920x1080 60 fps and 4K `--snapshot` stills for sharp push-ins.
# Output: promo/public/footage/ (gitignored). Needs a release build: `cargo build --release`.
#
# The demo feed runs on wall-clock time and scripts one BLOCKED agent at seconds 20-30 of every
# 40 s cycle (src/demo.rs), so --preroll picks the moment: 36 s lands after the first emergency
# with the telemetry graphs full; 18 s catches the real MAIN -> EMERGENCY takeover.
# Recording runs at ~0.6x real time (x264 -preset slow); run clips one at a time (memory).
set -euo pipefail
cd "$(dirname "$0")/../.."
NERV=./target/release/nerv
OUT=promo/public/footage
mkdir -p "$OUT"
[[ -x $NERV ]] || { echo "build first: cargo build --release" >&2; exit 1; }

clean=(--scanlines off --cmd grain)   # no scanlines, film grain toggled off: sharp zooms
rec() { local name=$1; shift; echo "== $name"; "$NERV" tv --demo --size 1920x1080 --fps 60 --record "$OUT/$name.mp4" "$@"; }
still() { local name=$1; shift; echo "== $name (4K still)"; "$NERV" tv --demo --size 3840x2160 --snapshot "$OUT/$name.png" "$@"; }

only=${1:-all}
want() { [[ $only == all || $only == "$1" ]]; }

want boot      && rec boot      --scene boot --seconds 5
want main      && rec main      --scene main --seconds 14 --preroll 36 "${clean[@]}"
want main-crt  && rec main-crt  --scene main --seconds 14 --preroll 36 --scanlines crt
want load      && rec load      --scene load --seconds 7 --preroll 36
want sync      && rec sync      --scene sync --seconds 7 --preroll 36
want emergency && rec emergency --scene main --seconds 8 --preroll 18.5
want resolve   && rec resolve   --scene main --seconds 6 --preroll 32
want stills && {
  still main-4k      --scene main --frames $((60 * 38)) "${clean[@]}"
  still emergency-4k --scene main --frames $((60 * 24)) "${clean[@]}"
}
ls -la "$OUT"
