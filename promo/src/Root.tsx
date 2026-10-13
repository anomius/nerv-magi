import React from "react";
import { Composition, Folder } from "remotion";
import { T } from "./timeline";
import { Film } from "./Film";
import { Hero } from "./Hero";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition id="NervMagiX" component={Film} durationInFrames={T.total} fps={T.fps} width={1920} height={1080} defaultProps={{ audio: true }} />
      <Composition id="HeroLoop" component={Hero} durationInFrames={420} fps={T.fps} width={1920} height={1080} />
      <Folder name="Recompose">
        <Composition
          id="NervMagiX4x5"
          component={Film}
          durationInFrames={T.total}
          fps={T.fps}
          width={1080}
          height={1350}
          defaultProps={{ audio: true }}
        />
      </Folder>
    </>
  );
};
