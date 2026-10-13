//! EMERGENCY: full-screen takeover when a pilot is blocked or memory is critical. Hazard bars
//! scroll in opposite directions, the border pulses red at 1 Hz, one card per blocked agent with
//! its time blocked counting up, a memory card when memory is critical, and a small MAGI strip.

use femtovg::{Align, Paint, Path};

use crate::state::{fmt_age, fmt_bytes, Agent, AgentStatus, AgentUsage};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Rect, DESIGN_H, DESIGN_W};
use crate::tv::panels::{alert, plate, Ctx};
use crate::tv::scene::EmergencyCause;

/// Most agent cards shown; the rest are summarized as `+N`.
pub const MAX_CARDS: usize = 6;

/// `H:MM:SS` of a duration in seconds.
pub fn hms(s: u64) -> String {
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Card grid for `n` cards inside `area`: up to 3 per row, rows share the height.
pub fn card_rects(area: Rect, n: usize) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let per_row = n.min(3);
    let rows = n.div_ceil(per_row);
    let gap = 28.0;
    let w = (area.w - gap * (per_row as f32 - 1.0)) / per_row as f32;
    let h = (area.h - gap * (rows as f32 - 1.0)) / rows as f32;
    (0..n).map(|i| Rect::new(area.x + (i % per_row) as f32 * (w + gap), area.y + (i / per_row) as f32 * (h + gap), w, h)).collect()
}

/// Layers: the hazard bars, the 1 Hz border pulse and the card hexes move (live); title, banner,
/// MAGI strip, cards and the other pilots change at data rate (base, 1 Hz).
pub fn draw(ctx: &mut Ctx, cause: EmergencyCause) {
    let t = ctx.t;
    let (base_layer, live_layer) = (ctx.layer.base(), ctx.layer.live());
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    if live_layer {
        gfx::hazard(c, Rect::new(0.0, 0.0, DESIGN_W, 90.0), t * 0.5, Palette::RED, Palette::BLACK);
        gfx::hazard(c, Rect::new(0.0, DESIGN_H - 90.0, DESIGN_W, 90.0), -t * 0.5, Palette::RED, Palette::BLACK);
        let pulse = 0.5 + 0.5 * (t * std::f32::consts::TAU).cos();
        gfx::stroke_rect(c, Rect::new(12.0, 102.0, DESIGN_W - 24.0, DESIGN_H - 204.0), gfx::alpha(Palette::RED, 0.35 + 0.65 * pulse), 10.0);
    }
    // the cards' hexes are live; everything else below is base
    let blocked: Vec<&Agent> = if ctx.scene.preview { preview_agents(ctx.state) } else { ctx.state.sorted_agents().into_iter().filter(|a| a.status == AgentStatus::Blocked).collect() };
    let mem = matches!(cause, EmergencyCause::MemoryCritical | EmergencyCause::Both);
    let n_agents = blocked.len().min(if mem { MAX_CARDS - 1 } else { MAX_CARDS });
    let n = n_agents + mem as usize;
    let few = n <= 2;
    let area = Rect::new(120.0, 670.0, DESIGN_W - 240.0, if few { 860.0 } else { 1300.0 });
    let rects = card_rects(area, n.max(1));
    if live_layer {
        for (a, r) in blocked.iter().take(n_agents).zip(&rects) {
            card_hex(ctx, *r, a);
        }
    }
    if !base_layer {
        return;
    }
    let c = &mut *ctx.canvas;

    // title: 緊急 + EMERGENCY, banner with the pattern
    let k = gfx::text_paint(&fonts.mincho(), 300.0, Palette::WHITE);
    let kw = gfx::glow_text(c, 120.0, 430.0, "緊急", &k, Palette::RED, 24.0);
    let e = gfx::text_paint(&fonts.bold(), 160.0, Palette::RED).with_letter_spacing(14.0);
    gfx::glow_text(c, 120.0 + kw + 60.0, 400.0, "EMERGENCY", &e, gfx::alpha(Palette::RED, 0.8), 18.0);
    let preview = ctx.scene.preview;
    let pattern = match cause {
        _ if preview => "PREVIEW · NO ACTIVE EMERGENCY",
        EmergencyCause::Blocked => "PATTERN: BLOCKED · PILOT INPUT REQUIRED",
        EmergencyCause::MemoryCritical => "PATTERN: MEMORY CRITICAL",
        EmergencyCause::Both => "PATTERN: BLOCKED + MEMORY CRITICAL",
    };
    let banner = Rect::new(120.0 + kw + 60.0, 440.0, DESIGN_W - 120.0 - (120.0 + kw + 60.0), 76.0);
    gfx::fill_rect(c, banner, if preview { Palette::AMBER } else { Palette::RED });
    let bp = gfx::text_paint(&fonts.bold(), 52.0, Palette::BLACK).with_letter_spacing(5.0);
    gfx::text(c, banner.x + 28.0, banner.y + 58.0, pattern, &bp);

    magi_strip(ctx, Rect::new(120.0, 560.0, DESIGN_W - 240.0, 70.0));

    // cards: the blocked pilots, or in a preview the longest-working one
    let shown: Vec<String> = blocked.iter().map(|a| a.pane_id.clone()).collect();
    let age_label = if preview { "PREVIEW · TIME IN STATE" } else { "TIME BLOCKED" };
    for (a, r) in blocked.iter().take(n_agents).zip(&rects) {
        agent_card(ctx, *r, a, age_label);
    }
    if mem {
        memory_card(ctx, rects[n_agents.min(rects.len() - 1)]);
    }
    if n == 0 {
        let p = gfx::text_paint(&ctx.fonts.bold(), 60.0, Palette::RED).with_letter_spacing(6.0);
        gfx::text_center(ctx.canvas, area, if preview { "PREVIEW · NO PILOTS" } else { "CAUSE CLEARED · HOLDING" }, &p);
    }
    if blocked.len() > n_agents {
        let p = gfx::text_paint(&ctx.fonts.bold(), 44.0, Palette::RED);
        gfx::text(ctx.canvas, area.x, area.bottom() + 54.0, &format!("+{} MORE BLOCKED", blocked.len() - n_agents), &p);
    }
    if few {
        others(ctx, Rect::new(120.0, 1560.0, DESIGN_W - 240.0, 480.0), &shown);
    }
}

/// Preview cards: the oldest working pilot (else the oldest pilot of any status).
pub fn preview_agents(state: &crate::state::State) -> Vec<&Agent> {
    let oldest = |s: Option<AgentStatus>| state.herdr.agents.iter().filter(|a| s.is_none_or(|s| a.status == s)).min_by_key(|a| a.since);
    oldest(Some(AgentStatus::Working)).or_else(|| oldest(None)).into_iter().collect()
}

/// The three votes in one line.
fn magi_strip(ctx: &mut Ctx, r: Rect) {
    let votes = ctx.state.magi(&ctx.config.thresholds);
    let c = &mut *ctx.canvas;
    gfx::stroke_rect(c, r, gfx::alpha(Palette::CYAN, 0.6), 2.0);
    let mut x = r.x + 24.0;
    let base = r.y + r.h / 2.0 + 14.0;
    let lp = gfx::text_paint(&ctx.fonts.bold(), 36.0, Palette::CYAN).with_letter_spacing(4.0);
    x += gfx::text(c, x, base, "MAGI", &lp) + 50.0;
    for v in &votes {
        let up = gfx::text_paint(&ctx.fonts.display(), 32.0, Palette::CYAN).with_letter_spacing(2.0);
        x += gfx::text(c, x, base, v.unit, &up) + 14.0;
        let color = if v.approve { Palette::GREEN } else { Palette::RED };
        x += gfx::text(c, x, base + 4.0, if v.approve { "承認" } else { "否定" }, &gfx::text_paint(&ctx.fonts.mincho(), 44.0, color)) + 14.0;
        let rp = gfx::text_paint(&ctx.fonts.mono(), 28.0, gfx::alpha(Palette::WHITE, 0.75));
        x += gfx::text(c, x, base, &v.reason, &rp) + 70.0;
    }
    let reason = alert::reason(ctx);
    let rp = gfx::text_paint(&ctx.fonts.mono(), 28.0, Palette::RED).with_text_align(Align::Right);
    let fit = gfx::ellipsize(ctx.canvas, &rp, &reason, (r.right() - 24.0 - x).max(0.0));
    gfx::text(ctx.canvas, r.right() - 24.0, base, &fit, &rp);
}

fn agent_card(ctx: &mut Ctx, r: Rect, a: &Agent, age_label: &str) {
    let meta = ctx.state.meta(a).cloned();
    let blocked_for = ctx.state.age(a).as_secs();
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let outline = plate::chamfer_path(r, plate::Cuts::TL_BR, 40.0);
    c.fill_path(&outline, &Paint::color(gfx::alpha(Palette::RED, 0.08)));
    c.stroke_path(&outline, &Paint::color(Palette::RED).with_line_width(4.0));
    // wide card (one or two up): a detail column on the right half
    let r = if r.w > 1800.0 {
        let split = r.x + r.w * 0.52;
        card_side(ctx, Rect::new(split, r.y + 40.0, r.right() - 50.0 - split, r.h - 80.0), a);
        Rect::new(r.x, r.y, r.w * 0.52, r.h)
    } else {
        r
    };
    let c = &mut *ctx.canvas;
    let (hx, hy, hr) = hex_geometry(r);

    let x = hx + hr + 40.0;
    let w = r.right() - 40.0 - x;
    let np = gfx::text_paint(&fonts.bold(), 64.0, Palette::WHITE).with_letter_spacing(3.0);
    gfx::text(c, x, r.y + 100.0, &gfx::ellipsize(c, &np, &a.display_name().to_uppercase(), w), &np);
    let wp = gfx::text_paint(&fonts.label(), 40.0, Palette::CYAN);
    gfx::text(c, x, r.y + 150.0, &gfx::ellipsize(c, &wp, &format!("{} / {}", a.workspace_label, a.tab_label), w), &wp);
    let tp = gfx::text_paint(&fonts.label(), 34.0, gfx::alpha(Palette::WHITE, 0.8));
    gfx::text(c, x, r.y + 196.0, &gfx::ellipsize(c, &tp, &a.title, w), &tp);

    // time blocked, counting up
    let y = (r.y + 196.0 + 40.0).max(hy + hr + 20.0);
    let lp = gfx::text_paint(&fonts.label(), 30.0, Palette::RED).with_letter_spacing(3.0);
    gfx::text(c, r.x + 40.0, y + 20.0, age_label, &lp);
    let seg = gfx::text_paint(&fonts.segment(), (r.h * 0.2).clamp(72.0, 140.0), Palette::RED);
    let base = y + 30.0 + seg.font_size();
    gfx::text(c, r.x + 40.0, base, "8:88:88", &seg.clone().with_color(gfx::alpha(Palette::RED, 0.1)));
    let sw = gfx::glow_text(c, r.x + 40.0, base, &hms(blocked_for), &seg, gfx::alpha(Palette::RED, 0.8), 14.0);
    let dx = r.x + 40.0 + sw.max(gfx::text_width(c, "8:88:88", &seg)) + 50.0;
    let dw = r.right() - 40.0 - dx;
    let mp = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::ORANGE, 0.85));
    let cp = gfx::text_paint(&fonts.mono(), 28.0, gfx::alpha(Palette::WHITE, 0.6));
    if dw > 200.0 {
        if let Some(m) = &meta {
            if let Some(b) = &m.branch {
                gfx::text(c, dx, base - seg.font_size() * 0.6, &gfx::ellipsize(c, &mp, &format!("⎇ {b}"), dw), &mp);
            }
            gfx::text(c, dx, base - seg.font_size() * 0.6 + 44.0, &gfx::ellipsize(c, &cp, &m.cwd, dw), &cp);
        }
        let rp = gfx::text_paint(&fonts.mono(), 28.0, gfx::alpha(Palette::WHITE, 0.6));
        gfx::text(c, dx, base - seg.font_size() * 0.6 + 88.0, &format!("{} · {} · {}", a.kind.to_uppercase(), a.pane_id, a.rss_bytes.map_or("--".into(), fmt_bytes)), &rp);
    }
    // context use, when the card has room between the timer row and the jump line
    let cmd_y = r.bottom() - 40.0;
    let sync_top = base + 70.0;
    if cmd_y - 50.0 - sync_top >= 140.0 {
        sync_block(ctx, Rect::new(r.x + 40.0, sync_top, r.w - 120.0, 140.0), a);
    }
    let c = &mut *ctx.canvas;
    // jump command
    let jp = gfx::text_paint(&fonts.mono(), 34.0, Palette::GREEN);
    let label = gfx::text_paint(&fonts.label(), 28.0, gfx::alpha(Palette::GREEN, 0.7)).with_letter_spacing(2.0);
    let lw = gfx::text(c, r.x + 40.0, cmd_y, "JUMP", &label);
    let name = a.name.clone().unwrap_or_else(|| a.pane_id.clone());
    gfx::text(c, r.x + 60.0 + lw, cmd_y, &gfx::ellipsize(c, &jp, &format!("$ herdr agent focus {name}"), r.w - 120.0 - lw), &jp);
}

/// Label/value rows of a pilot's profile: its type, then what its usage log knows.
pub fn profile_rows(type_label: String, u: Option<&AgentUsage>, now: std::time::SystemTime) -> Vec<(&'static str, String)> {
    let mut rows = vec![("TYPE", type_label)];
    let Some(u) = u else { return rows };
    let age = |t: std::time::SystemTime| fmt_age(now.duration_since(t).unwrap_or_default());
    if let Some(m) = &u.mode {
        rows.push(("MODE", m.to_uppercase()));
    }
    if let Some(v) = &u.version {
        rows.push(("VERSION", v.clone()));
    }
    if let Some(s) = u.session_started {
        rows.push(("SESSION", age(s)));
    }
    if u.turns > 0 {
        rows.push(("TURNS", u.turns.to_string()));
    }
    if let Some(l) = u.last_activity {
        rows.push(("LAST ACTIVITY", format!("{} AGO", age(l))));
    }
    rows
}

/// The pilot's context use under its timer, when the card has room: sync ratio (big, with a
/// 40-segment bar), context and token counts, cost.
fn sync_block(ctx: &mut Ctx, r: Rect, a: &Agent) {
    let st = ctx.state;
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let u = st.usage_of(a);
    let ratio = u.and_then(|u| u.sync_ratio());
    let color = crate::tv::usage::sync_color(ratio);
    let lp = gfx::text_paint(&fonts.label(), 30.0, color).with_letter_spacing(3.0);
    let w = gfx::text(c, r.x, r.y + 30.0, "SYNC RATIO", &lp);
    let estimated = u.is_some_and(|u| u.context_window_estimated);
    let value = ratio.map_or("--".to_string(), |v| format!("{}{v:.0}%", if estimated { "~" } else { "" }));
    gfx::text(c, r.x + w + 24.0, r.y + 34.0, &value, &gfx::text_paint(&fonts.mono(), 40.0, color));
    crate::tv::panels::units::sync_bar(c, Rect::new(r.x, r.y + 56.0, r.w, 26.0), ratio, 40);
    let dp = gfx::text_paint(&fonts.mono(), 28.0, gfx::alpha(Palette::WHITE, 0.75));
    let line = match u {
        Some(u) => {
            let tok = crate::tv::usage::fmt_tokens;
            let ctx_part = match (u.context_pct(), u.context_used, u.context_window) {
                (Some(p), Some(used), Some(win)) => format!("CTX {p:.0}% · {} / {}", tok(used), tok(win)),
                _ => "CTX --".into(),
            };
            let cost = u.cost_usd.map_or(String::new(), |c| format!(" · ${c:.2}"));
            format!("{ctx_part} · ↓{} ↑{}{cost}", tok(u.total_tokens() - u.output_tokens), tok(u.output_tokens))
        }
        None => "NO USAGE LOG FOR THIS PILOT".into(),
    };
    let fit = gfx::ellipsize(c, &dp, &line, r.w);
    gfx::text(c, r.x, r.y + 124.0, &fit, &dp);
}

/// Card-local text column (left part of a wide card).
fn card_main(r: Rect) -> Rect {
    if r.w > 1800.0 { Rect::new(r.x, r.y, r.w * 0.52, r.h) } else { r }
}

/// Centre and radius of a card's hex.
fn hex_geometry(main: Rect) -> (f32, f32, f32) {
    let hr = (main.h * 0.22).min(120.0);
    (main.x + 40.0 + hr, main.y + 40.0 + hr, hr)
}

/// The pulsing red hex carrying the pilot's initials (live layer).
fn card_hex(ctx: &mut Ctx, r: Rect, a: &Agent) {
    let (hx, hy, hr) = hex_geometry(card_main(r));
    let pulse = 0.55 + 0.45 * (ctx.t * std::f32::consts::TAU).cos();
    let c = &mut *ctx.canvas;
    gfx::hex(c, hx, hy, hr, Some(gfx::alpha(Palette::RED, pulse)), Palette::RED);
    let ip = gfx::text_paint(&ctx.fonts.bold(), hr * 0.5, Palette::WHITE).with_text_align(Align::Center);
    let initials: String = a.display_name().split(['-', '_', ' ']).filter_map(|w| w.chars().next()).take(3).collect::<String>().to_uppercase();
    gfx::text(c, hx, hy + hr * 0.18, &initials, &ip);
}

/// Recent transitions of this pilot, its sector-mates and the jobs running in its workspace.
fn card_side(ctx: &mut Ctx, r: Rect, a: &Agent) {
    let st = ctx.state;
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    gfx::line(c, r.x - 30.0, r.y, r.x - 30.0, r.bottom(), gfx::alpha(Palette::RED, 0.5), 2.0);
    let hp = gfx::text_paint(&fonts.bold(), 32.0, Palette::ORANGE).with_letter_spacing(4.0);
    let rp = gfx::text_paint(&fonts.mono(), 28.0, gfx::alpha(Palette::WHITE, 0.8));
    let mut y = r.y + 30.0;
    gfx::text(c, r.x, y, "RECENT TRANSITIONS", &hp);
    y += 44.0;
    let name = a.display_name();
    let mine: Vec<_> = st.ticker.iter().filter(|e| e.agent == name).take(4).collect();
    if mine.is_empty() {
        gfx::text(c, r.x, y, "NONE OBSERVED", &rp.clone().with_color(Palette::DIM));
        y += 38.0;
    }
    for e in mine {
        let (.., h, m, s) = crate::tv::panels::header::local_time(e.at);
        let w = gfx::text(c, r.x, y, &format!("{h:02}:{m:02}:{s:02}  {} →", e.from.label()), &rp);
        gfx::text(c, r.x + w + 14.0, y, e.to.label(), &gfx::text_paint(&fonts.mono(), 28.0, crate::tv::panels::status_color(e.to)));
        y += 38.0;
    }
    y += 24.0;
    gfx::text(c, r.x, y, &format!("SECTOR {}", a.workspace_label.to_uppercase()), &hp);
    y += 44.0;
    let mates: Vec<&Agent> = st.herdr.agents.iter().filter(|o| o.workspace_label == a.workspace_label && o.pane_id != a.pane_id).collect();
    if mates.is_empty() {
        gfx::text(c, r.x, y, "NO OTHER PILOTS", &rp.clone().with_color(Palette::DIM));
        y += 38.0;
    }
    for o in mates.iter().take(3) {
        alert::status_icon(c, r.x, y - 10.0, 22.0, o.status);
        let line = format!("{}  {} {}", o.display_name(), o.status.label(), fmt_age(st.age(o)));
        gfx::text(c, r.x + 36.0, y, &gfx::ellipsize(c, &rp, &line, r.w - 36.0), &rp);
        y += 38.0;
    }
    let jobs: Vec<_> = st.jobs.iter().filter(|j| j.workspace_label == a.workspace_label).collect();
    if !jobs.is_empty() && y < r.bottom() - 90.0 {
        y += 24.0;
        gfx::text(c, r.x, y, "JOBS IN SECTOR", &hp);
        y += 44.0;
        let jp = gfx::text_paint(&fonts.mono(), 26.0, Palette::LCD);
        for j in jobs {
            if y > r.bottom() - 10.0 {
                break;
            }
            gfx::text(c, r.x, y, &gfx::ellipsize(c, &jp, &format!("{} · {}", j.label, j.command), r.w), &jp);
            y += 36.0;
        }
    }
    // what the usage log knows about this pilot, in the room left
    if y < r.bottom() - 120.0 {
        y += 24.0;
        gfx::text(c, r.x, y, "PILOT PROFILE", &hp);
        y += 44.0;
        let kp = gfx::text_paint(&fonts.label(), 26.0, gfx::alpha(Palette::ORANGE, 0.75)).with_letter_spacing(1.5);
        for (k, v) in profile_rows(st.type_label(a), st.usage_of(a), st.now) {
            if y > r.bottom() - 10.0 {
                break;
            }
            gfx::text(c, r.x, y, k, &kp);
            gfx::text(c, r.x + 260.0, y, &gfx::ellipsize(c, &rp, &v, r.w - 260.0), &rp);
            y += 38.0;
        }
    }
}

fn memory_card(ctx: &mut Ctx, r: Rect) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let outline = plate::chamfer_path(r, plate::Cuts::TR_BL, 40.0);
    c.fill_path(&outline, &Paint::color(gfx::alpha(Palette::RED, 0.08)));
    c.stroke_path(&outline, &Paint::color(Palette::RED).with_line_width(4.0));
    let lp = gfx::text_paint(&fonts.bold(), 52.0, Palette::RED).with_letter_spacing(5.0);
    gfx::text(c, r.x + 40.0, r.y + 80.0, "MEMORY CRITICAL", &lp);
    let Some(sys) = &ctx.state.sys else { return };
    let seg = gfx::text_paint(&fonts.segment(), (r.h * 0.2).clamp(72.0, 140.0), Palette::RED);
    let base = r.y + 110.0 + seg.font_size();
    let avail = format!("{:.2}", sys.mem_available as f64 / 1e9);
    let w = gfx::glow_text(c, r.x + 40.0, base, &avail, &seg, gfx::alpha(Palette::RED, 0.8), 14.0);
    let up = gfx::text_paint(&fonts.label(), 36.0, Palette::RED);
    gfx::text(c, r.x + 60.0 + w, base, "GB AVAILABLE", &up);
    let dp = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::WHITE, 0.8));
    let crit = ctx.config.thresholds.mem_critical_bytes;
    gfx::text(c, r.x + 40.0, base + 56.0, &format!("CRITICAL < {} · TOTAL {} · SWAP {}/{}", fmt_bytes(crit), fmt_bytes(sys.mem_total), fmt_bytes(sys.swap_used), fmt_bytes(sys.swap_total)), &dp);
    // top 5 by RSS
    let mut procs = ctx.state.procs.clone();
    procs.sort_by(|a, b| b.rss_bytes.cmp(&a.rss_bytes));
    let mut y = base + 120.0;
    let hp = gfx::text_paint(&fonts.label(), 28.0, Palette::ORANGE).with_letter_spacing(2.0);
    gfx::text(c, r.x + 40.0, y, "TOP MEMORY", &hp);
    y += 46.0;
    let np = gfx::text_paint(&fonts.mono(), 32.0, Palette::WHITE);
    let vp = np.clone().with_text_align(Align::Right).with_color(Palette::AMBER);
    for p in procs.iter().take(5) {
        if y > r.bottom() - 20.0 {
            break;
        }
        gfx::text(c, r.x + 40.0, y, &format!("{:>7} {}", p.pid, p.name), &np);
        gfx::text(c, r.right() - 40.0, y, &fmt_bytes(p.rss_bytes), &vp);
        y += 42.0;
    }
}

/// When only one or two cards are up, the rest of the pilots below them.
fn others(ctx: &mut Ctx, r: Rect, shown: &[String]) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let hp = gfx::text_paint(&fonts.bold(), 40.0, Palette::ORANGE).with_letter_spacing(5.0);
    gfx::text(c, r.x, r.y + 40.0, "OTHER PILOTS", &hp);
    gfx::line(c, r.x, r.y + 60.0, r.right(), r.y + 60.0, gfx::alpha(Palette::ORANGE, 0.5), 2.0);
    let others: Vec<&Agent> = ctx.state.sorted_agents().into_iter().filter(|a| a.status != AgentStatus::Blocked && !shown.contains(&a.pane_id)).collect();
    let cols = 3;
    let col_w = r.w / cols as f32;
    let row_h = 64.0;
    let rows = ((r.h - 80.0) / row_h).floor().max(1.0) as usize;
    let np = gfx::text_paint(&fonts.display(), 38.0, Palette::WHITE);
    let sp = gfx::text_paint(&fonts.mono(), 30.0, Palette::WHITE);
    for (i, a) in others.iter().take(cols * rows).enumerate() {
        let x = r.x + (i / rows) as f32 * col_w;
        let y = r.y + 110.0 + (i % rows) as f32 * row_h;
        let color = crate::tv::panels::status_color(a.status);
        alert::status_icon(c, x, y - 12.0, 26.0, a.status);
        let name = gfx::ellipsize(c, &np, &a.display_name().to_uppercase(), col_w * 0.55);
        gfx::text(c, x + 44.0, y, &name, &np);
        gfx::text(c, x + 44.0 + col_w * 0.58, y, &format!("{} {}", a.status.label(), fmt_age(ctx.state.age(a))), &sp.clone().with_color(color));
    }
    let mut p = Path::new();
    p.rect(r.x, r.bottom(), r.w, 2.0);
    c.fill_path(&p, &Paint::color(gfx::alpha(Palette::RED, 0.4)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_uses_the_longest_working_pilot() {
        use crate::state::tests::{agent, t0};
        use crate::state::State;
        use std::time::Duration;
        let mut s = State::new(t0());
        let a = |pane: &str, st: AgentStatus, secs: u64| {
            let mut x = agent(pane, pane, st, "w1");
            x.since = t0() + Duration::from_secs(secs);
            x
        };
        s.herdr.agents = vec![a("young", AgentStatus::Working, 10), a("old", AgentStatus::Working, 5), a("idle", AgentStatus::Idle, 1)];
        assert_eq!(preview_agents(&s).iter().map(|a| a.pane_id.as_str()).collect::<Vec<_>>(), ["old"]);
        s.herdr.agents = vec![a("done", AgentStatus::Done, 9), a("idle", AgentStatus::Idle, 1)];
        assert_eq!(preview_agents(&s).iter().map(|a| a.pane_id.as_str()).collect::<Vec<_>>(), ["idle"]);
        s.herdr.agents.clear();
        assert!(preview_agents(&s).is_empty());
    }

    #[test]
    fn pilot_profile_rows_with_and_without_usage() {
        use crate::state::tests::t0;
        use crate::state::AgentUsage;
        use std::time::Duration;
        let u = AgentUsage {
            mode: Some("auto".into()),
            version: Some("2.1.0".into()),
            turns: 42,
            session_started: Some(t0()),
            last_activity: Some(t0() + Duration::from_secs(3000)),
            ..Default::default()
        };
        let now = t0() + Duration::from_secs(3600);
        let rows = profile_rows("CLAUDE OPUS 5.5 XHIGH".into(), Some(&u), now);
        assert_eq!(rows[0], ("TYPE", "CLAUDE OPUS 5.5 XHIGH".to_string()));
        assert!(rows.contains(&("MODE", "AUTO".to_string())));
        assert!(rows.contains(&("SESSION", "1h00m".to_string())), "{rows:?}");
        assert!(rows.contains(&("TURNS", "42".to_string())));
        assert!(rows.contains(&("LAST ACTIVITY", "10m AGO".to_string())), "{rows:?}");
        let bare = profile_rows("CODEX".into(), None, now);
        assert_eq!(bare, vec![("TYPE", "CODEX".to_string())]);
    }

    #[test]
    fn hms_counts_up() {
        assert_eq!(hms(4), "0:00:04");
        assert_eq!(hms(3725), "1:02:05");
    }

    #[test]
    fn cards_fill_the_area_without_overlap() {
        let area = Rect::new(120.0, 670.0, 3600.0, 1300.0);
        for n in 1..=6 {
            let rs = card_rects(area, n);
            assert_eq!(rs.len(), n);
            for r in &rs { assert!(r.x >= area.x - 0.01 && r.right() <= area.right() + 0.01 && r.y >= area.y - 0.01 && r.bottom() <= area.bottom() + 0.01); }
            for (i, a) in rs.iter().enumerate() { for b in &rs[i + 1..] { assert!(!a.overlaps(b)); } }
        }
        assert_eq!(card_rects(area, 4).iter().filter(|r| r.y == area.y).count(), 3);
    }
}
