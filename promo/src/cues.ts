/**
 * Every scene start and hit of the edit, labelled, on a timeline: the cue sheet's rows
 * (scripts/cue-sheet.mjs), and with `hit`, the onsets scripts/check-sync.mjs expects in the music.
 */
import type { Timeline } from "./timeline";

export type Cue = { at: number; label: string; hit?: string };

export const cueList = (t: Timeline): Cue[] => [
  { at: 0, label: "power-on: CRT line, boot text" },
  { at: t.boot.kanji, label: "起動 flash" },
  { at: t.scenes.title[0], label: "TITLE CARD smash in (第壱話 / エージェント、襲来): big hit", hit: "title smash" },
  { at: t.title.sub, label: "EPISODE:1 AGENT ATTACK line" },
  { at: t.scenes.pattern[0], label: "smash out → PATTERN DETECTION (hex sweep)", hit: "smash out" },
  { at: t.pattern.header, label: "パターン検出 header" },
  { at: t.scenes.magi[0], label: "MAGI triad draws", hit: "MAGI draw" },
  ...t.magi.votes.map((at, i) => ({ at, label: `MAGI vote ${i + 1}/3 → 承認`, hit: `vote ${["MELCHIOR", "BALTHASAR", "CASPER"][i]}` })),
  { at: t.magi.carried, label: "可決 3/3 APPROVED: big hit", hit: "可決 3/3" },
  { at: t.magi.riser[0], label: "riser into the reveal" },
  { at: t.scenes.reveal[0], label: "REVEAL: push into the dashboard", hit: "reveal push" },
  { at: t.reveal.wide, label: "EVERY AGENT. ONE SCREEN." },
  ...t.reveal.callouts.map((at, i) => ({ at, label: `callout ${i + 1}/4` })),
  { at: t.flow.load, label: "view flow: LOAD", hit: "LOAD wipe" },
  { at: t.flow.sync, label: "view flow: SYNC", hit: "SYNC roll" },
  { at: t.flow.main, label: "view flow: MAIN", hit: "MAIN dissolve" },
  { at: t.flow.alarm, label: "alarm pickup" },
  { at: t.emergency.takeover, label: "EMERGENCY takeover: klaxon", hit: "EMERGENCY" },
  { at: t.emergency.card, label: "BLOCKED PILOT card" },
  { at: t.emergency.clear, label: "pilot answers" },
  { at: t.emergency.resolve, label: "正常 NOMINAL", hit: "NOMINAL" },
  { at: t.scenes.outro[0], label: "OUTRO" },
  { at: t.outro.slam, label: "NERV MAGI title slam: final hit", hit: "NERV MAGI slam" },
  { at: t.outro.cut, label: "hard cut → つづく (silence)" },
  { at: t.total, label: "end" },
];
