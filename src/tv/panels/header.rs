//! Header: title, institutional kanji, 7-segment clock and date.

use std::time::{SystemTime, UNIX_EPOCH};

use femtovg::{Align, Paint, Path};

use super::Ctx;
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Local wall-clock fields: (year, month, day, weekday 0=Sun, hour, minute, second).
pub fn local_time(now: SystemTime) -> (i32, u32, u32, usize, u32, u32, u32) {
    let secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    (
        tm.tm_year + 1900,
        (tm.tm_mon + 1) as u32,
        tm.tm_mday as u32,
        tm.tm_wday.clamp(0, 6) as usize,
        tm.tm_hour as u32,
        tm.tm_min as u32,
        tm.tm_sec as u32,
    )
}

/// "1d 04:12" / "04:12:09"
pub fn fmt_uptime(s: u64) -> String {
    let (d, h, m) = (s / 86_400, s / 3600 % 24, s / 60 % 60);
    if d > 0 {
        format!("{d}d {h:02}:{m:02}")
    } else {
        format!("{h:02}:{m:02}:{:02}", s % 60)
    }
}

/// Width of the slanted clock plate at the header's right end.
const CLOCK_PLATE_W: f32 = 720.0;
const SLANT: f32 = 40.0;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if ctx.layer.base() {
        draw_base(ctx, r);
    }
    if ctx.layer.live() {
        let (.., h, mi, s) = local_time(ctx.state.now);
        let clock_paint = gfx::text_paint(&ctx.fonts.segment(), 72.0, Palette::GREEN).with_text_align(Align::Right);
        let right = r.right() - 24.0 - SLANT - 20.0;
        crate::tv::sprites::glow_text(
            ctx.canvas,
            ctx.sprites,
            "clock",
            right,
            r.y + 82.0,
            &format!("{h:02}:{mi:02}:{s:02}"),
            &clock_paint,
            Palette::GREEN,
            gfx::alpha(Palette::GREEN, 0.8),
            10.0,
        );
    }
}

fn draw_base(ctx: &mut Ctx, r: Rect) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let (y, mo, d, wd, ..) = local_time(ctx.state.now);
    let clock_paint = gfx::text_paint(&fonts.segment(), 72.0, Palette::GREEN).with_text_align(Align::Right);
    let right = r.right() - 24.0 - SLANT - 20.0;
    let base = r.y + 82.0;
    // slanted clock plate (parallelogram) at the right end; the header rule angles up into it
    let px = r.right() - 24.0 - CLOCK_PLATE_W;
    let mut plate = Path::new();
    plate.move_to(px + SLANT, r.y + 2.0);
    plate.line_to(r.right() - 24.0, r.y + 2.0);
    plate.line_to(r.right() - 24.0 - SLANT, r.bottom() - 2.0);
    plate.line_to(px, r.bottom() - 2.0);
    plate.close();
    c.fill_path(&plate, &Paint::color(Palette::PANEL));
    c.stroke_path(&plate, &Paint::color(Palette::ORANGE).with_line_width(3.0));
    let mut rule = Path::new();
    rule.move_to(r.x, r.bottom() + 6.0);
    rule.line_to(px - 60.0, r.bottom() + 6.0);
    rule.line_to(px - 20.0, r.bottom() - 34.0);
    rule.line_to(px - 4.0, r.bottom() - 34.0);
    c.stroke_path(&rule, &Paint::color(Palette::ORANGE).with_line_width(3.0));
    gfx::line(c, r.x, r.bottom() + 12.0, 640.0, r.bottom() + 12.0, gfx::alpha(Palette::ORANGE, 0.45), 1.5);

    let title = gfx::text_paint(&fonts.bold(), 52.0, Palette::ORANGE).with_letter_spacing(5.0);
    let w = gfx::text(c, r.x + 24.0, r.y + 54.0, "MAGI AGENT CONTROL", &title);
    let sub = gfx::text_paint(&fonts.mincho(), 30.0, gfx::alpha(Palette::WHITE, 0.75));
    gfx::text(c, r.x + 24.0, r.y + 88.0, "作戦行動 / OPERATIONS", &sub);
    let strip_x = r.x + 24.0 + w.max(560.0) + 40.0;
    let strip_end = px - 80.0;

    // clock ghost (the digits are live) and date to its left, inside the slanted plate
    let ghost = clock_paint.clone().with_color(gfx::alpha(Palette::GREEN, 0.12));
    gfx::text(c, right, base, "88:88:88", &ghost);
    let date = gfx::text_paint(&fonts.label(), 30.0, Palette::ORANGE).with_letter_spacing(1.5);
    gfx::text(c, px + SLANT + 20.0, r.y + 44.0, &format!("{y}.{mo:02}.{d:02} {}", WEEKDAYS[wd]), &date);
    let tz = gfx::text_paint(&fonts.mincho(), 24.0, gfx::alpha(Palette::ORANGE, 0.7));
    gfx::text(c, px + SLANT + 6.0, r.y + 82.0, "時刻 LOCAL", &tz);

    // system strip: label over value cells between the title and the clock plate
    let st = ctx.state;
    let x = st.sysx.as_ref();
    let sys = st.sys.as_ref();
    let dash = || "--".to_string();
    let cores = sys.map_or(0, |s| s.cpu_cores.len());
    let cells: Vec<(&str, String)> = vec![
        ("HOST", x.map_or_else(dash, |x| x.hostname.clone())),
        ("KERNEL", x.map_or_else(dash, |x| x.kernel.clone())),
        (
            "CPU",
            x.map_or_else(
                dash,
                |x| if cores > 0 { format!("{} · {cores}T", x.cpu_model) } else { x.cpu_model.clone() },
            ),
        ),
        ("UPTIME", sys.map_or_else(dash, |s| fmt_uptime(s.uptime_s))),
        (
            "LOAD 1/5/15",
            sys.map_or_else(dash, |s| format!("{:.2} {:.2} {:.2}", s.load_avg[0], s.load_avg[1], s.load_avg[2])),
        ),
        ("PROCS RUN/ALL", x.map_or_else(dash, |x| format!("{}/{}", x.procs_running, x.procs_total))),
        ("THREADS", x.map_or_else(dash, |x| x.threads_total.to_string())),
    ];
    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.75)).with_letter_spacing(1.5);
    let vp = gfx::text_paint(&fonts.mono(), 26.0, Palette::WHITE);
    let widths: Vec<f32> =
        cells.iter().map(|(l, v)| gfx::text_width(c, l, &lp).max(gfx::text_width(c, v, &vp))).collect();
    let avail = strip_end - strip_x;
    let gap = ((avail - widths.iter().sum::<f32>() - 14.0 * cells.len() as f32) / cells.len() as f32).clamp(20.0, 90.0);
    let mut cx = strip_x;
    for ((label, value), w) in cells.iter().zip(widths) {
        let room = (strip_end - cx - 8.0).max(0.0);
        if room < 60.0 {
            break;
        }
        // slanted separators echo the clock plate
        gfx::line(c, cx + 8.0, r.y + 14.0, cx, r.y + 84.0, gfx::alpha(Palette::ORANGE, 0.45), 1.5);
        gfx::text(c, cx + 16.0, r.y + 40.0, label, &lp);
        let v = gfx::ellipsize(c, &vp, value, room - 16.0);
        gfx::text(c, cx + 14.0, r.y + 78.0, &v, &vp);
        cx += w + gap + 16.0;
    }
}
