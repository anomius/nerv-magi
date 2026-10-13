//! TELEMETRY 監視: per-core bar bank (GHz under each, CAUTION/DANGER tags), CPU 120 s sparkline,
//! memory breakdown, GPU, temperatures, fan, mounts, interfaces and the rx/tx oscilloscope.

use femtovg::{Align, Color, Paint, Path};

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::{fmt_rate, SysState};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

/// Bar color by load: green, amber over 70 %, red over 90 %.
pub fn load_color(v: f32) -> Color {
    if v > 0.9 {
        Palette::RED
    } else if v > 0.7 {
        Palette::AMBER
    } else {
        Palette::GREEN
    }
}

pub fn gb(b: u64) -> String {
    if b >= 1_000_000_000_000 {
        format!("{:.1}T", b as f64 / 1e12)
    } else {
        format!("{:.1}G", b as f64 / 1e9)
    }
}

/// A series needs this many points before it is plotted without the ACQUIRING overlay.
pub const MIN_POINTS: usize = 10;

pub fn acquiring(values: &[Option<f32>]) -> bool {
    values.iter().flatten().count() < MIN_POINTS
}

/// x of the slow scanning sweep inside `r` at time `t` (one pass every 4 s, left → right).
pub fn sweep_x(r: Rect, t: f32) -> f32 {
    r.x + r.w * (t / 4.0).rem_euclid(1.0)
}

/// An ACQUIRING overlay on plot `r`: drawn now in a full pass, recorded by the base pass so every
/// live pass can draw it (the sweep moves each frame), skipped in a live pass (already recorded).
pub fn overlay(ctx: &mut Ctx, r: Rect, color: Color) {
    match ctx.layer {
        crate::tv::layers::Layer::All => acquiring_overlay(ctx.canvas, r, ctx.t, ctx.fonts, color),
        crate::tv::layers::Layer::Base => ctx.anim.overlays.push((r, color)),
        crate::tv::layers::Layer::Live => {}
    }
}

/// Dim `ACQUIRING` label and a scanning sweep line with a short fading trail inside `r`.
pub fn acquiring_overlay(c: &mut gfx::Cv, r: Rect, t: f32, fonts: &crate::tv::fonts::Fonts, color: Color) {
    let x = sweep_x(r, t);
    let trail = 60.0_f32.min(x - r.x);
    if trail > 0.0 {
        let p = Paint::linear_gradient(x - trail, 0.0, x, 0.0, gfx::alpha(color, 0.0), gfx::alpha(color, 0.18));
        let mut band = Path::new();
        band.rect(x - trail, r.y, trail, r.h);
        c.fill_path(&band, &p);
    }
    gfx::line(c, x, r.y, x, r.bottom(), gfx::alpha(color, 0.7), 2.0);
    let lp = gfx::text_paint(&fonts.bold(), 26.0, gfx::alpha(color, 0.45)).with_letter_spacing(6.0);
    gfx::text_center(c, r, "ACQUIRING", &lp);
}

/// Polyline of `values` (0..`top`) inside `r`; `None` gaps break the line.
pub fn sparkline(c: &mut gfx::Cv, r: Rect, values: &[Option<f32>], top: f32, color: Color, fill: bool) {
    let n = values.len().max(2);
    let mut p = Path::new();
    let mut area = Path::new();
    let mut pen = false;
    let mut first_x = None;
    let mut last_x = r.x;
    for (i, v) in values.iter().enumerate() {
        let x = r.x + r.w * i as f32 / (n - 1) as f32;
        match v {
            Some(v) => {
                let y = r.bottom() - r.h * (v / top.max(1e-6)).clamp(0.0, 1.0);
                if pen {
                    p.line_to(x, y);
                    area.line_to(x, y);
                } else {
                    p.move_to(x, y);
                    if first_x.is_none() {
                        first_x = Some(x);
                        area.move_to(x, r.bottom());
                    }
                    area.line_to(x, y);
                }
                pen = true;
                last_x = x;
            }
            None => pen = false,
        }
    }
    if fill && first_x.is_some() {
        area.line_to(last_x, r.bottom());
        area.close();
        c.fill_path(&area, &Paint::color(gfx::alpha(color, 0.15)));
    }
    c.stroke_path(&p, &Paint::color(color).with_line_width(2.5));
}

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    plate::plate(ctx.canvas, r, Cuts::TL_BR, "TELEMETRY", Some("監視"), Palette::ORANGE, ctx.fonts);
    let Some(sys) = ctx.state.sys.clone() else {
        let p = gfx::text_paint(&ctx.fonts.bold(), 40.0, Palette::DIM).with_letter_spacing(4.0);
        gfx::text_center(ctx.canvas, r, "N/A · WAITING FOR SENSORS", &p);
        return;
    };
    let top = r.y + plate::PLATE_TOP + 6.0;
    let h = r.bottom() - 20.0 - top;
    cores(ctx, Rect::new(r.x + 26.0, top, 480.0, h), &sys);
    middle(ctx, Rect::new(r.x + 536.0, top, 360.0, h), &sys);
    right(ctx, Rect::new(r.x + 926.0, top, r.w - 956.0, h), &sys);
}

fn cores(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    let fonts = ctx.fonts;
    let freqs = ctx.state.sysx.as_ref().map(|x| x.cpu_freq_mhz.clone()).unwrap_or_default();
    let n = sys.cpu_cores.len().max(1);
    let hot = sys.cpu_cores.iter().filter(|&&v| v > 0.9).count();
    let warm = sys.cpu_cores.iter().filter(|&&v| v > 0.7 && v <= 0.9).count();
    let c = &mut *ctx.canvas;
    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.5);
    gfx::text(c, r.x, r.y + 20.0, "CORE LOAD · GHz", &lp);
    // CAUTION / DANGER tags
    let mut tx = r.right();
    for (count, word, color) in [(hot, "DANGER", Palette::RED), (warm, "CAUTION", Palette::AMBER)] {
        let tp = gfx::text_paint(&fonts.bold(), 22.0, if count > 0 { Palette::BLACK } else { gfx::alpha(color, 0.5) }).with_letter_spacing(1.5);
        let s = format!("{word} {count}");
        let w = gfx::text_width(c, &s, &tp) + 16.0;
        tx -= w;
        let tag = Rect::new(tx, r.y, w, 28.0);
        if count > 0 {
            gfx::fill_rect(c, tag, color);
        } else {
            gfx::stroke_rect(c, tag, gfx::alpha(color, 0.5), 1.5);
        }
        gfx::text(c, tag.x + 8.0, tag.y + 22.0, &s, &tp);
        tx -= 8.0;
    }

    let bank = Rect::new(r.x, r.y + 40.0, r.w, 220.0);
    let slot = bank.w / n as f32;
    let bw = (slot * 0.62).min(30.0);
    let np = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.6)).with_text_align(Align::Center);
    let fp = gfx::text_paint(&fonts.mono(), 22.0, Palette::CYAN).with_text_align(Align::Center);
    for (i, &v) in sys.cpu_cores.iter().enumerate() {
        let x = bank.x + slot * i as f32 + (slot - bw) / 2.0;
        let bar = Rect::new(x, bank.y, bw, bank.h);
        gfx::seg_bar(c, bar, v, 20, load_color(v), gfx::alpha(Palette::CYAN, 0.12));
        let cx = bank.x + slot * (i as f32 + 0.5);
        gfx::text(c, cx, bank.bottom() + 26.0, &format!("{i:02}"), &np);
        // GHz staggered on two lines so neighbours never collide
        let ghz = freqs.get(i).map_or("--".to_string(), |m| format!("{:.1}", *m as f32 / 1000.0));
        gfx::text(c, cx, bank.bottom() + if i % 2 == 0 { 52.0 } else { 76.0 }, &ghz, &fp);
    }
    // readouts + CPU 120 s sparkline
    let y = bank.bottom() + 114.0;
    let cpu = ctx.anim.roll("cpu_total", sys.cpu_total * 100.0, ctx.t);
    let c = &mut *ctx.canvas;
    let x = gfx::label_value(c, fonts, r.x, y, "CPU", &format!("{cpu:.0}%"), load_color(sys.cpu_total));
    let x = gfx::label_value(c, fonts, x + 24.0, y, "PKG", &sys.temp_cpu_c.map_or("--".into(), |t| format!("{t:.0}°C")), Palette::WHITE);
    gfx::label_value(c, fonts, x + 24.0, y, "FAN", &sys.fan_rpm.map_or("--".into(), |f| format!("{f} RPM")), Palette::WHITE);
    let spark = Rect::new(r.x, y + 16.0, r.w, r.bottom() - y - 40.0);
    gfx::stroke_rect(c, spark, gfx::alpha(Palette::GREEN, 0.35), 1.5);
    let hist = ctx.state.history.cpu.values(ctx.state.now);
    sparkline(c, spark.inset(3.0), &hist, 1.0, Palette::GREEN, true);
    if acquiring(&hist) {
        overlay(ctx, spark.inset(3.0), Palette::GREEN);
    }
    let c = &mut *ctx.canvas;
    let sp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::GREEN, 0.7));
    gfx::text(c, spark.x + 6.0, spark.bottom() + 24.0, "CPU 120 S", &sp);
    gfx::text(c, spark.right() - 54.0, spark.bottom() + 24.0, "NOW", &sp);
}

/// One labelled horizontal bar row: label · bar · value (value column measured, bar takes the rest).
fn bar_row(ctx: &mut Ctx, x: f32, y: f32, w: f32, label: &str, frac: f32, value: &str, color: Color) {
    let c = &mut *ctx.canvas;
    let lp = gfx::text_paint(&ctx.fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.0);
    gfx::text(c, x, y, label, &lp);
    let vp = gfx::text_paint(&ctx.fonts.mono(), 24.0, Palette::WHITE).with_text_align(Align::Right);
    let vw = gfx::text(c, x + w, y, value, &vp);
    let bx = x + 96.0;
    let bw = (w - 96.0 - vw.max(70.0) - 14.0).max(20.0);
    gfx::seg_bar(c, Rect::new(bx, y - 16.0, bw, 16.0), frac, 24, color, gfx::alpha(Palette::DIM, 0.3));
}

fn middle(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    let fonts = ctx.fonts;
    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.5);
    let total = sys.mem_total.max(1) as f32;
    let cached = ctx.state.sysx.as_ref().map_or(0, |x| x.mem_cached);
    let used = sys.mem_used();
    gfx::text(ctx.canvas, r.x, r.y + 20.0, &format!("MEMORY · {} TOTAL", gb(sys.mem_total)), &lp);
    let mut y = r.y + 56.0;
    let rows: [(&str, f32, String, Color); 4] = [
        ("USED", used as f32 / total, gb(used), Palette::GREEN),
        ("CACHED", cached as f32 / total, gb(cached), Palette::CYAN),
        ("AVAIL", sys.mem_available as f32 / total, gb(sys.mem_available), Palette::LCD),
        ("SWAP", sys.swap_used as f32 / sys.swap_total.max(1) as f32, format!("{}/{}", gb(sys.swap_used), gb(sys.swap_total)), Palette::AMBER),
    ];
    for (label, frac, value, color) in rows {
        bar_row(ctx, r.x, y, r.w, label, frac, &value, color);
        y += 32.0;
    }

    // GPU
    y += 14.0;
    let (mhz, max, busy) = sys.gpu.as_ref().map_or((0, 0, 0.0), |g| (g.mhz, g.max_mhz, g.busy));
    let c = &mut *ctx.canvas;
    gfx::text(c, r.x, y, "GPU · INTEL ARC", &lp);
    let vp = gfx::text_paint(&fonts.mono(), 24.0, Palette::WHITE).with_text_align(Align::Right);
    let gpu_txt = if sys.gpu.is_some() { format!("{mhz}/{max} MHz") } else { "N/A".to_string() };
    gfx::text(c, r.right(), y, &gpu_txt, &vp);
    y += 32.0;
    bar_row(ctx, r.x, y, r.w, "BUSY", busy, &format!("{:.0}%", busy * 100.0), load_color(busy));
    y += 12.0;
    let ring: Vec<Option<f32>> = ctx.anim.ring("gpu_busy", busy, ctx.t).iter().map(|&v| Some(v)).collect();
    let spark = Rect::new(r.x, y, r.w, 64.0);
    let c = &mut *ctx.canvas;
    gfx::stroke_rect(c, spark, gfx::alpha(Palette::CYAN, 0.3), 1.0);
    let mut padded = vec![None; crate::tv::anim::RING_CAP.saturating_sub(ring.len())];
    padded.extend(ring);
    sparkline(c, spark.inset(3.0), &padded, 1.0, Palette::CYAN, true);
    if acquiring(&padded) {
        overlay(ctx, spark.inset(3.0), Palette::CYAN);
    }
    let c = &mut *ctx.canvas;

    // temperatures
    y = spark.bottom() + 36.0;
    gfx::text(c, r.x, y, "TEMPERATURE · °C", &lp);
    y += 32.0;
    let temps = [("CPU PKG", sys.temp_cpu_c), ("NVME", sys.temp_nvme_c), ("ACPI", sys.temp_acpi_c)];
    for (label, t) in temps {
        let frac = t.map_or(0.0, |t| (t / 100.0).clamp(0.0, 1.0));
        let color = t.map_or(Palette::DIM, |t| if t >= 90.0 { Palette::RED } else if t >= 75.0 { Palette::AMBER } else { Palette::GREEN });
        bar_row(ctx, r.x, y, r.w, label, frac, &t.map_or("N/A".into(), |t| format!("{t:.0}°C")), color);
        y += 32.0;
    }
}

fn right(ctx: &mut Ctx, r: Rect, sys: &SysState) {
    let fonts = ctx.fonts;
    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.5);
    let sysx = ctx.state.sysx.clone();
    let mut y = r.y + 20.0;
    gfx::text(ctx.canvas, r.x, y, "MOUNTS", &lp);
    y += 32.0;
    let mounts: Vec<(String, u64, u64)> = match &sysx {
        Some(x) if !x.mounts.is_empty() => x.mounts.iter().take(3).map(|m| (m.path.clone(), m.used, m.total)).collect(),
        _ => sys.disk.as_ref().map(|d| vec![("/".to_string(), d.used, d.total)]).unwrap_or_default(),
    };
    for (path, used, total) in mounts {
        let frac = used as f32 / total.max(1) as f32;
        let color = if frac > 0.9 { Palette::RED } else if frac > 0.75 { Palette::AMBER } else { Palette::GREEN };
        let label = gfx::ellipsize(ctx.canvas, &gfx::text_paint(&fonts.label(), 22.0, Palette::WHITE), &path, 90.0);
        bar_row(ctx, r.x, y, r.w, &label, frac, &format!("{:.0}%·{}", frac * 100.0, gb(total)), color);
        y += 32.0;
    }
    if let Some(d) = &sys.disk {
        let vp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.7));
        gfx::text(ctx.canvas, r.x, y, &format!("NVME R {}/s · W {}/s", fmt_rate(d.read_bps), fmt_rate(d.write_bps)), &vp);
        y += 30.0;
    }

    // interfaces table
    y += 10.0;
    let c = &mut *ctx.canvas;
    gfx::text(c, r.x, y, "IFACE", &lp);
    let hp = lp.clone().with_text_align(Align::Right);
    gfx::text(c, r.x + r.w - 110.0, y, "↓ RX", &hp);
    gfx::text(c, r.right(), y, "↑ TX", &hp);
    y += 30.0;
    let ifaces: Vec<(String, f64, f64, bool)> = match &sysx {
        Some(x) if !x.ifaces.is_empty() => x.ifaces.iter().take(3).map(|i| (i.name.clone(), i.rx_bps, i.tx_bps, i.up)).collect(),
        _ => vec![("ALL".into(), sys.net.rx_bps, sys.net.tx_bps, true)],
    };
    let vp = gfx::text_paint(&fonts.mono(), 24.0, Palette::WHITE);
    let rx_p = gfx::text_paint(&fonts.mono(), 24.0, Palette::CYAN).with_text_align(Align::Right);
    let tx_p = gfx::text_paint(&fonts.mono(), 24.0, Palette::MAGENTA).with_text_align(Align::Right);
    for (name, rx, tx, up) in ifaces {
        gfx::fill_rect(c, Rect::new(r.x, y - 14.0, 12.0, 12.0), if up { Palette::GREEN } else { Palette::DIM });
        gfx::text(c, r.x + 20.0, y, &gfx::ellipsize(c, &vp, &name, r.w - 230.0), &vp);
        gfx::text(c, r.x + r.w - 110.0, y, &fmt_rate(rx), &rx_p);
        gfx::text(c, r.right(), y, &fmt_rate(tx), &tx_p);
        y += 30.0;
    }

    // oscilloscope: rx cyan / tx magenta over a grid with crosshairs
    let scope = Rect::new(r.x, y + 4.0, r.w, r.bottom() - y - 34.0);
    gfx::fill_rect(c, scope, Palette::BLACK);
    gfx::stroke_rect(c, scope, gfx::alpha(Palette::CYAN, 0.5), 1.5);
    let mut grid = Path::new();
    for i in 1..4 {
        let gy = scope.y + scope.h * i as f32 / 4.0;
        grid.move_to(scope.x, gy);
        grid.line_to(scope.right(), gy);
    }
    for i in 1..6 {
        let gx = scope.x + scope.w * i as f32 / 6.0;
        grid.move_to(gx, scope.y);
        grid.line_to(gx, scope.bottom());
    }
    c.stroke_path(&grid, &Paint::color(gfx::alpha(Palette::CYAN, 0.12)).with_line_width(1.0));
    for i in 1..6 {
        for j in 1..4 {
            plate::crosshair(c, scope.x + scope.w * i as f32 / 6.0, scope.y + scope.h * j as f32 / 4.0, 6.0, gfx::alpha(Palette::CYAN, 0.45));
        }
    }
    let rx = ctx.state.history.net_rx.values(ctx.state.now);
    let tx = ctx.state.history.net_tx.values(ctx.state.now);
    let top = ctx.state.history.net_rx.max().max(ctx.state.history.net_tx.max()).max(1.0);
    sparkline(c, scope.inset(4.0), &rx, top, Palette::CYAN, false);
    sparkline(c, scope.inset(4.0), &tx, top, Palette::MAGENTA, false);
    if acquiring(&rx) {
        overlay(ctx, scope.inset(4.0), Palette::CYAN);
    }
    let c = &mut *ctx.canvas;
    let sp = gfx::text_paint(&fonts.mono(), 22.0, Palette::CYAN);
    let w = gfx::text(c, scope.x, scope.bottom() + 26.0, &format!("↓{}", fmt_rate(sys.net.rx_bps)), &sp);
    gfx::text(c, scope.x + w + 20.0, scope.bottom() + 26.0, &format!("↑{}", fmt_rate(sys.net.tx_bps)), &sp.with_color(Palette::MAGENTA));
    let mp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.5)).with_text_align(Align::Right);
    gfx::text(c, scope.right(), scope.bottom() + 26.0, &format!("MAX {}", fmt_rate(top as f64)), &mp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquiring_until_ten_points() {
        let mut v = vec![None; 120];
        assert!(acquiring(&v));
        for slot in v.iter_mut().rev().take(9) {
            *slot = Some(1.0);
        }
        assert!(acquiring(&v));
        v[100] = Some(0.5);
        assert!(!acquiring(&v));
    }

    #[test]
    fn sweep_stays_inside_and_moves_right() {
        let r = Rect::new(100.0, 0.0, 400.0, 50.0);
        for t in [0.0, 1.0, 3.99, 4.0, 77.7] {
            let x = sweep_x(r, t);
            assert!(x >= r.x && x <= r.right(), "{t}: {x}");
        }
        assert!(sweep_x(r, 1.0) < sweep_x(r, 2.0));
    }
}
