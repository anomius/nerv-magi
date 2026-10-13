//! The contract between collectors (which send `Update`s) and renderers (which read `State`).
//!
//! Collectors never touch `State` directly: they send `Update` values over one mpsc channel and the
//! UI thread applies them. Everything derived for display (sort order, alert level, MAGI votes,
//! workspace rollups) is a pure function here so both the TV and hub views agree and it is tested.

use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::Receiver;
use std::time::{Duration, SystemTime};

use serde::Serialize;

use crate::config::Thresholds;
use crate::history::Histories;

pub const TICKER_CAP: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Blocked,
    Working,
    Done,
    Idle,
    Unknown,
}

impl AgentStatus {
    /// herdr's `agent_status` strings: idle | working | blocked | done | unknown.
    pub fn parse(s: &str) -> Self {
        match s {
            "blocked" => Self::Blocked,
            "working" => Self::Working,
            "done" => Self::Done,
            "idle" => Self::Idle,
            _ => Self::Unknown,
        }
    }

    /// Display label in the Eva vocabulary.
    pub fn label(self) -> &'static str {
        match self {
            Self::Blocked => "BLOCKED",
            Self::Working => "ACTIVE",
            Self::Done => "COMPLETE",
            Self::Idle => "STANDBY",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Institutional Japanese label (glyphs are in assets/fonts/jp-glyphs.txt).
    pub fn kanji(self) -> &'static str {
        match self {
            Self::Blocked => "警告",
            Self::Working => "起動中",
            Self::Done => "完了",
            Self::Idle => "待機",
            Self::Unknown => "不明",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Agent {
    pub pane_id: String,
    pub name: Option<String>,
    /// claude | codex | … (herdr's `agent` field)
    pub kind: String,
    pub status: AgentStatus,
    /// When this agent entered `status` (as observed by this process).
    pub since: SystemTime,
    pub workspace_id: String,
    pub workspace_label: String,
    pub tab_label: String,
    /// herdr `terminal_title_stripped`: usually the chat's task title.
    pub title: String,
    /// Resident memory of the pane's process tree.
    pub rss_bytes: Option<u64>,
}

impl Agent {
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("({})", self.pane_id))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Workspace {
    pub id: String,
    pub label: String,
    pub number: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum HerdrLink {
    Connecting,
    Linked,
    Lost { since: SystemTime, reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct HerdrState {
    pub link: HerdrLink,
    pub agents: Vec<Agent>,
    pub workspaces: Vec<Workspace>,
}

impl Default for HerdrState {
    fn default() -> Self {
        Self { link: HerdrLink::Connecting, agents: Vec::new(), workspaces: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GpuState {
    pub mhz: u32,
    pub max_mhz: u32,
    /// 0..1, from rc6 residency (1 - idle fraction).
    pub busy: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiskState {
    pub used: u64,
    pub total: u64,
    pub read_bps: f64,
    pub write_bps: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NetState {
    pub rx_bps: f64,
    pub tx_bps: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SysState {
    /// 0..1
    pub cpu_total: f32,
    /// 0..1 per logical core
    pub cpu_cores: Vec<f32>,
    pub mem_total: u64,
    pub mem_available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub gpu: Option<GpuState>,
    pub temp_cpu_c: Option<f32>,
    pub temp_nvme_c: Option<f32>,
    pub temp_acpi_c: Option<f32>,
    pub fan_rpm: Option<u32>,
    pub disk: Option<DiskState>,
    pub net: NetState,
    pub load_avg: [f32; 3],
    pub uptime_s: u64,
}

impl SysState {
    pub fn mem_used(&self) -> u64 {
        self.mem_total.saturating_sub(self.mem_available)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    LowPower,
    Balanced,
    Performance,
    Unknown(String),
}

/// The STOP · SLOW · NORMAL · RACING lamps from the Eva activity-limit timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PowerMode {
    Stop,
    Slow,
    Normal,
    Racing,
}

impl PowerMode {
    pub const ALL: [PowerMode; 4] = [PowerMode::Stop, PowerMode::Slow, PowerMode::Normal, PowerMode::Racing];

    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "STOP",
            Self::Slow => "SLOW",
            Self::Normal => "NORMAL",
            Self::Racing => "RACING",
        }
    }
}

impl Profile {
    /// `/sys/firmware/acpi/platform_profile` or powerprofilesctl names.
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "low-power" | "quiet" | "power-saver" | "cool" => Self::LowPower,
            "balanced" | "balanced-performance" => Self::Balanced,
            "performance" => Self::Performance,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn mode(&self) -> PowerMode {
        match self {
            Self::LowPower => PowerMode::Slow,
            Self::Balanced => PowerMode::Normal,
            Self::Performance => PowerMode::Racing,
            Self::Unknown(_) => PowerMode::Stop,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BatStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatStatus {
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Battery {
    /// 0..100
    pub percent: f32,
    pub status: BatStatus,
    /// Time to empty while discharging, time to full while charging; None when unknown.
    pub seconds_left: Option<u64>,
    pub watts: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PowerState {
    pub ac_online: bool,
    pub battery: Option<Battery>,
    pub profile: Profile,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioState {
    /// Human name of the default sink, e.g. "WH-1000XM5".
    pub sink_name: String,
    /// 0..1.5 (wpctl allows boost above 1.0)
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlayStatus {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaState {
    pub player: String,
    pub status: PlayStatus,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Position when sampled.
    pub position_s: f64,
    pub length_s: Option<f64>,
    pub sampled_at: SystemTime,
}

impl MediaState {
    /// Position interpolated to `now` while playing, clamped to the track length.
    pub fn position_at(&self, now: SystemTime) -> f64 {
        let mut p = self.position_s;
        if self.status == PlayStatus::Playing {
            p += now.duration_since(self.sampled_at).unwrap_or_default().as_secs_f64();
        }
        match self.length_s {
            Some(len) => p.min(len),
            None => p,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TickerEvent {
    pub at: SystemTime,
    pub agent: String,
    pub from: AgentStatus,
    pub to: AgentStatus,
}

// ── Density additions (2026-10-13 revision): extra data for an information-dense board. ──────

/// One row of the btop-style process table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcRow {
    pub pid: i32,
    pub name: String,
    /// Percent of one core over the last sample interval (can exceed 100).
    pub cpu: f32,
    pub rss_bytes: u64,
    pub threads: u32,
    /// /proc state letter: R S D Z T I …
    pub state: char,
}

/// A herdr pane that is running a non-shell, non-agent command (dev servers, evals, tunnels…).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaneJob {
    pub pane_id: String,
    pub workspace_label: String,
    pub tab_label: String,
    /// herdr pane label, or the pane id when unlabeled.
    pub label: String,
    /// basename(argv[0]) + args, e.g. "cargo watch -x run".
    pub command: String,
    pub cwd: String,
}

/// Per-agent details that are not in herdr's agent status.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AgentMeta {
    /// foreground cwd, `~`-shortened.
    pub cwd: String,
    /// git branch of `cwd` (read from .git/HEAD, worktrees followed), None outside a repo / detached.
    pub branch: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mount {
    pub path: String,
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Iface {
    pub name: String,
    pub rx_bps: f64,
    pub tx_bps: f64,
    pub up: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SysExtra {
    pub hostname: String,
    pub kernel: String,
    pub cpu_model: String,
    /// Current frequency per logical core (scaling_cur_freq / 1000).
    pub cpu_freq_mhz: Vec<u32>,
    pub procs_total: u32,
    pub procs_running: u32,
    pub threads_total: u32,
    /// Page cache + buffers.
    pub mem_cached: u64,
    /// Real filesystems only (no tmpfs/overlay/squashfs), e.g. "/", "/home", "/boot".
    pub mounts: Vec<Mount>,
    /// Non-loopback interfaces.
    pub ifaces: Vec<Iface>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PowerExtra {
    /// charge_full / charge_full_design × 100 (or energy_*).
    pub health_pct: Option<f32>,
    pub cycles: Option<u32>,
    pub voltage_v: Option<f32>,
    pub current_a: Option<f32>,
    pub model: Option<String>,
}

// ── Usage additions (2026-10-13 revision 4): agent type and token / context / plan usage. ───────

/// Per-agent usage, read from the agent's own session log (Claude transcript / Codex rollout) and,
/// for Claude, the statusline cache.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AgentUsage {
    /// Latest model id, e.g. "claude-opus-5-5", "gpt-6-luna".
    pub model: Option<String>,
    /// Reasoning effort, e.g. "xhigh", "high".
    pub effort: Option<String>,
    /// Permission / approval mode, e.g. "auto", "on-request".
    pub mode: Option<String>,
    /// Agent CLI version, e.g. "2.1.296".
    pub version: Option<String>,
    /// Tokens currently in the context window (last turn's input + cache read + cache write).
    pub context_used: Option<u64>,
    pub context_window: Option<u64>,
    /// True when `context_window` was inferred (no statusline / rollout value).
    pub context_window_estimated: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    /// Assistant turns (unique message ids) in this session.
    pub turns: u32,
    pub session_started: Option<SystemTime>,
    pub last_activity: Option<SystemTime>,
    /// Claude Code's own session cost estimate (statusline `cost.total_cost_usd`), when available.
    pub cost_usd: Option<f64>,
}

impl AgentUsage {
    /// 0..100
    pub fn context_pct(&self) -> Option<f32> {
        match (self.context_used, self.context_window) {
            (Some(u), Some(w)) if w > 0 => Some((u as f64 / w as f64 * 100.0).min(100.0) as f32),
            _ => None,
        }
    }

    /// Eva "sync ratio": context still free, 0..100 (falls as the context fills up).
    pub fn sync_ratio(&self) -> Option<f32> {
        self.context_pct().map(|p| 100.0 - p)
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens
    }
}

/// "claude-opus-5-5" → "OPUS 5.5", "claude-sonnet-5-5[1m]" → "SONNET 5.5 1M", "gpt-6-luna" → "GPT-6 LUNA",
/// "claude-haiku-4-5-20251001" → "HAIKU 4.5".
pub fn model_short(model: &str) -> String {
    let (base, one_m) = match model.strip_suffix("[1m]") {
        Some(b) => (b, true),
        None => (model, false),
    };
    let base = base.strip_prefix("claude-").unwrap_or(base);
    let mut words: Vec<String> = Vec::new();
    let mut nums: Vec<&str> = Vec::new();
    for part in base.split('-') {
        if part.len() >= 8 && part.chars().all(|c| c.is_ascii_digit()) {
            continue; // date stamp
        }
        if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) && !words.is_empty() && !words.last().unwrap().ends_with(char::is_numeric) {
            nums.push(part);
            continue;
        }
        if !nums.is_empty() {
            words.push(nums.join("."));
            nums.clear();
        }
        words.push(part.to_uppercase());
    }
    if !nums.is_empty() {
        words.push(nums.join("."));
    }
    let mut s = words.join(" ");
    // keep "GPT-6" style hyphen for a word followed by a single number
    if let Some(rest) = s.strip_prefix("GPT ") {
        s = format!("GPT-{rest}");
    }
    if one_m {
        s.push_str(" 1M");
    }
    s
}

/// One plan usage window, e.g. Claude "5H" 37 % or Codex "WEEK" 17 %.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LimitWindow {
    /// "5H", "7D", "WEEK", …
    pub label: String,
    /// 0..100
    pub used_pct: f32,
    pub resets_at: Option<SystemTime>,
    pub window_minutes: Option<u32>,
}

/// Account-level plan limits for one provider.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PlanLimits {
    /// "claude" | "codex"
    pub provider: String,
    /// e.g. "pro", "max"
    pub plan: Option<String>,
    pub windows: Vec<LimitWindow>,
    /// Remaining credits, as reported (Codex).
    pub credits: Option<String>,
    /// When the source file was written.
    pub observed_at: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Herdr,
    Sys,
    Power,
    Audio,
    Media,
    Spectrum,
}

/// Everything collectors can tell the UI thread.
#[derive(Debug, Clone)]
pub enum Update {
    HerdrLink(HerdrLink),
    /// Full replacement of agents + workspaces (on connect and every periodic re-snapshot).
    HerdrSnapshot { agents: Vec<Agent>, workspaces: Vec<Workspace> },
    /// A pushed `pane.agent_status_changed` event.
    AgentStatus { pane_id: String, status: AgentStatus, title: Option<String> },
    /// (pane_id, resident bytes of the pane's process tree)
    AgentRss(Vec<(String, u64)>),
    Sys(SysState),
    Power(PowerState),
    Audio(Option<AudioState>),
    Media(Option<MediaState>),
    Spectrum(Vec<f32>),
    SourceOffline(Source, String),
    SourceOnline(Source),
    // density additions
    /// Top processes, already sorted by cpu desc (collector sends at most 40).
    Procs(Vec<ProcRow>),
    /// Full replacement of running pane jobs.
    Jobs(Vec<PaneJob>),
    /// (pane_id, meta) for agent panes; merged into `State::agent_meta`.
    AgentMeta(Vec<(String, AgentMeta)>),
    SysExtra(SysExtra),
    PowerExtra(PowerExtra),
    // usage additions
    /// (pane_id, usage) for agent panes; merged into `State::usage`.
    AgentUsage(Vec<(String, AgentUsage)>),
    /// Full replacement of plan limits (one entry per provider).
    PlanLimits(Vec<PlanLimits>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AlertLevel {
    Nominal,
    Caution,
    Emergency,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MagiVote {
    /// "MELCHIOR·1" | "BALTHASAR·2" | "CASPER·3"
    pub unit: &'static str,
    /// "COMPUTE" | "MEMORY" | "PILOTS"
    pub subject: &'static str,
    pub approve: bool,
    /// Short reason shown in the deliberation column, e.g. "CPU 34% · 58°C".
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AgentCounts {
    pub blocked: usize,
    pub working: usize,
    pub done: usize,
    pub idle: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceRollup {
    pub id: String,
    pub label: String,
    pub agents: AgentCounts,
}

#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub now: SystemTime,
    pub started: SystemTime,
    pub herdr: HerdrState,
    pub sys: Option<SysState>,
    pub power: Option<PowerState>,
    pub audio: Option<AudioState>,
    pub media: Option<MediaState>,
    pub spectrum: Vec<f32>,
    pub history: Histories,
    pub ticker: VecDeque<TickerEvent>,
    /// Sources that reported themselves offline, with the reason.
    pub offline: BTreeMap<Source, String>,
    /// Bumped on every agent status transition; renderers use it to trigger the red flash.
    pub transitions: u64,
    /// When the most recent transition *into* Blocked happened.
    pub last_blocked_at: Option<SystemTime>,
    // density additions
    pub procs: Vec<ProcRow>,
    pub jobs: Vec<PaneJob>,
    pub agent_meta: BTreeMap<String, AgentMeta>,
    pub sysx: Option<SysExtra>,
    pub powerx: Option<PowerExtra>,
    // usage additions
    pub usage: BTreeMap<String, AgentUsage>,
    pub limits: Vec<PlanLimits>,
}

impl State {
    pub fn new(now: SystemTime) -> Self {
        Self {
            now,
            started: now,
            herdr: HerdrState::default(),
            sys: None,
            power: None,
            audio: None,
            media: None,
            spectrum: Vec::new(),
            history: Histories::new(),
            ticker: VecDeque::new(),
            offline: BTreeMap::new(),
            transitions: 0,
            last_blocked_at: None,
            procs: Vec::new(),
            jobs: Vec::new(),
            agent_meta: BTreeMap::new(),
            sysx: None,
            powerx: None,
            usage: BTreeMap::new(),
            limits: Vec::new(),
        }
    }

    pub fn meta(&self, agent: &Agent) -> Option<&AgentMeta> {
        self.agent_meta.get(&agent.pane_id)
    }

    pub fn usage_of(&self, agent: &Agent) -> Option<&AgentUsage> {
        self.usage.get(&agent.pane_id)
    }

    /// "CLAUDE · OPUS 5.5 · XHIGH" (kind always; model and effort when known).
    pub fn type_label(&self, agent: &Agent) -> String {
        let mut parts = vec![agent.kind.to_uppercase()];
        if let Some(u) = self.usage_of(agent) {
            if let Some(m) = &u.model {
                parts.push(model_short(m));
            }
            if let Some(e) = &u.effort {
                parts.push(e.to_uppercase());
            }
        }
        parts.join(" · ")
    }

    pub fn limits_for(&self, provider: &str) -> Option<&PlanLimits> {
        self.limits.iter().find(|l| l.provider == provider)
    }

    pub fn apply(&mut self, update: Update, now: SystemTime) {
        self.now = now;
        match update {
            Update::HerdrLink(link) => self.herdr.link = link,
            Update::HerdrSnapshot { agents, workspaces } => self.apply_snapshot(agents, workspaces, now),
            Update::AgentStatus { pane_id, status, title } => {
                let mut transition = None;
                if let Some(agent) = self.herdr.agents.iter_mut().find(|a| a.pane_id == pane_id) {
                    if let Some(t) = title {
                        agent.title = t;
                    }
                    if agent.status != status {
                        transition = Some((agent.display_name(), agent.status, status));
                        agent.status = status;
                        agent.since = now;
                    }
                }
                if let Some((name, from, to)) = transition {
                    self.record_transition(name, from, to, now);
                }
            }
            Update::AgentRss(rows) => {
                for (pane_id, bytes) in rows {
                    if let Some(agent) = self.herdr.agents.iter_mut().find(|a| a.pane_id == pane_id) {
                        agent.rss_bytes = Some(bytes);
                    }
                }
            }
            Update::Sys(sys) => {
                self.history.cpu.record(now, sys.cpu_total);
                self.history.net_rx.record(now, sys.net.rx_bps as f32);
                self.history.net_tx.record(now, sys.net.tx_bps as f32);
                self.sys = Some(sys);
                self.offline.remove(&Source::Sys);
            }
            Update::Power(p) => self.power = Some(p),
            Update::Audio(a) => self.audio = a,
            Update::Media(m) => self.media = m,
            Update::Spectrum(bars) => self.spectrum = bars,
            Update::SourceOffline(src, reason) => {
                self.offline.insert(src, reason);
            }
            Update::SourceOnline(src) => {
                self.offline.remove(&src);
            }
            Update::Procs(rows) => self.procs = rows,
            Update::Jobs(jobs) => self.jobs = jobs,
            Update::AgentMeta(rows) => {
                for (pane_id, meta) in rows {
                    self.agent_meta.insert(pane_id, meta);
                }
                // drop meta for panes that are no longer agents
                let live: Vec<&str> = self.herdr.agents.iter().map(|a| a.pane_id.as_str()).collect();
                if !live.is_empty() {
                    self.agent_meta.retain(|k, _| live.contains(&k.as_str()));
                }
            }
            Update::SysExtra(x) => self.sysx = Some(x),
            Update::PowerExtra(x) => self.powerx = Some(x),
            Update::AgentUsage(rows) => {
                for (pane_id, u) in rows {
                    self.usage.insert(pane_id, u);
                }
                let live: Vec<&str> = self.herdr.agents.iter().map(|a| a.pane_id.as_str()).collect();
                if !live.is_empty() {
                    self.usage.retain(|k, _| live.contains(&k.as_str()));
                }
            }
            Update::PlanLimits(l) => self.limits = l,
        }
    }

    /// Advance time-based state; call once per rendered frame.
    pub fn tick(&mut self, now: SystemTime) {
        self.now = now;
        let working = self.counts().working as f32;
        self.history.working.record(now, working);
    }

    fn apply_snapshot(&mut self, mut agents: Vec<Agent>, workspaces: Vec<Workspace>, now: SystemTime) {
        let mut transitions = Vec::new();
        for agent in agents.iter_mut() {
            if let Some(prev) = self.herdr.agents.iter().find(|p| p.pane_id == agent.pane_id) {
                if agent.rss_bytes.is_none() {
                    agent.rss_bytes = prev.rss_bytes;
                }
                if prev.status == agent.status {
                    agent.since = prev.since;
                } else {
                    agent.since = now;
                    transitions.push((agent.display_name(), prev.status, agent.status));
                }
            }
        }
        self.herdr.agents = agents;
        self.herdr.workspaces = workspaces;
        for (name, from, to) in transitions {
            self.record_transition(name, from, to, now);
        }
    }

    fn record_transition(&mut self, agent: String, from: AgentStatus, to: AgentStatus, now: SystemTime) {
        self.transitions += 1;
        if to == AgentStatus::Blocked {
            self.last_blocked_at = Some(now);
        }
        self.ticker.push_front(TickerEvent { at: now, agent, from, to });
        self.ticker.truncate(TICKER_CAP);
    }

    /// Blocked first, then working, done, idle, unknown; oldest state first within a group.
    pub fn sorted_agents(&self) -> Vec<&Agent> {
        let mut v: Vec<&Agent> = self.herdr.agents.iter().collect();
        v.sort_by(|a, b| a.status.cmp(&b.status).then(a.since.cmp(&b.since)).then(a.pane_id.cmp(&b.pane_id)));
        v
    }

    pub fn counts(&self) -> AgentCounts {
        count(self.herdr.agents.iter())
    }

    pub fn magi(&self, t: &Thresholds) -> [MagiVote; 3] {
        let compute = match &self.sys {
            Some(s) => {
                let temp_ok = s.temp_cpu_c.map_or(true, |c| c < t.cpu_temp_max_c);
                let temp = s.temp_cpu_c.map_or("--".to_string(), |c| format!("{c:.0}°C"));
                MagiVote {
                    unit: "MELCHIOR·1",
                    subject: "COMPUTE",
                    approve: s.cpu_total < t.cpu_max && temp_ok,
                    reason: format!("CPU {:.0}% · {temp}", s.cpu_total * 100.0),
                }
            }
            None => MagiVote { unit: "MELCHIOR·1", subject: "COMPUTE", approve: false, reason: "NO DATA".into() },
        };
        let memory = match &self.sys {
            Some(s) => MagiVote {
                unit: "BALTHASAR·2",
                subject: "MEMORY",
                approve: s.mem_available >= t.mem_min_available_bytes,
                reason: format!("FREE {:.1}G", s.mem_available as f64 / 1e9),
            },
            None => MagiVote { unit: "BALTHASAR·2", subject: "MEMORY", approve: false, reason: "NO DATA".into() },
        };
        let c = self.counts();
        let linked = self.herdr.link == HerdrLink::Linked;
        let pilots = MagiVote {
            unit: "CASPER·3",
            subject: "PILOTS",
            approve: linked && c.blocked == 0,
            reason: if !linked {
                "LINK LOST".into()
            } else if c.blocked > 0 {
                format!("{} BLOCKED", c.blocked)
            } else {
                format!("{} ACTIVE", c.working)
            },
        };
        [compute, memory, pilots]
    }

    pub fn alert_level(&self, t: &Thresholds) -> AlertLevel {
        let approvals = self.magi(t).iter().filter(|v| v.approve).count();
        if self.counts().blocked > 0 || approvals <= 1 {
            AlertLevel::Emergency
        } else if approvals == 2 {
            AlertLevel::Caution
        } else {
            AlertLevel::Nominal
        }
    }

    /// One entry per workspace, in herdr's order.
    pub fn workspace_rollup(&self) -> Vec<WorkspaceRollup> {
        self.herdr
            .workspaces
            .iter()
            .map(|w| WorkspaceRollup {
                id: w.id.clone(),
                label: w.label.clone(),
                agents: count(self.herdr.agents.iter().filter(|a| a.workspace_id == w.id)),
            })
            .collect()
    }

    /// Seconds since the agent entered its current state.
    pub fn age(&self, agent: &Agent) -> Duration {
        self.now.duration_since(agent.since).unwrap_or_default()
    }
}

fn count<'a>(agents: impl Iterator<Item = &'a Agent>) -> AgentCounts {
    let mut c = AgentCounts::default();
    for a in agents {
        match a.status {
            AgentStatus::Blocked => c.blocked += 1,
            AgentStatus::Working => c.working += 1,
            AgentStatus::Done => c.done += 1,
            AgentStatus::Idle => c.idle += 1,
            AgentStatus::Unknown => c.unknown += 1,
        }
    }
    c
}

/// Apply every pending update without blocking. Returns how many were applied.
pub fn drain_into(rx: &Receiver<Update>, state: &mut State) -> usize {
    let mut n = 0;
    while let Ok(u) = rx.try_recv() {
        state.apply(u, SystemTime::now());
        n += 1;
    }
    n
}

/// "1h02m", "41m", "09s" for time-in-state columns.
pub fn fmt_age(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m", s / 60)
    } else {
        format!("{s:02}s")
    }
}

/// "412M", "1.2G"
pub fn fmt_bytes(b: u64) -> String {
    if b >= 1_000_000_000 {
        format!("{:.1}G", b as f64 / 1e9)
    } else {
        format!("{}M", b / 1_000_000)
    }
}

/// "1.2M", "80K", "512B" per second
pub fn fmt_rate(bps: f64) -> String {
    if bps >= 1e6 {
        format!("{:.1}M", bps / 1e6)
    } else if bps >= 1e3 {
        format!("{:.0}K", bps / 1e3)
    } else {
        format!("{bps:.0}B")
    }
}

/// Why the views take over with the EMERGENCY treatment (TV view, hub red bar). The MAGI tally
/// alone (`alert_level`) is styling, never a takeover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EmergencyCause {
    Blocked,
    MemoryCritical,
    Both,
}

impl State {
    /// Blocked when any agent is blocked; MemoryCritical when sys data is present and
    /// MemAvailable is below `t.mem_critical_bytes`; Both when both; else None.
    pub fn emergency(&self, t: &Thresholds) -> Option<EmergencyCause> {
        let blocked = self.herdr.agents.iter().any(|a| a.status == AgentStatus::Blocked);
        let memory = self.sys.as_ref().is_some_and(|s| s.mem_available < t.mem_critical_bytes);
        match (blocked, memory) {
            (true, true) => Some(EmergencyCause::Both),
            (true, false) => Some(EmergencyCause::Blocked),
            (false, true) => Some(EmergencyCause::MemoryCritical),
            (false, false) => None,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn t0() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)
    }

    pub fn agent(pane: &str, name: &str, status: AgentStatus, ws: &str) -> Agent {
        Agent {
            pane_id: pane.into(),
            name: Some(name.into()),
            kind: "claude".into(),
            status,
            since: t0(),
            workspace_id: ws.into(),
            workspace_label: ws.into(),
            tab_label: name.into(),
            title: String::new(),
            rss_bytes: None,
        }
    }

    fn sys(cpu: f32, temp: f32, avail_gb: f64) -> SysState {
        SysState {
            cpu_total: cpu,
            temp_cpu_c: Some(temp),
            mem_total: 15_000_000_000,
            mem_available: (avail_gb * 1e9) as u64,
            ..Default::default()
        }
    }

    fn linked_state(agents: Vec<Agent>) -> State {
        let mut s = State::new(t0());
        s.apply(Update::HerdrLink(HerdrLink::Linked), t0());
        s.apply(Update::HerdrSnapshot { agents, workspaces: vec![] }, t0());
        s
    }

    #[test]
    fn sorts_blocked_then_working_then_done_then_idle() {
        let s = linked_state(vec![
            agent("w1:p1", "idle-one", AgentStatus::Idle, "w1"),
            agent("w1:p2", "done-one", AgentStatus::Done, "w1"),
            agent("w1:p3", "blocked-one", AgentStatus::Blocked, "w1"),
            agent("w1:p4", "working-one", AgentStatus::Working, "w1"),
        ]);
        let names: Vec<String> = s.sorted_agents().iter().map(|a| a.display_name()).collect();
        assert_eq!(names, ["blocked-one", "working-one", "done-one", "idle-one"]);
    }

    #[test]
    fn status_event_records_transition_and_resets_since() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        let later = t0() + Duration::from_secs(30);
        s.apply(Update::AgentStatus { pane_id: "w1:p1".into(), status: AgentStatus::Blocked, title: None }, later);
        assert_eq!(s.herdr.agents[0].status, AgentStatus::Blocked);
        assert_eq!(s.herdr.agents[0].since, later);
        assert_eq!(s.ticker.len(), 1);
        assert_eq!(s.ticker[0].to, AgentStatus::Blocked);
        assert_eq!(s.last_blocked_at, Some(later));
        assert_eq!(s.transitions, 1);
    }

    #[test]
    fn same_status_event_is_not_a_transition() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        s.apply(
            Update::AgentStatus { pane_id: "w1:p1".into(), status: AgentStatus::Working, title: Some("new title".into()) },
            t0() + Duration::from_secs(5),
        );
        assert!(s.ticker.is_empty());
        assert_eq!(s.herdr.agents[0].since, t0());
        assert_eq!(s.herdr.agents[0].title, "new title");
    }

    #[test]
    fn snapshot_keeps_since_and_rss_for_unchanged_agents() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        s.apply(Update::AgentRss(vec![("w1:p1".into(), 400_000_000)]), t0());
        let later = t0() + Duration::from_secs(60);
        let mut fresh = agent("w1:p1", "a", AgentStatus::Working, "w1");
        fresh.since = later;
        s.apply(Update::HerdrSnapshot { agents: vec![fresh], workspaces: vec![] }, later);
        assert_eq!(s.herdr.agents[0].since, t0());
        assert_eq!(s.herdr.agents[0].rss_bytes, Some(400_000_000));
        assert!(s.ticker.is_empty());
    }

    #[test]
    fn snapshot_with_changed_status_is_a_transition() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        let later = t0() + Duration::from_secs(60);
        let mut fresh = agent("w1:p1", "a", AgentStatus::Done, "w1");
        fresh.since = later;
        s.apply(Update::HerdrSnapshot { agents: vec![fresh], workspaces: vec![] }, later);
        assert_eq!(s.herdr.agents[0].since, later);
        assert_eq!(s.ticker[0].from, AgentStatus::Working);
        assert_eq!(s.ticker[0].to, AgentStatus::Done);
    }

    #[test]
    fn ticker_is_capped() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        for i in 0..(TICKER_CAP + 5) {
            let status = if i % 2 == 0 { AgentStatus::Done } else { AgentStatus::Working };
            s.apply(Update::AgentStatus { pane_id: "w1:p1".into(), status, title: None }, t0());
        }
        assert_eq!(s.ticker.len(), TICKER_CAP);
    }

    #[test]
    fn magi_all_approve_is_nominal() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(sys(0.30, 55.0, 4.0)), t0());
        let t = Thresholds::default();
        assert!(s.magi(&t).iter().all(|v| v.approve));
        assert_eq!(s.alert_level(&t), AlertLevel::Nominal);
    }

    #[test]
    fn low_memory_is_caution() {
        let mut s = linked_state(vec![]);
        s.apply(Update::Sys(sys(0.30, 55.0, 1.0)), t0());
        let t = Thresholds::default();
        let votes = s.magi(&t);
        assert!(!votes[1].approve);
        assert_eq!(s.alert_level(&t), AlertLevel::Caution);
    }

    #[test]
    fn any_blocked_agent_is_emergency_even_with_two_approvals() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Blocked, "w1")]);
        s.apply(Update::Sys(sys(0.30, 55.0, 4.0)), t0());
        let t = Thresholds::default();
        assert!(!s.magi(&t)[2].approve);
        assert_eq!(s.alert_level(&t), AlertLevel::Emergency);
    }

    #[test]
    fn hot_cpu_denies_compute() {
        let mut s = linked_state(vec![]);
        s.apply(Update::Sys(sys(0.30, 95.0, 4.0)), t0());
        assert!(!s.magi(&Thresholds::default())[0].approve);
    }

    #[test]
    fn link_lost_denies_pilots() {
        let mut s = linked_state(vec![]);
        s.apply(Update::HerdrLink(HerdrLink::Lost { since: t0(), reason: "socket".into() }), t0());
        let v = &s.magi(&Thresholds::default())[2];
        assert!(!v.approve);
        assert_eq!(v.reason, "LINK LOST");
    }

    #[test]
    fn no_sys_data_denies_compute_and_memory() {
        let s = linked_state(vec![]);
        let v = s.magi(&Thresholds::default());
        assert!(!v[0].approve && !v[1].approve);
        assert_eq!(s.alert_level(&Thresholds::default()), AlertLevel::Emergency);
    }

    #[test]
    fn workspace_rollup_counts_per_workspace() {
        let mut s = State::new(t0());
        let workspaces = vec![
            Workspace { id: "w1".into(), label: "hub".into(), number: 1 },
            Workspace { id: "w2".into(), label: "search".into(), number: 2 },
        ];
        s.apply(
            Update::HerdrSnapshot {
                agents: vec![
                    agent("w2:p1", "a", AgentStatus::Working, "w2"),
                    agent("w2:p2", "b", AgentStatus::Blocked, "w2"),
                ],
                workspaces,
            },
            t0(),
        );
        let r = s.workspace_rollup();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].agents, AgentCounts::default());
        assert_eq!(r[1].agents.working, 1);
        assert_eq!(r[1].agents.blocked, 1);
    }

    #[test]
    fn media_position_interpolates_while_playing_and_clamps() {
        let m = MediaState {
            player: "spotify".into(),
            status: PlayStatus::Playing,
            title: "t".into(),
            artist: "a".into(),
            album: String::new(),
            position_s: 10.0,
            length_s: Some(12.0),
            sampled_at: t0(),
        };
        assert_eq!(m.position_at(t0() + Duration::from_secs(1)), 11.0);
        assert_eq!(m.position_at(t0() + Duration::from_secs(5)), 12.0);
        let paused = MediaState { status: PlayStatus::Paused, ..m };
        assert_eq!(paused.position_at(t0() + Duration::from_secs(5)), 10.0);
    }

    #[test]
    fn profile_maps_to_lamps() {
        assert_eq!(Profile::parse("balanced").mode(), PowerMode::Normal);
        assert_eq!(Profile::parse("low-power\n").mode(), PowerMode::Slow);
        assert_eq!(Profile::parse("performance").mode(), PowerMode::Racing);
        assert_eq!(Profile::parse("weird").mode(), PowerMode::Stop);
    }

    #[test]
    fn model_short_names() {
        assert_eq!(model_short("claude-opus-5-5"), "OPUS 5.5");
        assert_eq!(model_short("claude-sonnet-5-5[1m]"), "SONNET 5.5 1M");
        assert_eq!(model_short("claude-haiku-4-5-20251001"), "HAIKU 4.5");
        assert_eq!(model_short("claude-fable-5-1"), "FABLE 5.1");
        assert_eq!(model_short("gpt-6-luna"), "GPT-6 LUNA");
        assert_eq!(model_short("gpt-5.2"), "GPT-5.2");
    }

    #[test]
    fn usage_ratios_and_type_label() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        let u = AgentUsage {
            model: Some("claude-opus-5-5".into()),
            effort: Some("xhigh".into()),
            context_used: Some(250_000),
            context_window: Some(1_000_000),
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 30,
            cache_write_tokens: 40,
            ..Default::default()
        };
        s.apply(Update::AgentUsage(vec![("w1:p1".into(), u), ("w9:p9".into(), AgentUsage::default())]), t0());
        let a = &s.herdr.agents[0];
        let got = s.usage_of(a).unwrap();
        assert_eq!(got.context_pct(), Some(25.0));
        assert_eq!(got.sync_ratio(), Some(75.0));
        assert_eq!(got.total_tokens(), 100);
        assert!(!s.usage.contains_key("w9:p9"));
        assert_eq!(s.type_label(a), "CLAUDE · OPUS 5.5 · XHIGH");
        assert_eq!(AgentUsage::default().context_pct(), None);
        s.apply(Update::PlanLimits(vec![PlanLimits { provider: "codex".into(), plan: Some("pro".into()), ..Default::default() }]), t0());
        assert_eq!(s.limits_for("codex").unwrap().plan.as_deref(), Some("pro"));
        assert!(s.limits_for("claude").is_none());
    }

    #[test]
    fn density_updates_are_stored_and_meta_is_pruned() {
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        s.apply(
            Update::AgentMeta(vec![
                ("w1:p1".into(), AgentMeta { cwd: "~/x".into(), branch: Some("main".into()), session_id: None }),
                ("w9:p9".into(), AgentMeta::default()),
            ]),
            t0(),
        );
        assert_eq!(s.meta(&s.herdr.agents[0]).unwrap().branch.as_deref(), Some("main"));
        assert!(!s.agent_meta.contains_key("w9:p9"));
        s.apply(Update::Procs(vec![ProcRow { pid: 1, name: "claude".into(), cpu: 12.5, rss_bytes: 1, threads: 9, state: 'S' }]), t0());
        s.apply(Update::Jobs(vec![]), t0());
        s.apply(Update::SysExtra(SysExtra { hostname: "nerv-hq".into(), ..Default::default() }), t0());
        s.apply(Update::PowerExtra(PowerExtra { cycles: Some(42), ..Default::default() }), t0());
        assert_eq!(s.procs.len(), 1);
        assert_eq!(s.sysx.as_ref().unwrap().hostname, "nerv-hq");
        assert_eq!(s.powerx.as_ref().unwrap().cycles, Some(42));
    }

    #[test]
    fn formatters() {
        assert_eq!(fmt_age(Duration::from_secs(9)), "09s");
        assert_eq!(fmt_age(Duration::from_secs(41 * 60 + 3)), "41m");
        assert_eq!(fmt_age(Duration::from_secs(3720)), "1h02m");
        assert_eq!(fmt_bytes(412_000_000), "412M");
        assert_eq!(fmt_bytes(1_250_000_000), "1.2G");
        assert_eq!(fmt_rate(1_234_567.0), "1.2M");
        assert_eq!(fmt_rate(80_000.0), "80K");
    }

    #[test]
    fn emergency_blocked_memory_both_or_none() {
        let t = Thresholds::default(); // mem_critical_bytes 800 MB
        let mut s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(sys(0.30, 55.0, 4.0)), t0());
        assert_eq!(s.emergency(&t), None);

        let mut blocked = linked_state(vec![agent("w1:p1", "a", AgentStatus::Blocked, "w1")]);
        blocked.apply(Update::Sys(sys(0.30, 55.0, 4.0)), t0());
        assert_eq!(blocked.emergency(&t), Some(EmergencyCause::Blocked));

        s.apply(Update::Sys(sys(0.30, 55.0, 0.5)), t0());
        assert_eq!(s.emergency(&t), Some(EmergencyCause::MemoryCritical));

        blocked.apply(Update::Sys(sys(0.30, 55.0, 0.5)), t0());
        assert_eq!(blocked.emergency(&t), Some(EmergencyCause::Both));
    }

    #[test]
    fn emergency_needs_data_and_ignores_magi_tally() {
        let t = Thresholds::default();
        // no sys data yet: MAGI denies two votes (alert_level Emergency) but that is styling only
        let s = linked_state(vec![agent("w1:p1", "a", AgentStatus::Working, "w1")]);
        assert_eq!(s.alert_level(&t), AlertLevel::Emergency);
        assert_eq!(s.emergency(&t), None);
        // hot CPU + low (not critical) memory: 1/3 approvals, still no takeover
        let mut hot = linked_state(vec![]);
        hot.apply(Update::Sys(sys(0.95, 95.0, 1.0)), t0());
        assert_eq!(hot.alert_level(&t), AlertLevel::Emergency);
        assert_eq!(hot.emergency(&t), None);
        // blocked with no sys data is still Blocked
        let b = linked_state(vec![agent("w1:p1", "a", AgentStatus::Blocked, "w1")]);
        assert_eq!(b.emergency(&t), Some(EmergencyCause::Blocked));
    }
}
