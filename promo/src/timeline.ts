/**
 * The edit, on a musical grid. It is written once, in canonical time: 36 bars of 4/4 at 150 BPM, where
 * one beat is 24 frames at 60 fps. Every scene start, cut, flash, vote and wipe is a bar/beat position,
 * `bb(bar, beat, extra)` (`extra` in canonical frames, 1/24 beat).
 *
 * A track map (tracks/<name>.json) re-times that edit to a piece of music: its tempo, and optionally
 * the bar where each section starts. `makeTimeline(map)` warps every canonical position into the
 * track's beats (section by section), turns it into seconds and rounds it to a frame once, per cue, so
 * a non-integer frames-per-beat (128 BPM: 28.125) never drifts. Durations written in canonical frames
 * scale with the tempo (`d`), and scene-local clocks read in canonical frames (`k`), so each scene's
 * choreography stretches with the beat.
 *
 * Scenes get the timeline from React context (src/tl.tsx), fed by the composition's input props; the
 * music, SFX, click and cue-sheet scripts call makeTimeline() themselves (Node strips the types). The
 * default map, tracks/house-150.json, is the edit as written (the medley's tempo).
 */
import HOUSE from "../tracks/house-150.json" with { type: "json" };

export const FPS = 60;
/** The canonical grid: the edit as written, at the default track's tempo. */
export const BPM = 150;
export const BARS = 36;
const CANON_BEAT = 24; // canonical frames per beat

export const SECTIONS = ["title", "pattern", "magi", "reveal", "flow", "emergency", "outro", "end"] as const;
export type Section = (typeof SECTIONS)[number];
/** Bar where each section starts in the edit as written (power-on is bar 1; `end` is the bar after the last). */
export const CANON_SECTIONS: Record<Section, number> = { title: 3, pattern: 5, magi: 9, reveal: 15, flow: 25, emergency: 28, outro: 32, end: 37 };

/** A track map, tracks/<name>.json. Only `name` and `bpm` are required. */
export type TrackMap = {
  name: string;
  bpm: number;
  /** Beats per bar (the edit is written in 4/4; another meter maps one written bar onto one track bar). */
  beatsPerBar?: number;
  /** Time of bar 1 beat 1 in the audio file, in seconds: the video's first frame. */
  offsetSec?: number;
  /** Audio for a private preview (under byo-audio/, gitignored; never committed). */
  audio?: string;
  /** Bar of the track (bar 1 = offsetSec) where each section starts; omitted ones keep today's proportions. */
  sections?: Partial<Record<Section, number>>;
};
export const DEFAULT_MAP = HOUSE as TrackMap;

const ANCHORS = ["power-on", ...SECTIONS] as const;
const CANON_ANCHORS = [1, ...SECTIONS.map((s) => CANON_SECTIONS[s])];

/**
 * Start bar of every section. Given bars are kept; an omitted section sits at today's proportions
 * between its given neighbours (rounded to a whole bar), or after the last given one at today's
 * length. A section may be squeezed to half its written length at most.
 */
export const resolveSections = (given: TrackMap["sections"] = {}): Record<Section, number> => {
  const bars: (number | undefined)[] = [1, ...SECTIONS.map((s) => given[s])];
  bars.forEach((b, i) => {
    if (b !== undefined && (!Number.isInteger(b) || b < 1)) throw new Error(`track map: sections.${ANCHORS[i]} must be a whole bar >= 1 (got ${b})`);
  });
  const out = [1];
  for (let i = 1; i < ANCHORS.length; i++) {
    const b = bars[i];
    if (b !== undefined) {
      out.push(b);
      continue;
    }
    let a = i - 1;
    while (bars[a] === undefined) a--;
    let z = i + 1;
    while (z < ANCHORS.length && bars[z] === undefined) z++;
    const ba = bars[a] as number;
    out.push(
      z < ANCHORS.length
        ? Math.round(ba + ((CANON_ANCHORS[i] - CANON_ANCHORS[a]) * ((bars[z] as number) - ba)) / (CANON_ANCHORS[z] - CANON_ANCHORS[a]))
        : out[i - 1] + CANON_ANCHORS[i] - CANON_ANCHORS[i - 1],
    );
  }
  for (let i = 1; i < out.length; i++) {
    const len = out[i] - out[i - 1];
    const written = CANON_ANCHORS[i] - CANON_ANCHORS[i - 1];
    if (len * 2 < written) {
      throw new Error(`track map: ${ANCHORS[i - 1]} would last ${len} bar(s) (bars ${out[i - 1]}-${out[i]}); it needs at least ${Math.ceil(written / 2)} (written: ${written})`);
    }
  }
  return Object.fromEntries(SECTIONS.map((s, i) => [s, out[i + 1]])) as Record<Section, number>;
};

/** The edit's timing for one track map: every cue in frames, plus the track's beat grid. */
export const makeTimeline = (map: TrackMap = DEFAULT_MAP) => {
  if (typeof map.bpm !== "number" || !(map.bpm >= 40 && map.bpm <= 300)) throw new Error(`track map ${map.name}: bpm must be 40-300 (got ${map.bpm})`);
  const bpb = map.beatsPerBar ?? 4;
  if (!Number.isInteger(bpb) || bpb < 2 || bpb > 12) throw new Error(`track map ${map.name}: beatsPerBar must be a whole number 2-12 (got ${bpb})`);
  const sections = resolveSections(map.sections);
  const spb = 60 / map.bpm; // seconds per beat
  const beat = spb * FPS; // frames per beat, not necessarily whole
  // section anchors: canonical beats → track beats (both from bar 1 beat 1)
  const C = CANON_ANCHORS.map((b) => (b - 1) * 4);
  const K = [1, ...SECTIONS.map((s) => sections[s])].map((b) => (b - 1) * bpb);
  const warp = (c: number) => {
    let i = 0;
    while (i < C.length - 2 && c >= C[i + 1]) i++;
    const r = (K[i + 1] - K[i]) / (C[i + 1] - C[i]);
    const p = K[i] + (c - C[i]) * r;
    // a stretched section keeps the hits written on a beat on a beat of the track; a squeezed one scales evenly
    return r > 1 && Number.isInteger(c) ? Math.round(p) : p;
  };
  /** Seconds → frame: rounded once, per cue (the epsilon keeps exact halves from flipping). */
  const frameOf = (s: number) => Math.round(s * FPS + 1e-7);
  /** Track position (beats since bar 1 beat 1) of a canonical bar/beat, plus `extra` canonical frames. */
  const pos = (bar: number, b = 1, extra = 0) => warp((bar - 1) * 4 + (b - 1) + extra / CANON_BEAT);
  /** Frame of a canonical bar/beat, plus `extra` canonical frames. */
  const bb = (bar: number, b = 1, extra = 0) => frameOf(pos(bar, b, extra) * spb);
  /** A duration written in canonical frames, at this tempo (fractional). */
  const d = (frames: number) => (frames * beat) / CANON_BEAT;
  /** Frames at this tempo, read in canonical frames: a scene's local clock (`k(f - cue)`). */
  const k = (frames: number) => (frames * CANON_BEAT) / beat;
  /** Frames of the track's grid, every `step` beats, in [from, to). */
  const grid = (from: number, to: number, step = 1) => {
    const out: number[] = [];
    for (let i = Math.floor(from / beat / step) - 1; ; i++) {
      const f = frameOf(i * step * spb);
      if (f >= to) return out;
      if (f >= from) out.push(f);
    }
  };
  /** Frames since the last beat of the track (fractional). */
  const sinceBeat = (frame: number) => frame - Math.floor(frame / beat + 1e-9) * beat;
  /** 0..1 decaying pulse on beat 3 of every bar of the track (accents), `len` canonical frames long. */
  const beat3 = (frame: number, len = 10) => {
    const barStart = Math.floor(frame / beat / bpb + 1e-9) * bpb;
    const x = k(frame - (barStart + 2) * beat);
    return x >= 0 && x < len ? 1 - x / len : 0;
  };

  const t = {
    map,
    name: map.name,
    sections,
    fps: FPS,
    bpm: map.bpm,
    bpb,
    spb,
    /** Frames per beat and per bar (fractional at most tempos). */
    beat,
    bar: beat * bpb,
    offsetSec: map.offsetSec ?? 0,
    pos,
    bb,
    d,
    k,
    grid,
    sinceBeat,
    beat3,
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
      crt: bb(1, 1, 6),
      lines: [
        { text: "MAGI SYSTEM / BOOT", at: bb(1, 2), fpc: d(2) },
        { text: "CONNECTING HERDR", at: bb(1, 3, 10), fpc: d(2) },
      ],
      linked: bb(2),
      footage: bb(2, 2),
      kanji: bb(2, 3),
    },
    title: { sub: bb(3, 1, 8), out: bb(5) },
    pattern: {
      sweep: bb(5, 1, 6),
      /** One hex lights per 8th from bar 5 beat 3, left to right. */
      hexes: Array.from({ length: 10 }, (_, i) => bb(5, 3 + i / 2)),
      header: bb(7),
      zoom: bb(8, 2),
    },
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
  };

  // every cue, in written order, must land on its own frame (a squeeze too hard at a fast tempo would not)
  const order: [string, number][] = [
    ["boot.crt", t.boot.crt],
    ...t.boot.lines.map((l, i): [string, number] => [`boot.line${i + 1}`, l.at]),
    ["boot.linked", t.boot.linked],
    ["boot.footage", t.boot.footage],
    ["boot.kanji", t.boot.kanji],
    ["title", t.scenes.title[0]],
    ["title.sub", t.title.sub],
    ["pattern", t.scenes.pattern[0]],
    ["pattern.sweep", t.pattern.sweep],
    ...t.pattern.hexes.map((f, i): [string, number] => [`pattern.hex${i + 1}`, f]),
    ["pattern.header", t.pattern.header],
    ["pattern.zoom", t.pattern.zoom],
    ["magi", t.magi.draw],
    ["magi.motion", t.magi.motion],
    ["magi.deliberate", t.magi.deliberate],
    ...t.magi.votes.map((f, i): [string, number] => [`magi.vote${i + 1}`, f]),
    ["magi.carried", t.magi.carried],
    ["magi.poster", t.poster],
    ["magi.riser", t.magi.riser[0]],
    ["reveal", t.reveal.push],
    ["reveal.wide", t.reveal.wide],
    ...t.reveal.callouts.map((f, i): [string, number] => [`reveal.callout${i + 1}`, f]),
    ["flow", t.flow.load],
    ["flow.sync", t.flow.sync],
    ["flow.main", t.flow.main],
    ["flow.alarm", t.flow.alarm],
    ["emergency", t.emergency.takeover],
    ["emergency.card", t.emergency.card],
    ["emergency.clear", t.emergency.clear],
    ["emergency.resolve", t.emergency.resolve],
    ["emergency.line", t.emergency.line],
    ...t.outro.lines.map((f, i): [string, number] => [`outro.line${i + 1}`, f]),
    ["outro.slam", t.outro.slam],
    ["outro.cut", t.outro.cut],
    ["end", t.total],
  ];
  for (let i = 1; i < order.length; i++) {
    if (order[i][1] <= order[i - 1][1]) {
      throw new Error(`track map ${map.name}: ${order[i][0]} (frame ${order[i][1]}) does not come after ${order[i - 1][0]} (frame ${order[i - 1][1]}); give that section more bars`);
    }
  }
  return t;
};
export type Timeline = ReturnType<typeof makeTimeline>;

/** The edit as written (tracks/house-150.json). Scenes read the current track's timeline with useTL(). */
export const T = makeTimeline(DEFAULT_MAP);

/** Frames since `at` (negative before it). */
export const since = (frame: number, at: number) => frame - at;

/** Deterministic hash noise in [0, 1) (frame-stable flicker, glitches, grain seeds). */
export const hash = (n: number) => {
  const x = Math.sin(n * 12.9898 + 78.233) * 43758.5453;
  return x - Math.floor(x);
};

/** On/off blink with `period` frames, `duty` fraction on. */
export const blink = (frame: number, period: number, duty = 0.5) => (frame % period) / period < duty;
