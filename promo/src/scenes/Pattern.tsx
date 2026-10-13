import React from "react";
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from "remotion";
import { C, F, glow } from "../theme";
import { hash, type Timeline } from "../timeline";
import { useTL } from "../tl";
import { Hazard, Mono, OUT, caText, clamp, hexCells, hexPath } from "../fx";
import { lp, useLayout } from "../layout";
import { PILOTS, SECTORS, axial } from "../pilots";

/** Pilots in sweep order (left to right): the k-th lights on the k-th 8th of the sweep (pattern.hexes). */
const ORDER = PILOTS.map((p, i) => ({ i, x: axial(p.q, p.r, 1).x + p.r * 0.01 }))
  .sort((a, b) => a.x - b.x)
  .map((o) => o.i);
export const lightAt = (tl: Timeline, i: number) => tl.pattern.hexes[ORDER.indexOf(i)];

/** 0:07-0:13 Hex grid sweep: herdr panes light up as hexes, PATTERN: CLAUDE / CODEX. */
export const Pattern: React.FC = () => {
  const frame = useCurrentFrame();
  const tl = useTL();
  const S = tl.scenes.pattern[0];
  const P = tl.pattern;
  const f = frame + S;
  const { w, h, cx, portrait, tall, bleed } = useLayout();
  const size = lp(portrait, 104, 100);
  const cy = lp(portrait, 700, 640);
  // the sweep band passes each pilot hex exactly when it lights
  const xs = ORDER.map((i) => cx + axial(PILOTS[i].q, PILOTS[i].r, size).x);
  const times = P.hexes;
  const bandX = interpolate(f, [P.sweep, ...times, times[times.length - 1] + tl.d(40)], [-200, ...xs, w + 300], clamp);
  const lit = (i: number) => f >= lightAt(tl, i);
  const count = PILOTS.filter((_, i) => lit(i)).length;
  const sectors = Math.min(SECTORS, Math.round((count / PILOTS.length) * SECTORS));
  // handoff: the camera dives into the centre hex (the hub), which becomes MAGI's frame
  const dive = interpolate(f, [P.zoom, tl.scenes.pattern[1]], [1, 9], { ...clamp, easing: Easing.in(Easing.cubic) });
  const header = f >= P.header;
  const hk = tl.k(f - P.header);
  // the 9:16 stage: the grid and the sweep reach the frame's edges
  const bg = hexCells(w, h + bleed.top + bleed.bottom, lp(portrait, 52, 58), 0, -bleed.top);

  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: tall ? "visible" : "hidden" }}>
      <AbsoluteFill style={{ scale: String(dive), transformOrigin: `${cx}px ${cy}px` }}>
        {/* background grid, brightened by the sweep */}
        <svg width={w} height={h} style={{ position: "absolute", overflow: tall ? "visible" : undefined }}>
          {bg.map((c, k) => {
            const d = Math.abs(c.cx - bandX);
            const near = Math.max(0, 1 - d / 260);
            const born = interpolate(f, [S + tl.d((c.row + c.col) * 0.6), S + tl.d(14 + (c.row + c.col) * 0.6)], [0, 1], clamp);
            return (
              <path
                key={k}
                d={hexPath(c.cx, c.cy, lp(portrait, 49, 55))}
                fill={near > 0.6 && hash(k) > 0.7 ? "rgba(255,153,0,0.18)" : "none"}
                stroke={C.orange}
                strokeWidth={1.5}
                opacity={born * (0.12 + near * 0.55)}
              />
            );
          })}
          {/* the hub at the centre */}
          <path d={hexPath(cx, cy, size * 0.94)} fill="rgba(255,170,0,0.10)" stroke={C.amber} strokeWidth={4} opacity={0.9} />
        </svg>
        {/* sweep band */}
        <div
          style={{
            position: "absolute",
            left: bandX - 6,
            top: -bleed.top,
            width: 12,
            height: h + bleed.top + bleed.bottom,
            backgroundColor: C.amber,
            boxShadow: `0 0 60px 30px rgba(255,170,0,0.35)`,
            opacity: f < times[times.length - 1] + tl.d(40) ? 0.9 : 0,
          }}
        />
        <div
          style={{
            position: "absolute",
            left: cx - 120,
            top: cy - 32,
            width: 240,
            textAlign: "center",
            fontFamily: F.head,
            fontWeight: 700,
            fontSize: 44,
            letterSpacing: 4,
            color: C.amber,
            textShadow: glow(C.amber, 12),
          }}
        >
          HERDR
        </div>

        {/* pilot hexes */}
        {PILOTS.map((p, i) => {
          const { x, y } = axial(p.q, p.r, size);
          const on = lit(i);
          const k = tl.k(f - lightAt(tl, i));
          const color = p.kind === "claude" ? C.orange : C.cyan;
          const pop = on ? interpolate(k, [0, 5, 12], [1.25, 0.96, 1], clamp) : 1;
          const flick = on && k < 8 && Math.floor(k) % 2 === 0;
          return (
            <div key={p.name} style={{ position: "absolute", left: cx + x - size, top: cy + y - size, width: size * 2, height: size * 2, scale: String(pop) }}>
              <svg width={size * 2} height={size * 2} style={{ position: "absolute" }}>
                <path
                  d={hexPath(size, size, size * 0.94)}
                  fill={on ? (flick ? color : `${color}33`) : "rgba(255,255,255,0.02)"}
                  stroke={on ? color : "rgba(255,153,0,0.35)"}
                  strokeWidth={on ? 5 : 2}
                  style={{ filter: on ? `drop-shadow(0 0 12px ${color})` : undefined }}
                />
              </svg>
              {on ? (
                <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 2 }}>
                  <div style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 26, 28), color: flick ? "#000" : color, letterSpacing: 1 }}>
                    {p.kind.toUpperCase()}
                  </div>
                  <div style={{ fontFamily: F.cond, fontWeight: 600, fontSize: lp(portrait, 22, 24), color: flick ? "#000" : C.white }}>{p.name}</div>
                </div>
              ) : null}
            </div>
          );
        })}
      </AbsoluteFill>

      {/* HUD (does not dive) */}
      <AbsoluteFill style={{ opacity: interpolate(f, [P.zoom, P.zoom + tl.d(20)], [1, 0], clamp) }}>
        <Hazard style={{ left: 0, right: 0, top: -bleed.top, height: 30 }} speed={3} />
        <Hazard style={{ left: 0, right: 0, bottom: -bleed.bottom, height: 30 }} speed={-3} />
        <div style={{ position: "absolute", left: lp(portrait, 50, 70), top: lp(portrait, 54, 50), display: "flex", flexDirection: "column", gap: 0 }}>
          <span style={{ fontFamily: F.serif, fontWeight: 900, fontSize: lp(portrait, 60, 68), lineHeight: 1.1, color: C.white }}>全区画</span>
          <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 40, 44), lineHeight: 1.1, color: C.orange, letterSpacing: 4, textShadow: glow(C.orange, 10) }}>
            SECTOR SCAN
          </span>
        </div>
        <Mono size={24} color={C.orange} style={{ position: "absolute", left: lp(portrait, 52, 72), top: lp(portrait, 180, 186), opacity: 0.8 }}>
          {`HERDR LINK · ${String(Math.min(100, Math.max(0, Math.round(tl.k(f - S) * 0.9)))).padStart(3, "0")}%`}
        </Mono>

        {/* pattern labels */}
        {(["claude", "codex"] as const).map((kind, n) => {
          const show = PILOTS.some((p, i) => p.kind === kind && lit(i));
          const color = kind === "claude" ? C.orange : C.cyan;
          const k = PILOTS.filter((p, i) => p.kind === kind && lit(i)).length;
          const pos = portrait
            ? { left: n === 0 ? 50 : undefined, right: n === 1 ? 50 : undefined, top: 1060 }
            : { left: n === 0 ? 70 : undefined, right: n === 1 ? 70 : undefined, top: 520 };
          return show ? (
            <div key={kind} style={{ position: "absolute", ...pos, textAlign: n === 0 ? "left" : "right" }}>
              <div style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 50, 60), color, letterSpacing: 3, textShadow: glow(color, 14) }}>
                PATTERN: {kind.toUpperCase()}
              </div>
              <div style={{ fontFamily: F.seg, fontStyle: "italic", fontWeight: 700, fontSize: lp(portrait, 72, 92), color, textShadow: glow(color, 18) }}>
                {String(k).padStart(2, "0")}
              </div>
            </div>
          ) : null;
        })}

        {/* counter */}
        <div
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            bottom: lp(portrait, 70, 58),
            display: "flex",
            justifyContent: "center",
            alignItems: "baseline",
            gap: 18,
            color: C.white,
            fontFamily: F.head,
            fontWeight: 700,
            fontSize: lp(portrait, 54, 66),
            letterSpacing: 3,
          }}
        >
          <span style={{ fontFamily: F.seg, fontStyle: "italic", color: C.lcd, textShadow: glow(C.lcd, 14), fontSize: lp(portrait, 70, 84) }}>
            {String(count).padStart(2, "0")}
          </span>
          <span>PILOTS</span>
          <span style={{ color: C.orange }}>·</span>
          <span style={{ fontFamily: F.seg, fontStyle: "italic", color: C.lcd, textShadow: glow(C.lcd, 14), fontSize: lp(portrait, 70, 84) }}>{sectors}</span>
          <span>SECTORS</span>
        </div>

        {/* パターン検出 flash */}
        {header ? (
          <div
            style={{
              position: "absolute",
              left: 0,
              right: 0,
              top: lp(portrait, 190, 104),
              textAlign: "center",
              opacity: hk < 3 || (hk > 8 && hk < 10) ? 0.4 : 1,
            }}
          >
            <div
              style={{
                display: "inline-block",
                padding: "4px 36px 10px",
                backgroundColor: hk < 40 && Math.floor(hk / 4) % 2 === 0 ? C.red : "rgba(0,0,0,0.85)",
                border: `4px solid ${C.red}`,
                fontFamily: F.serif,
                fontWeight: 900,
                fontSize: lp(portrait, 130, 128),
                color: "#fff",
                lineHeight: 1.1,
                textShadow: caText(hk < 6 ? 10 : 3),
                scale: String(interpolate(hk, [0, 6], [1.2, 1], { ...clamp, easing: OUT })),
              }}
            >
              パターン検出
            </div>
            <div style={{ marginTop: 6, fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 44, 52), letterSpacing: 14, color: C.red, textShadow: glow(C.red, 12) }}>
              PATTERN DETECTED
            </div>
          </div>
        ) : null}
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
