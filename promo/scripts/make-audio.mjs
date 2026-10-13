#!/usr/bin/env node
// The promo's soundtrack, built in three stems and mixed:
//   1. music: scripts/music/compose.mjs writes the score as MIDI (the v2 "Eva medley": Bach's Cello
//      Suite No. 1 Prelude, an original battle march, Beethoven's "Ode to Joy"; all public domain or
//      original); FluidSynth renders it with FluidR3_GM (MIT).
//   2. SFX, synthesized here from scratch (no samples): CRT power-on, typing ticks, hex and MAGI vote
//      blips, whooshes on every wipe and camera move, a riser, a two-tone klaxon for EMERGENCY.
//   3. mix: the music ducks under the SFX (sidechaincompress), the last half bar is cut to silence,
//      then two-pass loudnorm to -14 LUFS integrated, true peak <= -2 dBTP (headroom for AAC).
// Every cue comes from src/timeline.ts (the same grid the scenes use). Output: public/audio/score.wav.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { T } from "../src/timeline.ts";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SOUNDFONT = process.env.SOUNDFONT ?? "/usr/share/soundfonts/FluidR3_GM.sf2";
const SR = 48000;
const N = Math.ceil((T.total / T.fps) * SR) + SR; // one second of tail room, trimmed later
const L = new Float32Array(N);
const R = new Float32Array(N);
const VERB = new Float32Array(N); // mono send into the reverb
const sec = (frame) => frame / T.fps;
const TAU = Math.PI * 2;

// deterministic noise
let seed = 0x2f6b9a1d;
const rnd = () => {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const noise = () => rnd() * 2 - 1;

/** Add a mono voice at time `t0` (s): fn(i, t) returns the sample; `pan` -1..1; `send` to reverb. */
const voice = (t0, dur, fn, { gain = 1, pan = 0, send = 0 } = {}) => {
  const s0 = Math.round(t0 * SR);
  const n = Math.round(dur * SR);
  const gl = gain * Math.cos(((pan + 1) * Math.PI) / 4);
  const gr = gain * Math.sin(((pan + 1) * Math.PI) / 4);
  for (let i = 0; i < n; i++) {
    const k = s0 + i;
    if (k < 0 || k >= N) continue;
    const v = fn(i, i / SR);
    L[k] += v * gl;
    R[k] += v * gr;
    if (send) VERB[k] += v * gain * send;
  }
};

/** Phase-accumulating oscillator with a frequency function of time. */
const osc = (shape) => {
  let ph = 0;
  return (freq) => {
    ph += freq / SR;
    ph -= Math.floor(ph);
    switch (shape) {
      case "sine":
        return Math.sin(ph * TAU);
      case "saw":
        return 2 * ph - 1;
      case "square":
        return ph < 0.5 ? 1 : -1;
      case "tri":
        return 1 - 4 * Math.abs(ph - 0.5);
    }
  };
};
const lowpass = (cut) => {
  let y = 0;
  return (x, c = cut) => {
    const a = 1 - Math.exp((-TAU * c) / SR);
    y += a * (x - y);
    return y;
  };
};
const highpass = (cut) => {
  const lp = lowpass(cut);
  return (x, c) => x - lp(x, c);
};
/** RBJ band-pass (constant peak gain), cutoff may change per sample. */
const bandpass = (q = 4) => {
  let x1 = 0, x2 = 0, y1 = 0, y2 = 0;
  return (x, f) => {
    const w = (TAU * Math.min(f, SR * 0.45)) / SR;
    const al = Math.sin(w) / (2 * q);
    const a0 = 1 + al;
    const y = ((al * x - al * x2) - -2 * Math.cos(w) * y1 - (1 - al) * y2) / a0;
    x2 = x1; x1 = x; y2 = y1; y1 = y;
    return y;
  };
};
const env = (t, a, d) => (t < a ? t / a : Math.exp(-(t - a) / d));

// ---------------------------------------------------------------------------------------------
// instruments

const thud = (at, { gain = 1, f0 = 150, f1 = 34, len = 1.4 } = {}) => {
  const o = osc("sine");
  const sub = osc("sine");
  const lp = lowpass(900);
  voice(sec(at), len, (i, t) => {
    const f = f1 + (f0 - f1) * Math.exp(-t / 0.07);
    const body = o(f) * env(t, 0.002, 0.42);
    const s = sub(f * 0.5) * env(t, 0.004, 0.7) * 0.6;
    const crack = lp(noise()) * env(t, 0.001, 0.05) * 1.6;
    return Math.tanh((body + s + crack) * 1.6) * 0.9;
  }, { gain, send: 0.25 });
};


const tick = (at, { gain = 0.16, f = 4200, pan = 0 } = {}) => {
  const bp = bandpass(3);
  const ff = f * (0.8 + rnd() * 0.4);
  voice(sec(at), 0.03, (i, t) => bp(noise(), ff) * env(t, 0.0003, 0.006) * 3, { gain, pan });
};

const blip = (at, freqs, { gain = 0.22, step = 0.045, len = 0.09, shape = "square", pan = 0, send = 0.35 } = {}) => {
  freqs.forEach((fq, k) => {
    const o = osc(shape);
    const lp = lowpass(5200);
    voice(sec(at) + k * step, len, (i, t) => lp(o(fq)) * env(t, 0.002, len / 3.5), { gain, pan, send });
  });
};

const whoosh = (at, dur, { gain = 0.35, f0 = 300, f1 = 4000, pan = 0 } = {}) => {
  const bp = bandpass(1.6);
  voice(sec(at), dur, (i, t) => {
    const p = t / dur;
    const shape = Math.sin(Math.PI * Math.min(1, p * 1.15)) ** 2;
    return bp(noise(), f0 * Math.pow(f1 / f0, p)) * shape * 1.6;
  }, { gain, pan, send: 0.2 });
};

const riser = (from, to) => {
  const dur = sec(to - from);
  const bp = bandpass(2.2);
  const o = osc("saw");
  const lp = lowpass(800);
  voice(sec(from), dur, (i, t) => {
    const p = t / dur;
    const n = bp(noise(), 200 * Math.pow(40, p)) * p * p * 1.8;
    const tone = lp(o(70 * Math.pow(9, p)), 300 + 5000 * p) * p * p * 0.5;
    return n + tone;
  }, { gain: 0.5, send: 0.15 });
};




const crtOn = (at) => {
  const o = osc("sine");
  const whine = osc("sine");
  const hp = highpass(3000);
  voice(sec(at), 1.6, (i, t) => {
    const thunk = o(58 + 30 * Math.exp(-t / 0.05)) * env(t, 0.002, 0.18) * 0.9;
    const w = whine(9200) * env(t, 0.05, 0.6) * 0.05;
    const stat = hp(noise()) * env(t, 0.001, 0.12) * 0.5;
    return thunk + w + stat;
  }, { gain: 0.7 });
};

/** Schroeder reverb on the send bus. */
const reverb = () => {
  const combs = [1557, 1617, 1491, 1422, 1277, 1356].map((d) => ({ buf: new Float32Array(d), i: 0, lp: 0 }));
  const aps = [225, 556, 441, 341].map((d) => ({ buf: new Float32Array(d), i: 0 }));
  for (let n = 0; n < N; n++) {
    const x = VERB[n];
    let y = 0;
    for (const c of combs) {
      const out = c.buf[c.i];
      c.lp = out * 0.6 + c.lp * 0.4;
      c.buf[c.i] = x + c.lp * 0.82;
      c.i = (c.i + 1) % c.buf.length;
      y += out;
    }
    y /= combs.length;
    for (const a of aps) {
      const b = a.buf[a.i];
      const o = -y + b;
      a.buf[a.i] = y + b * 0.5;
      a.i = (a.i + 1) % a.buf.length;
      y = o;
    }
    L[n] += y * 0.55;
    R[n] += y * 0.5;
  }
};

// ---------------------------------------------------------------------------------------------
// SFX cues (the orchestra plays the hits; these are the screen's own sounds)

/** Two-tone klaxon: alternating D5 / Ab5 (the tritone of the EMERGENCY brass cluster), one per beat. */
const klaxonTone = (at, freq) => {
  const o1 = osc("saw");
  const o2 = osc("square");
  const bp = bandpass(1.4);
  const len = sec(T.beat) * 0.9;
  voice(sec(at), len, (i, t) => {
    const a = Math.min(1, t / 0.012) * (t > len - 0.04 ? (len - t) / 0.04 : 1);
    return Math.tanh(bp(o1(freq) * 0.6 + o2(freq) * 0.4, freq * 2) * 2.5) * a;
  }, { gain: 0.32, send: 0.25 });
};

// 1-2 power-on
crtOn(T.boot.crt);
for (const line of T.boot.lines) {
  for (let c = 0; c < line.text.length; c++) if (line.text[c] !== " ") tick(line.at + c * line.fpc, { gain: 0.09, pan: -0.2 + rnd() * 0.4 });
}
blip(T.boot.linked, [1175, 1760], { gain: 0.2, step: 0.06 });
whoosh(T.boot.footage - 6, 0.35, { gain: 0.3, f0: 600, f1: 5000 });
whoosh(T.boot.kanji, sec(T.scenes.title[0] - 8 - T.boot.kanji), { gain: 0.28, f0: 200, f1: 3000 });
thud(T.boot.kanji, { gain: 0.5 });

// 3-4 title card: the orchestra plays the smash in and out; one tick for the English line
tick(T.title.sub, { gain: 0.22, f: 2500 });

// 5-8 pattern: the sweep, a blip per hex (8ths), the detection alarm, the dive
whoosh(T.pattern.sweep, sec(T.pattern.hexes + 10 * T.pattern.hexStep - T.pattern.sweep), { gain: 0.22, f0: 200, f1: 2400 });
for (let k = 0; k < 10; k++) blip(T.pattern.hexes + k * T.pattern.hexStep, [700 + k * 70], { gain: 0.11, len: 0.07, shape: "tri", pan: -0.8 + (k / 9) * 1.6 });
for (let k = 0; k < 3; k++) blip(T.pattern.header + k * (T.beat / 3), [880, 1320], { gain: 0.14, step: 0.02, len: 0.07 });
whoosh(T.pattern.zoom, sec(T.scenes.pattern[1] - T.pattern.zoom), { gain: 0.4, f0: 150, f1: 6000 });

// 9-14 MAGI: header typing, deliberation pulses on the beat, a vote chime per unit, the riser
for (let k = 0; k < 5; k++) for (let c = 0; c < 16; c += 2) tick(T.magi.draw + 8 + k * 9 + c, { gain: 0.06, f: 5000 });
for (let fr = T.magi.deliberate; fr < T.magi.votes[2]; fr += T.beat / 2) blip(fr, [((fr - T.magi.deliberate) / (T.beat / 2)) % 2 ? 415 : 440], { gain: 0.08, len: 0.05, shape: "sine", send: 0.2 });
const votePan = [0.7, 0, -0.7]; // MELCHIOR right, BALTHASAR centre, CASPER left
T.magi.votes.forEach((v, k) => blip(v, [1046, 1318, 1568].map((f) => f * 2 ** ((2 * k) / 12)), { gain: 0.18, step: 0.035, len: 0.1, pan: votePan[k] }));
riser(T.magi.riser[0], T.magi.riser[1]);

// 15-24 reveal: the push, a whoosh and a ping per callout
whoosh(T.reveal.push, sec(T.reveal.wide - T.reveal.push), { gain: 0.3, f0: 5000, f1: 300 });
tick(T.reveal.wide + 4, { gain: 0.22, f: 2200 });
T.reveal.callouts.forEach((c, k) => {
  whoosh(c - 2, 0.42, { gain: 0.26, f0: 500, f1: 3500, pan: k % 2 ? -0.5 : 0.5 });
  blip(c + 38, [1568, 2093], { gain: 0.1, step: 0.03, len: 0.06, pan: k % 2 ? -0.5 : 0.5 });
});

// 25-27 view flow: wipe, roll, hex dissolve; the alarm creeping in
whoosh(T.flow.load, 0.42, { gain: 0.34, f0: 300, f1: 6000, pan: -0.6 });
whoosh(T.flow.sync - 2, 0.4, { gain: 0.3, f0: 4000, f1: 200 });
for (let k = 0; k < 12; k++) tick(T.flow.main + k * 2, { gain: 0.12, f: 3000 + k * 200, pan: -0.8 + k * 0.14 });
for (let fr = T.flow.alarm; fr < T.scenes.emergency[0]; fr += 6) blip(fr, [1480], { gain: 0.09, len: 0.04 });

// 28-30 EMERGENCY: the klaxon on every beat until the cause clears
for (let fr = T.emergency.takeover, k = 0; fr < T.emergency.clear; fr += T.beat, k++) klaxonTone(fr, k % 2 ? 830.6 : 587.3);
thud(T.emergency.takeover, { gain: 0.4, f0: 140, f1: 30, len: 1.4 });
whoosh(T.emergency.card - 2, 0.45, { gain: 0.26, f0: 400, f1: 3000, pan: -0.4 });
blip(T.emergency.clear, [660, 440], { gain: 0.16, step: 0.08, len: 0.12 });
whoosh(T.emergency.line, sec(T.scenes.emergency[1] - T.emergency.line), { gain: 0.26, f0: 400, f1: 8000 });

// 32-36 outro: ticks for the lines and the URL (the final chord carries the title)
T.outro.lines.slice(0, 2).forEach((l) => tick(l, { gain: 0.2, f: 2600 }));
{
  const url = "github.com/anomius/nerv-magi";
  for (let c = 0; c < url.length; c++) tick(T.outro.lines[2] + c * 2, { gain: 0.1 });
}

reverb();

// ---------------------------------------------------------------------------------------------
// stems → mix → master

const len = Math.round(sec(T.total) * SR);
const dir = join(ROOT, "public/audio");
mkdirSync(dir, { recursive: true });
const wav = (path, l, r, scale = 1) => {
  const out = Buffer.alloc(44 + len * 8);
  out.write("RIFF", 0);
  out.writeUInt32LE(36 + len * 8, 4);
  out.write("WAVE", 8);
  out.write("fmt ", 12);
  out.writeUInt32LE(16, 16);
  out.writeUInt16LE(3, 20); // IEEE float
  out.writeUInt16LE(2, 22);
  out.writeUInt32LE(SR, 24);
  out.writeUInt32LE(SR * 8, 28);
  out.writeUInt16LE(8, 32);
  out.writeUInt16LE(32, 34);
  out.write("data", 36);
  out.writeUInt32LE(len * 8, 40);
  for (let i = 0; i < len; i++) {
    out.writeFloatLE(l[i] * scale, 44 + i * 8);
    out.writeFloatLE(r[i] * scale, 48 + i * 8);
  }
  writeFileSync(path, out);
};
let peak = 0;
for (let i = 0; i < len; i++) peak = Math.max(peak, Math.abs(L[i]), Math.abs(R[i]));
const sfx = join(dir, "sfx.wav");
wav(sfx, L, R, 0.5 / peak);

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
