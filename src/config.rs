//! User config: `$HERDR_PLUGIN_CONFIG_DIR/config.toml`, else `~/.config/nerv/config.toml`, else defaults.
//! Assets (fonts): `$NERV_ASSETS`, else `$HERDR_PLUGIN_ROOT/assets`, else next to the binary's repo
//! (`<repo>/target/release/nerv` → `<repo>/assets`), else the compile-time manifest dir.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// MELCHIOR approves while cpu_total is below this (0..1).
    pub cpu_max: f32,
    /// …and the CPU package temperature is below this.
    pub cpu_temp_max_c: f32,
    /// BALTHASAR approves while MemAvailable is at least this.
    pub mem_min_available_bytes: u64,
    /// Below this MemAvailable the TV takes over with the EMERGENCY view (memory critical).
    pub mem_critical_bytes: u64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            cpu_max: 0.85,
            cpu_temp_max_c: 90.0,
            mem_min_available_bytes: 1_500_000_000,
            mem_critical_bytes: 800_000_000,
        }
    }
}

/// Optional overrides: absolute paths to font files (e.g. a licensed Matisse EB for `mincho`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FontOverrides {
    pub display: Option<PathBuf>,
    pub display_bold: Option<PathBuf>,
    pub label: Option<PathBuf>,
    pub mono: Option<PathBuf>,
    pub mincho: Option<PathBuf>,
    pub segment: Option<PathBuf>,
}

/// CRT scanline look. `Off` < `Subtle` < `Heavy` < `Crt` is also the runtime cycle order
/// (`s` key in the TV window, SIGUSR1, or the herdr action `tv-scanlines`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ScanlineMode {
    Off,
    /// thin black line every 4 design px at 6 % (the original look)
    #[default]
    Subtle,
    /// thicker lines every 3 px at 14 %
    Heavy,
    /// 2 px lines at 18 % + RGB phosphor stripe mask + slight bloom on bright text
    Crt,
}

impl ScanlineMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Subtle,
            Self::Subtle => Self::Heavy,
            Self::Heavy => Self::Crt,
            Self::Crt => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Subtle => "SUBTLE",
            Self::Heavy => "HEAVY",
            Self::Crt => "CRT",
        }
    }
}

/// Mouse pointer in the TV window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorMode {
    /// Windowed: always visible. Fullscreen: shown when the mouse moves, hidden after
    /// `cursor_hide_secs` without movement.
    #[default]
    Auto,
    Always,
    Never,
}

impl CursorMode {
    /// Should the pointer be visible right now?
    pub fn visible(self, fullscreen: bool, secs_since_mouse_move: f32, hide_after: f32) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => !fullscreen || secs_since_mouse_move < hide_after,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TvConfig {
    /// Hyprland workspace for `tv-start` (e.g. the omarchy-cast TV desktop). None = current.
    pub workspace: Option<String>,
    /// AirPlay receiver for `tv-cast` (IP). None = the first paired receiver.
    pub cast_receiver: Option<String>,
    /// Send system audio to the TV while casting.
    pub cast_audio: bool,
    /// Master switch for scanlines (kept as a bool for older configs); the look is `scanline_style`.
    pub scanlines: bool,
    pub scanline_style: ScanlineMode,
    /// A soft bright band rolling down the screen every ~8 s, like a CRT refresh bar.
    pub scanline_roll: bool,
    /// 0.97–1.0 brightness flicker.
    pub crt_flicker: bool,
    /// Darken the screen edges.
    pub vignette: bool,
    /// Animated film grain over the whole screen (`g` / `tv-ctl grain` toggles at runtime).
    pub grain: bool,
    /// 0..1; 1.0 ≈ 8 % peak noise alpha. Default 0.35 keeps small text crisp.
    pub grain_strength: f32,
    pub cursor: CursorMode,
    pub cursor_hide_secs: f32,
    /// Start fullscreen (`--windowed` overrides; `f` toggles at runtime).
    pub start_fullscreen: bool,
    /// When a view is pinned (keys 1–4 / tv-ctl), EMERGENCY still takes over.
    pub emergency_overrides_pin: bool,
    pub fps_active: u32,
    pub fps_calm: u32,
    /// Rotate MAIN ⇄ LOAD views; EMERGENCY always takes over when needed.
    pub rotate: bool,
    pub main_secs: u32,
    pub load_secs: u32,
    /// SYNC (pilot usage) view slot in the rotation; 0 disables it.
    pub sync_secs: u32,
    /// Length of the BOOT (loading) view on launch.
    pub boot_secs: u32,
    /// Minimum time EMERGENCY stays up after its cause clears (avoids flapping).
    pub emergency_hold_secs: u32,
}

impl TvConfig {
    /// Effective scanline mode: `Off` when the master switch is off, else the style.
    pub fn scanline_mode(&self) -> ScanlineMode {
        if self.scanlines {
            self.scanline_style
        } else {
            ScanlineMode::Off
        }
    }
}

impl Default for TvConfig {
    fn default() -> Self {
        Self {
            workspace: None,
            cast_receiver: None,
            cast_audio: false,
            scanlines: true,
            scanline_style: ScanlineMode::Subtle,
            scanline_roll: true,
            crt_flicker: false,
            vignette: true,
            grain: true,
            grain_strength: 0.35,
            cursor: CursorMode::Auto,
            cursor_hide_secs: 3.0,
            start_fullscreen: true,
            emergency_overrides_pin: true,
            fps_active: 60,
            fps_calm: 10,
            rotate: true,
            main_secs: 45,
            load_secs: 15,
            sync_secs: 15,
            boot_secs: 4,
            emergency_hold_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SpectrumConfig {
    pub bars: usize,
    pub framerate: u32,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self { bars: 48, framerate: 30 }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub thresholds: Thresholds,
    pub fonts: FontOverrides,
    pub tv: TvConfig,
    pub spectrum: SpectrumConfig,
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("HERDR_PLUGIN_CONFIG_DIR") {
            return Some(PathBuf::from(dir).join("config.toml"));
        }
        std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config/nerv/config.toml"))
    }

    /// Missing file → defaults. A malformed file is an error (better than silently ignoring it).
    pub fn load() -> anyhow::Result<Self> {
        match Self::path() {
            Some(p) if p.exists() => {
                Self::parse(&std::fs::read_to_string(&p)?).map_err(|e| anyhow::anyhow!("{}: {e}", p.display()))
            }
            _ => Ok(Self::default()),
        }
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(toml::from_str(text)?)
    }
}

pub fn asset_dir() -> PathBuf {
    if let Ok(p) = std::env::var("NERV_ASSETS") {
        return PathBuf::from(p);
    }
    if let Ok(root) = std::env::var("HERDR_PLUGIN_ROOT") {
        let p = PathBuf::from(root).join("assets");
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        // <repo>/target/{release,debug}/nerv
        if let Some(repo) = exe.parent().and_then(Path::parent).and_then(Path::parent) {
            let p = repo.join("assets");
            if p.is_dir() {
                return p;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_defaults() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.thresholds.cpu_max, 0.85);
        assert_eq!(c.spectrum.bars, 48);
        assert!(c.tv.workspace.is_none());
        assert!(c.tv.rotate);
        assert_eq!(
            (c.tv.main_secs, c.tv.load_secs, c.tv.sync_secs, c.tv.boot_secs, c.tv.emergency_hold_secs),
            (45, 15, 15, 4, 5)
        );
        assert_eq!(c.thresholds.mem_critical_bytes, 800_000_000);
    }

    #[test]
    fn partial_config_overrides_only_given_keys() {
        let c = Config::parse("[tv]\nworkspace = \"name:TV\"\n[thresholds]\ncpu_max = 0.5\n").unwrap();
        assert_eq!(c.tv.workspace.as_deref(), Some("name:TV"));
        assert!(c.tv.cast_receiver.is_none() && !c.tv.cast_audio);
        assert_eq!(
            Config::parse("[tv]\ncast_receiver = \"192.0.2.7\"\n").unwrap().tv.cast_receiver.as_deref(),
            Some("192.0.2.7")
        );
        assert_eq!(c.tv.fps_active, 60);
        assert_eq!(c.thresholds.cpu_max, 0.5);
        assert_eq!(c.thresholds.mem_min_available_bytes, 1_500_000_000);
    }

    #[test]
    fn scanline_mode_from_config_and_cycle() {
        assert_eq!(Config::parse("").unwrap().tv.scanline_mode(), ScanlineMode::Subtle);
        assert_eq!(Config::parse("[tv]\nscanlines = false\n").unwrap().tv.scanline_mode(), ScanlineMode::Off);
        assert_eq!(Config::parse("[tv]\nscanline_style = \"crt\"\n").unwrap().tv.scanline_mode(), ScanlineMode::Crt);
        assert!(Config::parse("[tv]\nscanline_style = \"neon\"\n").is_err());
        let mut m = ScanlineMode::Off;
        let seen: Vec<_> = (0..5)
            .map(|_| {
                m = m.next();
                m
            })
            .collect();
        assert_eq!(
            seen,
            [ScanlineMode::Subtle, ScanlineMode::Heavy, ScanlineMode::Crt, ScanlineMode::Off, ScanlineMode::Subtle]
        );
    }

    #[test]
    fn cursor_mode_visibility() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.tv.cursor, CursorMode::Auto);
        assert!(c.tv.grain);
        assert_eq!(c.tv.grain_strength, 0.35);
        assert!(CursorMode::Auto.visible(false, 99.0, 3.0)); // windowed: always
        assert!(CursorMode::Auto.visible(true, 1.0, 3.0)); // fullscreen, just moved
        assert!(!CursorMode::Auto.visible(true, 5.0, 3.0)); // fullscreen, idle
        assert!(!CursorMode::Never.visible(false, 0.0, 3.0));
        assert!(CursorMode::Always.visible(true, 99.0, 3.0));
        assert_eq!(Config::parse("[tv]\ncursor = \"never\"\n").unwrap().tv.cursor, CursorMode::Never);
    }

    #[test]
    fn malformed_config_is_an_error() {
        assert!(Config::parse("[tv\nworkspace=").is_err());
    }

    #[test]
    fn bundled_fonts_exist() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
        for f in [
            "Oswald-Medium.ttf",
            "Oswald-Bold.ttf",
            "BarlowCondensed-Medium.ttf",
            "BarlowCondensed-SemiBold.ttf",
            "FiraCode-Medium.ttf",
            "NotoSerifJP-Black-subset.ttf",
            "DSEG7Classic-BoldItalic.ttf",
        ] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
    }
}
