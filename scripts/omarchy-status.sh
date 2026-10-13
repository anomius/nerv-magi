#!/usr/bin/env bash
# One JSON line for the Omarchy bar widget (BarWidget.qml / Panel.qml):
#   {herdr, agents:[{name,kind,status,workspace,pane}], tv, built, airplay, cast, root, bin}
# `--full` also asks omarchy-cast for the AirPlay session (slower; only while the panel is open).
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN="$ROOT/target/release/nerv"
[[ -x "$BIN" ]] || BIN=$(command -v nerv 2>/dev/null || echo "$BIN")
. "$ROOT/scripts/lib.sh"
CTL="$HOME/.config/omarchy/plugins/blacksheep.airplay/bin/airplay-ctl"

agents='[]'
herdr=false
if command -v herdr >/dev/null 2>&1 \
  && a=$(timeout 2 herdr agent list 2>/dev/null) \
  && w=$(timeout 2 herdr workspace list 2>/dev/null); then
  herdr=true
  agents=$(jq -c --argjson ws "$w" '
    ($ws.result.workspaces | map({key: .workspace_id, value: .label}) | from_entries) as $label
    | [.result.agents[] | {
        name: (.name // .pane_id), kind: .agent, status: .agent_status,
        workspace: ($label[.workspace_id] // .workspace_id), pane: .pane_id }]' <<<"$a" 2>/dev/null) || agents='[]'
fi

tv=false
tv_pid >/dev/null && tv=true
built=false
[[ -x "$BIN" ]] && built=true
airplay=false
[[ -x "$CTL" ]] && airplay=true

cast=null
if [[ "${1:-}" == "--full" && $airplay == true ]]; then
  cast=$(timeout 3 "$CTL" status 2>/dev/null \
    | jq -c 'if .session != null then {receiver: .session.receiver, workspace: .session.workspace, kind: .session.kind} else null end' 2>/dev/null) \
    || cast=null
  [[ -n "$cast" ]] || cast=null
fi

jq -cn --argjson herdr "$herdr" --argjson agents "$agents" --argjson tv "$tv" --argjson built "$built" \
  --argjson airplay "$airplay" --argjson cast "$cast" --arg root "$ROOT" --arg bin "$BIN" \
  '{herdr: $herdr, agents: $agents, tv: $tv, built: $built, airplay: $airplay, cast: $cast, root: $root, bin: $bin}'
