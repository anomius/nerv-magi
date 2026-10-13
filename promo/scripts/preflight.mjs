#!/usr/bin/env node
// Preflight for a render: the track map is valid, and every recorded clip is long enough for its
// timeline (a slow track or a stretched section plays more of each clip). Exits 1 with the fix if not.
//   preflight.mjs [tracks/x.json] [--footage public/footage]
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { clipNeeds } from "../src/footage.ts";
import { PROMO, loadTrack } from "./lib/track.mjs";

const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
const fi = args.indexOf("--footage");
const dir = fi >= 0 ? args.splice(fi, 2)[1] : join(PROMO, "public/footage");
const { tl, map } = loadTrack(args[0]);
let bad = 0;
const fps = 60;
console.log(`preflight: "${map.name}" ${tl.bpm} BPM, ${(tl.total / tl.fps).toFixed(2)} s (${tl.total} frames), sections ${JSON.stringify(tl.sections)}`);
for (const [file, need] of Object.entries(clipNeeds(tl))) {
  const path = join(dir, file);
  if (!existsSync(path)) {
    console.log(`  MISSING ${file}`);
    bad++;
    continue;
  }
  const dur = Number(execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path], { encoding: "utf8" }));
  const have = Math.floor(dur * fps + 1e-6);
  const ok = need.from >= 0 && need.to <= have;
  bad += ok ? 0 : 1;
  console.log(`  ${ok ? "ok  " : "SHORT"} ${file.padEnd(14)} plays frames ${need.from}-${need.to} of ${have}`);
}
if (!existsSync(join(dir, "main-4k.png"))) {
  console.log("  MISSING main-4k.png");
  bad++;
}
if (bad) console.log(`preflight: ${bad} problem(s). Record longer clips: bash scripts/record-footage.sh (see its --seconds), or give the sections fewer bars.`);
process.exit(bad ? 1 : 0);
