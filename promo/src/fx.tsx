import React from "react";
import { AbsoluteFill, interpolate, useCurrentFrame, useVideoConfig, Easing } from "remotion";
import { C, F, glow } from "./theme";
import { hash } from "./timeline";

const clamp = { extrapolateLeft: "clamp", extrapolateRight: "clamp" } as const;
export { clamp };

/** Ease-out used for most draw-ons. */
export const OUT = Easing.bezier(0.16, 1, 0.3, 1);
export const IN_OUT = Easing.bezier(0.65, 0, 0.35, 1);

// ---------------------------------------------------------------------------------------------
// Screen texture

/** CRT scanlines plus a slow rolling bright band. */
export const Scanlines: React.FC<{ opacity?: number; roll?: boolean; period?: number }> = ({ opacity = 0.22, roll = true, period }) => {
  const frame = useCurrentFrame();
  const { height } = useVideoConfig();
  // `period` (frames) makes the roll loop exactly (hero loop)
  const speed = period ? (height + 400) / period : 3.2;
  const band = ((frame * speed) % (height + 400)) - 200;
  return (
    <AbsoluteFill style={{ pointerEvents: "none" }}>
      <AbsoluteFill
        style={{
          opacity,
          backgroundImage: "repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0px, rgba(0,0,0,0) 2px, rgba(0,0,0,0.9) 2px, rgba(0,0,0,0.9) 4px)",
        }}
      />
      {roll ? (
        <div
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            top: band,
            height: 200,
            background: "linear-gradient(to bottom, rgba(255,255,255,0), rgba(255,255,255,0.035), rgba(255,255,255,0))",
          }}
        />
      ) : null}
    </AbsoluteFill>
  );
};

/** Film grain: SVG turbulence, reseeded every other frame. */
export const Grain: React.FC<{ opacity?: number }> = ({ opacity = 0.5 }) => {
  const frame = useCurrentFrame();
  const seed = Math.floor(frame / 2) % 64;
  const id = `grain-${seed}`;
  return (
    // soft-light: grain lives in the lit areas, blacks stay black
    <AbsoluteFill style={{ pointerEvents: "none", opacity, mixBlendMode: "soft-light" }}>
      <svg width="100%" height="100%">
        <filter id={id} x="0" y="0" width="100%" height="100%">
          <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed={seed} stitchTiles="stitch" />
          <feColorMatrix type="saturate" values="0" />
        </filter>
        <rect width="100%" height="100%" filter={`url(#${id})`} />
      </svg>
    </AbsoluteFill>
  );
};

export const Vignette: React.FC<{ strength?: number }> = ({ strength = 0.5 }) => (
  <AbsoluteFill
    style={{
      pointerEvents: "none",
      background: `radial-gradient(ellipse at center, rgba(0,0,0,0) 50%, rgba(0,0,0,${strength}) 100%)`,
    }}
  />
);

/** Full-frame flash for `dur` frames from `at`; a smash cut has no `tail`, a colour hit fades. */
export const Flash: React.FC<{ at: number; dur?: number; color?: string; peak?: number; tail?: number }> = ({ at, dur = 2, color = "#fff", peak = 1, tail = 0 }) => {
  const frame = useCurrentFrame();
  if (frame < at || frame >= at + dur + tail) return null;
  const o = frame < at + dur ? peak : interpolate(frame, [at + dur, at + dur + tail], [peak * 0.35, 0], clamp);
  return <AbsoluteFill style={{ backgroundColor: color, opacity: o, pointerEvents: "none" }} />;
};

/** SVG filter splitting R and B sideways by `px` (chromatic aberration); apply with `filter: url(#id)`. */
export const CaFilter: React.FC<{ id: string; px: number }> = ({ id, px }) => (
  <svg width="0" height="0" style={{ position: "absolute" }}>
    <filter id={id} x="-5%" y="0" width="110%" height="100%" colorInterpolationFilters="sRGB">
      <feColorMatrix in="SourceGraphic" type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="r" />
      <feOffset in="r" dx={-px} dy="0" result="r2" />
      <feColorMatrix in="SourceGraphic" type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="g" />
      <feColorMatrix in="SourceGraphic" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="b" />
      <feOffset in="b" dx={px} dy="0" result="b2" />
      <feBlend in="r2" in2="g" mode="screen" result="rg" />
      <feBlend in="rg" in2="b2" mode="screen" />
    </filter>
  </svg>
);

/** Text-shadow chromatic split for type flashes. */
export const caText = (px: number, extra = "") =>
  `${-px}px 0 rgba(255,0,40,0.85), ${px}px 0 rgba(0,255,255,0.85)${extra ? `, ${extra}` : ""}`;

// ---------------------------------------------------------------------------------------------
// Eva graphic language

/** Diagonal warning tape, scrolling sideways. */
export const Hazard: React.FC<{
  style?: React.CSSProperties;
  color?: string;
  stripe?: number;
  speed?: number;
  angle?: number;
}> = ({ style, color = C.amber, stripe = 26, speed = 2, angle = -45 }) => {
  const frame = useCurrentFrame();
  return (
    <div
      style={{
        position: "absolute",
        backgroundImage: `repeating-linear-gradient(${angle}deg, ${color} 0px, ${color} ${stripe}px, #000 ${stripe}px, #000 ${stripe * 2}px)`,
        backgroundSize: `${stripe * 2 * 1.4142}px ${stripe * 2 * 1.4142}px`,
        backgroundPosition: `${frame * speed}px 0px`,
        ...style,
      }}
    />
  );
};

/** Pointy-top hexagon path centred on (cx, cy). */
export const hexPath = (cx: number, cy: number, r: number) => {
  const p = [];
  for (let k = 0; k < 6; k++) {
    const a = ((-90 + 60 * k) * Math.PI) / 180;
    p.push(`${(cx + r * Math.cos(a)).toFixed(2)},${(cy + r * Math.sin(a)).toFixed(2)}`);
  }
  return `M${p.join("L")}Z`;
};

/** Centres of a pointy-top hex grid covering w×h with radius r. */
export const hexCells = (w: number, h: number, r: number, ox = 0, oy = 0) => {
  const dx = Math.sqrt(3) * r;
  const dy = 1.5 * r;
  const cells: { cx: number; cy: number; col: number; row: number }[] = [];
  const rows = Math.ceil(h / dy) + 2;
  const cols = Math.ceil(w / dx) + 2;
  for (let row = -1; row < rows; row++) {
    for (let col = -1; col < cols; col++) {
      cells.push({ cx: ox + col * dx + (row % 2 ? dx / 2 : 0), cy: oy + row * dy, col, row });
    }
  }
  return cells;
};

/** Corner brackets around a rect, drawn on by `p` (0..1). */
export const Brackets: React.FC<{
  x: number;
  y: number;
  w: number;
  h: number;
  p: number;
  color?: string;
  len?: number;
  weight?: number;
}> = ({ x, y, w, h, p, color = C.orange, len = 46, weight = 5 }) => {
  const l = len * p;
  const s: React.CSSProperties = { position: "absolute", backgroundColor: color, boxShadow: glow(color, 10) };
  return (
    <>
      <div style={{ ...s, left: x, top: y, width: l, height: weight }} />
      <div style={{ ...s, left: x, top: y, width: weight, height: l }} />
      <div style={{ ...s, left: x + w - l, top: y, width: l, height: weight }} />
      <div style={{ ...s, left: x + w - weight, top: y, width: weight, height: l }} />
      <div style={{ ...s, left: x, top: y + h - weight, width: l, height: weight }} />
      <div style={{ ...s, left: x, top: y + h - l, width: weight, height: l }} />
      <div style={{ ...s, left: x + w - l, top: y + h - weight, width: l, height: weight }} />
      <div style={{ ...s, left: x + w - weight, top: y + h - l, width: weight, height: l }} />
    </>
  );
};

/** Text typed `fpc` frames per character from `at`, with a block cursor while typing. */
export const Typed: React.FC<{
  text: string;
  at: number;
  fpc?: number;
  style?: React.CSSProperties;
  cursor?: boolean;
}> = ({ text, at, fpc = 2, style, cursor = true }) => {
  const frame = useCurrentFrame();
  const n = Math.max(0, Math.min(text.length, Math.floor((frame - at) / fpc) + 1));
  if (frame < at) return null;
  const typing = n < text.length;
  const showCursor = cursor && (typing || Math.floor(frame / 8) % 2 === 0);
  return (
    <span style={style}>
      {text.slice(0, n)}
      {showCursor ? <span style={{ opacity: 0.9 }}>▮</span> : null}
    </span>
  );
};

/** Glitchy horizontal jitter for a few frames after `at`. */
export const jitter = (frame: number, at: number, frames = 6, px = 18) => {
  const d = frame - at;
  if (d < 0 || d >= frames) return 0;
  return (hash(frame * 7.13) - 0.5) * 2 * px * (1 - d / frames);
};

/** A small mono data label (dense decorative HUD text). */
export const Mono: React.FC<{ children: React.ReactNode; style?: React.CSSProperties; color?: string; size?: number }> = ({
  children,
  style,
  color = C.amber,
  size = 22,
}) => (
  <div style={{ fontFamily: F.mono, fontWeight: 500, fontSize: size, color, letterSpacing: 1, whiteSpace: "pre", ...style }}>{children}</div>
);

/** Shake offset on a beat grid: a decaying kick at every `beat` frames between `from` and `to`. */
export const beatShake = (frame: number, from: number, to: number, beat: number, px: number) => {
  if (frame < from || frame >= to) return { x: 0, y: 0 };
  const d = (frame - from) % beat;
  const k = Math.max(0, 1 - d / 9);
  const n = Math.floor((frame - from) / beat);
  return { x: (hash(n * 3.1 + d) - 0.5) * 2 * px * k, y: (hash(n * 5.7 + d * 1.3) - 0.5) * 2 * px * k };
};
