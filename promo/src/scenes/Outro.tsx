import React from "react";
import { AbsoluteFill, Img, interpolate, useCurrentFrame } from "remotion";
import { C, F, REPO_URL, glow } from "../theme";
import { useFootage, useTL } from "../tl";
import { Hazard, OUT, Typed, caText, clamp, hexCells, hexPath, jitter } from "../fx";
import { lp, useLayout } from "../layout";

/** 0:48-0:56 NERV MAGI, what it is, where to get it; smash cut to black: つづく. */
export const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const tl = useTL();
  const footage = useFootage();
  const S = tl.scenes.outro[0];
  const O = tl.outro;
  const f = frame + S;
  const fk = tl.k(frame); // the scene's clock in canonical frames
  const { w, h, portrait, tall, bleed } = useLayout();
  const sk = tl.k(f - O.slam);
  const slam = interpolate(sk, [0, 7], [1.5, 1], { ...clamp, easing: OUT });
  const cut = f >= O.cut;
  const cells = hexCells(w, h + bleed.top + bleed.bottom, 64, 0, -bleed.top);
  const j = jitter(f, O.slam, tl.d(6), 30);
  // before the slam the lines sit centred; the title pushes them down into place
  const pushP = interpolate(sk, [0, 8], [0, 1], { ...clamp, easing: OUT });
  const push = -(1 - pushP) * lp(portrait, 250, 215);
  const lineIn = (i: number) => interpolate(f, [O.lines[i], O.lines[i] + tl.d(10)], [0, 1], { ...clamp, easing: OUT });

  if (cut) {
    const k = tl.k(f - O.cut);
    return (
      <AbsoluteFill style={{ backgroundColor: "#000" }}>
        {k >= 8 ? (
          <div
            style={{
              position: "absolute",
              right: lp(portrait, 140, 80),
              bottom: lp(portrait, 120, 130),
              fontFamily: F.serif,
              fontWeight: 900,
              fontSize: lp(portrait, 170, 170),
              color: "#F2F2F2",
              scale: "0.86 1",
              transformOrigin: "100% 100%",
            }}
          >
            つづく
          </div>
        ) : null}
      </AbsoluteFill>
    );
  }

  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: tall ? "visible" : "hidden" }}>
      {/* the dashboard, far behind */}
      <Img
        src={footage("main-4k.png")}
        style={{
          position: "absolute",
          width: 1920,
          height: 1080,
          left: (w - 1920) / 2,
          top: (h - 1080) / 2,
          opacity: 0.16,
          filter: "blur(3px) saturate(0.6)",
          scale: String(lp(portrait, tall ? 2 : 1.25, 1.5) + fk * 0.0004),
        }}
      />
      <svg width={w} height={h} style={{ position: "absolute", overflow: tall ? "visible" : undefined }}>
        {cells.map((c, k) => (
          <path
            key={k}
            d={hexPath(c.cx, c.cy, 60)}
            fill="none"
            stroke={C.orange}
            strokeWidth={1.4}
            opacity={0.08 + 0.25 * Math.max(0, Math.sin((c.cx * 0.004 + c.cy * 0.003 - frame * 0.05) * 2))}
          />
        ))}
      </svg>
      <Hazard style={{ left: 0, right: 0, top: -bleed.top, height: 22 }} speed={2} />
      <Hazard style={{ left: 0, right: 0, bottom: -bleed.bottom, height: 22 }} speed={-2} />

      <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", translate: `${j}px 0px`, scale: String(1 + fk * 0.00012) }}>
        <div style={{ textAlign: "center", marginTop: lp(portrait, -80, -40) }}>
          <div style={{ opacity: sk >= 0 ? 1 : 0 }}>
          <div style={{ fontFamily: F.serif, fontWeight: 900, fontSize: lp(portrait, 44, 62), color: C.orange, letterSpacing: lp(portrait, 6, 18), whiteSpace: "nowrap", opacity: sk > 4 ? 1 : 0 }}>
            汎用エージェント監視 · 第壱号機
          </div>
          <div style={{ position: "relative", scale: String(slam) }}>
            {[false, true].map((sheen) => (
              <div
                key={String(sheen)}
                style={{
                  position: sheen ? "absolute" : "relative",
                  inset: sheen ? 0 : undefined,
                  fontFamily: F.head,
                  fontWeight: 700,
                  fontSize: lp(portrait, 196, 290),
                  lineHeight: 1,
                  letterSpacing: lp(portrait, 8, 18),
                  whiteSpace: portrait ? "normal" : "nowrap",
                  ...(sheen
                    ? {
                        // a light sweep across the letters every two seconds
                        color: "transparent",
                        backgroundImage: "linear-gradient(100deg, rgba(255,210,122,0) 42%, rgba(255,226,170,0.95) 50%, rgba(255,210,122,0) 58%)",
                        backgroundSize: "300% 100%",
                        backgroundPosition: `${100 - ((((sk - 20) % 120) + 120) % 120 / 60) * 100}% 0%`,
                        WebkitBackgroundClip: "text",
                        backgroundClip: "text",
                        opacity: sk > 20 && (sk - 20) % 120 < 60 ? 1 : 0,
                      }
                    : { color: "#fff", textShadow: caText(sk < 8 ? 16 : 4, glow("rgba(255,153,0,0.55)", 40)) }),
                }}
              >
                {portrait ? (
                  <>
                    NERV
                    <br />
                    MAGI
                  </>
                ) : (
                  "NERV MAGI"
                )}
              </div>
            ))}
          </div>
          </div>
          <div style={{ translate: `0px ${push}px`, scale: String(1 + 0.12 * (1 - pushP)) }}>
          <div style={{ height: 8, margin: "18px auto 26px", width: `${interpolate(sk, [4, 20], [0, 100], { ...clamp, easing: OUT })}%`, maxWidth: lp(portrait, 900, 1500), backgroundColor: C.red, boxShadow: glow(C.red, 14) }} />
          <div
            style={{
              fontFamily: F.cond,
              fontWeight: 600,
              fontSize: lp(portrait, 58, 66),
              color: C.white,
              opacity: lineIn(0),
              translate: `0px ${(1 - lineIn(0)) * 20}px`,
              lineHeight: 1.15,
              padding: lp(portrait, "0 40px", "0"),
            }}
          >
            agents · telemetry · now playing, on your TV
          </div>
          <div
            style={{
              fontFamily: F.cond,
              fontWeight: 600,
              fontSize: lp(portrait, 46, 52),
              color: C.amber,
              opacity: lineIn(1),
              translate: `0px ${(1 - lineIn(1)) * 20}px`,
              marginTop: 10,
              lineHeight: 1.2,
              padding: lp(portrait, "0 50px", "0"),
            }}
          >
            herdr plugin · Omarchy plugin · GPU-native, no server
          </div>
          <div
            style={{
              display: "inline-block",
              marginTop: lp(portrait, 40, 34),
              padding: "8px 26px 12px",
              border: `3px solid ${C.green}`,
              fontFamily: F.mono,
              fontWeight: 500,
              fontSize: lp(portrait, 44, 54),
              color: C.green,
              textShadow: glow(C.green, 10),
              opacity: f >= O.lines[2] ? 1 : 0,
            }}
          >
            <Typed text={REPO_URL} at={O.lines[2] - S} fpc={tl.d(2)} />
          </div>
          </div>
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
