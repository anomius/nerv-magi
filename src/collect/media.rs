//! Now playing (MPRIS via playerctl): `playerctl --follow metadata` pushes track/status changes;
//! while playing, `playerctl position` is polled once a second so the renderer can interpolate.

use std::process::Command;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use super::proc::{argv, supervise};
use crate::state::{MediaState, PlayStatus, Source, Update};

const FORMAT: &str = "{{playerName}}\t{{status}}\t{{title}}\t{{artist}}\t{{album}}\t{{position}}\t{{mpris:length}}";

/// One `FORMAT` line (position/length in µs). Empty or short lines mean "no player" → None.
pub fn parse_line(line: &str, now: SystemTime) -> Option<MediaState> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 7 || f[0].is_empty() {
        return None;
    }
    let status = match f[1] {
        "Playing" => PlayStatus::Playing,
        "Paused" => PlayStatus::Paused,
        _ => PlayStatus::Stopped,
    };
    let micros = |s: &str| s.trim().parse::<f64>().ok().map(|us| us / 1e6);
    Some(MediaState {
        player: f[0].to_string(),
        status,
        title: f[2].to_string(),
        artist: f[3].to_string(),
        album: f[4].to_string(),
        position_s: micros(f[5]).unwrap_or(0.0),
        length_s: micros(f[6]).filter(|l| *l > 0.0),
        sampled_at: now,
    })
}

/// `playerctl position` prints seconds, e.g. `72.123456`.
pub fn parse_position(text: &str) -> Option<f64> {
    text.trim().parse().ok()
}

fn position(player: &str) -> Option<f64> {
    let out = Command::new("playerctl").args(["-p", player, "position"]).output().ok()?;
    parse_position(&String::from_utf8_lossy(&out.stdout))
}

pub fn spawn(tx: Sender<Update>) {
    let last: Arc<Mutex<Option<MediaState>>> = Arc::default();
    let (poll_last, poll_tx) = (last.clone(), tx.clone());
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let playing = poll_last.lock().unwrap().clone().filter(|m| m.status == PlayStatus::Playing);
        let Some(m) = playing else { continue };
        let Some(pos) = position(&m.player) else { continue };
        let fresh = MediaState { position_s: pos, sampled_at: SystemTime::now(), ..m.clone() };
        let mut cur = poll_last.lock().unwrap();
        if cur.as_ref() != Some(&m) {
            continue; // a pushed change landed while polling; it wins
        }
        *cur = Some(fresh.clone());
        drop(cur);
        if poll_tx.send(Update::Media(Some(fresh))).is_err() {
            return;
        }
    });
    let cmd = argv(&["playerctl", "--follow", "metadata", "--format", FORMAT]);
    thread::spawn(move || {
        supervise("playerctl", Source::Media, cmd, tx, move |line, tx| {
            let m = parse_line(line, SystemTime::now());
            *last.lock().unwrap() = m.clone();
            let _ = tx.send(Update::Media(m));
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_line() {
        let now = std::time::SystemTime::UNIX_EPOCH;
        let m = parse_line("spotify\tPlaying\tDecisive Battle\tShiro Sagisu\tOST\t72000000\t220000000", now).unwrap();
        assert_eq!(m.status, PlayStatus::Playing);
        assert_eq!(m.title, "Decisive Battle");
        assert_eq!(m.position_s, 72.0);
        assert_eq!(m.length_s, Some(220.0));
    }

    #[test]
    fn empty_or_short_line_is_none() {
        let now = std::time::SystemTime::UNIX_EPOCH;
        assert!(parse_line("", now).is_none());
        assert!(parse_line("spotify\tPlaying", now).is_none());
    }

    #[test]
    fn missing_length_is_none_length() {
        let now = std::time::SystemTime::UNIX_EPOCH;
        let m = parse_line("firefox\tPaused\tSome video\t\t\t1000000\t", now).unwrap();
        assert_eq!(m.length_s, None);
        assert_eq!(m.status, PlayStatus::Paused);
    }

    #[test]
    fn position_output_is_seconds() {
        assert_eq!(parse_position("72.123456\n"), Some(72.123456));
        assert_eq!(parse_position("No players found"), None);
    }
}
