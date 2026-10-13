//! Live audio spectrum from cava's raw ascii output (TV only).

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use super::proc::supervise;
use crate::state::{Source, Update};

/// cava config: `bars` mono bars at `framerate` fps from PipeWire, raw ascii `v1;v2;…;\n` on stdout.
pub fn config_text(bars: usize, framerate: u32) -> String {
    format!(
        "[general]\nbars = {bars}\nframerate = {framerate}\n\
         [input]\nmethod = pipewire\nsource = auto\n\
         [output]\nmethod = raw\nraw_target = /dev/stdout\ndata_format = ascii\nascii_max_range = 1000\n\
         bar_delimiter = 59\nframe_delimiter = 10\nchannels = mono\n"
    )
}

/// `0;500;1000;` → [0.0, 0.5, 1.0] (values / max, clamped; garbage → 0).
pub fn parse_frame(line: &str, max: f32) -> Vec<f32> {
    line.split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|s| (s.trim().parse::<f32>().unwrap_or(0.0) / max).clamp(0.0, 1.0))
        .collect()
}

fn config_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("nerv-cava.conf"),
        // SAFETY: getuid has no failure mode.
        None => std::env::temp_dir().join(format!("nerv-cava-{}.conf", unsafe { libc::getuid() })),
    }
}

pub fn spawn(tx: Sender<Update>, bars: usize, framerate: u32) {
    let path = config_path();
    if let Err(e) = std::fs::write(&path, config_text(bars, framerate)) {
        let _ = tx.send(Update::SourceOffline(Source::Spectrum, format!("cava config {}: {e}", path.display())));
        return;
    }
    let cmd = vec!["cava".to_string(), "-p".to_string(), path.display().to_string()];
    thread::spawn(move || {
        supervise("cava", Source::Spectrum, cmd, tx, |line, tx| {
            let _ = tx.send(Update::Spectrum(parse_frame(line, 1000.0)));
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_scales_and_ignores_trailing_delimiter() {
        assert_eq!(parse_frame("0;500;1000;", 1000.0), vec![0.0, 0.5, 1.0]);
        assert_eq!(parse_frame("", 1000.0), Vec::<f32>::new());
        assert_eq!(parse_frame("x;2000;", 1000.0), vec![0.0, 1.0]); // garbage → 0, clamp
    }

    #[test]
    fn cava_config_is_exact() {
        assert!(config_text(48, 30).starts_with("[general]\nbars = 48\nframerate = 30\n"));
        assert!(config_text(48, 30).contains("raw_target = /dev/stdout\n"));
        let exact = "[general]\nbars = 48\nframerate = 30\n[input]\nmethod = pipewire\nsource = auto\n[output]\nmethod = raw\nraw_target = /dev/stdout\ndata_format = ascii\nascii_max_range = 1000\nbar_delimiter = 59\nframe_delimiter = 10\nchannels = mono\n";
        assert_eq!(config_text(48, 30), exact);
    }
}
