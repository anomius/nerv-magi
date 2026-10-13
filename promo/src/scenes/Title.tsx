import React from "react";
import { AbsoluteFill, interpolate, useCurrentFrame } from "remotion";
import { F } from "../theme";
import { T } from "../timeline";
import { clamp, jitter } from "../fx";
import { useLayout } from "../layout";

const S = T.scenes.title[0];

/** White serif on black, condensed, laid out as a broken L: the episode card homage. */
const Word: React.FC<{ children: React.ReactNode; style: React.CSSProperties; squeeze?: number }> = ({ children, style, squeeze = 0.84 }) => (
  <div
    style={{
      position: "absolute",
      fontFamily: F.serif,
      fontWeight: 900,
      color: "#F2F2F2",
      lineHeight: 1,
      whiteSpace: "nowrap",
      transformOrigin: "0 0",
      scale: `${squeeze} 1`,
      textShadow: "0 0 2px rgba(255,255,255,0.6)",
      ...style,
    }}
  >
    {children}
  </div>
);

/** 0:03-0:07 第壱話 / エージェント、襲来 / EPISODE:1 AGENT ATTACK. Smash cut in and out. */
export const Title: React.FC = () => {
  const frame = useCurrentFrame();
  const f = frame + S;
  const { portrait } = useLayout();
  const push = interpolate(frame, [0, 240], [1, 1.035], clamp);
  const j = jitter(f, S, 5, 22);
  const sub = f >= T.title.sub;
  return (
    <AbsoluteFill style={{ backgroundColor: "#000" }}>
      <AbsoluteFill style={{ scale: String(push), translate: `${j}px 0px` }}>
        {portrait ? (
          <>
            <Word style={{ left: 64, top: 92, fontSize: 150 }}>第壱話</Word>
            <Word style={{ left: 56, top: 318, fontSize: 146, letterSpacing: -4 }}>エージェント、</Word>
            <Word style={{ left: 742, top: 532, fontSize: 300, lineHeight: 1.02 }} squeeze={0.9}>
              襲<br />来
            </Word>
            {sub ? (
              <Word style={{ left: 64, top: 600, fontSize: 124, lineHeight: 1.14 }} squeeze={0.8}>
                EPISODE:1<br />AGENT<br />ATTACK
              </Word>
            ) : null}
          </>
        ) : (
          <>
            <Word style={{ left: 128, top: 86, fontSize: 172 }}>第壱話</Word>
            <Word style={{ left: 120, top: 330, fontSize: 214, letterSpacing: -6 }}>エージェント、</Word>
            <Word style={{ left: 1532, top: 92, fontSize: 330, lineHeight: 1.0 }} squeeze={0.9}>
              襲<br />来
            </Word>
            {sub ? (
              <Word style={{ left: 128, top: 846, fontSize: 124 }} squeeze={0.78}>
                EPISODE:1 AGENT ATTACK
              </Word>
            ) : null}
          </>
        )}
      </AbsoluteFill>
    </AbsoluteFill>
  );
};
