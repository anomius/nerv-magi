/**
 * The edit, on a musical grid: 150 BPM, 4/4, at 60 fps one beat is 24 frames and one bar 96.
 * Every scene start, cut, flash, vote and wipe is written as bar/beat (`bb`). This file is the
 * single source of timing: the scenes import it, and the music and SFX scripts import it too
 * (Node strips the types), so picture and sound cannot drift apart.
 */
export const FPS = 60;
export const BPM = 150;
export const BEAT = 24;
export const BAR = 4 * BEAT;
export const BARS = 36;

/** Frame of `bar` (1-based) at `beat` (1-based, fractional allowed), plus `extra` frames. */
export const bb = (bar: number, beat = 1, extra = 0) => Math.round((bar - 1) * BAR + (beat - 1) * BEAT + extra);

export const T = {
  fps: FPS,
  bpm: BPM,
  beat: BEAT,
  bar: BAR,
  total: bb(BARS + 1),
  poster: bb(13, 2, 8),
  scenes: {
    powerOn: [0, bb(3)],
    title: [bb(3), bb(5)],
    pattern: [bb(5), bb(9)],
    magi: [bb(9), bb(15)],
    reveal: [bb(15), bb(25)],
    flow: [bb(25), bb(28)],
    emergency: [bb(28), bb(32)],
    outro: [bb(32), bb(BARS + 1)],
  },
  boot: {
    crt: 6,
    lines: [
      { text: "MAGI SYSTEM / BOOT", at: bb(1, 2), fpc: 2 },
      { text: "CONNECTING HERDR", at: bb(1, 3, 10), fpc: 2 },
    ],
    linked: bb(2),
    footage: bb(2, 2),
    kanji: bb(2, 3),
  },
  title: { sub: bb(3, 1, 8), out: bb(5) },
  pattern: { sweep: bb(5, 1, 6), hexes: bb(5, 3), hexStep: BEAT / 2, header: bb(7), zoom: bb(8, 2) },
  magi: {
    draw: bb(9),
    motion: bb(9, 3),
    deliberate: bb(9, 4),
    votes: [bb(10), bb(11), bb(12)],
    carried: bb(13),
    riser: [bb(14), bb(15)],
  },
  reveal: {
    push: bb(15),
    wide: bb(16),
    callouts: [bb(17), bb(19), bb(21), bb(23)],
    end: bb(25),
  },
  flow: { load: bb(25), sync: bb(26), main: bb(27), alarm: bb(27, 4) },
  emergency: { takeover: bb(28), card: bb(29), clear: bb(30, 3), resolve: bb(31), line: bb(31, 3) },
  outro: { lines: [bb(32), bb(33), bb(33, 3)], slam: bb(34, 3), cut: bb(36, 3), end: bb(BARS + 1) },
} as const;

/** Frames since `at` (negative before it). */
export const since = (frame: number, at: number) => frame - at;

/** Deterministic hash noise in [0, 1) (frame-stable flicker, glitches, grain seeds). */
export const hash = (n: number) => {
  const x = Math.sin(n * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x);
};

/** On/off blink with `period` frames, `duty` fraction on. */
export const blink = (frame: number, period: number, duty = 0.5) => (frame % period) / period < duty;

/** 0..1 decaying pulse on every beat-3 of a bar (accents), `len` frames long. */
export const beat3 = (frame: number, len = 10) => {
  const k = (frame % BAR) - 2 * BEAT;
  return k >= 0 && k < len ? 1 - k / len : 0;
};
