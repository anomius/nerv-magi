import React, { createContext, useContext, useMemo } from "react";
import { staticFile } from "remotion";
import { T, makeTimeline, type Timeline, type TrackMap } from "./timeline";

/** Input props of every cut (`--props`): what to cut to, and what to show. */
export type CutProps = {
  /** Sound under the picture, for Studio previews (renders are muted and finished with ffmpeg):
   * true = the medley (public/audio/score.wav, default track only), a string = a file under public/
   * laid in at the track's offsetSec, false = none. */
  audio?: boolean | string;
  /** The track map to cut to (the contents of tracks/<name>.json); default: the edit as written. */
  track?: TrackMap;
  /** Footage folder under public/: "footage" (1080p) or "footage-4k" (for `--scale 2` renders). */
  footage?: string;
};

type Ctx = { tl: Timeline; footage: string };
const Context = createContext<Ctx>({ tl: T, footage: "footage" });

export const TimelineProvider: React.FC<{ track?: TrackMap; footage?: string; children: React.ReactNode }> = ({ track, footage = "footage", children }) => {
  const value = useMemo(() => ({ tl: track ? makeTimeline(track) : T, footage }), [track, footage]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
};

/** The timeline of the track being cut to. */
export const useTL = () => useContext(Context).tl;

/** URL of a footage file in the current footage folder. */
export const useFootage = () => {
  const { footage } = useContext(Context);
  return (name: string) => staticFile(`${footage}/${name}`);
};
