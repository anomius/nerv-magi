//! femtovg drawing helpers in the Eva visual language: thin nested frames with notched labels,
//! hazard stripes, hexagons, glowing text, segmented bars. Everything takes design units; the
//! caller has already applied the layout transform.

use std::sync::atomic::{AtomicU32, Ordering};

use femtovg::renderer::OpenGl;
use femtovg::{Align, Baseline, Canvas, Color, Paint, Path};

use super::fonts::Fonts;
use super::layout::Rect;

pub type Cv = Canvas<OpenGl>;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgbf(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// The global palette (spec §4, plan Global Constraints). Red is for blocked/alerts only.
pub struct Palette;

impl Palette {
    pub const BLACK: Color = rgb(0x00, 0x00, 0x00);
    pub const PANEL: Color = rgb(0x0A, 0x0A, 0x0A);
    pub const ORANGE: Color = rgb(0xFF, 0x99, 0x00);
    pub const AMBER: Color = rgb(0xFF, 0xAA, 0x00);
    pub const GREEN: Color = rgb(0x00, 0xFF, 0x00);
    pub const LCD: Color = rgb(0x39, 0xFF, 0x14);
    pub const CYAN: Color = rgb(0x00, 0xFF, 0xFF);
    pub const RED: Color = rgb(0xFF, 0x00, 0x00);
    pub const MAGENTA: Color = rgb(0xFF, 0x00, 0xFF);
    pub const WHITE: Color = rgb(0xE0, 0xE0, 0xE0);
    pub const TIMER: Color = rgb(0xFF, 0xC4, 0x00);
    pub const DIM: Color = rgb(0x55, 0x55, 0x55);
}

/// `c` with its alpha replaced.
pub fn alpha(c: Color, a: f32) -> Color {
    Color::rgbaf(c.r, c.g, c.b, a.clamp(0.0, 1.0))
}

pub fn fill_rect(canvas: &mut Cv, r: Rect, color: Color) {
    let mut p = Path::new();
    p.rect(r.x, r.y, r.w, r.h);
    canvas.fill_path(&p, &Paint::color(color));
}

pub fn stroke_rect(canvas: &mut Cv, r: Rect, color: Color, width: f32) {
    let mut p = Path::new();
    p.rect(r.x, r.y, r.w, r.h);
    canvas.stroke_path(&p, &Paint::color(color).with_line_width(width));
}

pub fn line(canvas: &mut Cv, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, width: f32) {
    let mut p = Path::new();
    p.move_to(x0, y0);
    p.line_to(x1, y1);
    canvas.stroke_path(&p, &Paint::color(color).with_line_width(width));
}

/// A text paint: font list (first = primary, the rest are glyph fallbacks), size, color.
pub fn text_paint(fonts: &[femtovg::FontId], size: f32, color: Color) -> Paint {
    Paint::color(color).with_font(fonts).with_font_size(size).with_text_baseline(Baseline::Alphabetic)
}

pub fn text(canvas: &mut Cv, x: f32, y: f32, s: &str, paint: &Paint) -> f32 {
    let w = crate::tv::bench::span("text", || canvas.fill_text(x, y, s, paint).map(|m| m.width()).unwrap_or(0.0));
    let due = COLD.with(|c| {
        let mut b = c.borrow_mut();
        let cold = b.cold;
        b.tick(cold)
    });
    if due {
        canvas.flush();
    }
    w
}

/// Text draws per flush while a scene is cold.
pub const COLD_BATCH: usize = 32;

/// The first frames of a scene draw many glyphs femtovg has never seen; it rasterizes each one
/// by tessellating its outline into the frame's vertex buffer. One flush for all of them makes a
/// ~20 MB upload that the GL driver keeps resident for good, so while a scene is cold the text
/// helpers flush every COLD_BATCH draws instead.
#[derive(Debug, Default)]
pub struct ColdBatcher {
    pub cold: bool,
    n: usize,
}

impl ColdBatcher {
    /// Count one text draw; true when a flush is due.
    pub fn tick(&mut self, cold: bool) -> bool {
        if !cold {
            self.n = 0;
            return false;
        }
        self.n += 1;
        self.n % COLD_BATCH == 0
    }
}

thread_local! {
    static COLD: std::cell::RefCell<ColdBatcher> = std::cell::RefCell::new(ColdBatcher::default());
}

/// Mark the pass about to be drawn as cold (a scene's first frames) or warm.
pub fn set_cold(cold: bool) {
    COLD.with(|c| c.borrow_mut().cold = cold);
}

pub fn text_width(canvas: &Cv, s: &str, paint: &Paint) -> f32 {
    canvas.measure_text(0.0, 0.0, s, paint).map(|m| m.width()).unwrap_or(0.0)
}

static BLOOM_BITS: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

/// Multiplier on every glow radius (the CRT look adds +30 % bloom on bright text).
pub fn set_bloom(f: f32) {
    BLOOM_BITS.store(f.to_bits(), Ordering::Relaxed);
}

pub fn bloom() -> f32 {
    f32::from_bits(BLOOM_BITS.load(Ordering::Relaxed))
}

/// Offsets of the halo copies behind glowing text: 8 directions at ~30 % of the blur radius.
pub fn halo_offsets(blur: f32) -> [(f32, f32); 8] {
    let r = blur * 0.3;
    std::array::from_fn(|k| {
        let a = std::f32::consts::FRAC_PI_4 * k as f32;
        (r * a.cos(), r * a.sin())
    })
}

/// Text with a soft glow: a ring of faint copies in the glow colour behind it (no offscreen blur
/// pass: femtovg shadows cost ~6 % of a core each per frame at 4K).
pub fn glow_text(canvas: &mut Cv, x: f32, y: f32, s: &str, paint: &Paint, glow: Color, blur: f32) -> f32 {
    let halo = paint.clone().with_color(alpha(glow, glow.a * 0.22));
    for (dx, dy) in halo_offsets(blur * bloom()) {
        text(canvas, x + dx, y + dy, s, &halo);
    }
    text(canvas, x, y, s, paint)
}

/// Truncate `s` with `…` so it fits `max_w` in `paint`'s font.
pub fn ellipsize(canvas: &Cv, paint: &Paint, s: &str, max_w: f32) -> String {
    crate::tv::bench::span("ellipsize", || ellipsize_(canvas, paint, s, max_w))
}

fn ellipsize_(canvas: &Cv, paint: &Paint, s: &str, max_w: f32) -> String {
    if text_width(canvas, s, paint) <= max_w {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    // largest prefix length n such that prefix + "…" fits
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        if text_width(canvas, &cand, paint) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
}

/// A label/value pair: label (Barlow, dim orange) then value (Fira Code), returns the end x.
pub fn label_value(
    canvas: &mut Cv,
    fonts: &Fonts,
    x: f32,
    base: f32,
    label: &str,
    value: &str,
    value_color: Color,
) -> f32 {
    let lp = text_paint(&fonts.label(), 22.0, alpha(Palette::ORANGE, 0.75)).with_letter_spacing(1.0);
    let vp = text_paint(&fonts.mono(), 26.0, value_color);
    let w = text(canvas, x, base, label, &lp);
    x + w + 10.0 + text(canvas, x + w + 10.0, base, value, &vp)
}

/// Diagonal hazard stripes filling `r` (clipped). `phase` 0..1 scrolls one stripe period.
pub fn hazard(canvas: &mut Cv, r: Rect, phase: f32, stripe: Color, ground: Color) {
    canvas.save();
    canvas.intersect_scissor(r.x, r.y, r.w, r.h);
    fill_rect(canvas, r, ground);
    let w = r.h.max(1.0); // stripe width ~ strip height
    let period = 2.0 * w;
    let shift = phase.rem_euclid(1.0) * period;
    let mut p = Path::new();
    let mut x = r.x - r.h - period + shift;
    while x < r.right() + period {
        p.move_to(x, r.bottom());
        p.line_to(x + w, r.bottom());
        p.line_to(x + w + r.h, r.y);
        p.line_to(x + r.h, r.y);
        p.close();
        x += period;
    }
    canvas.fill_path(&p, &Paint::color(stripe));
    canvas.restore();
}

/// Flat-top hexagon.
pub fn hex_path(cx: f32, cy: f32, radius: f32) -> Path {
    let mut p = Path::new();
    for i in 0..6 {
        let a = std::f32::consts::FRAC_PI_3 * i as f32;
        let (x, y) = (cx + radius * a.cos(), cy + radius * a.sin());
        if i == 0 {
            p.move_to(x, y);
        } else {
            p.line_to(x, y);
        }
    }
    p.close();
    p
}

pub fn hex(canvas: &mut Cv, cx: f32, cy: f32, radius: f32, fill: Option<Color>, stroke: Color) {
    let p = hex_path(cx, cy, radius);
    if let Some(f) = fill {
        canvas.fill_path(&p, &Paint::color(f));
    }
    canvas.stroke_path(&p, &Paint::color(stroke).with_line_width(3.0));
}

/// Segmented bar: vertical when taller than wide (fills bottom-up), else horizontal (left→right).
pub fn seg_bar(canvas: &mut Cv, r: Rect, value: f32, segments: usize, on: Color, off: Color) {
    let n = segments.max(1);
    let lit = (value.clamp(0.0, 1.0) * n as f32).round() as usize;
    let vertical = r.h > r.w;
    let len = if vertical { r.h } else { r.w };
    let gap = (len / n as f32 * 0.22).max(1.0);
    let seg = (len - gap * (n as f32 - 1.0)) / n as f32;
    let mut on_path = Path::new();
    let mut off_path = Path::new();
    for i in 0..n {
        let o = i as f32 * (seg + gap);
        let cell =
            if vertical { Rect::new(r.x, r.bottom() - o - seg, r.w, seg) } else { Rect::new(r.x + o, r.y, seg, r.h) };
        let p = if i < lit { &mut on_path } else { &mut off_path };
        p.rect(cell.x, cell.y, cell.w, cell.h);
    }
    canvas.fill_path(&off_path, &Paint::color(off));
    canvas.fill_path(&on_path, &Paint::color(on));
}

/// Centered single-line text inside `r`.
pub fn text_center(canvas: &mut Cv, r: Rect, s: &str, paint: &Paint) {
    let p = paint.clone().with_text_align(Align::Center).with_text_baseline(Baseline::Middle);
    text(canvas, r.x + r.w / 2.0, r.y + r.h / 2.0, s, &p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_batcher_flushes_every_batch_only_while_cold() {
        let mut b = ColdBatcher::default();
        let due: Vec<bool> = (0..COLD_BATCH * 2).map(|_| b.tick(true)).collect();
        assert_eq!(due.iter().filter(|d| **d).count(), 2);
        assert!(due[COLD_BATCH - 1] && due[2 * COLD_BATCH - 1]);
        assert!((0..COLD_BATCH * 3).all(|_| !b.tick(false)));
    }

    #[test]
    fn halo_is_a_ring_scaled_by_blur() {
        let h = halo_offsets(10.0);
        for (x, y) in h {
            let r = (x * x + y * y).sqrt();
            assert!((r - 3.0).abs() < 0.01, "{x} {y}");
        }
        let dirs: std::collections::BTreeSet<(i32, i32)> = h
            .iter()
            .map(|(x, y)| (x.signum() as i32 * (x.abs() > 0.01) as i32, y.signum() as i32 * (y.abs() > 0.01) as i32))
            .collect();
        assert_eq!(dirs.len(), 8);
        assert!(halo_offsets(0.0).iter().all(|&(x, y)| x == 0.0 && y == 0.0));
    }
}
