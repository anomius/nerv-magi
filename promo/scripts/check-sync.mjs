#!/usr/bin/env node
// Sync check: every scripted hit in src/timeline.ts must be an onset in the rendered music, on the
// frame. Reads public/audio/music.wav, or the float32 stereo WAV given as the first argument (e.g.
// the audio decoded back out of the final MP4), measures energy per video frame (1/60 s) and
// finds the strongest rise within ±6 frames of each cue. Fails if any is off by more than 2.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { T, bb } from "../src/timeline.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const buf = readFileSync(process.argv[2] ?? join(ROOT, "public/audio/music.wav"));
const data = buf.indexOf("data") + 8;
const SR = buf.readUInt32LE(24);
const samples = (buf.length - data) / 8;
const per = SR / T.fps;
const energy = [];
for (let f = 0; f * per < samples; f++) {
  let e = 0;
  for (let i = Math.floor(f * per); i < Math.min(samples, Math.floor((f + 1) * per)); i++) {
    const l = buf.readFloatLE(data + i * 8);
    const r = buf.readFloatLE(data + i * 8 + 4);
    e += l * l + r * r;
  }
  energy.push(Math.sqrt(e / per));
}
const cues = {
  "title smash (bar 3)": T.scenes.title[0],
  "smash out (bar 5)": T.title.out,
  "MAGI draw (bar 9)": T.magi.draw,
  "vote MELCHIOR (bar 10)": T.magi.votes[0],
  "vote BALTHASAR (bar 11)": T.magi.votes[1],
  "vote CASPER (bar 12)": T.magi.votes[2],
  "可決 3/3 (bar 13)": T.magi.carried,
  "reveal push (bar 15)": T.reveal.push,
  "LOAD wipe (bar 25)": T.flow.load,
  "SYNC roll (bar 26)": T.flow.sync,
  "MAIN dissolve (bar 27)": T.flow.main,
  "EMERGENCY (bar 28)": T.emergency.takeover,
  "NOMINAL (bar 31)": T.emergency.resolve,
  "NERV MAGI slam (bar 34.3)": T.outro.slam,
};
let bad = 0;
for (const [name, at] of Object.entries(cues)) {
  let best = 0;
  let off = 0;
  for (let d = -6; d <= 6; d++) {
    const f = at + d;
    const rise = (energy[f] ?? 0) - (energy[f - 1] ?? 0);
    if (rise > best) [best, off] = [rise, d];
  }
  const ok = Math.abs(off) <= 2;
  bad += ok ? 0 : 1;
  console.log(`${ok ? "ok  " : "OFF "} ${name.padEnd(28)} frame ${String(at).padStart(4)}  onset ${off >= 0 ? "+" : ""}${off} f  rise ${best.toFixed(3)}`);
}
const tail = energy.slice(T.outro.cut + 1, T.total).reduce((m, e) => Math.max(m, e), 0);
console.log(`after the cut (bar ${(T.outro.cut / T.bar + 1).toFixed(1)}): max rms ${tail.toExponential(1)}${process.argv[2] ? "" : " (music stem; the mix gates it to 0)"}`);
console.log(bad ? `sync: ${bad} cue(s) off the grid` : "sync: every cue lands within 2 frames");
process.exit(bad ? 1 : 0);
