#!/usr/bin/env bash
# Render one cut of the film on one track map, with every soundtrack it can have:
#   render-cut.sh <cut> [tracks/x.json] [--4k] [--audio-only]
#   cuts: master (1920×1080), 4x5 (1080×1350), vertical (1080×1920), teaser15, cut30 (1920×1080, src/edits.ts)
#
# The picture is rendered once, muted (Remotion, the map as input props), encoded once, then muxed with:
#   <base>.mp4            the medley (default map only; spliced on the cut's bar lines for a short cut)
#   <base>-sfx-only.mp4   the synthesized SFX alone, on this timeline
#   <base>-silent.mp4     no audio track
#   <base>-guide.mp4      SFX + a click on every beat (whole film only), and click.wav beside it
#   <base>-preview.mp4    the map's own audio laid in at offsetSec (if the map's `audio` file exists)
#   cues.txt, cues.csv    the cue sheet
# Output: out/formats/<cut>/ for the default map, out/tracks/<name>/ (master) or out/tracks/<name>/<cut>/;
# --4k: out/4k/ (footage from public/footage-4k, Remotion --scale 2, H.264 High CRF 16 instead of 2-pass).
# --audio-only: just the sound files (click.wav, check.wav = the map's audio + click, ...) and the cue
# sheet, in seconds: confirm a map by ear before rendering the picture.
# Memory: waits for > 3 GB available before rendering; Remotion --concurrency 2 (1 at 4K).
set -euo pipefail
{ # the whole script is parsed before it runs: editing this file cannot derail a render in progress
cd "$(dirname "$0")/.."
NODE=(node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON)

[[ ${1:-} == -- ]] && shift # pnpm 10 passes the separator through
cut=${1:?usage: render-cut.sh <master|4x5|vertical|teaser15|cut30> [tracks/x.json] [--4k]}
shift
track=""
k4=0
audio_only=0
for a in "$@"; do
  case $a in
    --4k) k4=1 ;;
    --audio-only) audio_only=1 ;;
    *.json) track=$a ;;
    *) echo "render-cut: unknown argument $a" >&2; exit 2 ;;
  esac
done
edit=""
case $cut in
  master) comp=NervMagiX; kbps=6500 ;;
  4x5) comp=NervMagiX4x5; kbps=5500 ;;
  vertical) comp=NervMagiVertical; kbps=8000 ;;
  teaser15) comp=Teaser15; edit=teaser15; kbps=6500 ;;
  cut30) comp=Cut30; edit=cut30; kbps=6500 ;;
  *) echo "render-cut: unknown cut $cut" >&2; exit 2 ;;
esac

name=$("${NODE[@]}" --input-type=module -e "import { loadTrack } from './scripts/lib/track.mjs'; console.log(loadTrack(process.argv[1] || undefined).map.name)" "$track")
default=0
[[ -z $track || $name == house-150 ]] && default=1
if (( k4 )); then
  dir=out/4k; base=nerv-magi-${cut}-4k; [[ $cut == master ]] && base=nerv-magi-x-4k
  (( default )) || { dir=out/tracks/$name/4k-$cut; base=nerv-magi-$name-$cut-4k; }
elif (( default )); then
  dir=out/formats/$cut; base=nerv-magi-$cut
else
  dir=out/tracks/$name; base=nerv-magi-$name
  [[ $cut == master ]] || { dir=$dir/$cut; base=$base-$cut; }
fi
work=$dir/.work
mkdir -p "$work"
footage=footage
(( k4 )) && footage=footage-4k
echo "== $cut on \"$name\" → $dir/$base*.mp4"

bash scripts/sync-assets.sh
bash scripts/check-glyphs.sh
"${NODE[@]}" scripts/preflight.mjs ${track:+"$track"} --footage "public/$footage" || {
  (( k4 )) && echo "4K footage: bash scripts/record-footage.sh --4k" >&2
  exit 1
}

# sound: the medley stems must exist for the default map
if (( default )) && [[ ! -f public/audio/mix.wav ]]; then "${NODE[@]}" scripts/make-audio.mjs 2> >(grep -v '^ALSA' >&2); fi
"${NODE[@]}" scripts/cut-audio.mjs --out "$work/audio" ${track:+--track "$track"} ${edit:+--edit "$edit"}
"${NODE[@]}" scripts/cue-sheet.mjs ${track:+"$track"} ${edit:+--edit "$edit"} --out "$dir" --title "NERV MAGI promo, $cut"
if (( audio_only )); then
  for w in click check guide; do [[ -f $work/audio/$w.wav ]] && cp "$work/audio/$w.wav" "$dir/$w.wav"; done
  [[ -f $work/audio/track.wav ]] && cp "$work/audio/track.wav" "$dir/preview.wav"
  echo "== $dir (audio only)"; ls -la "$dir"
  exit 0
fi

# picture: muted, the map as input props
"${NODE[@]}" --input-type=module -e "
  import { existsSync, readFileSync, writeFileSync } from 'node:fs';
  import { loadTrack } from './scripts/lib/track.mjs';
  const { map } = loadTrack(process.argv[1] || undefined);
  const marks = 'public/' + process.argv[3] + '/clips.json';
  const clips = existsSync(marks) ? JSON.parse(readFileSync(marks, 'utf8')) : undefined;
  writeFileSync(process.argv[2], JSON.stringify({ audio: false, footage: process.argv[3], track: map, clips }));
" "$track" "$work/props.json" "$footage"
bash scripts/mem-guard.sh 3
video=$work/video.mp4
if (( k4 )); then
  # memory: 4K frames are 33 MB each; the decoded-frame caches are capped, and the intermediate is
  # x264 ultrafast at CRF 8 (preset fast holds ~2.4 GB at 4K; the final encode below is separate)
  pnpm exec remotion render "$comp" "$video" --props "$work/props.json" --scale 2 --concurrency 1 --jpeg-quality 95 \
    --media-cache-size-in-bytes 536870912 --offthreadvideo-cache-size-in-bytes 536870912 \
    --codec h264 --crf 8 --x264-preset ultrafast --pixel-format yuv420p --muted --log=warn
else
  pnpm exec remotion render "$comp" "$video" --props "$work/props.json" --concurrency 2 \
    --codec h264 --crf 12 --x264-preset fast --pixel-format yuv420p --muted --log=warn
fi

# encode once (silent), then mux each soundtrack onto a copy of that video stream
silent=$dir/$base-silent.mp4
bash scripts/mem-guard.sh 3
if (( k4 )); then
  # YouTube: H.264 High, closed GOP of half the frame rate, 2 B-frames, CRF 16, +faststart
  ffmpeg -hide_banner -loglevel error -y -i "$video" -an -c:v libx264 -profile:v high -level:v 5.2 -preset slow -crf 16 \
    -pix_fmt yuv420p -g 30 -keyint_min 30 -bf 2 -flags +cgop -sc_threshold 0 -movflags +faststart "$silent"
  echo "finish: $silent $(du -h "$silent" | cut -f1)"
else
  bash scripts/finish.sh "$video" - "$silent" "$kbps"
fi
with() { # with <suffix> <audio.wav>
  local out=$dir/$base$1.mp4
  cp "$silent" "$out"
  bash scripts/mux-audio.sh "$out" "$2" >/dev/null
  echo "mux: $out"
}
a=$work/audio
[[ -f $a/medley.wav ]] && with "" "$a/medley.wav"
with -sfx-only "$a/sfx.wav"
[[ -f $a/guide.wav ]] && { with -guide "$a/guide.wav"; cp "$a/click.wav" "$dir/click.wav"; }
[[ -f $a/track.wav ]] && with -preview "$a/track.wav"

echo "== $dir"
for f in "$dir"/$base*.mp4; do
  printf '%s  %s  ' "$f" "$(du -h "$f" | cut -f1)"
  ffprobe -v error -show_entries format=duration -show_entries stream=codec_name,profile,width,height,r_frame_rate,nb_frames,sample_rate,channels -of compact=p=0:nk=1 "$f" | tr '\n' ' '
  echo
  if ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 "$f" | grep -q .; then
    ffmpeg -hide_banner -nostats -i "$f" -af ebur128=peak=true -f null - 2>&1 | grep -E "^\s+(I|Peak):" | tr -s ' ' | tr '\n' ' '
    echo
  fi
done
exit
}
