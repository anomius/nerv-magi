import { loadFont } from "@remotion/fonts";
import { staticFile } from "remotion";

/** The repo the video points at. Keep it here only: the repo name may change. */
export const REPO_URL = "github.com/anomius/nerv-magi";

/** Palette of the TV screen (src/tv/gfx.rs). */
export const C = {
  black: "#000000",
  panel: "#0A0A0A",
  orange: "#FF9900",
  amber: "#FFAA00",
  green: "#00FF00",
  lcd: "#39FF14",
  cyan: "#00FFFF",
  red: "#FF0000",
  white: "#E0E0E0",
  timer: "#FFC400",
} as const;

/** Only the OFL fonts of assets/fonts (copied into public/fonts by scripts/sync-assets.sh). */
export const F = {
  head: "Oswald",
  cond: "Barlow Condensed",
  mono: "Fira Code",
  serif: "Noto Serif JP",
  seg: "DSEG7",
} as const;

const font = (family: string, file: string, weight: string, style = "normal") =>
  loadFont({ family, url: staticFile(`fonts/${file}`), weight, style, display: "block" });

// loadFont() holds the render (delayRender) until each face is ready.
font(F.head, "Oswald-Medium.ttf", "500");
font(F.head, "Oswald-Bold.ttf", "700");
font(F.cond, "BarlowCondensed-Medium.ttf", "500");
font(F.cond, "BarlowCondensed-SemiBold.ttf", "600");
font(F.mono, "FiraCode-Medium.ttf", "500");
font(F.serif, "NotoSerifJP-Black-subset.ttf", "900");
font(F.seg, "DSEG7Classic-BoldItalic.ttf", "700", "italic");

export const glow = (color: string, r = 18) => `0 0 ${r * 0.4}px ${color}, 0 0 ${r}px ${color}`;
