#!/usr/bin/env node
// Sync check: every scripted hit (src/cues.ts) must be an onset in the rendered music, on the frame.
// Reads public/audio/music.wav, or the WAV given as the first argument (e.g. the audio decoded back
// out of a final MP4), measures energy per video frame (1/60 s) and finds the strongest rise within
// ±6 frames of each cue. Fails if any is off by more than 2. With `--track tracks/x.json` the cues
// come from that map's timeline (e.g. to check a licensed track against its map); with `--edit` they
// are the hits inside a short cut's ranges, at their place in the cut (and every splice is checked
// for a click: no sample jump bigger than the music's own).
//   check-sync.mjs [audio] [--track tracks/x.json] [--edit teaser15|cut30]
import { join } from "node:path";
import { cueList } from "../src/cues.ts";
import { editSegments } from "../src/edits.ts";
import { PROMO, barBeat, loadTrack } from "./lib/track.mjs";
import { SR, decode } from "./lib/wav.mjs";

const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
const ti = args.indexOf("--track");
const trackFile = ti >= 0 ? args.splice(ti, 2)[1] : undefined;
const ei = args.indexOf("--edit");
const edit = ei >= 0 ? args.splice(ei, 2)[1] : undefined;
const { tl } = loadTrack(trackFile);
const file = args[0] ?? join(PROMO, "public/audio/music.wav");
const { L, R } = decode(file);
const samples = L.length;
const per = SR / tl.fps;
const energy = [];
for (let f = 0; f * per < samples; f++) {
  let e = 0;
  for (let i = Math.floor(f * per); i < Math.min(samples, Math.floor((f + 1) * per)); i++) e += L[i] * L[i] + R[i] * R[i];
  energy.push(Math.sqrt(e / per));
}
let bad = 0;
const segs = edit ? editSegments(tl, edit) : [{ from: 0, to: tl.total + 1, at: 0 }];
const hits = segs.flatMap((s) => cueList(tl).filter((c) => c.hit && c.at >= s.from && c.at < s.to).map((c) => ({ ...c, src: c.at, at: s.at + c.at - s.from })));
for (const { at, src, hit } of hits) {
  let best = 0;
  let off = 0;
  for (let d = -6; d <= 6; d++) {
    const f = at + d;
    const rise = (energy[f] ?? 0) - (energy[f - 1] ?? 0);
    if (rise > best) [best, off] = [rise, d];
  }
  const ok = Math.abs(off) <= 2;
  bad += ok ? 0 : 1;
  console.log(`${ok ? "ok  " : "OFF "} ${`${hit} (bar ${barBeat(tl, src)})`.padEnd(30)} frame ${String(at).padStart(4)}  onset ${off >= 0 ? "+" : ""}${off} f  rise ${best.toFixed(3)}`);
}
// splices: the biggest sample-to-sample jump within ±2 ms of each, against the file's 99.9th percentile
if (edit) {
  const jumps = [];
  for (let i = 1; i < samples; i++) jumps.push(Math.max(Math.abs(L[i] - L[i - 1]), Math.abs(R[i] - R[i - 1])));
  const p999 = [...jumps].sort((a, b) => a - b)[Math.floor(jumps.length * 0.999)];
  for (const s of segs.slice(1)) {
    const c = s.at * per;
    const j = Math.max(...jumps.slice(c - 96, c + 96));
    const ok = j <= p999 * 1.5;
    bad += ok ? 0 : 1;
    console.log(`${ok ? "ok  " : "CLICK"} splice at frame ${s.at} (from bar ${barBeat(tl, s.from)}): max jump ${j.toFixed(4)} (file p99.9 ${p999.toFixed(4)})`);
  }
}
const cutAt = segs.at(-1).at + tl.outro.cut - segs.at(-1).from;
const tail = energy.slice(cutAt + 1, segs.at(-1).at + Math.min(tl.total, segs.at(-1).to) - segs.at(-1).from).reduce((m, e) => Math.max(m, e), 0);
console.log(`after the hard cut (frame ${cutAt}): max rms ${tail.toExponential(1)}${args[0] ? "" : " (music stem; the mix gates it to 0)"}`);
console.log(bad ? `sync: ${bad} cue(s) off the grid` : "sync: every cue lands within 2 frames");
process.exit(bad ? 1 : 0);
