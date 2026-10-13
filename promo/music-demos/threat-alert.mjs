#!/usr/bin/env node
// Demo 2, "Threat alert": tension and threat, centred on E, 120 BPM (one beat = 30 frames at 60 fps). The idiom
// of an attack cue: a 16th-note ostinato in low strings and piano accented 3+3+3+3+4, dissonant low brass
// (minor seconds, tritones) swelling every two bars, a pulsing klaxon-like figure (square lead and muted
// trumpet, every pulse falling off in pitch), timpani and taiko, cymbal swells, and a build that climbs a
// semitone every two bars into one crash. All material is original.
// Form (28 bars): drone 1-2 | ostinato 3-6 | klaxon 7-10 | brass and choir 11-14 | the climb 15-22 |
// crash 23-24 | aftermath 25-28.
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Song, n } from "./smf.mjs";

const song = new Song({ bpm: 120, seed: 120 });
const at = (bar, beat = 1) => song.at(bar, beat);

const lowStr = song.part("Low strings", { ch: 0, program: 48, vol: 124, pan: 84, rev: 44 });
const cb = song.part("Contrabass", { ch: 1, program: 43, vol: 104, pan: 96, rev: 40 });
const pno = song.part("Piano", { ch: 2, program: 0, vol: 106, pan: 52, rev: 46 });
const tbn = song.part("Trombones", { ch: 3, program: 57, vol: 108, pan: 78, rev: 54 });
const tuba = song.part("Tuba", { ch: 4, program: 58, vol: 100, pan: 90, rev: 48 });
const hn = song.part("Horns", { ch: 5, program: 60, vol: 106, pan: 42, rev: 60 });
const klax = song.part("Klaxon (square lead)", { ch: 6, program: 80, vol: 66, pan: 64, rev: 50, human: 3 });
const mtp = song.part("Klaxon (muted trumpets)", { ch: 7, program: 59, vol: 88, pan: 70, rev: 52, human: 3 });
const timp = song.part("Timpani", { ch: 8, program: 47, vol: 120, pan: 56, rev: 50, human: 4 });
const dr = song.part("Bass drum, snare, cymbals", { ch: 9, program: 48, vol: 110, pan: 64, rev: 46, human: 4 });
const taiko = song.part("Taiko", { ch: 10, program: 116, vol: 112, pan: 58, rev: 40, human: 4 });
const trem = song.part("Tremolo strings", { ch: 11, program: 44, vol: 96, pan: 36, rev: 62 });
const choir = song.part("Choir", { ch: 12, program: 52, vol: 92, pan: 50, rev: 70 });
const brass = song.part("Brass section", { ch: 13, program: 61, vol: 96, pan: 64, rev: 54 });
const rcym = song.part("Reverse cymbal", { ch: 14, program: 119, vol: 104, pan: 64, rev: 50, human: 0 });
const pizz = song.part("Pizzicato", { ch: 15, program: 45, vol: 86, pan: 28, rev: 56 });

// Orchestra Kit: concert cymbals 59 and 57, splash 55 (41-53 are tuned timpani in this kit)
const BD = 36, SN = 38, CYM = 59, CYM2 = 57, SPLASH = 55;

// ---------------------------------------------------------------------------------------------- ostinato
// One bar of 16ths on root `r` (E2 = the home pitch): accents every three 16ths, the last group four long.
// Accent pitches: octave, minor 2nd, octave, tritone, minor 3rd; everything else the root.
const ACC = { 0: 12, 3: 1, 6: 12, 9: 6, 12: 3 };
const ostinato = (bar, r, lv = 1, { piano = true, bass = true } = {}) => {
  for (let i = 0; i < 16; i++) {
    const acc = i in ACC;
    const p = n(r) + (acc ? ACC[i] : 0);
    const b = at(bar) + i / 4;
    lowStr.note(b, acc ? 0.3 : 0.2, p, (acc ? 112 : 84) * lv, { exact: i === 0 });
    if (piano) pno.note(b, 0.22, acc ? [p - 12, p] : p - 12, (acc ? 98 : 64) * lv, { exact: i === 0 });
    if (bass && acc) cb.note(b, 0.4, n(r) - 12 + (ACC[i] === 12 ? 0 : ACC[i]), 100 * lv);
  }
};

// ---------------------------------------------------------------------------------------------- the klaxon
/** One pulse: a dyad held most of `len`, then falling ~1.5 semitones (a siren winding down). */
const pulse = (beat, len, dyad, vel) => {
  for (const p of [klax, mtp]) {
    p.bend(beat - 0.01, 0);
    p.note(beat, len * 0.92, dyad, vel - (p === klax ? 6 : 0), { exact: true });
    p.bendRamp(beat + len * 0.5, len * 0.42, 0, -1.5, 8);
  }
};
const klaxonBar = (bar, dyad, vel, every = 1) => {
  for (let k = 0; k < 4; k += every) pulse(at(bar) + k, every, dyad, vel + (k === 0 ? 8 : 0));
};

// ---------------------------------------------------------------------------------------------- brass
/** Low-brass swell over `beats` on a dissonant cluster, cut off sharply (the next downbeat speaks). */
const swell = (beat, beats, { tb, tu, hornDyad, from = 30, to = 124 }) => {
  tbn.note(beat, beats - 0.15, tb, 104).ramp(beat, beats - 0.2, from, to);
  tuba.note(beat, beats - 0.15, tu, 100).ramp(beat, beats - 0.2, from, to);
  if (hornDyad) hn.note(beat, beats - 0.15, hornDyad, 98).ramp(beat, beats - 0.2, from, to);
};
// FluidR3's reverse cymbal at C4 peaks 1.37 s after its note-on: start it so the peak is the downbeat
const reverseCymbal = (target) => rcym.note(target - (1.37 * song.bpm) / 60, 3, "C4", 112);
const susRoll = (beat, beats, v0, v1) => {
  const k = Math.round(beats * 8);
  for (let i = 0; i < k; i++) dr.note(beat + i / 8, 0.12, i % 2 ? CYM : CYM2, v0 + ((v1 - v0) * i) / k);
};
const snareRoll = (beat, beats, v0, v1) => {
  const k = Math.round(beats * 8);
  for (let i = 0; i < k; i++) dr.note(beat + i / 8, 0.1, SN, v0 + ((v1 - v0) * i) / k);
};

// ============================================================================================== 1-2 drone
song.marker(1, "Drone");
pno.note(at(1), 8, ["E0", "E1"], 96, { exact: true }).pedal(at(1), 8);
cb.note(at(1), 8, ["E1", "E2"], 80).ramp(at(1), 8, 30, 110);
tuba.note(at(1), 8, "E1", 70).ramp(at(1), 8, 20, 96);
const tr = (beat, beats, pitch, v0, v1) => {
  const k = Math.round(beats * 6);
  for (let i = 0; i < k; i++) timp.note(beat + i / 6, 0.2, pitch, v0 + ((v1 - v0) * i) / k);
};
tr(at(1, 2), 6.6, n("E2"), 20, 96);
reverseCymbal(at(3));
trem.note(at(1), 8, ["E4", "F4"], 54).ramp(at(1), 8, 20, 90);

// ============================================================================================== 3-6 ostinato
song.marker(3, "Ostinato");
dr.note(at(3), 1, BD, 112, { exact: true }).note(at(3), 2, CYM2, 96, { exact: true });
for (let b = 3; b <= 6; b++) ostinato(b, "E2", 0.78 + 0.05 * (b - 3), { piano: b >= 4 });
swell(at(3), 8, { tb: ["E2", "F2"], tu: "E1", to: 96 });
swell(at(5), 8, { tb: ["E2", "Bb2"], tu: "E1", to: 104 });
timp.note(at(5), 1, "E2", 104, { exact: true });

// ============================================================================================== 7-10 klaxon
song.marker(7, "Klaxon");
dr.note(at(7), 1, BD, 118, { exact: true }).note(at(7), 2, CYM, 104, { exact: true });
for (let b = 7; b <= 10; b++) {
  ostinato(b, "E2", 0.95);
  klaxonBar(b, ["Bb4", "E5"], 88 + 4 * (b - 7));
  // timpani and taiko on the accent groups
  [[0, "E2", 116], [1.5, "Bb2", 100], [3, "E2", 108]].forEach(([o, p, v]) => timp.note(at(b) + o, 0.6, p, v, { exact: o === 0 }));
  [0, 0.75, 1.5, 2.25, 3].forEach((o, i) => taiko.note(at(b) + o, 0.5, i === 0 ? "C3" : "C3", i === 0 ? 118 : 86, { exact: o === 0 }));
  dr.note(at(b), 0.8, BD, 116, { exact: true });
}
swell(at(7), 8, { tb: ["E2", "F2", "Bb2"], tu: "E1", hornDyad: ["Bb3", "E4"] });
swell(at(9), 8, { tb: ["E2", "F2", "Bb2"], tu: "E1", hornDyad: ["Bb3", "E4"], to: 127 });
reverseCymbal(at(11));

// ============================================================================================== 11-14 brass and choir
song.marker(11, "Brass and choir");
dr.note(at(11), 1, BD, 122, { exact: true }).note(at(11), 2, CYM, 116, { exact: true }).note(at(11), 2, CYM2, 104, { exact: true });
for (let b = 11; b <= 14; b++) {
  ostinato(b, "E2", 1);
  klaxonBar(b, ["Bb4", "E5"], 96);
  [[0, "E2", 120], [1.5, "Bb2", 106], [3, "E2", 112]].forEach(([o, p, v]) => timp.note(at(b) + o, 0.6, p, v, { exact: o === 0 }));
  [0, 0.75, 1.5, 2.25, 3].forEach((o, i) => taiko.note(at(b) + o, 0.5, "C3", i === 0 ? 122 : 92, { exact: o === 0 }));
  dr.note(at(b), 0.8, BD, 118, { exact: true });
  // horn and brass stabs on the 3+3 accents
  for (const o of [0, 0.75, 1.5]) {
    hn.note(at(b) + o, 0.3, ["Bb3", "E4"], 112, { exact: o === 0 });
    brass.note(at(b) + o, 0.3, ["E3", "Bb3", "F4"], 104, { exact: o === 0 });
  }
  // pizzicato ticks high up, off the beat
  for (const o of [0.5, 1.25, 2.5, 3.25]) pizz.note(at(b) + o, 0.2, b % 2 ? ["F5", "B5"] : ["E5", "Bb5"], 78);
}
choir.note(at(11), 16, ["E4", "F4", "Bb4"], 84).ramp(at(11), 16, 40, 110);
trem.note(at(11), 16, ["E5", "F5", "Bb5"], 70).ramp(at(11), 16, 40, 112);
swell(at(11), 8, { tb: ["E2", "F2"], tu: "E1" });
swell(at(13), 8, { tb: ["E2", "Bb2"], tu: "E1" });
reverseCymbal(at(15));

// ============================================================================================== 15-22 the climb
// The ostinato, the brass cluster and the klaxon climb a semitone every two bars (E, F, F#, G); the klaxon
// doubles to 8ths; a snare roll and a cymbal swell lead into the crash.
song.marker(15, "The climb");
["E2", "F2", "F#2", "G2"].forEach((r, k) => {
  const bar = 15 + 2 * k;
  const up = n(r) - n("E2");
  dr.note(at(bar), 1, BD, 120, { exact: true }).note(at(bar), 2, k % 2 ? CYM2 : CYM, 106 + 4 * k, { exact: true });
  for (let j = 0; j < 2; j++) {
    const b = bar + j;
    ostinato(b, r, 1 + 0.03 * k);
    klaxonBar(b, [n("Bb4") + up, n("E5") + up], 98 + 4 * k, 0.5);
    [[0, 0, 122], [1.5, 6, 110], [3, 0, 116]].forEach(([o, d, v]) => timp.note(at(b) + o, 0.6, n("E2") + up + d, v, { exact: o === 0 }));
    for (let o = 0; o < 4; o += 0.75) taiko.note(at(b) + o, 0.5, "C3", o === 0 ? 124 : 94 + 6 * k, { exact: o === 0 });
    dr.note(at(b), 0.8, BD, 120, { exact: true });
    for (const o of [0, 0.75, 1.5, 2.25, 3]) {
      hn.note(at(b) + o, 0.3, [n("Bb3") + up, n("E4") + up], 98 + 3 * k, { exact: o === 0 });
      brass.note(at(b) + o, 0.3, [n("E3") + up, n("Bb3") + up, n("F4") + up], 104 + 3 * k, { exact: o === 0 });
    }
    for (const o of [0.5, 1.25, 2.5, 3.25]) pizz.note(at(b) + o, 0.2, [n("F5") + up, n("B5") + up], 82 + 3 * k);
  }
  swell(at(bar), 8, { tb: [n("E2") + up, n("F2") + up, n("Bb2") + up], tu: n("E1") + up, from: 40, to: 127 });
  choir.note(at(bar), 8, [n("E4") + up, n("F4") + up, n("Bb4") + up], 90 + 5 * k).ramp(at(bar), 8, 64 + 6 * k, 112);
  trem.note(at(bar), 8, [n("E5") + up, n("F5") + up, n("Bb5") + up], 76 + 5 * k).ramp(at(bar), 8, 64 + 6 * k, 114);
});
snareRoll(at(21), 7.5, 40, 124);
susRoll(at(21), 7.5, 30, 112);
reverseCymbal(at(23));
tr(at(22, 3), 1.5, n("G2"), 80, 124);

// ============================================================================================== 23-24 crash
// Everything lands on one cluster (E F Bb over a low E), then rings and thins; the hall carries it.
song.marker(23, "Crash");
const CRASH = at(23);
const ex = { exact: true };
dr.note(CRASH, 4, CYM, 127, ex).note(CRASH, 4, CYM2, 124, ex).note(CRASH, 1, BD, 127, ex).note(CRASH, 3, SPLASH, 110, ex);
taiko.note(CRASH, 2, "C3", 127, ex).note(CRASH, 2, "G2", 120, ex);
timp.note(CRASH, 2, "E2", 127, ex).note(CRASH, 2, "Bb2", 120, ex);
pno.note(CRASH, 8, ["E0", "E1", "F1", "Bb1", "E2"], 124, ex).pedal(CRASH, 8);
cb.note(CRASH, 6, ["E1", "E2"], 124, ex).ramp(CRASH, 8, 127, 40);
lowStr.note(CRASH, 6, ["E2", "F2", "Bb2", "E3"], 120, ex).ramp(CRASH, 8, 127, 40);
tuba.note(CRASH, 3, "E1", 124, ex);
tbn.cc(CRASH - 0.05, 11, 127).note(CRASH, 3, ["E2", "F2", "Bb2"], 124, ex).ramp(CRASH + 1, 3, 127, 50);
hn.cc(CRASH - 0.05, 11, 127).note(CRASH, 3, ["Bb3", "E4", "F4"], 124, ex).ramp(CRASH + 1, 3, 127, 50);
brass.note(CRASH, 1.5, ["E3", "Bb3", "E4", "F4", "Bb4"], 124, ex);
mtp.note(CRASH, 1.5, ["Bb4", "E5", "F5"], 116, ex);
choir.note(CRASH, 7.5, ["E4", "F4", "Bb4", "E5"], 120, ex).ramp(CRASH, 8, 127, 30);
trem.note(CRASH, 7.5, ["E5", "F5", "Bb5", "E6"], 112, ex).ramp(CRASH, 8, 127, 24);
timp.note(at(24), 1, "E2", 70);

// ============================================================================================== 25-28 aftermath
// The ostinato returns on the piano alone, thinning out; a last, distant klaxon; a low hit; the hall.
song.marker(25, "Aftermath");
cb.note(at(25), 16, ["E1"], 70).ramp(at(25), 16, 70, 20);
trem.note(at(25), 12, ["E4", "F4"], 40).ramp(at(25), 12, 40, 10);
for (let b = 25; b <= 26; b++) {
  for (let i = 0; i < 16; i++) {
    if (b === 26 && i >= 8 && !(i in ACC)) continue; // thinning
    const acc = i in ACC;
    pno.note(at(b) + i / 4, 0.22, n("E2") + (acc ? ACC[i] : 0), (acc ? 60 : 36) * (b === 25 ? 1 : 0.8), { exact: i === 0 });
  }
}
pulse(at(26, 1), 1, ["Bb4", "E5"], 54);
pulse(at(26, 3), 1, ["Bb4", "E5"], 46);
// the final low hit, then nothing but the hall
pno.note(at(27), 8, ["E0", "E1", "Bb1"], 110, ex).pedal(at(27), 8);
timp.note(at(27), 2, "E2", 118, ex);
dr.note(at(27), 2, BD, 120, ex).note(at(27), 4, CYM2, 70, ex);
taiko.note(at(27), 2, "C3", 116, ex);
tuba.note(at(27), 4, "E1", 90, ex).ramp(at(27), 4, 100, 40);

// ---------------------------------------------------------------------------------------------- write
const HITS = [at(3), at(7), at(11), at(15), at(17), at(19), at(21), at(23), at(27)];
song.save(dirname(fileURLToPath(import.meta.url)), "threat-alert", {
  endBar: 28,
  tail: 3,
  meta: {
    title: "Threat alert",
    key: "E (Phrygian / chromatic), climbing to G",
    gain: 0.45,
    fadeSec: 2,
    reverb: { room: 0.78, damp: 0.3, width: 1, level: 0.6 },
    master: { eq: "equalizer=f=200:t=q:w=1.2:g=-2,highshelf=f=5000:g=2.5" },
    hits: HITS,
    gridChecks: [{ label: "16ths", stepBeats: 0.25, fromBar: 3, toBar: 22 }],
    cues: [
      [1, 1, "low drone, piano low E, timpani roll"], [3, 1, "ostinato enters (reverse cymbal lands here)"],
      [7, 1, "klaxon enters, timpani and taiko"], [11, 1, "horn stabs, choir and high tremolo"],
      [15, 1, "the climb: up a semitone every two bars (15, 17, 19, 21)"], [21, 1, "snare roll and cymbal swell begin"],
      [23, 1, "CRASH"], [25, 1, "aftermath: piano ostinato alone"], [27, 1, "last low hit"],
    ],
  },
});
