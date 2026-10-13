/**
 * The recorded clips (public/footage*, scripts/record-footage.sh) and where the edit enters them, in
 * clip frames (60 fps). `clipNeeds(tl)`: the last frame of each clip a timeline plays, for the
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
  /** emergency.mp4: the frame where the real red takeover starts. */
  takeover: 84,
  /** resolve.mp4: the frame where MAIN wipes back in after CAUSE CLEARED. */
  wipeBack: 183,
} as const;

/** Clip frames each clip must have for this timeline (and where it is entered: a negative entry cannot be played). */
export const clipNeeds = (tl: Timeline) => {
  const { d } = tl;
  const { boot: B, reveal: R, flow: FL, emergency: E, scenes: S } = tl;
  const liveFrom = Math.round(R.wide - d(14));
  const resolveIn = CLIP.wipeBack - (E.resolve - E.clear);
  return {
    "boot.mp4": { from: CLIP.bootIn, to: CLIP.bootIn + S.powerOn[1] - Math.round(B.footage - d(2)) },
    "main.mp4": { from: CLIP.mainWideIn, to: Math.max(CLIP.mainWideIn + Math.round(R.callouts[0] + d(16)) - liveFrom, CLIP.mainFlowIn + S.flow[1] - FL.main) },
    "load.mp4": { from: CLIP.loadIn, to: CLIP.loadIn + Math.round(FL.sync + d(18) - FL.load) },
    "sync.mp4": { from: CLIP.syncIn, to: CLIP.syncIn + Math.round(FL.main + d(22) - FL.sync) },
    "emergency.mp4": { from: CLIP.takeover - (E.takeover - S.emergency[0]), to: CLIP.takeover - (E.takeover - S.emergency[0]) + E.clear - S.emergency[0] },
    "resolve.mp4": { from: resolveIn, to: resolveIn + S.emergency[1] - E.clear },
  };
};
