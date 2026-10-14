/**
 * The recorded clips (public/footage*, scripts/record-footage.sh) and where the edit enters them, in
 * clip frames (60 fps). `clipNeeds(tl, marks)`: the frames of each clip a timeline plays, for the
 * preflight check (a slow track or a stretched section can outrun a clip).
 */
import type { Timeline } from "./timeline";

export const CLIP = {
  /** boot.mp4: from 2.4 s, under the CRT split. */
  bootIn: 144,
  /** main.mp4: the reveal's live wide shot enters at 1 s, the view flow's MAIN at 3 s. */
  mainWideIn: 60,
  mainFlowIn: 180,
  loadIn: 60,
  syncIn: 60,
} as const;

/**
 * The scripted moments inside two clips, which move a little from one recording to the next: measured
 * per footage folder by scripts/measure-clips.mjs into <folder>/clips.json and passed in as props.
 * The defaults are the 1080p set's.
 */
export type ClipMarks = {
  /** emergency.mp4: the frame where the real red takeover starts. */
  takeover: number;
  /** resolve.mp4: the frame where MAIN wipes back in after CAUSE CLEARED (one after its first changed frame). */
  wipeBack: number;
};
export const MARKS: ClipMarks = { takeover: 84, wipeBack: 183 };

/** Clip frames each clip must have for this timeline (and where it is entered: a negative entry cannot be played). */
export const clipNeeds = (tl: Timeline, marks: ClipMarks = MARKS) => {
  const { d } = tl;
  const { boot: B, reveal: R, flow: FL, emergency: E, scenes: S } = tl;
  const liveFrom = Math.round(R.wide - d(14));
  const resolveIn = marks.wipeBack - (E.resolve - E.clear);
  return {
    "boot.mp4": { from: CLIP.bootIn, to: CLIP.bootIn + S.powerOn[1] - Math.round(B.footage - d(2)) },
    "main.mp4": { from: CLIP.mainWideIn, to: Math.max(CLIP.mainWideIn + Math.round(R.callouts[0] + d(16)) - liveFrom, CLIP.mainFlowIn + S.flow[1] - FL.main) },
    "load.mp4": { from: CLIP.loadIn, to: CLIP.loadIn + Math.round(FL.sync + d(18) - FL.load) },
    "sync.mp4": { from: CLIP.syncIn, to: CLIP.syncIn + Math.round(FL.main + d(22) - FL.sync) },
    "emergency.mp4": { from: marks.takeover - (E.takeover - S.emergency[0]), to: marks.takeover - (E.takeover - S.emergency[0]) + E.clear - S.emergency[0] },
    "resolve.mp4": { from: resolveIn, to: resolveIn + S.emergency[1] - E.clear },
  };
};
