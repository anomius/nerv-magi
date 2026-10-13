//! LOAD (負荷): the system-load scene (rotation slot 2). Big TOXICITY / L.C.L. PURITY / ELAPSED /
//! TEMP readouts with CAUTION/DANGER tags, a tall per-core bar bank, a converging-lines plot of the
//! last 120 s per core, the memory stack, GPU and temperatures, and the top-25 process table.

use femtovg::{Align, Color, Paint, Path};

use crate::state::SysState;
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Layout, Rect, DESIGN_H};
use crate::tv::panels::telemetry::{acquiring, gb, load_color};
use crate::tv::panels::{alert, header, plate, procs, ticker, Ctx};

use super::CORE_KEYS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Ok,
    Caution,
    Danger,
}

/// Tag for a value against (caution, danger) thresholds; `rising` false when lower is worse.
pub fn tag(v: f32, caution: f32, danger: f32, rising: bool) -> Tag {
    let (c, d) = if rising { (v >= caution, v >= danger) } else { (v <= caution, v <= danger) };
    if d {
        Tag::Danger
    } else if c {
        Tag::Caution
    } else {
        Tag::Ok
    }
}

fn tag_color(t: Tag) -> Color {
    match t {
        Tag::Ok => Palette::GREEN,
        Tag::Caution => Palette::AMBER,
        Tag::Danger => Palette::RED,
    }
}

pub fn draw(ctx: &mut Ctx, layout: &Layout) {
    title_band(ctx, layout.header, "SYSTEM LOAD", "負荷");
    alert::status_strip(ctx, layout.alert);
    if !ctx.layer.base() {
        ticker::draw(ctx, layout.ticker);
        return;
    }
    let Some(sys) = ctx.state.sys.clone() else {
        let p = gfx::text_paint(&ctx.fonts.bold(), 60.0, Palette::DIM).with_letter_spacing(6.0);
        gfx::text_center(ctx.canvas, Rect::new(0.0, 210.0, 3840.0, 1800.0), "N/A · WAITING FOR SENSORS", &p);
        ticker::draw(ctx, layout.ticker);
        return;
    };
    // left edge ruler, right edge CAUTION column
    plate::ruler(ctx.canvas, 30.0, 230.0, 1760.0, true, Palette::ORANGE, ctx.fonts);
    readouts(ctx, Rect::new(120.0, 214.0, 3560.0, 300.0), &sys);
    let bank = Rect::new(120.0, 540.0, 1500.0, 720.0);
    core_bank(ctx, bank, &sys);
    langley(ctx, Rect::new(1660.0, 540.0, 2020.0, 720.0), &sys);
    memory_stack(ctx, Rect::new(120.0, 1290.0, 1500.0, 720.0), &sys);
    let t = Rect::new(1660.0, 1290.0, 2020.0, 720.0);
    plate::plate(ctx.canvas, t, plate::Cuts::BR, "PROCESSES", Some("演算"), Palette::ORANGE, ctx.fonts);
    procs::table(ctx, Rect::new(t.x + 26.0, t.y + plate::PLATE_TOP + 4.0, t.w - 52.0, t.h - plate::PLATE_TOP - 26.0), 25, 3);
    caution_column(ctx, Rect::new(3712.0, 214.0, 104.0, 1796.0), &sys);
    ticker::draw(ctx, layout.ticker);
}

/// Angled title band with kanji, host line and the 7-seg clock (LOAD and SYNC views).
pub fn title_band(ctx: &mut Ctx, r: Rect, title: &str, kanji: &str) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let (_, _, _, _, h, m, s) = header::local_time(ctx.state.now);
    let cp = gfx::text_paint(&fonts.segment(), 72.0, Palette::GREEN).with_text_align(Align::Right);
    if !ctx.layer.base() {
        gfx::glow_text(c, r.right() - 24.0, r.y + 82.0, &format!("{h:02}:{m:02}:{s:02}"), &cp, gfx::alpha(Palette::GREEN, 0.8), 10.0);
        return;
    }
    let mut band = Path::new();
    band.move_to(r.x, r.y);
    band.line_to(r.x + 1380.0, r.y);
    band.line_to(r.x + 1300.0, r.bottom());
    band.line_to(r.x, r.bottom());
    band.close();
    c.fill_path(&band, &Paint::color(Palette::ORANGE));
    let tp = gfx::text_paint(&fonts.bold(), 64.0, Palette::BLACK).with_letter_spacing(8.0);
    let w = gfx::text(c, r.x + 24.0, r.y + 72.0, title, &tp);
    gfx::text(c, r.x + 50.0 + w, r.y + 72.0, kanji, &gfx::text_paint(&fonts.mincho(), 56.0, Palette::BLACK));
    gfx::text(c, r.right() - 24.0, r.y + 82.0, "88:88:88", &cp.clone().with_color(gfx::alpha(Palette::GREEN, 0.12)));
    if ctx.layer.live() {
        gfx::glow_text(c, r.right() - 24.0, r.y + 82.0, &format!("{h:02}:{m:02}:{s:02}"), &cp, gfx::alpha(Palette::GREEN, 0.8), 10.0);
    }
    let host = ctx.state.sysx.as_ref().map_or(String::new(), |x| format!("{} · {} · {}", x.hostname, x.cpu_model, x.kernel));
    let hp = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::ORANGE, 0.85));
    gfx::text(c, r.x + 1440.0, r.y + 62.0, &gfx::ellipsize(c, &hp, &host, 1700.0), &hp);
    gfx::line(c, r.x + 1300.0, r.bottom(), r.right(), r.bottom(), Palette::ORANGE, 3.0);
}

fn readouts(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    let th = &ctx.config.thresholds;
    let cpu = ctx.anim.roll("load_cpu", sys.cpu_total * 100.0, ctx.t);
    let purity = sys.mem_available as f32 / sys.mem_total.max(1) as f32 * 100.0;
    let crit_pct = th.mem_critical_bytes as f32 / sys.mem_total.max(1) as f32 * 100.0;
    let low_pct = th.mem_min_available_bytes as f32 / sys.mem_total.max(1) as f32 * 100.0;
    let temp = sys.temp_cpu_c;
    let items: [(&str, &str, String, &str, Tag); 4] = [
        ("TOXICITY LEVEL", "汚染", format!("{cpu:.1}"), "%", tag(cpu, 70.0, th.cpu_max * 100.0, true)),
        ("L.C.L. PURITY", "純度", format!("{purity:.1}"), "%", tag(purity, low_pct, crit_pct, false)),
        ("ELAPSED TIME", "経過", header::fmt_uptime(sys.uptime_s), "", Tag::Ok),
        ("TEMP", "温度", temp.map_or("--".into(), |t| format!("{t:.0}")), "°C", temp.map_or(Tag::Ok, |t| tag(t, 80.0, th.cpu_temp_max_c, true))),
    ];
    let w = (r.w - 3.0 * 24.0) / 4.0;
    for (i, (label, kanji, value, unit, tg)) in items.iter().enumerate() {
        let b = Rect::new(r.x + i as f32 * (w + 24.0), r.y, w, r.h);
        let cuts = if i % 2 == 0 { plate::Cuts::TL_BR } else { plate::Cuts::TR_BL };
        plate::plate(ctx.canvas, b, cuts, label, None, Palette::ORANGE, ctx.fonts);
        let c = &mut *ctx.canvas;
        let color = tag_color(*tg);
        let kp = gfx::text_paint(&ctx.fonts.mincho(), 34.0, gfx::alpha(Palette::ORANGE, 0.8)).with_text_align(Align::Right);
        gfx::text(c, b.right() - 30.0, b.y + 32.0, kanji, &kp);
        let seg = gfx::text_paint(&ctx.fonts.segment(), 120.0, color);
        let vw = gfx::glow_text(c, b.x + 40.0, b.y + 196.0, value, &seg, gfx::alpha(color, 0.6), 10.0);
        gfx::text(c, b.x + 56.0 + vw, b.y + 196.0, unit, &gfx::text_paint(&ctx.fonts.display(), 48.0, color));
        let (word, tc) = match tg {
            Tag::Ok => ("NOMINAL", gfx::alpha(Palette::GREEN, 0.7)),
            Tag::Caution => ("CAUTION", Palette::AMBER),
            Tag::Danger => ("DANGER", Palette::RED),
        };
        let tag_r = Rect::new(b.x + 40.0, b.y + 226.0, 200.0, 44.0);
        if *tg == Tag::Ok {
            gfx::stroke_rect(c, tag_r, tc, 2.0);
        } else {
            gfx::fill_rect(c, tag_r, tc);
        }
        gfx::text_center(c, tag_r, word, &gfx::text_paint(&ctx.fonts.bold(), 30.0, if *tg == Tag::Ok { tc } else { Palette::BLACK }).with_letter_spacing(3.0));
    }
}

fn core_bank(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    plate::plate(ctx.canvas, r, plate::Cuts::TL_BR, "CORE BANK", Some("演算"), Palette::ORANGE, ctx.fonts);
    let freqs = ctx.state.sysx.as_ref().map(|x| x.cpu_freq_mhz.clone()).unwrap_or_default();
    let c = &mut *ctx.canvas;
    let n = sys.cpu_cores.len().max(1);
    let area = Rect::new(r.x + 40.0, r.y + plate::PLATE_TOP + 20.0, r.w - 80.0, r.h - plate::PLATE_TOP - 140.0);
    let slot = area.w / n as f32;
    let bw = (slot * 0.6).min(64.0);
    let np = gfx::text_paint(&ctx.fonts.mono(), 26.0, gfx::alpha(Palette::WHITE, 0.7)).with_text_align(Align::Center);
    let fp = gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::CYAN).with_text_align(Align::Center);
    let vp = gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::WHITE).with_text_align(Align::Center);
    for (i, &v) in sys.cpu_cores.iter().enumerate() {
        let cx = area.x + slot * (i as f32 + 0.5);
        let bar = Rect::new(cx - bw / 2.0, area.y, bw, area.h);
        // cyan → amber → red by value
        let on = if v > 0.9 { Palette::RED } else if v > 0.7 { Palette::AMBER } else { Palette::CYAN };
        gfx::seg_bar(c, bar, v, 30, on, gfx::alpha(Palette::CYAN, 0.10));
        gfx::text(c, cx, area.bottom() + 32.0, &format!("{:.0}", v * 100.0), &vp);
        gfx::text(c, cx, area.bottom() + 62.0, &format!("C{i:02}"), &np);
        let mhz = freqs.get(i).map_or("--".to_string(), |m| m.to_string());
        gfx::text(c, cx, area.bottom() + if i % 2 == 0 { 92.0 } else { 116.0 }, &mhz, &fp);
    }
}

/// S.A.LANGLEY-style plot: one faint line per core over the last 120 s plus the total, with
/// rulers and a crosshair on the newest total.
fn langley(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    plate::plate(ctx.canvas, r, plate::Cuts::TR_BL, "SYNC PLOT · 120 S", Some("同期"), Palette::ORANGE, ctx.fonts);
    let g = Rect::new(r.x + 90.0, r.y + plate::PLATE_TOP + 20.0, r.w - 130.0, r.h - plate::PLATE_TOP - 70.0);
    let rings: Vec<Vec<f32>> = CORE_KEYS.iter().take(sys.cpu_cores.len()).map(|k| ctx.anim.ring_values(k).to_vec()).collect();
    let c = &mut *ctx.canvas;
    gfx::fill_rect(c, g, Palette::BLACK);
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
    c.stroke_path(&grid, &Paint::color(gfx::alpha(Palette::CYAN, 0.10)).with_line_width(1.0));
    plate::ruler(c, g.x - 70.0, g.y, g.h, true, Palette::CYAN, ctx.fonts);
    let cap = crate::tv::anim::RING_CAP as f32;
    for (i, ring) in rings.iter().enumerate() {
        if ring.len() < 2 {
            continue;
        }
        let mut p = Path::new();
        let start = cap - ring.len() as f32;
        for (k, v) in ring.iter().enumerate() {
            let x = g.x + g.w * (start + k as f32) / (cap - 1.0);
            let y = g.bottom() - g.h * v.clamp(0.0, 1.0);
            if k == 0 { p.move_to(x, y) } else { p.line_to(x, y) }
        }
        let color = load_color(sys.cpu_cores.get(i).copied().unwrap_or(0.0));
        c.stroke_path(&p, &Paint::color(gfx::alpha(color, 0.35)).with_line_width(2.0));
    }
    let total = ctx.state.history.cpu.values(ctx.state.now);
    crate::tv::panels::telemetry::sparkline(c, g, &total, 1.0, Palette::WHITE, false);
    if let Some((i, v)) = total.iter().enumerate().rev().find_map(|(i, v)| v.map(|v| (i, v))) {
        let x = g.x + g.w * i as f32 / (total.len().max(2) - 1) as f32;
        let y = g.bottom() - g.h * v.clamp(0.0, 1.0);
        plate::crosshair(c, x, y, 26.0, Palette::WHITE);
        let lp = gfx::text_paint(&ctx.fonts.mono(), 26.0, Palette::WHITE).with_text_align(Align::Right);
        gfx::text(c, x - 30.0, y - 16.0, &format!("{:.0}%", v * 100.0), &lp);
    }
    if acquiring(&total) {
        crate::tv::panels::telemetry::overlay(ctx, g, Palette::CYAN);
    }
    let c = &mut *ctx.canvas;
    let sp = gfx::text_paint(&ctx.fonts.mono(), 24.0, gfx::alpha(Palette::CYAN, 0.7));
    gfx::text(c, g.x, g.bottom() + 34.0, "-120 S", &sp);
    gfx::text(c, g.right() - 60.0, g.bottom() + 34.0, "NOW", &sp);
    gfx::text(c, g.x + g.w / 2.0 - 160.0, g.bottom() + 34.0, "WHITE = TOTAL · LINES = CORES", &sp);
}

fn memory_stack(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    plate::plate(ctx.canvas, r, plate::Cuts::TR_BL, "MEMORY · GPU · THERMAL", Some("記憶"), Palette::ORANGE, ctx.fonts);
    let cached = ctx.state.sysx.as_ref().map_or(0, |x| x.mem_cached);
    let total = sys.mem_total.max(1) as f32;
    let rows: Vec<(&str, f32, String, Color)> = vec![
        ("USED", sys.mem_used() as f32 / total, format!("{} / {}", gb(sys.mem_used()), gb(sys.mem_total)), Palette::GREEN),
        ("CACHED", cached as f32 / total, gb(cached), Palette::CYAN),
        // MemFree is not in the contract; MemAvailable is the number that matters
        ("AVAIL", sys.mem_available as f32 / total, gb(sys.mem_available), Palette::LCD),
        ("SWAP", sys.swap_used as f32 / sys.swap_total.max(1) as f32, format!("{} / {}", gb(sys.swap_used), gb(sys.swap_total)), Palette::AMBER),
    ];
    let (mhz, max, busy) = sys.gpu.as_ref().map_or((0, 0, 0.0), |g| (g.mhz, g.max_mhz, g.busy));
    let temps = [("CPU PKG", sys.temp_cpu_c), ("NVME", sys.temp_nvme_c), ("ACPI", sys.temp_acpi_c)];
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let x = r.x + 40.0;
    let w = r.w - 80.0;
    let lp = gfx::text_paint(&fonts.label(), 30.0, gfx::alpha(Palette::ORANGE, 0.85)).with_letter_spacing(2.0);
    let vp = gfx::text_paint(&fonts.mono(), 32.0, Palette::WHITE).with_text_align(Align::Right);
    let mut y = r.y + plate::PLATE_TOP + 50.0;
    let row = |c: &mut gfx::Cv, y: f32, label: &str, frac: f32, value: &str, color: Color| {
        gfx::text(c, x, y, label, &lp);
        let vw = gfx::text(c, x + w, y, value, &vp);
        let bw = (w - 170.0 - vw.max(200.0) - 24.0).max(40.0);
        gfx::seg_bar(c, Rect::new(x + 170.0, y - 24.0, bw, 24.0), frac, 40, color, gfx::alpha(Palette::DIM, 0.3));
    };
    for (label, frac, value, color) in &rows {
        row(c, y, label, *frac, value, *color);
        y += 58.0;
    }
    y += 20.0;
    row(c, y, "GPU", busy, &format!("{mhz}/{max} MHz · {:.0}%", busy * 100.0), load_color(busy));
    y += 58.0;
    for (label, t) in temps {
        let frac = t.map_or(0.0, |t| (t / 100.0).clamp(0.0, 1.0));
        let color = t.map_or(Palette::DIM, |t| if t >= 90.0 { Palette::RED } else if t >= 75.0 { Palette::AMBER } else { Palette::GREEN });
        row(c, y, label, frac, &t.map_or("N/A".into(), |t| format!("{t:.0}°C")), color);
        y += 58.0;
    }
    row(c, y, "FAN", sys.fan_rpm.map_or(0.0, |f| (f as f32 / 6000.0).clamp(0.0, 1.0)), &sys.fan_rpm.map_or("N/A".into(), |f| format!("{f} RPM")), Palette::CYAN);
}

/// Lines of a core's tag cell, top to bottom: `C03` · `CAUTION`/`DANGER` (hot cores only) ·
/// `75%` (hot only), dropping lines the cell is too short for (the word goes last).
pub fn tag_lines(core: usize, v: f32, cell_h: f32) -> Vec<String> {
    let id = format!("C{core:02}");
    let word = if v > 0.9 { "DANGER" } else if v > 0.7 { "CAUTION" } else { return vec![id] };
    let pct = format!("{:.0}%", v * 100.0);
    match cell_h {
        h if h >= 100.0 => vec![id, word.into(), pct],
        h if h >= 60.0 => vec![id, word.into()],
        _ => vec![word.into()],
    }
}

/// Right edge: one tag cell per core, like the pin's tag column; hot cores amber (CAUTION) or
/// red (DANGER) with horizontal labels (rotated text was unreadable at 1080p).
fn caution_column(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    let c = &mut *ctx.canvas;
    let n = sys.cpu_cores.len().max(1);
    let h = r.h / n as f32;
    for (i, &v) in sys.cpu_cores.iter().enumerate() {
        let b = Rect::new(r.x, r.y + i as f32 * h + 4.0, r.w, h - 8.0);
        let hot = v > 0.7;
        let fill = if v > 0.9 { Palette::RED } else if hot { Palette::AMBER } else { gfx::alpha(Palette::CYAN, 0.15) };
        gfx::fill_rect(c, b, fill);
        let lines = tag_lines(i, v, b.h);
        let step = 34.0;
        let top = b.y + b.h / 2.0 - step * (lines.len() as f32 - 1.0) / 2.0 + 9.0;
        for (k, line) in lines.iter().enumerate() {
            let p = if !hot {
                gfx::text_paint(&ctx.fonts.mono(), 22.0, gfx::alpha(Palette::CYAN, 0.7))
            } else if line.ends_with('%') || line.starts_with('C') && line.len() == 3 {
                gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::BLACK)
            } else {
                gfx::text_paint(&ctx.fonts.bold(), 24.0, Palette::BLACK).with_letter_spacing(1.0)
            };
            let fit = gfx::ellipsize(c, &p, line, b.w - 8.0);
            gfx::text(c, b.x + b.w / 2.0, top + k as f32 * step, &fit, &p.with_text_align(Align::Center));
        }
    }
    let _ = DESIGN_H;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caution_tags_stack_horizontal_lines_by_cell_height() {
        assert_eq!(tag_lines(3, 0.75, 120.0), ["C03", "CAUTION", "75%"]);
        assert_eq!(tag_lines(3, 0.95, 120.0), ["C03", "DANGER", "95%"]);
        assert_eq!(tag_lines(12, 0.95, 70.0), ["C12", "DANGER"]);
        assert_eq!(tag_lines(12, 0.95, 40.0), ["DANGER"]);
        assert_eq!(tag_lines(0, 0.2, 120.0), ["C00"]);
    }

    #[test]
    fn tags_rise_or_fall() {
        assert_eq!(tag(50.0, 70.0, 85.0, true), Tag::Ok);
        assert_eq!(tag(75.0, 70.0, 85.0, true), Tag::Caution);
        assert_eq!(tag(90.0, 70.0, 85.0, true), Tag::Danger);
        // purity: lower is worse
        assert_eq!(tag(40.0, 10.0, 5.0, false), Tag::Ok);
        assert_eq!(tag(8.0, 10.0, 5.0, false), Tag::Caution);
        assert_eq!(tag(3.0, 10.0, 5.0, false), Tag::Danger);
    }
}
