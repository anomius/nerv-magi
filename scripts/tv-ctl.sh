#!/usr/bin/env bash
# Control the running TV window from anywhere (herdr actions, keybindings, ssh):
#   tv-ctl.sh main|load|sync|emergency   show and pin that view
#   tv-ctl.sh next|prev                  step through views (pins)
#   tv-ctl.sh auto                       resume automatic rotation
#   tv-ctl.sh pin                        pin / unpin the current view
#   tv-ctl.sh fullscreen                 toggle fullscreen / window
#   tv-ctl.sh scanlines                  cycle scanline look
#   tv-ctl.sh grain                      film grain on / off
#   tv-ctl.sh help                       toggle the on-screen key help
set -euo pipefail
cmd=${1:-}
case "$cmd" in
  main|load|sync|emergency|next|prev|auto|pin|fullscreen|scanlines|grain|help) ;;
  *) echo "usage: tv-ctl.sh main|load|sync|emergency|next|prev|auto|pin|fullscreen|scanlines|grain|help" >&2; exit 2 ;;
esac
. "$(dirname "$0")/lib.sh"
if pid=$(tv_pid); then
  echo "$cmd" >> "$CMDFILE"
  kill -USR2 "$pid"
  echo "nerv tv: $cmd"
else
  echo "nerv tv not running"
fi
