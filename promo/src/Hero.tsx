import React from "react";
import { AbsoluteFill, Img, interpolate, staticFile, useCurrentFrame, useVideoConfig } from "remotion";
import "./theme";
import { C, F, glow } from "./theme";
import { Brackets, Grain, IN_OUT, Mono, Scanlines, Vignette, clamp } from "./fx";
import { PANELS } from "./layout";
import { Shot, rectToScreen, type Cam } from "./camera";

const STOPS = [
  { rect: PANELS.roster, kanji: "稼働状況", label: "PILOT ROSTER" },
  { rect: PANELS.activity, kanji: "活動限界", label: "ACTIVITY LIMIT" },
  { rect: PANELS.telemetry, kanji: "監視", label: "TELEMETRY" },
  { rect: PANELS.sdat, kanji: "音声", label: "S-DAT" },
] as const;

/** README hero: the real MAIN view, a slow periodic drift and brackets hopping between panels.
 * Every motion has the composition's length as its period, so the last frame meets the first. */
export const Hero: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames: D } = useVideoConfig();
  const ph = (frame / D) * Math.PI * 2;
  const cam: Cam = { zoom: 1.1 + 0.035 * Math.sin(ph), fx: 960 + 60 * Math.sin(ph), fy: 540 + 24 * Math.sin(2 * ph), ax: 960, ay: 540 };
  const seg = D / STOPS.length;
  const i = Math.floor(frame / seg);
  const k = frame - i * seg;
  const a = STOPS[i % STOPS.length];
  const b = STOPS[(i + 1) % STOPS.length];
  const hop = interpolate(k, [seg - 18, seg], [0, 1], { ...clamp, easing: IN_OUT });
  const lerp = (x: number, y: number) => x + (y - x) * hop;
  const ra = rectToScreen(cam, a.rect);
  const rb = rectToScreen(cam, b.rect);
  const r = { x: lerp(ra.x, rb.x), y: lerp(ra.y, rb.y), w: lerp(ra.w, rb.w), h: lerp(ra.h, rb.h) };
  const tag = hop < 0.5 ? a : b;
  const tagO = Math.min(1, Math.abs(hop - 0.5) * 4);
  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: "hidden" }}>
      <Shot cam={cam}>
        <Img src={staticFile("footage/main-still.png")} style={{ position: "absolute", width: 1920, height: 1080 }} />
      </Shot>
      <div style={{ position: "absolute", left: r.x - 12, top: r.y - 12, width: r.w + 24, height: r.h + 24, boxShadow: "0 0 0 3000px rgba(0,0,0,0.45)" }} />
      <Brackets x={r.x - 12} y={r.y - 12} w={r.w + 24} h={r.h + 24} p={1} len={70} weight={6} />
      <div style={{ position: "absolute", left: r.x - 12, top: r.y - 74 < 20 ? r.y + r.h + 20 : r.y - 74, display: "flex", gap: 14, alignItems: "baseline", opacity: tagO }}>
        <span style={{ padding: "0 12px 4px", backgroundColor: C.orange, fontFamily: F.serif, fontWeight: 900, fontSize: 44, color: "#000" }}>{tag.kanji}</span>
        <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: 44, letterSpacing: 2, color: "#fff", textShadow: glow("#000", 8) }}>{tag.label}</span>
      </div>
      <div style={{ position: "absolute", left: 0, right: 0, bottom: 0, padding: "70px 60px 36px", background: "linear-gradient(to top, rgba(0,0,0,0.95) 55%, rgba(0,0,0,0))" }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 30 }}>
          <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: 120, letterSpacing: 8, color: "#fff", lineHeight: 1, textShadow: glow("rgba(255,153,0,0.5)", 24) }}>NERV MAGI</span>
          <span style={{ fontFamily: F.cond, fontWeight: 600, fontSize: 50, color: C.amber }}>agents · telemetry · now playing, on your TV</span>
        </div>
      </div>
      <div style={{ position: "absolute", right: 46, top: 36, display: "flex", alignItems: "center", gap: 12 }}>
        <div style={{ width: 20, height: 20, borderRadius: 10, backgroundColor: C.red, opacity: Math.floor(frame / 30) % 2 ? 1 : 0.25 }} />
        <Mono size={26} color={C.white}>
          nerv tv · MAIN 作戦行動
        </Mono>
      </div>
      <Scanlines period={D} />
      <Grain />
      <Vignette />
    </AbsoluteFill>
  );
};
