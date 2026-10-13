/**
 * Short cuts of the edit: ranges of written bars, [from, to), played in order. They are cut on bar
 * lines of whatever track the timeline is cut to, so the picture and the spliced audio
 * (scripts/splice-audio.mjs) cut on the same frames. `flash`: a white smash-cut flash on the cut
 * into that range (where the edit has no hit of its own there).
 */
import type { Timeline } from "./timeline";

export type Edit = { label: string; segments: readonly { bars: readonly [number, number]; flash?: boolean }[] };

export const EDITS = {
  /** ≈15 s: title card → MAGI 可決 → into the dashboard → NERV MAGI slam (9 bars at 150 BPM: 14.4 s). */
  teaser15: {
    label: "Teaser15",
    segments: [{ bars: [3, 5] }, { bars: [13, 17] }, { bars: [34, 37], flash: true }],
  },
  /** ≈30 s: title → pattern → MAGI → reveal (push and wide) → EMERGENCY → outro (19 bars: 30.4 s). */
  cut30: {
    label: "Cut30",
    segments: [{ bars: [3, 5] }, { bars: [7, 17], flash: true }, { bars: [28, 32] }, { bars: [34, 37], flash: true }],
  },
} as const satisfies Record<string, Edit>;
export type EditName = keyof typeof EDITS;
export const EDIT_NAMES = Object.keys(EDITS) as EditName[];

/** The edit's segments in frames of `tl`: source [from, to) and where each lands in the cut (`at`). */
export const editSegments = (tl: Timeline, name: EditName) => {
  let at = 0;
  return EDITS[name].segments.map((s) => {
    const from = tl.bb(s.bars[0]);
    const to = tl.bb(s.bars[1]);
    const seg = { from, to, at, bars: s.bars, flash: "flash" in s && s.flash === true };
    at += to - from;
    return seg;
  });
};

/** Length of the cut in frames. */
export const editLength = (tl: Timeline, name: EditName) => editSegments(tl, name).reduce((n, s) => n + s.to - s.from, 0);
