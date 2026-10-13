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

## How it is put together

- **One grid.** `src/timeline.ts`: 150 BPM, 4/4, one beat = 24 frames at 60 fps, one bar = 96.
  Every cut, flash, vote and wipe is written in bars and beats (`bb(bar, beat)`). The scenes, the
  MIDI composer and the SFX synth all import it, and `scripts/check-sync.mjs` measures that every
  scripted hit is an onset in the rendered music on its frame.
- **Real footage.** `scripts/record-footage.sh` records clips with `nerv tv --demo --record`
  (headless, fixed 1/60 s clock) and 4K stills with `--snapshot` for sharp push-ins. The demo roster
  is made up. Footage is gitignored and regenerated on demand.
- **Scenes** (`src/scenes/`): power-on, episode card, pattern detection, MAGI deliberation, the
  reveal (a match cut from the MAGI triad into the product's own MAGI panel, then four callouts),
  view flow, EMERGENCY, outro. `src/Film.tsx` strings them together; both aspect ratios share the
  same components and re-lay out the type (`lp(portrait, …)`).
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
