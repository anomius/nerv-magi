//! 1 Hz system sampler (`Sys` + `SysExtra`): /proc and /sys only, no child processes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use crate::state::{DiskState, GpuState, Iface, Mount, NetState, SysExtra, SysState, Update};

/// Mount sizes change slowly; statvfs them every 10 s.
const MOUNTS_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

/// First entry is the aggregate `cpu` line, then one per `cpuN`.
pub fn parse_cpu(text: &str) -> Vec<CpuTimes> {
    text.lines()
        .filter(|l| l.starts_with("cpu"))
        .filter_map(|l| {
            let v: Vec<u64> = l.split_whitespace().skip(1).map(|x| x.parse().ok()).collect::<Option<_>>()?;
            if v.len() < 4 {
                return None;
            }
            let idle = v[3] + v.get(4).copied().unwrap_or(0); // idle + iowait
            let total: u64 = v.iter().take(8).sum(); // exclude guest columns (already in user)
            Some(CpuTimes { busy: total - idle, total })
        })
        .collect()
}

fn frac(prev: CpuTimes, cur: CpuTimes) -> f32 {
    let dt = cur.total.saturating_sub(prev.total);
    if dt == 0 {
        return 0.0;
    }
    (cur.busy.saturating_sub(prev.busy) as f32 / dt as f32).clamp(0.0, 1.0)
}

pub fn cpu_usage(prev: &[CpuTimes], cur: &[CpuTimes]) -> (f32, Vec<f32>) {
    if prev.is_empty() || cur.is_empty() {
        return (0.0, Vec::new());
    }
    let total = frac(prev[0], cur[0]);
    let cores = prev.iter().zip(cur).skip(1).map(|(p, c)| frac(*p, *c)).collect();
    (total, cores)
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MemInfo {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    /// Buffers + Cached + SReclaimable.
    pub cached: u64,
}

pub fn parse_meminfo(text: &str) -> MemInfo {
    let mut m = MemInfo::default();
    for l in text.lines() {
        let mut it = l.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
        let Ok(kib) = v.parse::<u64>() else { continue };
        let bytes = kib * 1024;
        match k {
            "MemTotal:" => m.total = bytes,
            "MemAvailable:" => m.available = bytes,
            "SwapTotal:" => m.swap_total = bytes,
            "SwapFree:" => m.swap_free = bytes,
            "Buffers:" | "Cached:" | "SReclaimable:" => m.cached += bytes,
            _ => {}
        }
    }
    m
}

/// (name, rx_bytes, tx_bytes) per interface except `lo`, in /proc/net/dev order.
pub fn parse_ifaces(text: &str) -> Vec<(String, u64, u64)> {
    text.lines()
        .skip(2)
        .filter_map(|l| {
            let (name, rest) = l.split_once(':')?;
            let name = name.trim();
            let f: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (name != "lo" && f.len() >= 9).then(|| (name.to_string(), f[0], f[8]))
        })
        .collect()
}

/// (rx_bytes, tx_bytes) summed over all interfaces except `lo`. The sampler sums its own
/// `parse_ifaces` result instead of parsing twice.
#[allow(dead_code)]
pub fn parse_net_dev(text: &str) -> (u64, u64) {
    parse_ifaces(text).iter().fold((0, 0), |(rx, tx), (_, r, t)| (rx + r, tx + t))
}

/// `operstate` is `up`, or `unknown` (tun/wireguard never report a carrier) with IFF_UP and
/// IFF_RUNNING set in `flags`. `net_root` is normally `/sys/class/net`.
pub fn iface_up(net_root: &Path, name: &str) -> bool {
    let dir = net_root.join(name);
    match fs::read_to_string(dir.join("operstate")).as_deref().map(str::trim) {
        Ok("up") => true,
        Ok("unknown") => fs::read_to_string(dir.join("flags"))
            .ok()
            .and_then(|f| u32::from_str_radix(f.trim().trim_start_matches("0x"), 16).ok())
            .is_some_and(|f| f & 0x41 == 0x41),
        _ => false,
    }
}

const REAL_FS: [&str; 8] = ["ext4", "btrfs", "xfs", "vfat", "f2fs", "zfs", "ntfs3", "exfat"];

/// `/proc/self/mounts` escapes space, tab, newline and backslash as `\ooo` octal.
fn unescape_mount(path: &str) -> String {
    let b = path.as_bytes();
    let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
    while i < b.len() {
        let oct = b.get(i + 1..i + 4).filter(|o| b[i] == b'\\' && o.iter().all(|c| (b'0'..=b'7').contains(c)));
        match oct {
            Some(o) => {
                out.push((o[0] - b'0') * 64 + (o[1] - b'0') * 8 + (o[2] - b'0'));
                i += 4;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// (device, mount path, fstype) for real filesystems only, in mount order.
pub fn parse_mounts(text: &str) -> Vec<(String, String, String)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let (dev, path, fstype) = (f.next()?, f.next()?, f.next()?);
            REAL_FS.contains(&fstype).then(|| (dev.to_string(), unescape_mount(path), fstype.to_string()))
        })
        .collect()
}

/// One entry per device, the shortest mount path winning (btrfs subvolumes share a device).
pub fn dedup_mounts(mounts: Vec<(String, String, String)>) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for m in mounts {
        match out.iter_mut().find(|o| o.0 == m.0) {
            Some(o) if m.1.len() < o.1.len() => *o = m,
            Some(_) => {}
            None => out.push(m),
        }
    }
    out
}

/// `scaling_cur_freq` (kHz) of each `cpuN` under `cpu_root` (normally `/sys/devices/system/cpu`),
/// in MHz, ordered by N; 0 where a core has no cpufreq.
pub fn cpu_freqs(cpu_root: &Path) -> Vec<u32> {
    let mut cores: Vec<(u32, u32)> = fs::read_dir(cpu_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_str()?.strip_prefix("cpu")?.parse::<u32>().ok()?;
            Some((n, read_u64(&e.path().join("cpufreq/scaling_cur_freq")).map_or(0, |khz| (khz / 1000) as u32)))
        })
        .collect();
    cores.sort();
    cores.into_iter().map(|(_, mhz)| mhz).collect()
}

/// First `model name` in /proc/cpuinfo.
pub fn parse_cpu_model(cpuinfo: &str) -> String {
    cpuinfo
        .lines()
        .find_map(|l| l.strip_prefix("model name")?.split_once(':').map(|(_, v)| v.trim().to_string()))
        .unwrap_or_default()
}

/// Number of processes (numeric entries) under a proc root.
pub fn count_processes(proc_root: &Path) -> u32 {
    fs::read_dir(proc_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())))
        .count() as u32
}

/// (sectors_read, sectors_written) for one block device.
pub fn parse_diskstats(text: &str, dev: &str) -> Option<(u64, u64)> {
    text.lines().find_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() > 9 && f[2] == dev {
            Some((f[5].parse().ok()?, f[9].parse().ok()?))
        } else {
            None
        }
    })
}

/// First whole disk in diskstats: `nvme<N>n<M>` or `sd<letter>` (no partition suffix).
pub fn root_disk(diskstats: &str) -> Option<String> {
    let whole = |name: &str| {
        if let Some(rest) = name.strip_prefix("nvme") {
            let mut parts = rest.splitn(2, 'n');
            let (ctrl, ns) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            digits(ctrl) && digits(ns)
        } else {
            name.len() == 3 && name.starts_with("sd") && name.as_bytes()[2].is_ascii_lowercase()
        }
    };
    diskstats.lines().filter_map(|l| l.split_whitespace().nth(2)).find(|n| whole(n)).map(str::to_string)
}

/// `/proc/loadavg` → (1, 5, 15 minute averages, running, total). The `running/total` pair counts
/// kernel scheduling entities, i.e. threads.
pub fn parse_loadavg(text: &str) -> (f32, f32, f32, u32, u32) {
    let f: Vec<&str> = text.split_whitespace().collect();
    let avg = |i: usize| f.get(i).and_then(|x| x.parse::<f32>().ok());
    let (Some(a), Some(b), Some(c)) = (avg(0), avg(1), avg(2)) else { return (0.0, 0.0, 0.0, 0, 0) };
    let (run, total) = f
        .get(3)
        .and_then(|x| x.split_once('/'))
        .and_then(|(r, t)| Some((r.parse().ok()?, t.parse().ok()?)))
        .unwrap_or((0, 0));
    (a, b, c, run, total)
}

/// `/proc/uptime` → whole seconds since boot.
pub fn parse_uptime(text: &str) -> u64 {
    text.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).map_or(0, |s| s as u64)
}

/// In `hwmon_root` (normally `/sys/class/hwmon`), the `<prefix>N_input` of the chip whose `name` is
/// `chip`: the one whose `<prefix>N_label` equals `label`, else the lowest-numbered input.
pub fn hwmon_file(hwmon_root: &Path, chip: &str, prefix: &str, label: Option<&str>) -> Option<PathBuf> {
    let dir = fs::read_dir(hwmon_root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|d| fs::read_to_string(d.join("name")).is_ok_and(|n| n.trim() == chip))?;
    let mut inputs: Vec<(u32, PathBuf)> = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix(prefix)?.strip_suffix("_input")?.parse().ok()?;
            Some((n, e.path()))
        })
        .collect();
    inputs.sort();
    if let Some(want) = label {
        let labelled = inputs
            .iter()
            .find(|(n, _)| fs::read_to_string(dir.join(format!("{prefix}{n}_label"))).is_ok_and(|l| l.trim() == want));
        if let Some((_, p)) = labelled {
            return Some(p.clone());
        }
    }
    inputs.into_iter().next().map(|(_, p)| p)
}

/// Above this a fan reading is a driver quirk, not a laptop fan.
const FAN_MAX_RPM: u64 = 9000;

/// Fan inputs in preference order: `acpi_fan`, then `asus` as a fallback (on this machine
/// asus/fan1_input reads 12000+ while acpi_fan reads a plausible ~2800).
pub fn fan_inputs(hwmon_root: &Path) -> Vec<PathBuf> {
    ["acpi_fan", "asus"].iter().filter_map(|chip| hwmon_file(hwmon_root, chip, "fan", None)).collect()
}

/// First plausible reading (≤ 9000 rpm) among `inputs`.
pub fn fan_rpm(inputs: &[PathBuf]) -> Option<u32> {
    inputs.iter().filter_map(|p| read_u64(p)).find(|r| *r <= FAN_MAX_RPM).map(|r| r as u32)
}

/// A sysfs integer in thousandths (milli-°C) → units.
pub fn read_milli(path: &Path) -> Option<f32> {
    read_u64(path).map(|v| v as f32 / 1000.0)
}

fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// (total, used) bytes of the filesystem holding `path`.
fn statvfs(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    // SAFETY: statvfs only writes into the zeroed struct we pass; the path is NUL-terminated.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let frsize = s.f_frsize as u64;
    let total = s.f_blocks as u64 * frsize;
    let used = (s.f_blocks as u64).saturating_sub(s.f_bavail as u64) * frsize;
    Some((total, used))
}

/// Sensor paths are resolved once; counters keep the previous sample for rates.
struct Sampler {
    temp_cpu: Option<PathBuf>,
    temp_nvme: Option<PathBuf>,
    temp_acpi: Option<PathBuf>,
    fans: Vec<PathBuf>,
    gpu: Option<PathBuf>,
    disk_dev: Option<String>,
    prev_at: Option<Instant>,
    prev_cpu: Vec<CpuTimes>,
    prev_net: (u64, u64),
    prev_disk: Option<(u64, u64)>,
    prev_rc6_ms: Option<u64>,
    cpu_model: String,
    prev_ifaces: Vec<(String, u64, u64)>,
    mounts: Vec<Mount>,
    mounts_at: Option<Instant>,
}

impl Sampler {
    fn new() -> Self {
        let hw = Path::new("/sys/class/hwmon");
        let gpu = fs::read_dir("/sys/class/drm").ok().and_then(|it| {
            let mut cards: Vec<PathBuf> =
                it.flatten().map(|e| e.path()).filter(|p| p.join("gt_cur_freq_mhz").is_file()).collect();
            cards.sort();
            cards.into_iter().next()
        });
        Self {
            temp_cpu: hwmon_file(hw, "coretemp", "temp", Some("Package id 0")),
            temp_nvme: hwmon_file(hw, "nvme", "temp", Some("Composite")),
            temp_acpi: hwmon_file(hw, "acpitz", "temp", None),
            fans: fan_inputs(hw),
            gpu,
            disk_dev: root_disk(&read("/proc/diskstats")),
            prev_at: None,
            prev_cpu: Vec::new(),
            prev_net: (0, 0),
            prev_disk: None,
            prev_rc6_ms: None,
            cpu_model: parse_cpu_model(&read("/proc/cpuinfo")),
            prev_ifaces: Vec::new(),
            mounts: Vec::new(),
            mounts_at: None,
        }
    }

    /// The first sample has `cpu_total = 0` and zero rates (there is no previous sample).
    fn sample(&mut self) -> (SysState, SysExtra) {
        let now = Instant::now();
        let dt = self.prev_at.map(|p| now.duration_since(p).as_secs_f64()).filter(|d| *d > 0.0);
        let rate = |cur: u64, prev: u64| dt.map_or(0.0, |dt| cur.saturating_sub(prev) as f64 / dt);

        let cpu = parse_cpu(&read("/proc/stat"));
        let (cpu_total, cpu_cores) = if self.prev_cpu.is_empty() {
            (0.0, vec![0.0; cpu.len().saturating_sub(1)])
        } else {
            cpu_usage(&self.prev_cpu, &cpu)
        };

        let mem = parse_meminfo(&read("/proc/meminfo"));

        let per_iface = parse_ifaces(&read("/proc/net/dev"));
        let (rx, tx) = per_iface.iter().fold((0, 0), |(rx, tx), (_, r, t)| (rx + r, tx + t));
        let net = NetState { rx_bps: rate(rx, self.prev_net.0), tx_bps: rate(tx, self.prev_net.1) };
        let net_root = Path::new("/sys/class/net");
        let ifaces = per_iface
            .iter()
            .map(|(name, r, t)| {
                let prev = self.prev_ifaces.iter().find(|p| &p.0 == name);
                let (rx_bps, tx_bps) = prev.map_or((0.0, 0.0), |p| (rate(*r, p.1), rate(*t, p.2)));
                Iface { name: name.clone(), rx_bps, tx_bps, up: iface_up(net_root, name) }
            })
            .collect();
        if self.mounts_at.is_none_or(|at| now.duration_since(at) >= MOUNTS_EVERY) {
            self.mounts = dedup_mounts(parse_mounts(&read("/proc/self/mounts")))
                .into_iter()
                .filter_map(|(_, path, _)| statvfs(&path).map(|(total, used)| Mount { path, used, total }))
                .collect();
            self.mounts_at = Some(now);
        }
        let (l1, l5, l15, running, threads) = parse_loadavg(&read("/proc/loadavg"));

        let sectors = self.disk_dev.as_deref().and_then(|d| parse_diskstats(&read("/proc/diskstats"), d));
        let disk = statvfs("/").map(|(total, used)| {
            let (read_bps, write_bps) = match (sectors, self.prev_disk) {
                (Some((r, w)), Some((pr, pw))) => (rate(r, pr) * 512.0, rate(w, pw) * 512.0),
                _ => (0.0, 0.0),
            };
            DiskState { used, total, read_bps, write_bps }
        });

        let gpu = self.gpu.as_ref().and_then(|card| {
            let mhz = read_u64(&card.join("gt_cur_freq_mhz"))? as u32;
            let max_mhz = read_u64(&card.join("gt_max_freq_mhz")).unwrap_or(0) as u32;
            let rc6 = read_u64(&card.join("gt/gt0/rc6_residency_ms"));
            let busy = match (rc6, self.prev_rc6_ms, dt) {
                (Some(cur), Some(prev), Some(dt)) => {
                    (1.0 - cur.saturating_sub(prev) as f64 / (dt * 1000.0)).clamp(0.0, 1.0) as f32
                }
                _ => 0.0,
            };
            self.prev_rc6_ms = rc6;
            Some(GpuState { mhz, max_mhz, busy })
        });

        self.prev_at = Some(now);
        self.prev_cpu = cpu;
        self.prev_net = (rx, tx);
        self.prev_disk = sectors;
        self.prev_ifaces = per_iface;

        let extra = SysExtra {
            hostname: read("/proc/sys/kernel/hostname").trim().to_string(),
            kernel: read("/proc/sys/kernel/osrelease").trim().to_string(),
            cpu_model: self.cpu_model.clone(),
            cpu_freq_mhz: cpu_freqs(Path::new("/sys/devices/system/cpu")),
            procs_total: count_processes(Path::new("/proc")),
            procs_running: running,
            threads_total: threads,
            mem_cached: mem.cached,
            mounts: self.mounts.clone(),
            ifaces,
        };
        let sys = SysState {
            cpu_total,
            cpu_cores,
            mem_total: mem.total,
            mem_available: mem.available,
            swap_total: mem.swap_total,
            swap_used: mem.swap_total.saturating_sub(mem.swap_free),
            gpu,
            temp_cpu_c: self.temp_cpu.as_deref().and_then(read_milli),
            temp_nvme_c: self.temp_nvme.as_deref().and_then(read_milli),
            temp_acpi_c: self.temp_acpi.as_deref().and_then(read_milli),
            fan_rpm: fan_rpm(&self.fans),
            disk,
            net,
            load_avg: [l1, l5, l15],
            uptime_s: parse_uptime(&read("/proc/uptime")),
        };
        (sys, extra)
    }
}

pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || {
        let mut sampler = Sampler::new();
        loop {
            let started = Instant::now();
            let (sys, extra) = sampler.sample();
            if tx.send(Update::Sys(sys)).is_err() || tx.send(Update::SysExtra(extra)).is_err() {
                return;
            }
            thread::sleep(Duration::from_secs(1).saturating_sub(started.elapsed()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    const STAT1: &str = include_str!("../../tests/fixtures/proc_stat_1.txt");
    const STAT2: &str = include_str!("../../tests/fixtures/proc_stat_2.txt");

    #[test]
    fn cpu_usage_from_two_samples() {
        let a = parse_cpu(STAT1);
        let b = parse_cpu(STAT2);
        assert_eq!(a.len(), 3); // aggregate + 2 cores in the fixture
        let (total, cores) = cpu_usage(&a, &b);
        assert!((total - 0.25).abs() < 0.01, "{total}");
        assert_eq!(cores.len(), 2);
        assert!((cores[0] - 0.5).abs() < 0.01);
        assert!(cores[1].abs() < 0.01);
    }

    #[test]
    fn meminfo_parses_kib() {
        let m = parse_meminfo(include_str!("../../tests/fixtures/meminfo.txt"));
        assert_eq!(m.total, 16_000_000 * 1024);
        assert_eq!(m.available, 4_000_000 * 1024);
        assert_eq!(m.swap_total, 8_000_000 * 1024);
        assert_eq!(m.swap_free, 7_000_000 * 1024);
        assert_eq!(m.cached, (20 + 4_207_900 + 409_804) * 1024); // Buffers + Cached + SReclaimable
    }

    #[test]
    fn mounts_keep_real_filesystems_one_per_device() {
        let all = parse_mounts(include_str!("../../tests/fixtures/mounts.txt"));
        let paths: Vec<&str> = all.iter().map(|(_, p, _)| p.as_str()).collect();
        assert_eq!(paths, ["/", "/home", "/var/cache/pacman/pkg", "/boot", "/run/media/user/My Disk"]);
        assert_eq!(all[3], ("/dev/nvme0n1p1".to_string(), "/boot".to_string(), "vfat".to_string()));
        let one_each: Vec<String> = dedup_mounts(all).into_iter().map(|(_, p, _)| p).collect();
        assert_eq!(one_each, ["/", "/boot", "/run/media/user/My Disk"]);
    }

    #[test]
    fn ifaces_are_per_interface_without_loopback() {
        let v = parse_ifaces(include_str!("../../tests/fixtures/net_dev.txt"));
        assert_eq!(v, [("wlan0".to_string(), 2000, 500), ("eth0".to_string(), 1000, 200)]);
    }

    #[test]
    fn iface_up_from_operstate_or_flags() {
        let d = tempfile::tempdir().unwrap();
        let put = |p: &str, v: &str| {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, v).unwrap();
        };
        put("wlp1s0/operstate", "up\n");
        put("docker0/operstate", "down\n");
        put("docker0/flags", "0x1003\n");
        put("tailscale0/operstate", "unknown\n"); // tun devices never report a carrier
        put("tailscale0/flags", "0x11d1\n");
        put("dummy0/operstate", "unknown\n");
        put("dummy0/flags", "0x1002\n");
        assert!(iface_up(d.path(), "wlp1s0"));
        assert!(!iface_up(d.path(), "docker0"));
        assert!(iface_up(d.path(), "tailscale0"));
        assert!(!iface_up(d.path(), "dummy0"));
        assert!(!iface_up(d.path(), "missing0"));
    }

    #[test]
    fn cpu_freqs_sorted_by_index_in_mhz() {
        let d = tempfile::tempdir().unwrap();
        for (cpu, khz) in [(0, 1_909_052), (10, 400_000), (2, 2_500_000), (1, 800_000)] {
            let dir = d.path().join(format!("cpu{cpu}/cpufreq"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("scaling_cur_freq"), format!("{khz}\n")).unwrap();
        }
        std::fs::create_dir_all(d.path().join("cpufreq")).unwrap(); // not a core
        assert_eq!(cpu_freqs(d.path()), [1909, 800, 2500, 400]);
    }

    #[test]
    fn cpu_model_is_first_model_name() {
        let info = "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) Ultra 5 225H\n\nprocessor\t: 1\nmodel name\t: Intel(R) Core(TM) Ultra 5 225H\n";
        assert_eq!(parse_cpu_model(info), "Intel(R) Core(TM) Ultra 5 225H");
        assert_eq!(parse_cpu_model(""), "");
    }

    #[test]
    fn process_count_counts_numeric_dirs() {
        let d = tempfile::tempdir().unwrap();
        for n in ["1", "42", "self", "sys", "777"] {
            std::fs::create_dir_all(d.path().join(n)).unwrap();
        }
        assert_eq!(count_processes(d.path()), 3);
    }

    #[test]
    fn net_dev_sums_non_loopback() {
        let (rx, tx) = parse_net_dev(include_str!("../../tests/fixtures/net_dev.txt"));
        assert_eq!((rx, tx), (3000, 700)); // lo excluded
    }

    #[test]
    fn diskstats_picks_named_disk_sectors() {
        let d = parse_diskstats(include_str!("../../tests/fixtures/diskstats.txt"), "nvme0n1");
        assert_eq!(d, Some((709048620, 119793318)));
        assert_eq!(parse_diskstats("", "nvme0n1"), None);
    }

    #[test]
    fn garbage_input_does_not_panic() {
        assert!(parse_cpu("nonsense\ncpu x y").is_empty());
        let _ = parse_meminfo("MemTotal: lots kB");
        let _ = parse_net_dev("Inter-|\n face |\n eth0: x");
    }

    #[test]
    fn root_disk_is_first_whole_disk() {
        assert_eq!(root_disk(include_str!("../../tests/fixtures/diskstats.txt")).as_deref(), Some("nvme0n1"));
        let sd =
            "   7 0 loop0 1 0 2 0 0 0 0 0 0 0 0\n   8 1 sda1 1 0 2 0 0 0 0 0 0 0 0\n   8 0 sda 1 0 2 0 0 0 0 0 0 0 0\n";
        assert_eq!(root_disk(sd).as_deref(), Some("sda"));
        assert_eq!(root_disk(""), None);
    }

    #[test]
    fn hwmon_picks_labelled_input_else_first() {
        let d = tempfile::tempdir().unwrap();
        let put = |p: &str, v: &str| {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, v).unwrap();
        };
        put("hwmon0/name", "coretemp\n");
        put("hwmon0/temp1_label", "Core 0\n");
        put("hwmon0/temp1_input", "51000\n");
        put("hwmon0/temp3_label", "Package id 0\n");
        put("hwmon0/temp3_input", "58000\n");
        put("hwmon1/name", "acpitz\n");
        put("hwmon1/temp1_input", "45500\n");
        put("hwmon2/name", "asus\n");
        put("hwmon2/fan1_input", "2400\n");
        let hw = d.path();
        let pkg = hwmon_file(hw, "coretemp", "temp", Some("Package id 0")).unwrap();
        assert_eq!(pkg, hw.join("hwmon0/temp3_input"));
        assert_eq!(read_milli(&pkg), Some(58.0));
        assert_eq!(hwmon_file(hw, "acpitz", "temp", Some("Composite")), Some(hw.join("hwmon1/temp1_input")));
        assert_eq!(hwmon_file(hw, "asus", "fan", None), Some(hw.join("hwmon2/fan1_input")));
        assert_eq!(hwmon_file(hw, "nvme", "temp", Some("Composite")), None);
        assert_eq!(hwmon_file(&hw.join("missing"), "nvme", "temp", None), None);
    }

    #[test]
    fn fan_prefers_acpi_fan_and_ignores_implausible_rpm() {
        let hw = |fans: &[(&str, &str)]| {
            let d = tempfile::tempdir().unwrap();
            for (i, (chip, rpm)) in fans.iter().enumerate() {
                let dir = d.path().join(format!("hwmon{i}"));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("name"), format!("{chip}\n")).unwrap();
                std::fs::write(dir.join("fan1_input"), format!("{rpm}\n")).unwrap();
            }
            d
        };
        // this machine: asus reads 12000 (driver quirk), acpi_fan a plausible 2848
        let d = hw(&[("asus", "12000"), ("acpi_fan", "2848")]);
        assert_eq!(fan_rpm(&fan_inputs(d.path())), Some(2848));
        let d = hw(&[("asus", "2400")]);
        assert_eq!(fan_rpm(&fan_inputs(d.path())), Some(2400)); // asus only as a fallback
        let d = hw(&[("asus", "13800"), ("acpi_fan", "9500")]);
        assert_eq!(fan_rpm(&fan_inputs(d.path())), None); // both implausible
        let d = hw(&[("acpi_fan", "0")]);
        assert_eq!(fan_rpm(&fan_inputs(d.path())), Some(0)); // a stopped fan is a real reading
        assert_eq!(fan_rpm(&fan_inputs(&d.path().join("none"))), None);
    }

    #[test]
    fn loadavg_and_uptime() {
        assert_eq!(parse_loadavg("8.15 9.20 10.69 3/2900 1024162\n"), (8.15, 9.20, 10.69, 3, 2900));
        assert_eq!(parse_loadavg("junk"), (0.0, 0.0, 0.0, 0, 0));
        assert_eq!(parse_uptime("11194.43 108350.59\n"), 11194);
        assert_eq!(parse_uptime(""), 0);
    }
}
