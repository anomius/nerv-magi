import React from "react";
import { Composition, Folder, type CalculateMetadataFunction } from "remotion";
import { FPS, T, makeTimeline } from "./timeline";
import { Film } from "./Film";
import { Cut } from "./Cut";
import { Hero } from "./Hero";
import { editLength, type EditName } from "./edits";
import type { CutProps } from "./tl";

// The length follows the track map passed as input props (`--props '{"track": {...}}'`).
const filmLength: CalculateMetadataFunction<CutProps> = ({ props }) => ({ durationInFrames: makeTimeline(props.track).total });
const cutLength: CalculateMetadataFunction<CutProps & { edit: EditName }> = ({ props }) => ({
  durationInFrames: editLength(makeTimeline(props.track), props.edit),
});
const film: CutProps = { audio: true, footage: "footage" };

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="NervMagiX"
        component={Film}
        durationInFrames={T.total}
        fps={FPS}
        width={1920}
        height={1080}
        defaultProps={film}
        calculateMetadata={filmLength}
      />
      <Composition id="HeroLoop" component={Hero} durationInFrames={420} fps={FPS} width={1920} height={1080} />
      <Folder name="Recompose">
        <Composition
          id="NervMagiX4x5"
          component={Film}
          durationInFrames={T.total}
          fps={FPS}
          width={1080}
          height={1350}
          defaultProps={film}
          calculateMetadata={filmLength}
        />
        <Composition
          id="NervMagiVertical"
          component={Film}
          durationInFrames={T.total}
          fps={FPS}
          width={1080}
          height={1920}
          defaultProps={film}
          calculateMetadata={filmLength}
        />
      </Folder>
      <Folder name="Short">
        <Composition
          id="Teaser15"
          component={Cut}
          durationInFrames={editLength(T, "teaser15")}
          fps={FPS}
          width={1920}
          height={1080}
          defaultProps={{ ...film, edit: "teaser15" as EditName }}
          calculateMetadata={cutLength}
        />
        <Composition
          id="Cut30"
          component={Cut}
          durationInFrames={editLength(T, "cut30")}
          fps={FPS}
          width={1920}
          height={1080}
          defaultProps={{ ...film, edit: "cut30" as EditName }}
          calculateMetadata={cutLength}
        />
      </Folder>
    </>
  );
};
