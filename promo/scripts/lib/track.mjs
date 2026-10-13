// Load a track map (tracks/<name>.json) and its timeline. Paths are taken relative to promo/ (the
// scripts' working directory); a map's `audio` may be relative to promo/, to the repo root or to the map.
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_MAP, makeTimeline } from "../../src/timeline.ts";

export const PROMO = join(dirname(fileURLToPath(import.meta.url)), "../..");

/** { map, tl, file } for a map file, or the default map (the edit as written) when `file` is undefined. */
export const loadTrack = (file) => {
  if (!file) return { map: DEFAULT_MAP, tl: makeTimeline(DEFAULT_MAP), file: join(PROMO, "tracks/house-150.json") };
  const path = isAbsolute(file) ? file : resolve(process.cwd(), file);
  const map = JSON.parse(readFileSync(path, "utf8"));
  if (typeof map.name !== "string" || !/^[a-z0-9][a-z0-9._-]*$/i.test(map.name)) throw new Error(`${file}: "name" must be a file-safe string`);
  return { map, tl: makeTimeline(map), file: path };
};

/** The map's preview audio as an absolute path if it exists, null if not set, or { missing } if not found. */
export const trackAudio = ({ map, file }) => {
  if (!map.audio) return null;
  const candidates = isAbsolute(map.audio) ? [map.audio] : [join(PROMO, map.audio), join(PROMO, "..", map.audio), join(dirname(file), map.audio)];
  return candidates.find((p) => existsSync(p)) ?? { missing: map.audio };
};

/** "bar.beat" of a frame on the track's grid ("28.1", or "1.3+10f" between beats). */
export const barBeat = (tl, frame) => {
  const beats = frame / tl.beat;
  const whole = Math.round(beats);
  const onBeat = Math.abs(frame - whole * tl.beat) <= 0.5;
  const b = onBeat ? whole : Math.floor(beats + 1e-9);
  const label = `${Math.floor(b / tl.bpb) + 1}.${(b % tl.bpb) + 1}`;
  return onBeat ? label : `${label}+${Math.round(frame - b * tl.beat)}f`;
};

/** m:ss.cc */
export const clock = (s) => `${s < 0 ? "-" : ""}${Math.floor(Math.abs(s) / 60)}:${(Math.abs(s) % 60).toFixed(2).padStart(5, "0")}`;
