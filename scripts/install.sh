#!/usr/bin/env bash
# Build NERV MAGI and put `nerv` on your PATH (~/.local/bin), so the Omarchy bar widget, the herdr
# plugin and the scripts all find it. Safe to re-run after `git pull` / `omarchy plugin update`.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
say() { printf '\033[38;2;255;153;0m▍\033[0m %s\n' "$*"; }

command -v cargo >/dev/null || { echo "Rust is missing: install it with  omarchy pkg add rust  (or rustup), then re-run." >&2; exit 1; }
for dep in jq ffmpeg; do command -v "$dep" >/dev/null || say "optional: $dep is not installed (jq is needed by the bar widget)"; done

# Installed as an Omarchy plugin (~/.config/omarchy/plugins/…): build outside the plugin folder so
# the shell's plugin directory stays small and clean. A dev checkout builds into its own target/.
case "$ROOT" in
  "$HOME"/.config/*) export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/nerv-magi/target}" ;;
esac
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"

say "building nerv (release)…"
cargo build --release
install -Dm755 "$TARGET/release/nerv" "$HOME/.local/bin/nerv"
say "installed $HOME/.local/bin/nerv"

if command -v herdr >/dev/null && herdr status >/dev/null 2>&1; then
  say "herdr found: link the herdr plugin with   herdr plugin link \"$ROOT\""
fi
say "fonts: bundled in $ROOT/assets/fonts (OFL), nothing to download"
say "done. Try:  nerv tv --demo --windowed    ·    nerv hub"
