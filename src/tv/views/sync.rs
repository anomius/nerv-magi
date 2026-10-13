//! SYNC 操縦者同期: the Eva "sync test" view of agent usage (plan REVISION 4). One sync-ratio bar
//! per agent (free context, sorted by context used), each with its type plate; rolling fleet
//! token counters; plan-limit gauges with reset countdowns and CAUTION / DANGER tags.

use std::time::Duration;

use femtovg::{Align, Paint};

use crate::state::{fmt_age, Agent, AgentUsage, LimitWindow};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Layout, Rect};
use crate::tv::panels::{alert, plate, status_color, ticker, units, Ctx};
use crate::tv::usage::{self, fmt_tokens};

use super::load::{tag, title_band, Tag};

/// Agents with usage first, most context used first; agents without usage keep roster order after.
pub fn order<'a>(agents: Vec<&'a Agent>, usage_of: impl Fn(&Agent) -> Option<f32>) -> Vec<&'a Agent> {
    let mut v = agents;
    v.sort_by(|a, b| {
        let (pa, pb) = (usage_of(a), usage_of(b));
        pb.unwrap_or(-1.0).total_cmp(&pa.unwrap_or(-1.0))
    });
    v
}

pub fn draw(ctx: &mut Ctx, layout: &Layout) {
    title_band(ctx, layout.header, "PILOT SYNC", "操縦者同期");
    alert::status_strip(ctx, layout.alert);
    if !ctx.layer.base() {
        ticker::draw(ctx, layout.ticker);
        return;
    }
    plate::ruler(ctx.canvas, 30.0, 230.0, 1760.0, true, Palette::CYAN, ctx.fonts);
    bars(ctx, Rect::new(120.0, 214.0, 2420.0, 1796.0));
    fleet(ctx, Rect::new(2572.0, 214.0, 1244.0, 900.0));
    limits(ctx, Rect::new(2572.0, 1130.0, 1244.0, 880.0));
    ticker::draw(ctx, layout.ticker);
}

fn bars(ctx: &mut Ctx, r: Rect) {
    plate::plate(ctx.canvas, r, plate::Cuts::TL_BR, "SYNC RATIO", Some("同期"), Palette::ORANGE, ctx.fonts);
    let st = ctx.state;
    let agents = order(st.herdr.agents.iter().collect(), |a| st.usage_of(a).and_then(|u| u.context_pct()));
    let fonts = ctx.fonts;
    let c = &mut *ctx.canvas;
    let known = agents.iter().filter(|a| st.usage_of(a).is_some()).count();
    let note = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_text_align(Align::Right);
    let s = format!("{known}/{} REPORTING · FREE CONTEXT = SYNC", agents.len());
    let w = gfx::text_width(c, &s, &note);
    gfx::fill_rect(c, Rect::new(r.right() - 70.0 - w, r.y + 2.0, w + 24.0, 30.0), Palette::PANEL);
    gfx::text(c, r.right() - 58.0, r.y + 26.0, &s, &note);

    let area = Rect::new(r.x + 30.0, r.y + plate::PLATE_TOP + 16.0, r.w - 60.0, r.h - plate::PLATE_TOP - 46.0);
    if agents.is_empty() {
        let p = gfx::text_paint(&fonts.bold(), 48.0, Palette::DIM).with_letter_spacing(5.0);
        gfx::text_center(c, area, "NO PILOTS", &p);
        return;
    }
    let row_h = (area.h / agents.len() as f32).clamp(90.0, 180.0);
    let shown = ((area.h / row_h).floor() as usize).min(agents.len());
    for (i, a) in agents.iter().take(shown).enumerate() {
        let u = st.usage_of(a).cloned();
        sync_row(c, fonts, Rect::new(area.x, area.y + i as f32 * row_h, area.w, row_h - 10.0), a, u.as_ref(), st.age(a), ctx.t);
    }
    if agents.len() > shown {
        let p = gfx::text_paint(&fonts.bold(), 28.0, Palette::ORANGE);
        gfx::text(c, area.x, area.bottom() + 30.0, &format!("+{} MORE", agents.len() - shown), &p);
    }
}

fn sync_row(c: &mut gfx::Cv, fonts: &crate::tv::fonts::Fonts, r: Rect, a: &Agent, u: Option<&AgentUsage>, age: Duration, t: f32) {
    // type plate: kind / model / effort · mode
    let tp = Rect::new(r.x, r.y, 400.0, r.h);
    let outline = plate::chamfer_path(tp, plate::Cuts::TL_BR, 18.0);
    c.fill_path(&outline, &Paint::color(gfx::alpha(Palette::CYAN, 0.06)));
    c.stroke_path(&outline, &Paint::color(gfx::alpha(Palette::CYAN, 0.7)).with_line_width(2.0));
    let model = u.and_then(|u| u.model.as_deref()).map(crate::state::model_short).unwrap_or_else(|| "MODEL ?".into());
    let effort = u.and_then(|u| u.effort.clone()).unwrap_or_default().to_uppercase();
    let mode = u.and_then(|u| u.mode.clone()).unwrap_or_default().to_uppercase();
    let line_h = (r.h / 3.6).min(42.0);
    gfx::text(c, tp.x + 20.0, tp.y + line_h, &a.kind.to_uppercase(), &gfx::text_paint(&fonts.bold(), 26.0, Palette::CYAN).with_letter_spacing(3.0));
    let mp = gfx::text_paint(&fonts.display(), 34.0, Palette::WHITE);
    gfx::text(c, tp.x + 20.0, tp.y + line_h * 2.1, &gfx::ellipsize(c, &mp, &model, tp.w - 40.0), &mp);
    let ep = gfx::text_paint(&fonts.label(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_letter_spacing(1.5);
    let em = [effort, mode].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
    if r.h > 110.0 {
        gfx::text(c, tp.x + 20.0, tp.y + line_h * 3.1, &gfx::ellipsize(c, &ep, &em, tp.w - 40.0), &ep);
    }

    // name, workspace, status
    let x = tp.right() + 30.0;
    let readout_w = 360.0;
    let bar_w = r.right() - readout_w - x - 20.0;
    let np = gfx::text_paint(&fonts.display(), 34.0, Palette::WHITE).with_letter_spacing(1.0);
    let nw = gfx::text(c, x, r.y + 34.0, &gfx::ellipsize(c, &np, &a.display_name().to_uppercase(), bar_w * 0.5), &np);
    let wp = gfx::text_paint(&fonts.label(), 26.0, Palette::CYAN);
    let ww = gfx::text(c, x + nw + 20.0, r.y + 34.0, &gfx::ellipsize(c, &wp, &a.workspace_label, bar_w * 0.25), &wp);
    let sp = gfx::text_paint(&fonts.display(), 24.0, status_color(a.status)).with_letter_spacing(1.5);
    gfx::text(c, x + nw + ww + 40.0, r.y + 34.0, a.status.label(), &sp);

    // the sync bar
    let ratio = u.and_then(|u| u.sync_ratio());
    let pct = u.and_then(|u| u.context_pct());
    let bar = Rect::new(x, r.y + 48.0, bar_w, (r.h * 0.28).clamp(22.0, 44.0));
    units::sync_bar(c, bar, ratio, 40);
    // rulers at 20 / 50 % (the colour thresholds)
    for mark in [0.2, 0.5] {
        let mx = bar.x + bar.w * mark;
        gfx::line(c, mx, bar.y - 6.0, mx, bar.bottom() + 6.0, gfx::alpha(Palette::WHITE, 0.5), 2.0);
    }
    let critical = usage::sync_critical(pct);
    if critical {
        let pulse = 0.6 + 0.4 * (t * std::f32::consts::TAU).cos();
        let tag_p = gfx::text_paint(&fonts.bold(), 26.0, Palette::BLACK).with_letter_spacing(2.0);
        let tw = gfx::text_width(c, "SYNC CRITICAL", &tag_p) + 24.0;
        let tg = Rect::new(bar.right() - tw, r.y + 6.0, tw, 34.0);
        gfx::fill_rect(c, tg, gfx::alpha(Palette::RED, pulse));
        gfx::text_center(c, tg, "SYNC CRITICAL", &tag_p);
    }

    // detail line: turns · session age · tokens · cost
    let dp = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::WHITE, 0.75));
    let detail = match u {
        Some(u) => {
            let session = u.session_started.and_then(|s| s.elapsed().ok()).map_or("--".into(), fmt_age);
            let mut s = format!(
                "{} TURNS · SESSION {} · IN {} · OUT {} · CACHE {}",
                u.turns,
                session,
                fmt_tokens(u.input_tokens),
                fmt_tokens(u.output_tokens),
                fmt_tokens(u.cache_read_tokens)
            );
            if let Some(cost) = u.cost_usd {
                s.push_str(&format!(" · ${cost:.2}"));
            }
            s
        }
        None => format!("NO USAGE DATA · IN STATE {}", fmt_age(age)),
    };
    if r.h > 100.0 {
        gfx::text(c, x, bar.bottom() + 34.0, &gfx::ellipsize(c, &dp, &detail, bar_w), &dp);
    }

    // readout: DSEG sync %, CTX used / window
    let rx = r.right() - readout_w + 20.0;
    let color = if critical { Palette::RED } else { usage::sync_color(ratio) };
    let seg = gfx::text_paint(&fonts.segment(), (r.h * 0.5).clamp(48.0, 84.0), color);
    let txt = ratio.map_or("--.-".to_string(), |v| format!("{v:4.1}"));
    let base = r.y + (r.h * 0.5).clamp(48.0, 84.0) + 4.0;
    gfx::text(c, rx, base, "88.8", &seg.clone().with_color(gfx::alpha(color, 0.10)));
    let w = gfx::glow_text(c, rx, base, &txt, &seg, gfx::alpha(color, 0.6), 8.0);
    gfx::text(c, rx + w + 10.0, base, "%", &gfx::text_paint(&fonts.display(), 32.0, color));
    if let Some(u) = u {
        let cp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.7));
        let est = if u.context_window_estimated { "~" } else { "" };
        let ctx_line = format!("CTX {}{} / {}", est, u.context_used.map_or("--".into(), fmt_tokens), u.context_window.map_or("--".into(), fmt_tokens));
        gfx::text(c, rx, (base + 32.0).min(r.bottom() - 4.0), &ctx_line, &cp);
    }
    gfx::line(c, r.x, r.bottom() + 5.0, r.right(), r.bottom() + 5.0, gfx::alpha(Palette::ORANGE, 0.15), 1.0);
}

fn fleet(ctx: &mut Ctx, r: Rect) {
    plate::plate(ctx.canvas, r, plate::Cuts::TR_BL, "FLEET TOKENS", Some("情報"), Palette::ORANGE, ctx.fonts);
    let st = ctx.state;
    let all: Vec<&AgentUsage> = st.usage.values().collect();
    let total: u64 = all.iter().map(|u| u.total_tokens()).sum();
    let output: u64 = all.iter().map(|u| u.output_tokens).sum();
    let cache: u64 = all.iter().map(|u| u.cache_read_tokens).sum();
    let reasoning: u64 = all.iter().map(|u| u.reasoning_tokens).sum();
    let costs: Vec<f64> = all.iter().filter_map(|u| u.cost_usd).collect();
    let rows: [(&str, &str, u64); 4] = [("TOTAL TOKENS", "tot", total), ("OUTPUT", "out", output), ("CACHE READ", "cache", cache), ("REASONING", "reason", reasoning)];
    let mut values = Vec::new();
    for (label, key, v) in rows {
        values.push((label, ctx.anim.roll_u64(&format!("fleet_{key}"), v, ctx.t)));
    }
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let lp = gfx::text_paint(&fonts.label(), 28.0, gfx::alpha(Palette::ORANGE, 0.9)).with_letter_spacing(3.0);
    let mut y = r.y + plate::PLATE_TOP + 40.0;
    for (i, (label, v)) in values.iter().enumerate() {
        gfx::text(c, r.x + 40.0, y, label, &lp);
        let size = if i == 0 { 104.0 } else { 72.0 };
        let color = if i == 0 { Palette::LCD } else { Palette::GREEN };
        let seg = gfx::text_paint(&fonts.segment(), size, color).with_text_align(Align::Right);
        let base = y + size + 10.0;
        let digits = v.to_string();
        let ghost = "8".repeat(digits.len().max(9));
        gfx::text(c, r.right() - 40.0, base, &ghost, &seg.clone().with_color(gfx::alpha(color, 0.08)));
        gfx::glow_text(c, r.right() - 40.0, base, &digits, &seg, gfx::alpha(color, 0.5), 8.0);
        y = base + 46.0;
    }
    let cost_p = gfx::text_paint(&fonts.mono(), 32.0, Palette::WHITE);
    let cost = if costs.is_empty() { "COST --  (needs the statusline hook)".to_string() } else { format!("COST ${:.2}  ({} of {} sessions)", costs.iter().sum::<f64>(), costs.len(), all.len()) };
    gfx::text(c, r.x + 40.0, r.bottom() - 30.0, &gfx::ellipsize(c, &cost_p, &cost, r.w - 80.0), &cost_p);
}

fn limits(ctx: &mut Ctx, r: Rect) {
    plate::plate(ctx.canvas, r, plate::Cuts::BR, "PLAN LIMITS", Some("供給"), Palette::ORANGE, ctx.fonts);
    let st = ctx.state;
    let gauges: Vec<(String, Option<String>, Option<String>, LimitWindow)> = st
        .limits
        .iter()
        .flat_map(|l| l.windows.iter().map(move |w| (l.provider.to_uppercase(), l.plan.clone(), l.credits.clone(), w.clone())))
        .collect();
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let area = Rect::new(r.x + 40.0, r.y + plate::PLATE_TOP + 20.0, r.w - 80.0, r.h - plate::PLATE_TOP - 50.0);
    if gauges.is_empty() {
        let p = gfx::text_paint(&fonts.bold(), 40.0, Palette::DIM).with_letter_spacing(4.0);
        gfx::text_center(c, area, "NO PLAN LIMIT DATA", &p);
        return;
    }
    let h = (area.h / gauges.len() as f32).min(260.0);
    for (i, (provider, plan, credits, w)) in gauges.iter().enumerate() {
        let g = Rect::new(area.x, area.y + i as f32 * h, area.w, h - 20.0);
        let tg = tag(w.used_pct, 80.0, 95.0, true);
        let color = match tg {
            Tag::Ok => Palette::CYAN,
            Tag::Caution => Palette::AMBER,
            Tag::Danger => Palette::RED,
        };
        let hp = gfx::text_paint(&fonts.bold(), 34.0, Palette::ORANGE).with_letter_spacing(3.0);
        let mut head = format!("{provider} {}", w.label);
        if let Some(p) = plan {
            head.push_str(&format!(" · {}", p.to_uppercase()));
        }
        gfx::text(c, g.x, g.y + 34.0, &head, &hp);
        let seg = gfx::text_paint(&fonts.segment(), 72.0, color).with_text_align(Align::Right);
        gfx::glow_text(c, g.right() - 46.0, g.y + 80.0, &format!("{:.0}", w.used_pct), &seg, gfx::alpha(color, 0.6), 8.0);
        gfx::text(c, g.right(), g.y + 80.0, "%", &gfx::text_paint(&fonts.display(), 32.0, color).with_text_align(Align::Right));
        let bar = Rect::new(g.x, g.y + 52.0, g.w - 260.0, 30.0);
        gfx::seg_bar(c, bar, w.used_pct / 100.0, 40, color, gfx::alpha(Palette::DIM, 0.3));
        for mark in [0.8, 0.95] {
            let mx = bar.x + bar.w * mark;
            gfx::line(c, mx, bar.y - 6.0, mx, bar.bottom() + 6.0, gfx::alpha(Palette::WHITE, 0.6), 2.0);
        }
        let reset = w.resets_at.map_or("RESET --".to_string(), |at| format!("RESET IN {}", usage::fmt_reset(at.duration_since(st.now).unwrap_or_default())));
        let mut info = reset;
        if let Some(cr) = credits {
            info.push_str(&format!(" · CREDITS {cr}"));
        }
        gfx::text(c, g.x, bar.bottom() + 36.0, &info, &gfx::text_paint(&fonts.mono(), 26.0, gfx::alpha(Palette::WHITE, 0.8)));
        let (word, fill) = match tg {
            Tag::Ok => ("NOMINAL", None),
            Tag::Caution => ("CAUTION", Some(Palette::AMBER)),
            Tag::Danger => ("DANGER", Some(Palette::RED)),
        };
        let tr = Rect::new(g.right() - 200.0, bar.bottom() + 8.0, 200.0, 38.0);
        match fill {
            Some(f) => gfx::fill_rect(c, tr, f),
            None => gfx::stroke_rect(c, tr, gfx::alpha(Palette::GREEN, 0.7), 2.0),
        }
        gfx::text_center(c, tr, word, &gfx::text_paint(&fonts.bold(), 26.0, if fill.is_some() { Palette::BLACK } else { gfx::alpha(Palette::GREEN, 0.8) }).with_letter_spacing(3.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::agent;
    use crate::state::AgentStatus;

    #[test]
    fn most_context_first_unknown_last() {
        let a = agent("p1", "a", AgentStatus::Working, "w");
        let b = agent("p2", "b", AgentStatus::Working, "w");
        let c = agent("p3", "c", AgentStatus::Idle, "w");
        let pct = |x: &Agent| match x.pane_id.as_str() {
            "p1" => Some(20.0),
            "p2" => Some(90.0),
            _ => None,
        };
        let names: Vec<&str> = order(vec![&a, &b, &c], pct).iter().map(|x| x.pane_id.as_str()).collect();
        assert_eq!(names, ["p2", "p1", "p3"]);
    }
}
