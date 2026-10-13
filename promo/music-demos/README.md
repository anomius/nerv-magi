# Music demos

Four original instrumental demos in the *idiom* of an anime battle score, for the NERV MAGI launch video: to
audition alongside (or instead of) licensed tracks. Each is 59–70 s, ends cleanly into silence, and sits on a
constant tempo chosen so that a beat is a whole number of frames at 60 fps (and at 30 fps).

| demo | file | BPM | key | music / file length | beat at 60 fps |
| --- | --- | --- | --- | --- | --- |
| Battle march | `battle-march` | 150 | C minor → D minor | 57.6 s / 60.6 s | 24 frames (bar 96) |
| Threat alert | `threat-alert` | 120 | E (Phrygian, chromatic) | 56.0 s / 59.0 s | 30 frames (bar 120) |
| Night rain (jazz ballad) | `night-rain-jazz` | 72 | F minor | 66.7 s / 70.2 s | 50 frames (bar 200) |
| Cold light (ambient) | `cold-light` | 60 | A (Lydian colours) | 64.0 s / 70.0 s | 60 frames (bar 240) |

"Music" ends at the end of the last bar; the file adds the reverb tail, faded to silence. Frames at 30 fps are
half the 60 fps numbers. Bars and beats below are 1-based, and every listed downbeat is a grid-exact hit.

**Originality.** Every melody, chord progression and rhythm here was written for these demos. Only genre and
orchestration traits are borrowed: march snare rudiments, low-brass clusters, brushes, Lydian voicings. Nothing
quotes or paraphrases any Evangelion cue, "A Cruel Angel's Thesis", "Fly Me to the Moon" or "Komm, süsser Tod".
The jazz changes deliberately avoid running round the circle of fifths. No music, MIDI or samples were
downloaded: each demo is composed in code (`<name>.mjs` → `<name>.mid`) and rendered with FluidSynth and the
**FluidR3_GM** SoundFont (MIT licence).

## Render

```sh
node promo/music-demos/render.mjs                    # all four, one after another (~10 s each)
node promo/music-demos/render.mjs battle-march       # one
node promo/music-demos/render.mjs --stems cold-light # balance check: each part's loudness per section
```

Needs `fluidsynth`, `/usr/share/soundfonts/FluidR3_GM.sf2` and `ffmpeg`. Outputs go to `out/` (gitignored):
`<name>.wav` (48 kHz, 24-bit stereo), `<name>.mp3` (320k), `<name>.spectrum.png`, `<name>.wave.png` and
`<name>.check.json`. Renders are deterministic: humanizing uses a seeded PRNG.

- **Less like MIDI**: layered sections; ±8 ms timing and velocity humanizing on everything except grid hits;
  CC11 expression swells; per-part panning (orchestral seating); pitch-bend scoops, vibrato and falls (sax,
  klaxon); a sustain pedal on piano; FluidSynth reverb with a room per demo (`-R 1 -C 0`).
- **Master** (ffmpeg): 28 Hz high-pass, the demo's EQ, a gentle 1.5–1.8:1 compressor, light stereo widening,
  a 4x-oversampled limiter at −2 dBFS, then two-pass `loudnorm` (linear) to **−14 LUFS integrated, true peak
  ≤ −1 dBTP** (WAV −1.6 to −1.9 dBTP, MP3 −1.2 to −1.9 dBTP).
- **Checks**: EBU R128 on the WAV and MP3; `astats`; a spectrogram; per-part stem loudness; an onset check
  that every listed hit's attack sits on the grid. That check runs in two bands and is calibrated against
  isolated FluidR3 hits, which read +4 to +11 ms. All hits come within 4–11 ms of that.

`smf.mjs` is a small MIDI writer for these demos (separate from the promo score's `scripts/music/midi.mjs`).

---

## 1. Battle march: `battle-march`

150 BPM, 4/4, C minor, stepping up a whole tone to D minor at bar 25. 36 bars. Original, rendered with
FluidR3_GM (MIT).

**Instrumentation**: field snare (two concert-snare samples alternating as hands: flams, drags, 5- and 7-stroke
rolls, accents on 2 and 4), concert bass drum, clash and suspended cymbals, timpani tuned to tonic and dominant
(C/G, then D/A); the theme in horns and trombones in octaves; trumpet off-beat stabs and a repeated-note fanfare;
brass section, tuba, contrabass; strings with scale runs into the downbeats; tremolo strings in two sections;
piccolo on the last strain.

| bar.beat | s | frame @60 | |
| --- | --- | --- | --- |
| 1.1 | 0.00 | 0 | drum call: snare and bass drum alone |
| 3.1 | 3.20 | 192 | low brass and tremolo on the dominant, crescendo |
| **5.1** | 6.40 | 384 | tutti hit, theme in horns and trombones (C minor) |
| 9.1 | 12.80 | 768 | cymbals, second half of the theme |
| **13.1** | 19.20 | 1152 | tutti hit, trumpet fanfare strain |
| 17.1 | 25.60 | 1536 | cymbals |
| **21.1** | 32.00 | 1920 | bridge: the band drops out (quietest part, then a crescendo) |
| 24.1 | 36.80 | 2208 | A7 stabs on 1, 2-and, 4 (the turn to D minor) |
| **25.1** | 38.40 | 2304 | tutti hit, the theme a whole tone up (D minor) |
| 29.1 | 44.80 | 2688 | cymbals |
| **33.1** | 51.20 | 3072 | tutti hit, coda |
| 35.1 | 54.40 | 3264 | stingers on 1, 2-and, 3 |
| **36.1** | 56.00 | 3360 | final chord, held 1¾ beats |
| **36.3** | 56.80 | 3408 | the button: last hit, then the hall |

## 2. Threat alert: `threat-alert`

120 BPM, 4/4, centred on E (minor 2nds and tritones), climbing to G. 28 bars. Original, rendered with
FluidR3_GM (MIT).

**Instrumentation**: a 16th-note ostinato in low strings, contrabass and piano (accents grouped 3+3+3+3+4);
trombones, tuba and horns in swelling minor-2nd and tritone clusters, plus horn and brass-section stabs. The
klaxon-like figure is a square lead doubled by muted trumpets, each pulse falling off in pitch. Then timpani
(E/B♭), taiko, concert bass drum, a snare roll, cymbals and reverse-cymbal swells (peaks timed to the
downbeats), high tremolo strings, a choir cluster and high pizzicato ticks.

| bar.beat | s | frame @60 | |
| --- | --- | --- | --- |
| 1.1 | 0.00 | 0 | low drone, piano low E, timpani roll |
| **3.1** | 4.00 | 240 | the ostinato enters (a reverse cymbal lands here) |
| **7.1** | 12.00 | 720 | the klaxon enters, with timpani and taiko |
| **11.1** | 20.00 | 1200 | horn stabs, choir and high tremolo |
| **15.1** | 28.00 | 1680 | the climb: up a semitone every two bars (15, 17, 19, 21) |
| 21.1 | 40.00 | 2400 | snare roll and cymbal swell begin |
| **23.1** | 44.00 | 2640 | CRASH |
| 25.1 | 48.00 | 2880 | aftermath: the ostinato on piano alone, a last distant klaxon |
| **27.1** | 52.00 | 3120 | the last low hit, then the hall |

## 3. Night rain (jazz ballad): `night-rain-jazz`

72 BPM, 4/4 with swung 8ths, F minor. 20 bars. Original, rendered with FluidR3_GM (MIT).

**Instrumentation**: alto sax lead (scoops, delayed vibrato, expression swells, sitting slightly behind the
beat); piano comping in rootless voicings with pedal, plus high "raindrop" fills; upright bass (two-feel in A,
walking in B); brushes (swirl, taps, slaps, hi-hat foot, a ride from bar 13).

Changes: | Fm9 | G♭maj7♯11 | E♭m9 A♭13 | D♭maj9 C7♯9♭13 | Fm9 | A♭m9 D♭13 | G♭maj7♯11 C7♭9 | Fm(maj9) |
D♭maj9 | Cm7 F7♭9 | B♭m9 | Gm7♭5 C7♭9 | Fm9 Fm9/E♭ | Dm7♭5 D♭7♯11 | C7sus4 C7♭9 | Fm9 | D♭maj7♯11 C7alt |
Fm(maj9) |

| bar.beat | s | frame @60 | |
| --- | --- | --- | --- |
| 1.1 | 0.00 | 0 | piano intro |
| 2.4 | 5.83 | 350 | sax pickup |
| **3.1** | 6.67 | 400 | sax melody, bass and brushes in |
| 7.1 | 20.00 | 1200 | second phrase |
| **11.1** | 33.33 | 2000 | B: the bass walks |
| 13.1 | 40.00 | 2400 | the ride cymbal joins |
| 15.1 | 46.67 | 2800 | last phrase; the bass steps down F–E♭–D–D♭–C |
| 19.1 | 60.00 | 3600 | tag |
| **20.1** | 63.33 | 3800 | final chord, rings out |

## 4. Cold light (ambient): `cold-light`

60 BPM, 4/4, A with Lydian colours (open fifths, ninths, major sevenths, raised fourths; few thirds). 16 bars,
a chord every two bars. Original, rendered with FluidR3_GM (MIT).

**Instrumentation**: sparse high piano with pedal, and soft low fifths on the chord changes; slow strings in
two sections that swell and ease back on every chord; a faint choir pad; a very faint bowed-glass shimmer in
bars 11–14. Large hall.

| bar.beat | s | frame @60 | |
| --- | --- | --- | --- |
| 1.1 | 0.00 | 0 | strings alone, swelling in |
| 2.3 | 6.00 | 360 | first piano note |
| **3.1** | 8.00 | 480 | piano phrases; a new chord every two bars (3, 5, 7, 9, 11, 13, 15) |
| 5.1 | 16.00 | 960 | choir pad enters |
| **11.1** | 40.00 | 2400 | the lift: F major 7 ♯11, bowed-glass shimmer |
| 13.1 | 48.00 | 2880 | return |
| 15.1 | 56.00 | 3360 | close |
| **16.1** | 60.00 | 3600 | last piano dyad, then everything fades |
