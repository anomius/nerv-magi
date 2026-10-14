#!/usr/bin/env bash
# Start `nerv tv` once, detached from herdr. Fullscreen starts go to `[tv] workspace` when it is
# set (e.g. the workspace on your TV's output); `--windowed` opens where you are.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
. "$ROOT/scripts/lib.sh"
BIN="$ROOT/target/release/nerv"
[[ -x "$BIN" ]] || BIN=$(command -v nerv 2>/dev/null || echo "$BIN")
if pid=$(tv_pid); then
  echo "nerv tv already running (pid $pid)"; exit 0
fi
[[ -x "$BIN" ]] || { echo "nerv is not built: run $ROOT/scripts/install.sh" >&2; exit 1; }
NERV_ASSETS="$ROOT/assets" setsid -f "$BIN" tv "$@" >/dev/null 2>&1 < /dev/null
echo "nerv tv started"

ws=$(conf_get workspace)
if [[ -n "$ws" && "${NERV_PLACE:-1}" == 1 && " $* " != *" --windowed "* ]] && command -v hyprctl >/dev/null; then
  addr=$(wait_tv_window) && move_tv_window "$addr" "$ws" && echo "nerv tv on workspace $ws" \
    && { fullscreen_tv_window "$addr" || echo "nerv tv: not fullscreen on workspace $ws (press f on it)" >&2; }
fi
exit 0
