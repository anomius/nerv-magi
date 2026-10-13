#!/usr/bin/env node
// The soundtracks of one cut, on one track map. Writes into --out (all WAV, 48 kHz stereo):
//   sfx.wav     the SFX alone (scripts/sfx.mjs on this timeline), at a fixed gain: -18 LUFS or a
//               -2 dBTP ceiling, whichever is lower (no limiter; a short cut keeps the full film's gain)
//   medley.wav  the medley (public/audio/mix.wav, run `pnpm run audio` first), default map only,
//               loudness-normalized to -14 LUFS / -2 dBTP
//   track.wav   the map's own audio (`audio`, if the file exists), laid in at offsetSec, ducked under
//               the SFX, silenced at the hard cut like the medley, -14 LUFS / -2 dBTP (private preview)
//   click.wav, guide.wav   (whole film only) a click on every beat of the track, accented downbeats
//               (1.5 kHz) and other beats (1 kHz); guide = the SFX with the click mixed in
//   check.wav   (whole film, map audio present) the map's audio with the click: confirm the map by ear
// A short cut (--edit) splices every source at the cut's bar lines: the incoming range starts at full
// level on its bar line (4 ms fade-in just before it), the outgoing one rings 60 ms past it, fading.
//   cut-audio.mjs --out dir [--track tracks/x.json] [--edit teaser15|cut30]
import { mkdirSync, existsSync, rmSync } from "node:fs";
import { join } from "node:path";
import { EDITS, editSegments } from "../src/edits.ts";
import { synthSfx } from "./sfx.mjs";
import { PROMO, loadTrack, trackAudio } from "./lib/track.mjs";
import { SR, decode, ff, levelTo, loudness, loudnorm, silence, writeWav } from "./lib/wav.mjs";

const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : undefined;
};
const out = opt("--out");
const edit = opt("--edit");
const track = loadTrack(opt("--track"));
const { tl, map } = track;
if (!out) throw new Error("cut-audio: --out dir is required");
if (edit && !(edit in EDITS)) throw new Error(`cut-audio: unknown edit ${edit}`);
mkdirSync(out, { recursive: true });
const work = join(out, ".audio-work");
mkdirSync(work, { recursive: true });

const spf = SR / tl.fps; // samples per frame (800)
const filmLen = Math.round(tl.total * spf);
const FADE_IN = Math.round(0.004 * SR);
const FADE_OUT = Math.round(0.06 * SR);

/** The cut of a whole-film buffer: the film itself, or its ranges spliced on the bar lines. */
const cut = (src) => {
  if (!edit) return src;
  const segs = editSegments(tl, edit);
  const last = segs.at(-1);
  const n = (last.at + last.to - last.from) * spf;
  const dst = silence(n);
  segs.forEach((s, k) => {
    const first = k === 0;
    const end = k === segs.length - 1;
    const pre = first ? 0 : FADE_IN;
    const post = end ? 0 : FADE_OUT;
    const from = s.from * spf;
    const to = s.to * spf;
    const at = s.at * spf;
    for (let i = -pre; i < to - from + post; i++) {
      const j = at + i;
      if (j < 0 || j >= n) continue;
      let g = 1;
      if (i < 0) g = 0.5 - 0.5 * Math.cos((Math.PI * (i + pre)) / pre); // into the bar line
      else if (i >= to - from) g = 0.5 + 0.5 * Math.cos((Math.PI * (i - (to - from))) / post); // ringing past it
      dst.L[j] += (src.L[from + i] ?? 0) * g;
      dst.R[j] += (src.R[from + i] ?? 0) * g;
    }
  });
  return dst;
};

/** A whole-film buffer from an audio file whose time 0 is the video's frame 0 shifted by `offset` s. */
const aligned = (path, offset = 0) => {
  const a = decode(path, { start: Math.max(0, offset), dur: filmLen / SR + 1 });
  const buf = silence(filmLen);
  const shift = Math.round(Math.max(0, -offset) * SR);
  for (let i = 0; i + shift < filmLen && i < a.L.length; i++) {
    buf.L[i + shift] = a.L[i];
    buf.R[i + shift] = a.R[i];
  }
  return buf;
};
const cutLen = (buf) => buf.L.length;
const made = [];

// SFX: the gain is set on the whole film, then the cut is taken (a short cut sits at the film's level)
const sfx = synthSfx(tl);
let peak = 0;
for (let i = 0; i < sfx.len; i++) peak = Math.max(peak, Math.abs(sfx.L[i]), Math.abs(sfx.R[i]));
const sfxFilm = { L: sfx.L.subarray(0, filmLen).map((v) => (v * 0.5) / peak), R: sfx.R.subarray(0, filmLen).map((v) => (v * 0.5) / peak) };
writeWav(join(work, "sfx-film.wav"), sfxFilm, filmLen);
const film = levelTo(join(work, "sfx-film.wav"), join(work, "sfx-film-leveled.wav"));
const sfxCut = cut(sfxFilm);
writeWav(join(work, "sfx-cut.wav"), sfxCut, cutLen(sfxCut));
ff(["-i", join(work, "sfx-cut.wav"), "-af", `volume=${film.gain.toFixed(2)}dB`, "-ar", String(SR), "-c:a", "pcm_s24le", join(out, "sfx.wav")]);
made.push(["sfx.wav", loudness(join(out, "sfx.wav"))]);

const gate = (n) => {
  // the hard cut to つづく: 8 ms to silence on the cut's frame (sample-accurate afade)
  const at = (edit ? editSegments(tl, edit).at(-1).at + tl.outro.cut - editSegments(tl, edit).at(-1).from : tl.outro.cut) / tl.fps;
  return at < n / SR ? `afade=t=out:st=${(at - 0.008).toFixed(4)}:d=0.008` : "";
};

// the medley: only on the timeline it was composed for
const isDefault = JSON.stringify(tl.sections) === JSON.stringify(loadTrack().tl.sections) && tl.bpm === 150 && tl.offsetSec === 0;
const mixWav = join(PROMO, "public/audio/mix.wav");
if (isDefault && existsSync(mixWav)) {
  const m = cut(decode(mixWav));
  writeWav(join(work, "medley-cut.wav"), m, cutLen(m));
  made.push(["medley.wav", loudnorm(join(work, "medley-cut.wav"), join(out, "medley.wav"), { post: gate(cutLen(m)) })]);
}

// the map's own audio (private preview): ducked under the SFX like the medley, gated at the hard cut
const own = trackAudio(track);
if (own && typeof own === "object") console.warn(`cut-audio: ${map.name}: audio "${own.missing}" not found; no preview`);
if (typeof own === "string") {
  const m = cut(aligned(own, tl.offsetSec));
  writeWav(join(work, "track-cut.wav"), m, cutLen(m));
  ff([
    "-i", join(work, "track-cut.wav"), "-i", join(out, "sfx.wav"), "-filter_complex",
    `[1:a]asplit=2[fx][key];[0:a][key]sidechaincompress=threshold=0.15:ratio=2.5:attack=6:release=160:makeup=1[duck];` +
      `[duck][fx]amix=inputs=2:normalize=0:weights=1 0.9[out]`,
    "-map", "[out]", "-c:a", "pcm_f32le", join(work, "track-mix.wav"),
  ]);
  // a brick-wall limiter after the gain: a track that is not mastered (or the SFX on top) must not clip
  const post = ["alimiter=limit=0.6:attack=1:release=60:level=0", gate(cutLen(m))].filter(Boolean).join(",");
  made.push(["track.wav", loudnorm(join(work, "track-mix.wav"), join(out, "track.wav"), { post })]);
}

// the click guide (whole film): exact beat times of the track, not rounded to frames
if (!edit) {
  const click = silence(filmLen);
  const blipLen = Math.round(0.05 * SR);
  for (let b = 0; b * tl.spb * SR < filmLen; b++) {
    const down = b % tl.bpb === 0;
    const f0 = down ? 1500 : 1000;
    const amp = down ? 0.5 : 0.3;
    const s0 = Math.round(b * tl.spb * SR);
    for (let i = 0; i < blipLen && s0 + i < filmLen; i++) {
      const t = i / SR;
      const v = Math.sin(2 * Math.PI * f0 * t) * Math.min(1, t / 0.001) * Math.exp(-t / 0.012) * amp;
      click.L[s0 + i] += v;
      click.R[s0 + i] += v;
    }
  }
  writeWav(join(work, "click.wav"), click, filmLen);
  ff(["-i", join(work, "click.wav"), "-c:a", "pcm_s24le", join(out, "click.wav")]);
  made.push(["click.wav", loudness(join(out, "click.wav"))]);
  ff(["-i", join(out, "sfx.wav"), "-i", join(out, "click.wav"), "-filter_complex", "[0:a][1:a]amix=inputs=2:normalize=0:weights=1 0.7[out]", "-map", "[out]", "-c:a", "pcm_s24le", join(out, "guide.wav")]);
  made.push(["guide.wav", loudness(join(out, "guide.wav"))]);
  if (typeof own === "string") {
    // the song as the map places it, plus the click: the high clicks must sit on its bar lines
    const song = aligned(own, tl.offsetSec);
    writeWav(join(work, "song.wav"), song, filmLen);
    ff(["-i", join(work, "song.wav"), "-i", join(out, "click.wav"), "-filter_complex", "[0:a]volume=-6dB[s];[s][1:a]amix=inputs=2:normalize=0:weights=1 1.4,alimiter=limit=0.7:level=0[out]", "-map", "[out]", "-c:a", "pcm_s24le", join(out, "check.wav")]);
    made.push(["check.wav", loudness(join(out, "check.wav"))]);
  }
}

rmSync(work, { recursive: true, force: true });
const secs = (edit ? cutLen(sfxCut) : filmLen) / SR;
console.log(`cut-audio: ${map.name}${edit ? ` ${edit}` : ""} (${secs.toFixed(3)} s) → ${out}`);
for (const [f, l] of made) console.log(`  ${f.padEnd(11)} ${l.I} LUFS, true peak ${l.TP} dBTP`);
