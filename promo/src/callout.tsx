import React from "react";
import { interpolate } from "remotion";
import { C, F, glow } from "./theme";
import { Brackets, Mono, OUT, caText, clamp } from "./fx";
import type { Rect } from "./layout";

/** An Eva-style callout: brackets on a screen rect, a leader line, a kanji tag and a label. */
export const Callout: React.FC<{
  rect: Rect;
  k: number; // frames since the callout began
  index: string;
  kanji: string;
  title: string;
  detail: string[];
  side: "left" | "right" | "below";
  label: { x: number; y: number; w: number };
  color?: string;
  /** 0..1 accent pulse (beat 3 of each bar): the brackets kick out and flare. */
  accent?: number;
}> = ({ rect, k, index, kanji, title, detail, side, label, color = C.orange, accent = 0 }) => {
  const p = interpolate(k, [10, 24], [0, 1], { ...clamp, easing: OUT });
  const lineP = interpolate(k, [16, 28], [0, 1], { ...clamp, easing: OUT });
  const show = interpolate(k, [22, 30], [0, 1], clamp);
  const slide = interpolate(k, [22, 34], [side === "left" ? -40 : side === "right" ? 40 : 0, 0], { ...clamp, easing: OUT });
  // leader: from the bracket edge to the label
  const from =
    side === "left" ? { x: rect.x, y: rect.y + rect.h / 2 } : side === "right" ? { x: rect.x + rect.w, y: rect.y + rect.h / 2 } : { x: rect.x + rect.w / 2, y: rect.y + rect.h };
  const to = side === "left" ? { x: label.x + label.w + 16, y: label.y + 60 } : side === "right" ? { x: label.x - 16, y: label.y + 60 } : { x: label.x + 40, y: label.y - 14 };
  const kFlash = k >= 22 && k < 26;
  return (
    <>
      <Brackets
        x={rect.x - 10 - accent * 8}
        y={rect.y - 10 - accent * 8}
        w={rect.w + 20 + accent * 16}
        h={rect.h + 20 + accent * 16}
        p={p}
        color={accent > 0.5 ? "#fff" : color}
        len={Math.min(90, rect.w / 4) * (1 + accent * 0.3)}
        weight={6}
      />
      <svg width={1920} height={1920} style={{ position: "absolute", left: 0, top: 0, overflow: "visible" }}>
        <polyline
          points={`${from.x},${from.y} ${from.x + (to.x - from.x) * lineP},${from.y + (to.y - from.y) * lineP}`}
          stroke={color}
          strokeWidth={4}
          fill="none"
          style={{ filter: `drop-shadow(0 0 6px ${color})` }}
        />
        {lineP >= 1 ? <circle cx={to.x} cy={to.y} r={8} fill={color} /> : null}
      </svg>
      <div style={{ position: "absolute", left: label.x, top: label.y, width: label.w, opacity: show, translate: `${slide}px 0px`, textAlign: side === "left" ? "right" : "left" }}>
        <Mono size={26} color={color} style={{ opacity: 0.9 }}>
          {index}
        </Mono>
        <div
          style={{
            display: "inline-block",
            marginTop: 6,
            padding: "0 16px 6px",
            backgroundColor: color,
            fontFamily: F.serif,
            fontWeight: 900,
            fontSize: 64,
            lineHeight: 1.15,
            color: "#000",
            textShadow: kFlash ? caText(6) : undefined,
          }}
        >
          {kanji}
        </div>
        <div style={{ fontFamily: F.head, fontWeight: 700, fontSize: 84, lineHeight: 1.05, letterSpacing: 2, color: "#fff", marginTop: 8, textShadow: glow("rgba(255,153,0,0.5)", 18) }}>
          {title}
        </div>
        {detail.map((d, i) => (
          <div key={i} style={{ fontFamily: F.cond, fontWeight: 600, fontSize: 46, lineHeight: 1.18, color: C.amber, marginTop: i ? 0 : 8 }}>
            {d}
          </div>
        ))}
      </div>
    </>
  );
};
