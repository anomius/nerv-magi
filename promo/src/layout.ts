import { createContext, useContext } from "react";
import { useVideoConfig } from "remotion";

/**
 * The 9:16 cut lays its type out in a stage: the frame minus the platforms' unsafe bands (no text in
 * the top 220 px and bottom 380 px of 1080×1920). The stage, 1080×1320, is laid out like the 4:5 cut;
 * backgrounds bleed past it to the frame's edges.
 */
export const TALL_SAFE = { top: 220, bottom: 380 } as const;
export type Stage = { w: number; h: number; top: number; bottom: number };
export const StageContext = createContext<Stage | null>(null);

/** Frame geometry for the cuts: 1920×1080 master, 1080×1350 feed recompose, 1080×1920 vertical (stage). */
export const useLayout = () => {
  const { width, height, fps } = useVideoConfig();
  const stage = useContext(StageContext);
  const w = stage?.w ?? width;
  const h = stage?.h ?? height;
  const portrait = h > w;
  // `bleed`: how far the frame reaches past the stage (backgrounds extend by it; type never does)
  return { w, h, fps, portrait, tall: stage !== null, bleed: { top: stage?.top ?? 0, bottom: stage?.bottom ?? 0 }, cx: w / 2, cy: h / 2 };
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
