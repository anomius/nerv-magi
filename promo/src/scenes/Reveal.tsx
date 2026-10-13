import React from "react";
import { AbsoluteFill, Img, Sequence, interpolate, useCurrentFrame } from "remotion";
import { Video } from "@remotion/media";
import { C, F, glow } from "../theme";
import type { Timeline } from "../timeline";
import { useFootage, useTL } from "../tl";
import { CLIP } from "../footage";
import { Mono, OUT, caText, clamp } from "../fx";
import { PANELS, lp, useLayout } from "../layout";
import { type Key, Shot, camAt, rectToScreen, toScreen } from "../camera";
import { Callout } from "../callout";
import { TRIAD, Unit } from "../magi";
import { magiOrigin, magiScale } from "./Magi";

const MOVE = 26; // canonical frames for the camera to travel to a callout

const CALLOUTS = [
  { rect: PANELS.roster, kanji: "稼働状況", title: "PILOT ROSTER", detail: ["every agent: model, effort,", "context and cost"] },
  { rect: PANELS.activity, kanji: "活動限界", title: "ACTIVITY LIMIT", detail: ["battery time as the", "4:59 internal countdown"] },
  { rect: PANELS.telemetry, kanji: "監視", title: "TELEMETRY", detail: ["btop-grade cores, memory,", "GPU, temps and network"] },
  { rect: PANELS.sdat, kanji: "音声", title: "S-DAT", detail: ["now playing, with", "a live spectrum"] },
] as const;

/** Camera keyframes in absolute frames. Landscape: region on one side, label on the other. */
const keysFor = (tl: Timeline, portrait: boolean, h: number, tall: boolean): Key[] => {
  const R = tl.reveal;
  const { d } = tl;
  const o = magiOrigin(portrait, h);
  const z0 = magiScale(portrait);
  // 9:16: the wide shot sits lower, clear of the two-line label above it
  const wide = portrait ? { zoom: 0.5625, fx: 960, fy: 540, ax: 540, ay: tall ? 780 : 660 } : { zoom: 1, fx: 960, fy: 540, ax: 960, ay: 540 };
  const at = (r: { x: number; y: number; w: number; h: number }, zoom: number, ax: number, ay: number) => ({ zoom, fx: r.x + r.w / 2, fy: r.y + r.h / 2, ax, ay });
  const shots = portrait
    ? [at(PANELS.roster, 1.5, 540, 560), at(PANELS.activity, 2.0, 540, 470), at(PANELS.telemetry, 1.45, 540, 450), at(PANELS.sdat, 1.95, 540, 480)]
    : [at(PANELS.roster, 1.72, 640, 545), at(PANELS.activity, 2.25, 1290, 520), at(PANELS.telemetry, 1.8, 650, 520), at(PANELS.sdat, 2.15, 1280, 530)];
  const keys: Key[] = [
    { f: R.push, zoom: z0, fx: TRIAD.center.x, fy: TRIAD.center.y, ax: o.x, ay: o.y },
    { f: R.wide, ...wide },
    { f: R.callouts[0] - d(4), ...wide, zoom: wide.zoom * 1.03 },
  ];
  shots.forEach((s, i) => {
    const start = R.callouts[i];
    // the last callout holds through the hazard wipe into the view flow
    const end = i < 3 ? R.callouts[i + 1] : R.end + d(24);
    keys.push({ f: start + d(MOVE), ...s });
    keys.push({ f: end - d(2), ...s, zoom: s.zoom * 1.07 });
  });
  return keys;
};

const labelFor = (portrait: boolean, i: number) => {
  if (portrait) return { side: "below" as const, label: { x: 60, y: 940, w: 960 } };
  return i % 2 === 0 ? { side: "right" as const, label: { x: 1300, y: 300, w: 580 } } : { side: "left" as const, label: { x: 50, y: 300, w: 600 } };
};

/** 0:22-0:36 Push from the MAGI triad into the real MAIN view, then a guided tour with callouts. */
export const Reveal: React.FC = () => {
  const frame = useCurrentFrame();
  const tl = useTL();
  const footage = useFootage();
  const S = tl.scenes.reveal[0];
  const R = tl.reveal;
  const at = R.callouts;
  const c1 = at[0];
  const { d } = tl;
  const f = frame + S;
  const { fps, h, portrait, tall } = useLayout();
  const keys = keysFor(tl, portrait, h, tall);
  const cam = camAt(keys, f);
  // sources: the 4K still for every zoomed moment, live footage when the camera is wide
  const live = interpolate(f, [R.wide - d(10), R.wide + d(4)], [0, 1], clamp) * interpolate(f, [c1 - d(6), c1 + d(6)], [1, 0], clamp);
  // our triad hands over to the product's own MAGI panel during the first frames of the push
  const overlay = interpolate(f, [R.push + d(4), R.push + d(26)], [1, 0], clamp);
  const wideLabel = f >= R.wide + d(4) && f < c1 + d(4);
  const wk = tl.k(f - R.wide) - 4;
  const active = at.findIndex((a, i) => f >= a && (i === 3 || f < at[i + 1]));
  const next = active >= 0 && active < 3 ? at[active + 1] : Infinity;
  const dim = active >= 0 ? interpolate(tl.k(f - at[active]), [6, 22], [0, 1], clamp) * (next === Infinity ? 1 : interpolate(f, [next - d(8), next], [1, 0], clamp)) : 0;
  const liveFrom = Math.round(R.wide - d(14));

  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: tall ? "visible" : "hidden" }}>
      <Shot cam={cam}>
        <Img src={footage("main-4k.png")} style={{ position: "absolute", width: 1920, height: 1080 }} />
        <Sequence name="MAIN live (wide)" from={liveFrom - S} durationInFrames={Math.round(c1 + d(16)) - liveFrom} premountFor={fps}>
          <Video src={footage("main.mp4")} trimBefore={CLIP.mainWideIn} muted style={{ position: "absolute", width: 1920, height: 1080, opacity: live }} />
        </Sequence>
      </Shot>

      {/* the triad from MAGI, drawn in screen space at the camera's scale */}
      {overlay > 0 ? (
        <AbsoluteFill style={{ opacity: overlay }}>
          {TRIAD.units.map((u) => {
            const p = toScreen(cam, u.x, u.y);
            return <Unit key={u.id} {...u} x={p.x} y={p.y} s={cam.zoom} state="approved" since={99} frame={f} draw={1} />;
          })}
        </AbsoluteFill>
      ) : null}

      {/* spotlight on the active callout */}
      {active >= 0 ? (
        <>
          {(() => {
            const r = rectToScreen(cam, CALLOUTS[active].rect);
            return (
              <div
                style={{
                  position: "absolute",
                  left: r.x - 12,
                  top: r.y - 12,
                  width: r.w + 24,
                  height: r.h + 24,
                  boxShadow: `0 0 0 3000px rgba(0,0,0,${0.78 * dim})`,
                }}
              />
            );
          })()}
          {CALLOUTS.map((c, i) =>
            i === active ? (
              <Callout
                key={c.title}
                rect={rectToScreen(cam, c.rect)}
                k={tl.k(f - at[i]) - MOVE + 10}
                index={`${String(i + 1).padStart(2, "0")} / 04`}
                accent={tl.beat3(f)}
                kanji={c.kanji}
                title={c.title}
                detail={[...c.detail]}
                {...labelFor(portrait, i)}
              />
            ) : null,
          )}
        </>
      ) : null}

      {/* the wide shot's line */}
      {wideLabel ? (
        <div
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            top: lp<number | undefined>(portrait, 80, undefined),
            bottom: lp<number | undefined>(portrait, undefined, 0),
            padding: lp(portrait, "20px 40px", "40px 0 34px"),
            background: portrait ? "transparent" : "linear-gradient(to top, rgba(0,0,0,0.92) 65%, rgba(0,0,0,0))",
            textAlign: "center",
            opacity: interpolate(wk, [0, 6], [0, 1], clamp),
          }}
        >
          <div
            style={{
              fontFamily: F.head,
              fontWeight: 700,
              fontSize: lp(portrait, 110, 124),
              lineHeight: 1.02,
              letterSpacing: 3,
              color: "#fff",
              textShadow: caText(wk < 6 ? 8 : 0, glow("rgba(255,153,0,0.5)", 20)),
              translate: `0px ${interpolate(wk, [0, 10], [24, 0], { ...clamp, easing: OUT })}px`,
            }}
          >
            {portrait ? (
              <>
                EVERY AGENT.
                <br />
                ONE SCREEN.
              </>
            ) : (
              "EVERY AGENT. ONE SCREEN."
            )}
          </div>
          <Mono size={lp(portrait, 34, 40)} color={C.orange} style={{ marginTop: 10, whiteSpace: "normal" }}>
            REAL FOOTAGE · nerv tv --demo · GPU-NATIVE, NO WEB VIEW
          </Mono>
        </div>
      ) : null}

      {/* a REC tag: this is the product, not a mock-up */}
      {f >= R.wide ? (
        <div style={{ position: "absolute", right: lp(portrait, 40, 46), top: lp(portrait, 1280, 36), display: "flex", alignItems: "center", gap: 12, opacity: dim > 0.5 ? 0.9 : 0.7 }}>
          <div style={{ width: 20, height: 20, borderRadius: 10, backgroundColor: C.red, opacity: Math.floor(f / 20) % 2 ? 1 : 0.25 }} />
          <Mono size={26} color={C.white}>
            nerv tv · MAIN 作戦行動
          </Mono>
        </div>
      ) : null}
    </AbsoluteFill>
  );
};
