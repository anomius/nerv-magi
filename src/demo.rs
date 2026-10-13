//! `--demo`: a synthetic, animated stream of Updates so the views can be built, screenshotted and
//! demoed without herdr, a battery, or music. Every ~40 s one agent goes BLOCKED for ~10 s.

use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::state::*;

/// A made-up solo developer's week: a few projects, Claude and Codex agents in herdr.
const AGENTS: &[(&str, &str, &str, AgentStatus)] = &[
    ("auth-refactor", "claude", "w3", AgentStatus::Working),
    ("rate-limiter", "claude", "w3", AgentStatus::Working),
    ("checkout-flow", "claude", "w4", AgentStatus::Working),
    ("e2e-suite", "codex", "w4", AgentStatus::Working),
    ("push-notify", "claude", "w5", AgentStatus::Done),
    ("tf-plan-review", "codex", "w6", AgentStatus::Done),
    ("k8s-upgrade", "claude", "w6", AgentStatus::Idle),
    ("api-reference", "claude", "w7", AgentStatus::Idle),
    ("release-notes", "codex", "w7", AgentStatus::Idle),
    ("hypr-rice", "claude", "w2", AgentStatus::Idle),
];

const WORKSPACES: &[(&str, &str)] = &[
    ("w1", "hub"),
    ("w2", "dotfiles"),
    ("w3", "api-gateway"),
    ("w4", "storefront"),
    ("w5", "mobile-app"),
    ("w6", "infra"),
    ("w7", "docs-site"),
];

/// Pane id of `AGENTS[i]`, as `snapshot` numbers them.
fn pane(i: usize) -> String {
    format!("{}:p{}", AGENTS[i].2, i + 1)
}

fn ws_label(id: &str) -> String {
    WORKSPACES.iter().find(|w| w.0 == id).map_or(id, |w| w.1).to_string()
}

fn snapshot(now: SystemTime) -> Update {
    let agents = AGENTS
        .iter()
        .enumerate()
        .map(|(i, (name, kind, ws, status))| Agent {
            pane_id: pane(i),
            name: Some(name.to_string()),
            kind: kind.to_string(),
            status: *status,
            since: now - Duration::from_secs(60 * (i as u64 * 7 + 3)),
            workspace_id: ws.to_string(),
            workspace_label: ws_label(ws),
            tab_label: name.to_string(),
            title: format!("{} brief", name.replace('-', " ")),
            rss_bytes: Some(180_000_000 + i as u64 * 31_000_000),
        })
        .collect();
    let workspaces = WORKSPACES
        .iter()
        .enumerate()
        .map(|(i, (id, l))| Workspace { id: id.to_string(), label: l.to_string(), number: i as u32 + 1 })
        .collect();
    Update::HerdrSnapshot { agents, workspaces }
}

fn wave(t: f64, period: f64, phase: f64) -> f64 {
    0.5 + 0.5 * ((t / period + phase) * std::f64::consts::TAU).sin()
}

/// Spawn the demo feeder thread. It runs until the receiver is dropped.
pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || {
        let start = Instant::now();
        let now = SystemTime::now();
        let _ = tx.send(Update::HerdrLink(HerdrLink::Linked));
        let _ = tx.send(snapshot(now));
        let mut last_second = u64::MAX;
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        loop {
            let t = start.elapsed().as_secs_f64();
            let now = SystemTime::now();

            // spectrum @ 30 fps: smooth bands plus a little noise
            let bars: Vec<f32> = (0..48)
                .map(|i| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    let noise = (seed % 1000) as f64 / 1000.0;
                    let x = i as f64 / 48.0;
                    let v = (1.0 - x).powf(0.7) * wave(t, 1.7, x * 3.0) * 0.8 + noise * 0.2;
                    v.clamp(0.0, 1.0) as f32
                })
                .collect();
            if tx.send(Update::Spectrum(bars)).is_err() {
                return;
            }

            let sec = t as u64;
            if sec != last_second {
                last_second = sec;
                let cores: Vec<f32> = (0..14).map(|c| wave(t, 6.0 + c as f64, c as f64 * 0.37) as f32 * 0.9).collect();
                let cpu_total = cores.iter().sum::<f32>() / cores.len() as f32;
                let sys = SysState {
                    cpu_total,
                    cpu_cores: cores,
                    mem_total: 15_600_000_000,
                    mem_available: (2.0e9 + 2.5e9 * wave(t, 90.0, 0.0)) as u64,
                    swap_total: 8_000_000_000,
                    swap_used: 1_200_000_000,
                    gpu: Some(GpuState { mhz: 600 + (1600.0 * wave(t, 9.0, 0.2)) as u32, max_mhz: 2200, busy: wave(t, 9.0, 0.2) as f32 }),
                    temp_cpu_c: Some(48.0 + 30.0 * wave(t, 20.0, 0.1) as f32),
                    temp_nvme_c: Some(41.0),
                    temp_acpi_c: Some(45.0),
                    fan_rpm: Some(1800 + (1600.0 * wave(t, 20.0, 0.1)) as u32),
                    disk: Some(DiskState { used: 610_000_000_000, total: 1_000_000_000_000, read_bps: 2.0e6 * wave(t, 4.0, 0.0), write_bps: 5.0e5 * wave(t, 3.0, 0.5) }),
                    net: NetState { rx_bps: 1.5e6 * wave(t, 5.0, 0.0), tx_bps: 1.2e5 * wave(t, 7.0, 0.3) },
                    load_avg: [2.1, 1.8, 1.5],
                    uptime_s: 86_400 + sec,
                };
                let _ = tx.send(Update::Sys(sys));

                // battery: the series' 4:59:56 countdown, on internal power
                let left = (4 * 3600 + 59 * 60 + 56u64).saturating_sub(sec);
                let _ = tx.send(Update::Power(PowerState {
                    ac_online: false,
                    battery: Some(Battery { percent: 72.0 - sec as f32 / 600.0, status: BatStatus::Discharging, seconds_left: Some(left), watts: Some(9.4) }),
                    profile: Profile::Balanced,
                }));

                let _ = tx.send(Update::Audio(Some(AudioState { sink_name: "WH-1000XM5".into(), volume: 0.6, muted: false })));
                let track_len = 220.0;
                let _ = tx.send(Update::Media(Some(MediaState {
                    player: "spotify".into(),
                    status: PlayStatus::Playing,
                    title: "Decisive Battle".into(),
                    artist: "S-DAT Track 25".into(),
                    album: "Operation Yashima".into(),
                    position_s: t % track_len,
                    length_s: Some(track_len),
                    sampled_at: now,
                })));

                // density additions
                let freqs: Vec<u32> = (0..14).map(|c| 800 + (3700.0 * wave(t, 6.0 + c as f64, c as f64 * 0.37)) as u32).collect();
                let _ = tx.send(Update::SysExtra(SysExtra {
                    hostname: "nerv-hq".into(),
                    kernel: "7.2.5-3-omarchy".into(),
                    cpu_model: "Intel Core Ultra 5 225H".into(),
                    cpu_freq_mhz: freqs,
                    procs_total: 612,
                    procs_running: 3 + (sec % 4) as u32,
                    threads_total: 3105,
                    mem_cached: 4_800_000_000,
                    mounts: vec![
                        Mount { path: "/".into(), used: 610_000_000_000, total: 1_000_000_000_000 },
                        Mount { path: "/boot".into(), used: 210_000_000, total: 2_000_000_000 },
                    ],
                    ifaces: vec![
                        Iface { name: "wlan0".into(), rx_bps: 1.5e6 * wave(t, 5.0, 0.0), tx_bps: 1.2e5 * wave(t, 7.0, 0.3), up: true },
                        Iface { name: "tailscale0".into(), rx_bps: 2.0e3, tx_bps: 1.1e3, up: true },
                    ],
                }));
                let _ = tx.send(Update::PowerExtra(PowerExtra {
                    health_pct: Some(83.9),
                    cycles: Some(214),
                    voltage_v: Some(15.6),
                    current_a: Some(1.99),
                    model: Some("4-cell Li-ion".into()),
                }));
                let procs = [
                    ("node", 3.0e9), ("claude", 3.1e8), ("claude", 3.0e8), ("codex", 1.9e8), ("Hyprland", 1.6e8),
                    ("chromium", 1.2e8), ("postgres", 2.4e8), ("pnpm", 4.1e8), ("cava", 9.0e6), ("pipewire", 2.1e7),
                    ("herdr", 6.0e7), ("nerv", 4.5e7), ("kitty", 8.0e7), ("docker", 1.1e8), ("python3", 3.3e7),
                ];
                let mut rows: Vec<ProcRow> = procs
                    .iter()
                    .enumerate()
                    .map(|(i, (name, rss))| ProcRow {
                        pid: 1000 + i as i32 * 977,
                        name: name.to_string(),
                        cpu: (140.0 * wave(t, 3.0 + i as f64, i as f64 * 0.21)).max(0.1) as f32 / (1.0 + i as f32 * 0.4),
                        rss_bytes: *rss as u64,
                        threads: 4 + i as u32 * 3,
                        state: if i % 5 == 0 { 'R' } else { 'S' },
                    })
                    .collect();
                rows.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
                let _ = tx.send(Update::Procs(rows));
                if sec % 10 == 0 {
                    let job = |i: usize, pane_no: u32, label: &str, command: &str, cwd: &str| PaneJob {
                        pane_id: format!("{}:p{pane_no}", AGENTS[i].2),
                        workspace_label: ws_label(AGENTS[i].2),
                        tab_label: AGENTS[i].0.into(),
                        label: label.into(),
                        command: command.into(),
                        cwd: cwd.into(),
                    };
                    let _ = tx.send(Update::Jobs(vec![
                        job(0, 11, "api-dev", "cargo watch -x 'run -- --port 8080'", "~/code/api-gateway"),
                        job(3, 12, "web-dev", "pnpm dev --port 5173", "~/code/storefront"),
                        job(3, 13, "playwright", "pnpm exec playwright test --workers 4", "~/code/storefront"),
                        job(6, 14, "db-tunnel", "ssh -N -L 5432:db.internal:5432 bastion", "~"),
                    ]));
                    let meta = AGENTS
                        .iter()
                        .enumerate()
                        .map(|(i, (name, _, ws, _))| {
                            (pane(i), AgentMeta {
                                cwd: format!("~/code/{}", ws_label(ws)),
                                branch: Some(format!("feat/{name}")),
                                session_id: Some(format!("{:08x}-demo", i * 0x1f3a)),
                            })
                        })
                        .collect();
                    let _ = tx.send(Update::AgentMeta(meta));
                    let usage = AGENTS
                        .iter()
                        .enumerate()
                        .map(|(i, (_, kind, _, _))| {
                            let claude = *kind == "claude";
                            let window: u64 = if claude { 1_000_000 } else { 258_400 };
                            let used = ((window as f64) * (0.12 + 0.08 * i as f64 + 0.05 * wave(t, 30.0 + i as f64, 0.0))).min(window as f64 * 0.97) as u64;
                            (pane(i), AgentUsage {
                                model: Some(if claude { if i % 3 == 2 { "claude-sonnet-5-5" } else { "claude-opus-5-5" } } else { "gpt-6-luna" }.into()),
                                effort: Some(if claude { "xhigh" } else { "high" }.into()),
                                mode: Some(if claude { "auto" } else { "on-request" }.into()),
                                version: Some(if claude { "2.1.296" } else { "0.162.1" }.into()),
                                context_used: Some(used),
                                context_window: Some(window),
                                context_window_estimated: false,
                                input_tokens: 40_000 + i as u64 * 9_000 + sec * 120,
                                output_tokens: 18_000 + i as u64 * 4_100 + sec * 35,
                                cache_read_tokens: 2_500_000 + i as u64 * 900_000 + sec * 9_000,
                                cache_write_tokens: 120_000 + i as u64 * 30_000,
                                reasoning_tokens: 6_000 + i as u64 * 1_500,
                                turns: 40 + i as u32 * 11,
                                session_started: Some(now - Duration::from_secs(60 * (i as u64 * 9 + 20))),
                                last_activity: Some(now - Duration::from_secs(i as u64 * 7)),
                                cost_usd: if claude { Some(1.4 + i as f64 * 0.9) } else { None },
                            })
                        })
                        .collect();
                    let _ = tx.send(Update::AgentUsage(usage));
                    let epoch = |s: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
                    let now_s = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
                    let _ = tx.send(Update::PlanLimits(vec![
                        PlanLimits {
                            provider: "claude".into(),
                            plan: Some("max".into()),
                            windows: vec![
                                LimitWindow { label: "5H".into(), used_pct: 37.0 + (sec % 600) as f32 / 30.0, resets_at: Some(epoch(now_s + 2 * 3600 + 840)), window_minutes: Some(300) },
                                LimitWindow { label: "7D".into(), used_pct: 12.0, resets_at: Some(epoch(now_s + 4 * 86400)), window_minutes: Some(10080) },
                            ],
                            credits: None,
                            observed_at: Some(now),
                        },
                        PlanLimits {
                            provider: "codex".into(),
                            plan: Some("pro".into()),
                            windows: vec![LimitWindow { label: "WEEK".into(), used_pct: 17.0, resets_at: Some(epoch(now_s + 5 * 86400 + 3600)), window_minutes: Some(10080) }],
                            credits: Some("62500".into()),
                            observed_at: Some(now),
                        },
                    ]));
                }

                // scripted agent drama
                let cycle = sec % 40;
                let blocked = if (20..30).contains(&cycle) { AgentStatus::Blocked } else { AgentStatus::Working };
                let _ = tx.send(Update::AgentStatus { pane_id: pane(0), status: blocked, title: None });
                if sec % 25 == 12 {
                    let _ = tx.send(Update::AgentStatus { pane_id: pane(2), status: AgentStatus::Done, title: None });
                }
                if sec % 25 == 20 {
                    let _ = tx.send(Update::AgentStatus { pane_id: pane(2), status: AgentStatus::Working, title: None });
                }
            }
            thread::sleep(Duration::from_millis(33));
        }
    });
}
