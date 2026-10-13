# Contributing to NERV MAGI

Thanks for helping. Bug reports, new panels, collectors for more hardware, Omarchy widget polish and docs
are all welcome. For anything bigger than a bug fix, open an issue first so we can agree on the shape before
you build it.

## Ground rules

- **Local first.** No network requests and no telemetry. Collectors are read-only towards herdr, the agents and
  their session logs.
- **No real data** in screenshots, fixtures or demo content. Use `nerv tv --demo` (made-up projects and agents)
  and sanitized fixtures (`/home/user`, generic names).
- **Fonts:** only freely licensed ones (OFL or similar), fetched from their source repos by
  `scripts/fetch-fonts.sh` with their licenses next to them. Never commit a commercial font; users can point
  `[fonts]` at their own licensed copies.
- **An Evangelion homage, not Eva material:** no frames, audio, music or logos from the series or the films.
- **The Omarchy plugin folder is the repo root**, so it must never contain symlinks (`omarchy plugin validate`
  rejects them), and the widget must never edit the user's configuration.

## Set up

```bash
git clone https://github.com/anomius/nerv-magi.git && cd nerv-magi
cargo build --release                 # Rust stable
target/release/nerv tv --demo --windowed
target/release/nerv hub               # needs a running herdr for live data
target/release/nerv dump | jq .       # the State every view renders
```

Small machine? Put `[build]` / `jobs = 4` in `.cargo/config.toml`. It is gitignored, so it stays local.

## Where things live

| Path | What |
|---|---|
| `src/collect/` | Data collectors, one thread each. They take a root path (`/proc`, `/sys`, …) so tests can point them at fixtures; they never panic on a missing tool or file and send `SourceOffline` instead. |
| `src/state.rs` | The `State` contract shared by `tv`, `hub` and `dump` |
| `src/tv/` | The GPU window: `views/` (BOOT, MAIN, LOAD, SYNC, EMERGENCY), `panels/`, `fx.rs` (scanlines, grain), `layers.rs`, headless `snapshot.rs` / `record.rs` / `bench.rs` |
| `src/hub/` | The ratatui board |
| `src/control.rs` | Views, keys and remote control (`scripts/tv-ctl.sh`) |
| `scripts/` | herdr actions, casting, the Omarchy widget's status script; shared helpers in `scripts/lib.sh` |
| `BarWidget.qml`, `Panel.qml`, `manifest.json`, `preview.png` | The Omarchy bar widget and its marketplace listing |
| `herdr-plugin.toml` | The herdr plugin (pane, actions, build) |
| `assets/fonts/` | Bundled fonts and their licenses |
| `tests/fixtures/` | Captured, sanitized `/proc`, `/sys` and herdr JSON |
| `promo/` | The launch video (Remotion + real footage from `nerv tv --record`) |

## Before you open a pull request

- **Rust:** run `cargo fmt` (settings in `rustfmt.toml`) and keep `cargo test --release` green. Add tests for
  new parsing or logic: collectors and views have unit tests next to the code.
- **UI changes:** attach before and after renders. They're headless, so no window is needed:
  `nerv tv --demo --scene main --size 1920x1080 --frames 900 --snapshot main.png`
  (`--frames 1500` shows the demo's EMERGENCY). Check that the 4K budget still holds:
  `nerv tv --demo --size 3840x2160 --bench 30`, at most 5 % of one core and 80 MB RSS.
- **Shell:** `bash -n scripts/*.sh` (and `shellcheck` if you have it). Never splice external text (names,
  labels, paths) into a shell string: pass it as an argument.
- **QML:** `qmllint -I /usr/share/omarchy/shell BarWidget.qml Panel.qml`, show external text as
  `Text.PlainText` (or escape it for `StyledText`), and run `omarchy plugin validate .`.
  To try the widget live: `omarchy plugin add /path/to/your/clone --yes`, then `omarchy restart shell`
  after each change (a hot-reloaded widget keeps answering IPC from its old copy).
- **Commits:** small and focused, with messages like `feat(tv): …`, `fix(cast): …`, `docs: …`.
- **In the PR**, say what you tested it on: distro or Omarchy version, Hyprland, herdr version, GPU.

## Reporting a bug

Open an issue with:
- `nerv --version` (or the commit you built), `herdr --version`, `omarchy version` and your GPU.
- What you did, and what you expected instead.
- For data problems, the relevant part of `nerv dump`. Scrub names you don't want public.

By contributing, you agree that your contribution is licensed under the project's MIT license.
