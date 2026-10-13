// A minimal Standard MIDI File (format 1) writer: tracks of notes, program and controller changes,
// positioned in beats (quarter notes). No dependencies.
export const PPQ = 480;

const NAMES = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
/** "D4" → 62, "F#5" → 78, "Bb1" → 34 (C4 = 60). Numbers pass through. */
export const n = (name) => {
  if (typeof name === "number") return name;
  const m = /^([A-G])(#|b)?(-?\d)$/.exec(name);
  if (!m) throw new Error(`bad note ${name}`);
  return 12 * (Number(m[3]) + 1) + NAMES[m[1]] + (m[2] === "#" ? 1 : m[2] === "b" ? -1 : 0);
};

/** Deterministic PRNG (mulberry32) for humanizing. */
const prng = (seed) => () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};

export class Track {
  constructor(name, channel, program) {
    this.name = name;
    this.ch = channel;
    this.events = []; // [tick, order, bytes]
    /** Humanizing (off by default): timing jitter in beats (± this), velocity jitter (± this). */
    this.human = 0;
    this.humanVel = 0;
    this.rnd = prng([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) | 0, 7));
    if (program !== undefined) this.program(0, program);
  }
  #push(beat, order, bytes) {
    this.events.push([Math.round(beat * PPQ), order, bytes]);
  }
  program(beat, prog) {
    this.#push(beat, 0, [0xc0 | this.ch, prog]);
    return this;
  }
  cc(beat, num, val) {
    this.#push(beat, 1, [0xb0 | this.ch, num, Math.max(0, Math.min(127, Math.round(val)))]);
    return this;
  }
  /** Controller ramp from v0 to v1 over `len` beats. */
  ramp(beat, len, num, v0, v1, steps = 24) {
    for (let i = 0; i <= steps; i++) this.cc(beat + (len * i) / steps, num, v0 + ((v1 - v0) * i) / steps);
    return this;
  }
  /** One or more pitches at `beat` for `len` beats; `exact` skips humanizing (hits on the grid). */
  note(beat, len, pitches, vel = 96, exact = false) {
    for (const p of [].concat(pitches)) {
      const k = n(p);
      const dt = exact || !this.human ? 0 : (this.rnd() * 2 - 1) * this.human;
      const dv = exact || !this.humanVel ? 0 : (this.rnd() * 2 - 1) * this.humanVel;
      const at = Math.max(0, beat + dt);
      this.#push(at, 3, [0x90 | this.ch, k, Math.max(1, Math.min(127, Math.round(vel + dv)))]);
      this.#push(at + len * 0.98, 2, [0x80 | this.ch, k, 0]);
    }
    return this;
  }
  /** A sequence of [pitch|null, beats, vel?] from `beat`; returns the end beat. */
  line(beat, notes, vel = 96, legato = 0.95) {
    let t = beat;
    for (const [p, len, v] of notes) {
      if (p !== null) this.note(t, len * legato, p, v ?? vel);
      t += len;
    }
    return t;
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

const trackChunk = (name, events) => {
  const sorted = [...events].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const nameBytes = [...Buffer.from(name)];
  const data = [0, 0xff, 0x03, ...vlq(nameBytes.length), ...nameBytes];
  let last = 0;
  for (const [tick, , bytes] of sorted) {
    data.push(...vlq(tick - last), ...bytes);
    last = tick;
  }
  data.push(0, 0xff, 0x2f, 0);
  return chunk("MTrk", data);
};

/** Serialize: a conductor track (tempo, 4/4) plus every instrument track. */
export const writeMidi = (bpm, tracks) => {
  const us = Math.round(60_000_000 / bpm);
  const conductor = [
    [0, 0, [0xff, 0x51, 0x03, (us >> 16) & 255, (us >> 8) & 255, us & 255]],
    [0, 0, [0xff, 0x58, 0x04, 4, 2, 24, 8]],
  ];
  const header = chunk("MThd", [0, 1, 0, tracks.length + 1, (PPQ >> 8) & 255, PPQ & 255]);
  return Buffer.from([...header, ...trackChunk("conductor", conductor), ...tracks.flatMap((t) => trackChunk(t.name, t.events))]);
};
