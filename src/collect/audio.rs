//! Default sink name, volume and mute: `pactl subscribe` pushes change events, and each settled
//! burst triggers one `wpctl get-volume` + `wpctl inspect` of `@DEFAULT_AUDIO_SINK@`.

use std::io::ErrorKind;
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use super::proc::{argv, supervise};
use crate::state::{AudioState, Source, Update};

const DEBOUNCE: Duration = Duration::from_millis(100);
const SINK: &str = "@DEFAULT_AUDIO_SINK@";

/// `Volume: 0.60` or `Volume: 0.60 [MUTED]` → (0.60, muted).
pub fn parse_volume(text: &str) -> Option<(f32, bool)> {
    let rest = text.trim().strip_prefix("Volume:")?;
    let v = rest.split_whitespace().next()?.parse().ok()?;
    Some((v, rest.contains("[MUTED]")))
}

/// Longer descriptions are hardware strings ("Core Ultra 200H/200V Series Processors HD Audio
/// HDMI / DisplayPort 1 Output"); the nick ("HDMI 1") reads better when there is one.
const LONG_DESCRIPTION: usize = 24;

/// `node.description` from `wpctl inspect`, or `node.nick` when the description is missing or
/// longer than 24 characters and a nick exists.
pub fn parse_description(inspect: &str) -> Option<String> {
    let prop = |key: &str| {
        inspect.lines().find_map(|l| {
            let (k, v) = l.trim().trim_start_matches('*').split_once('=')?;
            (k.trim() == key).then(|| v.trim().trim_matches('"').to_string()).filter(|v| !v.is_empty())
        })
    };
    match (prop("node.description"), prop("node.nick")) {
        (Some(d), Some(n)) if d.chars().count() > LONG_DESCRIPTION => Some(n),
        (d, n) => d.or(n),
    }
}

/// pactl lines that can change the default sink's name, volume or mute.
pub fn is_sink_event(line: &str) -> bool {
    line.contains("sink") || line.contains("server")
}

/// One read of the default sink. `Err` when wpctl is not installed.
fn read_audio() -> Result<Option<AudioState>, ()> {
    let run = |args: &[&str]| match Command::new("wpctl").args(args).output() {
        Ok(o) => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Err(e) if e.kind() == ErrorKind::NotFound => Err(()),
        Err(_) => Ok(String::new()),
    };
    let Some((volume, muted)) = parse_volume(&run(&["get-volume", SINK])?) else { return Ok(None) };
    let sink_name = parse_description(&run(&["inspect", SINK])?).unwrap_or_else(|| "UNKNOWN".into());
    Ok(Some(AudioState { sink_name, volume, muted }))
}

pub fn spawn(tx: Sender<Update>) {
    let (poke_tx, poke_rx) = mpsc::channel::<()>();
    let out = tx.clone();
    // Reader: once at startup, then once per settled burst of sink/server events.
    thread::spawn(move || loop {
        match read_audio() {
            Ok(a) => {
                if out.send(Update::Audio(a)).is_err() {
                    return;
                }
            }
            Err(()) => {
                let _ = out.send(Update::SourceOffline(Source::Audio, "wpctl not installed".into()));
                return;
            }
        }
        if poke_rx.recv().is_err() {
            return;
        }
        thread::sleep(DEBOUNCE);
        while poke_rx.try_recv().is_ok() {}
    });
    thread::spawn(move || {
        supervise("pactl", Source::Audio, argv(&["pactl", "subscribe"]), tx, move |line, _| {
            if is_sink_event(line) {
                let _ = poke_tx.send(());
            }
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_and_mute() {
        assert_eq!(parse_volume("Volume: 0.60\n"), Some((0.60, false)));
        assert_eq!(parse_volume("Volume: 1.00 [MUTED]"), Some((1.0, true)));
        assert_eq!(parse_volume("garbage"), None);
    }

    #[test]
    fn description_then_nick() {
        let inspect = "id 91, type PipeWire:Interface:Node\n  * node.description = \"WH-1000XM5\"\n  * node.nick = \"HDMI 1\"\n";
        assert_eq!(parse_description(inspect).as_deref(), Some("WH-1000XM5"));
        assert_eq!(parse_description("  * node.nick = \"HDMI 1\"").as_deref(), Some("HDMI 1"));
        assert_eq!(parse_description(""), None);
        // long descriptions give way to the nick when there is one
        let hdmi = "  * node.description = \"Core Ultra 200H/200V Series Processors HD Audio HDMI / DisplayPort 1 Output\"\n  * node.nick = \"HDMI 1\"\n";
        assert_eq!(parse_description(hdmi).as_deref(), Some("HDMI 1"));
        let long_no_nick = "  * node.description = \"Core Ultra 200H/200V Series Processors HD Audio Speaker\"\n";
        assert_eq!(parse_description(long_no_nick).as_deref(), Some("Core Ultra 200H/200V Series Processors HD Audio Speaker"));
        let exactly_24 = "  * node.description = \"abcdefghijklmnopqrstuvwx\"\n  * node.nick = \"n\"\n";
        assert_eq!(parse_description(exactly_24).as_deref(), Some("abcdefghijklmnopqrstuvwx"));
    }

    #[test]
    fn sink_events_trigger_a_reread() {
        assert!(is_sink_event("Event 'change' on sink #59"));
        assert!(is_sink_event("Event 'change' on server #1"));
        assert!(!is_sink_event("Event 'new' on client #212"));
    }
}
