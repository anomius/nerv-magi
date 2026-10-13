//! Hub rendering: one pure `render(frame, state, config)` used by the live loop and the tests.
//!
//! Top to bottom: header (or the red EMERGENCY bar), agent rows, JOBS, CPU, DISK/NET, TOP,
//! power + sound, ticker. Every line is clipped to the pane width with `…`.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::config::{Config, Thresholds};
use crate::state::{
    fmt_age, fmt_bytes, fmt_rate, model_short, Agent, AgentStatus, AlertLevel, EmergencyCause, HerdrLink, PlayStatus,
    State,
};

pub const BLACK: Color = Color::Rgb(0x00, 0x00, 0x00);
pub const ORANGE: Color = Color::Rgb(0xFF, 0x99, 0x00);
pub const AMBER: Color = Color::Rgb(0xFF, 0xAA, 0x00);
pub const GREEN: Color = Color::Rgb(0x00, 0xFF, 0x00);
pub const CYAN: Color = Color::Rgb(0x00, 0xFF, 0xFF);
pub const RED: Color = Color::Rgb(0xFF, 0x00, 0x00);
pub const MAGENTA: Color = Color::Rgb(0xFF, 0x00, 0xFF);
pub const WHITE: Color = Color::Rgb(0xE0, 0xE0, 0xE0);
pub const YELLOW: Color = Color::Rgb(0xFF, 0xC4, 0x00);
pub const DIM: Color = Color::Rgb(0x55, 0x55, 0x55);

/// CPU, DISK/NET, TOP, power + sound (LIMITS is added when known).
const TELEMETRY_LINES: usize = 4;

// ── text fitting ─────────────────────────────────────────────────────────────────────────────

fn width(s: &str) -> usize {
    Span::raw(s).width()
}

/// Always cut: at most `w - 1` columns of `s`, then `…`.
fn cut(s: &str, w: usize) -> String {
    if w == 0 {
        return String::new();
    }
    let (mut out, mut used, mut buf) = (String::new(), 0, [0u8; 4]);
    for c in s.chars() {
        let cw = width(c.encode_utf8(&mut buf));
        if used + cw > w - 1 {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

/// `s` if it fits in `w` display columns, else cut with `…`.
pub fn fit(s: &str, w: usize) -> String {
    if width(s) <= w {
        s.to_string()
    } else {
        cut(s, w)
    }
}

/// `fit`, then right-padded to exactly `w` columns.
fn pad(s: &str, w: usize) -> String {
    let f = fit(s, w);
    let n = w.saturating_sub(width(&f));
    f + &" ".repeat(n)
}

fn rpad(s: &str, w: usize) -> String {
    let f = fit(s, w);
    " ".repeat(w.saturating_sub(width(&f))) + &f
}

/// Styled spans clipped to `w` columns; the span that crosses the edge ends in `…`.
fn clip(spans: Vec<Span<'static>>, w: usize) -> Line<'static> {
    if spans.iter().map(Span::width).sum::<usize>() <= w {
        return Line::from(spans);
    }
    let (mut out, mut used) = (Vec::new(), 0);
    for s in spans {
        let sw = s.width();
        if used + sw < w {
            used += sw;
            out.push(s);
        } else {
            out.push(Span::styled(cut(&s.content, w - used), s.style));
            break;
        }
    }
    Line::from(out)
}

fn sp(text: impl Into<String>, fg: Color) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(fg))
}

// ── pure helpers ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowPlan {
    /// Non-idle agents with their own row.
    pub shown: usize,
    /// Idle agents get their own rows too.
    pub idle_rows: bool,
    /// Non-idle agents summarized on a `+N MORE` line (which also carries the idle count).
    pub more: usize,
    /// A separate `○ STANDBY ×N` line.
    pub standby_row: bool,
}

/// Idle agents are listed individually when there is room, else collapsed into one STANDBY row;
/// if the non-idle ones still overflow, the last row becomes `+N MORE`.
pub fn plan_rows(non_idle: usize, idle: usize, budget: usize) -> RowPlan {
    let plan = |shown, idle_rows, more, standby_row| RowPlan { shown, idle_rows, more, standby_row };
    if non_idle + idle <= budget {
        return plan(non_idle, true, 0, false);
    }
    if non_idle + usize::from(idle > 0) <= budget {
        return plan(non_idle, false, 0, idle > 0);
    }
    if budget == 0 {
        return plan(0, false, 0, false);
    }
    plan(budget - 1, false, non_idle - (budget - 1), false)
}

/// One block glyph per core, `▁` (idle) … `█` (busy).
pub fn mini_bars(cores: &[f32]) -> String {
    const G: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    cores.iter().map(|v| G[(v.clamp(0.0, 1.0) * 7.0).round() as usize]).collect()
}

/// Token counts: "512", "48K", "1.2M", "119M", "2.5B".
pub fn count_short(n: u64) -> String {
    let f = n as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9)
    } else if f >= 1e8 {
        format!("{:.0}M", f / 1e6)
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.0}K", f / 1e3)
    } else {
        n.to_string()
    }
}

/// "xhigh" → "XH", "high" → "H", "medium" → "M", "low" → "L", "minimal" → "MIN"; anything else
/// upper-cased.
pub fn effort_short(effort: &str) -> String {
    match effort {
        "xhigh" => "XH".into(),
        "high" => "H".into(),
        "medium" => "M".into(),
        "low" => "L".into(),
        "minimal" => "MIN".into(),
        other => other.to_uppercase(),
    }
}

/// "OPUS 5.5·XH" / "GPT-6 LUNA·H"; the kind until the log has told us the model.
fn type_compact(s: &State, a: &Agent) -> String {
    let Some(u) = s.usage_of(a) else { return a.kind.clone() };
    let Some(model) = &u.model else { return a.kind.clone() };
    match &u.effort {
        Some(e) => format!("{}·{}", model_short(model), effort_short(e)),
        None => model_short(model),
    }
}

/// "ctx 41%", or "ctx~41%" when the window is estimated.
fn ctx_text(s: &State, a: &Agent) -> (String, Color) {
    let Some(u) = s.usage_of(a) else { return (String::new(), DIM) };
    let Some(pct) = u.context_pct() else { return (String::new(), DIM) };
    let color = if pct > 80.0 {
        RED
    } else if pct >= 50.0 {
        AMBER
    } else {
        GREEN
    };
    (format!("ctx{}{pct:.0}%", if u.context_window_estimated { "~" } else { " " }), color)
}

/// "↑1.2M ↓48K": prompt side (input + cache read + cache write) up, output down.
fn tokens_text(s: &State, a: &Agent) -> String {
    s.usage_of(a)
        .filter(|u| u.total_tokens() > 0)
        .map(|u| {
            format!(
                "↑{} ↓{}",
                count_short(u.input_tokens + u.cache_read_tokens + u.cache_write_tokens),
                count_short(u.output_tokens)
            )
        })
        .unwrap_or_default()
}

fn load_color(v: f32) -> Color {
    if v > 0.9 {
        RED
    } else if v > 0.7 {
        AMBER
    } else {
        GREEN
    }
}

/// Text for the red EMERGENCY bar. Whether there is one comes from `State::emergency`, the rule the
/// TV uses too; the MAGI tally alone only colours the normal header.
pub fn emergency_text(s: &State, t: &Thresholds) -> Option<String> {
    let blocked = || {
        let names: Vec<String> = s
            .sorted_agents()
            .into_iter()
            .filter(|a| a.status == AgentStatus::Blocked)
            .map(Agent::display_name)
            .collect();
        let more = if names.len() > 1 { format!(" +{}", names.len() - 1) } else { String::new() };
        format!("{} BLOCKED{more}", names.first().map_or("", String::as_str))
    };
    let memory =
        || format!("MEMORY CRITICAL · FREE {:.1}G", s.sys.as_ref().map_or(0, |y| y.mem_available) as f64 / 1e9);
    Some(match s.emergency(t)? {
        EmergencyCause::Blocked => blocked(),
        EmergencyCause::MemoryCritical => memory(),
        EmergencyCause::Both => format!("{} · {}", blocked(), memory()),
    })
}

fn local_tm(t: SystemTime) -> libc::tm {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: localtime_r only writes the struct we own.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    tm
}

fn hms(t: SystemTime) -> String {
    let tm = local_tm(t);
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

fn hm(t: SystemTime) -> String {
    let tm = local_tm(t);
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

fn clock(secs: u64) -> String {
    format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

fn status_style(s: AgentStatus) -> (&'static str, Color) {
    match s {
        AgentStatus::Blocked => ("▲", RED),
        AgentStatus::Working => ("●", GREEN),
        AgentStatus::Done => ("✔", CYAN),
        AgentStatus::Idle => ("○", DIM),
        AgentStatus::Unknown => ("?", DIM),
    }
}

// ── lines ────────────────────────────────────────────────────────────────────────────────────

fn header(s: &State, cfg: &Config, w: usize) -> Line<'static> {
    let t = &cfg.thresholds;
    let votes = s.magi(t);
    if let Some(cause) = emergency_text(s, t) {
        // Cause first; then the MAGI votes (full names, else short) and the clock while they fit.
        let bar = Style::new().fg(BLACK).bg(RED).add_modifier(Modifier::BOLD);
        let left = format!(" 緊急 EMERGENCY · {cause}");
        let tally = |short: bool| {
            let unit = |u: &'static str| {
                let n = u.split('·').next().unwrap_or(u);
                if short {
                    &n[..3]
                } else {
                    n
                }
            };
            votes
                .iter()
                .map(|v| format!("{} {}", unit(v.unit), if v.approve { "承認" } else { "否定" }))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let clock = format!("{} ", hms(s.now));
        let spread = |l: String, r: &str| {
            (width(&l) + 1 + width(r) <= w).then(|| format!("{l}{}{r}", " ".repeat(w - width(&l) - width(r))))
        };
        let with = |short| format!("{left} · {}", tally(short));
        let text = spread(with(false), &clock)
            .or_else(|| (width(&with(false)) <= w).then(|| pad(&with(false), w)))
            .or_else(|| spread(with(true), &clock))
            .or_else(|| (width(&with(true)) <= w).then(|| pad(&with(true), w)))
            .unwrap_or_else(|| pad(&left, w));
        return Line::from(Span::styled(text, bar));
    }
    let yes = votes.iter().filter(|v| v.approve).count();
    let (word, color) = match s.alert_level(t) {
        AlertLevel::Nominal => ("NOMINAL", GREEN),
        AlertLevel::Caution => ("CAUTION", AMBER),
        AlertLevel::Emergency => ("EMERGENCY", RED),
    };
    let build = |short: bool| {
        let mut v = vec![
            Span::styled("NERV · MAGI", Style::new().fg(ORANGE).add_modifier(Modifier::BOLD)),
            sp(format!("  {}  ", hms(s.now)), WHITE),
        ];
        for vote in &votes {
            let name = vote.unit.split('·').next().unwrap_or(vote.unit);
            let name = if short { &name[..3] } else { name };
            v.push(sp(format!("{name} "), ORANGE));
            v.push(if vote.approve { sp("承認  ", GREEN) } else { sp("否定  ", RED) });
        }
        v.push(sp(format!("{yes}/3 {word}"), color));
        v
    };
    let full = build(false);
    let spans = if full.iter().map(Span::width).sum::<usize>() <= w { full } else { build(true) };
    clip(spans, w)
}

fn branch_text(s: &State, a: &Agent) -> String {
    s.meta(a).and_then(|m| m.branch.as_deref()).map(|b| format!("⎇ {b}")).unwrap_or_default()
}

/// Column widths of the agent rows; `ctx` / `tok` 0 = column hidden.
#[derive(Debug, Clone, Copy)]
struct Cols {
    name: usize,
    ty: usize,
    ws: usize,
    branch: usize,
    ctx: usize,
    tok: usize,
}

/// Name + workspace + branch keep at least this many columns; narrower panes drop the token
/// column first, then ctx.
const MIN_FLEX: usize = 24;

fn agent_cols(w: usize, want: [usize; 3], ty: usize, ctx: usize, tok: usize) -> Cols {
    // glyph 2 · type · status 8 · age 5 · ram 5 · separators
    let fixed = 26 + ty;
    let extra = |ctx: usize, tok: usize| if ctx > 0 { ctx + 1 } else { 0 } + if tok > 0 { tok + 1 } else { 0 };
    let tok = if w.saturating_sub(fixed + extra(ctx, tok)) < MIN_FLEX { 0 } else { tok };
    let ctx = if w.saturating_sub(fixed + extra(ctx, tok)) < MIN_FLEX { 0 } else { ctx };
    let [name, ws, branch] = split_flex(w.saturating_sub(fixed + extra(ctx, tok)), want);
    Cols { name, ty, ws, branch, ctx, tok }
}

/// [name, workspace, branch] in `flex` columns: as wide as the content when it fits, else name
/// 45 % / workspace 23 % / branch the rest, with any slack handed back.
fn split_flex(flex: usize, want: [usize; 3]) -> [usize; 3] {
    if want.iter().sum::<usize>() <= flex {
        return want;
    }
    let mut c = [(flex * 45 / 100).max(8).min(want[0]), (flex * 23 / 100).min(want[1]), 0];
    c[2] = flex.saturating_sub(c[0] + c[1]).min(want[2]);
    let mut slack = flex.saturating_sub(c.iter().sum());
    for i in 0..3 {
        let add = (want[i] - c[i]).min(slack);
        c[i] += add;
        slack -= add;
    }
    c
}

fn agent_row(s: &State, a: &Agent, cols: Cols, w: usize) -> Line<'static> {
    let (glyph, color) = status_style(a.status);
    let Cols { name: nw, ty: tw, ws: ww, branch: bw, .. } = cols;
    let idle = matches!(a.status, AgentStatus::Idle | AgentStatus::Unknown);
    let text = if idle { DIM } else { WHITE };
    let mut name = Style::new().fg(text);
    if a.status == AgentStatus::Blocked {
        name = name.add_modifier(Modifier::BOLD);
    }
    let branch = branch_text(s, a);
    let spans = vec![
        sp(format!("{glyph} "), color),
        Span::styled(pad(&a.display_name(), nw), name),
        sp(format!(" {} ", pad(&type_compact(s, a), tw)), if idle { DIM } else { ORANGE }),
        sp(pad(&a.workspace_label, ww), if idle { DIM } else { CYAN }),
        sp(format!(" {} ", pad(&branch, bw)), DIM),
        sp(pad(a.status.label(), 8), color),
        sp(format!(" {}", rpad(&fmt_age(s.age(a)), 5)), if idle { DIM } else { GREEN }),
        sp(format!(" {}", rpad(&a.rss_bytes.map(fmt_bytes).unwrap_or_default(), 5)), text),
    ];
    let mut spans = spans;
    if cols.ctx > 0 {
        let (ctx, c) = ctx_text(s, a);
        spans.push(sp(format!(" {}", rpad(&ctx, cols.ctx)), if idle { DIM } else { c }));
    }
    if cols.tok > 0 {
        spans.push(sp(format!(" {}", pad(&tokens_text(s, a), cols.tok)), DIM));
    }
    clip(spans, w)
}

fn agent_lines(s: &State, budget: usize, w: usize) -> Vec<Line<'static>> {
    if budget == 0 {
        return Vec::new();
    }
    match &s.herdr.link {
        HerdrLink::Lost { since, reason } => {
            let age = fmt_age(s.now.duration_since(*since).unwrap_or_default());
            let spans = vec![
                Span::styled("MAGI LINK LOST · RETRY", Style::new().fg(AMBER).add_modifier(Modifier::BOLD)),
                sp(format!("  {age} · {reason}"), DIM),
            ];
            return vec![clip(spans, w)];
        }
        HerdrLink::Connecting => return vec![clip(vec![sp("MAGI LINK · CONNECTING…", DIM)], w)],
        HerdrLink::Linked => {}
    }
    let sorted = s.sorted_agents();
    let (idle, busy): (Vec<&Agent>, Vec<&Agent>) = sorted.into_iter().partition(|a| a.status == AgentStatus::Idle);
    if idle.is_empty() && busy.is_empty() {
        return vec![clip(vec![sp("NO PILOTS DETECTED", DIM)], w)];
    }
    let plan = plan_rows(busy.len(), idle.len(), budget);
    let rows: Vec<&Agent> = busy
        .iter()
        .take(plan.shown)
        .chain(idle.iter().take(if plan.idle_rows { idle.len() } else { 0 }))
        .copied()
        .collect();
    let widest = |f: &dyn Fn(&Agent) -> String| rows.iter().map(|a| width(&f(a))).max().unwrap_or(0);
    let cols = agent_cols(
        w,
        [widest(&|a| a.display_name()), widest(&|a| a.workspace_label.clone()), widest(&|a| branch_text(s, a))],
        widest(&|a| type_compact(s, a)).min(16),
        widest(&|a| ctx_text(s, a).0),
        widest(&|a| tokens_text(s, a)),
    );
    let mut out: Vec<Line> = rows.iter().map(|a| agent_row(s, a, cols, w)).collect();
    let names = || idle.iter().map(|a| a.display_name()).collect::<Vec<_>>().join(" · ");
    if plan.standby_row {
        out.push(clip(vec![sp(format!("○ STANDBY ×{}  ", idle.len()), DIM), sp(names(), DIM)], w));
    }
    if plan.more > 0 {
        let mut spans = vec![sp(format!("+{} MORE", plan.more), AMBER)];
        if !idle.is_empty() {
            spans.push(sp(format!(" · ○ STANDBY ×{}  {}", idle.len(), names()), DIM));
        }
        out.push(clip(spans, w));
    }
    out
}

fn job_lines(s: &State, budget: usize, w: usize) -> Vec<Line<'static>> {
    let lw = s.jobs.iter().map(|j| width(&j.label)).max().unwrap_or(0).min(w / 4);
    s.jobs
        .iter()
        .take(budget)
        .enumerate()
        .map(|(i, j)| {
            clip(
                vec![
                    sp(if i == 0 { "JOBS " } else { "     " }, ORANGE),
                    sp(format!("▶ {} ", pad(&j.label, lw)), CYAN),
                    sp(j.command.clone(), WHITE),
                    sp(format!("  {}/{}", j.workspace_label, j.tab_label), DIM),
                ],
                w,
            )
        })
        .collect()
}

fn cpu_line(s: &State, w: usize) -> Line<'static> {
    let Some(sys) = &s.sys else { return clip(vec![sp("CPU ", ORANGE), sp("N/A", DIM)], w) };
    let mut v = vec![sp("CPU ", ORANGE)];
    v.extend(sys.cpu_cores.iter().map(|c| sp(mini_bars(&[*c]), load_color(*c))));
    v.push(sp(format!(" {:.0}%", sys.cpu_total * 100.0), WHITE));
    if let Some(t) = sys.temp_cpu_c {
        v.push(sp(format!(" {t:.0}°C"), WHITE));
    }
    v.push(sp(" · MEM ", ORANGE));
    v.push(sp(format!("{:.1}/{:.1}G", sys.mem_used() as f64 / 1e9, sys.mem_total as f64 / 1e9), WHITE));
    if let Some(g) = &sys.gpu {
        v.push(sp(" · GPU ", ORANGE));
        v.push(sp(format!("{}MHz {:.0}%", g.mhz, g.busy * 100.0), WHITE));
    }
    if let Some(f) = sys.fan_rpm {
        v.push(sp(" · FAN ", ORANGE));
        v.push(sp(f.to_string(), WHITE));
    }
    clip(v, w)
}

fn disk_net_line(s: &State, w: usize) -> Line<'static> {
    let mut v = vec![sp("DISK ", ORANGE)];
    let pct = |used: u64, total: u64| if total == 0 { 0 } else { used * 100 / total };
    match &s.sysx {
        Some(x) if !x.mounts.is_empty() => {
            for (i, m) in x.mounts.iter().enumerate() {
                let sep = if i == 0 { "" } else { " · " };
                v.push(sp(format!("{sep}{} {}%", m.path, pct(m.used, m.total)), WHITE));
            }
        }
        _ => match s.sys.as_ref().and_then(|y| y.disk.as_ref()) {
            Some(d) => v.push(sp(format!("/ {}%", pct(d.used, d.total)), WHITE)),
            None => v.push(sp("N/A", DIM)),
        },
    }
    if let Some(d) = s.sys.as_ref().and_then(|y| y.disk.as_ref()) {
        v.push(sp(format!(" R{} W{}", fmt_rate(d.read_bps), fmt_rate(d.write_bps)), DIM));
    }
    v.push(sp("  NET ", ORANGE));
    let up: Vec<_> = s.sysx.iter().flat_map(|x| x.ifaces.iter()).filter(|i| i.up).collect();
    if up.is_empty() {
        match &s.sys {
            Some(y) => v.push(sp(format!("↓{} ↑{}", fmt_rate(y.net.rx_bps), fmt_rate(y.net.tx_bps)), WHITE)),
            None => v.push(sp("N/A", DIM)),
        }
    }
    for (i, f) in up.iter().enumerate() {
        v.push(sp(format!("{}{} ", if i == 0 { "" } else { " · " }, f.name), CYAN));
        v.push(sp(format!("↓{} ↑{}", fmt_rate(f.rx_bps), fmt_rate(f.tx_bps)), WHITE));
    }
    clip(v, w)
}

/// "⟳16:00" within a day, else the weekday ("⟳Tue"); nothing once past.
fn reset_short(now: SystemTime, at: SystemTime) -> Option<String> {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let left = at.duration_since(now).ok().filter(|d| !d.is_zero())?;
    Some(if left.as_secs() < 86_400 { hm(at) } else { DAYS[local_tm(at).tm_wday.rem_euclid(7) as usize].to_string() })
}

/// `CLAUDE 5H 37% ⟳16:00 · 7D 12% ⟳Mon │ CODEX WEEK 17% ⟳Tue · credits 62500`; None without limits.
fn limits_line(s: &State, w: usize) -> Option<Line<'static>> {
    if s.limits.is_empty() {
        return None;
    }
    let mut v = Vec::new();
    for (i, l) in s.limits.iter().enumerate() {
        if i > 0 {
            v.push(sp(" │ ", DIM));
        }
        v.push(sp(l.provider.to_uppercase(), ORANGE));
        for (j, win) in l.windows.iter().enumerate() {
            v.push(sp(format!("{}{} ", if j == 0 { " " } else { " · " }, win.label), WHITE));
            let c = if win.used_pct >= 95.0 {
                RED
            } else if win.used_pct >= 80.0 {
                AMBER
            } else {
                GREEN
            };
            v.push(sp(format!("{:.0}%", win.used_pct), c));
            if let Some(r) = win.resets_at.and_then(|at| reset_short(s.now, at)) {
                v.push(sp(format!(" ⟳{r}"), DIM));
            }
        }
        if let Some(c) = &l.credits {
            v.push(sp(format!(" · credits {c}"), DIM));
        }
    }
    Some(clip(v, w))
}

fn top_line(s: &State, w: usize) -> Line<'static> {
    let mut v = vec![sp("TOP ", ORANGE)];
    if s.procs.is_empty() {
        v.push(sp("—", DIM));
    }
    for (i, p) in s.procs.iter().take(5).enumerate() {
        v.push(sp(format!("{}{} ", if i == 0 { "" } else { " · " }, p.name), WHITE));
        v.push(sp(format!("{:.0}%", p.cpu), load_color(p.cpu / 100.0)));
    }
    clip(v, w)
}

fn power_sound_line(s: &State, w: usize) -> Line<'static> {
    let mut v = Vec::new();
    match &s.power {
        Some(p) => {
            let source = if p.ac_online { "EXTERNAL" } else { "INTERNAL" };
            match &p.battery {
                Some(b) => {
                    v.push(sp("BAT ", ORANGE));
                    v.push(sp(b.seconds_left.map_or("--:--:--".into(), clock), YELLOW));
                    v.push(sp(format!(" {source} {:.0}%", b.percent), WHITE));
                }
                None => v.push(sp(format!("PWR {source}"), WHITE)),
            }
            v.push(sp(format!(" · {}", p.profile.mode().label()), ORANGE));
        }
        None => v.push(sp("PWR N/A", DIM)),
    }
    v.push(sp("   ♪ ", MAGENTA));
    match &s.media {
        Some(m) => {
            let who = if m.artist.is_empty() { m.title.clone() } else { format!("{} — {}", m.artist, m.title) };
            v.push(sp(who, WHITE));
            let at = m.position_at(s.now) as u64;
            let len = m.length_s.map_or("--:--".into(), |l| format!("{:02}:{:02}", l as u64 / 60, l as u64 % 60));
            let paused = if m.status == PlayStatus::Playing { "" } else { " ⏸" };
            v.push(sp(format!(" {:02}:{:02}/{len}{paused}", at / 60, at % 60), DIM));
        }
        None => v.push(sp("NO TAPE", DIM)),
    }
    match &s.audio {
        Some(a) => {
            v.push(sp(format!(" · VOL {:.0}%", a.volume * 100.0), WHITE));
            if a.muted {
                v.push(sp(" MUTED", RED));
            }
            v.push(sp(format!(" {}", a.sink_name), DIM));
        }
        None => v.push(sp(" · NO SIGNAL", DIM)),
    }
    clip(v, w)
}

/// Ticker events (newest first) packed into lines of `w` columns.
fn ticker_lines(s: &State, w: usize) -> Vec<Line<'static>> {
    if s.ticker.is_empty() {
        return vec![clip(vec![sp("◀◀ ", ORANGE), sp("NO TRANSITIONS YET", DIM)], w)];
    }
    let mut lines = Vec::new();
    let mut cur = vec![sp("◀◀ ", ORANGE)];
    let mut used = 3;
    for e in &s.ticker {
        let (_, color) = status_style(e.to);
        let item =
            vec![sp(format!("{} ", hm(e.at)), DIM), sp(format!("{} → ", e.agent), WHITE), sp(e.to.label(), color)];
        let iw: usize = item.iter().map(Span::width).sum::<usize>() + 3;
        if used > 3 && used + iw > w {
            lines.push(clip(std::mem::replace(&mut cur, vec![sp("   ", ORANGE)]), w));
            used = 3;
        }
        if used > 3 {
            cur.push(sp(" · ", DIM));
        }
        cur.extend(item);
        used += iw;
    }
    lines.push(clip(cur, w));
    lines
}

pub fn render(frame: &mut Frame, state: &State, config: &Config) {
    let area = frame.area();
    let (w, h) = (area.width as usize, area.height as usize);
    let limits = limits_line(state, w);
    let rest = h.saturating_sub(1 + TELEMETRY_LINES + usize::from(limits.is_some()));

    let ticker = ticker_lines(state, w);
    let agents_wanted = match state.herdr.link {
        HerdrLink::Linked => state.herdr.agents.len().max(1),
        _ => 1,
    };
    // Minimums first (1 ticker line, 2 jobs), agents next, leftovers to jobs then ticker.
    let ticker_min = ticker.len().min(1).min(rest);
    let jobs_min = state.jobs.len().min(2).min(rest - ticker_min);
    let agents = agents_wanted.min(rest - ticker_min - jobs_min);
    let mut left = rest - ticker_min - jobs_min - agents;
    let jobs = jobs_min + (state.jobs.len() - jobs_min).min(left);
    left -= jobs - jobs_min;
    let ticker_n = ticker_min + (ticker.len() - ticker_min).min(left);

    let mut lines = vec![header(state, config, w)];
    lines.extend(agent_lines(state, agents, w));
    lines.extend(job_lines(state, jobs, w));
    lines.push(cpu_line(state, w));
    lines.push(disk_net_line(state, w));
    lines.push(top_line(state, w));
    lines.extend(limits);
    lines.push(power_sound_line(state, w));
    lines.extend(ticker.into_iter().take(ticker_n));
    lines.truncate(h);
    frame.render_widget(Paragraph::new(lines).style(Style::new().fg(WHITE).bg(BLACK)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{
        tests::{agent, t0},
        *,
    };
    use ratatui::{backend::TestBackend, style::Color, Terminal};

    fn text(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>() + "\n")
            .collect()
    }

    fn draw(s: &State, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, s, &crate::config::Config::default())).unwrap();
        term
    }

    fn linked(agents: Vec<Agent>) -> State {
        let mut s = State::new(t0());
        s.apply(Update::HerdrLink(HerdrLink::Linked), t0());
        s.apply(Update::HerdrSnapshot { agents, workspaces: vec![] }, t0());
        s
    }

    fn render_with(agents: Vec<Agent>, w: u16, h: u16) -> String {
        text(&draw(&linked(agents), w, h))
    }

    fn sys(cpu: f32, avail: u64) -> SysState {
        SysState {
            cpu_total: cpu,
            cpu_cores: vec![0.1, 0.5, 0.9, 0.3],
            mem_total: 15_600_000_000,
            mem_available: avail,
            temp_cpu_c: Some(58.0),
            ..Default::default()
        }
    }

    #[test]
    fn shows_blocked_first_and_standby_row() {
        let out = render_with(
            vec![
                agent("w1:p1", "idle-a", AgentStatus::Idle, "w1"),
                agent("w1:p2", "blocked-b", AgentStatus::Blocked, "w1"),
                agent("w1:p3", "working-c", AgentStatus::Working, "w1"),
            ],
            80,
            20,
        );
        let b = out.find("blocked-b").unwrap();
        let w = out.find("working-c").unwrap();
        assert!(b < w);
        assert!(out.contains("STANDBY"));
        assert!(out.contains("MELCHIOR"));
    }

    #[test]
    fn many_agents_and_long_names_fit() {
        let agents = (0..25)
            .map(|i| {
                agent(
                    &format!("w1:p{i}"),
                    &format!("agent-with-a-very-long-name-number-{i}"),
                    AgentStatus::Working,
                    "w1",
                )
            })
            .collect();
        let out = render_with(agents, 60, 20);
        assert_eq!(out.lines().count(), 20);
        assert!(out.lines().all(|l| l.chars().count() <= 60));
        assert!(out.contains('…'));
        assert!(out.contains("MORE"));
    }

    #[test]
    fn link_lost_banner() {
        let mut s = State::new(t0());
        s.apply(Update::HerdrLink(HerdrLink::Lost { since: t0(), reason: "x".into() }), t0());
        let mut term = Terminal::new(TestBackend::new(80, 12)).unwrap();
        term.draw(|f| render(f, &s, &crate::config::Config::default())).unwrap();
        assert!(text(&term).contains("MAGI LINK LOST"));
    }

    #[test]
    fn emergency_header_names_the_blocked_agent() {
        let mut s = linked(vec![
            agent("w1:p2", "blocked-b", AgentStatus::Blocked, "w1"),
            agent("w1:p3", "working-c", AgentStatus::Working, "w1"),
        ]);
        s.apply(Update::Sys(sys(0.3, 5_000_000_000)), t0());
        let term = draw(&s, 80, 12);
        let header = text(&term).lines().next().unwrap().to_string();
        assert!(header.contains("EMERGENCY") && header.contains('緊'), "{header}");
        assert!(header.contains("blocked-b BLOCKED"), "{header}");
        let buf = term.backend().buffer();
        assert_eq!(buf[(0, 0)].bg, Color::Rgb(255, 0, 0));
        assert_eq!(buf[(79, 0)].bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn memory_critical_header_without_blocked_agents() {
        let mut s = linked(vec![agent("w1:p3", "working-c", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(sys(0.3, 500_000_000)), t0()); // below the 800 MB default
        let term = draw(&s, 80, 12);
        let header = text(&term).lines().next().unwrap().to_string();
        assert!(header.contains("MEMORY CRITICAL"), "{header}");
        assert_eq!(term.backend().buffer()[(0, 0)].bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn both_causes_name_the_agent_and_memory() {
        let mut s = linked(vec![agent("w1:p2", "blocked-b", AgentStatus::Blocked, "w1")]);
        s.apply(Update::Sys(sys(0.3, 500_000_000)), t0());
        let term = draw(&s, 100, 12);
        let header = text(&term).lines().next().unwrap().to_string();
        assert!(header.contains("blocked-b BLOCKED") && header.contains("MEMORY CRITICAL"), "{header}");
        assert_eq!(term.backend().buffer()[(0, 0)].bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn magi_tally_alone_is_styling_not_a_red_bar() {
        // hot CPU + low (not critical) memory: MAGI 1/3 → alert_level Emergency, but no takeover
        let mut s = linked(vec![agent("w1:p3", "working-c", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(SysState { temp_cpu_c: Some(95.0), ..sys(0.95, 1_000_000_000) }), t0());
        assert_eq!(s.alert_level(&crate::config::Thresholds::default()), AlertLevel::Emergency);
        let term = draw(&s, 80, 12);
        let header = text(&term).lines().next().unwrap().to_string();
        assert!(header.contains("MELCHIOR") && header.contains("1/3 EMERGENCY"), "{header}");
        assert_ne!(term.backend().buffer()[(0, 0)].bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn nominal_header_is_not_a_red_bar() {
        let mut s = linked(vec![agent("w1:p3", "working-c", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(sys(0.3, 5_000_000_000)), t0());
        let term = draw(&s, 80, 12);
        let header = text(&term).lines().next().unwrap().to_string();
        assert!(header.contains("MELCHIOR") && header.contains("NOMINAL"), "{header}");
        assert_ne!(term.backend().buffer()[(0, 0)].bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn dense_lines_show_jobs_top_cores_disk_net_and_branch() {
        let mut s = linked(vec![agent("w1:p1", "be-search-port", AgentStatus::Working, "w1")]);
        s.apply(Update::Sys(sys(0.34, 5_000_000_000)), t0());
        s.apply(
            Update::SysExtra(SysExtra {
                mounts: vec![Mount { path: "/".into(), used: 362_000_000_000, total: 510_000_000_000 }],
                ifaces: vec![Iface { name: "wlp1s0".into(), rx_bps: 1_200_000.0, tx_bps: 80_000.0, up: true }],
                ..Default::default()
            }),
            t0(),
        );
        s.apply(
            Update::Jobs(vec![PaneJob {
                pane_id: "w2:p3".into(),
                workspace_label: "hub".into(),
                tab_label: "jobs".into(),
                label: "api-server".into(),
                command: "uv run uvicorn app.main:app".into(),
                cwd: "~/x".into(),
            }]),
            t0(),
        );
        s.apply(
            Update::Procs(vec![
                ProcRow { pid: 1, name: "node".into(), cpu: 126.0, rss_bytes: 1, threads: 9, state: 'R' },
                ProcRow { pid: 2, name: "cava".into(), cpu: 12.0, rss_bytes: 1, threads: 2, state: 'S' },
            ]),
            t0(),
        );
        s.apply(
            Update::AgentMeta(vec![(
                "w1:p1".into(),
                AgentMeta { cwd: "~/x".into(), branch: Some("feat/search".into()), session_id: None },
            )]),
            t0(),
        );
        let out = text(&draw(&s, 100, 20));
        assert!(out.contains("JOBS") && out.contains("api-server") && out.contains("uv run uvicorn"), "{out}");
        assert!(out.contains("TOP") && out.contains("node 126%"), "{out}");
        assert!(out.contains("▂▅▇▃"), "{out}"); // one glyph per core
        assert!(out.contains("DISK") && out.contains("NET") && out.contains("wlp1s0"), "{out}");
        assert!(out.contains("⎇ feat/search"), "{out}");
    }

    #[test]
    fn wide_panes_keep_columns_tight_to_their_content() {
        let mut s = linked(vec![agent("w1:p1", "be-search-port", AgentStatus::Working, "search")]);
        s.apply(
            Update::AgentMeta(vec![(
                "w1:p1".into(),
                AgentMeta { cwd: "~".into(), branch: Some("main".into()), session_id: None },
            )]),
            t0(),
        );
        let out = text(&draw(&s, 200, 10));
        assert!(out.contains("● be-search-port claude search ⎇ main ACTIVE"), "{out}");
    }

    fn usage_state() -> State {
        let mut codex = agent("w1:p2", "fe-fixes", AgentStatus::Working, "w1");
        codex.kind = "codex".into();
        let mut s = linked(vec![agent("w1:p1", "be-port", AgentStatus::Working, "w1"), codex]);
        let now = t0();
        s.apply(
            Update::AgentUsage(vec![
                (
                    "w1:p1".into(),
                    AgentUsage {
                        model: Some("claude-opus-5-5".into()),
                        effort: Some("xhigh".into()),
                        context_used: Some(410_000),
                        context_window: Some(1_000_000),
                        input_tokens: 200_000,
                        cache_read_tokens: 1_000_000,
                        output_tokens: 48_000,
                        ..Default::default()
                    },
                ),
                (
                    "w1:p2".into(),
                    AgentUsage {
                        model: Some("gpt-6-luna".into()),
                        effort: Some("high".into()),
                        context_used: Some(73_300),
                        context_window: Some(258_400),
                        context_window_estimated: true,
                        input_tokens: 343_124,
                        output_tokens: 67_897,
                        ..Default::default()
                    },
                ),
            ]),
            now,
        );
        let window = |label: &str, pct: f32, in_secs: u64| LimitWindow {
            label: label.into(),
            used_pct: pct,
            resets_at: Some(now + std::time::Duration::from_secs(in_secs)),
            window_minutes: None,
        };
        s.apply(
            Update::PlanLimits(vec![
                PlanLimits {
                    provider: "claude".into(),
                    windows: vec![window("5H", 37.0, 3600), window("7D", 12.0, 5 * 86400)],
                    ..Default::default()
                },
                PlanLimits {
                    provider: "codex".into(),
                    plan: Some("pro".into()),
                    windows: vec![window("WEEK", 17.0, 3 * 86400)],
                    credits: Some("62500".into()),
                    ..Default::default()
                },
            ]),
            now,
        );
        s
    }

    #[test]
    fn agent_rows_show_type_context_and_tokens() {
        let out = text(&draw(&usage_state(), 120, 20));
        assert!(out.contains("OPUS 5.5·XH") && out.contains("GPT-6 LUNA·H"), "{out}");
        assert!(out.contains("ctx 41%") && out.contains("ctx~28%"), "{out}"); // ~ = estimated window
        assert!(out.contains("↑1.2M ↓48K") && out.contains("↑343K ↓68K"), "{out}");
    }

    #[test]
    fn limits_line_lists_providers_windows_resets_and_credits() {
        let out = text(&draw(&usage_state(), 120, 20));
        let line = out.lines().find(|l| l.contains("CODEX")).unwrap_or_default().to_string();
        assert!(line.contains("CLAUDE 5H 37%") && line.contains("7D 12%"), "{line}");
        assert!(line.contains("│ CODEX WEEK 17%") && line.contains("credits 62500"), "{line}");
        assert!(line.contains('⟳'), "{line}");
    }

    #[test]
    fn narrow_panes_drop_tokens_then_context() {
        let out = text(&draw(&usage_state(), 60, 20));
        assert!(out.lines().all(|l| l.chars().count() <= 60));
        assert!(out.contains("be-port") && !out.contains('↑'), "{out}");
        let mid = text(&draw(&usage_state(), 75, 20));
        assert!(mid.contains("ctx 41%") && !mid.contains("↑1.2M"), "{mid}");
    }

    #[test]
    fn compact_counts_and_effort() {
        for (n, want) in [
            (512, "512"),
            (48_000, "48K"),
            (1_200_000, "1.2M"),
            (343_124, "343K"),
            (119_189_808, "119M"),
            (2_500_000_000, "2.5B"),
        ] {
            assert_eq!(count_short(n), want);
        }
        for (e, want) in
            [("xhigh", "XH"), ("high", "H"), ("medium", "M"), ("low", "L"), ("minimal", "MIN"), ("max", "MAX")]
        {
            assert_eq!(effort_short(e), want);
        }
    }

    #[test]
    fn fit_truncates_by_display_width() {
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("abc", 4), "abc");
        assert_eq!(fit("緊急警告", 5), "緊急…");
        assert_eq!(fit("abc", 0), "");
    }

    #[test]
    fn row_plan_lists_idle_when_room_then_collapses() {
        assert_eq!(plan_rows(3, 6, 15), RowPlan { shown: 3, idle_rows: true, more: 0, standby_row: false });
        assert_eq!(plan_rows(3, 6, 5), RowPlan { shown: 3, idle_rows: false, more: 0, standby_row: true });
        assert_eq!(plan_rows(25, 0, 15), RowPlan { shown: 14, idle_rows: false, more: 11, standby_row: false });
        // on overflow the idle count rides on the MORE line
        assert_eq!(plan_rows(25, 4, 15), RowPlan { shown: 14, idle_rows: false, more: 11, standby_row: false });
        assert_eq!(plan_rows(2, 0, 0), RowPlan { shown: 0, idle_rows: false, more: 0, standby_row: false });
    }

    #[test]
    fn mini_bars_one_glyph_per_core() {
        assert_eq!(mini_bars(&[0.0, 0.1, 0.5, 0.9, 1.0, 1.7]), "▁▂▅▇██");
        assert_eq!(mini_bars(&[]), "");
    }
}
