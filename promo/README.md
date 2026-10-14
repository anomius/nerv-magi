# NERV MAGI · launch video

A 57.6 s motion graphic for X, built with [Remotion](https://www.remotion.dev) 4 from real
footage of `nerv tv` (no mock-ups), cut to a 150 BPM score: an "Eva medley" of public-domain
classics and an original march.

```sh
cd promo && pnpm i && pnpm run render
```

`render` does everything: copies the fonts, checks every Japanese glyph against the font subset,
records the dashboard footage if it is missing (`cargo build --release`, then `nerv tv --record`),
renders the soundtrack and checks its sync, renders the compositions with Remotion
(`--concurrency 2`) and encodes them for X with ffmpeg. Render only some outputs with
`pnpm run render -- master poster` (ids: `master 4x5 hero poster frames`). After a change to the
music only, `pnpm run audio && pnpm run mux` puts the new soundtrack on the existing renders.

Needs: Node ≥ 22.18 (the scripts import `src/timeline.ts` directly), pnpm, ffmpeg, FluidSynth and
the FluidR3_GM soundfont (`/usr/share/soundfonts/FluidR3_GM.sf2`, or set `SOUNDFONT`), `uvx`
(fonttools, for the glyph check) and a Rust toolchain for the footage.

| output | |
|---|---|
| `out/nerv-magi-x.mp4` | master, 1920×1080, 60 fps, H.264 High, AAC 48 kHz 192k, −14 LUFS |
| `out/nerv-magi-x-4x5.mp4` | 1080×1350 recompose for the mobile feed (same edit and music) |
| `out/hero-loop.mp4` | 7 s seamless loop (picture) with 7 s of the march; `hero-loop-silent.mp4` without |
| `out/poster.png` | 1920×1080 still |
| `out/frames/`, `out/contact-sheet*.png` | a keyframe every 2 s, for review |

More cuts, each with the medley, `-sfx-only` and `-silent` versions and a cue sheet (`cues.txt`, `cues.csv`):

| command | output |
|---|---|
| `pnpm run render:cut -- teaser15` | `out/formats/teaser15/`: ≈15 s, title card → 可決 → into the dashboard → NERV MAGI (9 bars, 14.4 s) |
| `pnpm run render:cut -- cut30` | `out/formats/cut30/`: ≈30 s, title → pattern → MAGI → reveal → EMERGENCY → outro (19 bars, 30.4 s) |
| `pnpm run render:cut -- vertical` | `out/formats/vertical/`: 1080×1920 for Shorts, Reels, TikTok (the full edit, re-laid out) |
| `pnpm run render:4k` | `out/4k/`: 3840×2160 master for YouTube (run `pnpm run footage:4k` once first: the dashboard re-recorded at 4K, laid in at native size under Remotion `--scale 2`; H.264 High CRF 16, closed 30-frame GOP) |
| `pnpm run render:track -- tracks/<name>.json` | `out/tracks/<name>/`: the edit re-timed to your track (below) |

`render:formats` renders the three short/vertical cuts in a row. Every render waits for 3 GB of free
memory first (`scripts/mem-guard.sh`).

The 4K footage needs care: the demo feed scripts its drama (the BLOCKED agent) on the process clock,
and at 3840×2160 the recorder is slower than real time, so on the real clock the drama races ahead of
the frames. `footage:4k` runs `nerv` under `scripts/rec-clock` (an `LD_PRELOAD` shim that slows its
clocks to 0.2×: the recorder paces itself at 12 fps and each clip tells its 1080p twin's story) with
x264 at ultrafast CRF 10 (`scripts/rec-ffmpeg`). Where the takeover and the wipe back to MAIN land in a
recording is measured into `public/<footage>/clips.json` (`scripts/measure-clips.mjs`), and the edit
enters the clips there.

The short cuts are cut on bar lines
(`src/edits.ts`); their medley is the score spliced at the same bar lines (4 ms in, 60 ms ring-out), so
every hit stays on its frame. The vertical lays the 4:5 type out in a 1080×1320 stage between the
platforms' unsafe bands (no text in the top 220 px or the bottom 380 px); backgrounds bleed to the edges.

## Re-time the edit to your own track

The edit is written once on a 150 BPM grid; a **track map** re-times it to any track. Licensed audio
stays out of git: put it in `promo/byo-audio/` (ignored).

1. Put the track at `byo-audio/tracks/<name>.wav` (anything ffmpeg reads).
2. Estimate its grid: `pnpm run tempo -- byo-audio/tracks/<name>.wav --name <name> --out tracks/<name>.json`.
   It prints BPM, the first downbeat (`offsetSec`) and how sure it is. **It is an estimate**: half or
   double tempo and a downbeat one beat off are the usual misses.
3. Confirm by ear, in seconds: `pnpm run render:track -- tracks/<name>.json --audio-only`, then play
   `out/tracks/<name>/check.wav` (your track with a click on every beat; the high click is the downbeat).
   Fix `bpm` / `offsetSec` until the high click sits on the bar lines. `cues.txt` lists every hit with
   its bar and its time in your file.
4. Optional, `sections`: the bar of your track (bar 1 = `offsetSec`) where each section starts
   (`title pattern magi reveal flow emergency outro end`; written: 3 5 9 15 25 28 32 37). Omitted ones
   keep today's proportions, so a bpm-only map works. Put 可決 (`magi` + 4 bars) on a drop, the slam on
   the last chorus, etc. Sections at their written length keep every hit where it is written; a longer
   one stretches its choreography (hits on beats), a shorter one (down to half) squeezes it.
5. Render: `pnpm run render:track -- tracks/<name>.json [master vertical teaser15 cut30]` (≈10 min for
   the master). Out: `nerv-magi-<name>-sfx-only.mp4` (lay your track under it at −`offsetSec`),
   `-guide.mp4` (SFX + click), `-preview.mp4` (your track laid in, ducked under the SFX, −14 LUFS:
   private, never publish licensed music you have not cleared), `click.wav`, `cues.txt`/`cues.csv`.

```json
{ "name": "my-track", "bpm": 128, "beatsPerBar": 4, "offsetSec": 1.875,
  "audio": "byo-audio/tracks/my-track.wav", "sections": { "magi": 9, "end": 37 } }
```

`tracks/house-150.json` is the edit as written (the medley); `example-128.json` (bpm only) and
`example-140.json` (a longer intro and outro) are made-up maps that show the re-timing. `preflight.mjs`
checks that the recorded clips are long enough for a map (a slow track plays more of each clip).

## How it is put together

- **One grid.** `src/timeline.ts`: written at 150 BPM, 4/4, one beat = 24 frames at 60 fps, one bar
  = 96. Every cut, flash, vote and wipe is a bar and beat (`bb(bar, beat)`). `makeTimeline(map)`
  re-times all of it to a track map (input props `{ track }`; cues rounded to a frame once each, so
  128 BPM's 28.125 frames a beat never drift; scene-local animation runs on a tempo-scaled clock).
  The scenes read it from React context (`src/tl.tsx`); the MIDI composer, the SFX synth
  (`scripts/sfx.mjs`), the click, the cue sheet and `scripts/check-sync.mjs` call it directly, and
  the sync check measures that every scripted hit is an onset in the rendered music on its frame.
- **Real footage.** `scripts/record-footage.sh` records clips with `nerv tv --demo --record`
  (headless, fixed 1/60 s clock) and 4K stills with `--snapshot` for sharp push-ins. The demo roster
  is made up. Footage is gitignored and regenerated on demand.
- **Scenes** (`src/scenes/`): power-on, episode card, pattern detection, MAGI deliberation, the
  reveal (a match cut from the MAGI triad into the product's own MAGI panel, then four callouts),
  view flow, EMERGENCY, outro. `src/Film.tsx` strings them together; every aspect ratio shares the
  same components and re-lays out the type (`lp(portrait, …)`; 9:16 on a safe-band stage).
  `src/Cut.tsx` plays bar ranges of the film back to back for the short cuts (`src/edits.ts`).
- **Checks.** `scripts/verify-cut.mjs <mp4> [--track] [--edit] [--sync]`: streams, frame count, that
  every hard cut and flash falls on its exact frame (scene-change score), loudness, and the music sync
  in the audio decoded back out.
- **Look.** Palette from `src/tv/gfx.rs`, and only the OFL fonts in `../assets/fonts` (copied, never
  symlinked: the repo root is an Omarchy plugin).

## Music and sound (credits)

The score is an "Eva medley" (`scripts/music/compose.mjs` → `nerv-magi-score.mid`): public-domain
works that Evangelion made iconic, performed from our own MIDI, around an original battle march.
Nothing from any Evangelion soundtrack is used or imitated.

| bars | piece | Eva nod |
|---|---|---|
| 1–2 | **J. S. Bach**, Cello Suite No. 1 in G major, **BWV 1007** (c. 1720), Prelude, m. 1, solo cello | Shinji's cello (ep. 15) |
| 3–30 | original battle march in D minor (field snare, timpani ostinato, horns and trombones in octaves), up a whole step to E minor for the climax; 可決 is a tutti D-major fanfare; EMERGENCY is dissonant low-brass clusters with taiko | the genre's idiom only |
| 31–36 | **L. van Beethoven**, Symphony No. 9 in D minor, **Op. 125** (1824), finale, "Ode to Joy" theme (consequent phrase, D major), choir, strings, organ, brass | Kaworu (ep. 24) |

Handel's "Hallelujah" (Messiah, HWV 56; ep. 22) was considered for 可決 but not quoted: its exact
opening could not be verified here, so the bar is an original fanfare instead.
The first score (march + Ode only) is kept as `compose-v1.mjs` / `nerv-magi-score-v1.mid`.

- **Rendering:** FluidSynth 2 with the FluidR3_GM soundfont by Frank Wen (MIT license), hall reverb,
  then a gentle bus compressor and a little stereo width.
- **SFX:** synthesized from scratch in `scripts/make-audio.mjs` (no samples): CRT power-on, typing,
  hex and MAGI vote blips, whooshes, riser, a two-tone klaxon (D–A♭). The music ducks under them;
  the mix is normalized to −14 LUFS integrated, true peak ≤ −1 dBTP.
- **Sync:** `scripts/check-sync.mjs` checks that every scripted hit is an onset on its frame, in the
  stem, the mix, and the audio decoded back out of the final MP4.

## Fonts

Oswald, Barlow Condensed, Fira Code, Noto Serif JP (Black, subset) and DSEG7: all SIL OFL 1.1, see
`../assets/fonts/OFL-*.txt`.

## Homage, not copy

Evangelion's visual language (episode cards, MAGI deliberation, hex grids, warning tape) is
referenced in original artwork only: no frames, logos, music or sound effects from the show or the
films.
