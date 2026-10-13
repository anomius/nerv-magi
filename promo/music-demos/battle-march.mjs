#!/usr/bin/env node
// Demo 1, "Battle march": an original militaristic orchestral march, C minor, 150 BPM (one beat = 24 frames at
// 60 fps, the promo's grid), stepping up a whole tone to D minor for its last strain. It borrows the idiom only:
// field snare with rudiments (flams, drags, 5- and 7-stroke rolls), timpani tuned to tonic and dominant, a
// minor-key theme in horns and trombones in octaves, trumpet off-beat stabs, string tremolo and scale runs.
// Every melody, chord progression and rhythm here was written for this demo.
// Form (36 bars): drum call 1-4 | theme 5-12 | fanfare strain 13-20 | bridge 21-24 | theme up a tone 25-32 | coda 33-36.
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Song, n } from "./smf.mjs";

const song = new Song({ bpm: 150, seed: 150 });
const at = (bar, beat = 1) => song.at(bar, beat);

// parts: GM programs, orchestral seating (pan), reverb send
const vln = song.part("Violins", { ch: 0, program: 48, vol: 100, pan: 34, rev: 56 });
const tremHi = song.part("Tremolo high", { ch: 1, program: 44, vol: 104, pan: 46, rev: 60 });
const tremLo = song.part("Tremolo low", { ch: 2, program: 44, vol: 100, pan: 86, rev: 56 });
const cb = song.part("Contrabass", { ch: 3, program: 43, vol: 100, pan: 94, rev: 44 });
const hn = song.part("Horns", { ch: 4, program: 60, vol: 118, pan: 44, rev: 60 });
const tbn = song.part("Trombones", { ch: 5, program: 57, vol: 104, pan: 80, rev: 52 });
const tpt = song.part("Trumpets", { ch: 6, program: 56, vol: 102, pan: 70, rev: 52 });
const tuba = song.part("Tuba", { ch: 7, program: 58, vol: 100, pan: 88, rev: 44 });
const timp = song.part("Timpani", { ch: 8, program: 47, vol: 126, pan: 56, rev: 50, human: 5 });
const dr = song.part("Snare, bass drum, cymbals", { ch: 9, program: 48, vol: 112, pan: 62, rev: 40, human: 5, humanVel: 7 });
const brass = song.part("Brass section", { ch: 10, program: 61, vol: 92, pan: 64, rev: 50 });
const picc = song.part("Piccolo", { ch: 11, program: 72, vol: 70, pan: 30, rev: 58 });

// Orchestra Kit (bank 128, program 48): two concert-snare samples alternate as right and left hand; concert
// cymbals are 59 and 57 (41-53 are tuned timpani in this kit), a suspended-cymbal roll alternates the two
const SN = [38, 40], BD = 36, CYM = 59, CYM2 = 57, SUS = [59, 57];

// ---------------------------------------------------------------------------------------------- harmony
const PC = { C: 0, "C#": 1, Db: 1, D: 2, Eb: 3, E: 4, F: 5, "F#": 6, Gb: 6, G: 7, Ab: 8, A: 9, Bb: 10, B: 11 };
const CH = {
  Cm: ["C", "Eb", "G"], Ab: ["Ab", "C", "Eb"], Fm: ["F", "Ab", "C"], G: ["G", "B", "D"], G7: ["G", "B", "D", "F"],
  Db: ["Db", "F", "Ab"], Eb: ["Eb", "G", "Bb"], Bb: ["Bb", "D", "F"], A7: ["A", "C#", "E", "G"], A: ["A", "C#", "E"],
  Dm: ["D", "F", "A"], Gm: ["G", "Bb", "D"], F: ["F", "A", "C"], C: ["C", "E", "G"],
};
/** Close voicing of `c`: `count` chord tones upward from the first one at or above `low`. */
const voice = (c, low, count = CH[c].length) => {
  const pcs = CH[c].map((t) => PC[t]);
  const out = [];
  for (let k = n(low); out.length < count; k++) if (pcs.includes(k % 12)) out.push(k);
  return out;
};
/** Root of `c` in the octave starting at `low`. */
const root = (c, low) => {
  for (let k = n(low); ; k++) if (k % 12 === PC[CH[c][0]]) return k;
};
/** A chord name a whole tone up (the last strain's key change). */
const shift = (c, semis) => (semis === 2 ? { Cm: "Dm", Ab: "Bb", Fm: "Gm", G: "A", G7: "A7", Db: "Eb", Eb: "F", Bb: "C" }[c] ?? c : c);

// ---------------------------------------------------------------------------------------------- drums
let hand = 0;
const stroke = (beat, vel, exact = false) => dr.note(beat, 0.2, SN[(hand ^= 1)], vel, { exact });
/** Flam: a soft grace note ~28 ms ahead of the main stroke. */
const flam = (beat, vel, exact = false) => {
  if (beat < dr.ms(28)) return stroke(beat, vel, exact); // nothing can come before the first beat
  dr.note(beat - dr.ms(28), 0.1, SN[hand], vel * 0.42, { exact: true });
  stroke(beat, vel, exact);
};
/** Drag: two quick grace notes ahead of the main stroke. */
const drag = (beat, vel) => {
  dr.note(beat - dr.ms(46), 0.08, SN[hand], vel * 0.4, { exact: true }).note(beat - dr.ms(23), 0.08, SN[hand], vel * 0.45, { exact: true });
  stroke(beat, vel);
};
/** N-stroke roll ending with an accent on `end` (32nds before it, slightly crescendo). */
const nRoll = (end, strokes, vel) => {
  for (let i = strokes - 1; i >= 1; i--) stroke(end - i / 8, vel * (0.55 + 0.25 * (1 - i / strokes)));
  stroke(end, vel, true);
};
/** Closed roll (32nds) crescendo from v0 to v1 over `beats`. */
const roll = (beat, beats, v0, v1, note = null) => {
  const k = Math.round(beats * 8);
  for (let i = 0; i < k; i++) {
    const v = v0 + ((v1 - v0) * i) / k;
    if (note) dr.note(beat + i / 8, 0.1, [].concat(note)[i % [].concat(note).length], v);
    else stroke(beat + i / 8, v);
  }
};
/** One bar of march snare, `kind` 0 or 1 (a two-bar cell); accents on 2 and 4, bass drum on 1 and 3. */
const march = (bar, kind, lv = 1) => {
  const b = at(bar);
  const v = (x) => x * lv;
  if (kind === 0) {
    flam(b, v(100), true);
    stroke(b + 0.5, v(64));
    stroke(b + 0.75, v(70));
    stroke(b + 1, v(112));
    stroke(b + 1.5, v(66));
    flam(b + 2, v(96));
    stroke(b + 2.5, v(64));
    stroke(b + 2.75, v(70));
    drag(b + 3, v(114));
    stroke(b + 3.5, v(66));
    stroke(b + 3.75, v(74));
  } else {
    stroke(b, v(100), true);
    nRoll(b + 1, 5, v(114));
    stroke(b + 1.5, v(66));
    stroke(b + 1.75, v(70));
    flam(b + 2, v(96));
    nRoll(b + 3, 7, v(118));
    stroke(b + 3.5, v(68));
    stroke(b + 3.75, v(76));
  }
  dr.note(b, 0.6, BD, v(108), { exact: true }).note(b + 2, 0.6, BD, v(92));
};
/** Clash cymbals (two samples) on a phrase start. */
const clash = (beat, vel = 118) => dr.note(beat, 3, CYM, vel, { exact: true }).note(beat, 3, CYM2, vel - 10, { exact: true });

// ---------------------------------------------------------------------------------------------- timpani
// Two drums tuned to tonic and dominant (C3/G2, then D3/A2 after the key change), as a real pair would be.
const timpDrums = (key) => (key === 0 ? [n("C3"), n("G2")] : [n("D3"), n("A2")]);
const timpPick = (c, key, prefer = 0) => {
  const drums = timpDrums(key);
  const pcs = CH[c].map((t) => PC[t]);
  const ok = drums.filter((d) => pcs.includes(d % 12));
  return ok.length ? (ok.includes(drums[prefer]) ? drums[prefer] : ok[0]) : null;
};
const timpBar = (bar, c, key, vel = 100) => {
  const a = timpPick(c, key, 0), b = timpPick(c, key, 1);
  if (a) timp.note(at(bar), 0.9, a, vel, { exact: true });
  if (b) timp.note(at(bar, 3), 0.9, b, vel - 12);
};
const timpRoll = (beat, beats, pitch, v0, v1) => {
  const k = Math.round(beats * 6);
  for (let i = 0; i < k; i++) timp.note(beat + i / 6, 0.2, pitch, v0 + ((v1 - v0) * i) / k);
};

// ---------------------------------------------------------------------------------------------- orchestra
/** Low brass and basses on the roots, beats 1 and 3 (fifth on 3), with an 8th pickup on beat 4. */
const bassBar = (bar, c, vel = 96, walk = false) => {
  const r = root(c, "E1");
  const fifth = r + 7 > n("C2") + 4 ? r - 5 : r + 7;
  tuba.note(at(bar), 0.9, r, vel).note(at(bar, 3), 0.9, fifth, vel - 8);
  cb.note(at(bar), 0.9, r + 12, vel - 4).note(at(bar, 3), 0.9, fifth + 12, vel - 12);
  if (walk) {
    tuba.note(at(bar, 4.5), 0.45, r + 2, vel - 14);
    cb.note(at(bar, 4.5), 0.45, r + 14, vel - 16);
  }
};
/** Trumpet stabs on the off-beats: `pattern` lists beat offsets within the bar. */
const stabs = (bar, c, vel = 92, pattern = [1.5, 3.5]) => {
  for (const o of pattern) tpt.note(at(bar) + o, 0.28, voice(c, "G4", 3), vel, { legato: 1 });
};
/** Tremolo strings holding `c` for `beats`, with an expression swell. */
const trem = (beat, beats, c, vel = 62, from = 70, to = 100) => {
  tremHi.note(beat, beats, voice(c, "C5", 3), vel);
  tremLo.note(beat, beats, voice(c, "G3", 3), vel - 4);
  tremHi.ramp(beat, beats, from, to);
  tremLo.ramp(beat, beats, from, to);
};
/** Violin scale run in 16ths over `beats`, landing on `target` at `beat + beats`. */
const run = (beat, beats, from, target, scale, vel = 70) => {
  const pcs = scale.map((s) => PC[s]);
  const steps = Math.round(beats * 4);
  const notes = [];
  for (let k = n(target) - 1; notes.length < steps && k > 0; k--) if (pcs.includes(k % 12)) notes.unshift(k);
  notes.forEach((p, i) => vln.note(beat + i / 4, 0.24, [p, p - 12 >= n(from) ? p - 12 : p], vel + i * 2));
  vln.note(beat + beats, 1, [n(target), n(target) - 12], vel + 20);
};
/** The big hit: chord `c` on every section, cymbals, bass drum and timpani on the grid. */
const tutti = (beat, c, key, { len = 1, vel = 118, cym = true } = {}) => {
  const opt = { exact: true };
  tpt.note(beat, len, voice(c, "G4", 3), vel, opt);
  hn.note(beat, len, voice(c, "C4", 3), vel - 4, opt);
  tbn.note(beat, len, voice(c, "C3", 3), vel, opt);
  brass.note(beat, len, voice(c, "G3", 4), vel - 8, opt);
  tuba.note(beat, len, root(c, "E1"), vel, opt);
  cb.note(beat, len, root(c, "E2"), vel, opt);
  vln.note(beat, len * 0.8, voice(c, "G4", 4), vel - 12, opt);
  const t = timpPick(c, key);
  if (t) timp.note(beat, 1.2, t, 124, opt);
  dr.note(beat, 0.8, BD, 124, opt);
  if (cym) clash(beat, 120);
};

// ---------------------------------------------------------------------------------------------- the theme
// Antecedent (Cm Ab Fm G) rises to Eb, consequent (Cm Fm Db-G Cm) to F and closes on the tonic. In C minor;
// `key` 2 plays it a whole tone up.
const THEME = [
  ["G4", 0.75], ["C5", 0.25], ["C5", 1], ["Eb5", 0.75], ["D5", 0.25], ["C5", 1], // Cm
  ["Eb5", 1.5], ["C5", 0.5], ["Ab4", 1], ["Eb4", 1], // Ab
  ["F4", 0.75], ["Ab4", 0.25], ["C5", 1.5], ["Bb4", 0.5], ["Ab4", 1], // Fm
  ["B4", 2], ["D5", 0.75], ["B4", 0.25], ["G4", 1], // G
  ["G4", 0.75], ["C5", 0.25], ["C5", 1], ["Eb5", 0.75], ["F5", 0.25], ["Eb5", 1], // Cm
  ["F5", 1.5], ["Eb5", 0.5], ["C5", 1], ["Ab4", 1], // Fm
  ["Db5", 1.5], ["C5", 0.5], ["B4", 1], ["D5", 1], // Db G
  ["C5", 3], [null, 1], // Cm
];
const THEME_CHORDS = [["Cm"], ["Ab"], ["Fm"], ["G"], ["Cm"], ["Fm"], ["Db", "G"], ["Cm"]];
/** Phrase dynamics: velocity follows the line's height, louder on long notes. */
const shaped = (line, base, key = 0, oct = 0) =>
  line.map(([p, l]) => (p === null ? [null, l] : [n(p) + key + oct, l, base + (n(p) - 67) * 0.8 + (l >= 1.5 ? 6 : 0)]));

/** Theme in horns + trombones in octaves (and trumpets, piccolo for the climax). */
const theme = (bar, key, { vel = 100, tptToo = false, piccToo = false } = {}) => {
  hn.line(at(bar), shaped(THEME, vel, key), vel, 0.9);
  tbn.line(at(bar), shaped(THEME, vel - 4, key, -12), vel, 0.9);
  if (tptToo) tpt.line(at(bar), shaped(THEME, vel - 2, key), vel, 0.88);
  if (piccToo) picc.line(at(bar), shaped(THEME, vel - 18, key, 12), vel, 0.82);
  // each two-bar phrase swells, then eases off into the next
  for (const p of [hn, tbn]) for (let k = 0; k < 8; k += 2) p.ramp(at(bar + k), 6, 88, 116).ramp(at(bar + k) + 6, 2, 116, 96);
};

// ============================================================================================== 1-4 drum call
song.marker(1, "Drum call");
march(1, 0, 0.7);
march(2, 1, 0.78);
dr.ramp(0, 8, 90, 115);
timp.note(at(2), 0.9, "C3", 82, { exact: true }).note(at(2, 3), 0.9, "G2", 80);
march(3, 0, 0.86);
timpBar(3, "G", 0, 96);
// low brass and tremolo on the dominant, crescendo into the theme
tuba.note(at(3), 8, "G1", 84).ramp(at(3), 8, 50, 120);
tbn.note(at(3), 8, ["G2", "D3"], 84).ramp(at(3), 8, 50, 124);
cb.note(at(3), 8, "G2", 86).ramp(at(3), 8, 50, 118);
trem(at(3), 8, "G", 70, 40, 118);
hn.note(at(4), 4, ["B3", "D4", "G4"], 86).ramp(at(4), 4, 50, 120);
roll(at(4), 3.5, 54, 122);
timpRoll(at(4), 3.5, n("G2"), 50, 118);
roll(at(4), 3.5, 30, 96, SUS);
run(at(4, 3), 2, "G3", "C6", ["C", "D", "Eb", "F", "G", "A", "B"], 64);

// ============================================================================================== 5-12 theme
song.marker(5, "Theme (C minor)");
tutti(at(5), "Cm", 0, { len: 0.6, vel: 112 });
theme(5, 0, { vel: 100 });
THEME_CHORDS.forEach((cs, i) => {
  const bar = 5 + i;
  march(bar, i % 2, 0.94);
  if (cs.length === 1) {
    bassBar(bar, cs[0], 94, i === 3);
    timpBar(bar, cs[0], 0, 106);
    stabs(bar, cs[0], 90);
    if (i !== 7) trem(at(bar), 4, cs[0], 66, 70, 96);
  } else {
    // two chords in the bar: half notes
    cs.forEach((c, j) => {
      tuba.note(at(bar, 1 + 2 * j), 1.8, root(c, "E1"), 96);
      cb.note(at(bar, 1 + 2 * j), 1.8, root(c, "E2"), 92);
      trem(at(bar, 1 + 2 * j), 2, c, 60, 80, 104);
      tpt.note(at(bar, 1.5 + 2 * j), 0.28, voice(c, "G4", 3), 90, { legato: 1 });
    });
    timp.note(at(bar, 3), 0.9, "G2", 104);
  }
});
clash(at(9), 106);
// the cadence in bar 12: tonic on 1, a trumpet triplet answer on beat 4, run into the fanfare
tpt.line(at(12, 4), [[voice("G", "G4", 3), 1 / 3, 96], [voice("G", "G4", 3), 1 / 3, 92], [voice("G", "G4", 3), 1 / 3, 104]], 96, 0.8);
trem(at(12), 2, "Cm", 60, 80, 96);
run(at(12, 3), 2, "C4", "Eb6", ["C", "D", "Eb", "F", "G", "Ab", "Bb"], 66);

// ============================================================================================== 13-20 fanfare strain
// Trumpets carry a repeated-note fanfare; horns hold the harmony; trombones and tuba march underneath.
song.marker(13, "Fanfare strain");
const FANFARE = [
  ["Eb5", 1], ["Eb5", 1 / 3], ["Eb5", 1 / 3], ["Eb5", 1 / 3], ["Ab5", 1.5], ["G5", 0.5], // Ab
  ["F5", 1], ["F5", 1 / 3], ["F5", 1 / 3], ["F5", 1 / 3], ["Bb5", 1.5], ["Ab5", 0.5], // Bb
  ["G5", 1.5], ["F5", 0.5], ["Eb5", 1], ["Bb4", 1], // Eb
  ["C5", 1], ["Eb5", 1], ["G5", 2], // Cm
  ["Ab5", 1.5], ["G5", 0.5], ["F5", 1], ["C5", 1], // Fm
  ["D5", 1], ["G5", 1], ["F5", 1.5], ["D5", 0.5], // G7
  ["Eb5", 1], ["C5", 1], ["Ab5", 1.5], ["G5", 0.5], // Ab
  ["G5", 3], [null, 1], // G
];
const FAN_CHORDS = ["Ab", "Bb", "Eb", "Cm", "Fm", "G7", "Ab", "G"];
tutti(at(13), "Ab", 0, { len: 0.6, vel: 116 });
tpt.line(at(13), FANFARE.map(([p, l]) => [p, l, p && n(p) >= n("Ab5") ? 118 : 110]), 110, 0.86);
brass.line(at(13), FANFARE.map(([p, l]) => [p && n(p) - 12, l, 84]), 84, 0.86);
FAN_CHORDS.forEach((c, i) => {
  const bar = 13 + i;
  march(bar, i % 2, 1.06);
  bassBar(bar, c, 104, i % 2 === 1);
  timpBar(bar, c, 0, 110);
  hn.note(at(bar), 3.8, voice(c, "Eb4", 3), 72).ramp(at(bar), 4, 62, 92);
  tbn.note(at(bar), 0.9, voice(c, "G2", 3), 106).note(at(bar, 3), 0.9, voice(c, "G2", 3), 100);
  tbn.note(at(bar, 2.5), 0.3, voice(c, "G2", 3), 84).note(at(bar, 4.5), 0.3, voice(c, "G2", 3), 84);
  trem(at(bar), 4, c, 76, 80, 108);
  // the violins answer the fanfare with a rising 8th-note arpeggio in the back half of every other bar
  if (i % 2 === 1 && i < 7) voice(c, "G4", 4).forEach((p, j) => vln.note(at(bar, 3) + j / 2, 0.45, p, 66 + j * 6));
});
clash(at(17), 112);
// into the bridge: 7-stroke roll, the timpani on G, a run down-and-up
roll(at(20, 3), 1.5, 60, 120);
timpRoll(at(20, 3), 1.5, n("G2"), 70, 118);
run(at(20, 3), 2, "G3", "C6", ["C", "D", "Eb", "F", "G", "A", "B"], 70);

// ============================================================================================== 21-24 bridge
// The band drops out: drums, timpani and an 8th-note low-brass ostinato; the theme's head in the trombones,
// echoed by the horns; then Bb and A7 (the dominant of D minor) to step the key up a whole tone.
song.marker(21, "Bridge");
dr.note(at(21), 0.8, BD, 112, { exact: true });
clash(at(21), 102);
const ost = (bar, c, vel) => {
  const r = root(c, "E1");
  [0, 0, 12, 0, 7, 0, 12, 7].forEach((d, i) => {
    tuba.note(at(bar) + i / 2, 0.42, r + d, vel - (i % 2) * 12);
    cb.note(at(bar) + i / 2, 0.42, r + 12 + d, vel - 6 - (i % 2) * 12);
  });
};
[["Cm", 0.64, 80], ["Ab", 0.72, 84], ["Bb", 0.9, 94]].forEach(([c, lv, v], i) => {
  march(21 + i, i % 2, lv);
  ost(21 + i, c, v);
  timpBar(21 + i, c, 0, v);
  trem(at(21 + i), 4, c, 50 + 6 * i, 46, 90);
});
tbn.line(at(21), shaped(THEME.slice(0, 6), 90, 0, -12), 90, 0.9);
// the head again on Ab, transposed diatonically
hn.line(at(22), shaped([["Eb4", 0.75], ["Ab4", 0.25], ["Ab4", 1], ["C5", 0.75], ["Bb4", 0.25], ["Ab4", 1]], 86), 86, 0.9);
tpt.note(at(23), 3.8, voice("Bb", "F4", 3), 88).ramp(at(23), 4, 50, 108);
hn.note(at(23), 3.8, voice("Bb", "D4", 3), 90).ramp(at(23), 4, 56, 108);
tbn.note(at(23), 3.8, voice("Bb", "D3", 3), 90).ramp(at(23), 4, 56, 108);
// bar 24: A7 stabs (1, 2&, 4) with timpani, a snare roll, the run into D minor
for (const o of [0, 1.5, 3]) {
  const v = o === 0 ? 112 : 106;
  tpt.note(at(24) + o, 0.4, voice("A7", "G4", 3), v, { legato: 1 });
  hn.note(at(24) + o, 0.45, voice("A7", "C#4", 3), v - 6, { legato: 1 });
  tbn.note(at(24) + o, 0.45, voice("A7", "E3", 3), v, { legato: 1 });
  brass.note(at(24) + o, 0.4, voice("A7", "A3", 4), v - 10, { legato: 1 });
  tuba.note(at(24) + o, 0.45, "A1", v);
  cb.note(at(24) + o, 0.45, "A2", v - 6);
  timp.note(at(24) + o, 0.6, "A2", v);
}
dr.note(at(24), 0.6, BD, 118, { exact: true });
dr.note(at(24), 2, CYM2, 96);
roll(at(24, 2), 2.5, 56, 124);
roll(at(24, 2), 2.5, 30, 104, SUS);
run(at(24, 3), 2, "A3", "D6", ["D", "E", "F", "G", "A", "B", "C#"], 72);

// ============================================================================================== 25-32 theme, D minor
song.marker(25, "Theme up a tone (D minor)");
for (const p of [tpt, hn, tbn]) p.cc(at(24), 11, 112);
tutti(at(25), "Dm", 2, { len: 0.6, vel: 122 });
theme(25, 2, { vel: 108, tptToo: true, piccToo: true });
THEME_CHORDS.forEach((cs, i) => {
  const bar = 25 + i;
  march(bar, i % 2, 1.04);
  // light cymbal on 2 and 4 with the snare accents
  dr.note(at(bar, 2), 0.5, CYM2, 64).note(at(bar, 4), 0.5, CYM2, 68);
  const cs2 = cs.map((c) => shift(c, 2));
  if (cs2.length === 1) {
    bassBar(bar, cs2[0], 102, i === 3);
    timpBar(bar, cs2[0], 2, 114);
    if (i !== 7) trem(at(bar), 4, cs2[0], 74, 84, 110);
    for (const o of [0.5, 1.5, 2.5, 3.5]) brass.note(at(bar) + o, 0.26, voice(cs2[0], "A3", 3), o % 2 > 1 ? 102 : 92, { legato: 1 });
  } else {
    cs2.forEach((c, j) => {
      tuba.note(at(bar, 1 + 2 * j), 1.8, root(c, "E1"), 104);
      cb.note(at(bar, 1 + 2 * j), 1.8, root(c, "E2"), 100);
      trem(at(bar, 1 + 2 * j), 2, c, 64, 86, 110);
      brass.note(at(bar, 1.5 + 2 * j), 0.26, voice(c, "A3", 3), 94, { legato: 1 });
    });
    timp.note(at(bar, 3), 0.9, "A2", 110);
  }
  // 8th-note string figure under the theme (chord tones, up then down)
  const v = voice(cs2[0], "D5", 4);
  [0, 1, 2, 3, 2, 1, 0, 1].forEach((k, j) => vln.note(at(bar) + j / 2, 0.42, v[k], 66 + (j % 2 ? 0 : 10)));
});
clash(at(27), 100);
clash(at(29), 116);
clash(at(31), 100);
run(at(32, 3), 2, "D4", "F6", ["D", "E", "F", "G", "A", "Bb", "C"], 70);
roll(at(32, 3), 1.5, 60, 122);

// ============================================================================================== 33-36 coda
// Bb | Gm A | stinger hits | final D minor chord with a timpani roll, cut clean on beat 3 of bar 36.
song.marker(33, "Coda");
tutti(at(33), "Bb", 2, { len: 0.8, vel: 120 });
const CODA = [["F5", 1.5], ["D5", 0.5], ["Bb4", 1], ["F5", 1], ["G5", 1.5], ["F5", 0.5], ["E5", 1], ["C#5", 1]];
for (const [p, o, v] of [[hn, 0, 112], [tpt, 0, 114], [tbn, -12, 110], [picc, 12, 92]]) p.line(at(33), CODA.map(([q, l]) => [n(q) + o, l]), v, 0.92);
march(33, 0, 1.06);
march(34, 1, 1.08);
bassBar(33, "Bb", 106);
tuba.note(at(34), 1.8, "G1", 106).note(at(34, 3), 1.8, "A1", 110);
cb.note(at(34), 1.8, "G2", 102).note(at(34, 3), 1.8, "A2", 106);
trem(at(33), 4, "Bb", 70, 90, 116);
trem(at(34), 2, "Gm", 72, 96, 120);
trem(at(34, 3), 2, "A7", 74, 100, 124);
timp.note(at(33), 0.9, "D3", 112, { exact: true });
timpRoll(at(34, 3), 2, n("A2"), 80, 124);
clash(at(34, 3), 108);
// stingers: Dm on 1, A7 on 2&, Dm on 3; silence on 4 but a 5-stroke roll into the final chord
for (const [o, c] of [[0, "Dm"], [1.5, "A7"], [2, "Dm"]]) {
  if (o === 0) tutti(at(35) + o, c, 2, { len: 0.5, vel: 124 });
  else {
    const opt = { exact: o % 1 === 0, legato: 1 };
    tpt.note(at(35) + o, 0.35, voice(c, "G4", 3), 118, opt);
    hn.note(at(35) + o, 0.35, voice(c, "C#4", 3), 112, opt);
    tbn.note(at(35) + o, 0.35, voice(c, "D3", 3), 118, opt);
    brass.note(at(35) + o, 0.35, voice(c, "A3", 4), 108, opt);
    tuba.note(at(35) + o, 0.35, root(c, "E1"), 118, opt);
    cb.note(at(35) + o, 0.35, root(c, "E2"), 110, opt);
    timp.note(at(35) + o, 0.6, c === "Dm" ? "D3" : "A2", 120, opt);
    dr.note(at(35) + o, 0.4, BD, 116, opt);
  }
}
nRoll(at(36), 7, 124);
// the final chord: everyone on D minor, the timpani rolling under it; a 16th of air, then the button on beat 3
const FIN = at(36);
const HOLD = 1.75;
tutti(FIN, "Dm", 2, { len: HOLD, vel: 126 });
hn.note(FIN, HOLD, ["D4", "F4", "A4", "D5"], 120, { exact: true });
tbn.note(FIN, HOLD, ["D2", "A2", "D3", "F3"], 120, { exact: true });
tremHi.note(FIN, HOLD, ["D5", "F5", "A5", "D6"], 100, { exact: true });
tremLo.note(FIN, HOLD, ["D3", "A3", "D4", "F4"], 100, { exact: true });
vln.note(FIN, HOLD, ["D5", "A5", "D6"], 110, { exact: true });
picc.note(FIN, HOLD, "D6", 96, { exact: true });
timpRoll(FIN + 1 / 6, HOLD - 1 / 3, n("D3"), 96, 124);
roll(FIN + 0.125, HOLD - 0.25, 70, 116);
// the button: one short tutti on beat 3, then only the hall
tutti(at(36, 3), "Dm", 2, { len: 0.5, vel: 127 });

// ---------------------------------------------------------------------------------------------- write
// Hits on the grid that the render's onset check looks for: phrase starts, the bridge, the stingers.
const HITS = [at(5), at(13), at(21), at(24), at(25), at(33), at(35), at(36), at(36, 3)];
song.save(dirname(fileURLToPath(import.meta.url)), "battle-march", {
  endBar: 36,
  tail: 3,
  meta: {
    title: "Battle march",
    key: "C minor → D minor (bar 25)",
    gain: 0.45,
    fadeSec: 1.5,
    reverb: { room: 0.72, damp: 0.35, width: 0.9, level: 0.55 },
    // less mud around 240 Hz, more air: horns and low brass carry the theme and they are dark samples
    master: { eq: "equalizer=f=240:t=q:w=1.2:g=-2.5,highshelf=f=5000:g=3" },
    hits: HITS,
    gridChecks: [{ label: "8ths", stepBeats: 0.5, fromBar: 5, toBar: 34 }],
    cues: [
      [1, 1, "drum call: snare and bass drum alone"], [3, 1, "low brass and tremolo on the dominant, crescendo"],
      [5, 1, "tutti hit, theme in horns and trombones (C minor)"], [9, 1, "cymbals, second half of the theme"],
      [13, 1, "tutti hit, trumpet fanfare strain"], [17, 1, "cymbals"], [21, 1, "bridge: the band drops out"],
      [24, 1, "A7 stabs on 1, 2-and, 4 (the turn to D minor)"], [25, 1, "tutti hit, theme a whole tone up (D minor)"],
      [29, 1, "cymbals"], [33, 1, "tutti hit, coda"], [35, 1, "stingers on 1, 2-and, 3"],
      [36, 1, "final chord, held"], [36, 3, "the button: last hit"],
    ],
  },
});
