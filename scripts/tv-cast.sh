#!/usr/bin/env bash
# One step: put the NERV dashboard fullscreen on the TV over AirPlay.
#   1. start omarchy-cast's "second desktop" on the receiver (unless it is already up)
#   2. start `nerv tv` and move it onto the TV's workspace (your focus stays where it is)
#   3. watch the cast: when the TV goes away (turned off, input switched), stop the dashboard
#      instead of letting it land on the laptop panel.
# Receiver: $1, else [tv] cast_receiver in config.toml, else the first paired receiver.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CTL="$HOME/.config/omarchy/plugins/blacksheep.airplay/bin/airplay-ctl"
. "$ROOT/scripts/lib.sh"
fail() { echo "tv-cast: $*" >&2; exit 1; }
[[ -x "$CTL" ]] || fail "omarchy-cast is not installed (missing $CTL)"
command -v jq >/dev/null || fail "jq is required"

status=$("$CTL" status)
host=${1:-$(conf_get cast_receiver)}
[[ -n "$host" ]] || host=$(jq -r '.paired[0] // empty' <<<"$status")
[[ -n "$host" ]] || fail "no AirPlay receiver: pair one from the bar (AirPlay icon) first"
audio=$([[ "$(conf_get cast_audio)" == "true" ]] && echo on || echo off)

ws=$(jq -r --arg h "$host" 'if .session != null and .session.kind == "extend" and .session.receiver == $h then .session.workspace else empty end' <<<"$status")
if [[ -z "$ws" ]]; then
  if [[ "$(jq -r '.unit_active' <<<"$status")" == "true" ]]; then
    fail "another AirPlay session is running; stop it from the bar first"
  fi
  echo "tv-cast: connecting to $host (second desktop, audio $audio)…"
  # A receiver that has just ended a session refuses the next one for a few seconds
  # (documented by the sender's author; seen on the TCL too): retry a refused start.
  for attempt in 1 2 3 4; do
  since=$(date '+%Y-%m-%d %H:%M:%S')
  out=$("$CTL" start "$host" extend "" "audio=$audio" 2>&1) || fail "airplay start failed: $out"
  for _ in $(seq 1 40); do
    st=$("$CTL" status)
    ws=$(jq -r '.session.workspace // empty' <<<"$st")
    [[ -n "$ws" ]] && break
    if [[ "$(jq -r '.unit_active' <<<"$st")" != "true" ]]; then
      why=$(journalctl --user -u blacksheep-airplay-session --since "$since" --no-pager 2>/dev/null | grep -m1 -oE 'error: .*' || true)
      case "$why" in
        *"Connection refused"*)
          [[ $attempt -lt 4 ]] && { echo "tv-cast: TV refused (just ended a session or still waking); retrying in 5 s…"; sleep 5; continue 2; }
          fail "the TV is not accepting AirPlay right now (standby?): turn it on and retry" ;;
        *) fail "the AirPlay session ended during setup: ${why:-see journalctl --user -u blacksheep-airplay-session}" ;;
      esac
    fi
    sleep 0.5
  done
  break
  done
  [[ -n "$ws" ]] || fail "the TV did not come up within 20 s (is it on, and on this network?)"
fi
echo "tv-cast: TV desktop is workspace $ws"

NERV_PLACE=0 "$ROOT/scripts/tv-start.sh" >/dev/null
addr=$(wait_tv_window) || fail "nerv tv window did not appear"
move_tv_window "$addr" "$ws" || fail "could not move the dashboard to workspace $ws"
fullscreen_tv_window "$addr" || echo "tv-cast: warning: the dashboard is not fullscreen on the TV (press f on it)" >&2

# Close the dashboard when the cast ends (one transient user unit; replaced on re-run).
systemctl --user stop nerv-tv-cast-watch.service >/dev/null 2>&1 || true
systemd-run --user --quiet --collect --unit=nerv-tv-cast-watch \
  bash -c 'while "$1" status | jq -e .unit_active >/dev/null; do sleep 5; done; "$2"' nerv-cast-watch "$CTL" "$ROOT/scripts/tv-stop.sh" \
  || echo "tv-cast: warning: could not start the cast watcher; the dashboard will not close when the TV turns off" >&2
echo "tv-cast: NERV MAGI is on the TV (workspace $ws)"
