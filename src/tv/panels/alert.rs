//! The EMERGENCY strip (only at alert level Emergency) and the full-screen red flash.

use femtovg::{Paint, Path};

use super::{status_color, Ctx};
use crate::state::{fmt_age, AgentStatus, AlertLevel, HerdrLink};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Rect, DESIGN_H, DESIGN_W};

/// The line under 緊急 EMERGENCY: who needs a pilot, or why MAGI disagrees.
pub fn reason(ctx: &Ctx) -> String {
    let s = ctx.state;
    if let Some(a) = s.sorted_agents().into_iter().find(|a| a.status == AgentStatus::Blocked) {
        return format!("PILOT INPUT REQUIRED: {} · {} · {}", a.display_name(), a.workspace_label, fmt_age(s.age(a)));
    }
    match &s.herdr.link {
        HerdrLink::Lost { .. } => return "MAGI LINK LOST · RETRY".into(),
        HerdrLink::Connecting => return "MAGI LINK · CONNECTING".into(),
        HerdrLink::Linked => {}
    }
    let denied: Vec<&str> = s.magi(&ctx.config.thresholds).iter().filter(|v| !v.approve).map(|v| v.subject).collect();
    if denied.is_empty() {
        "ALL UNITS APPROVE".into()
    } else {
        format!("MAGI DENIAL: {}", denied.join(" · "))
    }
}

/// Status icon as a shape (the bundled fonts have no ✔): ▲ blocked, ● working, ✔ done, ○ idle.
pub fn status_icon(c: &mut gfx::Cv, x: f32, cy: f32, size: f32, s: AgentStatus) {
    let color = status_color(s);
    let h = size / 2.0;
    let mut p = Path::new();
    match s {
        AgentStatus::Blocked => {
            p.move_to(x + h, cy - h);
            p.line_to(x + size, cy + h);
            p.line_to(x, cy + h);
            p.close();
            c.fill_path(&p, &Paint::color(color));
        }
        AgentStatus::Working | AgentStatus::Unknown => {
            p.circle(x + h, cy, h);
            c.fill_path(&p, &Paint::color(color));
        }
        AgentStatus::Done => {
            p.move_to(x, cy);
            p.line_to(x + size * 0.38, cy + h * 0.8);
            p.line_to(x + size, cy - h);
            c.stroke_path(&p, &Paint::color(color).with_line_width(size * 0.2));
        }
        AgentStatus::Idle => {
            p.circle(x + h, cy, h - 1.5);
            c.stroke_path(&p, &Paint::color(color).with_line_width(3.0));
        }
    }
}

/// The alert row on MAIN/LOAD: counts per status · MAGI verdict · herdr link · scene rotation ·
/// sources · newest event. (EMERGENCY is its own view.)
pub fn status_strip(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    let st = ctx.state;
    let c = &mut *ctx.canvas;
    // the `? HELP` hint owns the right end; everything else stops before it
    const HINT_W: f32 = 170.0;
    let full = r;
    let base = r.y + r.h / 2.0 + 10.0;
    // angled band (◢ ends, like the ticker) with a slanted orange tab holding the first label
    let mut band = Path::new();
    band.move_to(slant_x(r, STRIP_SLOPE, r.y, true), r.y);
    band.line_to(r.right(), r.y);
    band.line_to(r.right() - STRIP_SLOPE, r.bottom());
    band.line_to(r.x, r.bottom());
    band.close();
    c.fill_path(&band, &Paint::color(Palette::PANEL));
    c.stroke_path(&band, &Paint::color(gfx::alpha(Palette::ORANGE, 0.6)).with_line_width(1.5));
    let lp = gfx::text_paint(&ctx.fonts.label(), 24.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(2.0);
    let tab_text = gfx::text_paint(&ctx.fonts.label(), 24.0, Palette::BLACK).with_letter_spacing(2.0);
    let text_x = slant_x(r, STRIP_SLOPE, base, true) + 22.0;
    let tab_w = text_x - r.x + gfx::text_width(c, "PILOT STATUS", &tab_text) + 30.0;
    let mut tab = Path::new();
    tab.move_to(r.x + STRIP_SLOPE, r.y);
    tab.line_to(r.x + tab_w, r.y);
    tab.line_to(r.x + tab_w - STRIP_SLOPE, r.bottom());
    tab.line_to(r.x, r.bottom());
    tab.close();
    c.fill_path(&tab, &Paint::color(gfx::alpha(Palette::ORANGE, 0.9)));
    gfx::text(c, text_x, base, "PILOT STATUS", &tab_text);
    let hp = gfx::text_paint(&ctx.fonts.label(), 24.0, gfx::alpha(Palette::WHITE, 0.4))
        .with_letter_spacing(2.0)
        .with_text_align(femtovg::Align::Right);
    gfx::text(c, slant_x(full, STRIP_SLOPE, base, false) - 20.0, base, "? HELP", &hp);
    let cy = r.y + r.h / 2.0;
    let n = st.counts();
    let mut x = r.x + tab_w + 12.0;
    for (count, s) in [
        (n.blocked, AgentStatus::Blocked),
        (n.working, AgentStatus::Working),
        (n.done, AgentStatus::Done),
        (n.idle, AgentStatus::Idle),
    ] {
        let color = if count > 0 { status_color(s) } else { Palette::DIM };
        status_icon(c, x, cy, 24.0, s);
        x += 36.0;
        let np = gfx::text_paint(&ctx.fonts.mono(), 28.0, color);
        x += gfx::text(c, x, base + 1.0, &count.to_string(), &np) + 10.0;
        let wp = gfx::text_paint(&ctx.fonts.display(), 26.0, color).with_letter_spacing(1.5);
        x += gfx::text(c, x, base, s.label(), &wp) + 40.0;
    }
    sep(c, x - 20.0, r);

    let votes = st.magi(&ctx.config.thresholds);
    let approvals = votes.iter().filter(|v| v.approve).count();
    let level = st.alert_level(&ctx.config.thresholds);
    let (word, color) = match level {
        AlertLevel::Nominal => ("NOMINAL", Palette::GREEN),
        AlertLevel::Caution => ("CAUTION", Palette::AMBER),
        AlertLevel::Emergency => ("EMERGENCY", Palette::RED),
    };
    x += 20.0;
    x += gfx::text(c, x, base, "MAGI", &lp) + 16.0;
    for v in &votes {
        let k = gfx::text_paint(&ctx.fonts.mincho(), 26.0, if v.approve { Palette::GREEN } else { Palette::RED });
        let up = gfx::text_paint(&ctx.fonts.display(), 22.0, gfx::alpha(Palette::CYAN, 0.9));
        x += gfx::text(c, x, base, v.unit, &up) + 8.0;
        x += gfx::text(c, x, base, if v.approve { "承認" } else { "否定" }, &k) + 22.0;
    }
    let vp = gfx::text_paint(&ctx.fonts.bold(), 28.0, color).with_letter_spacing(2.0);
    x += gfx::text(c, x, base, &format!("{approvals}/3 {word}"), &vp) + 40.0;
    sep(c, x - 20.0, r);

    x += 20.0;
    x += gfx::text(c, x, base, "HERDR LINK", &lp) + 14.0;
    let (link, lc) = match &st.herdr.link {
        HerdrLink::Linked => ("LINKED".to_string(), Palette::GREEN),
        HerdrLink::Connecting => ("CONNECTING".to_string(), Palette::AMBER),
        HerdrLink::Lost { since, .. } => {
            (format!("LOST {}", fmt_age(st.now.duration_since(*since).unwrap_or_default())), Palette::AMBER)
        }
    };
    x += gfx::text(c, x, base, &link, &gfx::text_paint(&ctx.fonts.mono(), 26.0, lc)) + 24.0;
    let info = format!("{} AGENTS · {} WS", st.herdr.agents.len(), st.herdr.workspaces.len());
    x +=
        gfx::text(c, x, base, &info, &gfx::text_paint(&ctx.fonts.mono(), 24.0, gfx::alpha(Palette::WHITE, 0.6))) + 40.0;
    sep(c, x - 20.0, r);

    x += 20.0;
    x += gfx::text(c, x, base, "VIEW", &lp) + 14.0;
    let view = match ctx.scene.countdown {
        Some((next, secs)) => {
            format!("{} · {} · {} IN {:.0}s", ctx.scene.mode.label(), ctx.scene.scene.name(), next.name(), secs.ceil())
        }
        None => ctx.scene.mode.label(),
    };
    let pinned = ctx.scene.mode != crate::control::ViewMode::Auto;
    x += gfx::text(
        c,
        x,
        base,
        &view,
        &gfx::text_paint(&ctx.fonts.mono(), 24.0, if pinned { Palette::AMBER } else { Palette::CYAN }),
    ) + 40.0;
    sep(c, x - 20.0, r);
    let limits = crate::tv::usage::limits_line(&st.limits, st.now);
    if !limits.is_empty() {
        x += 20.0;
        x += gfx::text(c, x, base, "LIMITS", &lp) + 14.0;
        let worst = st.limits.iter().flat_map(|l| l.windows.iter()).map(|w| w.used_pct).fold(0.0f32, f32::max);
        let lc = if worst >= 95.0 {
            Palette::RED
        } else if worst >= 80.0 {
            Palette::AMBER
        } else {
            gfx::alpha(Palette::WHITE, 0.85)
        };
        let lpaint = gfx::text_paint(&ctx.fonts.mono(), 24.0, lc);
        let fit = gfx::ellipsize(c, &lpaint, &limits, (r.right() - HINT_W - x).clamp(0.0, 980.0));
        x += gfx::text(c, x, base, &fit, &lpaint) + 40.0;
        sep(c, x - 20.0, r);
    }
    if !st.offline.is_empty() {
        x += 20.0;
        x += gfx::text(c, x, base, "SOURCES", &lp) + 14.0;
        let names: Vec<String> = st.offline.keys().map(|k| format!("{k:?}").to_uppercase()).collect();
        let sp = gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::AMBER);
        let fit =
            gfx::ellipsize(c, &sp, &format!("OFFLINE {}", names.join(" ")), (r.right() - HINT_W - x).clamp(0.0, 300.0));
        x += gfx::text(c, x, base, &fit, &sp) + 40.0;
        sep(c, x - 20.0, r);
    }

    let r = Rect::new(r.x, r.y, r.w - HINT_W, r.h);

    // newest status transition, when there is room left
    if r.right() - x < 320.0 {
        return;
    }
    x += 20.0;
    x += gfx::text(c, x, base, "LAST EVENT", &lp) + 14.0;
    let ev = match st.ticker.front() {
        Some(e) => {
            let (.., h, m, s) = super::header::local_time(e.at);
            (format!("{h:02}:{m:02}:{s:02} {} → {}", e.agent, e.to.label()), status_color(e.to))
        }
        None => ("NONE".to_string(), Palette::DIM),
    };
    let ep = gfx::text_paint(&ctx.fonts.mono(), 24.0, ev.1);
    if r.right() - 16.0 - x > 80.0 {
        let fit = gfx::ellipsize(c, &ep, &ev.0, r.right() - 16.0 - x);
        gfx::text(c, x, base, &fit, &ep);
    }
}

/// Run of the status strip's sloped ends (its height: 45°).
const STRIP_SLOPE: f32 = 56.0;

/// x of the strip's sloped left (or right) edge at height `y`: the left edge runs from
/// (x + slope, top) to (x, bottom), the right one from (right, top) to (right − slope, bottom).
pub fn slant_x(r: Rect, slope: f32, y: f32, left: bool) -> f32 {
    let down = ((y - r.y) / r.h).clamp(0.0, 1.0) * slope;
    if left {
        r.x + slope - down
    } else {
        r.right() - down
    }
}

fn sep(c: &mut gfx::Cv, x: f32, r: Rect) {
    gfx::line(c, x, r.y + 10.0, x, r.bottom() - 10.0, gfx::alpha(Palette::ORANGE, 0.5), 1.5);
}

/// Full-screen red overlay that fades out over half a second after an agent blocks.
pub fn flash(ctx: &mut Ctx) {
    let a = ctx.anim.flash_alpha(ctx.t);
    if a > 0.0 {
        gfx::fill_rect(ctx.canvas, Rect::new(0.0, 0.0, DESIGN_W, DESIGN_H), gfx::alpha(Palette::RED, a));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slanted_edges_of_the_strip() {
        let r = Rect::new(24.0, 136.0, 3792.0, 56.0);
        // left edge: from (x + slope, top) down to (x, bottom); right edge: (right, top) to (right - slope, bottom)
        assert_eq!(slant_x(r, 56.0, r.y, true), r.x + 56.0);
        assert_eq!(slant_x(r, 56.0, r.bottom(), true), r.x);
        assert_eq!(slant_x(r, 56.0, r.y, false), r.right());
        assert_eq!(slant_x(r, 56.0, r.bottom(), false), r.right() - 56.0);
        assert_eq!(slant_x(r, 56.0, r.y + 28.0, true), r.x + 28.0);
    }
}
