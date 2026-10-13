#!/usr/bin/env node
// ESTIMATE a track's tempo and first downbeat, to start a track map. Confirm it by ear: render the map
// (pnpm run render:track) and play <name>-guide.mp4 / -preview.mp4, whose click sits on the map's grid.
//   detect-tempo.mjs <audio> [--name x] [--out tracks/x.json] [--min 70 --max 180]
// How: ffmpeg decodes to mono 11025 Hz; an onset envelope (log-magnitude spectral flux, 11.6 ms hop);
// its autocorrelation, weighted toward 120 BPM, picks the beat period, refined to 0.01 BPM by lining a
// beat comb up with the onsets over the whole file; the beat phase maximizes the onsets on the comb; the
// downbeat is the bar position where the harmony changes most (chroma novelty) and the low end (kick,
// bass, < 120 Hz) hits hardest; offsetSec is the first downbeat once the beat has started. Steady tempo only: a track that speeds up, swings hard or
// opens without drums can fool it (a half/double tempo or a downbeat one beat off is the usual error).
import { execFileSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";

const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
const opt = (name, dflt) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : dflt;
};
const name = opt("--name");
const out = opt("--out");
const minBpm = Number(opt("--min", 70));
const maxBpm = Number(opt("--max", 180));
const file = args[0];
if (!file || !existsSync(file)) {
  console.error("usage: detect-tempo.mjs <audio> [--name x] [--out tracks/x.json] [--min 70 --max 180]");
  process.exit(2);
}

const SR = 11025;
const HOP = 128;
const N = 1024;
const raw = execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-i", file, "-f", "f32le", "-ac", "1", "-ar", String(SR), "-"], { maxBuffer: 1 << 30 });
const x = new Float32Array(raw.buffer, raw.byteOffset, raw.length / 4);
const dt = HOP / SR; // seconds per envelope frame

// --- onset envelopes: full band and low band (< 180 Hz), log-magnitude spectral flux
const fft = (re, im) => {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j], re[i]];
      [im[i], im[j]] = [im[j], im[i]];
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const a = (-2 * Math.PI) / len;
    for (let i = 0; i < n; i += len) {
      for (let k = 0; k < len / 2; k++) {
        const c = Math.cos(a * k);
        const s = Math.sin(a * k);
        const tr = re[i + k + len / 2] * c - im[i + k + len / 2] * s;
        const ti = re[i + k + len / 2] * s + im[i + k + len / 2] * c;
        re[i + k + len / 2] = re[i + k] - tr;
        im[i + k + len / 2] = im[i + k] - ti;
        re[i + k] += tr;
        im[i + k] += ti;
      }
    }
  }
};
const win = Float32Array.from({ length: N }, (_, i) => 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / N));
const frames = Math.max(0, Math.floor((x.length - N) / HOP));
const lowBin = Math.round((120 * N) / SR);
const env = new Float32Array(frames);
const low = new Float32Array(frames);
// pitch class of each bin 100-2000 Hz (chroma: what chord is playing)
const pcOf = Array.from({ length: N / 2 }, (_, k) => {
  const f = (k * SR) / N;
  return f < 100 || f > 2000 ? -1 : ((Math.round(12 * Math.log2(f / 440)) % 12) + 12) % 12;
});
const chroma = Array.from({ length: frames }, () => new Float32Array(12));
let prev = new Float32Array(N / 2);
for (let f = 0; f < frames; f++) {
  const re = new Float64Array(N);
  const im = new Float64Array(N);
  for (let i = 0; i < N; i++) re[i] = x[f * HOP + i] * win[i];
  fft(re, im);
  const mag = new Float32Array(N / 2);
  let e = 0;
  let l = 0;
  for (let k = 1; k < N / 2; k++) {
    const a = Math.hypot(re[k], im[k]);
    mag[k] = Math.log1p(100 * a);
    const d = Math.max(0, mag[k] - prev[k]);
    e += d;
    if (k <= lowBin) l += d;
    if (pcOf[k] >= 0) chroma[f][pcOf[k]] += a;
  }
  env[f] = e;
  low[f] = l;
  prev = mag;
}
// remove the slow trend (0.5 s moving average), keep the rises
const detrend = (v) => {
  const w = Math.round(0.25 / dt);
  const out = new Float32Array(v.length);
  let sum = 0;
  const q = [];
  for (let i = 0; i < v.length + w; i++) {
    if (i < v.length) {
      sum += v[i];
      q.push(v[i]);
    }
    if (q.length > 2 * w + 1) sum -= q.shift();
    const c = i - w;
    if (c >= 0) out[c] = Math.max(0, v[c] - sum / q.length);
  }
  return out;
};
const E = detrend(env);
const LOW = detrend(low);
// a frame's log flux peaks when an onset is ~80 % into its window, ~75 ms after the frame starts (calibrated on
// clicks and on drums with known onsets: within ±5 ms after this)
const LAG = 0.075;
const at = (v, t) => {
  // envelope at time t (s), linearly interpolated
  const p = (t - LAG) / dt;
  const i = Math.floor(p);
  if (i < 0 || i + 1 >= v.length) return 0;
  return v[i] + (v[i + 1] - v[i]) * (p - i);
};

// --- tempo: autocorrelation over the BPM range, weighted by a log-normal prior around 120 BPM
const lagOf = (bpm) => 60 / bpm / dt;
let best = { score: -1, bpm: 0 };
const acs = [];
for (let lag = Math.floor(lagOf(maxBpm)); lag <= Math.ceil(lagOf(minBpm)); lag++) {
  let s = 0;
  for (let i = 0; i + lag < E.length; i++) s += E[i] * E[i + lag];
  const bpm = 60 / (lag * dt);
  const w = Math.exp(-0.5 * (Math.log2(bpm / 120) / 1) ** 2);
  acs.push(s);
  if (s * w > best.score) best = { score: s * w, bpm };
}
const acMean = acs.reduce((a, b) => a + b, 0) / acs.length;
// refine: the BPM (±2 %, 0.01 steps) and phase whose beat comb collects the most onset energy
const comb = (bpm, phase, v = E) => {
  const p = 60 / bpm;
  let s = 0;
  for (let t = phase; t < E.length * dt; t += p) s += at(v, t);
  return s;
};
const phaseFor = (bpm) => {
  const p = 60 / bpm;
  let b = { s: -1, ph: 0 };
  for (let ph = 0; ph < p; ph += dt / 4) {
    const s = comb(bpm, ph);
    if (s > b.s) b = { s, ph };
  }
  return b;
};
let fine = { bpm: best.bpm, s: -1, ph: 0 };
for (let bpm = best.bpm * 0.98; bpm <= best.bpm * 1.02; bpm += 0.01) {
  const r = phaseFor(bpm);
  if (r.s > fine.s) fine = { bpm, ...r };
}
let bpm = Math.round(fine.bpm * 100) / 100;
// octave check: when the off-beats carry nearly as much onset energy as the beats, the beat is twice as fast
const offRatio = (() => {
  const p = 60 / bpm;
  const ph = phaseFor(bpm).ph;
  return comb(bpm, ph + p / 2) / Math.max(1e-9, comb(bpm, ph));
})();
const doubled = offRatio >= 0.6 && bpm * 2 <= maxBpm;
if (doubled) bpm = Math.round(bpm * 200) / 100;
const spb = 60 / bpm;
const phase = phaseFor(bpm).ph;

// --- downbeat: the bar position (of 4) where the harmony changes most and the low end hits hardest
const bpb = 4;
const chromaMean = (t0, t1) => {
  const c = new Float32Array(12);
  for (let f = Math.max(0, Math.round((t0 - LAG) / dt)); f < Math.min(frames, Math.round((t1 - LAG) / dt)); f++) for (let i = 0; i < 12; i++) c[i] += chroma[f][i];
  return c;
};
const cosDist = (a, b) => {
  let ab = 0, aa = 0, bb = 0;
  for (let i = 0; i < 12; i++) [ab, aa, bb] = [ab + a[i] * b[i], aa + a[i] * a[i], bb + b[i] * b[i]];
  return aa && bb ? 1 - ab / Math.sqrt(aa * bb) : 0;
};
const lowScore = [0, 0, 0, 0];
const harmScore = [0, 0, 0, 0];
for (let k = 0; phase + k * spb < E.length * dt; k++) {
  const t = phase + k * spb;
  lowScore[k % bpb] += at(LOW, t);
  harmScore[k % bpb] += cosDist(chromaMean(t - spb, t), chromaMean(t, t + spb));
}
const norm = (v) => {
  const s = v.reduce((a, b) => a + b, 0) || 1;
  return v.map((x) => x / s);
};
const posScore = norm(lowScore).map((v, i) => 0.5 * v + 0.5 * norm(harmScore)[i]);
const down = posScore.indexOf(Math.max(...posScore));
// first downbeat after the beat has started: the first beat whose onset reaches 30 % of the median beat onset
const beatOn = [];
for (let k = 0; phase + k * spb < E.length * dt; k++) beatOn.push(at(E, phase + k * spb));
const median = [...beatOn].sort((a, b) => a - b)[Math.floor(beatOn.length / 2)];
let first = beatOn.findIndex((v) => v >= 0.3 * median);
first = first < 0 ? 0 : first;
while ((first - down) % bpb !== 0) first++;
const offset = phase + first * spb;

const conf = best.score > 0 ? Math.min(1, (Math.max(...acs) / acMean - 1) / 3) : 0;
const sorted = [...posScore].sort((a, b) => b - a);
const downConf = sorted[0] > 0 ? 1 - sorted[1] / sorted[0] : 0;
const map = { name: name ?? "my-track", bpm, beatsPerBar: 4, offsetSec: Math.round(offset * 1000) / 1000 };
console.log(`detect-tempo (ESTIMATE, confirm by ear): ${file}`);
console.log(`  tempo     ${bpm} BPM (beat ${spb.toFixed(4)} s; autocorrelation peak ${conf > 0.5 ? "clear" : conf > 0.2 ? "fair" : "weak"}, ${(conf * 100).toFixed(0)} %; off-beat/beat onsets ${offRatio.toFixed(2)}${doubled ? ", so doubled" : ""})`);
console.log(`  beat grid first beat at ${phase.toFixed(3)} s; downbeat = beat ${down + 1} of 4 on that grid (${downConf > 0.15 ? "clear" : "uncertain"}, margin ${(downConf * 100).toFixed(0)} %)`);
console.log(`  bar 1     ${offset.toFixed(3)} s (first downbeat once the beat has started)`);
console.log(`  check: half or double tempo (${(bpm / 2).toFixed(2)} / ${(bpm * 2).toFixed(2)}) and a downbeat one beat off are the usual misses`);
console.log(JSON.stringify(map, null, 2));
if (out) {
  if (existsSync(out)) throw new Error(`${out} exists; not overwriting`);
  writeFileSync(out, JSON.stringify({ ...map, audio: `byo-audio/tracks/${file.split("/").pop()}` }, null, 2) + "\n");
  console.log(`wrote ${out} (add "sections" if the track's phrases need them)`);
}
