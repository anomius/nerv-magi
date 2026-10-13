import React from "react";
import { AbsoluteFill, Easing, interpolate, useCurrentFrame } from "remotion";
import { C, F, glow } from "../theme";
import { hash, type Timeline } from "../timeline";
import { useTL } from "../tl";
import { Hazard, Mono, OUT, Typed, beatShake, caText, clamp, hexPath } from "../fx";
import { lp, useLayout } from "../layout";
import { Links, TRIAD, Unit, VOTE_ORDER, type Vote } from "../magi";

export const magiScale = (portrait: boolean) => (portrait ? 2.9 : 3);
/** Centre of the triad; in portrait it keeps its distance from the frame's (or the 9:16 stage's) bottom. */
export const magiOrigin = (portrait: boolean, h: number) => (portrait ? { x: 540, y: 770 - (1350 - h) } : { x: 960, y: 540 });

/** Vote state of unit `u` at absolute frame `f` (`since` in canonical frames). */
export const voteState = (tl: Timeline, u: number, f: number): { state: Vote; since: number } => {
  const M = tl.magi;
  const k = VOTE_ORDER.indexOf(u as 0 | 1 | 2);
  const at = M.votes[k];
  if (f >= at) return { state: "approved", since: tl.k(f - at) };
  if (f >= M.deliberate) return { state: "deliberating", since: tl.k(f - M.deliberate) };
  return { state: "idle", since: 0 };
};

const HEADER = ["CODE      : 473", "FILE      : MAGI_SYS", "EXTENSION : 3023", "EX_MODE   : OFF", "PRIORITY  : AAA"];

/** 0:13-0:22 MAGI deliberation: three units, 審議中, then 承認 one by one, 可決 3/3. */
export const Magi: React.FC = () => {
  const frame = useCurrentFrame();
  const tl = useTL();
  const S = tl.scenes.magi[0];
  const M = tl.magi;
  const { d } = tl;
  const f = frame + S;
  const { w, h, portrait, tall, bleed } = useLayout();
  const s = magiScale(portrait);
  const o = magiOrigin(portrait, h);
  const approvedN = M.votes.filter((v) => f >= v).length;
  const carried = f >= M.carried;
  const ck = tl.k(f - M.carried);
  // the hub hex from the dive shrinks onto the MAGI core
  const core = { x: o.x + (TRIAD.magi.x - TRIAD.center.x) * s, y: o.y + (TRIAD.magi.y - TRIAD.center.y) * s };
  // the pattern dive ends with the hub hex at ~9x its 94 px radius, centred at y 640 (landscape)
  const hexR = interpolate(f, [S, S + d(22)], [lp(portrait, 900, 846), 34 * s], { ...clamp, easing: OUT });
  const hexY = interpolate(f, [S, S + d(22)], [lp(portrait, 700, 640), core.y], { ...clamp, easing: OUT });
  const links = interpolate(f, [S + d(12), S + d(60)], [0, 1], { ...clamp, easing: OUT });
  const riser = interpolate(f, [M.riser[0], M.riser[1]], [0, 1], { ...clamp, easing: Easing.in(Easing.quad) });
  const shake = beatShake(f, M.carried, M.carried + d(12), 30, 16);
  const voteShake = M.votes.reduce((acc, v) => {
    const b = beatShake(f, v, v + d(8), 30, 7);
    return { x: acc.x + b.x, y: acc.y + b.y };
  }, shake);
  const linkColor = approvedN === 3 ? C.green : C.orange;
  const bannerOut = interpolate(f, [M.riser[1] - d(14), M.riser[1] - d(4)], [0, 1], { ...clamp, easing: Easing.in(Easing.cubic) });

  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: tall ? "visible" : "hidden" }}>
      <AbsoluteFill style={{ translate: `${voteShake.x}px ${voteShake.y}px` }}>
        {/* faint field grid */}
        <AbsoluteFill
          style={{
            top: -bleed.top,
            bottom: -bleed.bottom,
            opacity: 0.22,
            backgroundImage: `linear-gradient(rgba(255,153,0,0.35) 1px, transparent 1px), linear-gradient(90deg, rgba(255,153,0,0.35) 1px, transparent 1px)`,
            backgroundSize: "60px 60px",
            backgroundPosition: `${frame * 0.4}px ${frame * 0.25}px`,
          }}
        />
        {/* triad, pushed slightly by the riser */}
        <AbsoluteFill style={{ scale: String(1 + riser * 0.05), transformOrigin: `${o.x}px ${o.y}px` }}>
          <Links s={s} ox={o.x} oy={o.y} p={links} color={linkColor} width={w} height={h} />
          <svg width={w} height={h} style={{ position: "absolute" }}>
            <path d={hexPath(core.x, hexY, hexR)} fill="rgba(255,170,0,0.06)" stroke={approvedN === 3 ? C.green : C.amber} strokeWidth={4} />
          </svg>
          <div
            style={{
              position: "absolute",
              left: core.x - 200,
              top: core.y - 18 * s,
              width: 400,
              textAlign: "center",
              fontFamily: F.head,
              fontWeight: 700,
              fontSize: 26 * s,
              letterSpacing: 2.5 * s,
              color: approvedN === 3 ? C.green : C.amber,
              textShadow: glow(approvedN === 3 ? C.green : C.amber, 8 * s),
              opacity: interpolate(f, [S + d(10), S + d(22)], [0, 1], clamp),
            }}
          >
            MAGI
          </div>
          {TRIAD.units.map((u, i) => {
            const v = voteState(tl, i, f);
            const draw = interpolate(f, [S + d(20 + i * 8), S + d(34 + i * 8)], [0, 1], clamp);
            const flick = draw < 1 && hash(f + i) > 0.5 ? 0.3 : 1;
            return (
              <Unit
                key={u.id}
                {...u}
                x={o.x + (u.x - TRIAD.center.x) * s}
                y={o.y + (u.y - TRIAD.center.y) * s}
                s={s}
                state={v.state}
                since={v.since}
                frame={f}
                draw={draw * flick}
              />
            );
          })}
        </AbsoluteFill>

        {/* header data */}
        <div style={{ position: "absolute", left: lp(portrait, 40, 56), top: lp(portrait, 44, 96) }}>
          <Mono size={lp(portrait, 30, 38)} color={C.orange} style={{ lineHeight: 1.32, textShadow: glow(C.orange, 8) }}>
            {HEADER.map((line, k) => (
              <div key={k}>
                <Typed text={line} at={d(8 + k * 9)} fpc={d(1)} cursor={false} />
              </div>
            ))}
          </Mono>
        </div>

        {/* deliberation meter and timer (landscape side columns) */}
        {!portrait && f >= M.deliberate - d(10) ? (
          <>
            <div style={{ position: "absolute", left: 56, top: 430, width: 400 }}>
              <div style={{ display: "flex", alignItems: "baseline", gap: 14, marginBottom: 10 }}>
                <span style={{ fontFamily: F.serif, fontWeight: 900, fontSize: 44, color: C.white }}>審議</span>
                <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: 30, letterSpacing: 3, color: C.orange }}>DELIBERATION</span>
              </div>
              {VOTE_ORDER.map((u, k) => {
                const p = interpolate(f, [M.deliberate, M.votes[k]], [0, 1], clamp);
                const done = f >= M.votes[k];
                const color = done ? C.green : C.orange;
                return (
                  <div key={u} style={{ marginBottom: 12 }}>
                    <Mono size={28} color={color}>{TRIAD.units[u].id.replace("·", "-")}</Mono>
                    <div style={{ display: "flex", gap: 3, marginTop: 4 }}>
                      {Array.from({ length: 20 }, (_, b) => (
                        <div key={b} style={{ width: 18, height: 22, backgroundColor: b / 20 < p ? color : "rgba(255,153,0,0.15)", boxShadow: b / 20 < p ? glow(color, 5) : undefined }} />
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
            <div style={{ position: "absolute", right: 56, top: 470, textAlign: "right" }}>
              <div style={{ fontFamily: F.serif, fontWeight: 900, fontSize: 40, color: C.white }}>審議時間</div>
              <div
                style={{
                  fontFamily: F.seg,
                  fontStyle: "italic",
                  fontWeight: 700,
                  fontSize: 76,
                  color: carried ? C.green : C.timer,
                  textShadow: glow(carried ? C.green : C.timer, 14),
                }}
              >
                {(() => {
                  const t = Math.max(0, (Math.min(f, M.carried) - M.deliberate) / tl.fps);
                  return `${String(Math.floor(t)).padStart(2, "0")}:${String(Math.floor((t % 1) * 100)).padStart(2, "0")}`;
                })()}
              </div>
              <Mono size={24} color={C.orange} style={{ opacity: 0.8 }}>
                {carried ? "RESOLVED · UNANIMOUS" : "VOTING · 3 UNITS"}
              </Mono>
            </div>
          </>
        ) : null}

        {/* the motion: 提訴 */}
        {f >= M.motion ? (
          <div
            style={{
              position: "absolute",
              right: lp(portrait, 40, 56),
              top: lp(portrait, 44, 66),
              textAlign: "right",
              translate: `${interpolate(f, [M.motion, M.motion + d(8)], [60, 0], { ...clamp, easing: OUT })}px 0px`,
            }}
          >
            <span
              style={{
                display: "inline-block",
                padding: "0 18px 6px",
                backgroundColor: C.red,
                fontFamily: F.serif,
                fontWeight: 900,
                fontSize: lp(portrait, 92, 104),
                color: "#fff",
                lineHeight: 1.1,
                textShadow: caText(tl.k(f - M.motion) < 6 ? 8 : 0),
              }}
            >
              提訴
            </span>
            <div style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 46, 58), color: C.white, letterSpacing: 2, lineHeight: 1.08, marginTop: 10 }}>
              DEPLOY DASHBOARD
              <br />
              TO TV
            </div>
          </div>
        ) : null}

        {/* tally */}
        <div
          style={{
            position: "absolute",
            left: lp(portrait, 40, 56),
            bottom: lp(portrait, 40, 92),
            opacity: f >= M.deliberate ? 1 : 0,
          }}
        >
          <Mono size={lp(portrait, 26, 30)} color={C.orange} style={{ opacity: 0.85 }}>
            RESULT OF THE DELIBERATION
          </Mono>
          <div style={{ display: "flex", alignItems: "baseline", gap: 18, marginTop: 4 }}>
            <span
              style={{
                fontFamily: F.serif,
                fontWeight: 900,
                fontSize: lp(portrait, 76, 92),
                color: approvedN ? C.green : C.orange,
                opacity: approvedN === 0 && Math.floor(f / 10) % 2 ? 0.25 : 1,
                textShadow: glow(approvedN ? C.green : C.orange, 12),
              }}
            >
              {approvedN ? "承認" : "審議中"}
            </span>
            <span style={{ fontFamily: F.seg, fontStyle: "italic", fontWeight: 700, fontSize: lp(portrait, 72, 88), color: approvedN ? C.green : C.orange }}>
              {approvedN}/3
            </span>
          </div>
        </div>
        {/* vote log */}
        <Mono
          size={lp(portrait, 24, 26)}
          color={C.green}
          style={{ position: "absolute", right: lp(portrait, 40, 36), ...(portrait ? { bottom: 48 } : { top: 776 }), textAlign: "right", lineHeight: 1.4 }}
        >
          {VOTE_ORDER.map((u, k) =>
            f >= M.votes[k] ? (
              <div key={u}>
                {TRIAD.units[u].id.replace("·", "-")} <span style={{ color: "#fff" }}>· </span>
                <span style={{ fontFamily: F.serif, fontWeight: 900 }}>承認</span> APPROVED
              </div>
            ) : (
              <div key={u} style={{ color: C.orange, opacity: f >= M.deliberate ? 0.7 : 0 }}>
                {TRIAD.units[u].id.replace("·", "-")} · ......
              </div>
            ),
          )}
        </Mono>
        <Mono size={22} color={C.orange} style={{ position: "absolute", left: 0, right: 0, bottom: 28, opacity: portrait ? 0 : 0.45, overflow: "hidden" }}>
          {Array.from({ length: 12 }, (_, k) => (Math.floor(hash(k + Math.floor(f / 4)) * 0xffffffff) >>> 0).toString(16).toUpperCase().padStart(8, "0")).join("  ")}
        </Mono>
      </AbsoluteFill>

      {/* 可決 */}
      {carried ? (
        <AbsoluteFill style={{ justifyContent: "center", alignItems: "center", translate: `${bannerOut * w * 1.1}px 0px` }}>
          <div style={{ position: "relative", width: "100%", height: 26, marginBottom: 8 }}>
            <Hazard style={{ inset: 0 }} color={C.green} speed={5} />
          </div>
          <div
            style={{
              width: "100%",
              padding: lp(portrait, "22px 0 30px", "18px 0 26px"),
              backgroundColor: ck < 3 ? "#00ff00" : "rgba(0,0,0,0.97)",
              borderTop: `10px solid ${C.green}`,
              borderBottom: `10px solid ${C.green}`,
              display: "flex",
              flexDirection: portrait ? "column" : "row",
              alignItems: "center",
              justifyContent: "center",
              gap: lp(portrait, 0, 100),
              scale: String(interpolate(ck, [0, 6], [1.18, 1], { ...clamp, easing: OUT })),
              boxShadow: `0 0 80px rgba(0,255,0,0.35)`,
            }}
          >
            <span
              style={{
                fontFamily: F.serif,
                fontWeight: 900,
                fontSize: lp(portrait, 210, 240),
                lineHeight: 1,
                color: ck < 3 ? "#002200" : C.green,
                textShadow: ck < 3 ? undefined : caText(ck < 10 ? 10 : 3, glow(C.green, 30)),
              }}
            >
              可決
            </span>
            <span style={{ fontFamily: F.head, fontWeight: 700, fontSize: lp(portrait, 110, 128), letterSpacing: 6, color: ck < 3 ? "#002200" : "#fff", lineHeight: 1 }}>
              3/3 APPROVED
            </span>
          </div>
          <div style={{ position: "relative", width: "100%", height: 26, marginTop: 8 }}>
            <Hazard style={{ inset: 0 }} color={C.green} speed={-5} />
          </div>
        </AbsoluteFill>
      ) : null}
    </AbsoluteFill>
  );
};
