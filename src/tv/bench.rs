//! `nerv tv --bench SECS`: headless perf run with the window's frame pacing (not the snapshot's
//! fixed 60 fps), so CPU and RSS can be measured at any size without opening a window. Reports
//! process CPU (all threads, % of one core) over the steady part (after BENCH_WARMUP_S) and RSS.

use std::time::{Duration, Instant};

use femtovg::RenderTarget;

use crate::cli::TvArgs;
use crate::config::Config;

use super::View;

/// Seconds excluded from the steady numbers (BOOT renders at full frame rate).
pub const BENCH_WARMUP_S: f32 = 6.0;

/// CPU seconds of the calling thread.
pub fn thread_cpu() -> f64 {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

/// Accumulated render-thread CPU per frame phase.
#[derive(Debug, Default)]
pub struct Prof {
    phases: Vec<(&'static str, f64, u32)>,
}

impl Prof {
    pub fn add(&mut self, phase: &'static str, since_cpu: f64) {
        let d = thread_cpu() - since_cpu;
        match self.phases.iter_mut().find(|p| p.0 == phase) {
            Some(p) => {
                p.1 += d;
                p.2 += 1;
            }
            None => self.phases.push((phase, d, 1)),
        }
    }
}

thread_local! {
    static SPANS: std::cell::RefCell<Option<Prof>> = const { std::cell::RefCell::new(None) };
}

/// Run `f`, adding its render-thread CPU to `name` while a bench is profiling.
pub fn span<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    if !SPANS.with(|s| s.borrow().is_some()) {
        return f();
    }
    let c0 = thread_cpu();
    let r = f();
    SPANS.with(|s| {
        if let Some(p) = s.borrow_mut().as_mut() {
            p.add(name, c0);
        }
    });
    r
}

/// (comm, cpu seconds) of every thread of this process.
fn threads() -> Vec<(String, f64)> {
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else { return Vec::new() };
    dir.flatten()
        .filter_map(|e| {
            let stat = std::fs::read_to_string(e.path().join("stat")).ok()?;
            let (l, r) = (stat.find('(')?, stat.rfind(')')?);
            let f: Vec<&str> = stat[r + 2..].split_whitespace().collect();
            let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
            Some((format!("{}:{}", stat[l + 1..r].to_string(), e.file_name().to_string_lossy()), ticks / hz))
        })
        .collect()
}

/// A `kB` field of /proc/self/status.
pub fn status_kb(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix(':')?.trim().strip_suffix("kB")?.trim().parse().ok())
}

/// CPU seconds over wall seconds, as % of one core.
pub fn cpu_pct(cpu_s: f64, wall_s: f64) -> f64 {
    if wall_s <= 0.0 {
        0.0
    } else {
        cpu_s / wall_s * 100.0
    }
}

/// User + system CPU seconds of the whole process (all threads, collectors included).
fn cpu_seconds() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

fn rss_kb() -> (u64, u64) {
    let text = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    (status_kb(&text, "VmRSS").unwrap_or(0), status_kb(&text, "VmHWM").unwrap_or(0))
}

pub fn run(args: &TvArgs, config: Config, secs: u32) -> anyhow::Result<()> {
    let (w, h) = args.size;
    let (_gl, canvas, target) = super::snapshot::offscreen(w, h)?;
    let mut view = View::new(canvas, config, args)?;
    let start = Instant::now();
    let end = Duration::from_secs(secs.max(1) as u64);
    let warm = Duration::from_secs_f32(BENCH_WARMUP_S.min(secs as f32 / 2.0));
    let (mut frames, mut steady_frames) = (0u32, 0u32);
    let mut steady: Option<(Instant, f64)> = None;
    let mut rss_max = 0u64;
    let mut threads0 = Vec::new();
    let mut worst: Vec<(f64, f32, &'static str)> = Vec::new();
    loop {
        let now = start.elapsed();
        if now >= end {
            break;
        }
        if steady.is_none() && now >= warm {
            steady = Some((Instant::now(), cpu_seconds()));
            threads0 = threads();
            view.prof = Some(Prof::default());
            SPANS.with(|s| *s.borrow_mut() = Some(Prof::default()));
        }
        let c0 = thread_cpu();
        view.frame(w, h, now.as_secs_f32(), RenderTarget::Image(target));
        worst.push((thread_cpu() - c0, now.as_secs_f32(), view.scene_name()));
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        worst.truncate(5);
        frames += 1;
        if steady.is_some() {
            steady_frames += 1;
            rss_max = rss_max.max(rss_kb().0);
        }
        // same policy as the window: the next frame is due one frame interval after this one
        std::thread::sleep(view.frame_interval());
    }
    let (t0, c0) = steady.unwrap_or((start, 0.0));
    let wall = t0.elapsed().as_secs_f64();
    let cpu = cpu_pct(cpu_seconds() - c0, wall);
    let (rss, hwm) = rss_kb();
    println!(
        "nerv bench {w}x{h} {secs}s: {frames} frames, steady {:.1} fps, cpu {cpu:.1}% of one core, rss {} MB (steady max {} MB, peak {} MB)",
        steady_frames as f64 / wall.max(1e-9),
        rss / 1024,
        rss_max / 1024,
        hwm / 1024
    );
    let mut per: Vec<(String, f64)> = threads()
        .into_iter()
        .map(|(n, s)| {
            let s0 = threads0.iter().find(|(m, _)| *m == n).map_or(0.0, |(_, v)| *v);
            (n, cpu_pct(s - s0, wall))
        })
        .filter(|(_, p)| *p >= 0.05)
        .collect();
    per.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("  threads: {}", per.iter().map(|(n, p)| format!("{n} {p:.2}%")).collect::<Vec<_>>().join(" · "));
    println!(
        "  worst frames: {}",
        worst.iter().map(|(c, t, s)| format!("{:.1} ms @{t:.2}s {s}", c * 1e3)).collect::<Vec<_>>().join(" · ")
    );
    if let Some(p) = &view.prof {
        let s: Vec<String> = p
            .phases
            .iter()
            .map(|(n, c, k)| format!("{n} {:.2}% ({k}×, {:.2} ms)", cpu_pct(*c, wall), c / *k as f64 * 1e3))
            .collect();
        println!("  render: {}", s.join(" · "));
    }
    if let Some(mut p) = SPANS.with(|s| s.borrow_mut().take()) {
        p.phases.sort_by(|a, b| b.1.total_cmp(&a.1));
        let s: Vec<String> = p
            .phases
            .iter()
            .map(|(n, c, k)| format!("{n} {:.2}% ({:.3} ms)", cpu_pct(*c, wall), c / *k as f64 * 1e3))
            .collect();
        println!("  spans: {}", s.join(" · "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kb_fields_from_proc_status() {
        let text = "Name:\tnerv\nVmHWM:\t  103424 kB\nVmRSS:\t   95232 kB\nRssAnon:\t   40000 kB\n";
        assert_eq!(status_kb(text, "VmRSS"), Some(95_232));
        assert_eq!(status_kb(text, "VmHWM"), Some(103_424));
        assert_eq!(status_kb(text, "VmSwap"), None);
    }

    #[test]
    fn cpu_percent_of_one_core() {
        assert!((cpu_pct(3.0, 60.0) - 5.0).abs() < 1e-6);
        assert_eq!(cpu_pct(1.0, 0.0), 0.0);
    }
}
