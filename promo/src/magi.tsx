import React from "react";
import { interpolate } from "remotion";
import { C, F, glow } from "./theme";
import { clamp } from "./fx";

/** The MAGI triad as the MAIN view draws it, in 1920×1080 footage pixels. */
export const TRIAD = {
  center: { x: 1052, y: 290 },
  magi: { x: 1052, y: 283 },
  units: [
    { id: "BALTHASAR·2", role: "MEMORY", data: "FREE 3.9G", x: 1052, y: 178 },
    { id: "CASPER·3", role: "PILOTS", data: "3 ACTIVE", x: 975, y: 385 },
    { id: "MELCHIOR·1", role: "COMPUTE", data: "CPU 56% · 58°C", x: 1130, y: 385 },
  ],
  unit: { w: 142, h: 86 },
} as const;

export type Vote = "idle" | "deliberating" | "approved";

/** Vote order: MELCHIOR, BALTHASAR, CASPER (indices into TRIAD.units). */
export const VOTE_ORDER = [2, 0, 1] as const;

const chamfer = (w: number, h: number, c: number) =>
  `polygon(${c}px 0, ${w - c}px 0, ${w}px ${c}px, ${w}px ${h - c}px, ${w - c}px ${h}px, ${c}px ${h}px, 0 ${h - c}px, 0 ${c}px)`;

/** One MAGI unit at scale `s` (1 = footage size), centred on (x, y). */
export const Unit: React.FC<{
  id: string;
  role: string;
  data: string;
  x: number;
  y: number;
  s: number;
  state: Vote;
  since: number; // frames since the state began
  frame: number;
  draw: number; // 0..1 draw-on
}> = ({ id, role, data, x, y, s, state, since, frame, draw }) => {
  const w = TRIAD.unit.w * s;
  const h = TRIAD.unit.h * s;
  const approved = state === "approved";
  const color = approved ? C.green : state === "deliberating" ? C.orange : C.amber;
  const snap = approved && since < 3;
  const blinkOn = state === "deliberating" ? Math.floor((frame + id.length * 5) / 10) % 2 === 0 : true;
  const stamp = approved ? interpolate(since, [0, 5], [1.45, 1], clamp) : 1;
  return (
    <div style={{ position: "absolute", left: x - w / 2, top: y - h / 2, width: w, height: h, opacity: draw }}>
      <div
        style={{
          position: "absolute",
          inset: 0,
          clipPath: chamfer(w, h, 9 * s),
          backgroundColor: color,
          opacity: snap ? 1 : 1,
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 1.6 * s,
          clipPath: chamfer(w - 3.2 * s, h - 3.2 * s, 8.4 * s),
          backgroundColor: snap ? "#d8ffd8" : approved ? "#032a06" : state === "deliberating" ? "#2a1600" : "#120c00",
        }}
      />
      <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column", alignItems: "center", paddingTop: 5 * s }}>
        <div style={{ fontFamily: F.head, fontWeight: 700, fontSize: 13.5 * s, letterSpacing: 0.8 * s, color: approved ? C.green : C.amber, lineHeight: 1.05 }}>{id}</div>
        <div style={{ fontFamily: F.mono, fontSize: 6.2 * s, color: approved ? "#7dff7d" : C.orange, opacity: 0.85, lineHeight: 1.2 }}>{role}</div>
        <div
          style={{
            fontFamily: F.serif,
            fontWeight: 900,
            fontSize: 30 * s,
            lineHeight: 1.1,
            color: snap ? "#003300" : color,
            opacity: blinkOn ? 1 : 0.18,
            scale: String(stamp),
            textShadow: snap ? undefined : glow(color, 6 * s),
            letterSpacing: state === "deliberating" ? -1 * s : 0,
          }}
        >
          {approved ? "承認" : "審議中"}
        </div>
        <div style={{ fontFamily: F.mono, fontSize: 7 * s, color: approved ? "#9dff9d" : C.amber, opacity: 0.9 }}>{approved ? data : "· · · · ·"}</div>
      </div>
    </div>
  );
};

/** Connector lines from each unit to the MAGI core, drawn on by `p`. */
export const Links: React.FC<{ s: number; ox: number; oy: number; p: number; color: string; width: number; height: number }> = ({
  s,
  ox,
  oy,
  p,
  color,
  width,
  height,
}) => {
  const core = { x: ox + (TRIAD.magi.x - TRIAD.center.x) * s, y: oy + (TRIAD.magi.y - TRIAD.center.y) * s };
  return (
    <svg width={width} height={height} style={{ position: "absolute", left: 0, top: 0 }}>
      {TRIAD.units.map((u, i) => {
        const ux = ox + (u.x - TRIAD.center.x) * s;
        const uy = oy + (u.y - TRIAD.center.y) * s;
        const ex = core.x + (ux - core.x) * 0.42;
        const ey = core.y + (uy - core.y) * 0.42;
        const sx = ux + (core.x - ux) * 0.38;
        const sy = uy + (core.y - uy) * 0.38;
        return (
          <line
            key={i}
            x1={sx}
            y1={sy}
            x2={sx + (ex - sx) * p}
            y2={sy + (ey - sy) * p}
            stroke={color}
            strokeWidth={1.6 * s}
            style={{ filter: `drop-shadow(0 0 ${3 * s}px ${color})` }}
          />
        );
      })}
    </svg>
  );
};
