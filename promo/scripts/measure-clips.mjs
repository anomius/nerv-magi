#!/usr/bin/env node
// Measure the scripted moments in a footage folder's clips and write <folder>/clips.json, which the
// renders pass to the edit (src/footage.ts, ClipMarks): the frame of emergency.mp4 where the red
// takeover starts (chroma jumps) and the frame of resolve.mp4 where MAIN wipes back in (one after the
// first frame that changes after the CAUSE CLEARED hold). Run by record-footage.sh after recording.
//   measure-clips.mjs [public/footage-4k]
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv.slice(2).filter((a) => a !== "--")[0] ?? "public/footage";
/** Mean luma and red-difference chroma of every frame (downscaled), from ffmpeg's signalstats. */
const stats = (file) => {
  const out = execFileSync("sh", ["-c", `ffmpeg -hide_banner -nostats -i "${file}" -an -vf "scale=96:-2,signalstats,metadata=print:file=-" -f null - 2>/dev/null`], {
    encoding: "utf8",
    maxBuffer: 1 << 28,
  });
  const y = [...out.matchAll(/YAVG=([\d.]+)/g)].map((m) => Number(m[1]));
  const v = [...out.matchAll(/VAVG=([\d.]+)/g)].map((m) => Number(m[1]));
  return { y, v };
};
const em = stats(join(dir, "emergency.mp4"));
const takeover = em.v.findIndex((v, i) => i > 0 && v - em.v[i - 1] > 20);
const rs = stats(join(dir, "resolve.mp4"));
const first = rs.y.findIndex((y, i) => i > 0 && Math.abs(y - rs.y[i - 1]) > 3);
if (takeover < 0 || first < 0) {
  console.error(`measure-clips: ${dir}: no ${takeover < 0 ? "takeover in emergency.mp4" : "wipe in resolve.mp4"}; re-record that clip`);
  process.exit(1);
}
// the clip must stay red (the BLOCKED card) until the edit leaves it: no second state change
const settle = em.v.findIndex((v, i) => i > takeover + 40 && Math.abs(v - em.v[i - 1]) > 4);
const marks = { takeover, wipeBack: first + 1 };
writeFileSync(join(dir, "clips.json"), JSON.stringify(marks) + "\n");
console.log(`measure-clips: ${dir}: takeover at frame ${takeover}, MAIN wipes back at ${marks.wipeBack}` + (settle >= 0 ? ` (emergency changes again at ${settle})` : " (emergency holds to the end)"));
