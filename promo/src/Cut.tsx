import React from "react";
import { AbsoluteFill, Sequence, staticFile, useVideoConfig } from "remotion";
import { Audio } from "@remotion/media";
import { Flash } from "./fx";
import { FilmPicture } from "./Film";
import { TimelineProvider, useTL, type CutProps } from "./tl";
import { editSegments, type EditName } from "./edits";

const Segments: React.FC<{ edit: EditName; audio: CutProps["audio"] }> = ({ edit, audio }) => {
  const tl = useTL();
  const { fps } = useVideoConfig();
  const segs = editSegments(tl, edit);
  return (
    <AbsoluteFill style={{ backgroundColor: "#000" }}>
      {segs.map((s) => (
        // the outer sequence places the range in the cut; the inner one starts the film at the range's first frame
        <Sequence key={s.at} name={`bars ${s.bars[0]}-${s.bars[1] - 1}`} from={s.at} durationInFrames={s.to - s.from} premountFor={fps}>
          <Sequence from={-s.from}>
            <FilmPicture quiet={s.at === 0 ? s.from : undefined} />
          </Sequence>
        </Sequence>
      ))}
      {segs.map((s) => (s.flash ? <Flash key={`flash-${s.at}`} at={s.at} dur={2} /> : null))}
      {/* Studio preview only: the cut's own splice of the medley (renders are muted, finished with ffmpeg) */}
      {audio === true ? <Audio name="Score (spliced)" src={staticFile(`audio/${edit}.wav`)} premountFor={fps} /> : null}
    </AbsoluteFill>
  );
};

/** A short cut of the film (src/edits.ts), on the timeline of `props.track`. */
export const Cut: React.FC<CutProps & { edit: EditName }> = ({ edit, audio = false, track, footage, clips }) => (
  <TimelineProvider track={track} footage={footage} clips={clips}>
    <Segments edit={edit} audio={audio} />
  </TimelineProvider>
);
