use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "nerv", version, about = "NERV · MAGI: herdr agents, telemetry, power and sound")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Fullscreen Eva screen for the TV (GPU window).
    Tv(TvArgs),
    /// Compact terminal view for a herdr pane.
    Hub(HubArgs),
    /// Print the live State as JSON.
    Dump(DumpArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TvArgs {
    /// Synthetic animated data instead of live collectors.
    #[arg(long)]
    pub demo: bool,
    /// Render headless to this PNG and exit.
    #[arg(long)]
    pub snapshot: Option<PathBuf>,
    /// Snapshot size, e.g. 3840x2160.
    #[arg(long, default_value = "3840x2160", value_parser = parse_size)]
    pub size: (u32, u32),
    /// Frames to simulate before a snapshot (lets data arrive and animations settle).
    #[arg(long, default_value_t = 90)]
    pub frames: u32,
    /// Windowed instead of fullscreen (development).
    #[arg(long)]
    pub windowed: bool,
    /// Override the configured scanline look (off | subtle | heavy | crt).
    #[arg(long, value_enum)]
    pub scanlines: Option<crate::config::ScanlineMode>,
    /// Force one view (snapshots and windowed debugging): main | load | sync | emergency | boot.
    #[arg(long, value_parser = ["main", "load", "sync", "emergency", "boot"])]
    pub scene: Option<String>,
    /// Apply control words at startup, as `tv-ctl.sh` takes them (e.g. `--cmd help --cmd grain`).
    #[arg(long = "cmd", value_name = "WORD")]
    pub cmds: Vec<String>,
    /// Headless perf run for this many seconds at --size with the window's frame pacing; prints
    /// CPU % and RSS.
    #[arg(long, value_name = "SECS")]
    pub bench: Option<u32>,
    /// Record headless to this video (H.264 via an ffmpeg child) and exit; uses --size, --fps
    /// and --seconds.
    #[arg(long, value_name = "OUT.mp4")]
    pub record: Option<PathBuf>,
    /// Frame rate of --record (the animation clock steps exactly 1/fps per frame).
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..=240))]
    pub fps: u32,
    /// Length of --record in seconds.
    #[arg(long, default_value_t = 10.0)]
    pub seconds: f64,
    /// Seconds --record renders before its first written frame (data arrives, graphs fill, the
    /// demo reaches a scripted moment); the clock runs on through it.
    #[arg(long, value_name = "SECS", default_value_t = 0.0)]
    pub preroll: f64,
}

#[derive(Debug, Clone, Args)]
pub struct HubArgs {
    #[arg(long)]
    pub demo: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DumpArgs {
    #[arg(long)]
    pub demo: bool,
    /// Print one snapshot (after ~1.5 s of collection) and exit.
    #[arg(long)]
    pub once: bool,
}

pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let w = w.parse().map_err(|_| "bad width")?;
    let h = h.parse().map_err(|_| "bad height")?;
    if w < 320 || h < 200 {
        return Err("size too small (min 320x200)".into());
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_cmd_words_repeat() {
        let cli = Cli::try_parse_from(["nerv", "tv", "--cmd", "help", "--cmd", "grain"]).unwrap();
        let Command::Tv(a) = cli.command else { panic!("not tv") };
        assert_eq!(a.cmds, ["help", "grain"]);
    }

    #[test]
    fn record_takes_path_fps_and_seconds() {
        let cli = Cli::try_parse_from(["nerv", "tv", "--demo", "--record", "clip.mp4", "--seconds", "7.5", "--size", "1920x1080"]).unwrap();
        let Command::Tv(a) = cli.command else { panic!("not tv") };
        assert_eq!(a.record.as_deref(), Some(std::path::Path::new("clip.mp4")));
        assert_eq!(a.fps, 60);
        assert_eq!(a.seconds, 7.5);
        let cli = Cli::try_parse_from(["nerv", "tv", "--record", "c.mp4", "--fps", "30"]).unwrap();
        let Command::Tv(a) = cli.command else { panic!("not tv") };
        assert_eq!(a.fps, 30);
        assert!(Cli::try_parse_from(["nerv", "tv", "--record", "c.mp4", "--fps", "0"]).is_err());
    }

    #[test]
    fn record_preroll_defaults_to_none() {
        let cli = Cli::try_parse_from(["nerv", "tv", "--record", "c.mp4"]).unwrap();
        let Command::Tv(a) = cli.command else { panic!("not tv") };
        assert_eq!(a.preroll, 0.0);
        let cli = Cli::try_parse_from(["nerv", "tv", "--record", "c.mp4", "--preroll", "36"]).unwrap();
        let Command::Tv(a) = cli.command else { panic!("not tv") };
        assert_eq!(a.preroll, 36.0);
    }

    #[test]
    fn size_parses_and_validates() {
        assert_eq!(parse_size("3840x2160"), Ok((3840, 2160)));
        assert!(parse_size("3840").is_err());
        assert!(parse_size("10x10").is_err());
    }
}
