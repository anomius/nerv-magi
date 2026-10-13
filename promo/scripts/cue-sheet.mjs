#!/usr/bin/env node
// Cue sheet for a cut: every scene start and hit (src/cues.ts) in video time, frames, bars of the track
// and time in the track's audio file (video time + offsetSec), straight from the timeline. For a short
// cut (--edit), the rows are the cues inside its ranges, re-timed to the cut, plus a row per splice.
// Writes cues.txt (readable) and cues.csv (import as markers in an editor) into --out.
//   cue-sheet.mjs [tracks/x.json] [--edit teaser15|cut30] [--out dir] [--title text]
// Default: the edit as written (tracks/house-150.json) into out/byo-audio/.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { cueList } from "../src/cues.ts";
import { EDITS, editSegments } from "../src/edits.ts";
import { barBeat, clock, loadTrack } from "./lib/track.mjs";

/** Write cues.txt and cues.csv for `tl` (whole film, or `edit`) into `dir`; returns the row count. */
export const writeCueSheet = ({ tl, edit, dir, title }) => {
  const fps = tl.fps;
  const off = tl.offsetSec;
  const rows = []; // { at (frame in the cut), src (frame in the full edit), label }
  if (edit) {
    const segs = editSegments(tl, edit);
    for (const s of segs) {
      rows.push({ at: s.at, src: s.from, label: `CUT → bars ${s.bars[0]}-${s.bars[1] - 1} of the edit (track bar ${barBeat(tl, s.from)})` });
      for (const c of cueList(tl)) if (c.at >= s.from && c.at < s.to) rows.push({ at: s.at + c.at - s.from, src: c.at, label: c.label });
    }
    const end = segs.at(-1);
    rows.push({ at: end.at + end.to - end.from, src: end.to, label: "end" });
  } else {
    for (const c of cueList(tl)) rows.push({ at: c.at, src: c.at, label: c.label });
  }
  const total = rows.at(-1).at;
  const sections = Object.entries(tl.sections).map(([k, v]) => `${k} ${v}`).join(", ");
  const head = [
    `${title ?? "NERV MAGI promo"}: cue sheet${edit ? ` (${EDITS[edit].label}, spliced on bar lines)` : ""}`,
    `track map "${tl.name}": ${tl.bpm} BPM, ${tl.bpb}/4, ${fps} fps (${tl.beat.toFixed(3)} frames a beat, ${(tl.bar / fps).toFixed(3)} s a bar); ` +
      `bar 1 beat 1 is at ${off.toFixed(3)} s in the track's audio`,
    `sections start on bars: ${sections}`,
    `length ${clock(total / fps)} (${total} frames). Columns: video time, video frame, bar.beat of the track (+f: frames after the beat),`,
    `time in the track's audio file${edit ? " (where that moment is in the uncut track)" : ""}, cue.`,
    "",
  ].join("\n");
  const line = (r) =>
    `${clock(r.at / fps).padStart(8)}  ${String(r.at).padStart(5)}  ${barBeat(tl, r.src).padEnd(9)}  ${clock(r.src / fps + off).padStart(8)}  ${r.label}`;
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "cues.txt"), head + rows.map(line).join("\n") + "\n");
  writeFileSync(
    join(dir, "cues.csv"),
    "video_seconds,video_frame,track_bar,track_audio_seconds,label\n" +
      rows.map((r) => `${(r.at / fps).toFixed(3)},${r.at},${barBeat(tl, r.src)},${(r.src / fps + off).toFixed(3)},"${r.label.replace(/"/g, "'")}"`).join("\n") +
      "\n",
  );
  return rows.length;
};

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
  const opt = (name) => {
    const i = args.indexOf(name);
    return i >= 0 ? args.splice(i, 2)[1] : undefined;
  };
  const edit = opt("--edit");
  const out = opt("--out");
  const title = opt("--title");
  if (edit && !(edit in EDITS)) throw new Error(`unknown edit ${edit} (${Object.keys(EDITS).join(", ")})`);
  const { tl } = loadTrack(args[0]);
  const dir = out ?? "out/byo-audio";
  const n = writeCueSheet({ tl, edit, dir, title });
  console.log(`cues: ${dir}/cues.txt and cues.csv (${n} rows)`);
}
