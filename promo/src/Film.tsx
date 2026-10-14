import React from "react";
import { AbsoluteFill, Sequence, staticFile, useVideoConfig } from "remotion";
import { Audio } from "@remotion/media";
import "./theme";
import { Flash, Grain, Scanlines, Vignette } from "./fx";
import { StageContext, TALL_SAFE, useLayout } from "./layout";
import { TimelineProvider, useTL, type CutProps } from "./tl";
import { PowerOn } from "./scenes/PowerOn";
import { Title } from "./scenes/Title";
import { Pattern } from "./scenes/Pattern";
import { Magi } from "./scenes/Magi";
import { Reveal } from "./scenes/Reveal";
import { Flow } from "./scenes/Flow";
import { Emergency } from "./scenes/Emergency";
import { Outro } from "./scenes/Outro";

const len = (s: readonly number[]) => s[1] - s[0];

/** The eight scenes, the screen texture and the smash-cut flashes, on the current timeline. */
const Scenes: React.FC = () => {
  const tl = useTL();
  const { fps } = useLayout();
  const S = tl.scenes;
  return (
    <>
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
      {/* the last callout runs on a beat under the view flow's hazard wipe */}
      <Sequence name="4 Reveal" from={S.reveal[0]} durationInFrames={len(S.reveal) + Math.round(tl.beat)} premountFor={fps}>
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
    </>
  );
};

/** Texture over everything, and the smash cuts (frame-length flashes, so they stay in frames).
 * `quiet`: a frame whose flash is skipped (a short cut opening on it: no white first frame / thumbnail). */
const Finish: React.FC<{ quiet?: number }> = ({ quiet }) => {
  const tl = useTL();
  const hit = (at: number) => at !== quiet;
  return (
    <>
      <Scanlines />
      <Grain />
      <Vignette />
      {hit(tl.scenes.title[0]) ? <Flash at={tl.scenes.title[0]} dur={2} /> : null}
      {hit(tl.scenes.title[1]) ? <Flash at={tl.scenes.title[1]} dur={2} /> : null}
      <Flash at={tl.boot.kanji} dur={1} />
      {hit(tl.magi.carried) ? <Flash at={tl.magi.carried} dur={1} color="#cfffcf" peak={0.8} tail={4} /> : null}
      {hit(tl.emergency.takeover) ? <Flash at={tl.emergency.takeover} dur={2} color={"#ff2a00"} peak={0.85} tail={4} /> : null}
      <Flash at={tl.emergency.resolve} dur={1} color="#cfffcf" peak={0.7} tail={4} />
      <Flash at={tl.outro.slam} dur={2} />
      <Flash at={tl.outro.cut} dur={2} />
    </>
  );
};

/** Studio preview sound: the medley, or a file under public/ laid in at the track's offsetSec. */
export const Soundtrack: React.FC<{ audio: CutProps["audio"] }> = ({ audio }) => {
  const tl = useTL();
  const { fps } = useVideoConfig();
  if (!audio) return null;
  if (audio === true) return <Audio name="Score" src={staticFile("audio/score.wav")} premountFor={fps} />;
  const off = Math.round(tl.offsetSec * fps);
  return (
    <Sequence name="Track" from={Math.max(0, -off)} premountFor={fps}>
      <Audio src={staticFile(audio)} trimBefore={Math.max(0, off)} />
    </Sequence>
  );
};

/** The edit's picture on the current timeline; in a 9:16 frame the scenes play on the safe-band stage. */
export const FilmPicture: React.FC<{ quiet?: number }> = ({ quiet }) => {
  const { width, height } = useVideoConfig();
  if (height / width < 1.5) {
    return (
      <AbsoluteFill style={{ backgroundColor: "#000" }}>
        <Scenes />
        <Finish quiet={quiet} />
      </AbsoluteFill>
    );
  }
  const stage = { w: width, h: height - TALL_SAFE.top - TALL_SAFE.bottom, top: TALL_SAFE.top, bottom: TALL_SAFE.bottom };
  return (
    <AbsoluteFill style={{ backgroundColor: "#000", overflow: "hidden" }}>
      <StageContext.Provider value={stage}>
        <div style={{ position: "absolute", left: 0, top: stage.top, width: stage.w, height: stage.h }}>
          <Scenes />
        </div>
      </StageContext.Provider>
      <Finish quiet={quiet} />
    </AbsoluteFill>
  );
};

/** NERV MAGI launch film: one continuous flow of eight scenes, cut to the track in `props.track`. */
export const Film: React.FC<CutProps> = ({ audio = true, track, footage, clips }) => (
  <TimelineProvider track={track} footage={footage} clips={clips}>
    <FilmPicture />
    <Soundtrack audio={audio} />
  </TimelineProvider>
);
