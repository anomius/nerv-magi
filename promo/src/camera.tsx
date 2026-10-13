import React from "react";
import { interpolate } from "remotion";
import { IN_OUT, clamp } from "./fx";
import type { Rect } from "./layout";

/** A camera over 1920×1080 footage space: footage point (fx, fy) lands on screen point (ax, ay). */
export type Cam = { zoom: number; fx: number; fy: number; ax: number; ay: number };
export type Key = Cam & { f: number };

/** Camera at frame `f` between keyframes (eased in-out per segment, held outside). */
export const camAt = (keys: Key[], f: number): Cam => {
  if (f <= keys[0].f) return keys[0];
  for (let k = 1; k < keys.length; k++) {
    const a = keys[k - 1];
    const b = keys[k];
    if (f <= b.f) {
      const t = interpolate(f, [a.f, b.f], [0, 1], { ...clamp, easing: IN_OUT });
      // zoom moves in log space so push-ins feel even
      const zoom = Math.exp(Math.log(a.zoom) + (Math.log(b.zoom) - Math.log(a.zoom)) * t);
      return { zoom, fx: a.fx + (b.fx - a.fx) * t, fy: a.fy + (b.fy - a.fy) * t, ax: a.ax + (b.ax - a.ax) * t, ay: a.ay + (b.ay - a.ay) * t };
    }
  }
  return keys[keys.length - 1];
};

/** Footage point → screen point. */
export const toScreen = (c: Cam, x: number, y: number) => ({ x: c.ax + (x - c.fx) * c.zoom, y: c.ay + (y - c.fy) * c.zoom });

/** Footage rect → screen rect. */
export const rectToScreen = (c: Cam, r: Rect): Rect => {
  const p = toScreen(c, r.x, r.y);
  return { x: p.x, y: p.y, w: r.w * c.zoom, h: r.h * c.zoom };
};

/** Children laid out in 1920×1080 footage space, seen through camera `c`. */
export const Shot: React.FC<{ cam: Cam; children: React.ReactNode; style?: React.CSSProperties }> = ({ cam, children, style }) => (
  <div
    style={{
      position: "absolute",
      left: 0,
      top: 0,
      width: 1920,
      height: 1080,
      transformOrigin: "0 0",
      transform: `translate(${cam.ax - cam.fx * cam.zoom}px, ${cam.ay - cam.fy * cam.zoom}px) scale(${cam.zoom})`,
      ...style,
    }}
  >
    {children}
  </div>
);
