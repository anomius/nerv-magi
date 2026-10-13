#!/usr/bin/env node
// NERV MAGI promo score, v2: an "Eva medley" cut to the edit (src/timeline.ts: 150 BPM, 4/4, one beat
// = 24 frames, one bar = 96). Public-domain works that Evangelion made iconic, stitched together with
// an original battle march written in the genre's idiom (field snare, timpani ostinato, horns and
// trombones in octaves). No melody, harmony or rhythm of any Evangelion soundtrack is used or imitated.
//
//   bars 1-2   J. S. Bach, Cello Suite No. 1 in G major, BWV 1007 (c. 1720), Prelude, measure 1, solo cello
//              (the suite Shinji plays: Evangelion ep. 15). Its 16ths fall on our 8ths.
//   bars 3-4   tutti hit on the title card, then near silence
//   bars 5-14  original battle march in D minor; the three MAGI votes are brass stabs on bars 10-12;
//              可決 (bar 13) is a tutti D-major fanfare (Handel's "Hallelujah" was considered, but its exact
//              opening notes could not be verified here, so it is not quoted)
//   bars 15-27 the march at full; up a whole step to E minor for the climax (bars 24-27)
//   bars 28-30 EMERGENCY: dissonant low-brass clusters on the klaxon beats, timpani and taiko, wood ticks
//   bars 31-36 L. van Beethoven, Symphony No. 9 in D minor, Op. 125 (1824), finale, the "Ode to Joy"
//              theme, consequent phrase in D major (Evangelion ep. 24); NERV MAGI lands on its final chord
//
// Output: scripts/music/nerv-magi-score.mid. Rendered with FluidSynth + FluidR3_GM (MIT).
// The earlier v1 score (march + Ode only) is kept in compose-v1.mjs.
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { BARS, BPM } from "../../src/timeline.ts";
import { Track, n, writeMidi } from "./midi.mjs";

const OUT = join(dirname(fileURLToPath(import.meta.url)), "nerv-magi-score.mid");
/** Beat position of `bar` (1-based) at `beat` (1-based). */
const at = (bar, beat = 1) => (bar - 1) * 4 + (beat - 1);
const MS8 = 0.02; // ±8 ms of timing jitter at 150 BPM, in beats

// GM programs (0-based) and channels; channel 9 is percussion.
const strings = new Track("Strings", 0, 48);
const trem = new Track("Tremolo strings", 1, 44);
const basses = new Track("Contrabass", 2, 43);
const tpt = new Track("Trumpets", 3, 56);
const hn = new Track("Horns", 4, 60);
const tbn = new Track("Trombones", 5, 57);
const tuba = new Track("Tuba", 6, 58);
const timp = new Track("Timpani", 7, 47);
const hit = new Track("Orchestra hit", 8, 55);
const dr = new Track("Percussion", 9);
const choir = new Track("Choir", 10, 52);
const organ = new Track("Organ", 11, 19);
const taiko = new Track("Taiko", 12, 116);
const pizz = new Track("Pizzicato", 13, 45);
const brass = new Track("Brass section", 14, 61);
const cello = new Track("Solo cello", 15, 42);
const all = [strings, trem, basses, tpt, hn, tbn, tuba, timp, hit, dr, choir, organ, taiko, pizz, brass, cello];
// [volume, pan, reverb send]: a rough orchestral seating; choir and organ sit in a bigger room
const SEAT = {
  Strings: [100, 40, 70], "Tremolo strings": [86, 52, 70], Contrabass: [108, 88, 55], Trumpets: [98, 76, 55],
  Horns: [104, 50, 62], Trombones: [104, 86, 55], Tuba: [104, 92, 50], Timpani: [118, 64, 45],
  "Orchestra hit": [86, 64, 50], Percussion: [112, 64, 35], Choir: [127, 64, 110], Organ: [76, 64, 105],
  Taiko: [118, 64, 40], Pizzicato: [90, 36, 60], "Brass section": [104, 70, 55], "Solo cello": [127, 70, 34],
};
for (const t of all) {
  const [vol, pan, rev] = SEAT[t.name];
  t.cc(0, 7, vol).cc(0, 10, pan).cc(0, 11, 100).cc(0, 91, rev).cc(0, 93, 0);
  t.human = t === dr ? MS8 / 2 : MS8;
  t.humanVel = 6;
}

const KICK = 36, SNARE = 38, CRASH = 49, CRASH2 = 57, SPLASH = 55, RIDE = 51, WOOD_HI = 76, WOOD_LO = 77;

const CH = {
  Dm: { root: "D", tones: ["D", "F", "A"] },
  D: { root: "D", tones: ["D", "F#", "A"] },
  Bb: { root: "Bb", tones: ["Bb", "D", "F"] },
  C: { root: "C", tones: ["C", "E", "G"] },
  Gm: { root: "G", tones: ["G", "Bb", "D"] },
  A: { root: "A", tones: ["A", "C#", "E"] },
  A7: { root: "A", tones: ["A", "C#", "E", "G"] },
  B: { root: "B", tones: ["B", "D#", "F#"] },
  Em: { root: "E", tones: ["E", "G", "B"] },
};
const pc = (name, oct) => n(`${name}${oct}`);
const bassRoot = (c, o) => {
  const k = pc(CH[c].root, o);
  return k % 12 > 5 ? k - 12 : k;
};
const voicing = (c, o) => CH[c].tones.map((t, i) => pc(t, o) + (i && pc(t, o) <= pc(CH[c].tones[0], o) ? 12 : 0));
const fifthOf = (c) => bassRoot(c, 2) + 7 - (bassRoot(c, 2) + 7 > 45 ? 12 : 0);

// ---------------------------------------------------------------------------------------------
// building blocks (every roll into a big downbeat stops an 8th early: a breath before the cut)

/** Field snare, one bar: flams on 1 and 3, a 5-stroke roll into an accent on 2, a 7-stroke into 4. */
const fieldSnare = (bar, lv = 1) => {
  const b = at(bar);
  const s = (t, v, exact = false) => dr.note(b + t, 0.12, SNARE, v * lv, exact);
  s(-1 / 32, 52); s(0, 92, true); // flam on 1
  [0.5, 0.625, 0.75, 0.875].forEach((t, i) => s(t, 54 + i * 6)); // 5-stroke roll…
  s(1, 122); // …into the accent on 2
  s(1.5, 70);
  s(2 - 1 / 32, 50); s(2, 90); // flam on 3
  [2.25, 2.375, 2.5, 2.625, 2.75, 2.875].forEach((t, i) => s(t, 52 + i * 5)); // 7-stroke roll…
  s(3, 124); // …into the accent on 4
  s(3.5, 70); s(3.75, 62);
  dr.note(b, 0.5, KICK, 110 * lv, true).note(b + 2, 0.5, KICK, 88 * lv);
};
/** Timpani ostinato on tonic and dominant: T D T D with a pickup into the next bar. */
const timpOstinato = (bar, c, lv = 1) => {
  const r = bassRoot(c, 2);
  const d = fifthOf(c);
  timp.note(at(bar), 0.7, r, 112 * lv, true).note(at(bar, 2), 0.7, d, 90 * lv).note(at(bar, 3), 0.7, r, 100 * lv).note(at(bar, 4), 0.4, d, 88 * lv).note(at(bar, 4.5), 0.4, d, 96 * lv);
};
/** Tuba and contrabass on the roots (quarters, the downbeat exact). */
const roots = (bar, c, lv = 1) => {
  for (let k = 0; k < 4; k++) {
    tuba.note(at(bar) + k, 0.8, bassRoot(c, 1), (k % 2 ? 84 : 100) * lv, k === 0);
    basses.note(at(bar) + k, 0.85, bassRoot(c, 2), (k % 2 ? 80 : 96) * lv, k === 0);
  }
};
/** Trumpets answer off the beat: short chord stabs on the "and" of 2 and 4. */
const tptStabs = (bar, c, vel = 92) => {
  tpt.note(at(bar, 2.5), 0.3, voicing(c, 4), vel).note(at(bar, 4.5), 0.3, voicing(c, 4), vel - 6);
};
/** Pitch classes of the keys the march moves through (harmonic minor: the leading tone pulls up). */
const DMIN = [2, 4, 5, 7, 9, 10, 1]; // D E F G A Bb C#
const EMIN = [4, 6, 7, 9, 11, 0, 3]; // E F# G A B C D#
/** A string run: the last `count` scale notes below `to`, as 16ths landing on the next downbeat. */
const run = (bar, to, count = 8, key = DMIN) => {
  const top = n(to);
  const below = [];
  for (let p = top - 1; below.length < count && p > top - 36; p--) if (key.includes(((p % 12) + 12) % 12)) below.unshift(p);
  below.forEach((p, i) => strings.note(at(bar + 1) - (below.length - i) / 4, 0.22, p, 70 + i * 4));
  strings.note(at(bar + 1), 1, top, 104, true);
};
const tremPad = (beat, beats, c, o = 3, vel = 70, from = 60, to = 100) => {
  trem.note(beat, beats, [...voicing(c, o), ...voicing(c, o + 1)], vel);
  trem.ramp(beat, beats, 11, from, to, 12);
};
const roll = (beat, beats, v0 = 40, v1 = 118, note = SNARE) => {
  const k = beats * 8;
  for (let i = 0; i < k; i++) dr.note(beat + i / 8, 0.1, note, v0 + ((v1 - v0) * i) / k, true);
};
const timpRoll = (beat, beats, pitch, v0 = 40, v1 = 115) => {
  const k = beats * 6;
  for (let i = 0; i < k; i++) timp.note(beat + i / 6, 0.15, pitch, v0 + ((v1 - v0) * i) / k, true);
};
/** Everything on the downbeat at once (always exact: these are the cuts). */
const tutti = (beat, c, { len = 1, vel = 122, orch = true, choirToo = false } = {}) => {
  const v = voicing(c, 4);
  tpt.note(beat, len, v.map((p) => p + 12), vel, true);
  hn.note(beat, len, v, vel - 6, true);
  tbn.note(beat, len, voicing(c, 3), vel, true);
  tuba.note(beat, len, bassRoot(c, 1), vel, true);
  strings.note(beat, len * 0.6, [...voicing(c, 3), ...voicing(c, 4)], vel - 10, true);
  basses.note(beat, len, bassRoot(c, 2), vel, true);
  timp.note(beat, 1.5, bassRoot(c, 2), 127, true);
  if (orch) hit.note(beat, 0.8, bassRoot(c, 4), vel - 10, true);
  if (choirToo) choir.note(beat, len, [...voicing(c, 4), voicing(c, 4)[0] + 12], 118, true);
  dr.note(beat, 2, CRASH, 118, true).note(beat, 2, CRASH2, 108, true).note(beat, 0.5, KICK, 127, true);
};
/** A MAGI vote: a brass stab on the downbeat. */
const stab = (beat, c, vel = 120) => {
  brass.note(beat, 0.55, [...voicing(c, 4), bassRoot(c, 3)], vel, true);
  tbn.note(beat, 0.5, voicing(c, 3), vel, true);
  tpt.note(beat, 0.5, voicing(c, 5)[0], vel, true);
  timp.note(beat, 1, bassRoot(c, 2), 120, true);
  dr.note(beat, 1, SPLASH, 104, true).note(beat, 0.4, KICK, 120, true);
};
/** The theme in horns and trombones in octaves (trumpets double an octave up when `tptToo`). */
const theme = (beat, notes, { vel = 104, tptToo = false, cresc = 10 } = {}) => {
  const total = notes.reduce((s, [, l]) => s + l, 0);
  let t = beat;
  for (const [p, len] of notes) {
    const v = vel - cresc / 2 + (cresc * (t - beat)) / total; // a crescendo through each phrase
    hn.note(t, len * 0.9, p, v);
    tbn.note(t, len * 0.9, n(p) - 12, v - 4);
    if (tptToo) tpt.note(t, len * 0.88, n(p) + 12, v - 2);
    t += len;
  }
};

// ---------------------------------------------------------------------------------------------
// 1-2 power-on: Bach, Cello Suite No. 1, BWV 1007, Prelude, measure 1 (its 16ths on our 8ths)
const preludeM1 = ["G2", "D3", "B3", "A3", "B3", "D3", "B3", "D3"];
[...preludeM1, ...preludeM1].forEach((p, i) => {
  const accent = i % 8 === 0 ? 10 : 0; // lean on the bass G of each half
  cello.note(at(1) + i / 2, i % 8 === 0 ? 0.9 : 0.5, p, 104 + accent + i * 0.6);
});
cello.ramp(at(1), 8, 11, 116, 127, 16);
// a cymbal swell under the second half, stopping an 8th before the cut
roll(at(2, 2), 2.5, 12, 84, RIDE);

// 3-4 title card: one hit on the smash cut, then near silence
tutti(at(3), "Dm", { len: 1.2 });
strings.note(at(3, 2), 6.5, ["A5", "D6"], 32);
basses.note(at(3, 2), 6.5, "D2", 38);
timp.note(at(4), 1, "D2", 50, true);
roll(at(4, 3), 1.5, 30, 112);

// 5-12 the battle march (D minor); MAGI votes on 10, 11, 12
const marchA = ["Dm", "Dm", "Bb", "A", "Dm", "Bb", "Gm", "A"];
tutti(at(5), "Dm");
marchA.forEach((c, i) => {
  const bar = 5 + i;
  if (bar !== 8) fieldSnare(bar, bar < 7 ? 0.9 : 1);
  timpOstinato(bar, c);
  roots(bar, c);
  if (bar >= 7) tptStabs(bar, c, bar >= 9 ? 98 : 88);
  tremPad(at(bar), 4, c, 3, 64, 70, 96);
  if (bar % 2 === 1) dr.note(at(bar), 2, CRASH, 104, true); // a crash at every phrase start
});
theme(at(5), [
  ["D4", 0.75], ["D4", 0.25], ["A4", 1], ["A4", 0.75], ["G4", 0.25], ["F4", 0.75], ["E4", 0.25],
  ["D4", 1.5], ["E4", 0.5], ["F4", 0.75], ["G4", 0.25], ["A4", 1],
  ["Bb4", 0.75], ["A4", 0.25], ["Bb4", 0.75], ["C5", 0.25], ["D5", 2],
  ["C#5", 1.5], ["B4", 0.5], ["C#5", 0.75], ["D5", 0.25], ["E5", 1],
]);
// pizzicato climbs with the hexes (8ths from bar 5 beat 3)
["D4", "F4", "A4", "D5", "E5", "F5", "A5", "C6", "D6", "F6"].forEach((p, i) => pizz.note(at(5, 3) + i / 2, 0.4, p, 72 + i * 3));
// bar 7: パターン検出 — a stab
stab(at(7), "Bb", 112);
// bar 8: the dive — snare roll, scale run into bar 9, timpani roll
roll(at(8), 3.5, 50, 124);
timpRoll(at(8), 3.5, "A1", 50, 118);
run(8, "D6", 16);
// 9-12: MAGI
tutti(at(9), "Dm", { len: 0.8 });
theme(at(9), [
  ["D5", 1.5], ["A4", 0.5], ["D5", 0.75], ["E5", 0.25], ["F5", 1],
  ["F5", 2], ["D5", 0.75], ["C5", 0.25], ["Bb4", 1],
  ["G4", 0.75], ["A4", 0.25], ["Bb4", 0.75], ["C5", 0.25], ["D5", 1], ["G5", 1],
  ["E5", 3], ["C#5", 1],
], { vel: 108 });
stab(at(10), "Bb");
stab(at(11), "Gm");
stab(at(12), "A");
run(12, "D5", 8);

// 13 可決 3/3: a tutti D-major fanfare, choir included; a dotted brass call answers on 3-4
tutti(at(13), "D", { len: 1.6, choirToo: true });
tpt.line(at(13, 3), [["D5", 0.75], ["D5", 0.25], ["F#5", 0.75], ["A5", 0.25]], 112, 0.9);
hn.line(at(13, 3), [["A4", 0.75], ["A4", 0.25], ["D5", 0.75], ["F#5", 0.25]], 104, 0.9);
// 14: the riser into the reveal
tpt.note(at(14), 3.4, "A5", 100).ramp(at(14), 3.5, 11, 60, 127);
trem.note(at(14), 3.5, ["A3", "C#4", "E4", "G4", "A4"], 82);
roll(at(14), 3.5, 40, 127);
roll(at(14), 3.5, 20, 110, RIDE);
timpRoll(at(14), 3.5, "A1", 40, 122);
run(14, "D6", 12);

// 15-27 reveal and view flow: the march at full; up a whole step to E minor for the last four bars
const marchB = ["Dm", "Dm", "Bb", "C", "Gm", "A", "Dm", "Bb", "B", "Em", "Em", "C", "B"];
tutti(at(15), "Dm", { len: 0.8, orch: false });
marchB.forEach((c, i) => {
  const bar = 15 + i;
  fieldSnare(bar, 1);
  timpOstinato(bar, c);
  roots(bar, c, 1.05);
  tptStabs(bar, c, 96);
  strings.note(at(bar), 4, voicing(c, 3), 64);
  dr.note(at(bar, 3), 1, SPLASH, 84); // accents on beat 3 (the callouts' pulse)
  if (i % 2 === 0) dr.note(at(bar), 2, CRASH, 112, true); // every camera move / callout lands on a crash
  if (i % 2 === 1 && bar < 23) run(bar, voicing(marchB[i + 1], 5)[0], 8, DMIN);
});
theme(at(15), [
  // 15-16 Dm
  ["D4", 1.5], ["A3", 0.5], ["D4", 0.75], ["E4", 0.25], ["F4", 1],
  ["G4", 0.75], ["F4", 0.25], ["E4", 0.75], ["D4", 0.25], ["A4", 2],
  // 17-18 Bb C (a third higher)
  ["F4", 1.5], ["C4", 0.5], ["F4", 0.75], ["G4", 0.25], ["A4", 1],
  ["Bb4", 0.75], ["A4", 0.25], ["G4", 0.75], ["F4", 0.25], ["C5", 2],
  // 19-20 Gm A
  ["Bb4", 1.5], ["G4", 0.5], ["Bb4", 0.75], ["C5", 0.25], ["D5", 1],
  ["E5", 0.75], ["D5", 0.25], ["C#5", 0.75], ["B4", 0.25], ["A4", 2],
  // 21-22 Dm Bb
  ["D5", 1.5], ["A4", 0.5], ["D5", 0.75], ["E5", 0.25], ["F5", 1],
  ["G5", 0.75], ["F5", 0.25], ["E5", 0.75], ["D5", 0.25], ["F5", 2],
], { vel: 106 });
// 23: pivot on B (the dominant of E minor); 24-27: the climax a whole step up, trumpets on top
theme(at(23), [["F#4", 1.5], ["B3", 0.5], ["D#4", 0.75], ["E4", 0.25], ["F#4", 1]], { vel: 110 });
roll(at(23, 3), 1.5, 60, 124);
tutti(at(24), "Em", { len: 0.8, orch: false });
theme(at(24), [
  ["E4", 1.5], ["B3", 0.5], ["E4", 0.75], ["F#4", 0.25], ["G4", 1],
  ["A4", 0.75], ["G4", 0.25], ["F#4", 0.75], ["E4", 0.25], ["B4", 2],
  ["G4", 1.5], ["E4", 0.5], ["G4", 0.75], ["A4", 0.25], ["B4", 1],
  ["F#4", 2], ["D#4", 1], ["B3", 1],
], { vel: 114, tptToo: true });
// the wipes (25, 26, 27): a hit on each downbeat
["Em", "C", "B"].forEach((c, i) => tutti(at(25 + i), c, { len: 0.6 }));
run(26, "B5", 8, EMIN);

// 28-30 EMERGENCY: low-brass clusters stab on every klaxon beat (minor 2nds, a tritone), timpani and
// taiko pulse, wood ticks in 8ths, a high string cluster; a swell, a breath, then NOMINAL on bar 31
dr.note(at(28), 2, CRASH, 127, true).note(at(28), 2, CRASH2, 120, true);
hit.note(at(28), 0.8, "D4", 118, true);
for (let b = 28; b <= 30; b++) {
  for (let k = 1; k <= 4; k++) {
    if (b === 30 && k > 2) break;
    const odd = k % 2 === 1;
    const cluster = odd ? ["D2", "Eb2", "Ab2"] : ["Db2", "D2", "G#2"];
    tbn.note(at(b, k), 0.45, cluster, k === 1 ? 124 : 108, true);
    tuba.note(at(b, k), 0.45, odd ? "D1" : "Db1", 112, true);
    hn.note(at(b, k), 0.45, odd ? ["D3", "Ab3"] : ["Eb3", "A3"], 100, true);
    taiko.note(at(b, k), 0.8, odd ? "C2" : "G1", k === 1 ? 127 : 104, true);
    timp.note(at(b, k), 0.7, odd ? "D2" : "Ab1", k === 1 ? 124 : 100, true);
  }
  for (let e = 0; e < 8; e++) dr.note(at(b) + e / 2, 0.1, e % 2 ? WOOD_LO : WOOD_HI, e % 2 ? 70 : 92);
  if (b < 30) trem.note(at(b), 3.9, ["A4", "Bb4", "Eb5", "E5"], 70).ramp(at(b), 4, 11, 60, 110);
  dr.note(at(b), 0.5, KICK, 120, true);
}
tbn.note(at(30, 3), 1.5, ["A2", "C#3", "G3"], 104);
trem.note(at(30, 3), 1.5, ["A4", "C#5", "E5", "G5"], 84).ramp(at(30, 3), 1.5, 11, 60, 127);
roll(at(30), 3.5, 20, 124, RIDE);
roll(at(30, 3), 1.5, 50, 124);
timpRoll(at(30, 3), 1.5, "A1", 60, 124);

// 31-36 NOMINAL → outro: Beethoven, Symphony No. 9, Op. 125, "Ode to Joy" (public domain), the theme's
// consequent phrase in D major: F# F# G A | A G F# E | D D E F# | E. D D — choir, strings, organ, brass
tutti(at(31), "D", { len: 1, vel: 118, orch: false });
const ode = [
  ["F#4", 1], ["F#4", 1], ["G4", 1], ["A4", 1],
  ["A4", 1], ["G4", 1], ["F#4", 1], ["E4", 1],
  ["D4", 1], ["D4", 1], ["E4", 1], ["F#4", 1],
  ["E4", 1.5], ["D4", 0.5], ["D4", 2],
];
choir.line(at(31), ode, 124, 0.97);
strings.line(at(31), ode.map(([p, l]) => [n(p) + 12, l]), 112, 0.97);
tpt.line(at(32), ode.slice(4).map(([p, l]) => [n(p) + 12, l]), 102, 0.95);
hn.line(at(31), ode.map(([p, l]) => [n(p), l]), 96, 0.95);
const odeChords = [
  [at(31), 4, "D"],
  [at(32), 2, "D"], [at(32, 3), 2, "A"],
  [at(33), 2, "D"], [at(33, 3), 1, "A"], [at(33, 4), 1, "D"],
  [at(34), 2, "A7"],
];
for (const [b, len, c] of odeChords) {
  organ.note(b, len, [...voicing(c, 3), bassRoot(c, 2)], 72);
  tbn.note(b, len, voicing(c, 3), 90);
  tuba.note(b, len, bassRoot(c, 1), 94);
  basses.note(b, len, bassRoot(c, 2), 96);
  trem.note(b, len, voicing(c, 4), 56);
}
timpRoll(at(31, 2), 1.5, "D2", 30, 70);
timp.note(at(32, 3), 1, "A1", 90).note(at(33), 1, "D2", 96).note(at(33, 3), 1, "A1", 96);
// the final chord (bar 34 beat 3): NERV MAGI slams; it rings through bar 35, then a silent half bar
const fin = at(34, 3);
const ring = at(36, 3) - fin - 0.05;
tutti(fin, "D", { len: 1.5 });
choir.note(fin, ring, ["D4", "F#4", "A4", "D5"], 118, true);
organ.note(fin, ring, ["D2", "D3", "F#3", "A3", "D4"], 86, true);
strings.note(fin, ring, ["D3", "A3", "D4", "F#4", "A4", "D5"], 92, true);
hn.note(fin + 1.5, ring - 1.5, ["D4", "F#4", "A4"], 88);
basses.note(fin, ring, ["D1", "D2"], 104, true);
timpRoll(fin + 1.5, ring - 1.6, "D2", 90, 40);
for (const t of [choir, organ, strings, hn, basses, trem]) t.ramp(fin + 2, ring - 2, 11, 100, 70, 12);

writeFileSync(OUT, writeMidi(BPM, all));
const notes = all.reduce((s, t) => s + t.events.filter((e) => (e[2][0] & 0xf0) === 0x90).length, 0);
console.log(`music: ${OUT} (${BARS} bars at ${BPM} BPM, ${(BARS * 4 * 60) / BPM} s, ${notes} notes, ${all.length} tracks, v2 medley)`);
