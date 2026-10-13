//! MAGI: the three-unit vote (MELCHIOR·1 compute, BALTHASAR·2 memory, CASPER·3 pilots), drawn as
//! trapezoid plates around a hub, with circuit traces out to the panels each unit judges.

use femtovg::{Align, Paint, Path};

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::{AlertLevel, MagiVote};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Layout, Rect};

const PLATE_W: f32 = 280.0;
const PLATE_H: f32 = 176.0;
/// The cut on the corner(s) facing the hub.
const HUB_CUT: f32 = 56.0;

/// Plate outline with big cuts on the hub-facing corners: [top-left, top-right, bottom-right, bottom-left].
fn unit_path(r: Rect, cuts: [f32; 4]) -> Path {
    let [tl, tr, br, bl] = cuts;
    let mut p = Path::new();
    p.move_to(r.x + tl, r.y);
    p.line_to(r.right() - tr, r.y);
    p.line_to(r.right(), r.y + tr);
    p.line_to(r.right(), r.bottom() - br);
    p.line_to(r.right() - br, r.bottom());
    p.line_to(r.x + bl, r.bottom());
    p.line_to(r.x, r.bottom() - bl);
    p.line_to(r.x, r.y + tl);
    p.close();
    p
}

/// Unit plate rects in vote order [MELCHIOR·1, BALTHASAR·2, CASPER·3].
pub fn plates(r: Rect) -> [Rect; 3] {
    let area = Rect::new(r.x + 370.0, r.y + plate::PLATE_TOP + 10.0, r.w - 390.0, r.h - plate::PLATE_TOP - 30.0);
    let cx = area.x + area.w / 2.0;
    let top = Rect::new(cx - PLATE_W / 2.0, area.y, PLATE_W, PLATE_H);
    let left = Rect::new(area.x, area.bottom() - PLATE_H, PLATE_W, PLATE_H);
    let right = Rect::new(area.right() - PLATE_W, area.bottom() - PLATE_H, PLATE_W, PLATE_H);
    [right, top, left]
}

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    plate::plate(ctx.canvas, r, Cuts::TR_BL, "MAGI", Some("審議"), Palette::ORANGE, ctx.fonts);
    let th = &ctx.config.thresholds;
    let votes = ctx.state.magi(th);
    let level = ctx.state.alert_level(th);
    deliberation(ctx, Rect::new(r.x + 30.0, r.y + plate::PLATE_TOP + 4.0, 320.0, r.h - plate::PLATE_TOP - 30.0), &votes, level);

    let [mel, bal, cas] = plates(r);
    let hub = (bal.x + bal.w / 2.0, (bal.bottom() + cas.y) / 2.0);
    let wire = Paint::color(gfx::alpha(Palette::CYAN, 0.7)).with_line_width(3.0);
    let mut p = Path::new();
    p.move_to(hub.0, hub.1 - 28.0);
    p.line_to(hub.0, bal.bottom());
    p.move_to(hub.0 - 80.0, hub.1 + 10.0);
    p.line_to(cas.right() - HUB_CUT / 2.0, cas.y + HUB_CUT / 2.0);
    p.move_to(hub.0 + 80.0, hub.1 + 10.0);
    p.line_to(mel.x + HUB_CUT / 2.0, mel.y + HUB_CUT / 2.0);
    ctx.canvas.stroke_path(&p, &wire);
    let hp = gfx::text_paint(&ctx.fonts.bold(), 44.0, Palette::CYAN).with_letter_spacing(8.0);
    gfx::text_center(ctx.canvas, Rect::new(hub.0 - 90.0, hub.1 - 28.0, 180.0, 56.0), "MAGI", &hp);

    // hub-facing cuts: BALTHASAR's bottom corners, CASPER's top-right, MELCHIOR's top-left
    unit(ctx, bal, &votes[1], 1, [0.0, 0.0, HUB_CUT, HUB_CUT]);
    unit(ctx, cas, &votes[2], 2, [0.0, HUB_CUT, 0.0, 0.0]);
    unit(ctx, mel, &votes[0], 0, [HUB_CUT, 0.0, 0.0, 0.0]);
}

fn unit(ctx: &mut Ctx, r: Rect, v: &MagiVote, i: usize, cuts: [f32; 4]) {
    let visible = ctx.anim.vote(i, v.approve, ctx.t);
    let (verdict, color) = if v.approve { ("承認", Palette::GREEN) } else { ("否定", Palette::RED) };
    let path = unit_path(r, cuts);
    ctx.canvas.fill_path(&path, &Paint::color(gfx::alpha(color, 0.10)));
    ctx.canvas.stroke_path(&path, &Paint::color(Palette::CYAN).with_line_width(3.0));
    let c = &mut *ctx.canvas;
    let mid = r.x + r.w / 2.0;
    let np = gfx::text_paint(&ctx.fonts.display(), 28.0, Palette::CYAN).with_letter_spacing(2.0).with_text_align(Align::Center);
    gfx::text(c, mid, r.y + 38.0, v.unit, &np);
    let subj = gfx::text_paint(&ctx.fonts.label(), 22.0, gfx::alpha(Palette::CYAN, 0.6)).with_text_align(Align::Center).with_letter_spacing(3.0);
    gfx::text(c, mid, r.y + 60.0, v.subject, &subj);
    if visible {
        let vp = gfx::text_paint(&ctx.fonts.mincho(), 72.0, color).with_text_align(Align::Center);
        gfx::glow_text(c, mid, r.y + 136.0, verdict, &vp, gfx::alpha(color, 0.9), 12.0);
    }
    let sp = gfx::text_paint(&ctx.fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.75)).with_text_align(Align::Center);
    let inset = cuts[2].max(cuts[3]);
    let reason = gfx::ellipsize(c, &sp, &v.reason, r.w - 24.0 - inset);
    gfx::text(c, mid, r.y + 166.0, &reason, &sp);
}

fn deliberation(ctx: &mut Ctx, r: Rect, votes: &[MagiVote; 3], level: AlertLevel) {
    let c = &mut *ctx.canvas;
    let dim = gfx::alpha(Palette::ORANGE, 0.75);
    let p = gfx::text_paint(&ctx.fonts.mono(), 22.0, dim);
    let th = &ctx.config.thresholds;
    let mut y = r.y + 22.0;
    for line in ["CODE : 258", "FILE : MAGI_SYS", "EXTENSION : 4096", "EX_MODE : OFF", "PRIORITY : AAA"] {
        gfx::text(c, r.x, y, line, &p);
        y += 28.0;
    }
    y += 10.0;
    gfx::text(c, r.x, y, "RESULT OF THE DELIBERATION", &p.clone().with_color(Palette::ORANGE));
    y += 8.0;
    gfx::line(c, r.x, y, r.x + r.w, y, gfx::alpha(Palette::ORANGE, 0.5), 1.5);
    y += 30.0;
    let rules = [
        format!("< {:.0}% · < {:.0}°C", th.cpu_max * 100.0, th.cpu_temp_max_c),
        format!("AVAIL ≥ {:.1}G", th.mem_min_available_bytes as f64 / 1e9),
        "LINKED · NONE BLOCKED".to_string(),
    ];
    for (v, rule) in votes.iter().zip(rules) {
        let color = if v.approve { Palette::GREEN } else { Palette::RED };
        let up = gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::CYAN);
        let w = gfx::text(c, r.x, y, v.unit, &up);
        let kp = gfx::text_paint(&ctx.fonts.mincho(), 24.0, color);
        gfx::text(c, r.x + w + 12.0, y, if v.approve { "承認" } else { "否定" }, &kp);
        y += 28.0;
        let rp = gfx::text_paint(&ctx.fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.8));
        gfx::text(c, r.x + 18.0, y, &gfx::ellipsize(c, &rp, &v.reason, r.w - 18.0), &rp);
        y += 26.0;
        let qp = gfx::text_paint(&ctx.fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.4));
        gfx::text(c, r.x + 18.0, y, &rule, &qp);
        y += 36.0;
    }
    let approvals = votes.iter().filter(|v| v.approve).count();
    let (word, color) = match level {
        AlertLevel::Nominal => ("NOMINAL", Palette::GREEN),
        AlertLevel::Caution => ("CAUTION", Palette::AMBER),
        AlertLevel::Emergency => ("EMERGENCY", Palette::RED),
    };
    let vp = gfx::text_paint(&ctx.fonts.bold(), 40.0, color).with_letter_spacing(3.0);
    gfx::text(c, r.x, r.bottom() - 6.0, &format!("{approvals}/3 {word}"), &vp);
}

/// Circuit traces from each unit plate to the panel it judges, routed through the gutters:
/// CASPER·3 → pilot roster, MELCHIOR·1 → telemetry, BALTHASAR·2 → processes.
pub fn traces(ctx: &mut Ctx, l: &Layout) {
    if !ctx.layer.base() {
        return;
    }
    let [mel, bal, cas] = plates(l.magi);
    let gap_y = l.magi.bottom() + (l.timer.y - l.magi.bottom()) / 2.0;
    let gx = l.magi.right() + (l.telemetry.x - l.magi.right()) / 2.0;
    let routes = [
        // MELCHIOR·1 → telemetry
        (0, vec![(mel.right(), mel.y + mel.h * 0.6), (gx - 6.0, mel.y + mel.h * 0.6), (gx - 6.0, l.telemetry.y + 60.0), (l.telemetry.x, l.telemetry.y + 60.0)]),
        // BALTHASAR·2 → processes
        (1, vec![(bal.right(), bal.y + 40.0), (gx + 8.0, bal.y + 40.0), (gx + 8.0, l.procs.y + 60.0), (l.procs.x, l.procs.y + 60.0)]),
        // CASPER·3 → roster (via the gutter under MAGI)
        (2, vec![(cas.x + 60.0, cas.bottom()), (cas.x + 60.0, gap_y), (l.band.right() + 12.0, gap_y), (l.band.right(), gap_y)]),
    ];
    for (i, pts) in routes {
        let pulse = ctx.anim.trace_pulse(i, ctx.t);
        plate::trace(ctx.canvas, &pts, Palette::ORANGE, pulse);
    }
    for (unit, pts) in extra_routes(l) {
        let pulse = unit.and_then(|i| ctx.anim.trace_pulse(i, ctx.t));
        plate::trace(ctx.canvas, &pts, gfx::alpha(Palette::ORANGE, 0.75), pulse);
    }
}

/// Traces to the panels outside the three judgements (plan REVISION 2: "traces to every panel"),
/// all through the gaps between plates: SECTORS over the plates' top edge and JOBS (CASPER·3's
/// pulse: pilots), TIMER across the gap under MAGI (MELCHIOR·1's), S-DAT off the left-gutter bus.
/// (pulse unit index, polyline).
pub fn extra_routes(l: &Layout) -> Vec<(Option<usize>, Vec<(f32, f32)>)> {
    let under_magi = l.magi.bottom() + (l.timer.y - l.magi.bottom()) / 2.0;
    let over_plates = l.alert.bottom() + (l.magi.y - l.alert.bottom()) / 2.0;
    let gutter = l.band.right() + (l.magi.x - l.band.right()) / 2.0;
    let under_roster = l.units.bottom() + (l.jobs.y - l.units.bottom()) / 2.0;
    let sx = l.magi.x + l.magi.w * 0.78;
    let tx = l.magi.x + l.magi.w * 0.86;
    let tap = l.sdat.y + 70.0;
    let jx = l.jobs.right() - 160.0;
    vec![
        (Some(2), vec![(sx, l.magi.y), (sx, over_plates), (l.sectors.x + 140.0, over_plates), (l.sectors.x + 140.0, l.sectors.y)]),
        (Some(0), vec![(tx, l.magi.bottom()), (tx, under_magi), (tx + 60.0, under_magi), (tx + 60.0, l.timer.y)]),
        (None, vec![(gutter, under_magi), (gutter, tap), (l.sdat.x, tap)]),
        (Some(2), vec![(gutter, tap), (gutter, under_roster), (jx, under_roster), (jx, l.jobs.y)]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_traces_end_on_their_panels_and_stay_in_the_gaps() {
        let l = Layout::new(3840.0, 2160.0);
        let boxes = l.content_boxes();
        let on_edge = |b: Rect, (x, y): (f32, f32)| {
            let within = x >= b.x - 0.5 && x <= b.right() + 0.5 && y >= b.y - 0.5 && y <= b.bottom() + 0.5;
            let edge = (x - b.x).abs() < 0.5 || (x - b.right()).abs() < 0.5 || (y - b.y).abs() < 0.5 || (y - b.bottom()).abs() < 0.5;
            within && edge
        };
        let strictly_inside = |(x, y): (f32, f32)| boxes.iter().any(|b| x > b.x + 1.0 && x < b.right() - 1.0 && y > b.y + 1.0 && y < b.bottom() - 1.0);
        let routes = extra_routes(&l);
        let targets = [l.sectors, l.timer, l.sdat, l.jobs];
        assert_eq!(routes.len(), targets.len());
        for ((_, pts), target) in routes.iter().zip(targets) {
            assert!(on_edge(target, *pts.last().unwrap()), "{pts:?} does not end on {target:?}");
            for w in pts.windows(2) {
                for k in 1..40 {
                    let f = k as f32 / 40.0;
                    let p = (w[0].0 + (w[1].0 - w[0].0) * f, w[0].1 + (w[1].1 - w[0].1) * f);
                    assert!(!strictly_inside(p), "{p:?} of {pts:?} runs through a panel");
                }
            }
        }
    }
}
