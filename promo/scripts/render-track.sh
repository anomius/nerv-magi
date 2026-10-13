#!/usr/bin/env bash
# Re-time the film to a track: `pnpm run render:track -- tracks/<name>.json [cuts...]`
# Renders the cut(s) (default: master) on the map's timeline with scripts/render-cut.sh: the video with
# the SFX only, a click guide (click.wav) and the video with it mixed in, the cue sheet, and, when the
# map's `audio` file exists, a private preview with that audio laid in at offsetSec.
# Output: out/tracks/<name>/. Cuts: master 4x5 vertical teaser15 cut30.
# --audio-only: the click, check.wav (your audio + the click) and the cue sheet, in seconds, no picture.
set -euo pipefail
{ # the whole script is parsed before it runs: editing this file cannot derail a render in progress
cd "$(dirname "$0")/.."
[[ ${1:-} == -- ]] && shift # pnpm 10 passes the separator through
map=${1:?usage: render-track.sh tracks/<name>.json [master 4x5 vertical teaser15 cut30]}
shift
[[ -f $map ]] || { echo "render-track: no such map: $map" >&2; exit 2; }
flags=()
cuts=()
for a in "$@"; do [[ $a == --* ]] && flags+=("$a") || cuts+=("$a"); done
(( ${#cuts[@]} )) || cuts=(master)
for c in "${cuts[@]}"; do bash scripts/render-cut.sh "$c" "$map" "${flags[@]}"; done
exit
}
