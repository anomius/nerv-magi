#!/usr/bin/env bash
# Full, reproducible render of the promo: `pnpm run render` (from promo/).
#   1. fonts copied from assets/fonts, JP glyph coverage checked
#   2. footage recorded with `nerv tv --record` if any clip is missing (builds nerv first)
#   3. soundtrack: the march (MIDI → FluidSynth + FluidR3_GM) and the synthesized SFX, mixed and
#      loudness-normalized; then a sync check that every scripted hit is an onset on its frame
#   4. Remotion renders muted intermediates (concurrency 2: this machine has 15 GB)
#      (the picture does not depend on the music: after a score change, `render -- mux` is enough)
#   5. ffmpeg finishes them for X, plus poster, keyframes every 2 s and a contact sheet
# Pass targets to render only some of them: `pnpm run render -- master poster`
# (master 4x5 hero poster frames; `mux` re-applies a new soundtrack to existing renders, no Remotion).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=out
WORK=$OUT/.work
mkdir -p "$WORK" "$OUT/frames"
want() { [[ " $ONLY " == *" $1 "* ]]; }
ONLY="${*:-master 4x5 hero poster frames}"

bash scripts/sync-assets.sh
bash scripts/check-glyphs.sh

clips=(boot main main-crt load sync emergency resolve)
missing=0
for c in "${clips[@]}"; do [[ -f public/footage/$c.mp4 ]] || missing=1; done
for s in main-4k emergency-4k; do [[ -f public/footage/$s.png ]] || missing=1; done
if (( missing )); then
  echo "footage missing: building nerv and recording (takes ~6 min)"
  (cd .. && cargo build --release)
  bash scripts/record-footage.sh
fi

NODE=(node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON)
"${NODE[@]}" scripts/make-audio.mjs 2> >(grep -v '^ALSA' >&2)
"${NODE[@]}" scripts/check-sync.mjs                       # the music stem: every hit on its frame
"${NODE[@]}" scripts/check-sync.mjs public/audio/mix.wav  # and still after the SFX and ducking

remotion() { pnpm exec remotion render "$@" --concurrency 2 --log=warn; }
mute=(--codec h264 --crf 12 --x264-preset fast --pixel-format yuv420p --muted --props '{"audio":false}')

if want master; then
  remotion NervMagiX "$WORK/master-video.mp4" "${mute[@]}"
  bash scripts/finish.sh "$WORK/master-video.mp4" public/audio/score.wav "$OUT/nerv-magi-x.mp4" 6500
fi
if want 4x5; then
  remotion NervMagiX4x5 "$WORK/4x5-video.mp4" "${mute[@]}"
  bash scripts/finish.sh "$WORK/4x5-video.mp4" public/audio/score.wav "$OUT/nerv-magi-x-4x5.mp4" 5500
fi
# the loop's soundtrack: 7 s of the march from bar 17 (the callouts it shows), faded at the seam
LOOP_FROM=$("${NODE[@]}" --input-type=module -e 'import { bb } from "./src/timeline.ts"; console.log(bb(17) / 60)')
if want hero; then
  remotion HeroLoop "$WORK/hero-video.mp4" "${mute[@]}"
  bash scripts/finish.sh "$WORK/hero-video.mp4" - "$OUT/hero-loop-silent.mp4" 7000
  cp "$OUT/hero-loop-silent.mp4" "$OUT/hero-loop.mp4"
  bash scripts/mux-audio.sh "$OUT/hero-loop.mp4" public/audio/score.wav "$LOOP_FROM"
fi
if want mux; then
  for f in nerv-magi-x nerv-magi-x-4x5; do [[ -f $OUT/$f.mp4 ]] && bash scripts/mux-audio.sh "$OUT/$f.mp4" public/audio/score.wav; done
  if [[ -f $OUT/hero-loop-silent.mp4 ]]; then
    cp "$OUT/hero-loop-silent.mp4" "$OUT/hero-loop.mp4"
    bash scripts/mux-audio.sh "$OUT/hero-loop.mp4" public/audio/score.wav "$LOOP_FROM"
  fi
fi
if want poster; then
  pnpm exec remotion still NervMagiX "$OUT/poster.png" --frame="$("${NODE[@]}" --input-type=module -e 'import { T } from "./src/timeline.ts"; console.log(T.poster)')" --props '{"audio":false}' --log=warn
fi
if want frames; then
  rm -f "$OUT"/frames/*.png
  ffmpeg -hide_banner -loglevel error -i "$OUT/nerv-magi-x.mp4" -vf "fps=1/2" "$OUT/frames/master-%02d.png"
  [[ -f $OUT/nerv-magi-x-4x5.mp4 ]] && ffmpeg -hide_banner -loglevel error -i "$OUT/nerv-magi-x-4x5.mp4" -vf "fps=1/2" "$OUT/frames/4x5-%02d.png"
  ffmpeg -hide_banner -loglevel error -y -pattern_type glob -i "$OUT/frames/master-*.png" -vf "scale=480:-1,tile=5x6:padding=4:color=gray" -frames:v 1 "$OUT/contact-sheet.png"
  [[ -f $OUT/nerv-magi-x-4x5.mp4 ]] && ffmpeg -hide_banner -loglevel error -y -pattern_type glob -i "$OUT/frames/4x5-*.png" -vf "scale=270:-1,tile=8x4:padding=4:color=gray" -frames:v 1 "$OUT/contact-sheet-4x5.png"
fi

echo "== outputs"
for f in "$OUT"/*.mp4; do
  printf '%s  %s  ' "$f" "$(du -h "$f" | cut -f1)"
  ffprobe -v error -show_entries format=duration -show_entries stream=codec_name,profile,width,height,r_frame_rate,pix_fmt,sample_rate,channels,bit_rate -of compact=p=0:nk=1 "$f" | tr '\n' ' '
  echo
  if ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$f" | grep -q .; then
    ffmpeg -hide_banner -nostats -i "$f" -af ebur128=peak=true -f null - 2>&1 | grep -E "^\s+(I|Peak):" | tr -s ' ' | tr '\n' ' '
    echo
  fi
done
