//! SECTORS: workspaces as an irregular honeycomb cluster (plan REVISION 2).

use femtovg::{Align, Paint};

use super::plate::{self, Cuts};
use super::{status_color, Ctx};
use crate::state::AgentStatus;
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    plate::plate(ctx.canvas, r, Cuts::TR_BL, "SECTORS", Some("区画"), Palette::ORANGE, ctx.fonts);
    let roll = ctx.state.workspace_rollup();
    let fonts = ctx.fonts;
    // legend on the header rule
    let alert = roll.iter().filter(|w| w.agents.blocked > 0).count();
    let busy = roll.iter().filter(|w| w.agents.working > 0).count();
    let lp = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_text_align(Align::Right);
    let legend = format!("{} SECTORS · {busy} ACTIVE · {alert} ALERT", roll.len());
    let lw = gfx::text_width(ctx.canvas, &legend, &lp);
    gfx::fill_rect(ctx.canvas, Rect::new(r.right() - 64.0 - lw, r.y + 2.0, lw + 24.0, 30.0), Palette::PANEL);
    gfx::text(ctx.canvas, r.right() - 52.0, r.y + 26.0, &legend, &lp);

    let area = Rect::new(r.x + 24.0, r.y + plate::PLATE_TOP + 4.0, r.w - 48.0, r.h - plate::PLATE_TOP - 24.0);
    if roll.is_empty() {
        let p = gfx::text_paint(&fonts.bold(), 40.0, Palette::DIM).with_letter_spacing(4.0);
        gfx::text_center(ctx.canvas, area, "NO SECTORS", &p);
        return;
    }
    let (rad, pts) = hex_layout(area, roll.len(), 112.0);
    big_count(ctx.canvas, fonts, r, roll.len(), &pts, rad);
    let pulse = 0.5 + 0.5 * (ctx.t * std::f32::consts::TAU).cos();
    let c = &mut *ctx.canvas;
    for (w, (x, y)) in roll.iter().zip(pts) {
        let n = &w.agents;
        let total = n.blocked + n.working + n.done + n.idle + n.unknown;
        let (fill, stroke) = if n.blocked > 0 {
            (Some(gfx::alpha(Palette::RED, 0.45 + 0.4 * pulse)), Palette::RED)
        } else if n.working > 0 {
            (Some(gfx::alpha(Palette::ORANGE, 0.35)), Palette::ORANGE)
        } else if total > 0 {
            (None, Palette::ORANGE)
        } else {
            (None, gfx::alpha(Palette::ORANGE, 0.4))
        };
        gfx::hex(c, x, y, rad * 0.94, fill, stroke);
        let inner_w = rad * 1.45;
        let fg = if n.blocked > 0 || n.working > 0 { Palette::WHITE } else { gfx::alpha(Palette::WHITE, 0.75) };
        if n.blocked > 0 {
            let ep = gfx::text_paint(&fonts.bold(), (rad * 0.24).clamp(22.0, 28.0), Palette::WHITE)
                .with_text_align(Align::Center)
                .with_letter_spacing(2.0);
            gfx::text(c, x, y - rad * 0.42, "EMERGENCY", &ep);
        } else {
            let ip =
                gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_text_align(Align::Center);
            gfx::text(c, x, y - rad * 0.42, &w.id, &ip);
        }
        let np = gfx::text_paint(&fonts.bold(), (rad * 0.42).clamp(26.0, 46.0), fg).with_text_align(Align::Center);
        gfx::text(c, x, y + rad * 0.12, &total.to_string(), &np);
        let lp = gfx::text_paint(&fonts.label(), 24.0, fg).with_text_align(Align::Center);
        let label = gfx::ellipsize(c, &lp, &w.label, inner_w);
        gfx::text(c, x, y + rad * 0.42, &label, &lp);
        // one pip per agent, colored by status
        let pips: Vec<AgentStatus> = [
            (n.blocked, AgentStatus::Blocked),
            (n.working, AgentStatus::Working),
            (n.done, AgentStatus::Done),
            (n.idle, AgentStatus::Idle),
        ]
        .iter()
        .flat_map(|&(k, s)| std::iter::repeat(s).take(k))
        .take(8)
        .collect();
        let pw = 12.0;
        let mut px = x - (pips.len() as f32 * (pw + 4.0) - 4.0) / 2.0;
        for s in pips {
            let color = status_color(s);
            let sq = Rect::new(px, y + rad * 0.56, pw, pw);
            if s == AgentStatus::Idle {
                gfx::stroke_rect(c, sq, color, 2.0);
            } else {
                gfx::fill_rect(c, sq, color);
            }
            px += pw + 4.0;
        }
        if n.done > 0 && n.blocked == 0 {
            let mut d = femtovg::Path::new();
            d.circle(x + rad * 0.62, y - rad * 0.5, 8.0);
            c.fill_path(&d, &Paint::color(Palette::CYAN));
        }
    }
}

/// Centers for `n` flat-top hexes of circumradius ≤ `max_r`, packed as an irregular cluster
/// centered in `r`; the radius shrinks until every hex fits. Returns (radius, centers).
pub fn hex_layout(r: Rect, n: usize, max_r: f32) -> (f32, Vec<(f32, f32)>) {
    if n == 0 {
        return (max_r, Vec::new());
    }
    // flat-top axial grid at unit radius; pick the n cells nearest the center under an
    // anisotropic metric (wide boxes → wide clusters) plus a fixed per-cell jitter, so the
    // cluster edge is ragged rather than a strip or a perfect ring
    let aspect = (r.w / r.h).clamp(1.0, 4.0);
    let s3 = 3f32.sqrt();
    let reach = (n as f32).sqrt().ceil() as i32 + 4;
    let mut cells: Vec<(f32, f32, f32)> = Vec::new();
    for q in -reach * 2..=reach * 2 {
        for rr in -reach..=reach {
            let x = 1.5 * q as f32;
            let y = s3 * (rr as f32 + q as f32 / 2.0);
            let jitter = (((q * 73_856_093) ^ (rr * 19_349_663)) as u32 % 1000) as f32 / 1000.0;
            let d = (x / aspect).powi(2) + y.powi(2) + 0.9 * jitter;
            cells.push((d, x, y));
        }
    }
    cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    cells.truncate(n);
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(_, x, y) in &cells {
        x0 = x0.min(x - 1.0);
        x1 = x1.max(x + 1.0);
        y0 = y0.min(y - 1.0);
        y1 = y1.max(y + 1.0);
    }
    let radius = max_r.min(r.w / (x1 - x0)).min(r.h / (y1 - y0));
    let cx = r.x + r.w / 2.0 - (x0 + x1) / 2.0 * radius;
    let cy = r.y + r.h / 2.0 - (y0 + y1) / 2.0 * radius;
    (radius, cells.iter().map(|&(_, x, y)| (cx + x * radius, cy + y * radius)).collect())
}

#[allow(dead_code)] // the plan's honeycomb test API; drawing uses hex_layout
pub fn hex_positions(r: Rect, n: usize, radius: f32) -> Vec<(f32, f32)> {
    hex_layout(r, n, radius).1
}

/// Size of the sector count numeral.
const COUNT_SIZE: f32 = 150.0;

/// Box of the big count numeral: right-aligned in the plate, its lower part hanging over the
/// plate's bottom edge into the gap (the pins' numerals overlap plate edges).
pub fn count_box(r: Rect, text_w: f32, size: f32) -> Rect {
    let h = size * 0.8;
    Rect::new(r.right() - 40.0 - text_w, r.bottom() + 24.0 - h, text_w, h)
}

/// No hex (centre, radius) touches `b`.
pub fn clear_of_hexes(b: Rect, pts: &[(f32, f32)], rad: f32) -> bool {
    pts.iter().all(|&(x, y)| x + rad < b.x || x - rad > b.right() || y + rad < b.y || y - rad > b.bottom())
}

/// The sector count as a big 7-segment numeral over the plate's bottom-right edge, with its
/// ghost; skipped when the honeycomb reaches that corner.
fn big_count(c: &mut gfx::Cv, fonts: &crate::tv::fonts::Fonts, r: Rect, n: usize, pts: &[(f32, f32)], rad: f32) {
    let seg =
        gfx::text_paint(&fonts.segment(), COUNT_SIZE, gfx::alpha(Palette::ORANGE, 0.85)).with_text_align(Align::Right);
    let ghost = "88";
    let b = count_box(r, gfx::text_width(c, ghost, &seg), COUNT_SIZE);
    if !clear_of_hexes(b, pts, rad) {
        return;
    }
    let base = b.bottom();
    gfx::text(c, b.right(), base, ghost, &seg.clone().with_color(gfx::alpha(Palette::ORANGE, 0.08)));
    gfx::text(c, b.right(), base, &format!("{:02}", n.min(99)), &seg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_count_straddles_the_bottom_edge_and_avoids_hexes() {
        let r = Rect::new(2460.0, 210.0, 1356.0, 490.0);
        let b = count_box(r, 300.0, 150.0);
        assert!(b.y < r.bottom() && b.bottom() > r.bottom(), "straddles the plate's bottom edge: {b:?}");
        assert!(b.right() <= r.right() && b.x >= r.x);
        assert!(clear_of_hexes(b, &[(2700.0, 400.0)], 100.0));
        assert!(!clear_of_hexes(b, &[(b.x + 10.0, b.y + 10.0)], 50.0));
    }

    #[test]
    fn honeycomb_positions_stay_inside_panel() {
        let r = crate::tv::layout::Rect { x: 24.0, y: 1572.0, w: 1700.0, h: 430.0 };
        let pts = hex_positions(r, 14, 70.0);
        assert_eq!(pts.len(), 14);
        for (x, y) in pts {
            assert!(x - 70.0 >= r.x && x + 70.0 <= r.x + r.w && y - 70.0 >= r.y && y + 70.0 <= r.y + r.h);
        }
    }

    #[test]
    fn cluster_fits_shrinks_and_never_overlaps() {
        let r = Rect::new(2460.0, 260.0, 1356.0, 430.0);
        for n in [1, 3, 8, 14, 30] {
            let (rad, pts) = hex_layout(r, n, 110.0);
            assert_eq!(pts.len(), n);
            assert!(rad <= 110.0 && rad > 10.0, "{n}: {rad}");
            for &(x, y) in &pts {
                assert!(
                    x - rad >= r.x - 0.01
                        && x + rad <= r.right() + 0.01
                        && y - rad >= r.y - 0.01
                        && y + rad <= r.bottom() + 0.01,
                    "{n}"
                );
            }
            for (i, a) in pts.iter().enumerate() {
                for b in &pts[i + 1..] {
                    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                    assert!(d >= 3f32.sqrt() * rad - 0.01, "{n}: {a:?} {b:?}");
                }
            }
        }
        // 8 hexes do not fill a rectangle: the cluster is not a plain row
        let (_, pts) = hex_layout(r, 8, 110.0);
        let rows: std::collections::BTreeSet<i32> = pts.iter().map(|p| p.1.round() as i32).collect();
        assert!(rows.len() >= 2);
    }
}
