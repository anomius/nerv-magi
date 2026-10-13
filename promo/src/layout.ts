import { useVideoConfig } from "remotion";

/** Frame geometry for the two cuts: 1920×1080 master and the 1080×1350 feed recompose. */
export const useLayout = () => {
  const { width: w, height: h, fps } = useVideoConfig();
  const portrait = h > w;
  return { w, h, fps, portrait, cx: w / 2, cy: h / 2 };
};

/** Pick a value: `lp(portrait, forPortrait, forLandscape)`. */
export const lp = <T,>(portrait: boolean, port: T, land: T): T => (portrait ? port : land);

/** Dashboard panels in 1920×1080 footage coordinates (measured on the MAIN view). */
export const PANELS = {
  magi: { x: 718, y: 100, w: 494, h: 340 },
  triad: { cx: 1052, cy: 290 },
  roster: { x: 12, y: 100, w: 655, h: 482 },
  activity: { x: 720, y: 448, w: 492, h: 258 },
  telemetry: { x: 1230, y: 355, w: 680, h: 335 },
  sdat: { x: 720, y: 715, w: 492, h: 290 },
  sectors: { x: 1230, y: 100, w: 680, h: 250 },
  card: { x: 30, y: 280, w: 900, h: 260 },
} as const;
export type Rect = { x: number; y: number; w: number; h: number };
