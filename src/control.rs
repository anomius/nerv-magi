//! Keyboard and remote control of the TV window: which view is shown, rotation pin, fullscreen,
//! scanlines. Remote path: `scripts/tv-ctl.sh <command>` appends a line to
//! `<runtime_dir>/nerv-tv.cmd` and sends SIGUSR2; the TV drains the file with `take_commands`.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The views a person can pick (BOOT is never picked; it only shows on launch / reconnect).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ViewName {
    Main,
    Load,
    Sync,
    Emergency,
}

impl ViewName {
    pub const ALL: [ViewName; 4] = [ViewName::Main, ViewName::Load, ViewName::Sync, ViewName::Emergency];

    pub fn label(self) -> &'static str {
        match self {
            Self::Main => "MAIN",
            Self::Load => "LOAD",
            Self::Sync => "SYNC",
            Self::Emergency => "EMERGENCY",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "main" => Some(Self::Main),
            "load" => Some(Self::Load),
            "sync" => Some(Self::Sync),
            "emergency" => Some(Self::Emergency),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvCommand {
    /// Show this view and pin it (rotation paused).
    View(ViewName),
    Next,
    Prev,
    /// Back to automatic rotation (MAIN → LOAD → SYNC, EMERGENCY takeover).
    Auto,
    /// Pin the current view, or resume rotation if already pinned.
    TogglePin,
    ToggleFullscreen,
    Scanlines,
    ToggleGrain,
    ToggleHelp,
    Quit,
}

impl TvCommand {
    /// Remote command words (one per line in the command file).
    pub fn parse(s: &str) -> Option<Self> {
        let w = s.trim().to_ascii_lowercase();
        if let Some(v) = ViewName::parse(&w) {
            return Some(Self::View(v));
        }
        match w.as_str() {
            "next" => Some(Self::Next),
            "prev" | "previous" => Some(Self::Prev),
            "auto" | "rotate" => Some(Self::Auto),
            "pin" => Some(Self::TogglePin),
            "fullscreen" | "fs" => Some(Self::ToggleFullscreen),
            "scanlines" => Some(Self::Scanlines),
            "grain" => Some(Self::ToggleGrain),
            "help" => Some(Self::ToggleHelp),
            _ => None,
        }
    }
}

/// Keyboard map. `key` is winit's logical key as text: a character ("1", "s", " ") or a named key
/// ("Tab", "ArrowRight", "ArrowLeft", "Escape").
pub fn key_command(key: &str, shift: bool) -> Option<TvCommand> {
    Some(match key {
        "1" => TvCommand::View(ViewName::Main),
        "2" => TvCommand::View(ViewName::Load),
        "3" => TvCommand::View(ViewName::Sync),
        "4" => TvCommand::View(ViewName::Emergency),
        "Tab" if shift => TvCommand::Prev,
        "Tab" | "ArrowRight" => TvCommand::Next,
        "ArrowLeft" => TvCommand::Prev,
        "0" | "a" => TvCommand::Auto,
        " " | "Space" | "p" => TvCommand::TogglePin,
        "f" | "F11" => TvCommand::ToggleFullscreen,
        "s" => TvCommand::Scanlines,
        "g" => TvCommand::ToggleGrain,
        "?" | "h" => TvCommand::ToggleHelp,
        "q" | "Escape" => TvCommand::Quit,
        _ => return None,
    })
}

/// Rows for the on-screen help overlay (`?`).
pub const KEY_HELP: &[(&str, &str)] = &[
    ("1 2 3 4", "MAIN · LOAD · SYNC · EMERGENCY (pins the view)"),
    ("TAB  →  ←", "next / previous view"),
    ("SPACE  P", "pin / unpin the current view"),
    ("0  A", "automatic rotation"),
    ("F  F11", "fullscreen / window"),
    ("S", "scanlines: off · subtle · heavy · crt"),
    ("G", "film grain on / off"),
    ("?  H", "this help"),
    ("Q  ESC", "quit"),
];

/// Whether the view rotates or is pinned by the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Auto,
    Pinned(ViewName),
}

impl ViewMode {
    /// Apply a view command. `current` is the view on screen now (what Next/Prev/TogglePin act on).
    /// Non-view commands return the mode unchanged.
    pub fn apply(self, cmd: TvCommand, current: ViewName) -> ViewMode {
        match cmd {
            TvCommand::View(v) => ViewMode::Pinned(v),
            TvCommand::Next => ViewMode::Pinned(current.next()),
            TvCommand::Prev => ViewMode::Pinned(current.prev()),
            TvCommand::Auto => ViewMode::Auto,
            TvCommand::TogglePin => match self {
                ViewMode::Auto => ViewMode::Pinned(current),
                ViewMode::Pinned(_) => ViewMode::Auto,
            },
            _ => self,
        }
    }

    /// Status-strip text, e.g. "AUTO" or "PINNED · LOAD".
    pub fn label(self) -> String {
        match self {
            ViewMode::Auto => "AUTO".into(),
            ViewMode::Pinned(v) => format!("PINNED · {}", v.label()),
        }
    }
}

/// Where the pid and command files live: `$XDG_RUNTIME_DIR`, else a private `/tmp/nerv-<uid>`
/// (mode 0700). `scripts/lib.sh` uses the same rule.
pub fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    use std::os::unix::fs::DirBuilderExt;
    // SAFETY: getuid has no failure mode.
    let dir = PathBuf::from(format!("/tmp/nerv-{}", unsafe { libc::getuid() }));
    let _ = std::fs::DirBuilder::new().mode(0o700).create(&dir);
    dir
}

pub fn cmd_path() -> PathBuf {
    runtime_dir().join("nerv-tv.cmd")
}

/// Atomically take the command file and parse it; unknown words are ignored. Missing file → empty.
///
/// The file is first renamed to a private name, then read: a word that `tv-ctl.sh` appends while we
/// read lands in a fresh file (picked up on the next SIGUSR2) instead of being deleted unread.
pub fn take_commands(path: &Path) -> Vec<TvCommand> {
    let taken = path.with_extension(format!("taking.{}", std::process::id()));
    if std::fs::rename(path, &taken).is_err() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(&taken).unwrap_or_default();
    let _ = std::fs::remove_file(&taken);
    text.lines().filter_map(TvCommand::parse).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_names_parse_and_cycle() {
        assert_eq!(ViewName::parse(" Load\n"), Some(ViewName::Load));
        assert_eq!(ViewName::parse("boot"), None);
        assert_eq!(ViewName::Main.next(), ViewName::Load);
        assert_eq!(ViewName::Emergency.next(), ViewName::Main);
        assert_eq!(ViewName::Main.prev(), ViewName::Emergency);
    }

    #[test]
    fn remote_words() {
        assert_eq!(TvCommand::parse("sync"), Some(TvCommand::View(ViewName::Sync)));
        assert_eq!(TvCommand::parse("NEXT"), Some(TvCommand::Next));
        assert_eq!(TvCommand::parse("auto"), Some(TvCommand::Auto));
        assert_eq!(TvCommand::parse("fullscreen"), Some(TvCommand::ToggleFullscreen));
        assert_eq!(TvCommand::parse("grain"), Some(TvCommand::ToggleGrain));
        assert_eq!(TvCommand::parse("rm -rf /"), None);
    }

    #[test]
    fn keys() {
        assert_eq!(key_command("2", false), Some(TvCommand::View(ViewName::Load)));
        assert_eq!(key_command("Tab", false), Some(TvCommand::Next));
        assert_eq!(key_command("Tab", true), Some(TvCommand::Prev));
        assert_eq!(key_command(" ", false), Some(TvCommand::TogglePin));
        assert_eq!(key_command("f", false), Some(TvCommand::ToggleFullscreen));
        assert_eq!(key_command("g", false), Some(TvCommand::ToggleGrain));
        assert_eq!(key_command("x", false), None);
    }

    #[test]
    fn view_mode_transitions() {
        let auto = ViewMode::Auto;
        assert_eq!(auto.apply(TvCommand::View(ViewName::Sync), ViewName::Main), ViewMode::Pinned(ViewName::Sync));
        assert_eq!(auto.apply(TvCommand::Next, ViewName::Load), ViewMode::Pinned(ViewName::Sync));
        assert_eq!(auto.apply(TvCommand::TogglePin, ViewName::Load), ViewMode::Pinned(ViewName::Load));
        let pinned = ViewMode::Pinned(ViewName::Load);
        assert_eq!(pinned.apply(TvCommand::TogglePin, ViewName::Load), ViewMode::Auto);
        assert_eq!(pinned.apply(TvCommand::Auto, ViewName::Load), ViewMode::Auto);
        assert_eq!(pinned.apply(TvCommand::Scanlines, ViewName::Load), pinned);
        assert_eq!(pinned.label(), "PINNED · LOAD");
    }

    #[test]
    fn command_file_is_drained() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("nerv-tv.cmd");
        assert!(take_commands(&p).is_empty());
        std::fs::write(&p, "load\nbogus\nauto\n").unwrap();
        assert_eq!(take_commands(&p), vec![TvCommand::View(ViewName::Load), TvCommand::Auto]);
        assert!(!p.exists());
        // A word appended after the take starts lands in a new file, not in the void.
        std::fs::write(&p, "sync\n").unwrap();
        let first = take_commands(&p);
        std::fs::write(&p, "main\n").unwrap(); // tv-ctl appends again
        assert_eq!(first, vec![TvCommand::View(ViewName::Sync)]);
        assert_eq!(take_commands(&p), vec![TvCommand::View(ViewName::Main)]);
        assert!(std::fs::read_dir(d.path()).unwrap().count() == 0, "no leftover temp files");
    }
}
