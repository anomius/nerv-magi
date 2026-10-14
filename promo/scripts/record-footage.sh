#!/usr/bin/env bash
# Record the real dashboard footage the promo cuts from (headless, no window):
# `nerv tv --record` clips at 1920x1080 60 fps and 4K `--snapshot` stills for sharp push-ins.
# Output: promo/public/footage/ (gitignored). Needs a release build: `cargo build --release`.
#   record-footage.sh [--4k] [clip|stills|all]
# --4k: the same clips at 3840x2160 into promo/public/footage-4k/ (for the 4K master: Remotion --scale 2
# lays them in at their native resolution), plus the stills; main-crt is skipped (unused in the edit).
# At 4K the recorder is slower than real time, while the demo feed scripts its drama (the BLOCKED agent)
# on the process clock: so nerv runs under scripts/rec-clock (its clocks slowed to SLOWCLOCK_RATE, 0.2:
# it paces itself at 12 fps and every clip tells the same story as its 1080p twin; the "max lag" it
# prints is in that clock and should stay near 0), and x264 runs through scripts/rec-ffmpeg (ultrafast,
# CRF 10, light on memory) so the encoder keeps up.
#
# The demo feed runs on wall-clock time and scripts one BLOCKED agent at seconds 20-30 of every
# 40 s cycle (src/demo.rs), so --preroll picks the moment: 36 s lands after the first emergency
# with the telemetry graphs full; 18 s catches the real MAIN -> EMERGENCY takeover.
# Recording runs at ~0.6x real time at 1080p (x264 -preset slow), several times slower at 4K; clips
# are recorded one at a time, each after a memory check (scripts/mem-guard.sh).
set -euo pipefail
{ # parsed whole before it runs: editing this file cannot derail a recording in progress
cd "$(dirname "$0")/../.."
NERV=./target/release/nerv
[[ -x $NERV ]] || { echo "build first: cargo build --release" >&2; exit 1; }

size=1920x1080
OUT=promo/public/footage
k4=0
[[ ${1:-} == --4k ]] && { k4=1; size=3840x2160; OUT=promo/public/footage-4k; shift; PATH="$PWD/promo/scripts/rec-ffmpeg:$PATH"; }
mkdir -p "$OUT"
slow=()
if (( k4 )); then
  so=promo/out/.rec-clock/slowclock.so
  if [[ ! -f $so || promo/scripts/rec-clock/slowclock.c -nt $so ]]; then
    mkdir -p "$(dirname "$so")"
    cc -O2 -shared -fPIC -o "$so" promo/scripts/rec-clock/slowclock.c -ldl
  fi
  slow=(env LD_PRELOAD="$PWD/$so" SLOWCLOCK_RATE="${SLOWCLOCK_RATE:-0.2}")
fi

clean=(--scanlines off --cmd grain)   # no scanlines, film grain toggled off: sharp zooms
rec() {
  local name=$1; shift
  bash promo/scripts/mem-guard.sh 3
  echo "== $name ($size)"
  "${slow[@]}" "$NERV" tv --demo --size "$size" --fps 60 --record "$OUT/$name.mp4" "$@"
}
still() { local name=$1; shift; echo "== $name (4K still)"; "$NERV" tv --demo --size 3840x2160 --snapshot "$OUT/$name.png" "$@"; }

only=${1:-all}
want() { [[ $only == all || $only == "$1" ]]; }

want boot      && rec boot      --scene boot --seconds 5
want main      && rec main      --scene main --seconds 14 --preroll 36 "${clean[@]}"
(( k4 )) || { want main-crt && rec main-crt --scene main --seconds 14 --preroll 36 --scanlines crt; }
want load      && rec load      --scene load --seconds 7 --preroll 36
want sync      && rec sync      --scene sync --seconds 7 --preroll 36
want emergency && rec emergency --scene main --seconds 8 --preroll 18.5
want resolve   && rec resolve   --scene main --seconds 6 --preroll 32
want stills && {
  still main-4k      --scene main --frames $((60 * 38)) "${clean[@]}"
  still emergency-4k --scene main --frames $((60 * 24)) "${clean[@]}"
}
# where the scripted moments landed in this recording (clips.json, passed to the edit by the renders)
if [[ -f $OUT/emergency.mp4 && -f $OUT/resolve.mp4 ]]; then
  node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON promo/scripts/measure-clips.mjs "$OUT"
fi
ls -la "$OUT"
exit
}
