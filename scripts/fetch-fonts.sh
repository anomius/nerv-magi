#!/usr/bin/env bash
# Rebuild assets/fonts from their source git repos (all OFL-1.1).
#   google/fonts @ pinned commit: Oswald, Barlow Condensed, Fira Code, Noto Serif JP
#   keshikan/DSEG v0.46: DSEG7 Classic
# Variable fonts are frozen to static weights (femtovg cannot pick variable axes) and
# Noto Serif JP is subset to kana + the kanji in assets/fonts/jp-glyphs.txt.
# Needs: curl, unzip, uvx (fonttools is fetched by uvx).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/assets/fonts"
GF_SHA=66928797f271fed5ee428544e7229685e2d90127
GF="https://raw.githubusercontent.com/google/fonts/$GF_SHA/ofl"
DSEG_ZIP=https://github.com/keshikan/DSEG/releases/download/v0.46/fonts-DSEG_v046.zip
DL=$(mktemp -d)   # downloads stay in their own empty dir
trap 'rm -rf "$DL"' EXIT
ft() { uvx --quiet --from fonttools "$@"; }

fetch() { curl -fsSL "$1" -o "$2"; }
fetch "$GF/oswald/Oswald%5Bwght%5D.ttf"            "$DL/Oswald-VF.ttf"
fetch "$GF/oswald/OFL.txt"                          "$OUT/OFL-Oswald.txt"
fetch "$GF/barlowcondensed/BarlowCondensed-Medium.ttf"   "$OUT/BarlowCondensed-Medium.ttf"
fetch "$GF/barlowcondensed/BarlowCondensed-SemiBold.ttf" "$OUT/BarlowCondensed-SemiBold.ttf"
fetch "$GF/barlowcondensed/OFL.txt"                 "$OUT/OFL-BarlowCondensed.txt"
fetch "$GF/firacode/FiraCode%5Bwght%5D.ttf"         "$DL/FiraCode-VF.ttf"
fetch "$GF/firacode/OFL.txt"                        "$OUT/OFL-FiraCode.txt"
fetch "$GF/notoserifjp/NotoSerifJP%5Bwght%5D.ttf"   "$DL/NotoSerifJP-VF.ttf"
fetch "$GF/notoserifjp/OFL.txt"                     "$OUT/OFL-NotoSerifJP.txt"
fetch "$DSEG_ZIP" "$DL/dseg.zip"
unzip -q -o "$DL/dseg.zip" -d "$DL/dseg"
cp "$(find "$DL/dseg" -name 'DSEG7Classic-BoldItalic.ttf' | head -1)" "$OUT/"
cp "$(find "$DL/dseg" -name 'DSEG-LICENSE.txt' | head -1)" "$OUT/OFL-DSEG.txt"

ft fonttools varLib.instancer "$DL/Oswald-VF.ttf"   wght=500 --static -q -o "$OUT/Oswald-Medium.ttf"
ft fonttools varLib.instancer "$DL/Oswald-VF.ttf"   wght=700 --static -q -o "$OUT/Oswald-Bold.ttf"
ft fonttools varLib.instancer "$DL/FiraCode-VF.ttf" wght=500 --static -q -o "$OUT/FiraCode-Medium.ttf"
ft fonttools varLib.instancer "$DL/NotoSerifJP-VF.ttf" wght=900 --static -q -o "$DL/NotoSerifJP-Black.ttf"
ft pyftsubset "$DL/NotoSerifJP-Black.ttf" \
  --text-file="$OUT/jp-glyphs.txt" \
  --unicodes="U+0020-007E,U+3000-303F,U+3040-309F,U+30A0-30FF,U+FF00-FFEF,U+00B7,U+2026" \
  --layout-features='*' --output-file="$OUT/NotoSerifJP-Black-subset.ttf"
ls -la "$OUT"
