// Cue sheet for laying your own music under the cut: every scene start and hit, in seconds and
// frames, straight from src/timeline.ts (Node strips the types). Writes out/byo-audio/cues.txt
// (readable) and cues.csv (import as markers in an editor).
//   node scripts/cue-sheet.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { T, FPS, BPM, bb } from "../src/timeline.ts";

const cues = [
  [0, "power-on: CRT line, boot text"],
  [T.boot.kanji, "起動 flash"],
  [T.scenes.title[0], "TITLE CARD smash in (第壱話 / エージェント、襲来) — big hit"],
  [T.title.sub, "EPISODE:1 AGENT ATTACK line"],
  [T.scenes.pattern[0], "smash out → PATTERN DETECTION (hex sweep)"],
  [T.pattern.header, "パターン検出 header"],
  [T.scenes.magi[0], "MAGI triad draws"],
  ...T.magi.votes.map((f, i) => [f, `MAGI vote ${i + 1}/3 → 承認`]),
  [T.magi.carried, "可決 3/3 APPROVED — big hit"],
  [T.magi.riser[0], "riser into the reveal"],
  [T.scenes.reveal[0], "REVEAL: push into the dashboard"],
  [T.reveal.wide, "EVERY AGENT. ONE SCREEN."],
  ...T.reveal.callouts.map((f, i) => [f, `callout ${i + 1}/4`]),
  [T.flow.load, "view flow: LOAD"],
  [T.flow.sync, "view flow: SYNC"],
  [T.flow.main, "view flow: MAIN"],
  [T.flow.alarm, "alarm pickup"],
  [T.emergency.takeover, "EMERGENCY takeover — klaxon"],
  [T.emergency.card, "BLOCKED PILOT card"],
  [T.emergency.clear, "pilot answers"],
  [T.emergency.resolve, "正常 NOMINAL"],
  [T.scenes.outro[0], "OUTRO"],
  [T.outro.slam, "NERV MAGI title slam — final hit"],
  [T.outro.cut, "hard cut → つづく"],
  [T.total, "end"],
];

const ts = (f) => {
  const s = f / FPS;
  return `${String(Math.floor(s / 60)).padStart(1, "0")}:${(s % 60).toFixed(2).padStart(5, "0")}`;
};
const bar = (f) => `bar ${Math.floor(f / bb(2)) + 1}.${Math.floor((f % bb(2)) / bb(1, 2)) + 1}`;

mkdirSync("out/byo-audio", { recursive: true });
const head = `NERV MAGI promo — cue sheet (${BPM} BPM, 4/4, ${FPS} fps, ${ts(T.total)} long)\n` +
  `Hits sit on bar lines: one bar = ${(bb(2) / FPS).toFixed(2)} s. Align your track's downbeat to 0:00.00 (or offset all cues).\n\n`;
writeFileSync("out/byo-audio/cues.txt", head + cues.map(([f, l]) => `${ts(f)}  ${bar(f).padEnd(9)}  frame ${String(f).padStart(4)}  ${l}`).join("\n") + "\n");
writeFileSync("out/byo-audio/cues.csv", "seconds,frame,bar,label\n" + cues.map(([f, l]) => `${(f / FPS).toFixed(3)},${f},${bar(f)},"${l.replace(/"/g, "'")}"`).join("\n") + "\n");
console.log(`wrote out/byo-audio/cues.txt and cues.csv (${cues.length} cues)`);
