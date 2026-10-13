#!/usr/bin/env node
// v1 score, kept for reference (the promo now uses compose.mjs, the v2 Eva medley):
// an original militaristic march in D minor at 150 BPM, cut to the edit (src/timeline.ts), ending on
// the consequent phrase of Beethoven's "Ode to Joy" (Symphony No. 9, 1824, public domain).
// Output: scripts/music/nerv-magi-score-v1.mid. Render it the same way as the v2 score.
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { BARS, BPM } from "../../src/timeline.ts";
import { Track, n, writeMidi } from "./midi.mjs";

const OUT = join(dirname(fileURLToPath(import.meta.url)), "nerv-magi-score-v1.mid");
/** Beat position of `bar` (1-based) at `beat` (1-based). */
const at = (bar, beat = 1) => (bar - 1) * 4 + (beat - 1);

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
const all = [strings, trem, basses, tpt, hn, tbn, tuba, timp, hit, dr, choir, organ, taiko, pizz, brass];
const MIX = { Strings: 100, "Tremolo strings": 88, Contrabass: 108, Trumpets: 100, Horns: 96, Trombones: 100, Tuba: 104, Timpani: 118, "Orchestra hit": 92, Percussion: 112, Choir: 108, Organ: 72, Taiko: 118, Pizzicato: 96, "Brass section": 104 };
for (const t of all) t.cc(0, 7, MIX[t.name]).cc(0, 11, 100).cc(0, 91, t === dr || t === taiko ? 30 : 50);

const KICK = 36, SNARE = 38, CRASH = 49, CRASH2 = 57, SPLASH = 55, RIDE = 51, TOM_LO = 41, TOM_HI = 48;

// chords (root position voicings) used by the ostinati and stabs
const CH = {
  Dm: { root: "D", tones: ["D", "F", "A"] },
  D: { root: "D", tones: ["D", "F#", "A"] },
  Bb: { root: "Bb", tones: ["Bb", "D", "F"] },
  C: { root: "C", tones: ["C", "E", "G"] },
  Gm: { root: "G", tones: ["G", "Bb", "D"] },
  A: { root: "A", tones: ["A", "C#", "E"] },
  A7: { root: "A", tones: ["A", "C#", "E", "G"] },
};
const pc = (name, oct) => n(`${name}${oct}`);
/** Root in octave `o`, folded so roots above F sit an octave lower (keeps the bass line tight). */
const bassRoot = (c, o) => {
  const k = pc(CH[c].root, o);
  return k % 12 > 5 ? k - 12 : k;
};
const voicing = (c, o) => CH[c].tones.map((t, i) => pc(t, o) + (i && pc(t, o) <= pc(CH[c].tones[0], o) ? 12 : 0));

// ---------------------------------------------------------------------------------------------
// building blocks (every roll into a big downbeat stops an 8th early: a breath before the cut)

/** Military snare ostinato for one bar (16ths), accents on 1 and 3; `lv` scales velocity. */
const snareBar = (bar, lv = 1) => {
  const pat = [1, 0, 0.7, 0.7, 0.9, 0, 0.75, 0, 1, 0.6, 0.65, 0.7, 0.9, 0, 0.8, 0.8];
  pat.forEach((v, i) => v && dr.note(at(bar) + i / 4, 0.2, SNARE, (58 + 62 * v) * lv));
  dr.note(at(bar), 0.5, KICK, 104 * lv).note(at(bar, 3), 0.5, KICK, 92 * lv);
};
/** Snare roll (32nds) crescendo over `beats` from `beat`. */
const roll = (beat, beats, v0 = 40, v1 = 118, note = SNARE) => {
  const k = beats * 8;
  for (let i = 0; i < k; i++) dr.note(beat + i / 8, 0.1, note, v0 + ((v1 - v0) * i) / k);
};
const timpRoll = (beat, beats, pitch, v0 = 40, v1 = 115) => {
  const k = beats * 6;
  for (let i = 0; i < k; i++) timp.note(beat + i / 6, 0.15, pitch, v0 + ((v1 - v0) * i) / k);
};
/** Low-brass march ostinato (8ths) on chord `c` for one bar. */
const ostinato = (bar, c, vel = 92) => {
  const r = bassRoot(c, 2);
  const fifth = r + 7;
  const pat = [r, r, r - 12, r, r, r, fifth, r];
  pat.forEach((p, i) => {
    tuba.note(at(bar) + i / 2, 0.42, p - 12, vel - (i % 2) * 12);
    tbn.note(at(bar) + i / 2, 0.42, p, vel - 6 - (i % 2) * 14);
  });
};
/** A full-orchestra hit on `beat`: chord `c`, timpani, cymbals, kick (and the GM orchestra hit). */
const tutti = (beat, c, { len = 1, vel = 122, orch = true } = {}) => {
  const v = voicing(c, 4);
  tpt.note(beat, len, v.map((p) => p + 12), vel);
  hn.note(beat, len, v, vel - 6);
  tbn.note(beat, len, voicing(c, 3), vel);
  tuba.note(beat, len, bassRoot(c, 1), vel);
  strings.note(beat, len * 0.6, [...voicing(c, 3), ...voicing(c, 4)], vel - 10);
  basses.note(beat, len, bassRoot(c, 2), vel);
  timp.note(beat, 1.5, bassRoot(c, 2), 127);
  if (orch) hit.note(beat, 0.8, bassRoot(c, 4), vel - 8);
  dr.note(beat, 2, CRASH, 118).note(beat, 2, CRASH2, 108).note(beat, 0.5, KICK, 127);
};
/** Short brass-section stab (MAGI votes). */
const stab = (beat, c, vel = 118) => {
  brass.note(beat, 0.55, [...voicing(c, 4), bassRoot(c, 3)], vel);
  tpt.note(beat, 0.5, voicing(c, 5)[0], vel);
  timp.note(beat, 1, bassRoot(c, 2), 118);
  dr.note(beat, 1, SPLASH, 100).note(beat, 0.4, KICK, 118);
};
/** Tremolo string pad for `beats` with an expression swell. */
const pad = (beat, beats, c, o = 3, vel = 70, from = 50, to = 100) => {
  trem.note(beat, beats, [...voicing(c, o), ...voicing(c, o + 1)], vel);
  trem.ramp(beat, beats, 11, from, to, 12);
};

// ---------------------------------------------------------------------------------------------
// 1-2 power-on: sub pedal, tremolo, a cymbal and timpani crescendo into the title hit; no beat
basses.note(at(1), 8, ["D1", "D2"], 72).ramp(at(1), 8, 11, 40, 100);
trem.note(at(1), 8, ["D3", "A3"], 50).ramp(at(1), 8, 11, 25, 92);
// the crescendo stops an 8th early: a breath, then the smash cut lands
roll(at(2), 3.5, 20, 100, RIDE);
timpRoll(at(2), 3.5, "D2", 25, 110);
roll(at(2, 3), 1.5, 50, 120);

// 3-4 title card: one hit on the smash cut, then near silence (a high, thin string harmonic)
tutti(at(3), "Dm", { len: 1.2 });
strings.note(at(3, 2), 6.5, ["A5", "D6"], 34);
basses.note(at(3, 2), 6.5, "D2", 40);
timp.note(at(4), 1, "D2", 52);
roll(at(4, 3), 1.5, 30, 112);

// 5-8 pattern detection: the march enters; pizzicato climbs with the hexes (8ths from bar 5 beat 3)
tutti(at(5), "Dm");
for (let b = 5; b <= 7; b++) snareBar(b, b === 5 ? 0.85 : 1);
["Dm", "Dm", "Bb", "A"].forEach((c, i) => ostinato(5 + i, c, 88));
pad(at(5), 12, "Dm", 3, 64, 40, 95);
["D4", "F4", "A4", "D5", "E5", "F5", "A5", "C6", "D6", "F6"].forEach((p, i) => pizz.note(at(5, 3) + i / 2, 0.4, p, 92 + i * 3));
stab(at(7), "Bb");
hn.note(at(7), 4, ["Bb3", "D4", "F4"], 84);
// the dive: snare roll, a two-octave run, timpani
roll(at(8), 3.5, 50, 124);
timpRoll(at(8), 3.5, "A1", 50, 118);
["D4", "E4", "F4", "G4", "A4", "Bb4", "C#5", "D5", "E5", "F5", "G5", "A5", "Bb5", "C#6", "D6", "E6"].forEach((p, i) =>
  strings.note(at(8) + i / 4, 0.24, p, 70 + i * 3),
);
pad(at(8), 4, "A", 3, 70, 60, 115);

// 9-14 MAGI: a brass line over the march; the three votes land on consecutive downbeats
const magiChords = ["Dm", "Bb", "Gm", "A", "D", "A7"];
tutti(at(9), "Dm", { len: 0.8 });
magiChords.slice(0, 5).forEach((c, i) => {
  snareBar(9 + i, 0.92);
  ostinato(9 + i, c === "A7" ? "A" : c, 90);
  pad(at(9 + i), 4, c, 3, 62, 70, 90);
});
const magiTheme = [
  // bar 9 Dm
  ["A4", 1], ["D5", 1.5], ["E5", 0.5], ["F5", 1],
  // bar 10 Bb (MELCHIOR votes)
  ["F5", 2], ["D5", 1], ["Bb4", 1],
  // bar 11 Gm (BALTHASAR)
  ["G4", 1.5], ["A4", 0.5], ["Bb4", 1], ["D5", 1],
  // bar 12 A (CASPER)
  ["C#5", 3], ["E5", 1],
  // bar 13 D: 可決
  ["D5", 2], ["F#5", 1], ["A5", 1],
];
tpt.line(at(9), magiTheme, 100, 0.92);
hn.line(at(9), magiTheme.map(([p, l]) => [n(p) - 12, l]), 90, 0.92);
stab(at(10), "Bb");
stab(at(11), "Gm");
stab(at(12), "A");
tutti(at(13), "D", { len: 2 });
// 14: the riser into the reveal
roll(at(14), 3.5, 40, 127);
roll(at(14), 3.5, 20, 110, RIDE);
timpRoll(at(14), 3.5, "A1", 40, 122);
tpt.note(at(14), 3.8, "A5", 96).ramp(at(14), 4, 11, 60, 127);
trem.note(at(14), 4, ["A3", "C#4", "E4", "G4", "A4"], 80);
["A3", "B3", "C#4", "D4", "E4", "F4", "G4", "A4", "Bb4", "C#5", "D5", "E5", "F5", "G5", "A5", "C#6"].forEach((p, i) =>
  strings.note(at(14, 3) + i / 8, 0.12, p, 80 + i * 2),
);

// 15-24 reveal and callouts: the march at full; cymbal on every callout bar line, accents on beat 3
const revealChords = ["Dm", "Dm", "Bb", "C", "Gm", "A", "Dm", "Bb", "Gm", "A"];
revealChords.forEach((c, i) => {
  const bar = 15 + i;
  snareBar(bar, 1);
  ostinato(bar, c, 96);
  strings.note(at(bar), 4, voicing(c, 3), 66);
  timp.note(at(bar), 0.8, bassRoot(c, 2), 104).note(at(bar, 3), 0.8, bassRoot(c, 2) + 7 > 45 ? bassRoot(c, 2) - 5 : bassRoot(c, 2) + 7, 96);
  dr.note(at(bar, 3), 1, SPLASH, 84);
  if (i % 2 === 0) dr.note(at(bar), 2, CRASH, 112);
  // strings' running 8ths (chord tones, up then down)
  const v = [...voicing(c, 4), voicing(c, 5)[0]];
  [0, 1, 2, 3, 2, 1, 0, 1].forEach((k, j) => strings.note(at(bar) + j / 2, 0.45, v[k], 58 + (j % 2 ? 0 : 10)));
});
tutti(at(15), "Dm", { len: 0.8, orch: false });
const strain = [
  // 15-16 Dm
  ["D5", 1], ["D5", 0.5], ["E5", 0.5], ["F5", 1], ["A5", 1],
  ["G5", 1.5], ["F5", 0.5], ["E5", 2],
  // 17-18 Bb C
  ["F5", 1.5], ["G5", 0.5], ["F5", 1], ["D5", 1],
  ["E5", 3], ["C5", 1],
  // 19-20 Gm A
  ["D5", 1.5], ["E5", 0.5], ["F5", 1], ["G5", 1],
  ["A5", 3], ["E5", 1],
  // 21-22 Dm Bb
  ["F5", 1], ["E5", 0.5], ["D5", 0.5], ["A5", 1], ["D6", 1],
  ["Bb5", 3], ["A5", 1],
  // 23-24 Gm A
  ["G5", 1], ["F5", 0.5], ["E5", 0.5], ["D5", 1], ["Bb4", 1],
  ["C#5", 3], ["A4", 1],
];
tpt.line(at(15), strain, 104, 0.9);
hn.line(at(15), strain.map(([p, l]) => [n(p) - 12, l]), 94, 0.9);
roll(at(24, 3), 1.5, 60, 122);

// 25-27 view flow: a hit on every wipe (downbeats); the march thins; a fill into the alarm
["Dm", "Bb", "A"].forEach((c, i) => {
  tutti(at(25 + i), c, { len: 0.6 });
  ostinato(25 + i, c, 96);
  snareBar(25 + i, 0.95);
  pad(at(25 + i), 4, c, 3, 70, 70, 100);
});
[TOM_HI, TOM_HI, TOM_LO, TOM_LO].forEach((t, i) => dr.note(at(27, 4) + i / 4, 0.2, t, 64 + i * 6));

// 28-30 EMERGENCY: the march stops; taiko and timpani pulse under a dissonant low-brass cluster
// (the klaxon itself is in the SFX stem); a cymbal swell resolves into NOMINAL on bar 31
for (let b = 28; b <= 30; b++) {
  for (let k = 1; k <= 4; k++) taiko.note(at(b, k), 0.8, k % 2 ? "C2" : "G1", k === 1 ? 127 : 104);
  timp.note(at(b), 1, "D2", 122).note(at(b, 3), 1, "Ab1", 110);
  dr.note(at(b), 0.5, KICK, 120);
  if (b < 30) {
    tbn.note(at(b), 3.9, ["D2", "Eb2", "Ab2"], 104).ramp(at(b), 4, 11, 127, 70);
    tuba.note(at(b), 3.9, "D1", 110);
    hn.note(at(b), 3.9, ["D3", "Ab3"], 92);
    trem.note(at(b), 3.9, ["Eb5", "D5", "A4"], 74);
  }
}
// the takeover lands on the downbeat: everything hits at once
dr.note(at(28), 2, CRASH, 127).note(at(28), 2, CRASH2, 120);
hit.note(at(28), 0.8, "D4", 120);
tbn.note(at(30), 1.9, ["D2", "Eb2", "Ab2"], 108);
tuba.note(at(30), 1.9, "D1", 110);
// the swell stops an 8th early: a breath, then NOMINAL lands
tbn.note(at(30, 3), 1.5, ["A2", "C#3", "G3"], 100);
trem.note(at(30, 3), 1.5, ["A4", "C#5", "E5", "G5"], 80).ramp(at(30, 3), 1.5, 11, 60, 127);
roll(at(30), 3.5, 20, 124, RIDE);
roll(at(30, 3), 1.5, 50, 124);
timpRoll(at(30, 3), 1.5, "A1", 60, 124);

// 31-34 NOMINAL → outro: Beethoven, Symphony No. 9, "Ode to Joy" (public domain), the theme's
// consequent phrase in D major — F# F# G A | A G F# E | D D E F# | E. D D — broad: choir, strings, organ
const ode = [
  ["F#4", 1], ["F#4", 1], ["G4", 1], ["A4", 1],
  ["A4", 1], ["G4", 1], ["F#4", 1], ["E4", 1],
  ["D4", 1], ["D4", 1], ["E4", 1], ["F#4", 1],
  ["E4", 1.5], ["D4", 0.5], ["D4", 2],
];
choir.cc(at(31), 7, 127).line(at(31), ode, 124, 0.97);
strings.line(at(31), ode.map(([p, l]) => [n(p) + 12, l]), 112, 0.97);
// the brass take the tune for the climb into the final cadence
tpt.line(at(32), ode.slice(4).map(([p, l]) => [n(p) + 12, l]), 104, 0.95);
hn.line(at(31), ode.map(([p, l]) => [n(p), l]), 96, 0.95);
// harmony: D | D A | D A D | A7 D
const odeChords = [
  [at(31), 4, "D"],
  [at(32), 2, "D"], [at(32, 3), 2, "A"],
  [at(33), 2, "D"], [at(33, 3), 1, "A"], [at(33, 4), 1, "D"],
  [at(34), 2, "A7"],
];
for (const [b, len, c] of odeChords) {
  organ.note(b, len, [...voicing(c, 3), bassRoot(c, 2)], 70);
  tbn.note(b, len, voicing(c, 3), 92);
  tuba.note(b, len, bassRoot(c, 1), 96);
  basses.note(b, len, bassRoot(c, 2), 96);
  trem.note(b, len, voicing(c, 4), 56);
}
// NOMINAL: the Ode lands on the downbeat with the whole orchestra
tutti(at(31), "D", { len: 1, vel: 118, orch: false });
timpRoll(at(31, 2), 1.5, "D2", 30, 70);
timp.note(at(32, 3), 1, "A1", 90).note(at(33), 1, "D2", 96).note(at(33, 3), 1, "A1", 96);

// the final chord (bar 34 beat 3): NERV MAGI slams; it rings through bar 35, then a silent half bar
const fin = at(34, 3);
const ring = at(36, 3) - fin - 0.05;
tutti(fin, "D", { len: 1.5 });
choir.note(fin, ring, ["D4", "F#4", "A4", "D5"], 112);
organ.note(fin, ring, ["D2", "D3", "F#3", "A3", "D4"], 84);
strings.note(fin, ring, ["D3", "A3", "D4", "F#4", "A4", "D5"], 92);
hn.note(fin + 1.5, ring - 1.5, ["D4", "F#4", "A4"], 88);
basses.note(fin, ring, ["D1", "D2"], 104);
timpRoll(fin + 1.5, ring - 1.6, "D2", 90, 40);
for (const t of [choir, organ, strings, hn, basses, trem]) t.ramp(fin + 2, ring - 2, 11, 100, 70, 12);

writeFileSync(OUT, writeMidi(BPM, all));
const notes = all.reduce((s, t) => s + t.events.filter((e) => (e[2][0] & 0xf0) === 0x90).length, 0);
console.log(`music: ${OUT} (${BARS} bars at ${BPM} BPM, ${(BARS * 4 * 60) / BPM} s, ${notes} notes, ${all.length} tracks)`);
