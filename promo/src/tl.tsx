import React, { createContext, useContext, useMemo } from "react";
import { staticFile } from "remotion";
import { T, makeTimeline, type Timeline, type TrackMap } from "./timeline";
import { MARKS, type ClipMarks } from "./footage";

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
  /** The scripted moments in that folder's clips (its clips.json; default: the 1080p set's). */
  clips?: ClipMarks;
};

type Ctx = { tl: Timeline; footage: string; clips: ClipMarks };
const Context = createContext<Ctx>({ tl: T, footage: "footage", clips: MARKS });

export const TimelineProvider: React.FC<{ track?: TrackMap; footage?: string; clips?: ClipMarks; children: React.ReactNode }> = ({
  track,
  footage = "footage",
  clips = MARKS,
  children,
}) => {
  const value = useMemo(() => ({ tl: track ? makeTimeline(track) : T, footage, clips }), [track, footage, clips]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
};

/** The timeline of the track being cut to. */
export const useTL = () => useContext(Context).tl;

/** The scripted moments in the current footage folder's clips. */
export const useClipMarks = () => useContext(Context).clips;

/** URL of a footage file in the current footage folder. */
export const useFootage = () => {
  const { footage } = useContext(Context);
  return (name: string) => staticFile(`${footage}/${name}`);
};
