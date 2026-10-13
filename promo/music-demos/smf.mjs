// A small Standard MIDI File (format 1) writer for the music demos. Self-contained on purpose: the promo
// score's helpers (../scripts/music/midi.mjs) belong to the score and change with it, and the demos need a
// few things those don't do: millisecond humanizing, pitch bend (scoops, vibrato), bank select, section
// markers, overlap repair for re-struck notes, and a silent tail so FluidSynth renders the reverb out.
// Positions are in beats (quarter notes); bars and beats are 1-based. No dependencies.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const PPQ = 960;

const NAMES = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
const ACC = { "": 0, "#": 1, "##": 2, b: -1, bb: -2 };
/** "D4" → 62, "F#5" → 78, "Bb1" → 34, "Cb4" → 59 (C4 = 60). Numbers pass through. */
export const n = (name) => {
  if (typeof name === "number") return name;
  const m = /^([A-G])(##|#|bb|b)?(-?\d)$/.exec(name);
  if (!m) throw new Error(`bad note ${name}`);
  return 12 * (Number(m[3]) + 1) + NAMES[m[1]] + ACC[m[2] ?? ""];
};

/** Deterministic PRNG (mulberry32), so every render of a demo is identical. */
export const prng = (seed) => () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const hash = (s) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7);
const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));

export class Part {
  /**
   * @param song   the Song (for tempo, used to turn milliseconds into beats)
   * @param opts   ch, program, bank (0, 8, … or 128 for a drum kit on ch 9), vol, pan (0..127), rev, cho,
   *               human (timing jitter, ± ms), humanVel (± velocity), lag (constant ms offset, + = behind)
   */
  constructor(song, name, { ch, program = 0, bank = 0, vol = 100, pan = 64, rev = 40, cho = 0, expr = 100, human = 8, humanVel = 6, lag = 0 }) {
    Object.assign(this, { song, name, ch, human, humanVel, lag });
    this.notes = []; // { t, end, key, vel } in beats
    this.ctl = []; // [beat, order, bytes]
    this.rnd = prng(hash(name) ^ song.seed);
    const head = (bytes) => this.ctl.push([0, 0, bytes]);
    if (ch !== 9 && bank) head([0xb0 | ch, 0, bank]);
    if (ch !== 9 || program) head([0xc0 | ch, program]);
    for (const [num, v] of [[7, vol], [10, pan], [91, rev], [93, cho], [11, expr]]) head([0xb0 | ch, num, v]);
  }
  /** Milliseconds → beats at the song's base tempo. */
  ms(v) {
    return (v / 1000) * (this.song.bpm / 60);
  }
  /** One or more pitches at `beat` for `len` beats. `exact` puts the attack on the grid (no timing jitter). */
  note(beat, len, pitches, vel = 90, { exact = false, legato = 0.98 } = {}) {
    for (const p of [].concat(pitches)) {
      const dt = exact ? 0 : this.ms(this.lag + (this.rnd() * 2 - 1) * this.human);
      const dv = (this.rnd() * 2 - 1) * this.humanVel * (exact ? 0.5 : 1);
      const t = Math.max(0, beat + dt);
      const end = beat + len * legato + (exact ? 0 : this.ms((this.rnd() * 2 - 1) * this.human));
      this.notes.push({ t, end: Math.max(t + 0.02, end), key: n(p), vel: clamp(Math.round(vel + dv), 1, 127) });
    }
    return this;
  }
  /** A sequence of [pitch | pitches | null, beats, vel?] from `beat`; returns the end beat. */
  line(beat, notes, vel = 90, legato = 0.96) {
    let t = beat;
    for (const [p, len, v] of notes) {
      if (p !== null) this.note(t, len, p, v ?? vel, { legato });
      t += len;
    }
    return t;
  }
  cc(beat, num, val) {
    this.ctl.push([beat, 1, [0xb0 | this.ch, num, clamp(Math.round(val), 0, 127)]]);
    return this;
  }
  /** Controller ramp from v0 to v1 over `len` beats (default CC11, expression). */
  ramp(beat, len, v0, v1, num = 11, steps = Math.max(4, Math.ceil(len * 8))) {
    for (let i = 0; i <= steps; i++) this.cc(beat + (len * i) / steps, num, v0 + ((v1 - v0) * i) / steps);
    return this;
  }
  /** Pitch bend in semitones (GM default range ±2). */
  bend(beat, semis) {
    const v = clamp(Math.round(8192 + (semis / 2) * 8192), 0, 16383);
    this.ctl.push([beat, 1, [0xe0 | this.ch, v & 0x7f, v >> 7]]);
    return this;
  }
  bendRamp(beat, len, s0, s1, steps = Math.max(4, Math.ceil(len * 16))) {
    for (let i = 0; i <= steps; i++) this.bend(beat + (len * i) / steps, s0 + ((s1 - s0) * i) / steps);
    return this;
  }
  /** Scoop into a note from `semis` below over `ms` milliseconds (horn and sax attacks). */
  scoop(beat, semis = -0.4, ms = 90) {
    return this.bendRamp(beat - this.ms(4), this.ms(ms), semis, 0, 8);
  }
  /** Delayed vibrato on a held note: depth in semitones, rate in Hz, fading in after `delay` beats. */
  vibrato(beat, len, { depth = 0.12, rate = 5.2, delay = 0.5 } = {}) {
    const secs = (len / this.song.bpm) * 60;
    const steps = Math.ceil(secs * rate * 8);
    for (let i = 0; i <= steps; i++) {
      const b = beat + (len * i) / steps;
      const fade = clamp((b - beat - delay) / Math.max(0.25, len * 0.3), 0, 1);
      this.bend(b, depth * fade * Math.sin(2 * Math.PI * rate * ((b - beat) / this.song.bpm) * 60));
    }
    return this.bend(beat + len, 0);
  }
  /** Sustain pedal down at `beat`, up just before `beat + len`. */
  pedal(beat, len) {
    return this.cc(beat + 0.02, 64, 127).cc(beat + len - 0.03, 64, 0);
  }
}

export class Song {
  constructor({ bpm, bar = 4, seed = 1 }) {
    Object.assign(this, { bpm, bar, seed });
    this.parts = [];
    this.markers = [];
  }
  /** Beat position of `bar` (1-based) at `beat` (1-based, fractional ok). */
  at(bar, beat = 1) {
    return (bar - 1) * this.bar + (beat - 1);
  }
  /** Seconds at a beat position (constant tempo: every demo keeps its grid, so cuts stay on frames). */
  sec(beat) {
    return (beat * 60) / this.bpm;
  }
  part(name, opts) {
    const p = new Part(this, name, opts);
    this.parts.push(p);
    return p;
  }
  marker(bar, text) {
    this.markers.push([this.at(bar), text]);
    return this;
  }
  /**
   * Notes of one key on one channel never overlap (a re-strike would be cut by the earlier note-off), and two
   * attacks of the same key within 25 ms (a tutti chord tone landing on a melody note) merge into one.
   */
  #repair() {
    const merge = (25 / 1000) * (this.bpm / 60);
    const byKey = new Map();
    for (const p of this.parts)
      for (const x of p.notes) {
        const k = `${p.ch}:${x.key}`;
        if (!byKey.has(k)) byKey.set(k, []);
        byKey.get(k).push(x);
      }
    for (const list of byKey.values()) {
      list.sort((a, b) => a.t - b.t);
      let prev = null;
      for (const x of list) {
        if (prev && x.t - prev.t < merge) {
          prev.end = Math.max(prev.end, x.end);
          prev.vel = Math.max(prev.vel, x.vel);
          x.dropped = true;
          continue;
        }
        if (prev && prev.end > x.t - 1 / PPQ) prev.end = Math.max(prev.t + 1 / PPQ, x.t - 1 / PPQ);
        prev = x;
      }
    }
    for (const p of this.parts) p.notes = p.notes.filter((x) => !x.dropped);
  }
  /**
   * Write `<dir>/<name>.mid` and `<dir>/out/<name>.json` (bars, tempo, hits on the grid, sections, render
   * settings) for render.mjs. `endBar`: the music ends at the end of that bar; `tail` seconds of silence follow
   * so the reverb rings out (FluidSynth stops rendering at the end of the MIDI file).
   */
  save(dir, name, { endBar, tail = 3, meta = {} }) {
    this.#repair();
    // SOLO=<part name>: write only that part to out/<name>.solo.mid (render.mjs --stems measures the balance)
    const solo = process.env.SOLO;
    if (solo) {
      this.parts = this.parts.filter((p) => p.name === solo);
      if (!this.parts.length) throw new Error(`no part named ${solo}`);
    }
    const endBeat = this.at(endBar + 1);
    const eot = endBeat + (tail * this.bpm) / 60;
    const tick = (b) => Math.max(0, Math.round(b * PPQ));
    const us = Math.round(60_000_000 / this.bpm);
    const conductor = [
      [0, 0, [0xff, 0x51, 0x03, (us >> 16) & 255, (us >> 8) & 255, us & 255]],
      [0, 0, [0xff, 0x58, 0x04, this.bar, 2, 24, 8]],
      ...this.markers.map(([b, text]) => [b, 1, [0xff, 0x06, ...vlq(Buffer.byteLength(text)), ...Buffer.from(text)]]),
    ];
    const tracks = [trackChunk("conductor", conductor, tick, eot)];
    let count = 0;
    for (const p of this.parts) {
      const ev = [...p.ctl];
      for (const x of p.notes) {
        ev.push([x.t, 3, [0x90 | p.ch, x.key, x.vel]], [x.end, 2, [0x80 | p.ch, x.key, 0]]);
        count++;
      }
      tracks.push(trackChunk(p.name, ev, tick, eot));
    }
    const header = chunk("MThd", [0, 1, 0, tracks.length, (PPQ >> 8) & 255, PPQ & 255]);
    mkdirSync(join(dir, "out"), { recursive: true });
    const mid = solo ? join(dir, "out", `${name}.solo.mid`) : join(dir, `${name}.mid`);
    writeFileSync(mid, Buffer.from([...header, ...tracks.flat()]));
    if (solo) return null;
    const info = {
      name,
      bpm: this.bpm,
      beatsPerBar: this.bar,
      bars: endBar,
      musicSec: this.sec(endBeat),
      lengthSec: this.sec(endBeat) + tail,
      sections: this.markers.map(([b, text]) => ({ bar: b / this.bar + 1, sec: +this.sec(b).toFixed(3), text })),
      parts: this.parts.map((p) => p.name),
      ...meta,
    };
    writeFileSync(join(dir, "out", `${name}.json`), JSON.stringify(info, null, 2));
    console.log(`${name}: ${mid} (${endBar} bars at ${this.bpm} BPM = ${info.musicSec.toFixed(2)} s + ${tail} s tail, ${count} notes, ${this.parts.length} parts)`);
    return info;
  }
}

const vlq = (v) => {
  const out = [v & 0x7f];
  while ((v >>= 7)) out.unshift((v & 0x7f) | 0x80);
  return out;
};
const chunk = (id, data) => {
  const len = data.length;
  return [...Buffer.from(id), (len >>> 24) & 255, (len >>> 16) & 255, (len >>> 8) & 255, len & 255, ...data];
};
/** One MTrk: events sorted by time, then order (program/meta 0, controllers 1, note-off 2, note-on 3). */
const trackChunk = (name, events, tick, eot) => {
  const sorted = events.map(([b, o, bytes]) => [tick(b), o, bytes]).sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const nameBytes = [...Buffer.from(name)];
  const data = [0, 0xff, 0x03, ...vlq(nameBytes.length), ...nameBytes];
  let last = 0;
  for (const [t, , bytes] of sorted) {
    data.push(...vlq(t - last), ...bytes);
    last = t;
  }
  data.push(...vlq(Math.max(0, tick(eot) - last)), 0xff, 0x2f, 0);
  return chunk("MTrk", data);
};
