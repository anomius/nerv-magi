#!/usr/bin/env bash
# Stop the dashboard and the AirPlay cast that tv-cast.sh started.
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CTL="$HOME/.config/omarchy/plugins/blacksheep.airplay/bin/airplay-ctl"
systemctl --user stop nerv-tv-cast-watch.service >/dev/null 2>&1
"$ROOT/scripts/tv-stop.sh"
[[ -x "$CTL" ]] && "$CTL" stop
