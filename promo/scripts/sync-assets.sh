#!/usr/bin/env bash
# Copy the repo's OFL fonts into promo/public/fonts (plain copies, never symlinks: the repo root is
# an Omarchy plugin and `omarchy plugin validate` rejects symlinks). The copies are gitignored.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p public/fonts
cp ../assets/fonts/*.ttf ../assets/fonts/OFL-*.txt public/fonts/
echo "fonts: $(ls public/fonts/*.ttf | wc -l) copied to promo/public/fonts"
