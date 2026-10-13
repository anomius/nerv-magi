#!/usr/bin/env bash
# Every CJK / kana character used in promo/src must exist in the Noto Serif JP subset
# (assets/fonts/NotoSerifJP-Black-subset.ttf); otherwise it would render as tofu. To add glyphs:
# append them to assets/fonts/jp-glyphs.txt and re-run scripts/fetch-fonts.sh.
set -euo pipefail
cd "$(dirname "$0")/../.."
uvx --quiet --from fonttools python - <<'PY'
import pathlib, re, sys
from fontTools.ttLib import TTFont
cmap = TTFont("assets/fonts/NotoSerifJP-Black-subset.ttf").getBestCmap()
used = set()
for p in pathlib.Path("promo/src").rglob("*.ts*"):
    used |= set(re.findall(r"[　-ヿ㐀-鿿＀-￯]", p.read_text()))
missing = sorted(c for c in used if ord(c) not in cmap)
print(f"glyphs: {len(used)} JP characters used, {len(missing)} missing" + (f": {''.join(missing)}" if missing else ""))
sys.exit(1 if missing else 0)
PY
