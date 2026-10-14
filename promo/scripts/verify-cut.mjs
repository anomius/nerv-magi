#!/usr/bin/env node
// Verify a finished cut: stream specs, length to the frame, and that every hard cut and flash of the
// timeline happens on its exact frame in the picture (ffmpeg's scene-change score per frame peaks
// there). With audio: loudness, and for the medley the sync check on the audio decoded back out.
//   verify-cut.mjs <video.mp4> [--track tracks/x.json] [--edit teaser15|cut30] [--sync]
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { editLength, editSegments } from "../src/edits.ts";
import { loadTrack } from "./lib/track.mjs";
import { loudness } from "./lib/wav.mjs";

const args = process.argv.slice(2).filter((a) => a !== "--"); // pnpm 10 passes "--" through
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : undefined;
};
const trackFile = opt("--track");
const edit = opt("--edit");
const si = args.indexOf("--sync");
const sync = si >= 0 && args.splice(si, 1);
const file = args[0];
const { tl } = loadTrack(trackFile);
let bad = 0;
const check = (ok, msg) => {
  bad += ok ? 0 : 1;
  console.log(`${ok ? "ok  " : "FAIL"} ${msg}`);
};

const probe = JSON.parse(execFileSync("ffprobe", ["-v", "error", "-show_streams", "-show_format", "-of", "json", file], { encoding: "utf8" }));
const v = probe.streams.find((s) => s.codec_type === "video");
const a = probe.streams.find((s) => s.codec_type === "audio");
const frames = Number(v.nb_frames);
const want = edit ? editLength(tl, edit) : tl.total;
console.log(`${file}: ${v.codec_name} ${v.profile} ${v.width}x${v.height} ${v.r_frame_rate} ${v.pix_fmt}, ${(Number(probe.format.size) / 1e6).toFixed(1)} MB, ${Number(probe.format.duration).toFixed(3)} s` +
  (a ? `; ${a.codec_name} ${a.profile} ${a.sample_rate} Hz ${a.channels} ch ${Math.round(a.bit_rate / 1000)} kb/s` : "; no audio"));
check(v.r_frame_rate === "60/1", `60 fps (${v.r_frame_rate})`);
check(frames === want, `${frames} frames, timeline says ${want} (${(want / 60).toFixed(3)} s)`);
const moov = execFileSync("sh", ["-c", `head -c 200000 "${file}" | grep -c moov || true`], { encoding: "utf8" }).trim();
check(Number(moov) > 0, "faststart (moov before mdat)");

// hard cuts and flashes: where the picture must jump, in this cut's frames
const fullCuts = [
  [tl.scenes.title[0], "title smash in"],
  [tl.scenes.title[1], "smash out"],
  [tl.boot.kanji, "起動 flash"],
  [tl.magi.carried, "可決 flash"],
  [tl.emergency.takeover, "EMERGENCY flash"],
  [tl.emergency.resolve, "NOMINAL flash"],
  [tl.outro.slam, "NERV MAGI slam"],
  [tl.outro.cut, "hard cut → つづく"],
];
const cuts = edit
  ? editSegments(tl, edit).flatMap((s, k) => [
      ...(k ? [[s.at, `splice → bars ${s.bars[0]}-${s.bars[1] - 1}`]] : []),
      ...fullCuts.filter(([f]) => f > s.from && f < s.to).map(([f, l]) => [s.at + f - s.from, l]),
    ])
  : fullCuts;
const out = execFileSync(
  "sh",
  ["-c", `ffmpeg -hide_banner -nostats -i "${file}" -an -vf "scale=320:-2,select='gte(scene,0)',metadata=print:key=lavfi.scene_score:file=-" -f null - 2>/dev/null`],
  { encoding: "utf8", maxBuffer: 1 << 28 },
);
const score = [];
for (const m of out.matchAll(/pts_time:([\d.]+)[\s\S]*?lavfi\.scene_score=([\d.]+)/g)) score[Math.round(Number(m[1]) * 60)] = Number(m[2]);
for (const [f, label] of cuts) {
  const s = score[f] ?? 0;
  const near = Math.max(...[-3, -2, -1, 1, 2, 3].map((d) => score[f + d] ?? 0));
  const before = score[f - 1] ?? 0; // the jump must start on this frame, not the one before
  check(s > 0.15 && s >= near && before < s / 2, `${label.padEnd(26)} frame ${String(f).padStart(5)}  scene score ${s.toFixed(2)} (frame before ${before.toFixed(2)}, neighbours ≤ ${near.toFixed(2)})`);
}

if (a) {
  const l = loudness(file);
  console.log(`     loudness ${l.I} LUFS integrated, true peak ${l.TP} dBTP`);
  check(l.TP <= -1, "true peak ≤ -1 dBTP");
  if (sync) {
    const dir = mkdtempSync(join(tmpdir(), "verify-"));
    const wav = join(dir, "a.wav");
    execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-i", file, "-c:a", "pcm_f32le", wav]);
    try {
      const r = execFileSync(process.execPath, ["--disable-warning=MODULE_TYPELESS_PACKAGE_JSON", "scripts/check-sync.mjs", wav, ...(trackFile ? ["--track", trackFile] : []), ...(edit ? ["--edit", edit] : [])], { encoding: "utf8" });
      console.log(r.trimEnd().replace(/^/gm, "     "));
    } catch (e) {
      console.log(String(e.stdout).replace(/^/gm, "     "));
      check(false, "music sync");
    }
    rmSync(dir, { recursive: true, force: true });
  }
}
console.log(bad ? `verify: ${bad} problem(s)` : "verify: ok");
process.exit(bad ? 1 : 0);
