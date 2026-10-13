//! EVA UNITS: one row per non-idle agent, a STANDBY row for idle ones, the SYNC trace below.

use femtovg::{Align, Paint, Path};

use super::plate::{self, Cuts};
use super::{status_color, Ctx};
use crate::state::{fmt_age, fmt_bytes, Agent, AgentStatus};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;
use crate::tv::usage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowPlan {
    /// Non-idle agents drawn as full rows.
    pub shown: usize,
    /// Non-idle agents summarized in a `+N MORE` row.
    pub more: usize,
    pub standby_row: bool,
}

/// One agent: two lines (status line + dim title · branch · cwd line).
pub const ROW_H: f32 = 52.0;
/// Space above the first row (frame tab).
pub const HEAD_H: f32 = 44.0;
/// The SYNC trace at the bottom of the panel.
pub const SYNC_H: f32 = 120.0;

/// Rows grow from ROW_H up to ROW_H_MAX when there are few agents (a third detail line appears
/// from ROW_H_3 up), so a short roster does not leave the plate empty.
pub const ROW_H_MAX: f32 = 88.0;
pub const ROW_H_3: f32 = 76.0;

pub fn row_height(rows: usize, avail: f32) -> f32 {
    if rows == 0 {
        return ROW_H;
    }
    (avail / rows as f32).clamp(ROW_H, ROW_H_MAX)
}

/// How many rows fit in a units panel of height `panel_h` (design px). Every agent gets its own
/// row when they all fit; on overflow idle agents collapse into one STANDBY row first, then the
/// non-idle ones that still do not fit are summarized in one `+N MORE` row.
pub fn plan_rows(non_idle: &[AgentStatus], idle: usize, panel_h: f32) -> RowPlan {
    let max_rows = ((panel_h - HEAD_H - SYNC_H) / ROW_H).floor().max(0.0) as usize;
    if non_idle.len() + idle <= max_rows {
        return RowPlan { shown: non_idle.len() + idle, more: 0, standby_row: false };
    }
    let standby_row = idle > 0 && max_rows > 0;
    let budget = max_rows - standby_row as usize;
    if non_idle.len() <= budget {
        RowPlan { shown: non_idle.len(), more: 0, standby_row }
    } else {
        let shown = budget.saturating_sub(1);
        RowPlan { shown, more: non_idle.len() - shown, standby_row }
    }
}

/// The roster plate in `r`, plus the chevron band in `band` (rows marked MT-nn).
pub fn draw(ctx: &mut Ctx, r: Rect, band: Rect) {
    let (base, live) = (ctx.layer.base(), ctx.layer.live());
    if base {
        plate::plate(ctx.canvas, r, Cuts::TL_BR, "PILOT ROSTER", Some("稼働状況"), Palette::ORANGE, ctx.fonts);
    }
    let sorted = ctx.state.sorted_agents();
    let (idle, non_idle): (Vec<&Agent>, Vec<&Agent>) = sorted.iter().partition(|a| a.status == AgentStatus::Idle);
    let statuses: Vec<AgentStatus> = non_idle.iter().map(|a| a.status).collect();
    // the plate's header band is a little taller than HEAD_H
    let plan = plan_rows(&statuses, idle.len(), r.h - (plate::PLATE_TOP - HEAD_H));
    if base {
        summary(ctx, r);
    }

    let inner = Rect::new(r.x + 12.0, r.y, r.w - 24.0, r.h);
    // all fit → sorted order including idle; overflow → non-idle only, then MORE, then STANDBY
    let rows: Vec<&Agent> = if plan.standby_row || plan.more > 0 {
        non_idle.iter().take(plan.shown).copied().collect()
    } else {
        sorted.clone()
    };
    let n_rows = rows.len() + (plan.more > 0) as usize + plan.standby_row as usize;
    let row_h = row_height(n_rows, r.h - plate::PLATE_TOP - SYNC_H);
    let max_ram = rows.iter().filter_map(|a| a.rss_bytes).max().unwrap_or(0);
    let mut y = r.y + plate::PLATE_TOP;
    let mut marks = Vec::new();
    for (i, a) in rows.iter().enumerate() {
        if base {
            row_base(ctx, Rect::new(inner.x, y, inner.w, row_h), a, i, max_ram);
        }
        marks.push((y, row_h, a.status));
        y += row_h;
    }
    if plan.more > 0 {
        if base {
            let p = gfx::text_paint(&ctx.fonts.bold(), 28.0, Palette::ORANGE).with_letter_spacing(3.0);
            gfx::text(ctx.canvas, inner.x + 36.0, y + 34.0, &format!("+{} MORE", plan.more), &p);
        }
        y += row_h;
    }
    if plan.standby_row {
        if base {
            standby(ctx, Rect::new(inner.x, y, inner.w, row_h), &idle);
        }
        y += row_h;
    }
    if live {
        live_rails(ctx, &rows, inner, r.y + plate::PLATE_TOP, row_h);
        let moving = ctx.state.counts().working > 0;
        plate::chevron_band_live(ctx.canvas, band, &marks, ctx.t, moving);
    }
    if !base {
        return;
    }
    plate::chevron_band_base(ctx.canvas, band, &marks, ctx.fonts);
    if matches!(ctx.state.herdr.link, crate::state::HerdrLink::Lost { .. }) {
        let p = gfx::text_paint(&ctx.fonts.bold(), 44.0, Palette::AMBER).with_letter_spacing(5.0);
        let mid = Rect::new(r.x, r.y + HEAD_H, r.w, r.h - HEAD_H - SYNC_H);
        let b = Rect::new(mid.x + 200.0, mid.y + mid.h / 2.0 - 50.0, mid.w - 400.0, 100.0);
        gfx::fill_rect(ctx.canvas, b, Palette::BLACK);
        gfx::stroke_rect(ctx.canvas, b, Palette::AMBER, 3.0);
        gfx::text_center(ctx.canvas, mid, "MAGI LINK LOST · RETRY", &p);
    }
    // the SYNC trace takes whatever height the rows leave (at least SYNC_H)
    let sync_top = (y + 16.0).min(r.bottom() - SYNC_H);
    sync_trace(ctx, Rect::new(inner.x + 8.0, sync_top, inner.w - 40.0, r.bottom() - sync_top - 16.0));
}

/// Pilot count and total agent RAM on the frame's top edge, right side.
fn summary(ctx: &mut Ctx, r: Rect) {
    let agents = &ctx.state.herdr.agents;
    let ram: u64 = agents.iter().filter_map(|a| a.rss_bytes).sum();
    let s = format!("{} PILOTS · Σ RAM {}", agents.len(), if ram > 0 { fmt_bytes(ram) } else { "--".into() });
    let p = gfx::text_paint(&ctx.fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_text_align(Align::Right);
    let w = gfx::text_width(ctx.canvas, &s, &p);
    let right = r.right() - 30.0;
    gfx::fill_rect(ctx.canvas, Rect::new(right - w - 14.0, r.y + 2.0, w + 28.0, 30.0), Palette::PANEL);
    gfx::text(ctx.canvas, right, r.y + 26.0, &s, &p);
}

fn lamp(ctx: &mut Ctx, x: f32, cy: f32, status: AgentStatus) {
    let color = status_color(status);
    let a = if status == AgentStatus::Blocked {
        0.35 + 0.65 * (0.5 + 0.5 * (ctx.t * std::f32::consts::TAU).cos())
    } else {
        1.0
    };
    let sq = Rect::new(x, cy - 9.0, 18.0, 18.0);
    if status == AgentStatus::Idle {
        gfx::stroke_rect(ctx.canvas, sq, color, 2.5);
    } else {
        gfx::fill_rect(ctx.canvas, sq, gfx::alpha(color, a));
    }
}

/// Column x offsets inside a row (design px from the row's left edge).
const NAME_X: f32 = 34.0;
const NAME_W: f32 = 280.0;
const TYPE_X: f32 = 324.0;
const TYPE_W: f32 = 280.0;
const STATUS_X: f32 = 614.0;
const SYNC_X: f32 = 780.0;
const SYNC_W: f32 = 116.0;
const CTX_R: f32 = 990.0;
const AGE_R: f32 = 1080.0;
const RAM_R: f32 = 1170.0;
const RAIL_X: f32 = 1184.0;

/// Mini sync-ratio bar: free context, coloured by level; dashed outline when unknown.
pub fn sync_bar(c: &mut gfx::Cv, r: Rect, ratio: Option<f32>, segments: usize) {
    match ratio {
        Some(v) => gfx::seg_bar(c, r, v / 100.0, segments, usage::sync_color(Some(v)), gfx::alpha(Palette::DIM, 0.3)),
        None => {
            let mut p = Path::new();
            p.rect(r.x, r.y, r.w, r.h);
            c.stroke_path(
                &p,
                &Paint::color(gfx::alpha(Palette::DIM, 0.8)).with_line_width(1.5).with_line_dash(&[6.0, 4.0]),
            );
        }
    }
}

/// Live motion of every row in a few paths: blocked lamps pulse, working rails scroll, blocked
/// rails get hazard stripes.
fn live_rails(ctx: &mut Ctx, rows: &[&Agent], inner: Rect, top: f32, row_h: f32) {
    let (mut chevrons, mut lines) = (Path::new(), Path::new());
    let step = 18.0;
    let offset = (ctx.t * 3.0).fract() * step;
    for (i, a) in rows.iter().enumerate() {
        let r = Rect::new(inner.x, top + i as f32 * row_h, inner.w, row_h);
        let rail = Rect::new(r.x + RAIL_X, r.y + 9.0, r.w - RAIL_X - 6.0, 14.0);
        match a.status {
            AgentStatus::Working => {
                lines.move_to(rail.x, rail.y);
                lines.line_to(rail.right(), rail.y);
                lines.move_to(rail.x, rail.bottom());
                lines.line_to(rail.right(), rail.bottom());
                let mut x = rail.x + offset - step;
                while x < rail.right() - 6.0 {
                    if x >= rail.x {
                        chevrons.move_to(x, rail.y + 3.0);
                        chevrons.line_to(x + 6.0, rail.y + rail.h / 2.0);
                        chevrons.line_to(x, rail.bottom() - 3.0);
                    }
                    x += step;
                }
            }
            AgentStatus::Blocked => {
                lamp(ctx, r.x + 8.0, r.y + 16.0, a.status);
                gfx::hazard(ctx.canvas, rail, ctx.t * 0.8, Palette::RED, Palette::BLACK);
            }
            _ => {}
        }
    }
    ctx.canvas.stroke_path(&lines, &Paint::color(gfx::alpha(Palette::GREEN, 0.35)).with_line_width(1.0));
    ctx.canvas.stroke_path(&chevrons, &Paint::color(gfx::alpha(Palette::GREEN, 0.9)).with_line_width(2.5));
}

fn row_base(ctx: &mut Ctx, r: Rect, a: &Agent, i: usize, max_ram: u64) {
    let rail = Rect::new(r.x + RAIL_X, r.y + 9.0, r.w - RAIL_X - 6.0, 14.0);
    let color = status_color(a.status);
    let fonts = ctx.fonts;
    let usage = ctx.state.usage_of(a).cloned();
    if a.status == AgentStatus::Blocked {
        gfx::fill_rect(ctx.canvas, Rect::new(r.x, r.y + 2.0, r.w, r.h - 4.0), gfx::alpha(Palette::RED, 0.18));
        gfx::fill_rect(ctx.canvas, Rect::new(r.x, r.y + 2.0, 5.0, r.h - 4.0), Palette::RED);
    } else if i % 2 == 1 {
        gfx::fill_rect(ctx.canvas, Rect::new(r.x, r.y + 1.0, r.w, r.h - 2.0), gfx::alpha(Palette::WHITE, 0.025));
    }
    let l1 = r.y + 25.0;
    let l2 = r.y + 47.0;
    if a.status != AgentStatus::Blocked {
        lamp(ctx, r.x + 8.0, r.y + 16.0, a.status);
    }
    let type_label = ctx.state.type_label(a).replace(" · ", " ");
    let c = &mut *ctx.canvas;

    let name_p = gfx::text_paint(
        &fonts.display(),
        28.0,
        if a.status == AgentStatus::Idle { gfx::alpha(Palette::WHITE, 0.6) } else { Palette::WHITE },
    );
    let name = gfx::ellipsize(c, &name_p, &a.display_name().to_uppercase(), NAME_W);
    gfx::text(c, r.x + NAME_X, l1, &name, &name_p);

    let type_p = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.85)).with_letter_spacing(1.0);
    gfx::text(c, r.x + TYPE_X, l1, &gfx::ellipsize(c, &type_p, &type_label, TYPE_W), &type_p);

    let k_p = gfx::text_paint(&fonts.mincho(), 22.0, gfx::alpha(color, 0.85));
    let kw = gfx::text(c, r.x + STATUS_X, l1, a.status.kanji(), &k_p);
    let st_p = gfx::text_paint(&fonts.display(), 24.0, color).with_letter_spacing(1.0);
    gfx::text(c, r.x + STATUS_X + kw + 8.0, l1, a.status.label(), &st_p);

    // sync ratio (free context) + CTX %
    let ratio = usage.as_ref().and_then(|u| u.sync_ratio());
    let pct = usage.as_ref().and_then(|u| u.context_pct());
    sync_bar(c, Rect::new(r.x + SYNC_X, l1 - 15.0, SYNC_W, 14.0), ratio, 10);
    let est = usage.as_ref().is_some_and(|u| u.context_window_estimated);
    let ctx_txt = pct.map_or("--".to_string(), |p| format!("{}{p:.0}%", if est { "~" } else { "" }));
    let ctx_color = if usage::sync_critical(pct) { Palette::RED } else { usage::sync_color(ratio) };
    let ctx_p = gfx::text_paint(&fonts.mono(), 24.0, ctx_color).with_text_align(Align::Right);
    gfx::text(c, r.x + CTX_R, l1, &ctx_txt, &ctx_p);

    let age_p = gfx::text_paint(&fonts.mono(), 26.0, Palette::GREEN).with_text_align(Align::Right);
    gfx::text(c, r.x + AGE_R, l1, &fmt_age(ctx.state.age(a)), &age_p);
    let ram_p = gfx::text_paint(&fonts.mono(), 26.0, gfx::alpha(Palette::WHITE, 0.7)).with_text_align(Align::Right);
    gfx::text(c, r.x + RAM_R, l1, &a.rss_bytes.map_or("--".to_string(), fmt_bytes), &ram_p);

    match a.status {
        AgentStatus::Done => gfx::seg_bar(c, rail, 1.0, 12, gfx::alpha(Palette::CYAN, 0.6), Palette::PANEL),
        AgentStatus::Idle | AgentStatus::Unknown => gfx::stroke_rect(c, rail, gfx::alpha(Palette::DIM, 0.6), 1.0),
        _ => {}
    }

    // line 2: workspace · title · ⎇ branch · cwd (dim) … tokens and cost at the right
    let meta = ctx.state.meta(a);
    let tok = usage.as_ref().map(|u| {
        let out = u.output_tokens;
        let cost = u.cost_usd.map_or(String::new(), |c| format!(" ${c:.2}"));
        format!("↓{} ↑{}{cost}", usage::fmt_tokens(u.total_tokens().saturating_sub(out)), usage::fmt_tokens(out))
    });
    let tok_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::CYAN, 0.85)).with_text_align(Align::Right);
    let tok_w = tok.as_ref().map_or(0.0, |s| gfx::text(c, r.right() - 8.0, l2, s, &tok_p));
    let end = r.right() - 8.0 - tok_w - 24.0;
    let ws_p = gfx::text_paint(&fonts.label(), 22.0, Palette::CYAN);
    let mut x = r.x + NAME_X;
    x += gfx::text(c, x, l2, &gfx::ellipsize(c, &ws_p, &a.workspace_label, 170.0), &ws_p) + 18.0;
    let t_p = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::WHITE, 0.5));
    let title = if a.title.is_empty() { a.tab_label.as_str() } else { a.title.as_str() };
    x += gfx::text(c, x, l2, &gfx::ellipsize(c, &t_p, title, 260.0), &t_p) + 18.0;
    if let Some(m) = meta {
        let b_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::ORANGE, 0.7));
        if let Some(branch) = &m.branch {
            let room = (end - x).min(320.0);
            if room > 60.0 {
                x += gfx::text(c, x, l2, &gfx::ellipsize(c, &b_p, &format!("⎇ {branch}"), room), &b_p) + 18.0;
            }
        }
        let cwd_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.4));
        if end - x > 60.0 {
            gfx::text(c, x, l2, &gfx::ellipsize(c, &cwd_p, &m.cwd, end - x), &cwd_p);
        }
    }
    if r.h >= ROW_H_3 {
        // line 3: pane · tab · session · since · mode · turns · version, RAM bar at the right
        let l3 = r.y + 71.0;
        let d_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::CYAN, 0.45));
        let since = super::header::local_time(a.since);
        let session = meta.and_then(|m| m.session_id.clone()).unwrap_or_else(|| "--".into());
        let mut line = format!(
            "PANE {} · TAB {} · SESSION {} · SINCE {:02}:{:02}:{:02}",
            a.pane_id, a.tab_label, session, since.4, since.5, since.6
        );
        if let Some(u) = &usage {
            if let Some(m) = &u.mode {
                line.push_str(&format!(" · MODE {}", m.to_uppercase()));
            }
            line.push_str(&format!(" · {} TURNS", u.turns));
            if let Some(v) = &u.version {
                line.push_str(&format!(" · v{v}"));
            }
        }
        let fit = gfx::ellipsize(c, &d_p, &line, RAIL_X - NAME_X - 30.0);
        gfx::text(c, r.x + NAME_X, l3, &fit, &d_p);
        if let (Some(b), true) = (a.rss_bytes, max_ram > 0) {
            let bar = Rect::new(r.x + RAIL_X, l3 - 14.0, r.w - RAIL_X - 6.0, 12.0);
            gfx::seg_bar(
                c,
                bar,
                b as f32 / max_ram as f32,
                12,
                gfx::alpha(Palette::WHITE, 0.55),
                gfx::alpha(Palette::DIM, 0.3),
            );
        }
    }
    gfx::line(c, r.x + NAME_X, r.bottom(), r.right(), r.bottom(), gfx::alpha(Palette::ORANGE, 0.12), 1.0);
}

fn standby(ctx: &mut Ctx, r: Rect, idle: &[&Agent]) {
    let l1 = r.y + 25.0;
    lamp(ctx, r.x + 8.0, r.y + 16.0, AgentStatus::Idle);
    let c = &mut *ctx.canvas;
    let k = gfx::text_paint(&ctx.fonts.mincho(), 22.0, Palette::DIM);
    let kw = gfx::text(c, r.x + NAME_X, l1, "待機", &k);
    let p = gfx::text_paint(&ctx.fonts.display(), 28.0, gfx::alpha(Palette::WHITE, 0.6)).with_letter_spacing(2.0);
    gfx::text(c, r.x + NAME_X + kw + 10.0, l1, &format!("STANDBY ×{}", idle.len()), &p);
    let names: Vec<String> = idle.iter().map(|a| a.display_name()).collect();
    let n_p = gfx::text_paint(&ctx.fonts.label(), 22.0, gfx::alpha(Palette::WHITE, 0.5));
    let line = gfx::ellipsize(c, &n_p, &names.join(" · "), r.w - NAME_X - 8.0);
    gfx::text(c, r.x + NAME_X, r.y + 47.0, &line, &n_p);
}

/// SYNC: working agents over the last 60 min (10 s buckets), cyan polyline over a 10-step grid.
fn sync_trace(ctx: &mut Ctx, r: Rect) {
    let c = &mut *ctx.canvas;
    let values = ctx.state.history.working.values(ctx.state.now);
    let now = values.iter().rev().flatten().next().copied().unwrap_or(0.0);
    let peak = values.iter().flatten().fold(0.0f32, |m, &v| m.max(v));

    let label = gfx::text_paint(&ctx.fonts.bold(), 26.0, Palette::ORANGE).with_letter_spacing(3.0);
    let w = gfx::text(c, r.x, r.y + 30.0, "SYNC", &label);
    let k = gfx::text_paint(&ctx.fonts.mincho(), 24.0, Palette::ORANGE);
    gfx::text(c, r.x + w + 10.0, r.y + 29.0, "同期", &k);
    let lp = gfx::text_paint(&ctx.fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.75));
    let vp = gfx::text_paint(&ctx.fonts.mono(), 26.0, Palette::CYAN);
    gfx::text(c, r.x, r.y + 62.0, "ACTIVE NOW", &lp);
    gfx::text(c, r.x + 130.0, r.y + 62.0, &format!("{now:.0}"), &vp);
    gfx::text(c, r.x, r.y + 92.0, "PEAK 60M", &lp);
    gfx::text(c, r.x + 130.0, r.y + 92.0, &format!("{peak:.0}"), &vp);

    let g = Rect::new(r.x + 190.0, r.y + 6.0, r.w - 190.0, r.h - 6.0);
    gfx::stroke_rect(c, g, gfx::alpha(Palette::CYAN, 0.5), 1.5);
    let mut grid = Path::new();
    for i in 1..10 {
        let y = g.y + g.h * i as f32 / 10.0;
        grid.move_to(g.x, y);
        grid.line_to(g.right(), y);
    }
    for i in 1..12 {
        let x = g.x + g.w * i as f32 / 12.0;
        grid.move_to(x, g.y);
        grid.line_to(x, g.bottom());
    }
    c.stroke_path(&grid, &Paint::color(gfx::alpha(Palette::CYAN, 0.12)).with_line_width(1.0));
    let top = (peak * 1.25).ceil().max(4.0);
    let n = values.len().max(2);
    let mut p = Path::new();
    let mut pen = false;
    let mut last = None;
    for (i, v) in values.iter().enumerate() {
        match v {
            Some(v) => {
                let x = g.x + g.w * i as f32 / (n - 1) as f32;
                let y = g.bottom() - g.h * (v / top).clamp(0.0, 1.0);
                if pen {
                    p.line_to(x, y)
                } else {
                    p.move_to(x, y)
                }
                pen = true;
                last = Some((x, y));
            }
            None => pen = false,
        }
    }
    c.stroke_path(&p, &Paint::color(Palette::CYAN).with_line_width(2.5));
    if let Some((x, y)) = last {
        gfx::fill_rect(c, Rect::new(x - 5.0, y - 5.0, 10.0, 10.0), Palette::CYAN);
    }
    if super::telemetry::acquiring(&values) {
        super::telemetry::overlay(ctx, g.inset(2.0), Palette::CYAN);
    }
    let c = &mut *ctx.canvas;
    let sp = gfx::text_paint(&ctx.fonts.mono(), 22.0, gfx::alpha(Palette::CYAN, 0.6));
    gfx::text(c, g.x + 6.0, g.y + 22.0, &format!("{top:.0}"), &sp);
    gfx::text(c, g.x + 6.0, g.bottom() - 6.0, "-60M", &sp);
    gfx::text(c, g.right() - 52.0, g.bottom() - 6.0, "NOW", &sp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_budget_collapses_overflow() {
        // REVISION 1: 1150 design px panel, 52 px two-line rows, header 44, sync trace 120 → 18 rows
        let plan = plan_rows(&[AgentStatus::Working; 20], 0, 1150.0);
        assert_eq!((plan.shown, plan.more, plan.standby_row), (17, 3, false));
        // everything fits: idle agents get their own rows, no STANDBY collapse
        let plan = plan_rows(&[AgentStatus::Working; 3], 6, 1150.0);
        assert_eq!((plan.shown, plan.more, plan.standby_row), (9, 0, false));
    }

    #[test]
    fn overflow_collapses_idle_first_then_more() {
        // 15 + 6 = 21 > 18: idle collapse into STANDBY, all 15 non-idle still fit
        let plan = plan_rows(&[AgentStatus::Working; 15], 6, 1150.0);
        assert_eq!((plan.shown, plan.more, plan.standby_row), (15, 0, true));
        // 20 + 6: STANDBY row + 16 rows + "+4 MORE"
        let plan = plan_rows(&[AgentStatus::Working; 20], 6, 1150.0);
        assert_eq!((plan.shown, plan.more, plan.standby_row), (16, 4, true));
    }
}
