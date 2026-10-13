#!/usr/bin/env bash
# Put a new soundtrack on finished renders without touching the picture:
#   mux-audio.sh <video.mp4> <audio.wav> [start_s]
# video stream copied, AAC-LC 192k 48 kHz stereo, +faststart. With start_s, a clip of the audio as long
# as the video is taken from there (the hero loop), with short fades so its seam does not click.
set -euo pipefail
v=$1; a=$2; start=${3:-}
tmp=$(dirname "$v")/.mux-$(basename "$v")
if [[ -n $start ]]; then
  dur=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$v")
  af="atrim=start=$start:duration=$dur,asetpts=PTS-STARTPTS,afade=t=in:d=0.03,afade=t=out:st=$(awk "BEGIN{print $dur - 0.15}"):d=0.15"
  ffmpeg -hide_banner -loglevel error -y -i "$v" -i "$a" -map 0:v:0 -map 1:a:0 -c:v copy -af "$af" -c:a aac -b:a 192k -ar 48000 -ac 2 -shortest -movflags +faststart "$tmp"
else
  ffmpeg -hide_banner -loglevel error -y -i "$v" -i "$a" -map 0:v:0 -map 1:a:0 -c:v copy -c:a aac -b:a 192k -ar 48000 -ac 2 -shortest -movflags +faststart "$tmp"
fi
mv "$tmp" "$v"
echo "mux: $v $(du -h "$v" | cut -f1)"
