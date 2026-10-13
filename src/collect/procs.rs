//! btop-style process table, sampled every 2 s from /proc.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{ProcRow, Update};

const PERIOD: Duration = Duration::from_secs(2);
/// Rows sent per sample (the views show 20 or fewer).
const TOP: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub struct RawProc {
    pub pid: i32,
    pub name: String,
    pub state: char,
    /// utime + stime, in clock ticks.
    pub ticks: u64,
    pub threads: u32,
    pub rss_pages: u64,
}

/// One `/proc/<pid>/stat` line. comm sits between the first `(` and the **last** `)`; after it,
/// counting from 0: state 0, utime 11, stime 12, num_threads 17, rss 21 (proc(5) fields 3, 14,
/// 15, 20, 24).
pub fn parse_stat(text: &str) -> Option<RawProc> {
    let (open, close) = (text.find('(')?, text.rfind(')')?);
    if close < open {
        return None;
    }
    let f: Vec<&str> = text[close + 1..].split_whitespace().collect();
    if f.len() < 22 {
        return None;
    }
    let num = |i: usize| f[i].parse::<u64>().ok();
    Some(RawProc {
        pid: text[..open].trim().parse().ok()?,
        name: text[open + 1..close].to_string(),
        state: f[0].chars().next()?,
        ticks: num(11)? + num(12)?,
        threads: num(17)? as u32,
        rss_pages: num(21)?,
    })
}

/// Thread names some runtimes give their main thread (Node 22+ → `MainThread`), which then
/// shows up as the process comm.
const GENERIC_COMM: [&str; 1] = ["MainThread"];

/// comm, or `basename(argv[0])` when comm is a generic thread name.
pub fn display_name(comm: &str, cmdline: &[u8]) -> String {
    if GENERIC_COMM.contains(&comm) {
        let argv0 = cmdline.split(|b| *b == 0).next().unwrap_or_default();
        let base = argv0.rsplit(|b| *b == b'/').next().unwrap_or_default();
        if !base.is_empty() {
            return String::from_utf8_lossy(base).into_owned();
        }
    }
    comm.to_string()
}

/// Percent of one core: Δticks / (Δwall_s × ticks per second) × 100.
pub fn cpu_pct(prev_ticks: u64, cur_ticks: u64, dt_s: f64, hz: f64) -> f32 {
    if dt_s <= 0.0 || hz <= 0.0 {
        return 0.0;
    }
    (cur_ticks.saturating_sub(prev_ticks) as f64 / (dt_s * hz) * 100.0) as f32
}

pub struct Sampler {
    root: PathBuf,
    hz: f64,
    page: u64,
    prev: HashMap<i32, u64>,
    prev_at: Option<Instant>,
}

impl Sampler {
    pub fn new() -> Self {
        // SAFETY: sysconf has no preconditions.
        let (hz, page) = unsafe { (libc::sysconf(libc::_SC_CLK_TCK), libc::sysconf(libc::_SC_PAGESIZE)) };
        Self::with(
            Path::new("/proc"),
            if hz > 0 { hz as f64 } else { 100.0 },
            if page > 0 { page as u64 } else { 4096 },
        )
    }

    pub fn with(root: &Path, hz: f64, page: u64) -> Self {
        Self { root: root.to_path_buf(), hz, page, prev: HashMap::new(), prev_at: None }
    }

    /// Top processes by cpu (tie: RSS). Pids that vanish mid-scan are skipped; a pid seen for the
    /// first time reports 0 % until the next sample.
    pub fn sample_at(&mut self, now: Instant) -> Vec<ProcRow> {
        let dt = self.prev_at.map(|p| now.duration_since(p).as_secs_f64());
        let mut seen = HashMap::new();
        let mut rows: Vec<ProcRow> = fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())))
            .filter_map(|e| parse_stat(&fs::read_to_string(e.path().join("stat")).ok()?))
            .map(|p| {
                let cpu = match (self.prev.get(&p.pid), dt) {
                    (Some(&prev), Some(dt)) => cpu_pct(prev, p.ticks, dt, self.hz),
                    _ => 0.0,
                };
                seen.insert(p.pid, p.ticks);
                let name = if GENERIC_COMM.contains(&p.name.as_str()) {
                    display_name(
                        &p.name,
                        &fs::read(self.root.join(p.pid.to_string()).join("cmdline")).unwrap_or_default(),
                    )
                } else {
                    p.name
                };
                ProcRow {
                    pid: p.pid,
                    name,
                    cpu,
                    rss_bytes: p.rss_pages * self.page,
                    threads: p.threads,
                    state: p.state,
                }
            })
            .collect();
        self.prev = seen;
        self.prev_at = Some(now);
        rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.rss_bytes.cmp(&a.rss_bytes)).then(a.pid.cmp(&b.pid)));
        rows.truncate(TOP);
        rows
    }
}

pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || {
        let mut sampler = Sampler::new();
        // The first sample has no deltas (all 0 %); take the second one early.
        let mut period = Duration::from_millis(500);
        loop {
            let started = Instant::now();
            if tx.send(Update::Procs(sampler.sample_at(started))).is_err() {
                return;
            }
            thread::sleep(period.saturating_sub(started.elapsed()));
            period = PERIOD;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn parse_stat_handles_parens_and_spaces_in_comm() {
        let line = "1234 (tmux: server (x)) S 1 1234 1234 0 -1 4194560 100 0 0 0 250 50 0 0 20 0 3 0 100 1000000 2048 18446744073709551615";
        let p = parse_stat(line).unwrap();
        assert_eq!(p.pid, 1234);
        assert_eq!(p.name, "tmux: server (x)");
        assert_eq!(p.state, 'S');
        assert_eq!(p.ticks, 300);
        assert_eq!(p.threads, 3);
        assert_eq!(p.rss_pages, 2048);
        assert_eq!(parse_stat("garbage"), None);
        assert_eq!(parse_stat("12 (short) S 1 2"), None);
    }

    #[test]
    fn parse_stat_reads_this_process() {
        let p = parse_stat(&std::fs::read_to_string("/proc/self/stat").unwrap()).unwrap();
        assert_eq!(p.pid as u32, std::process::id());
        // The main thread's state (R or S under the test harness); a token offset error would
        // put a number here instead of a state letter.
        assert!("RSDIT".contains(p.state), "{:?}", p.state);
        assert!(p.threads >= 1);
        assert!(p.rss_pages > 0);
    }

    #[test]
    fn cpu_pct_is_percent_of_one_core() {
        assert_eq!(cpu_pct(100, 300, 2.0, 100.0), 100.0);
        assert_eq!(cpu_pct(100, 900, 2.0, 100.0), 400.0);
        assert_eq!(cpu_pct(300, 100, 2.0, 100.0), 0.0);
        assert_eq!(cpu_pct(0, 100, 0.0, 100.0), 0.0);
    }

    fn put_stat(root: &std::path::Path, pid: i32, name: &str, ticks: u64, rss_pages: u64) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let line = format!("{pid} ({name}) S 1 {pid} {pid} 0 -1 0 0 0 0 0 {ticks} 0 0 0 20 0 2 0 1 1000 {rss_pages}");
        std::fs::write(dir.join("stat"), line).unwrap();
    }

    #[test]
    fn sampler_ranks_by_cpu_and_skips_vanished_pids() {
        let d = tempfile::tempdir().unwrap();
        put_stat(d.path(), 1, "idle", 0, 10);
        put_stat(d.path(), 2, "gone-soon", 0, 500);
        std::fs::create_dir_all(d.path().join("3")).unwrap(); // listed, but stat vanished
        std::fs::create_dir_all(d.path().join("self")).unwrap(); // not a pid
        let mut s = Sampler::with(d.path(), 100.0, 4096);
        let t0 = Instant::now();
        let first = s.sample_at(t0);
        assert_eq!(first.iter().map(|r| r.pid).collect::<Vec<_>>(), [2, 1]); // all 0 % → by RSS
        assert_eq!(first[0].rss_bytes, 500 * 4096);
        assert_eq!(first[0].threads, 2);

        put_stat(d.path(), 1, "idle", 200, 10);
        put_stat(d.path(), 4, "new", 50, 1);
        std::fs::remove_dir_all(d.path().join("2")).unwrap();
        let second = s.sample_at(t0 + Duration::from_secs(2));
        let pids: Vec<i32> = second.iter().map(|r| r.pid).collect();
        assert_eq!(pids, [1, 4]);
        assert_eq!(second[0].cpu, 100.0);
        assert_eq!(second[1].cpu, 0.0); // first sight: no delta yet
    }

    #[test]
    fn generic_thread_comm_falls_back_to_argv0() {
        assert_eq!(display_name("MainThread", b"/usr/bin/node\0/x/server.js\0"), "node");
        assert_eq!(display_name("MainThread", b""), "MainThread");
        assert_eq!(display_name("claude", b"node\0/x/cli.js\0"), "claude");
    }

    #[test]
    fn sampler_names_node_main_threads_by_argv0() {
        let d = tempfile::tempdir().unwrap();
        put_stat(d.path(), 7, "MainThread", 0, 1);
        std::fs::write(d.path().join("7/cmdline"), b"/usr/bin/node\0/srv/app.js\0").unwrap();
        let rows = Sampler::with(d.path(), 100.0, 4096).sample_at(Instant::now());
        assert_eq!(rows[0].name, "node");
    }

    #[test]
    fn sampler_keeps_top_40() {
        let d = tempfile::tempdir().unwrap();
        for pid in 1..=45 {
            put_stat(d.path(), pid, "p", 0, pid as u64);
        }
        let rows = Sampler::with(d.path(), 100.0, 4096).sample_at(Instant::now());
        assert_eq!(rows.len(), 40);
        assert_eq!(rows[0].pid, 45);
    }
}
