//! Child-process helper for collectors that follow a long-lived tool's stdout.
//!
//! Children run in their own process group (so a tool's helpers die with it) and get
//! `PR_SET_PDEATHSIG`, so they are killed even when nerv exits without unwinding (Ctrl-C).

use std::io::{BufRead, BufReader, ErrorKind};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{Source, Update};

const BACKOFF: [Duration; 4] =
    [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(30)];
/// A child that ran at least this long before exiting restarts the backoff from 1 s.
const HEALTHY_RUN: Duration = Duration::from_secs(30);

/// Kills the child's whole process group and reaps it, however supervise returns.
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: plain syscall; the child is its own group leader (process_group(0)).
        unsafe { libc::kill(-(self.0.id() as i32), libc::SIGTERM) };
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(argv: &[String]) -> std::io::Result<Guard> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).process_group(0);
    // SAFETY: prctl is async-signal-safe; nothing else runs between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
    cmd.spawn().map(Guard)
}

/// Run `argv`, call `on_line` for each stdout line, respawn when it exits (backoff 1 s, 2 s, 5 s,
/// then every 30 s; reported as `SourceOffline` until the next line arrives). A missing binary is
/// reported once and supervise returns. Updates `on_line` sends are forwarded to `tx`; when the
/// receiver is gone the child is killed and supervise returns. Runs on the calling thread.
pub fn supervise(
    name: &'static str,
    source: Source,
    argv: Vec<String>,
    tx: Sender<Update>,
    mut on_line: impl FnMut(&str, &Sender<Update>) + Send + 'static,
) {
    let (line_tx, line_rx) = mpsc::channel();
    let mut attempt = 0;
    let mut offline = false;
    loop {
        let started = Instant::now();
        match start(&argv) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                let _ = tx.send(Update::SourceOffline(source, format!("{name} not installed")));
                return;
            }
            Err(e) => {
                if tx.send(Update::SourceOffline(source, format!("{name}: {e}"))).is_err() {
                    return;
                }
                offline = true;
            }
            Ok(mut child) => {
                let mut out = BufReader::new(child.0.stdout.take().expect("stdout is piped"));
                let mut buf = Vec::new();
                while matches!(out.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                    if offline {
                        if tx.send(Update::SourceOnline(source)).is_err() {
                            return;
                        }
                        offline = false;
                    }
                    let line = String::from_utf8_lossy(&buf);
                    on_line(line.trim_end_matches(['\n', '\r']), &line_tx);
                    buf.clear();
                    for u in line_rx.try_iter() {
                        if tx.send(u).is_err() {
                            return; // `child` drops here: group killed, reaped
                        }
                    }
                }
                drop(child);
                if tx.send(Update::SourceOffline(source, format!("{name} exited, restarting"))).is_err() {
                    return;
                }
                offline = true;
            }
        }
        if started.elapsed() >= HEALTHY_RUN {
            attempt = 0;
        }
        thread::sleep(BACKOFF[attempt.min(BACKOFF.len() - 1)]);
        attempt += 1;
    }
}

/// `argv` from string literals.
pub fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::state::{Source, Update};

    #[test]
    fn missing_binary_reports_offline_once_and_returns() {
        let (tx, rx) = std::sync::mpsc::channel();
        supervise("nope", Source::Media, vec!["/nonexistent/nerv-test-bin".into()], tx, |_, _| {});
        let got: Vec<_> = rx.iter().collect(); // supervise returned, so tx is dropped
        assert_eq!(got.len(), 1);
        assert!(matches!(&got[0], Update::SourceOffline(Source::Media, m) if m.contains("not installed")));
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    #[test]
    fn lines_are_forwarded_then_exit_is_reported() {
        let (tx, rx) = mpsc::channel();
        let h = thread::spawn(move || {
            supervise("sh", Source::Spectrum, sh("echo 1; echo 2"), tx, |l, tx| {
                let _ = tx.send(Update::Spectrum(vec![l.parse().unwrap()]));
            })
        });
        let next = || rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(next(), Update::Spectrum(v) if v == [1.0]));
        assert!(matches!(next(), Update::Spectrum(v) if v == [2.0]));
        assert!(matches!(next(), Update::SourceOffline(Source::Spectrum, m) if m.contains("exited")));
        // after the 1 s backoff it respawns; the first line back marks the source online again
        assert!(matches!(next(), Update::SourceOnline(Source::Spectrum)));
        drop(rx);
        h.join().unwrap();
    }

    #[test]
    fn receiver_gone_kills_the_child() {
        let (tx, rx) = mpsc::channel();
        let h = thread::spawn(move || {
            supervise("sh", Source::Audio, sh("echo $$; while :; do echo 0; sleep 0.05; done"), tx, |l, tx| {
                let _ = tx.send(Update::Spectrum(vec![l.parse().unwrap()]));
            })
        });
        let Update::Spectrum(v) = rx.recv_timeout(Duration::from_secs(5)).unwrap() else {
            panic!("expected the pid line")
        };
        let pid = v[0] as i32;
        assert!(std::path::Path::new(&format!("/proc/{pid}")).exists());
        drop(rx);
        h.join().unwrap(); // returns on the next forwarded line
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists(), "child {pid} still alive");
    }
}
