#!/usr/bin/env node
// Demo 3, "Night rain": a slow, melancholy jazz ballad, F minor, 72 BPM (one beat = 50 frames at 60 fps).
// Quartet: brushes, upright bass, piano comping in rootless voicings, and a lyrical alto sax lead with scoops,
// delayed vibrato and expression swells. The changes and the melody are original; they avoid the
// round-the-circle-of-fifths progression of the standards on purpose (the bass mostly moves by step: a bII
// colour, ii-V's into new keys, and a chromatic descent from F to C).
// Form (20 bars): piano intro 1-2 | A 3-10 | B 11-18 (bass walks) | tag 19-20 (final chord held).
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Song, n } from "./smf.mjs";

const song = new Song({ bpm: 72, seed: 72 });
const at = (bar, beat = 1) => song.at(bar, beat);
const S = 2 / 3; // swung 8th: long-short as 2/3 + 1/3 of a beat
const s = 1 / 3;

const sax = song.part("Alto sax", { ch: 0, program: 65, vol: 104, pan: 60, rev: 58, lag: 10 });
const pno = song.part("Piano", { ch: 1, program: 0, vol: 106, pan: 42, rev: 46, humanVel: 5 });
const bass = song.part("Upright bass", { ch: 2, program: 32, vol: 100, pan: 74, rev: 28 });
const dr = song.part("Brushes", { ch: 9, program: 40, vol: 127, pan: 66, rev: 40, human: 7, humanVel: 6 });

// Brush kit (bank 128, program 40): 38 tap, 39 slap, 40 swirl; 44 hi-hat foot, 51 ride, 36 kick
const TAP = 38, SLAP = 39, SWIRL = 40, HH = 44, RIDE = 51, KICK = 36;

// ---------------------------------------------------------------------------------------------- changes
// Rootless left-hand voicings (3-7-9 / 7-3-13 shapes around middle C) and bass roots.
const V = {
  "Fm9": [["Ab3", "C4", "Eb4", "G4"], "F2"],
  "Gbmaj7#11": [["Bb3", "C4", "Db4", "F4"], "Gb1"],
  "Ebm9": [["Gb3", "Bb3", "Db4", "F4"], "Eb2"],
  "Ab13": [["Gb3", "C4", "F4"], "Ab1"],
  "Dbmaj9": [["F3", "Ab3", "C4", "Eb4"], "Db2"],
  "C7#9b13": [["E3", "Bb3", "Eb4", "Ab4"], "C2"],
  "Abm9": [["B3", "Eb4", "Gb4", "Bb4"], "Ab1"],
  "Db13": [["B3", "F4", "Bb4"], "Db2"],
  "Gm7b5": [["F3", "Bb3", "Db4"], "G1"],
  "C7b9": [["E3", "Bb3", "Db4"], "C2"],
  "Fm(maj9)": [["Ab3", "C4", "E4", "G4"], "F1"],
  "Cm7": [["Eb3", "G3", "Bb3", "D4"], "C2"],
  "F7b9": [["A3", "Eb4", "Gb4"], "F1"],
  "Bbm9": [["Ab3", "C4", "Db4", "F4"], "Bb1"],
  "Fm9/Eb": [["Ab3", "C4", "F4", "G4"], "Eb2"],
  "Dm7b5": [["Ab3", "C4", "F4"], "D2"],
  "Db7#11": [["B3", "F4", "G4"], "Db2"],
  "C7sus4": [["Bb3", "F4", "G4"], "C2"],
  "Dbmaj7#11": [["F3", "C4", "G4"], "Db2"],
};
// bars 3-20: one or two chords (two = half a bar each)
const CHANGES = [
  ["Fm9"], ["Gbmaj7#11"], ["Ebm9", "Ab13"], ["Dbmaj9", "C7#9b13"], // A
  ["Fm9"], ["Abm9", "Db13"], ["Gbmaj7#11", "C7b9"], ["Fm(maj9)"],
  ["Dbmaj9"], ["Cm7", "F7b9"], ["Bbm9"], ["Gm7b5", "C7b9"], // B
  ["Fm9", "Fm9/Eb"], ["Dm7b5", "Db7#11"], ["C7sus4", "C7b9"], ["Fm9"],
  ["Dbmaj7#11", "C7#9b13"], ["Fm(maj9)"], // tag
];
const FIRST = 3;

// ---------------------------------------------------------------------------------------------- piano
/** Comp one chord: a soft voicing on the beat, sustained with the pedal, and a lighter re-strike later on. */
const comp = (beat, beats, name, vel = 56, restrike = 1 + S) => {
  const [v] = V[name];
  pno.note(beat, beats, v, vel).pedal(beat, beats);
  if (restrike < beats - 0.2) pno.note(beat + restrike, beats - restrike, v.slice(1), vel - 12);
};
/** High "raindrop" figure in the right hand: a few soft notes in swung 8ths or triplets. */
const drops = (beat, notes, vel = 46, step = s) =>
  notes.forEach((p, i) => pno.note(beat + i * step, step * 2.2, p, vel - i * 2));

// ---------------------------------------------------------------------------------------------- bass
/** Two-feel: root for two beats, then the fifth and a chromatic approach into the next root. */
const twoFeel = (bar, names, next) => {
  if (names.length === 2) {
    names.forEach((nm, j) => bass.note(at(bar, 1 + 2 * j), 1.9, V[nm][1], 96, { exact: j === 0 }));
    return;
  }
  const r = n(V[names[0]][1]);
  const tgt = n(V[next][1]);
  bass.note(at(bar), 1.9, r, 98, { exact: true });
  bass.note(at(bar, 3), 0.9, r + 7 > n("G2") ? r - 5 : r + 7, 84);
  bass.note(at(bar, 4), 0.9, tgt + (tgt > r ? -1 : 1), 80);
};
/** The chord's third and fifth above `r`, read from the voicing (so a half-diminished chord gets its flat five). */
const tones = (name, r) => {
  const pcs = V[name][0].map((p) => n(p) % 12);
  const has = (d) => pcs.includes((r + d) % 12);
  return { third: r + (has(4) ? 4 : has(3) ? 3 : 5), fifth: r + (has(6) && !has(7) ? 6 : 7) };
};
/** Walking quarters: root, third, fifth, a half-step approach into the next root (two chords: root, fifth each). */
const walk = (bar, names, next) => {
  const tgt = n(V[next][1]);
  let line;
  if (names.length === 2) {
    const [a, b] = names.map((nm) => n(V[nm][1]));
    const fa = tones(names[0], a).fifth;
    line = [a, fa > n("Bb2") ? fa - 12 : fa, b, tgt + (tgt > b ? -1 : 1)];
  } else {
    let r = n(V[names[0]][1]);
    if (r + 7 > n("Bb2")) r -= 12;
    const { third, fifth } = tones(names[0], r);
    line = [r, third, fifth, tgt + (tgt > fifth ? -1 : 1)];
  }
  line.forEach((p, i) => bass.note(at(bar, 1 + i), 0.92, p, i === 0 ? 96 : 84 + (i === 3 ? -6 : 0), { exact: i === 0 }));
};

// ---------------------------------------------------------------------------------------------- brushes
/** One bar of ballad brushes: a swirl on 1 and 3, taps in a swung ride pattern, slap and hat on 2 and 4. */
const brushes = (bar, lv = 1, ride = false) => {
  const b = at(bar);
  dr.note(b, 1.9, SWIRL, 78 * lv, { exact: true }).note(b + 2, 1.9, SWIRL, 70 * lv);
  for (const [o, v] of [[0, 62], [1, 72], [1 + S, 50], [2, 62], [3, 74], [3 + S, 52]]) dr.note(b + o, 0.3, TAP, v * lv);
  dr.note(b + 1, 0.3, SLAP, 56 * lv).note(b + 3, 0.3, SLAP, 60 * lv);
  dr.note(b + 1, 0.2, HH, 52 * lv).note(b + 3, 0.2, HH, 54 * lv);
  if (ride) for (const [o, v] of [[0, 50], [1, 56], [1 + S, 38], [2, 50], [3, 56], [3 + S, 40]]) dr.note(b + o, 1, RIDE, v * lv);
  if (bar % 4 === 3) dr.note(b, 0.5, KICK, 44 * lv);
};

// ---------------------------------------------------------------------------------------------- the sax
/**
 * Play a phrase of [pitch|null, beats, vel?]: expression swells on long notes, a scoop into the first note and
 * into marked ones ("~" suffix), delayed vibrato on anything a beat and a half or longer.
 */
const phrase = (beat, notes, vel = 84) => {
  let t = beat;
  let first = true;
  for (const [raw, len, v] of notes) {
    if (raw === null) {
      t += len;
      first = true;
      continue;
    }
    const scoop = first || raw.endsWith("~");
    const p = raw.replace("~", "");
    if (scoop) sax.scoop(t + sax.ms(10), -0.6, 80);
    sax.note(t, len, p, v ?? vel, { legato: 0.97 });
    const e0 = first ? 70 : 88;
    if (len >= 1.5) {
      sax.ramp(t, len * 0.45, e0, 112).ramp(t + len * 0.45, len * 0.5, 112, 86);
      sax.vibrato(t + 0.02, len * 0.95, { depth: 0.16, rate: 5, delay: 0.45 });
    } else sax.ramp(t, Math.min(len, 0.4), e0, 100);
    first = false;
    t += len;
  }
  return t;
};

// ============================================================================================== intro 1-2
song.marker(1, "Piano intro");
// rolled Fm9, then Gbmaj7#11, each with a few raindrops up high
const roll = (beat, notes, vel) => notes.forEach((p, i) => pno.note(beat + i * 0.06, 4 - i * 0.06, p, vel - i * 2, { exact: i === 0 }));
roll(at(1), ["F2", "C3", "Ab3", "Eb4", "G4", "C5"], 76);
pno.pedal(at(1), 4);
drops(at(1, 2 + S), ["G5", "F5", "C5"], 60);
drops(at(1, 4), ["Eb5", "C5"], 54, S);
roll(at(2), ["Gb2", "Db3", "Bb3", "C4", "F4", "Bb4"], 72);
pno.pedal(at(2), 3.9);
drops(at(2, 2), ["F5", "Db5", "C5"], 56);
dr.note(at(2), 3.8, SWIRL, 50).note(at(2, 3), 1.9, SWIRL, 60);
dr.note(at(2, 4), 0.3, TAP, 52);

// ============================================================================================== A 3-10, B 11-18, tag 19-20
song.marker(3, "A (sax)");
song.marker(11, "B (bass walks)");
song.marker(19, "Tag");
CHANGES.forEach((names, i) => {
  const bar = FIRST + i;
  const next = CHANGES[i + 1]?.[0] ?? "Fm9";
  const B = bar >= 11 && bar <= 18;
  const lv = bar < 11 ? 0.9 : B ? 1 + 0.04 * Math.min(bar - 11, 5) : 0.85;
  if (bar === 20) return; // the final chord, below
  // piano
  if (names.length === 1) comp(at(bar), 4, names[0], 60 * lv + 4, bar % 2 ? 1 + S : 2 + S);
  else names.forEach((nm, j) => comp(at(bar, 1 + 2 * j), 2, nm, 60 * lv + 4, 1 + S));
  // bass and brushes
  (B ? walk : twoFeel)(bar, names, next);
  brushes(bar, lv, B && bar >= 13);
});
// piano fills in the sax's breaths
drops(at(6, 4), ["C6", "Bb5", "G5"], 52);
drops(at(10, 3), ["E5", "G5", "C6", "E6"], 50);
drops(at(14, 3 + S), ["Db6", "Bb5"], 50, s);
drops(at(18, 3), ["Ab5", "G5", "Eb5", "C5"], 52);

// sax: pickup in bar 2, then A, B and the tag
const A1 = [
  ["C4", S], ["Eb4", s], // pickup (bar 2, beat 4)
  ["G4", 2], ["Ab4", S], ["G4", s], ["F4", 1], // Fm9: the ninth, a sigh down
  ["Db5~", 1], ["C5", S], ["Bb4", s], ["F4", 2], // Gbmaj7#11: down to the major seventh
  ["Gb4", S], ["Ab4", s], ["Bb4", 1], ["C5", 1], ["F5~", 1], // Ebm9 | Ab13: up to the thirteenth
  ["Eb5", 1.5], ["C5", 0.5], ["Ab4", S], ["G4", s], ["E4", 1], // Dbmaj9 | C7#9b13
];
const A2 = [
  ["F4", 2], [null, 1], ["Ab4", S], ["Bb4", s], // Fm9: home, breathe
  ["B4~", 1.5], ["Bb4", 0.5], ["Ab4", 1], ["F4", 1], // Abm9 | Db13
  ["F5~", 1.5], ["Eb5", 0.5], ["Db5", 1], ["Bb4", S], ["G4", s], // Gbmaj7#11 | C7b9
  ["Ab4", 1], ["G4", 2.5], [null, 0.5], // Fm(maj9): the ninth again, held
];
const B1 = [
  [null, S], ["Ab4", s], ["C5", S], ["Eb5", s], ["F5", 2], // Dbmaj9: rising, to the third
  ["Eb5", 1], ["D5", S], ["C5", s], ["Eb5~", 1], ["A4", 1], // Cm7 | F7b9: the tritone drop
  ["Db5", 2], ["C5", 1], ["Bb4", 1], // Bbm9
  ["F4", S], ["Bb4", s], ["Db5", 1], ["E5~", 1], ["Db5", 1], // Gm7b5 | C7b9: the flat nine on top
];
const B2 = [
  ["C5", 2], ["Bb4", S], ["Ab4", s], ["G4", 1], // Fm9 | Fm9/Eb: down with the bass
  ["F4", 1], ["Ab4", 1], ["G4~", 1], ["B4", 1], // Dm7b5 | Db7#11
  ["Bb4", 1.5], ["F4", 0.5], ["E4", 1], ["Db5", 1], // C7sus4 | C7b9
  ["C5", 3], [null, 1], // Fm9
];
const TAG = [
  ["Ab4", S], ["C5", s], ["G5~", 2], ["F5", S], ["Eb5", s], ["E5", 1], // Dbmaj7#11 | C7alt
  ["G4", 4], // Fm(maj9): the ninth, held over the final chord
];
phrase(at(2, 4), A1, 82);
phrase(at(7), A2, 84);
phrase(at(11), B1, 90);
phrase(at(15), B2, 92);
phrase(at(19), TAG, 82);

// ============================================================================================== the final chord
// Bar 20: Fm(maj9) rolled on the piano with the low F, a swirl and a soft ride swell; everything rings out.
const END = at(20);
bass.note(END, 4, "F1", 92, { exact: true });
roll(END, ["F2", "C3", "Ab3", "C4", "E4", "G4", "C5"], 70);
pno.pedal(END, 4.4);
drops(at(20, 2 + S), ["G5", "C6", "E6"], 46);
dr.note(END, 3.8, SWIRL, 64, { exact: true });
for (let i = 0; i < 16; i++) dr.note(END + i / 4, 0.4, RIDE, 34 + i * 2 - (i > 11 ? (i - 11) * 8 : 0));
sax.ramp(END + 2, 2, 100, 50);

// ---------------------------------------------------------------------------------------------- write
const HITS = [3, 5, 7, 9, 11, 13, 15, 17, 19, 20].map((b) => at(b));
song.save(dirname(fileURLToPath(import.meta.url)), "night-rain-jazz", {
  endBar: 20,
  tail: 3.5,
  meta: {
    title: "Night rain",
    key: "F minor",
    gain: 0.6,
    fadeSec: 2.5,
    reverb: { room: 0.55, damp: 0.5, width: 0.8, level: 0.5 },
    // a touch less boom from the bass around 90 Hz, a little air
    master: { eq: "equalizer=f=90:t=q:w=1:g=-2,highshelf=f=6000:g=1.5", threshold: -18, ratio: 1.6, width: 1.1 },
    hits: HITS,
    gridChecks: [{ label: "quarters", stepBeats: 1, fromBar: 3, toBar: 19 }],
    cues: [
      [1, 1, "piano intro"], [2, 4, "sax pickup"], [3, 1, "sax melody, bass and brushes in"],
      [7, 1, "second phrase"], [11, 1, "B: the bass walks"], [13, 1, "ride cymbal joins"],
      [15, 1, "last phrase, bass steps down F-Eb-D-Db-C"], [19, 1, "tag"], [20, 1, "final chord, rings out"],
    ],
  },
});
