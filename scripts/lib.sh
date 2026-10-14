# Shared by the scripts in this folder (sourced, not run).
# Runtime files: $XDG_RUNTIME_DIR, else a private /tmp/nerv-<uid> (same rule as control::runtime_dir).
NERV_RUN="${XDG_RUNTIME_DIR:-/tmp/nerv-$(id -u)}"
[[ -n "${XDG_RUNTIME_DIR:-}" ]] || mkdir -p -m 700 "$NERV_RUN"
PIDFILE="$NERV_RUN/nerv-tv.pid"
CMDFILE="$NERV_RUN/nerv-tv.cmd"
CONF="${HERDR_PLUGIN_CONFIG_DIR:-$HOME/.config/nerv}/config.toml"

# PID of the running `nerv tv`, only when the pidfile names a live process that really is nerv
# (a stale pidfile whose PID was reused by something else must never be signalled).
tv_pid() {
  local p
  p=$(cat "$PIDFILE" 2>/dev/null) || return 1
  [[ "$p" =~ ^[0-9]+$ && "$(cat "/proc/$p/comm" 2>/dev/null)" == nerv ]] || return 1
  echo "$p"
}

# A value from the [tv] table of config.toml, without quotes or a trailing # comment.
conf_get() {
  [[ -f "$CONF" ]] || return 0
  sed -n '/^\[tv\]/,/^\[/{
    /^[[:space:]]*'"$1"'[[:space:]]*=/{
      s/^[^=]*=[[:space:]]*//
      /^"/{ s/^"\([^"]*\)".*$/\1/p; d; }
      s/[[:space:]]*#.*$//
      s/[[:space:]]*$//
      p
    }
  }' "$CONF" | head -1
}

# Address of the nerv-tv window, waiting up to ~10 s for it to map.
wait_tv_window() {
  local a
  for _ in $(seq 1 40); do
    a=$(hyprctl clients -j 2>/dev/null | jq -r '[.[] | select(.class == "nerv-tv")][0].address // empty')
    [[ -n "$a" ]] && { echo "$a"; return 0; }
    sleep 0.25
  done
  return 1
}

# Move window <address> to workspace <name|number> without following it.
move_tv_window() {
  [[ "$1" =~ ^0x[[:xdigit:]]+$ && "$2" =~ ^[[:alnum:]:_.-]+$ ]] || { echo "refusing window '$1' / workspace '$2'" >&2; return 1; }
  local out
  # Hyprland ≥ 0.55 (Lua config) dispatch, else the classic dispatcher.
  out=$(hyprctl dispatch "hl.dsp.window.move({ workspace = \"$2\", follow = false, window = \"address:$1\" })" 2>&1)
  [[ "$out" == ok ]] || hyprctl dispatch movetoworkspacesilent "$2,address:$1" >/dev/null
}

# Hyprland's fullscreen mode of window <address> (0 = not fullscreen), empty when it is gone.
tv_fullscreen_state() {
  hyprctl clients -j 2>/dev/null | jq -r --arg a "$1" 'first(.[] | select(.address == $a) | .fullscreen) // empty'
}

# After a move to another monitor's workspace (e.g. the TV): Hyprland drops the fullscreen there,
# and a window that asks for fullscreen takes the focus. Ask again through the control channel
# only when the window is not fullscreen, then hand the focus back to the monitor that had it.
fullscreen_tv_window() {
  [[ "$1" =~ ^0x[[:xdigit:]]+$ ]] || { echo "refusing window '$1'" >&2; return 1; }
  [[ "$(conf_get start_fullscreen)" != false && "$(tv_fullscreen_state "$1")" == 0 ]] || return 0
  local pid mon s out
  pid=$(tv_pid) || return 1
  mon=$(hyprctl monitors -j 2>/dev/null | jq -r 'first(.[] | select(.focused) | .name) // empty')
  echo fullscreen >> "$CMDFILE"
  kill -USR2 "$pid"
  for _ in $(seq 1 40); do
    s=$(tv_fullscreen_state "$1")
    [[ -n "$s" && "$s" != 0 ]] && break
    sleep 0.1
  done
  # The focus can move a moment after the fullscreen state does: keep handing it back for ~1 s.
  if [[ "$mon" =~ ^[[:alnum:]_.-]+$ ]]; then
    for _ in $(seq 1 10); do
      if [[ "$(hyprctl monitors -j 2>/dev/null | jq -r 'first(.[] | select(.focused) | .name) // empty')" != "$mon" ]]; then
        out=$(hyprctl dispatch "hl.dsp.focus({ monitor = \"$mon\" })" 2>&1)
        [[ "$out" == ok ]] || hyprctl dispatch focusmonitor "$mon" >/dev/null
      fi
      sleep 0.1
    done
  fi
  [[ -n "$s" && "$s" != 0 ]]
}
