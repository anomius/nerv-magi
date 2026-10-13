import React from "react";
import { AbsoluteFill, Sequence, interpolate, useCurrentFrame } from "remotion";
import { Video } from "@remotion/media";
import { C, F, glow } from "../theme";
import { hash } from "../timeline";
import { useFootage, useTL } from "../tl";
import { CLIP } from "../footage";
import { Hazard, Mono, OUT, caText, clamp, hexCells, hexPath } from "../fx";
import { lp, useLayout } from "../layout";
import { Shot, type Cam } from "../camera";

const WIPE = 20; // canonical frames

/** Full-frame footage through the scene camera (`from` absolute, `dur` frames). */
const Clip: React.FC<{ src: string; from: number; dur?: number; trim: number; cam: Cam; style?: React.CSSProperties; name: string }> = ({
  src,
  from,
  dur,
  trim,
  cam,
  style,
  name,
}) => {
  const { fps } = useLayout();
  const tl = useTL();
  const footage = useFootage();
  return (
    <Sequence name={name} from={from - tl.scenes.flow[0]} durationInFrames={dur === undefined ? undefined : Math.round(dur)} premountFor={fps}>
      <AbsoluteFill style={style}>
        <Shot cam={cam}>
          <Video src={footage(src)} trimBefore={trim} muted style={{ position: "absolute", width: 1920, height: 1080 }} />
        </Shot>
      </AbsoluteFill>
    </Sequence>
  );
};

/** A view name card: big latin + kanji on a black plate. */
const ViewLabel: React.FC<{ k: number; latin: string; kanji: string; sub: string }> = ({ k, latin, kanji, sub }) => {
  const { portrait } = useLayout();
  if (k < 0) return null;
  const x = interpolate(k, [0, 10], [-80, 0], { ...clamp, easing: OUT });
  return (
    <div
      style={{
        position: "absolute",
        left: lp(portrait, 0, 0),
        bottom: lp(portrait, 70, 110),
        translate: `${x}px 0px`,
        padding: lp(portrait, "14px 70px 20px 60px", "14px 60px 20px 40px"),
        background: "linear-gradient(90deg, rgba(0,0,0,0.94) 75%, rgba(0,0,0,0))",
        borderLeft: `14px solid ${C.orange}`,
      }}
    >
      <div style={{ display: "flex", alignItems: "baseline", gap: 26 }}>
        <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 150, 130), lineHeight: 1, letterSpacing: 4, color: "#fff", textShadow: caText(k < 6 ? 8 : 0) }}>
          {latin}
        </span>
        <span style={{ fontFamily: F.serif, fontWeight: 900, fontSize: lp(portrait, 140, 120), lineHeight: 1, color: C.orange, textShadow: glow(C.orange, 16) }}>{kanji}</span>
      </div>
      <div style={{ fontFamily: F.cond, fontWeight: 600, fontSize: lp(portrait, 44, 48), color: C.amber, marginTop: 6 }}>{sub}</div>
    </div>
  );
};

/** 0:36-0:42 LOAD, SYNC, back to MAIN: hazard-stripe wipe, scanline roll, hex dissolve. */
export const Flow: React.FC = () => {
  const frame = useCurrentFrame();
  const tl = useTL();
  const S = tl.scenes.flow[0];
  const FL = tl.flow;
  const { d } = tl;
  const f = frame + S;
  const { w, h, portrait, tall, bleed } = useLayout();
  const cam: Cam = portrait ? { zoom: 0.74, fx: 960, fy: 540, ax: 540, ay: 560 } : { zoom: 1, fx: 960, fy: 540, ax: 960, ay: 540 };
  const drift = (at: number) => ({ ...cam, zoom: cam.zoom * (1 + Math.max(0, tl.k(f - at)) * 0.00035) });

  // hazard-stripe wipe: a diagonal edge sweeping left to right
  const wx = interpolate(f, [FL.load, FL.load + d(WIPE)], [-400, w + 400], { ...clamp, easing: OUT });
  const slant = 260;
  // scanline roll: LOAD slips up out of frame, SYNC rolls in from below
  const roll = interpolate(f, [FL.sync, FL.sync + d(18)], [0, 1], { ...clamp, easing: OUT });
  // hex dissolve: cells reveal MAIN in a hashed order
  const cells = hexCells(w, h, lp(portrait, 60, 70));
  const dissolve = (k: number) => FL.main + d(Math.floor(hash(k * 1.7) * 20));

  return (
    // transparent: the reveal's last shot runs on underneath until the hazard wipe has passed
    <AbsoluteFill style={{ overflow: tall ? "visible" : "hidden" }}>
      {/* LOAD behind the hazard wipe */}
      <Clip
        name="LOAD"
        src="load.mp4"
        from={FL.load}
        dur={FL.sync + d(18) - FL.load}
        trim={CLIP.loadIn}
        cam={drift(FL.load)}
        style={{
          clipPath: f < FL.load + d(WIPE) ? `polygon(0 0, ${wx}px 0, ${wx - slant}px 100%, 0 100%)` : undefined,
          translate: `0px ${-roll * h}px`,
        }}
      />
      {f >= FL.load && f < FL.load + d(WIPE + 2) ? (
        <div style={{ position: "absolute", left: wx - 120, top: -40 - bleed.top, width: 150, height: h + 80 + bleed.top + bleed.bottom, transform: `skewX(${-Math.atan(slant / h) * (180 / Math.PI)}deg)`, overflow: "hidden" }}>
          <Hazard style={{ inset: 0 }} stripe={30} speed={0} color={C.amber} />
        </div>
      ) : null}
      {/* SYNC rolls in from below */}
      <Clip name="SYNC" src="sync.mp4" from={FL.sync} dur={FL.main + d(22) - FL.sync} trim={CLIP.syncIn} cam={drift(FL.sync)} style={{ translate: `0px ${(1 - roll) * h}px` }} />
      {f >= FL.sync && f < FL.sync + d(18) ? (
        <>
          <div style={{ position: "absolute", left: 0, right: 0, top: (1 - roll) * h - 26, height: 26, backgroundColor: "#000" }} />
          <AbsoluteFill
            style={{
              backgroundImage: "repeating-linear-gradient(to bottom, rgba(255,170,0,0.0) 0 3px, rgba(255,170,0,0.22) 3px 5px)",
              opacity: 1 - roll,
            }}
          />
        </>
      ) : null}
      {/* MAIN through the hex dissolve */}
      <svg width={0} height={0} style={{ position: "absolute" }}>
        <clipPath id="hexdissolve">
          {cells.map((c, k) => (f >= dissolve(k) ? <path key={k} d={hexPath(c.cx, c.cy, lp(portrait, 61, 71))} /> : null))}
        </clipPath>
      </svg>
      <Clip name="MAIN back" src="main.mp4" from={FL.main} trim={CLIP.mainFlowIn} cam={drift(FL.main)} style={{ clipPath: f < FL.main + d(22) ? "url(#hexdissolve)" : undefined }} />
      {f >= FL.main ? (
        <>
          {f < FL.main + d(26) ? (
            <svg width={w} height={h} style={{ position: "absolute" }}>
              {cells.map((c, k) => {
                const x = tl.k(f - dissolve(k));
                return x >= 0 && x < 5 ? <path key={k} d={hexPath(c.cx, c.cy, lp(portrait, 58, 68))} fill="rgba(255,170,0,0.35)" stroke={C.amber} strokeWidth={3} /> : null;
              })}
            </svg>
          ) : null}
        </>
      ) : null}

      <ViewLabel k={f >= FL.sync ? -1 : tl.k(f - FL.load) - 8} latin="LOAD" kanji="負荷" sub="cores, memory, GPU, thermals, processes" />
      <ViewLabel k={f >= FL.main ? -1 : tl.k(f - FL.sync) - 10} latin="SYNC" kanji="同期率" sub="context fill per pilot, tokens, plan limits" />
      <ViewLabel k={tl.k(f - FL.main) - 12} latin="MAIN" kanji="作戦行動" sub="views rotate on their own, or pin one" />

      {/* the alarm creeping in before EMERGENCY */}
      {f >= FL.alarm ? (
        <AbsoluteFill
          style={{
            top: -bleed.top,
            bottom: -bleed.bottom,
            backgroundColor: C.red,
            opacity: Math.floor(tl.k(f - FL.alarm) / 5) % 2 ? 0.0 : 0.16 + tl.k(f - FL.alarm) * 0.008,
          }}
        />
      ) : null}
      {f >= FL.alarm ? (
        <Mono size={lp(portrait, 34, 40)} color={C.red} style={{ position: "absolute", right: 50, top: 44, textShadow: glow(C.red, 12) }}>
          {Math.floor(tl.k(f - FL.alarm) / 6) % 2 ? "" : "▲ PILOT STATUS CHANGE"}
        </Mono>
      ) : null}
    </AbsoluteFill>
  );
};
