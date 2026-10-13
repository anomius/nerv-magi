#!/usr/bin/env bash
# Encode a muted Remotion intermediate into an X-ready file:
#   finish.sh <video-in> <audio.wav|-> <out.mp4> <video kbit/s>
# H.264 High, yuv420p, 2-second GOP, two-pass to a target bitrate (predictable size);
# AAC-LC 48 kHz stereo 192k when an audio file is given; -movflags +faststart.
set -euo pipefail
in=$1; audio=$2; out=$3; kbps=$4
work=$(dirname "$in")/x264-$(basename "$out" .mp4)
fps=$(ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate -of csv=p=0 "$in")
gop=$(( ${fps%/*} / ${fps#*/} * 2 ))
venc=(-c:v libx264 -profile:v high -preset slow -b:v "${kbps}k" -maxrate "$((kbps * 3 / 2))k" -bufsize "$((kbps * 2))k"
      -pix_fmt yuv420p -g "$gop" -keyint_min "$gop" -sc_threshold 0 -passlogfile "$work")
ffmpeg -hide_banner -loglevel error -y -i "$in" -an "${venc[@]}" -pass 1 -f mp4 /dev/null
if [[ $audio == - ]]; then
  ffmpeg -hide_banner -loglevel error -y -i "$in" -an "${venc[@]}" -pass 2 -movflags +faststart "$out"
else
  ffmpeg -hide_banner -loglevel error -y -i "$in" -i "$audio" -map 0:v:0 -map 1:a:0 "${venc[@]}" -pass 2 \
    -c:a aac -b:a 192k -ar 48000 -ac 2 -shortest -movflags +faststart "$out"
fi
rm -f "$work"*.log "$work"*.mbtree
echo "finish: $out $(du -h "$out" | cut -f1)"
