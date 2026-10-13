//! PROCESSES 演算: btop-style table of the busiest processes.

use femtovg::{Align, Color};

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::fmt_bytes;
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

pub const ROW_H: f32 = 32.0;
pub const HEADER_H: f32 = 36.0;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    plate::plate(ctx.canvas, r, Cuts::BR, "PROCESSES", Some("演算"), Palette::ORANGE, ctx.fonts);
    let t = Rect::new(r.x + 26.0, r.y + plate::PLATE_TOP + 4.0, r.w - 52.0, r.h - plate::PLATE_TOP - 26.0);
    table(ctx, t, 20, 0);
}

/// btop-style table in `t`: up to `max` rows (fewer if they do not fit). The `highlight_mem`
/// biggest RSS users get an amber MEM cell (the LOAD view uses 3).
pub fn table(ctx: &mut Ctx, t: Rect, max: usize, highlight_mem: usize) {
    let fonts = ctx.fonts;
    let st = ctx.state;
    let c = &mut *ctx.canvas;
    let n = fit_rows(t.h, ROW_H, HEADER_H).min(max).min(st.procs.len());
    let rows = &st.procs[..n];
    let total = st.sysx.as_ref().map_or(0, |x| x.procs_total);
    let sum_cpu: f32 = st.procs.iter().map(|p| p.cpu).sum();
    let summary = format!("TOP {n} OF {total} · Σ CPU {sum_cpu:.0}%");
    let sp = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_text_align(Align::Right);
    let sw = gfx::text_width(c, &summary, &sp);
    gfx::fill_rect(c, Rect::new(t.right() - sw - 34.0, t.y - plate::PLATE_TOP - 2.0, sw + 24.0, 30.0), Palette::PANEL);
    gfx::text(c, t.right() - 22.0, t.y - plate::PLATE_TOP + 22.0, &summary, &sp);

    // columns (offsets from t.x): PID │ NAME │ CPU% + bar │ MEM + bar │ THR │ S
    let w = t.w;
    let pid_r = 100.0;
    let name_x = 124.0;
    let cpu_r = name_x + w * 0.26 + 110.0;
    let cpu_bar = (cpu_r + 16.0, w * 0.17);
    let mem_r = cpu_bar.0 + cpu_bar.1 + 120.0;
    let mem_bar = (mem_r + 16.0, w * 0.14);
    let thr_r = w - 56.0;
    let s_x = w - 26.0;
    let hp = gfx::text_paint(&fonts.label(), 22.0, Palette::ORANGE).with_letter_spacing(2.0);
    let hr = hp.clone().with_text_align(Align::Right);
    let hy = t.y + 24.0;
    gfx::text(c, t.x + pid_r, hy, "PID", &hr);
    gfx::text(c, t.x + name_x, hy, "NAME", &hp);
    gfx::text(c, t.x + cpu_r, hy, "CPU%", &hr);
    gfx::text(c, t.x + mem_r, hy, "MEM", &hr);
    gfx::text(c, t.x + thr_r, hy, "THR", &hr);
    gfx::text(c, t.x + s_x, hy, "S", &hp);
    gfx::line(c, t.x, t.y + HEADER_H - 4.0, t.right(), t.y + HEADER_H - 4.0, gfx::alpha(Palette::ORANGE, 0.5), 1.5);
    if rows.is_empty() {
        let p = gfx::text_paint(&fonts.bold(), 36.0, Palette::DIM).with_letter_spacing(4.0);
        gfx::text_center(c, Rect::new(t.x, t.y + HEADER_H, t.w, t.h - HEADER_H), "NO PROCESS DATA", &p);
        return;
    }
    let max_rss = rows.iter().map(|p| p.rss_bytes).max().unwrap_or(1).max(1);
    let mut by_mem: Vec<u64> = rows.iter().map(|p| p.rss_bytes).collect();
    by_mem.sort_unstable_by(|a, b| b.cmp(a));
    let mem_cut = by_mem.get(highlight_mem.saturating_sub(1)).copied().filter(|_| highlight_mem > 0);
    let dp = gfx::text_paint(&fonts.mono(), 24.0, Palette::WHITE);
    let dr = dp.clone().with_text_align(Align::Right);
    let dim = dr.clone().with_color(gfx::alpha(Palette::WHITE, 0.55));
    let mut y = t.y + HEADER_H;
    for (i, p) in rows.iter().enumerate() {
        if i % 2 == 1 {
            gfx::fill_rect(c, Rect::new(t.x - 8.0, y, t.w + 16.0, ROW_H), gfx::alpha(Palette::WHITE, 0.03));
        }
        let base = y + 24.0;
        gfx::text(c, t.x + pid_r, base, &p.pid.to_string(), &dim);
        let name = gfx::ellipsize(c, &dp, &p.name, cpu_r - name_x - 120.0);
        gfx::text(c, t.x + name_x, base, &name, &dp);
        let cc = cpu_color(p.cpu);
        gfx::text(c, t.x + cpu_r, base, &format!("{:.1}", p.cpu), &dr.clone().with_color(cc));
        gfx::seg_bar(c, Rect::new(t.x + cpu_bar.0, y + 9.0, cpu_bar.1, 14.0), (p.cpu / 100.0).clamp(0.0, 1.0), 10, cc, gfx::alpha(Palette::DIM, 0.3));
        let hot_mem = mem_cut.is_some_and(|cut| p.rss_bytes >= cut);
        let mem_color = if hot_mem { Palette::AMBER } else { Palette::WHITE };
        gfx::text(c, t.x + mem_r, base, &fmt_bytes(p.rss_bytes), &dr.clone().with_color(mem_color));
        gfx::seg_bar(c, Rect::new(t.x + mem_bar.0, y + 9.0, mem_bar.1, 14.0), p.rss_bytes as f32 / max_rss as f32, 10, gfx::alpha(mem_color, 0.8), gfx::alpha(Palette::DIM, 0.3));
        gfx::text(c, t.x + thr_r, base, &p.threads.to_string(), &dim);
        let sc = if p.state == 'R' { Palette::CYAN } else { gfx::alpha(Palette::WHITE, 0.45) };
        gfx::text(c, t.x + s_x, base, &p.state.to_string(), &dp.clone().with_color(sc));
        y += ROW_H;
    }
}

/// Table rows that fit under a header in height `h`.
pub fn fit_rows(h: f32, row_h: f32, header_h: f32) -> usize {
    ((h - header_h) / row_h).floor().max(0.0) as usize
}

/// CPU % of one core → bar color: green, amber over 50 %, red over 90 %.
pub fn cpu_color(pct: f32) -> Color {
    if pct > 90.0 {
        Palette::RED
    } else if pct > 50.0 {
        Palette::AMBER
    } else {
        Palette::GREEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rows_floors_and_never_underflows() {
        assert_eq!(fit_rows(560.0, 30.0, 34.0), 17);
        assert_eq!(fit_rows(20.0, 30.0, 34.0), 0);
        assert_eq!(fit_rows(94.0, 30.0, 34.0), 2);
    }

    #[test]
    fn cpu_thresholds() {
        assert_eq!(cpu_color(12.0), Palette::GREEN);
        assert_eq!(cpu_color(51.0), Palette::AMBER);
        assert_eq!(cpu_color(140.0), Palette::RED);
    }
}
