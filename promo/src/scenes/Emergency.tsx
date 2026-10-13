import React from "react";
import { AbsoluteFill, Sequence, interpolate, staticFile, useCurrentFrame } from "remotion";
import { Video } from "@remotion/media";
import { C, F, glow } from "../theme";
import { T, hash } from "../timeline";
import { CaFilter, Hazard, Mono, OUT, beatShake, caText, clamp } from "../fx";
import { lp, useLayout } from "../layout";
import { type Key, Shot, camAt, rectToScreen } from "../camera";
import { Callout } from "../callout";

const S = T.scenes.emergency[0];
const E = T.emergency;
const CARD = { x: 60, y: 335, w: 900, h: 430 };
const TAKEOVER_IN_CLIP = 84; // frame of emergency.mp4 where the red takeover starts
const WIPE_IN_CLIP = 183; // frame of resolve.mp4 where MAIN wipes back in

/** 0:42-0:48 EMERGENCY: the real takeover, flashing type, hazard tape, shake on the beat, then NOMINAL. */
export const Emergency: React.FC = () => {
  const frame = useCurrentFrame();
  const f = frame + S;
  const { w, h, fps, portrait } = useLayout();
  const wide = portrait ? { zoom: 0.74, fx: 960, fy: 540, ax: 540, ay: 600 } : { zoom: 1, fx: 960, fy: 540, ax: 960, ay: 540 };
  const card = portrait
    ? { zoom: 1.12, fx: CARD.x + CARD.w / 2, fy: CARD.y + CARD.h / 2, ax: 540, ay: 470 }
    : { zoom: 1.4, fx: CARD.x + CARD.w / 2, fy: CARD.y + CARD.h / 2, ax: 700, ay: 560 };
  const keys: Key[] = [
    { f: S, ...wide },
    { f: E.card, ...wide, zoom: wide.zoom * 1.02 },
    { f: E.card + 24, ...card },
    { f: E.clear - 2, ...card, zoom: card.zoom * 1.06 },
    { f: E.clear, ...wide },
  ];
  const cam = camAt(keys, f);
  const alarm = f < E.clear;
  const shake = alarm ? beatShake(f, S, E.clear, T.beat, lp(portrait, 26, 20)) : { x: 0, y: 0 };
  const beatK = (f - S) % T.beat;
  const pulse = alarm ? Math.max(0, 1 - beatK / 12) : 0;
  const typeOn = f >= E.takeover + 4 && f < E.card + 10;
  const tk = f - E.takeover - 4;
  const word = Math.floor(tk / 12) % 2 === 0;
  const resolved = f >= E.resolve;
  const rk = f - E.resolve;
  const line = interpolate(f, [E.line, E.line + 12], [0, 1], { ...clamp, easing: OUT });
  const lineOpen = interpolate(f, [E.line + 12, T.scenes.emergency[1]], [0, 1], { ...clamp, easing: OUT });

  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: "hidden" }}>
      <CaFilter id="ca-em" px={8} />
      <AbsoluteFill style={{ translate: `${shake.x}px ${shake.y}px`, filter: alarm && beatK < 3 ? "url(#ca-em)" : undefined }}>
        <Sequence name="EMERGENCY footage" durationInFrames={E.clear - S} premountFor={fps}>
          <Shot cam={cam}>
            <Video src={staticFile("footage/emergency.mp4")} trimBefore={TAKEOVER_IN_CLIP - (E.takeover - S)} muted style={{ position: "absolute", width: 1920, height: 1080 }} />
          </Shot>
        </Sequence>
        <Sequence name="Resolve footage" from={E.clear - S} premountFor={fps}>
          <Shot cam={cam}>
            <Video src={staticFile("footage/resolve.mp4")} trimBefore={WIPE_IN_CLIP - (E.resolve - E.clear)} muted style={{ position: "absolute", width: 1920, height: 1080 }} />
          </Shot>
        </Sequence>
        {/* red pulse on every beat */}
        <AbsoluteFill style={{ backgroundColor: C.red, mixBlendMode: "screen", opacity: pulse * 0.32 }} />
      </AbsoluteFill>

      {/* hazard tape */}
      {alarm ? (
        <>
          <Hazard style={{ left: 0, right: 0, top: 0, height: lp(portrait, 54, 46) }} color={C.red} stripe={34} speed={7} />
          <Hazard style={{ left: 0, right: 0, bottom: 0, height: lp(portrait, 54, 46) }} color={C.red} stripe={34} speed={-7} />
        </>
      ) : null}

      {/* EMERGENCY / 緊急事態 */}
      {typeOn ? (
        <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
          <div
            style={{
              width: "100%",
              textAlign: "center",
              padding: "10px 0 20px",
              backgroundColor: tk % 12 < 2 ? C.red : "rgba(0,0,0,0.82)",
              borderTop: `8px solid ${C.red}`,
              borderBottom: `8px solid ${C.red}`,
              scale: String(interpolate(tk % 12, [0, 4], [1.08, 1], clamp)),
            }}
          >
            <span
              style={{
                fontFamily: word ? F.head : F.serif,
                fontWeight: word ? 700 : 900,
                fontSize: word ? lp(portrait, 176, 260) : lp(portrait, 196, 250),
                letterSpacing: word ? lp(portrait, 6, 12) : 0,
                lineHeight: 1.05,
                color: tk % 12 < 2 ? "#000" : "#fff",
                textShadow: tk % 12 < 2 ? undefined : caText(12, glow(C.red, 30)),
              }}
            >
              {word ? "EMERGENCY" : "緊急事態"}
            </span>
          </div>
        </AbsoluteFill>
      ) : null}

      {/* the blocked pilot card */}
      {f >= E.card && f < E.clear ? (
        <>
          <div
            style={{
              position: "absolute",
              ...(() => {
                const r = rectToScreen(cam, CARD);
                return { left: r.x - 10, top: r.y - 10, width: r.w + 20, height: r.h + 20 };
              })(),
              boxShadow: `0 0 0 3000px rgba(0,0,0,${0.62 * interpolate(f, [E.card + 10, E.card + 26], [0, 1], clamp)})`,
            }}
          />
          <Callout
            rect={rectToScreen(cam, CARD)}
            k={f - E.card}
            index="BLOCKED · 03s"
            kanji="入力要求"
            title="BLOCKED PILOT"
            detail={portrait ? ["it takes the screen", "until you answer"] : ["it takes the screen", "until you answer"]}
            side={portrait ? "below" : "right"}
            label={portrait ? { x: 60, y: 840, w: 960 } : { x: 1390, y: 250, w: 500 }}
            color={C.red}
          />
        </>
      ) : null}

      {/* NOMINAL */}
      {resolved ? (
        <AbsoluteFill style={{ justifyContent: "center", alignItems: "center" }}>
          <div
            style={{
              display: "flex",
              flexDirection: portrait ? "column" : "row",
              alignItems: "center",
              gap: lp(portrait, 0, 60),
              padding: lp(portrait, "16px 60px 26px", "10px 80px 22px"),
              backgroundColor: rk < 3 ? C.green : "rgba(0,0,0,0.86)",
              border: `8px solid ${C.green}`,
              scale: String(interpolate(rk, [0, 6], [1.15, 1], { ...clamp, easing: OUT })),
              boxShadow: "0 0 90px rgba(0,255,0,0.35)",
              opacity: 1 - lineOpen,
            }}
          >
            <span style={{ fontFamily: F.serif, fontWeight: 900, fontSize: lp(portrait, 200, 210), lineHeight: 1.05, color: rk < 3 ? "#002200" : C.green, textShadow: rk < 3 ? undefined : glow(C.green, 24) }}>
              正常
            </span>
            <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 150, 170), letterSpacing: 10, color: rk < 3 ? "#002200" : "#fff" }}>NOMINAL</span>
          </div>
          <Mono size={lp(portrait, 36, 42)} color={C.green} style={{ position: "absolute", bottom: lp(portrait, 200, 150), opacity: rk > 10 ? 1 : 0, padding: "6px 22px", backgroundColor: "rgba(0,0,0,0.82)" }}>
            MAGI 3/3 · ALL PILOTS ACCOUNTED FOR
          </Mono>
        </AbsoluteFill>
      ) : null}
      {f >= E.clear && f < E.resolve ? (
        <Mono size={lp(portrait, 40, 48)} color={C.amber} style={{ position: "absolute", left: 0, right: 0, textAlign: "center", top: lp(portrait, 980, 760), textShadow: glow(C.amber, 10) }}>
          {hash(Math.floor(f / 6)) > 0.25 ? "PILOT ANSWERED · CAUSE CLEARED" : ""}
        </Mono>
      ) : null}

      {/* hand-off: a green line opens into the outro */}
      {f >= E.line ? (
        <>
          <AbsoluteFill style={{ backgroundColor: "#000", clipPath: `inset(${50 - lineOpen * 50}% 0 ${50 - lineOpen * 50}% 0)` }} />
          <div
            style={{
              position: "absolute",
              left: (w * (1 - line)) / 2,
              width: w * line,
              top: h / 2 - 3 - (lineOpen * h) / 2,
              height: 6,
              backgroundColor: C.green,
              boxShadow: glow(C.green, 20),
            }}
          />
          <div
            style={{
              position: "absolute",
              left: (w * (1 - line)) / 2,
              width: w * line,
              top: h / 2 - 3 + (lineOpen * h) / 2,
              height: 6,
              backgroundColor: C.green,
              boxShadow: glow(C.green, 20),
            }}
          />
        </>
      ) : null}
    </AbsoluteFill>
  );
};
