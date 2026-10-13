#!/usr/bin/env node
// The promo's soundtrack, built in three stems and mixed:
//   1. music: scripts/music/compose.mjs writes the score as MIDI (the v2 "Eva medley": Bach's Cello
//      Suite No. 1 Prelude, an original battle march, Beethoven's "Ode to Joy"; all public domain or
//      original); FluidSynth renders it with FluidR3_GM (MIT).
//   2. SFX, synthesized from scratch in scripts/sfx.mjs (no samples): CRT power-on, typing ticks, hex
//      and MAGI vote blips, whooshes on every wipe and camera move, a riser, a two-tone klaxon.
//   3. mix: the music ducks under the SFX (sidechaincompress), the last half bar is cut to silence,
//      then two-pass loudnorm to -14 LUFS integrated, true peak <= -2 dBTP (headroom for AAC).
// Every cue comes from src/timeline.ts (the same grid the scenes use); the SFX synth is scripts/sfx.mjs.
// This is the medley's pipeline, for the edit as written (tracks/house-150.json). Output: public/audio/score.wav.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { T } from "../src/timeline.ts";
import { synthSfx } from "./sfx.mjs";
import { SR, writeWav } from "./lib/wav.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SOUNDFONT = process.env.SOUNDFONT ?? "/usr/share/soundfonts/FluidR3_GM.sf2";
const sec = (frame) => frame / T.fps;
const { L, R } = synthSfx(T);

// ---------------------------------------------------------------------------------------------
// stems → mix → master

const len = Math.round(sec(T.total) * SR);
const dir = join(ROOT, "public/audio");
mkdirSync(dir, { recursive: true });
let peak = 0;
for (let i = 0; i < len; i++) peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i]));
const sfx = join(dir, "sfx.wav");
writeWav(sfx, { L, R }, len, 0.5 / peak);

const sh = (cmd) => execFileSync("sh", ["-c", cmd], { encoding: "utf8" });
const ff = (args) => execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: ["ignore", "inherit", "inherit"] });

// music: compose the MIDI, render it
execFileSync(process.execPath, ["--disable-warning=MODULE_TYPELESS_PACKAGE_JSON", join(ROOT, "scripts/music/compose.mjs")], { stdio: "inherit" });
const midi = join(ROOT, "scripts/music/nerv-magi-score.mid");
const music = join(dir, "music.wav");
const hasFluid = (() => {
  try {
    execFileSync("sh", ["-c", "command -v fluidsynth"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
})();
if (!hasFluid || !existsSync(SOUNDFONT)) {
  console.error(`audio: fluidsynth or ${SOUNDFONT} missing (pacman: fluidsynth soundfont-fluid); cannot render the music`);
  process.exit(2);
}
const raw = join(dir, "music-raw.wav");
// reverb on (sends per instrument: CC91, choir and organ highest), chorus off, a mid-size hall
execFileSync(
  "fluidsynth",
  ["-ni", "-q", "-a", "file", "-F", raw, "-r", String(SR), "-g", "0.6", "-R", "1", "-C", "0",
   "-o", "synth.reverb.room-size=0.72", "-o", "synth.reverb.damp=0.35", "-o", "synth.reverb.width=0.9", "-o", "synth.reverb.level=0.75",
   SOUNDFONT, midi],
  { stdio: "inherit" },
);
// pad or trim the render to the film's length
ff(["-i", raw, "-af", `apad,atrim=0:${(len / SR).toFixed(6)},aresample=${SR}`, "-ac", "2", "-c:a", "pcm_f32le", music]);

// mix: music ducked by the SFX, then silence for the last half bar (the hard cut to つづく)
const cut = sec(T.outro.cut);
// afade is sample-accurate (volume's enable= switches per audio frame): 8 ms down to silence at the cut
const gate = `afade=t=out:st=${(cut - 0.008).toFixed(4)}:d=0.008`;
const mix = join(dir, "mix.wav");
ff([
  "-i", music, "-i", sfx, "-filter_complex",
  // the music bus: a gentle glue compressor and a little stereo width, then ducked under the SFX
  `[0:a]acompressor=threshold=-18dB:ratio=2:attack=20:release=250:makeup=1.5,extrastereo=m=1.25[bus];` +
  `[1:a]asplit=2[fx][key];[bus][key]sidechaincompress=threshold=0.15:ratio=2.5:attack=6:release=160:makeup=1[duck];` +
    `[duck][fx]amix=inputs=2:normalize=0:weights=1 0.9,${gate}[out]`,
  "-map", "[out]", "-c:a", "pcm_f32le", mix,
]);

const target = "I=-14:TP=-2:LRA=11";
const measure = (() => {
  const r = sh(`ffmpeg -hide_banner -nostats -i "${mix}" -af loudnorm=${target}:print_format=json -f null - 2>&1`);
  return JSON.parse(r.slice(r.lastIndexOf("{"), r.lastIndexOf("}") + 1));
})();
const final = join(dir, "score.wav");
ff([
  "-i", mix, "-af",
  `loudnorm=${target}:measured_I=${measure.input_i}:measured_TP=${measure.input_tp}:measured_LRA=${measure.input_lra}` +
    `:measured_thresh=${measure.input_thresh}:offset=${measure.target_offset}:linear=true,${gate}`,
  "-ar", String(SR), "-ac", "2", "-c:a", "pcm_s24le", final,
]);
const check = sh(`ffmpeg -hide_banner -nostats -i "${final}" -af ebur128=peak=true -f null - 2>&1 | tail -16`);
const I = check.match(/I:\s+(-?[\d.]+) LUFS/)?.[1];
const TP = check.match(/Peak:\s+(-?[\d.]+) dBFS/)?.[1];
console.log(`audio: ${final} (${(len / SR).toFixed(2)} s) integrated ${I} LUFS, true peak ${TP} dBTP`);
