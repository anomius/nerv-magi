//! Per-agent usage from the agents' own session logs, every 5 s and incrementally: Claude
//! transcripts (`~/.claude/projects/*/<sid>.jsonl`), Codex rollouts
//! (`~/.codex/sessions/YYYY/MM/DD/rollout-*-<sid>.jsonl`) and, when the user has set it up, the
//! Claude statusline cache (`$XDG_RUNTIME_DIR/nerv/claude-status/<sid>.json`).
//!
//! Read-only: this module only opens those files for reading. It never creates or edits a
//! statusline script, a log, or any Claude/Codex config.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::herdr;
use crate::state::{AgentUsage, LimitWindow, PlanLimits, Update};

const PERIOD: Duration = Duration::from_secs(5);
/// A log that was not found is looked for again after this long.
const RESEARCH: Duration = Duration::from_secs(60);
/// Codex day directories searched, newest first.
const CODEX_DAYS: usize = 30;
/// Line buffers grown past this by one huge line (rollouts carry multi-MB lines) are dropped
/// instead of kept, so a long-running view does not hold on to them.
const KEEP_BUF: usize = 64 * 1024;
const CLAUDE_WINDOW: u64 = 200_000;
const CLAUDE_WINDOW_1M: u64 = 1_000_000;

// ── time ─────────────────────────────────────────────────────────────────────────────────────

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// `2026-10-13T08:17:26.984Z` / `…+05:30` → SystemTime.
pub fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |from: usize, to: usize| s.get(from..to)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut rest = s.get(19..)?;
    let mut nanos = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits = frac.bytes().take_while(u8::is_ascii_digit).count();
        let f = frac.get(..digits.min(9)).filter(|f| !f.is_empty())?;
        nanos = f.parse::<u32>().ok()? * 10u32.pow(9 - f.len() as u32);
        rest = &frac[digits..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ if rest.len() == 6 && rest.as_bytes()[3] == b':' => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            sign * (rest.get(1..3)?.parse::<i64>().ok()? * 3600 + rest.get(4..6)?.parse::<i64>().ok()? * 60)
        }
        _ => return None,
    };
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - offset;
    (secs >= 0).then(|| UNIX_EPOCH + Duration::new(secs as u64, nanos))
}

/// Unix seconds (number or numeric string) or an RFC 3339 string.
fn reset_time(v: &Value) -> Option<SystemTime> {
    // Logs are external data: "inf", NaN or absurd values are dropped, never a panic.
    let secs = |s: f64| {
        if !s.is_finite() || !(0.0..=1e11).contains(&s) {
            return None;
        }
        UNIX_EPOCH.checked_add(Duration::try_from_secs_f64(s).ok()?)
    };
    match v {
        Value::Number(n) => secs(n.as_f64()?),
        Value::String(s) => s.parse::<f64>().ok().and_then(secs).or_else(|| parse_rfc3339(s)),
        _ => None,
    }
}

/// The first `"timestamp":"…"` in a raw line, without parsing the rest of it.
fn raw_timestamp(line: &str) -> Option<SystemTime> {
    let start = line.find("\"timestamp\":\"")? + 13;
    let end = start + line[start..].find('"')?;
    parse_rfc3339(&line[start..end])
}

/// The first `n` bytes of `line` (on a char boundary): where both logs put a line's `type`.
fn head(line: &str, n: usize) -> &str {
    let mut end = line.len().min(n);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

fn span(started: &mut Option<SystemTime>, last: &mut Option<SystemTime>, t: Option<SystemTime>) {
    if let Some(t) = t {
        *started = Some(started.map_or(t, |s| s.min(t)));
        *last = Some(last.map_or(t, |l| l.max(t)));
    }
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

// ── logs ─────────────────────────────────────────────────────────────────────────────────────

/// Something that consumes a session log line by line.
pub trait Feed {
    fn feed(&mut self, line: &str);
}

#[derive(Debug, Default, Clone, Copy)]
struct Tokens {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    reasoning: u64,
}

/// A Claude Code transcript. Several lines can carry the same assistant message (streamed
/// content blocks); each `message.id` counts once, with its last usage.
#[derive(Debug, Default)]
pub struct ClaudeLog {
    messages: HashMap<String, Tokens>,
    latest: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    version: Option<String>,
    mode: Option<String>,
    started: Option<SystemTime>,
    last: Option<SystemTime>,
}

impl Feed for ClaudeLog {
    fn feed(&mut self, line: &str) {
        if !line.contains("\"assistant\"") && !line.contains("\"permission-mode\"") {
            span(&mut self.started, &mut self.last, raw_timestamp(line));
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        span(&mut self.started, &mut self.last, v["timestamp"].as_str().and_then(parse_rfc3339));
        match v["type"].as_str() {
            Some("permission-mode") => self.mode = text(&v["permissionMode"]).or(self.mode.take()),
            Some("assistant") => {
                let msg = &v["message"];
                if msg["model"].as_str() == Some("<synthetic>") {
                    return; // locally generated, no API call behind it
                }
                self.model = text(&msg["model"]).or(self.model.take());
                self.effort = text(&v["effort"]).or(self.effort.take());
                self.version = text(&v["version"]).or(self.version.take());
                let (Some(id), Some(u)) = (msg["id"].as_str(), msg.get("usage")) else { return };
                let n = |k: &str| u[k].as_u64().unwrap_or(0);
                let tokens = Tokens {
                    input: n("input_tokens"),
                    output: n("output_tokens"),
                    cache_read: n("cache_read_input_tokens"),
                    cache_write: n("cache_creation_input_tokens"),
                    reasoning: u["output_tokens_details"]["thinking_tokens"].as_u64().unwrap_or(0),
                };
                self.messages.insert(id.to_string(), tokens);
                self.latest = Some(id.to_string());
            }
            _ => {}
        }
    }
}

impl ClaudeLog {
    /// Without the statusline cache the window is estimated: 1M for `[1m]` models or once the
    /// context is past 200k, else 200k.
    pub fn usage(&self) -> AgentUsage {
        let sum = self.messages.values().fold(Tokens::default(), |a, t| Tokens {
            input: a.input + t.input,
            output: a.output + t.output,
            cache_read: a.cache_read + t.cache_read,
            cache_write: a.cache_write + t.cache_write,
            reasoning: a.reasoning + t.reasoning,
        });
        let context_used =
            self.latest.as_ref().and_then(|id| self.messages.get(id)).map(|t| t.input + t.cache_read + t.cache_write);
        let one_m = self.model.as_deref().is_some_and(|m| m.ends_with("[1m]"))
            || context_used.is_some_and(|c| c > CLAUDE_WINDOW);
        AgentUsage {
            model: self.model.clone(),
            effort: self.effort.clone(),
            mode: self.mode.clone(),
            version: self.version.clone(),
            context_used,
            context_window: Some(if one_m { CLAUDE_WINDOW_1M } else { CLAUDE_WINDOW }),
            context_window_estimated: true,
            input_tokens: sum.input,
            output_tokens: sum.output,
            cache_read_tokens: sum.cache_read,
            cache_write_tokens: sum.cache_write,
            reasoning_tokens: sum.reasoning,
            turns: self.messages.len() as u32,
            session_started: self.started,
            last_activity: self.last,
            cost_usd: None,
        }
    }
}

/// A Codex rollout. `token_count` totals are cumulative, so the latest one wins.
#[derive(Debug, Default)]
pub struct CodexLog {
    usage: AgentUsage,
    rate_limits: Option<(Value, Option<SystemTime>)>,
}

impl Feed for CodexLog {
    fn feed(&mut self, line: &str) {
        let ts = raw_timestamp(line);
        let u = &mut self.usage;
        if let Some(t) = ts {
            u.last_activity = Some(u.last_activity.map_or(t, |l| l.max(t)));
        }
        let h = head(line, 200);
        if !(h.contains("\"session_meta\"") || h.contains("\"turn_context\"") || h.contains("\"token_count\"")) {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let p = &v["payload"];
        match v["type"].as_str() {
            Some("session_meta") => {
                u.version = text(&p["cli_version"]).or(u.version.take());
                u.session_started = p["timestamp"].as_str().and_then(parse_rfc3339).or(ts);
            }
            Some("turn_context") => {
                u.model = text(&p["model"]).or(u.model.take());
                u.effort = text(&p["effort"]).or(u.effort.take());
                u.mode = text(&p["approval_policy"]).or(u.mode.take());
                u.turns += 1;
            }
            Some("event_msg") if p["type"] == "token_count" => {
                let info = &p["info"];
                let total = &info["total_token_usage"];
                if total.is_object() {
                    let n = |k: &str| total[k].as_u64().unwrap_or(0);
                    u.input_tokens = n("input_tokens").saturating_sub(n("cached_input_tokens"));
                    u.cache_read_tokens = n("cached_input_tokens");
                    u.output_tokens = n("output_tokens");
                    u.reasoning_tokens = n("reasoning_output_tokens");
                }
                u.context_used = info["last_token_usage"]["input_tokens"].as_u64().or(u.context_used);
                u.context_window = info["model_context_window"].as_u64().or(u.context_window);
                if p["rate_limits"].is_object() {
                    self.rate_limits = Some((p["rate_limits"].clone(), ts));
                }
            }
            _ => {}
        }
    }
}

impl CodexLog {
    pub fn usage(&self) -> AgentUsage {
        self.usage.clone()
    }

    pub fn limits(&self) -> Option<PlanLimits> {
        self.rate_limits.as_ref().map(|(rl, at)| codex_limits(rl, *at))
    }
}

// ── plan limits ──────────────────────────────────────────────────────────────────────────────

/// 300 → "5H", 10080 → "WEEK", else "<n>M".
pub fn window_label(minutes: u32) -> String {
    match minutes {
        300 => "5H".into(),
        10_080 => "WEEK".into(),
        m => format!("{m}M"),
    }
}

/// "62500.0000000000" → "62500".
pub fn trim_credits(balance: &str) -> String {
    if balance.contains('.') {
        balance.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        balance.to_string()
    }
}

/// A Codex `rate_limits` object → plan limits (primary/secondary windows, credits when finite).
pub fn codex_limits(rl: &Value, observed_at: Option<SystemTime>) -> PlanLimits {
    let windows = ["primary", "secondary"]
        .iter()
        .filter_map(|k| {
            let w = rl.get(*k).filter(|w| w.is_object())?;
            let minutes = w["window_minutes"].as_u64().map(|m| m as u32);
            Some(LimitWindow {
                label: minutes.map_or_else(|| k.to_uppercase(), window_label),
                used_pct: w["used_percent"].as_f64()? as f32,
                resets_at: reset_time(&w["resets_at"]),
                window_minutes: minutes,
            })
        })
        .collect();
    let c = &rl["credits"];
    let finite = c["has_credits"].as_bool() == Some(true) && c["unlimited"].as_bool() != Some(true);
    let balance = c["balance"].as_str().map(str::to_string).or_else(|| c["balance"].as_f64().map(|b| b.to_string()));
    PlanLimits {
        provider: "codex".into(),
        plan: text(&rl["plan_type"]),
        windows,
        credits: balance.filter(|_| finite).map(|b| trim_credits(&b)),
        observed_at,
    }
}

/// Statusline cache → exact context window and Claude Code's own cost estimate.
pub fn apply_status(u: &mut AgentUsage, status: &Value) {
    if let Some(w) = status["context_window"]["context_window_size"].as_u64() {
        u.context_window = Some(w);
        u.context_window_estimated = false;
    }
    if let Some(c) = status["cost"]["total_cost_usd"].as_f64() {
        u.cost_usd = Some(c);
    }
}

/// Statusline cache `rate_limits.five_hour` / `seven_day` → Claude plan limits.
pub fn claude_limits(status: &Value, observed_at: SystemTime) -> Option<PlanLimits> {
    let rl = &status["rate_limits"];
    let windows: Vec<LimitWindow> = [("five_hour", "5H", 300), ("seven_day", "7D", 10_080)]
        .iter()
        .filter_map(|(key, label, minutes)| {
            let w = &rl[*key];
            Some(LimitWindow {
                label: label.to_string(),
                used_pct: w["used_percentage"].as_f64()? as f32,
                resets_at: reset_time(&w["resets_at"]),
                window_minutes: Some(*minutes),
            })
        })
        .collect();
    (!windows.is_empty()).then(|| PlanLimits {
        provider: "claude".into(),
        plan: None,
        windows,
        credits: None,
        observed_at: Some(observed_at),
    })
}

// ── files ────────────────────────────────────────────────────────────────────────────────────

/// Follows one append-only JSONL file by byte offset.
pub struct Tail {
    path: PathBuf,
    offset: u64,
    /// An incomplete last line, waiting for its newline.
    partial: Vec<u8>,
}

impl Tail {
    pub fn new(path: PathBuf) -> Self {
        Self { path, offset: 0, partial: Vec::new() }
    }

    /// Feeds every complete line appended since the last call. A file that shrank was rewritten:
    /// `log` is reset and the file re-read from the start.
    pub fn read_new<L: Feed + Default>(&mut self, log: &mut L) -> io::Result<()> {
        let mut f = File::open(&self.path)?;
        let len = f.metadata()?.len();
        if len < self.offset {
            *log = L::default();
            self.offset = 0;
            self.partial.clear();
        }
        if len == self.offset {
            return Ok(());
        }
        f.seek(SeekFrom::Start(self.offset))?;
        let mut r = BufReader::with_capacity(64 * 1024, f.take(len - self.offset));
        let mut buf = std::mem::take(&mut self.partial);
        loop {
            let n = r.read_until(b'\n', &mut buf)?;
            if n == 0 {
                break;
            }
            self.offset += n as u64;
            if buf.last() == Some(&b'\n') {
                let line = String::from_utf8_lossy(&buf[..buf.len() - 1]);
                log.feed(line.trim_end_matches('\r'));
                if buf.capacity() > KEEP_BUF {
                    buf = Vec::new();
                } else {
                    buf.clear();
                }
            }
        }
        self.partial = buf;
        Ok(())
    }
}

/// Where the logs live.
pub struct Roots {
    pub claude_projects: PathBuf,
    pub codex_sessions: PathBuf,
    pub status_dir: Option<PathBuf>,
}

impl Roots {
    /// `$CLAUDE_CONFIG_DIR` / `$CODEX_HOME` when set, else `~/.claude` / `~/.codex`.
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let dir =
            |var: &str, fallback: &str| std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| home.join(fallback));
        Self {
            claude_projects: dir("CLAUDE_CONFIG_DIR", ".claude").join("projects"),
            codex_sessions: dir("CODEX_HOME", ".codex").join("sessions"),
            status_dir: std::env::var_os("XDG_RUNTIME_DIR").map(|d| PathBuf::from(d).join("nerv/claude-status")),
        }
    }
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// `<projects>/*/<sid>.jsonl`.
pub fn find_claude(projects: &Path, sid: &str) -> Option<PathBuf> {
    sorted_dirs(projects).into_iter().map(|d| d.join(format!("{sid}.jsonl"))).find(|p| p.is_file())
}

/// `<sessions>/YYYY/MM/DD/rollout-*-<sid>.jsonl`, newest day directories first, at most `max_days`.
pub fn find_codex(sessions: &Path, sid: &str, max_days: usize) -> Option<PathBuf> {
    let suffix = format!("-{sid}.jsonl");
    let newest_first = |d: &Path| {
        let mut v = sorted_dirs(d);
        v.reverse();
        v
    };
    newest_first(sessions).iter().flat_map(|y| newest_first(y)).flat_map(|m| newest_first(&m)).take(max_days).find_map(
        |day| {
            fs::read_dir(&day).into_iter().flatten().flatten().map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("rollout-") && n.ends_with(&suffix))
            })
        },
    )
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Claude limits from the freshest statusline cache file that has them.
fn freshest_claude_limits(dir: &Path) -> Option<PlanLimits> {
    let mut files: Vec<(SystemTime, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort();
    files.iter().rev().find_map(|(mtime, p)| claude_limits(&read_json(p)?, *mtime))
}

/// `--permission-mode <m>` or `--permission-mode=<m>`.
pub fn mode_from_argv(argv: &[String]) -> Option<String> {
    argv.iter().enumerate().find_map(|(i, a)| match a.strip_prefix("--permission-mode") {
        Some("") => argv.get(i + 1).cloned(),
        Some(rest) => rest.strip_prefix('=').map(str::to_string),
        None => None,
    })
}

// ── collector ────────────────────────────────────────────────────────────────────────────────

/// One agent pane and the session it is running.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub pane_id: String,
    pub kind: String,
    pub session: String,
}

/// `agent.list` result → agents that have a session id.
pub fn targets_from(agent_list: &Value) -> Vec<Target> {
    agent_list["agents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            Some(Target {
                pane_id: a["pane_id"].as_str()?.to_string(),
                kind: a["agent"].as_str()?.to_string(),
                session: text(&a["agent_session"]["value"])?,
            })
        })
        .collect()
}

enum Log {
    Claude(ClaudeLog),
    Codex(CodexLog),
}

impl Log {
    fn fresh(claude: bool) -> Self {
        if claude {
            Log::Claude(ClaudeLog::default())
        } else {
            Log::Codex(CodexLog::default())
        }
    }
}

struct Session {
    searched: Option<Instant>,
    tail: Option<Tail>,
    log: Log,
    /// Mode from the pane's argv, asked once per session (Claude, when the transcript has none).
    argv_mode: Option<Option<String>>,
}

pub struct Collector {
    roots: Roots,
    sessions: HashMap<String, Session>,
}

impl Collector {
    pub fn new(roots: Roots) -> Self {
        Self { roots, sessions: HashMap::new() }
    }

    /// One pass: read what each agent's log gained since the last pass. Agents whose log is not
    /// found get no row. Returns the rows and the plan limits (Claude first, then Codex).
    pub fn tick(
        &mut self,
        targets: &[Target],
        now: Instant,
        mut argv_mode: impl FnMut(&Target) -> Option<String>,
    ) -> (Vec<(String, AgentUsage)>, Vec<PlanLimits>) {
        self.sessions.retain(|sid, _| targets.iter().any(|t| &t.session == sid));
        let (mut rows, mut codex): (Vec<(String, AgentUsage)>, Option<PlanLimits>) = (Vec::new(), None);
        for t in targets {
            let claude = match t.kind.as_str() {
                "claude" => true,
                "codex" => false,
                _ => continue,
            };
            let s = self.sessions.entry(t.session.clone()).or_insert_with(|| Session {
                searched: None,
                tail: None,
                log: Log::fresh(claude),
                argv_mode: None,
            });
            if s.tail.is_none() && s.searched.is_none_or(|at| now.saturating_duration_since(at) >= RESEARCH) {
                let path = if claude {
                    find_claude(&self.roots.claude_projects, &t.session)
                } else {
                    find_codex(&self.roots.codex_sessions, &t.session, CODEX_DAYS)
                };
                s.tail = path.map(Tail::new);
                s.searched = Some(now);
            }
            let Some(tail) = s.tail.as_mut() else { continue };
            let read = match &mut s.log {
                Log::Claude(l) => tail.read_new(l),
                Log::Codex(l) => tail.read_new(l),
            };
            if read.is_err() {
                // the file went away: start over and look for it again later
                (s.tail, s.log) = (None, Log::fresh(claude));
                continue;
            }
            let mut u = match &s.log {
                Log::Claude(l) => l.usage(),
                Log::Codex(l) => {
                    if let Some(l) = l.limits() {
                        if codex.as_ref().is_none_or(|c| l.observed_at > c.observed_at) {
                            codex = Some(l);
                        }
                    }
                    l.usage()
                }
            };
            if claude {
                if u.mode.is_none() {
                    u.mode = s.argv_mode.get_or_insert_with(|| argv_mode(t)).clone();
                }
                if let Some(status) =
                    self.roots.status_dir.as_ref().and_then(|d| read_json(&d.join(format!("{}.json", t.session))))
                {
                    apply_status(&mut u, &status);
                }
            }
            rows.push((t.pane_id.clone(), u));
        }
        let claude_limits = self.roots.status_dir.as_deref().and_then(freshest_claude_limits);
        (rows, claude_limits.into_iter().chain(codex).collect())
    }
}

pub fn spawn(tx: Sender<Update>) {
    thread::spawn(move || {
        let socket = herdr::socket_path();
        let mut collector = Collector::new(Roots::from_env());
        loop {
            let started = Instant::now();
            if let Ok(list) = herdr::call(&socket, "agent.list", json!({})) {
                let (rows, limits) = collector.tick(&targets_from(&list), started, |t| {
                    let info = herdr::call(&socket, "pane.process_info", json!({"pane_id": t.pane_id})).ok()?;
                    let argv: Vec<String> = info["process_info"]["foreground_processes"]
                        .as_array()?
                        .iter()
                        .flat_map(|p| p["argv"].as_array().into_iter().flatten())
                        .filter_map(|a| a.as_str().map(str::to_string))
                        .collect();
                    mode_from_argv(&argv)
                });
                if tx.send(Update::AgentUsage(rows)).is_err() || tx.send(Update::PlanLimits(limits)).is_err() {
                    return;
                }
            }
            thread::sleep(PERIOD.saturating_sub(started.elapsed()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_time_rejects_non_finite_and_absurd_values() {
        use serde_json::json;
        assert_eq!(reset_time(&json!("inf")), None);
        assert_eq!(reset_time(&json!("NaN")), None);
        assert_eq!(reset_time(&json!(1e300)), None);
        assert_eq!(reset_time(&json!(-5)), None);
        assert_eq!(reset_time(&json!(1_791_926_703)), Some(UNIX_EPOCH + Duration::from_secs(1_791_926_703)));
        assert_eq!(reset_time(&json!("1791926703")), Some(UNIX_EPOCH + Duration::from_secs(1_791_926_703)));
    }
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const CLAUDE_SID: &str = "11111111-2222-4333-8444-555555555555";
    const CODEX_SID: &str = "01a00000-0000-7000-8000-000000000001";

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/usage/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn at(secs: u64) -> Option<SystemTime> {
        Some(UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn fed<L: Feed + Default>(text: &str) -> L {
        let mut log = L::default();
        text.lines().for_each(|l| log.feed(l));
        log
    }

    #[test]
    fn rfc3339_parses_utc_fraction_and_offsets() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(UNIX_EPOCH));
        assert_eq!(
            parse_rfc3339("2026-10-13T08:17:26.984Z"),
            Some(UNIX_EPOCH + Duration::from_millis(1_791_879_446_984))
        );
        assert_eq!(parse_rfc3339("2026-10-13T13:30:00+05:30"), at(1_791_878_400));
        assert_eq!(parse_rfc3339("2000-02-29T23:59:59Z"), at(951_868_799));
        assert_eq!(parse_rfc3339("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("yesterday"), None);
    }

    #[test]
    fn claude_counts_each_message_id_once_and_skips_synthetic() {
        let u = fed::<ClaudeLog>(&fixture("claude_session.jsonl")).usage();
        // msg_01 twice (last usage wins), msg_syn skipped, msg_02 once
        assert_eq!(u.turns, 2);
        assert_eq!((u.input_tokens, u.output_tokens), (14, 70));
        assert_eq!((u.cache_read_tokens, u.cache_write_tokens, u.reasoning_tokens), (1000, 1200, 30));
        assert_eq!(u.model.as_deref(), Some("claude-opus-5-5"));
    }

    #[test]
    fn claude_context_comes_from_the_latest_message() {
        let u = fed::<ClaudeLog>(&fixture("claude_session.jsonl")).usage();
        assert_eq!(u.context_used, Some(4 + 1000 + 200));
        assert_eq!(
            (u.effort.as_deref(), u.version.as_deref(), u.mode.as_deref()),
            (Some("xhigh"), Some("2.1.297"), Some("auto"))
        );
        assert_eq!((u.session_started, u.last_activity), (at(1_791_878_400), at(1_791_878_460)));
        assert_eq!((u.context_window, u.context_window_estimated), (Some(200_000), true));
    }

    fn assistant(id: &str, model: &str, input: u64, cache_read: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-10-13T08:00:00Z","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":{input},"cache_creation_input_tokens":0,"cache_read_input_tokens":{cache_read},"output_tokens":1}}}}}}"#
        )
    }

    #[test]
    fn claude_window_estimate_200k_or_1m() {
        let small = fed::<ClaudeLog>(&assistant("a", "claude-opus-5-5", 10, 150_000)).usage();
        assert_eq!(small.context_window, Some(200_000));
        let one_m = fed::<ClaudeLog>(&assistant("a", "claude-sonnet-5-5[1m]", 10, 1_000)).usage();
        assert_eq!(one_m.context_window, Some(1_000_000));
        let big = fed::<ClaudeLog>(&assistant("a", "claude-opus-5-5", 10, 250_000)).usage();
        assert_eq!((big.context_window, big.context_window_estimated), (Some(1_000_000), true));
    }

    #[test]
    fn statusline_cache_sets_window_cost_and_claude_limits() {
        let status: serde_json::Value = serde_json::from_str(&fixture("claude_status.json")).unwrap();
        let mut u = fed::<ClaudeLog>(&fixture("claude_session.jsonl")).usage();
        apply_status(&mut u, &status);
        assert_eq!((u.context_window, u.context_window_estimated, u.cost_usd), (Some(1_000_000), false, Some(3.2)));
        let l = claude_limits(&status, UNIX_EPOCH).unwrap();
        assert_eq!(l.provider, "claude");
        let w: Vec<(&str, f32, Option<SystemTime>)> =
            l.windows.iter().map(|w| (w.label.as_str(), w.used_pct, w.resets_at)).collect();
        assert_eq!(w, [("5H", 37.0, at(1_791_900_000)), ("7D", 12.5, at(1_792_454_400))]);
        assert_eq!(claude_limits(&serde_json::json!({"cost": {}}), UNIX_EPOCH), None);
    }

    #[test]
    fn codex_totals_context_and_plan_limits() {
        let log = fed::<CodexLog>(&fixture("codex_rollout.jsonl"));
        let u = log.usage();
        assert_eq!(
            (u.model.as_deref(), u.effort.as_deref(), u.mode.as_deref()),
            (Some("gpt-6-luna"), Some("high"), Some("on-request"))
        );
        assert_eq!((u.version.as_deref(), u.turns), (Some("0.162.1"), 2));
        assert_eq!(
            (u.input_tokens, u.cache_read_tokens, u.output_tokens, u.reasoning_tokens),
            (10_000, 30_000, 900, 400)
        );
        assert_eq!(
            (u.context_used, u.context_window, u.context_window_estimated),
            (Some(23_522), Some(258_400), false)
        );
        assert_eq!((u.session_started, u.last_activity), (at(1_791_878_400), at(1_791_878_730)));
        let l = log.limits().unwrap();
        assert_eq!(
            (l.provider.as_str(), l.plan.as_deref(), l.credits.as_deref()),
            ("codex", Some("pro"), Some("62500"))
        );
        let w: Vec<(&str, f32, Option<u32>)> =
            l.windows.iter().map(|w| (w.label.as_str(), w.used_pct, w.window_minutes)).collect();
        assert_eq!(w, [("WEEK", 17.0, Some(10080)), ("5H", 42.0, Some(300))]);
        assert_eq!(l.observed_at, at(1_791_878_730));
    }

    #[test]
    fn window_labels_and_credit_trimming() {
        assert_eq!(
            (window_label(300), window_label(10080), window_label(60)),
            ("5H".to_string(), "WEEK".to_string(), "60M".to_string())
        );
        assert_eq!(trim_credits("62500.0000000000"), "62500");
        assert_eq!(trim_credits("12.5000"), "12.5");
        let unlimited = serde_json::json!({"primary": null, "credits": {"has_credits": true, "unlimited": true, "balance": "0"}, "plan_type": "team"});
        assert_eq!(codex_limits(&unlimited, None).credits, None);
    }

    #[derive(Default)]
    struct Lines(Vec<String>);
    impl Feed for Lines {
        fn feed(&mut self, line: &str) {
            self.0.push(line.to_string());
        }
    }

    #[test]
    fn tail_reads_appends_incrementally_and_resets_on_truncation() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("log.jsonl");
        std::fs::write(&p, "a\nb\npart").unwrap();
        let (mut tail, mut got) = (Tail::new(p.clone()), Lines::default());
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0, ["a", "b"]); // the partial line waits for its newline
        std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"ial\nc\n").unwrap();
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0, ["a", "b", "partial", "c"]);
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0.len(), 4); // nothing new
        std::fs::write(&p, "z\n").unwrap(); // rewritten shorter
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0, ["z"]); // the accumulator was reset, then re-fed from 0
    }

    #[test]
    fn tail_does_not_keep_a_huge_line_buffer() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("log.jsonl");
        let huge = "x".repeat(3_000_000);
        std::fs::write(&p, format!("{huge}\nsmall\n{huge}")).unwrap(); // ends mid-line too
        let (mut tail, mut got) = (Tail::new(p), Lines::default());
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0.len(), 2);
        assert_eq!(tail.partial.len(), 3_000_000); // the unfinished line is kept…
        std::fs::OpenOptions::new().append(true).open(&tail.path).unwrap().write_all(b"\n").unwrap();
        tail.read_new(&mut got).unwrap();
        assert_eq!(got.0.len(), 3);
        assert!(tail.partial.capacity() <= 64 * 1024, "kept {} bytes", tail.partial.capacity());
        // …but not its buffer
    }

    #[test]
    fn finds_claude_transcript_and_newest_codex_rollout() {
        let d = tempfile::tempdir().unwrap();
        let put = |p: &str| {
            let path = d.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "{}\n").unwrap();
        };
        put(&format!("claude/-home-user-project/{CLAUDE_SID}.jsonl"));
        put(&format!("codex/2026/10/12/rollout-2026-10-12T09-00-00-{CODEX_SID}.jsonl"));
        put(&format!("codex/2026/09/01/rollout-2026-09-01T09-00-00-01a00000-0000-7000-8000-00000000000f.jsonl"));
        put("codex/2026/10/13/rollout-2026-10-13T09-00-00-other.jsonl");
        let claude = find_claude(&d.path().join("claude"), CLAUDE_SID).unwrap();
        assert!(claude.ends_with(format!("-home-user-project/{CLAUDE_SID}.jsonl")));
        assert!(find_codex(&d.path().join("codex"), CODEX_SID, 30).unwrap().to_string_lossy().contains("2026/10/12"));
        assert_eq!(find_codex(&d.path().join("codex"), "01a00000-0000-7000-8000-00000000000f", 2), None); // older than 2 day dirs
        assert_eq!(find_claude(&d.path().join("claude"), "nope"), None);
        assert_eq!(find_codex(&d.path().join("missing"), CODEX_SID, 30), None);
    }

    #[test]
    fn collector_rows_only_for_found_logs_and_follows_appends() {
        let d = tempfile::tempdir().unwrap();
        let roots = Roots {
            claude_projects: d.path().join("claude"),
            codex_sessions: d.path().join("codex"),
            status_dir: Some(d.path().join("status")),
        };
        let transcript = d.path().join(format!("claude/-proj/{CLAUDE_SID}.jsonl"));
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, fixture("claude_session.jsonl")).unwrap();
        let targets = [
            Target { pane_id: "w1:p1".into(), kind: "claude".into(), session: CLAUDE_SID.into() },
            Target { pane_id: "w1:p2".into(), kind: "codex".into(), session: CODEX_SID.into() },
        ];
        let mut c = Collector::new(roots);
        let t0 = Instant::now();
        let (rows, limits) = c.tick(&targets, t0, |_| None);
        assert_eq!(rows.len(), 1, "codex rollout missing → no row");
        assert_eq!((rows[0].0.as_str(), rows[0].1.turns), ("w1:p1", 2));
        assert!(limits.is_empty());

        // the rollout appears; it is only looked for again after 60 s
        let rollout = d.path().join(format!("codex/2026/10/13/rollout-2026-10-13T08-00-00-{CODEX_SID}.jsonl"));
        std::fs::create_dir_all(rollout.parent().unwrap()).unwrap();
        std::fs::write(&rollout, fixture("codex_rollout.jsonl")).unwrap();
        assert_eq!(c.tick(&targets, t0 + Duration::from_secs(5), |_| None).0.len(), 1);
        let (rows, limits) = c.tick(&targets, t0 + Duration::from_secs(61), |_| None);
        assert_eq!(rows.len(), 2);
        assert_eq!(limits.iter().map(|l| l.provider.as_str()).collect::<Vec<_>>(), ["codex"]);

        // an appended turn is picked up incrementally
        use std::io::Write;
        let more = assistant("msg_03", "claude-opus-5-5", 1, 5000) + "\n";
        std::fs::OpenOptions::new().append(true).open(&transcript).unwrap().write_all(more.as_bytes()).unwrap();
        let (rows, _) = c.tick(&targets, t0 + Duration::from_secs(66), |_| None);
        let claude = &rows.iter().find(|r| r.0 == "w1:p1").unwrap().1;
        assert_eq!((claude.turns, claude.context_used), (3, Some(5001)));

        // a statusline cache file, when present, adds Claude limits
        std::fs::create_dir_all(d.path().join("status")).unwrap();
        std::fs::write(d.path().join(format!("status/{CLAUDE_SID}.json")), fixture("claude_status.json")).unwrap();
        let (rows, limits) = c.tick(&targets, t0 + Duration::from_secs(71), |_| None);
        assert_eq!(limits.iter().map(|l| l.provider.as_str()).collect::<Vec<_>>(), ["claude", "codex"]);
        assert_eq!(rows.iter().find(|r| r.0 == "w1:p1").unwrap().1.cost_usd, Some(3.2));
    }

    #[test]
    fn mode_falls_back_to_permission_mode_argv() {
        let d = tempfile::tempdir().unwrap();
        let roots = Roots { claude_projects: d.path().into(), codex_sessions: d.path().join("x"), status_dir: None };
        let transcript = d.path().join(format!("-proj/{CLAUDE_SID}.jsonl"));
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(&transcript, assistant("a", "claude-opus-5-5", 1, 1) + "\n").unwrap(); // no permission-mode line
        let targets = [Target { pane_id: "w1:p1".into(), kind: "claude".into(), session: CLAUDE_SID.into() }];
        let mut calls = 0;
        let mut c = Collector::new(roots);
        for i in 0..3 {
            let (rows, _) = c.tick(&targets, Instant::now() + Duration::from_secs(i * 5), |_| {
                calls += 1;
                Some("plan".into())
            });
            assert_eq!(rows[0].1.mode.as_deref(), Some("plan"));
        }
        assert_eq!(calls, 1, "argv is asked once per session");
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            mode_from_argv(&argv(&["claude", "--model", "x", "--permission-mode", "auto"])).as_deref(),
            Some("auto")
        );
        assert_eq!(mode_from_argv(&argv(&["claude", "--permission-mode=plan"])).as_deref(), Some("plan"));
        assert_eq!(mode_from_argv(&argv(&["claude"])), None);
    }

    #[test]
    fn targets_come_from_agent_sessions() {
        let p = format!("{}/tests/fixtures/herdr_agent_list.json", env!("CARGO_MANIFEST_DIR"));
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        let t = targets_from(&v["result"]);
        assert_eq!(t.len(), 3);
        assert_eq!((t[0].pane_id.as_str(), t[0].kind.as_str()), ("w1:p1", "claude"));
        assert_eq!(t[0].session, "00000000-0000-0000-0000-000000000001");
        assert_eq!(t[1].kind, "codex");
    }
}
