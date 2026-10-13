//! `nerv tv --record OUT.mp4`: headless video capture. Renders every frame offscreen on a fixed
//! animation clock (`t = i / fps`), reads it back and pipes rgb24 rawvideo to an `ffmpeg` child
//! (libx264). No window, no dropped or duplicated frames: a slow encoder slows the loop down
//! instead (the demo feed runs on wall-clock time, so the data only drifts a little ahead).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context as _};
use femtovg::rgb::RGBA8;
use femtovg::RenderTarget;

use crate::cli::TvArgs;
use crate::config::Config;

use super::View;

/// Animation time of frame `i` at `fps`.
pub fn frame_time(i: u32, fps: u32) -> f32 {
    (i as f64 / fps as f64) as f32
}

/// Frames in a clip of `seconds` at `fps` (at least one).
pub fn frame_count(seconds: f64, fps: u32) -> u32 {
    ((seconds * fps as f64).round() as u32).max(1)
}

/// Which frames of a recording are rendered and which are written.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// Preroll frames: rendered, not written.
    pub skip: u32,
    /// All rendered frames (preroll + clip).
    pub total: u32,
}

impl Plan {
    pub fn new(preroll_s: f64, seconds: f64, fps: u32) -> Self {
        let skip = if preroll_s > 0.0 { frame_count(preroll_s, fps) } else { 0 };
        Self { skip, total: skip + frame_count(seconds, fps) }
    }

    pub fn writes(self, i: u32) -> bool {
        i >= self.skip
    }
}

/// ffmpeg arguments: rgb24 rawvideo of `w`×`h` at `fps` on stdin → H.264 yuv420p at `out`.
pub fn ffmpeg_args(w: u32, h: u32, fps: u32, out: &Path) -> Vec<String> {
    let size = format!("{w}x{h}");
    let fps = fps.to_string();
    let mut args: Vec<String> = [
        "-hide_banner", "-loglevel", "error", "-y",
        "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &size, "-r", &fps, "-i", "-",
        "-c:v", "libx264", "-crf", "14", "-preset", "slow", "-pix_fmt", "yuv420p",
        "-movflags", "+faststart",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(out.to_string_lossy().into_owned());
    args
}

/// Pack RGBA pixels into `rgb` (3 bytes each), dropping alpha.
pub fn rgba_to_rgb(px: &[RGBA8], rgb: &mut Vec<u8>) {
    rgb.clear();
    rgb.extend(px.iter().flat_map(|p| [p.r, p.g, p.b]));
}

pub fn run(args: &TvArgs, config: Config) -> anyhow::Result<()> {
    let out = args.record.as_ref().ok_or_else(|| anyhow!("--record path missing"))?;
    let (w, h, fps) = (args.size.0, args.size.1, args.fps);
    let plan = Plan::new(args.preroll, args.seconds, fps);
    let frames = plan.total - plan.skip;
    let (_gl, canvas, target) = super::snapshot::offscreen(w, h)?;
    let mut view = View::new(canvas, config, args)?;

    let mut ffmpeg = Command::new("ffmpeg")
        .args(ffmpeg_args(w, h, fps, out))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("spawn ffmpeg (is it installed?)")?;
    let mut pipe = ffmpeg.stdin.take().ok_or_else(|| anyhow!("ffmpeg stdin"))?;
    // a writer thread overlaps the GL readback with the encoder; the small bound keeps memory flat
    // and turns a slow encoder into back-pressure on the render loop
    let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(4);
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for buf in rx {
            pipe.write_all(&buf)?;
        }
        Ok(())
    });
    let start = Instant::now();
    let mut max_lag = Duration::ZERO;
    for i in 0..plan.total {
        // never ahead of the wall clock, so the demo feed and the animations stay together;
        // behind it (slow encoder) every frame is still rendered and written
        let due = start + Duration::from_secs_f64(i as f64 / fps as f64);
        match due.checked_duration_since(Instant::now()) {
            Some(wait) => std::thread::sleep(wait),
            None => max_lag = max_lag.max(Instant::now() - due),
        }
        view.frame(w, h, frame_time(i, fps), RenderTarget::Image(target));
        if !plan.writes(i) {
            continue;
        }
        let img = view.canvas.screenshot().map_err(|e| anyhow!("read pixels: {e:?}"))?;
        if img.width() != w as usize || img.height() != h as usize {
            bail!("readback is {}x{}, expected {w}x{h}", img.width(), img.height());
        }
        let mut rgb = Vec::with_capacity(w as usize * h as usize * 3);
        rgba_to_rgb(img.buf(), &mut rgb);
        if tx.send(rgb).is_err() {
            break; // the writer stopped: its error is reported below
        }
    }
    drop(tx);
    let written = writer.join().map_err(|_| anyhow!("ffmpeg writer thread panicked"))?;
    let status = ffmpeg.wait()?;
    if let Err(e) = written {
        bail!("writing frames to ffmpeg ({status}): {e}");
    }
    if !status.success() {
        bail!("ffmpeg failed ({status}) writing {}", out.display());
    }
    eprintln!(
        "nerv: recorded {} ({w}x{h} @ {fps} fps, {frames} frames = {:.2} s from t = {:.2} s) in {:.1} s, max lag {:.0} ms",
        out.display(),
        frames as f64 / fps as f64,
        frame_time(plan.skip, fps),
        start.elapsed().as_secs_f32(),
        max_lag.as_secs_f64() * 1e3
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_time_is_a_fixed_clock() {
        assert_eq!(frame_time(0, 60), 0.0);
        assert_eq!(frame_time(60, 60), 1.0);
        assert_eq!(frame_time(90, 30), 3.0);
        // no drift after a long clip: frame 36000 at 60 fps is exactly 10 min
        assert_eq!(frame_time(36_000, 60), 600.0);
    }

    #[test]
    fn frame_count_rounds_to_whole_frames() {
        assert_eq!(frame_count(5.0, 60), 300);
        assert_eq!(frame_count(7.5, 60), 450);
        assert_eq!(frame_count(1.0 / 3.0, 60), 20);
        assert_eq!(frame_count(0.0, 60), 1);
    }

    #[test]
    fn preroll_frames_are_rendered_but_not_written() {
        // 2 s of preroll then 1 s of clip at 30 fps: frames 0..60 skipped, 60..90 written
        let plan = Plan::new(2.0, 1.0, 30);
        assert_eq!((plan.skip, plan.total), (60, 90));
        assert!(!plan.writes(59));
        assert!(plan.writes(60) && plan.writes(89));
        // the clock keeps running through the preroll: the first written frame is at t = 2 s
        assert_eq!(frame_time(60, 30), 2.0);
        assert_eq!(Plan::new(0.0, 5.0, 60).skip, 0);
    }

    #[test]
    fn rgb24_drops_alpha_and_reuses_the_buffer() {
        use femtovg::rgb::RGBA8;
        let px = [RGBA8 { r: 1, g: 2, b: 3, a: 255 }, RGBA8 { r: 4, g: 5, b: 6, a: 0 }];
        let mut rgb = vec![9; 32];
        rgba_to_rgb(&px, &mut rgb);
        assert_eq!(rgb, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn ffmpeg_reads_rgb24_from_stdin_and_writes_h264() {
        let args = ffmpeg_args(1920, 1080, 60, Path::new("/tmp/out dir/clip.mp4"));
        let joined = args.join(" ");
        assert!(joined.contains("-f rawvideo -pix_fmt rgb24 -s 1920x1080 -r 60 -i -"), "{joined}");
        assert!(joined.contains("-c:v libx264 -crf 14 -preset slow -pix_fmt yuv420p"), "{joined}");
        // overwrite, quiet, and the path is one argument even with a space in it
        assert_eq!(args.first().map(String::as_str), Some("-hide_banner"));
        assert!(args.contains(&"-y".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("/tmp/out dir/clip.mp4"));
    }
}
