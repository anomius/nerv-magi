import React from "react";
import { AbsoluteFill, Sequence, interpolate, staticFile, useCurrentFrame } from "remotion";
import { Video } from "@remotion/media";
import { C, F, glow } from "../theme";
import { T, hash } from "../timeline";
import { CaFilter, Mono, OUT, Typed, caText, clamp } from "../fx";
import { lp, useLayout } from "../layout";

const B = T.boot;

/** 0:00-0:03 CRT line opens, amber hairline, boot text types, real BOOT footage, 起動 flash. */
export const PowerOn: React.FC = () => {
  const f = useCurrentFrame();
  const { w, h, fps, portrait } = useLayout();
  const line = B.crt;
  // CRT: a white dot stretches to a line, flares open, collapses into the amber hairline
  const lineW = interpolate(f, [line, line + 10], [0, 1], { ...clamp, easing: OUT });
  const open = interpolate(f, [line + 9, line + 12, line + 19], [0, 1, 0], clamp);
  const hairY = lp(portrait, h * 0.56, h * 0.55);
  const amber = interpolate(f, [line + 16, line + 26], [0, 1], clamp);
  const typeSize = lp(portrait, 54, 64);
  const linked = f >= B.linked;
  // the hairline splits open onto the real BOOT view
  const split = interpolate(f, [B.footage, B.footage + 10], [0, 1], { ...clamp, easing: OUT });
  const kanji = f >= B.kanji;
  const k = f - B.kanji;
  const kScale = kanji ? interpolate(k, [0, 6, 28], [1.35, 1, 0.94], clamp) : 1;
  const kFlicker = kanji && (k < 2 || (k > 10 && k < 12) || (k > 20 && hash(f) > 0.5));
  const dots = Math.max(0, Math.min(5, Math.floor((f - (B.lines[1].at + B.lines[1].text.length * B.lines[1].fpc)) / 3)));

  return (
    <AbsoluteFill style={{ backgroundColor: C.black }}>
      {/* real BOOT footage, behind the split */}
      <Sequence name="BOOT footage" from={B.footage - 2} premountFor={fps}>
        <AbsoluteFill
          style={{
            clipPath: `inset(${(1 - split) * 50}% 0 ${(1 - split) * 50}% 0)`,
            filter: kanji ? "url(#ca-boot) brightness(0.45) saturate(1.4)" : undefined,
          }}
        >
          <Video
            src={staticFile("footage/boot.mp4")}
            trimBefore={Math.round(2.4 * fps)}
            muted
            style={{
              position: "absolute",
              width: 1920,
              height: 1080,
              left: 0,
              top: lp(portrait, 135, 0),
              scale: String(interpolate(f, [B.footage, 180], [lp(portrait, 1.05, 1.4), lp(portrait, 1.16, 1.55)], clamp)),
              transformOrigin: "0% 32%",
            }}
          />
        </AbsoluteFill>
      </Sequence>
      <CaFilter id="ca-boot" px={6} />

      {/* CRT line */}
      {f < B.footage ? (
        <div
          style={{
            position: "absolute",
            left: (w * (1 - lineW)) / 2,
            width: w * lineW,
            top: interpolate(amber, [0, 1], [h / 2, hairY]) - 1 - open * h * 0.2,
            height: 3 + open * h * 0.4,
            background:
              open > 0.05
                ? "linear-gradient(to bottom, rgba(255,255,255,0) 0%, rgba(255,255,255,0.55) 30%, #fff 47%, #fff 53%, rgba(255,255,255,0.55) 70%, rgba(255,255,255,0) 100%)"
                : amber > 0.5
                  ? C.amber
                  : "#fff",
            opacity: f < line ? 0 : 1,
            boxShadow: glow(amber > 0.5 ? C.amber : "#fff", 24),
          }}
        />
      ) : null}

      {/* boot text */}
      {f < B.footage + 6 ? (
        <AbsoluteFill style={{ opacity: f < B.footage ? 1 : 1 - split }}>
          <div
            style={{
              position: "absolute",
              left: lp(portrait, 70, 170),
              top: hairY - typeSize * 2.9,
              fontFamily: F.mono,
              fontWeight: 500,
              fontSize: typeSize,
              color: C.amber,
              lineHeight: 1.25,
              textShadow: glow(C.amber, 14),
            }}
          >
            <div>
              <Typed text={B.lines[0].text} at={B.lines[0].at} fpc={B.lines[0].fpc} cursor={f < B.lines[1].at} />
            </div>
            <div style={{ whiteSpace: "nowrap" }}>
              <Typed text={B.lines[1].text} at={B.lines[1].at} fpc={B.lines[1].fpc} cursor={false} />
              {f >= B.lines[1].at ? <span style={{ opacity: 0.7 }}>{" " + "·".repeat(dots)}</span> : null}
              {linked ? <span style={{ color: C.green, textShadow: glow(C.green, 16) }}>{" LINKED"}</span> : null}
            </div>
          </div>
          <Mono
            size={lp(portrait, 24, 26)}
            color={C.orange}
            style={{ position: "absolute", left: lp(portrait, 70, 170), top: hairY + 22, opacity: amber * 0.85 }}
          >
            {`MELCHIOR-1 ${f > 60 ? "OK" : ".."}   BALTHASAR-2 ${f > 80 ? "OK" : ".."}   CASPER-3 ${f > 100 ? "OK" : ".."}`}
          </Mono>
          <Mono size={22} color={C.orange} style={{ position: "absolute", right: lp(portrait, 70, 170), top: hairY + 22, opacity: amber * 0.6 }}>
            {portrait ? "" : `NERV · TERMINAL 01 · ${String(Math.floor(f * 16.7)).padStart(5, "0")}`}
          </Mono>
        </AbsoluteFill>
      ) : null}

      {/* 起動 */}
      {kanji ? (
        <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", backgroundColor: kFlicker ? "rgba(255,0,0,0.25)" : undefined }}>
          <div
            style={{
              fontFamily: F.serif,
              fontWeight: 900,
              fontSize: lp(portrait, 400, 520),
              lineHeight: 1,
              color: kFlicker ? C.red : "#fff",
              scale: String(kScale),
              letterSpacing: -10,
              textShadow: caText(k < 8 ? 14 : 5, glow("rgba(255,60,0,0.6)", 40)),
            }}
          >
            起動
          </div>
          <div
            style={{
              position: "absolute",
              bottom: lp(portrait, 260, 150),
              fontFamily: F.head,
              fontWeight: 700,
              fontSize: 54,
              letterSpacing: 18,
              color: C.orange,
              textShadow: glow(C.orange, 14),
            }}
          >
            MAGI · ACTIVATION
          </div>
        </AbsoluteFill>
      ) : null}
    </AbsoluteFill>
  );
};
