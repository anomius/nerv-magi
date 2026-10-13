// Audio I/O for the scripts: decode anything ffmpeg reads to float32 stereo at 48 kHz, write float32
// WAVs, run ffmpeg, measure loudness. Buffers are { L, R } of Float32Array (one per channel).
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";

export const SR = 48000;

/** Decode `path` to { L, R } at 48 kHz (ffmpeg), optionally from `start` s for `dur` s. */
export const decode = (path, { start, dur } = {}) => {
  const args = ["-hide_banner", "-loglevel", "error"];
  if (start !== undefined) args.push("-ss", String(start));
  args.push("-i", path);
  if (dur !== undefined) args.push("-t", String(dur));
  args.push("-f", "f32le", "-ac", "2", "-ar", String(SR), "-");
  const buf = execFileSync("ffmpeg", args, { maxBuffer: 1 << 30 });
  const n = buf.length / 8;
  const L = new Float32Array(n);
  const R = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    L[i] = buf.readFloatLE(i * 8);
    R[i] = buf.readFloatLE(i * 8 + 4);
  }
  return { L, R };
};

/** Write { L, R } (the first `len` samples, zero-padded) as a float32 stereo WAV, times `scale`. */
export const writeWav = (path, { L, R }, len = L.length, scale = 1) => {
  const out = Buffer.alloc(44 + len * 8);
  out.write("RIFF", 0);
  out.writeUInt32LE(36 + len * 8, 4);
  out.write("WAVE", 8);
  out.write("fmt ", 12);
  out.writeUInt32LE(16, 16);
  out.writeUInt16LE(3, 20); // IEEE float
  out.writeUInt16LE(2, 22);
  out.writeUInt32LE(SR, 24);
  out.writeUInt32LE(SR * 8, 28);
  out.writeUInt16LE(8, 32);
  out.writeUInt16LE(32, 34);
  out.write("data", 36);
  out.writeUInt32LE(len * 8, 40);
  for (let i = 0; i < len; i++) {
    out.writeFloatLE((L[i] ?? 0) * scale, 44 + i * 8);
    out.writeFloatLE((R[i] ?? 0) * scale, 48 + i * 8);
  }
  writeFileSync(path, out);
};

/** Silence of `n` samples. */
export const silence = (n) => ({ L: new Float32Array(n), R: new Float32Array(n) });

/** ffmpeg, quiet, overwriting. */
export const ff = (args) => execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args], { stdio: ["ignore", "inherit", "inherit"] });

/** EBU R128 of a file: integrated loudness (LUFS) and true peak (dBTP). */
export const loudness = (path) => {
  const r = execFileSync("sh", ["-c", `ffmpeg -hide_banner -nostats -i "${path}" -af ebur128=peak=true -f null - 2>&1 | tail -16`], { encoding: "utf8" });
  return { I: Number(r.match(/I:\s+(-?[\d.]+|-inf) LUFS/)?.[1]), TP: Number(r.match(/Peak:\s+(-?[\d.]+|-inf) dBFS/)?.[1]) };
};

/** Two-pass loudnorm (linear) to `I` LUFS, true peak `TP` dBTP; `post` appends filters (e.g. the end gate). */
export const loudnorm = (inp, out, { I = -14, TP = -2, LRA = 11, post = "", codec = "pcm_s24le" } = {}) => {
  const target = `I=${I}:TP=${TP}:LRA=${LRA}`;
  const r = execFileSync("sh", ["-c", `ffmpeg -hide_banner -nostats -i "${inp}" -af loudnorm=${target}:print_format=json -f null - 2>&1`], { encoding: "utf8" });
  const m = JSON.parse(r.slice(r.lastIndexOf("{"), r.lastIndexOf("}") + 1));
  ff([
    "-i", inp, "-af",
    `loudnorm=${target}:measured_I=${m.input_i}:measured_TP=${m.input_tp}:measured_LRA=${m.input_lra}:measured_thresh=${m.input_thresh}:offset=${m.target_offset}:linear=true${post ? `,${post}` : ""}`,
    "-ar", String(SR), "-ac", "2", "-c:a", codec, out,
  ]);
  return loudness(out);
};

/** A fixed gain to `I` LUFS or to a `TP` dBTP ceiling, whichever is lower (no limiter: transients stay). */
export const levelTo = (inp, out, { I = -18, TP = -2, codec = "pcm_s24le" } = {}) => {
  const m = loudness(inp);
  const gain = Math.min(I - m.I, TP - m.TP);
  ff(["-i", inp, "-af", `volume=${gain.toFixed(2)}dB`, "-ar", String(SR), "-ac", "2", "-c:a", codec, out]);
  return { ...loudness(out), gain };
};
