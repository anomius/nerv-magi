#!/usr/bin/env node
// Demo 4, "Cold light": sparse ambient, A (Lydian colours), 60 BPM (one beat = 1 s = 60 frames at 60 fps).
// Sparse high piano over slow, breathing string chords and a faint choir pad, in a large hall. The voicings
// leave out thirds where they can (open fifths, ninths, major sevenths, raised fourths): cold and luminous.
// Original melody and harmony.
// Form (16 bars of 4 s, two bars a chord): strings alone 1-2 | piano 3-10 | the lift (F major 7 #11) 11-12 |
// return 13-14 | close 15-16.
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Song } from "./smf.mjs";

const song = new Song({ bpm: 60, seed: 60 });
const at = (bar, beat = 1) => song.at(bar, beat);

const pno = song.part("Piano", { ch: 0, program: 0, vol: 127, pan: 54, rev: 84, humanVel: 4 });
const hi = song.part("Strings high", { ch: 1, program: 49, vol: 74, pan: 38, rev: 90 });
const lo = song.part("Strings low", { ch: 2, program: 49, vol: 80, pan: 88, rev: 80 });
const choir = song.part("Choir pad", { ch: 3, program: 52, vol: 70, pan: 62, rev: 100 });
const glass = song.part("Bowed glass", { ch: 4, program: 92, vol: 52, pan: 70, rev: 100 });

// two bars a chord: [low strings, high strings, choir]
const CHORDS = [
  [["A2", "E3"], ["B3", "E4", "G#4"], null], // A (add9, maj7), no third
  [["F#2", "C#3"], ["A3", "E4", "G#4"], null], // F#m9
  [["D2", "A2"], ["F#3", "C#4", "G#4"], ["A3", "E4"]], // Dmaj7#11
  [["E2", "B2"], ["A3", "B3", "E4"], ["B3", "E4"]], // Esus4 (→ E in the second bar)
  [["C#2", "G#2"], ["E3", "B3", "D#4"], ["G#3", "D#4"]], // C#m9
  [["F2", "C3"], ["A3", "E4", "B4"], ["A3", "E4"]], // Fmaj7#11: the lift
  [["D2", "A2"], ["F#3", "C#4", "G#4"], ["A3", "E4"]], // Dmaj7#11
  [["A2", "E3"], ["B3", "E4"], ["A3", "E4"]], // Asus2
];

song.marker(1, "Strings").marker(3, "Piano").marker(11, "The lift").marker(13, "Return").marker(15, "Close");
CHORDS.forEach(([l, h, c], i) => {
  const bar = 1 + 2 * i;
  const b = at(bar);
  const last = i === CHORDS.length - 1;
  // each chord breathes: swells in, holds, eases back (the last one just fades)
  const vel = i === 5 ? 64 : 56;
  lo.note(b, 8, l, vel - 2).ramp(b, 3, 40, 92).ramp(b + 3, 5, 92, last ? 30 : 64);
  if (i === 3) {
    // Esus4 resolves to E in its second bar: the A moves down to G#
    hi.note(b, 4, h, vel).note(b + 4, 4, ["G#3", "B3", "E4"], vel - 2);
  } else hi.note(b, 8, h, vel);
  hi.ramp(b, 3, 36, 96).ramp(b + 3, 5, 96, last ? 24 : 60);
  if (c) choir.note(b, 8, c, 44 + (i === 5 ? 8 : 0)).ramp(b, 4, 30, 88).ramp(b + 4, 4, 88, last ? 20 : 50);
});
// the lift: a faint bowed-glass shimmer an octave up, bars 11-14
glass.note(at(11), 8, ["E5", "B5"], 40).ramp(at(11), 8, 20, 80);
glass.note(at(13), 7, ["G#5", "C#6"], 36).ramp(at(13), 7, 80, 10);

// ---------------------------------------------------------------------------------------------- piano
// [bar, beat, pitch(es), beats, vel]; first notes of phrases on the grid (exact), the rest humanized
const P = [
  [2, 3, "E5", 2, 60],
  [3, 1, "G#5", 1.5, 66], [3, 2.5, "C#6", 0.5, 60], [3, 3, "B5", 2, 64],
  [4, 1, "A5", 4, 62],
  [5, 2, "G#5", 1, 64], [5, 3, "A5", 1, 62], [5, 4, "E6", 1, 66],
  [6, 1, "C#6", 3, 64],
  [7, 1, "B5", 1.5, 66], [7, 2.5, "A5", 0.5, 60], [7, 3, "E5", 2, 62],
  [8, 3, "G#5", 2, 60],
  [9, 1, "E6", 2, 68], [9, 3, "D#6", 1, 64], [9, 4, "B5", 1, 62],
  [10, 1, "G#5", 4, 60],
  [11, 1, "B5", 2, 70], [11, 3, "E6", 2, 72],
  [12, 1, "C6", 2, 66], [12, 3, "A5", 2, 62],
  [13, 1, "G#5", 2, 66], [13, 3, "F#5", 1, 62], [13, 4, "C#6", 1, 64],
  [14, 1, "A5", 4, 60],
  [15, 1, "E6", 2, 62], [15, 3, "B5", 2, 58],
  [16, 1, ["E5", "A5"], 4, 54],
];
const PHRASE_STARTS = new Set(["3:1", "5:2", "7:1", "9:1", "11:1", "13:1", "15:1", "16:1"]);
for (const [bar, beat, p, len, v] of P) pno.note(at(bar, beat), len + 0.5, p, v, { exact: PHRASE_STARTS.has(`${bar}:${beat}`) });
// a soft low fifth under each chord change from bar 3, and the pedal held through each two-bar chord
for (let i = 1; i < CHORDS.length; i++) {
  const bar = 1 + 2 * i;
  pno.note(at(bar), 7.5, CHORDS[i][0].map((q) => q.replace(/\d$/, (d) => String(Number(d) + 1))), 44, { exact: true });
  pno.pedal(at(bar), 8);
}
pno.pedal(at(2, 3), 2); // the first note rings into bar 3, where the chord pedals take over

// ---------------------------------------------------------------------------------------------- write
const HITS = [...PHRASE_STARTS].map((k) => k.split(":").map(Number)).map(([b, bt]) => at(b, bt));
song.save(dirname(fileURLToPath(import.meta.url)), "cold-light", {
  endBar: 16,
  tail: 6,
  meta: {
    title: "Cold light",
    key: "A (Lydian colours)",
    gain: 0.7,
    fadeSec: 3,
    reverb: { room: 0.92, damp: 0.25, width: 1, level: 0.75 },
    master: { eq: "highpass=f=45,highshelf=f=7000:g=2", threshold: -14, ratio: 1.5, width: 1.2 },
    hits: HITS,
    gridChecks: [{ label: "quarters", stepBeats: 1, fromBar: 3, toBar: 16 }],
    cues: [
      [1, 1, "strings alone, swelling in"], [2, 3, "first piano note"], [3, 1, "piano phrases; a new chord every two bars"],
      [5, 1, "choir pad enters"], [11, 1, "the lift: F major 7 #11, bowed-glass shimmer"], [13, 1, "return"],
      [15, 1, "close"], [16, 1, "last piano dyad, everything fades"],
    ],
  },
});
