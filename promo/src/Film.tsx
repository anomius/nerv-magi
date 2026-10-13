import React from "react";
import { AbsoluteFill, Sequence, staticFile } from "remotion";
import { Audio } from "@remotion/media";
import "./theme";
import { T } from "./timeline";
import { Flash, Grain, Scanlines, Vignette } from "./fx";
import { useLayout } from "./layout";
import { PowerOn } from "./scenes/PowerOn";
import { Title } from "./scenes/Title";
import { Pattern } from "./scenes/Pattern";
import { Magi } from "./scenes/Magi";
import { Reveal } from "./scenes/Reveal";
import { Flow } from "./scenes/Flow";
import { Emergency } from "./scenes/Emergency";
import { Outro } from "./scenes/Outro";

const S = T.scenes;
const len = (s: readonly number[]) => s[1] - s[0];

/** NERV MAGI launch film: one continuous flow of eight beats (see timeline.json). */
export const Film: React.FC<{ audio?: boolean }> = ({ audio = true }) => {
  const { fps } = useLayout();
  return (
    <AbsoluteFill style={{ backgroundColor: "#000" }}>
      <Sequence name="0 Power-on" from={S.powerOn[0]} durationInFrames={len(S.powerOn)} premountFor={fps}>
        <PowerOn />
      </Sequence>
      <Sequence name="1 Title card" from={S.title[0]} durationInFrames={len(S.title)} premountFor={fps}>
        <Title />
      </Sequence>
      <Sequence name="2 Pattern detection" from={S.pattern[0]} durationInFrames={len(S.pattern)} premountFor={fps}>
        <Pattern />
      </Sequence>
      <Sequence name="3 MAGI deliberation" from={S.magi[0]} durationInFrames={len(S.magi)} premountFor={fps}>
        <Magi />
      </Sequence>
      <Sequence name="4 Reveal" from={S.reveal[0]} durationInFrames={len(S.reveal) + T.beat} premountFor={fps}>
        <Reveal />
      </Sequence>
      <Sequence name="5 View flow" from={S.flow[0]} durationInFrames={len(S.flow)} premountFor={fps}>
        <Flow />
      </Sequence>
      <Sequence name="6 Emergency" from={S.emergency[0]} durationInFrames={len(S.emergency)} premountFor={fps}>
        <Emergency />
      </Sequence>
      <Sequence name="7 Outro" from={S.outro[0]} durationInFrames={len(S.outro)} premountFor={fps}>
        <Outro />
      </Sequence>

      {/* texture over everything */}
      <Scanlines />
      <Grain />
      <Vignette />

      {/* smash cuts */}
      <Flash at={S.title[0]} dur={2} />
      <Flash at={S.title[1]} dur={2} />
      <Flash at={T.boot.kanji} dur={1} />
      <Flash at={T.magi.carried} dur={1} color="#cfffcf" peak={0.8} tail={4} />
      <Flash at={T.emergency.takeover} dur={2} color={"#ff2a00"} peak={0.85} tail={4} />
      <Flash at={T.emergency.resolve} dur={1} color="#cfffcf" peak={0.7} tail={4} />
      <Flash at={T.outro.slam} dur={2} />
      <Flash at={T.outro.cut} dur={2} />

      {audio ? <Audio name="Score" src={staticFile("audio/score.wav")} premountFor={fps} /> : null}
    </AbsoluteFill>
  );
};
