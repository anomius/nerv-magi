#!/usr/bin/env node
// Render the music demos, one at a time: compose (node <name>.mjs → <name>.mid + out/<name>.json), synthesize
// with FluidSynth + FluidR3_GM (reverb on, chorus off, 48 kHz float, so nothing clips before the master), then
// master with ffmpeg and check the result by analysis.
//   master: high-pass 28 Hz → the demo's EQ → a gentle 1.5-1.8:1 compressor → light stereo widening → a
//           4x-oversampled peak limiter at -2 dBFS (keeps the peak-to-loudness ratio low enough that the loudness
//           gain stays linear) → two-pass loudnorm to -14 LUFS integrated, true peak <= -1 dBTP → 48 kHz 24-bit
//           WAV, and a 320k MP3.
//   checks: EBU R128 (I, LRA, true peak) on the WAV and the MP3, astats, a spectrogram and a waveform PNG, and
//           an onset check: the attack of every grid-exact hit the demo lists, and the phase of the beat grid.
// Outputs (gitignored): out/<name>.wav, .mp3, .spectrum.png, .wave.png, .check.json.
// usage: node promo/music-demos/render.mjs [name ...]             (no names: every demo)
//        node promo/music-demos/render.mjs --stems <name>     (balance check: each part's loudness per section)
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const DIR = dirname(fileURLToPath(import.meta.url));
const OUT = join(DIR, "out");
const SF2 = "/usr/share/soundfonts/FluidR3_GM.sf2";
const SR = 48000;
const DEMOS = ["battle-march", "threat-alert", "night-rain-jazz", "cold-light"];
const TARGET = { I: -14, TP: -1 };

/** Run ffmpeg; returns stderr (where the measuring filters print). Throws on failure. */
const ff = (args) => {
  const r = spawnSync("ffmpeg", ["-hide_banner", "-nostats", "-y", ...args], { encoding: "utf8", maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`ffmpeg ${args.join(" ")}\n${r.stderr}`);
  return r.stderr;
};
const ebur = (file) => {
  const s = ff(["-i", file, "-af", "ebur128=peak=true:framelog=quiet", "-f", "null", "-"]);
  const sum = s.slice(s.lastIndexOf("Summary:"));
  const num = (re) => Number(re.exec(sum)?.[1]);
  return { I: num(/I:\s+(-?[\d.]+) LUFS/), LRA: num(/LRA:\s+([\d.]+) LU/), TP: num(/Peak:\s+(-?[\d.]+|-inf) dBFS/) };
};
const loudnormJson = (file, filter) => {
  const s = ff(["-i", file, "-af", filter, "-f", "null", "-"]);
  return JSON.parse(s.slice(s.lastIndexOf("{"), s.lastIndexOf("}") + 1));
};
const astats = (file) => {
  const s = ff(["-i", file, "-af", "astats=measure_perchannel=none", "-f", "null", "-"]);
  const pick = (k) => Number(new RegExp(`${k}:\\s+(-?[\\d.e+-]+|-?inf)`).exec(s.slice(s.indexOf("Overall")))?.[1]);
  const peakDb = pick("Peak level dB"), rmsDb = pick("RMS level dB");
  return { peakDb, rmsDb, crestDb: +(peakDb - rmsDb).toFixed(1), dc: pick("DC offset"), flat: pick("Flat factor"), peakCount: pick("Peak count") };
};
/**
 * Mono float samples, high-passed (default 1.5 kHz): attacks carry their bite up there (stick, mallet, hammer,
 * tongue), and a 2 ms window doesn't ripple on the waveform the way it does on 100-200 Hz brass and basses.
 */
const HIGH = "highpass=f=1500,highpass=f=1500";
const LOW = "lowpass=f=120,lowpass=f=120"; // the bass-drum / timpani / bass band, for a second opinion
const mono = (file, filter = HIGH) => {
  const raw = execFileSync("ffmpeg", ["-v", "error", "-i", file, "-af", filter, "-ac", "1", "-ar", String(SR), "-f", "f32le", "-"], { maxBuffer: 1 << 30 });
  return new Float32Array(raw.buffer, raw.byteOffset, raw.length / 4);
};

/**
 * Onset strength: the rise of the short-window log-RMS envelope over the previous few milliseconds. Its peak near
 * an expected hit marks the attack. High band: 2 ms window, 0.5 ms hop; low band: 10 ms (a bass period).
 */
const onsetCurve = (x, { winMs = 2, backMs = 5, hopMs = 0.5 } = {}) => {
  const hop = Math.round((SR * hopMs) / 1000), win = Math.round((SR * winMs) / 1000), back = Math.round(backMs / hopMs);
  const cum = new Float64Array(x.length + 1);
  for (let i = 0; i < x.length; i++) cum[i + 1] = cum[i] + x[i] * x[i];
  const frames = Math.floor((x.length - win) / hop);
  const L = new Float32Array(frames);
  for (let f = 0; f < frames; f++) L[f] = 10 * Math.log10((cum[f * hop + win] - cum[f * hop]) / win + 1e-12);
  const o = new Float32Array(frames);
  for (let f = back; f < frames; f++) o[f] = Math.max(0, L[f] - L[f - back]) * (L[f] > -60 ? 1 : 0);
  return { o, msPerFrame: hopMs, offsetMs: winMs / 2 };
};
const LOW_CURVE = { winMs: 10, backMs: 10, hopMs: 1 };
// What a grid-exact hit reads as in each band, measured on isolated FluidR3 hits (snare, bass drum, cymbals,
// timpani, piano, brushes, bass): +3.5..7.5 ms high, +4..11 ms low. A hit's deviation from the grid is taken in
// whichever band shows it more clearly (a roll into a downbeat masks the highs, a drone masks the lows).
const CAL = { high: 5, low: 8 };
const frameOf = (oc, sec) => Math.round((sec * 1000 - oc.offsetMs) / oc.msPerFrame);
/**
 * For each expected hit (s): the attack nearest the grid, as an offset in ms (+ = late). In a dense mix a cymbal
 * or the hall can make a later bump as big as the attack itself, so take the earliest local peak in [-15, +60] ms
 * that reaches 60% of the window's maximum (from -15: flam grace notes and roll strokes lead into downbeats).
 */
const hitOffsets = (oc, secs) =>
  secs.map((t) => {
    const c = frameOf(oc, t);
    const lo = Math.max(1, c - Math.round(15 / oc.msPerFrame)), hi = Math.min(oc.o.length - 2, c + Math.round(60 / oc.msPerFrame));
    let max = 0;
    for (let f = lo; f <= hi; f++) max = Math.max(max, oc.o[f]);
    for (let f = lo; f <= hi; f++)
      if (oc.o[f] >= 0.6 * max && oc.o[f] >= oc.o[f - 1] && oc.o[f] >= oc.o[f + 1]) return +((f - c) * oc.msPerFrame).toFixed(1);
    return null;
  });
/** Phase (ms) of the grid with step `stepSec` that lines up best with the attacks, and its contrast. */
const gridPhase = (oc, stepSec, fromSec, toSec) => {
  const scores = [];
  for (let ms = -60; ms <= 60; ms += 1) {
    let s = 0, k = 0;
    for (let t = fromSec; t < toSec; t += stepSec, k++) {
      const c = frameOf(oc, t + ms / 1000);
      let m = 0;
      for (let f = c - 4; f <= c + 4; f++) if (f > 0 && f < oc.o.length) m = Math.max(m, oc.o[f]);
      s += m;
    }
    scores.push([ms, s / Math.max(1, k)]);
  }
  const best = scores.reduce((a, b) => (b[1] > a[1] ? b : a));
  const sorted = scores.map((s) => s[1]).sort((a, b) => a - b);
  return { phaseMs: best[0], contrast: +(best[1] / sorted[sorted.length >> 1]).toFixed(2) };
};
const levelDb = (x, from, to) => {
  let e = 0;
  const a = Math.max(0, Math.floor(from * SR)), b = Math.min(x.length, Math.floor(to * SR));
  for (let i = a; i < b; i++) e += x[i] * x[i];
  return +(10 * Math.log10(e / Math.max(1, b - a) + 1e-20)).toFixed(1);
};

/** FluidSynth, FluidR3_GM, reverb on (the demo's room), chorus off, 48 kHz float. */
const synth = (mid, wav, meta) => {
  const rv = { room: 0.6, damp: 0.4, width: 0.9, level: 0.6, ...meta.reverb };
  const r = spawnSync("fluidsynth", ["-ni", "-q", "-R", "1", "-C", "0", "-r", String(SR), "-g", String(meta.gain ?? 0.5),
    "-o", `synth.reverb.room-size=${rv.room}`, "-o", `synth.reverb.damp=${rv.damp}`, "-o", `synth.reverb.width=${rv.width}`,
    "-o", `synth.reverb.level=${rv.level}`, "-o", "synth.polyphony=1024", "-o", "audio.file.format=float",
    "-F", wav, SF2, mid], { encoding: "utf8" });
  const errs = (r.stderr ?? "").split("\n").filter((l) => l && !l.startsWith("ALSA lib"));
  if (r.status !== 0) throw new Error(`fluidsynth failed:\n${errs.join("\n")}`);
  if (errs.length) console.log(errs.join("\n"));
};
/** Integrated loudness of [from, from + dur) seconds of a file. */
const segLufs = (file, from, dur) => {
  const s = ff(["-ss", String(from), "-t", String(dur), "-i", file, "-af", "ebur128=framelog=quiet", "-f", "null", "-"]);
  const v = Number(/I:\s+(-?[\d.]+) LUFS/.exec(s.slice(s.lastIndexOf("Summary:")))?.[1]);
  return Number.isFinite(v) && v > -70 ? v : null;
};

/**
 * The mix by numbers: render every part alone (same gain and reverb as the full mix) and print its loudness in
 * each section relative to the full mix there, so the balance can be checked without listening.
 */
const stems = (name) => {
  execFileSync("node", [join(DIR, `${name}.mjs`)], { stdio: "ignore" });
  const meta = JSON.parse(readFileSync(join(OUT, `${name}.json`), "utf8"));
  const secs = meta.sections.map((s, i) => [s.text, s.sec, (meta.sections[i + 1]?.sec ?? meta.musicSec) - s.sec]);
  const full = join(OUT, `${name}.stem.wav`);
  synth(join(DIR, `${name}.mid`), full, meta);
  const ref = secs.map(([, a, d]) => segLufs(full, a, d));
  const rows = [["(full mix, LUFS)", ...ref.map((v) => v?.toFixed(1) ?? "-")]];
  for (const part of meta.parts) {
    execFileSync("node", [join(DIR, `${name}.mjs`)], { stdio: "ignore", env: { ...process.env, SOLO: part } });
    synth(join(OUT, `${name}.solo.mid`), full, meta);
    rows.push([part, ...secs.map(([, a, d], i) => {
      const v = segLufs(full, a, d);
      return v === null || ref[i] === null ? "·" : (v - ref[i]).toFixed(1);
    })]);
  }
  rmSync(full, { force: true });
  rmSync(join(OUT, `${name}.solo.mid`), { force: true });
  const head = ["part (dB vs mix)", ...secs.map(([t]) => t.slice(0, 12))];
  const w = [26, ...secs.map(() => 13)];
  for (const r of [head, ...rows]) console.log(r.map((c, i) => String(c).padEnd(w[i])).join(""));
};

const render = (name) => {
  console.log(`\n== ${name}`);
  execFileSync("node", [join(DIR, `${name}.mjs`)], { stdio: "inherit" });
  const meta = JSON.parse(readFileSync(join(OUT, `${name}.json`), "utf8"));
  const f = (ext) => join(OUT, `${name}${ext}`);
  const mc = { eq: "", threshold: -16, ratio: 1.8, width: 1.15, ...meta.master };

  // 1. synthesize (FluidSynth keeps going until every voice has died; the length is fixed below)
  synth(join(DIR, `${name}.mid`), f(".raw.wav"), meta);
  const len = meta.lengthSec;
  const fade = Math.min(2.5, meta.fadeSec ?? 2);

  // 2. pre-master: level to about -20 LUFS, high-pass, the demo's EQ, a gentle compressor, light widening
  //    (all in float, so nothing clips on the way)
  const raw = ebur(f(".raw.wav"));
  ff(["-i", f(".raw.wav"), "-af",
    `apad,atrim=0:${len.toFixed(6)},volume=${(-20 - raw.I).toFixed(2)}dB,highpass=f=28:poles=2,${mc.eq ? `${mc.eq},` : ""}` +
    `acompressor=threshold=${mc.threshold}dB:ratio=${mc.ratio}:attack=25:release=250:knee=4,extrastereo=m=${mc.width}:c=0,` +
    `afade=t=out:st=${(len - fade).toFixed(3)}:d=${fade}`,
    "-c:a", "pcm_f32le", f(".pre.wav")]);

  // 3. gain to the target, then a 4x-oversampled lookahead limiter (latency-compensated) at -2 dBFS, so the
  //    MP3 (whose encoder adds a few tenths of a dB of true peak) stays under -1 dBTP too
  const pre = ebur(f(".pre.wav"));
  ff(["-i", f(".pre.wav"), "-af",
    `volume=${(TARGET.I - pre.I).toFixed(2)}dB,aresample=${4 * SR},` +
    `alimiter=limit=${10 ** (-2 / 20)}:level=0:latency=1:attack=1.5:release=80,aresample=${SR}`,
    "-c:a", "pcm_f32le", f(".lim.wav")]);

  // 4. two-pass loudnorm (linear) to -14 LUFS / -1 dBTP; 24-bit WAV, then MP3 320k
  const ln = `loudnorm=I=${TARGET.I}:TP=${TARGET.TP}:LRA=50`;
  const m = loudnormJson(f(".lim.wav"), `${ln}:print_format=json`);
  const ln2 = `${ln}:linear=true:measured_I=${m.input_i}:measured_TP=${m.input_tp}:measured_LRA=${m.input_lra}:measured_thresh=${m.input_thresh}:offset=${m.target_offset}`;
  const pass2 = ff(["-i", f(".lim.wav"), "-af", `${ln2}:print_format=json,aresample=${SR}:dither_method=triangular`,
    "-ar", String(SR), "-c:a", "pcm_s24le", f(".wav")]);
  const normType = JSON.parse(pass2.slice(pass2.lastIndexOf("{"), pass2.lastIndexOf("}") + 1)).normalization_type;
  ff(["-i", f(".wav"), "-c:a", "libmp3lame", "-b:a", "320k", f(".mp3")]);

  // 5. pictures
  ff(["-i", f(".wav"), "-lavfi", "showspectrumpic=s=1600x600:legend=1:fscale=log:start=30:stop=20000:drange=100:color=intensity", f(".spectrum.png")]);
  ff(["-i", f(".wav"), "-lavfi", "showwavespic=s=1600x300:split_channels=0:scale=sqrt:colors=0x7fd6ff", f(".wave.png")]);

  // 6. checks
  const fin = ebur(f(".wav"));
  const mp3 = ebur(f(".mp3"));
  const st = astats(f(".wav"));
  const xb = mono(f(".wav"), "anull"); // broadband, for the levels at the edges
  const om = onsetCurve(mono(f(".wav")));
  const or = onsetCurve(mono(f(".raw.wav")));
  const ol = onsetCurve(mono(f(".wav"), LOW), LOW_CURVE);
  const beatSec = 60 / meta.bpm;
  const hitSecs = (meta.hits ?? []).map((b) => b * beatSec);
  const offs = hitOffsets(om, hitSecs);
  const offsRaw = hitOffsets(or, hitSecs);
  const offsLow = hitOffsets(ol, hitSecs);
  const devs = offs.map((h, i) => Math.min(h === null ? 99 : Math.abs(h - CAL.high), offsLow[i] === null ? 99 : Math.abs(offsLow[i] - CAL.low)));
  // a constant offset is the detector and the samples' attack; the spread is what says "on the grid"
  const stats = (a) => ((a = a.filter((v) => v !== null)).length
    ? { meanMs: +(a.reduce((s, v) => s + v, 0) / a.length).toFixed(1), minMs: Math.min(...a), maxMs: Math.max(...a), spreadMs: +(Math.max(...a) - Math.min(...a)).toFixed(1) }
    : null);
  const grid = (meta.gridChecks ?? []).map((g) => ({ ...g, ...gridPhase(om, g.stepBeats * beatSec, g.fromBar ? (g.fromBar - 1) * meta.beatsPerBar * beatSec : 0, g.toBar ? g.toBar * meta.beatsPerBar * beatSec : meta.musicSec) }));
  const durationSec = Number(execFileSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", f(".wav")], { encoding: "utf8" }));
  const check = {
    name, bpm: meta.bpm, key: meta.key, bars: meta.bars, durationSec, musicSec: meta.musicSec,
    loudness: { wav: fin, mp3, raw: raw.I, normalization: normType },
    astats: st,
    edges: { first50msDb: levelDb(xb, 0, 0.05), last100msDb: levelDb(xb, durationSec - 0.1, durationSec) },
    hits: {
      count: offs.length, high: stats(offs), low: stats(offsLow), rawHigh: stats(offsRaw), maxDevMs: Math.max(...devs),
      perHit: meta.hits.map((b, i) => ({ bar: Math.floor(b / meta.beatsPerBar) + 1, beat: (b % meta.beatsPerBar) + 1, highMs: offs[i], lowMs: offsLow[i], devMs: devs[i] })),
    },
    grid,
  };
  writeFileSync(f(".check.json"), JSON.stringify(check, null, 2));
  for (const t of [".raw.wav", ".pre.wav", ".lim.wav"]) rmSync(f(t), { force: true });
  console.log(`${name}: ${durationSec.toFixed(2)} s | I ${fin.I} LUFS, LRA ${fin.LRA} LU, TP ${fin.TP} dBTP (mp3 TP ${mp3.TP}) [${normType}]` +
    ` | ${offs.length} hits within ${check.hits.maxDevMs} ms of the grid (high band ${check.hits.high?.minMs}..${check.hits.high?.maxMs}, low ${check.hits.low?.minMs}..${check.hits.low?.maxMs} ms)` +
    ` | grid ${grid.map((g) => `${g.label} ${g.phaseMs} ms ×${g.contrast}`).join(", ")} | last 100 ms ${check.edges.last100msDb} dB`);
  return check;
};

export { mono, onsetCurve, hitOffsets, gridPhase, LOW, LOW_CURVE };

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2);
  const names = args.filter((a) => !a.startsWith("--"));
  for (const name of names.length ? names : DEMOS) (args.includes("--stems") ? stems : render)(name);
}
