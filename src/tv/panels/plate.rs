//! Non-grid Eva composition (plan REVISION 2): chamfered plates with angled header bands, circuit
//! traces with dot terminals, the vertical chevron band, tick rulers and crosshairs, skew.
//! Decoration may cross the gaps between content boxes; text stays inside its box.

use femtovg::{Color, Paint, Path};

use crate::state::AgentStatus;
use crate::tv::fonts::Fonts;
use crate::tv::gfx::{self, Cv, Palette};
use crate::tv::layout::Rect;

/// Chamfer size of a plate corner.
pub const CUT: f32 = 30.0;
/// Content starts this far below a plate's top edge (under the header band).
pub const PLATE_TOP: f32 = 50.0;

/// Which corners are cut at 45°: top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, Copy)]
pub struct Cuts(pub [bool; 4]);

impl Cuts {
    pub const TL_BR: Cuts = Cuts([true, false, true, false]);
    pub const TR_BL: Cuts = Cuts([false, true, false, true]);
    pub const BR: Cuts = Cuts([false, false, true, false]);
}

pub fn chamfer_path(r: Rect, cuts: Cuts, c: f32) -> Path {
    let [tl, tr, br, bl] = cuts.0;
    let mut p = Path::new();
    let cut = |b: bool| if b { c } else { 0.0 };
    p.move_to(r.x + cut(tl), r.y);
    p.line_to(r.right() - cut(tr), r.y);
    p.line_to(r.right(), r.y + cut(tr));
    p.line_to(r.right(), r.bottom() - cut(br));
    p.line_to(r.right() - cut(br), r.bottom());
    p.line_to(r.x + cut(bl), r.bottom());
    p.line_to(r.x, r.bottom() - cut(bl));
    p.line_to(r.x, r.y + cut(tl));
    p.close();
    p
}

/// A chamfered plate: panel fill, 3 px outline, inner hairline, and an angled header band ending
/// in a ◢ slope with the title (black on `color`) and kanji after it.
pub fn plate(canvas: &mut Cv, r: Rect, cuts: Cuts, title: &str, kanji: Option<&str>, color: Color, fonts: &Fonts) {
    let outline = chamfer_path(r, cuts, CUT);
    canvas.fill_path(&outline, &Paint::color(Palette::PANEL));
    canvas.stroke_path(&outline, &Paint::color(color).with_line_width(3.0));
    let inner = chamfer_path(r.inset(7.0), cuts, CUT - 3.0);
    canvas.stroke_path(&inner, &Paint::color(gfx::alpha(color, 0.25)).with_line_width(1.5));

    // header band: starts after the top-left cut, slopes down-right at its end
    let band_h = 36.0;
    let tp = gfx::text_paint(&fonts.bold(), 30.0, Palette::BLACK).with_letter_spacing(2.5);
    let tw = gfx::text_width(canvas, title, &tp);
    let x0 = r.x + if cuts.0[0] { CUT } else { 0.0 };
    let x1 = x0 + tw + 40.0;
    let mut band = Path::new();
    band.move_to(x0, r.y);
    band.line_to(x1 + band_h, r.y);
    band.line_to(x1, r.y + band_h);
    band.line_to(r.x, r.y + band_h);
    if cuts.0[0] {
        band.line_to(r.x, r.y + CUT);
    }
    band.close();
    canvas.fill_path(&band, &Paint::color(color));
    gfx::text(canvas, x0 + 14.0, r.y + 29.0, title, &tp);
    let mut x = x1 + band_h + 12.0;
    if let Some(k) = kanji {
        let kp = gfx::text_paint(&fonts.mincho(), 28.0, color);
        x += gfx::text(canvas, x, r.y + 29.0, k, &kp) + 14.0;
    }
    // double rule from the band to the right edge
    let end = r.right() - if cuts.0[1] { CUT } else { 0.0 } - 12.0;
    if end > x + 20.0 {
        gfx::line(canvas, x, r.y + 12.0, end, r.y + 12.0, gfx::alpha(color, 0.6), 1.5);
        gfx::line(canvas, x, r.y + 20.0, end, r.y + 20.0, gfx::alpha(color, 0.3), 1.0);
    }
}

/// A right-angle circuit trace with dot terminals. `pulse` (0..1) puts a bright dot travelling
/// along it (used when the vote at its origin changes).
pub fn trace(canvas: &mut Cv, pts: &[(f32, f32)], color: Color, pulse: Option<f32>) {
    if pts.len() < 2 {
        return;
    }
    let mut p = Path::new();
    p.move_to(pts[0].0, pts[0].1);
    for &(x, y) in &pts[1..] {
        p.line_to(x, y);
    }
    canvas.stroke_path(&p, &Paint::color(gfx::alpha(color, 0.7)).with_line_width(2.5));
    for &(x, y) in [pts[0], pts[pts.len() - 1]].iter() {
        let mut d = Path::new();
        d.circle(x, y, 7.0);
        canvas.fill_path(&d, &Paint::color(color));
    }
    if let Some(f) = pulse {
        let (x, y) = point_along(pts, f.clamp(0.0, 1.0));
        let mut d = Path::new();
        d.circle(x, y, 9.0);
        canvas.save();
        canvas.set_shadow_color(color);
        canvas.set_shadow_blur(10.0);
        canvas.fill_path(&d, &Paint::color(Palette::WHITE));
        canvas.restore();
    }
}

/// The point at fraction `f` of a polyline's length.
pub fn point_along(pts: &[(f32, f32)], f: f32) -> (f32, f32) {
    let segs: Vec<f32> = pts.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).collect();
    let total: f32 = segs.iter().sum();
    let mut want = total * f;
    for (w, len) in pts.windows(2).zip(&segs) {
        if want <= *len && *len > 0.0 {
            let k = want / len;
            return (w[0].0 + (w[1].0 - w[0].0) * k, w[0].1 + (w[1].1 - w[0].1) * k);
        }
        want -= len;
    }
    pts[pts.len() - 1]
}

/// Label box of row `(y0, h)` in the chevron band.
fn band_box(r: Rect, y0: f32, h: f32) -> Rect {
    Rect::new(r.x + 3.0, y0 + h / 2.0 - 13.0, r.w - 6.0, 26.0)
}

/// Vertical chevron band beside the roster, static part: background, a red segment beside each
/// blocked row, an `MT-nn` label per row and the SYNC caption.
pub fn chevron_band_base(canvas: &mut Cv, r: Rect, rows: &[(f32, f32, AgentStatus)], fonts: &Fonts) {
    gfx::fill_rect(canvas, r, Palette::BLACK);
    gfx::line(canvas, r.x, r.y, r.x, r.bottom(), gfx::alpha(Palette::CYAN, 0.5), 1.5);
    gfx::line(canvas, r.right(), r.y, r.right(), r.bottom(), gfx::alpha(Palette::CYAN, 0.5), 1.5);
    let (mut red, mut teal, mut boxes, mut red_boxes) = (Path::new(), Path::new(), Path::new(), Path::new());
    for &(y0, h, s) in rows {
        match s {
            AgentStatus::Blocked => red.rect(r.x + 2.0, y0, r.w - 4.0, h),
            AgentStatus::Working => teal.rect(r.x + 2.0, y0, r.w - 4.0, h),
            _ => {}
        }
        let b = band_box(r, y0, h);
        if s == AgentStatus::Blocked { red_boxes.rect(b.x, b.y, b.w, b.h) } else { boxes.rect(b.x, b.y, b.w, b.h) }
    }
    canvas.fill_path(&red, &Paint::color(gfx::alpha(Palette::RED, 0.85)));
    canvas.fill_path(&teal, &Paint::color(gfx::alpha(Palette::CYAN, 0.10)));
    canvas.fill_path(&boxes, &Paint::color(Palette::BLACK));
    canvas.fill_path(&red_boxes, &Paint::color(Palette::RED));
    let lp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::CYAN, 0.9)).with_text_align(femtovg::Align::Center);
    let wp = lp.clone().with_color(Palette::WHITE);
    for (i, &(y0, h, s)) in rows.iter().enumerate() {
        let b = band_box(r, y0, h);
        gfx::text(canvas, r.x + r.w / 2.0, b.y + 21.0, &format!("MT-{:02}", i + 1), if s == AgentStatus::Blocked { &wp } else { &lp });
    }
    let sp = gfx::text_paint(&fonts.bold(), 26.0, Palette::CYAN).with_text_align(femtovg::Align::Center).with_letter_spacing(2.0);
    gfx::text(canvas, r.x + r.w / 2.0, r.bottom() - 10.0, "SYNC", &sp);
}

/// Live part of the band: teal chevrons scrolling down while `moving`, never over a label box or
/// the SYNC caption (so the static labels stay on the cached layer). One path.
pub fn chevron_band_live(canvas: &mut Cv, r: Rect, rows: &[(f32, f32, AgentStatus)], t: f32, moving: bool) {
    let step = 26.0;
    let offset = if moving { (t * 1.5).fract() * step } else { 0.0 };
    let boxes: Vec<Rect> = rows.iter().map(|&(y0, h, _)| band_box(r, y0, h)).collect();
    let clear = |y: f32| y > r.y + 2.0 && y + 14.0 < r.bottom() - 40.0 && !boxes.iter().any(|b| y + 14.0 > b.y - 2.0 && y < b.bottom() + 2.0);
    let mut p = Path::new();
    let mut y = r.y - step + offset;
    while y < r.bottom() {
        if clear(y) {
            p.move_to(r.x + 12.0, y);
            p.line_to(r.x + r.w / 2.0, y + 14.0);
            p.line_to(r.right() - 12.0, y);
        }
        y += step;
    }
    canvas.stroke_path(&p, &Paint::color(gfx::alpha(Palette::CYAN, 0.55)).with_line_width(4.0));
}

/// Tick ruler along a line: minor ticks every `step`, labelled majors `−20 … ±0 … +20`.
pub fn ruler(canvas: &mut Cv, x: f32, y: f32, len: f32, vertical: bool, color: Color, fonts: &Fonts) {
    ruler_ticks(canvas, x, y, len, vertical, 18.0, color);
    let n = 40;
    let step = len / n as f32;
    let lp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(color, 0.7));
    for (i, label) in ["-20", "-10", "±0", "+10", "+20"].iter().enumerate() {
        let o = i as f32 * 10.0 * step;
        if vertical {
            gfx::text(canvas, x + 22.0, y + o + 8.0, label, &lp);
        } else {
            gfx::text(canvas, x + o + 4.0, y - 22.0, label, &lp);
        }
    }
}

/// The ticks of a ruler only (40 steps: major every 10, mid every 5), `max_tick` long, pointing
/// right (vertical) or up (horizontal) from (x, y). For edges too narrow for labels.
pub fn ruler_ticks(canvas: &mut Cv, x: f32, y: f32, len: f32, vertical: bool, max_tick: f32, color: Color) {
    let n = 40;
    let step = len / n as f32;
    let mut p = Path::new();
    for i in 0..=n {
        let tick = max_tick * if i % 10 == 0 { 1.0 } else if i % 5 == 0 { 0.66 } else { 0.4 };
        let o = i as f32 * step;
        if vertical {
            p.move_to(x, y + o);
            p.line_to(x + tick, y + o);
        } else {
            p.move_to(x + o, y);
            p.line_to(x + o, y - tick);
        }
    }
    canvas.stroke_path(&p, &Paint::color(gfx::alpha(color, 0.7)).with_line_width(1.5));
}

/// Static yellow/black structural hazard strip (pin: "NERV ONLY" bar); stripes run across the
/// strip whichever way it lies.
pub fn structural_hazard(canvas: &mut Cv, r: Rect) {
    if r.w >= r.h {
        gfx::hazard(canvas, r, 0.0, Palette::TIMER, Palette::BLACK);
        return;
    }
    canvas.save();
    canvas.translate(r.x, r.bottom());
    canvas.rotate(-std::f32::consts::FRAC_PI_2);
    gfx::hazard(canvas, Rect::new(0.0, 0.0, r.h, r.w), 0.0, Palette::TIMER, Palette::BLACK);
    canvas.restore();
}

/// `+` crosshair mark.
pub fn crosshair(canvas: &mut Cv, x: f32, y: f32, size: f32, color: Color) {
    let mut p = Path::new();
    p.move_to(x - size, y);
    p.line_to(x + size, y);
    p.move_to(x, y - size);
    p.line_to(x, y + size);
    canvas.stroke_path(&p, &Paint::color(color).with_line_width(2.0));
}

/// Run `draw` with a horizontal shear of `degrees` about the horizontal line `pivot_y`, so the
/// group leans without drifting out of its box.
pub fn skewed(canvas: &mut Cv, degrees: f32, pivot_y: f32, draw: impl FnOnce(&mut Cv)) {
    canvas.save();
    canvas.translate(0.0, pivot_y);
    canvas.skew_x(degrees.to_radians());
    canvas.translate(0.0, -pivot_y);
    draw(canvas);
    canvas.restore();
}

/// Yellow/black structural hazard edge.
pub fn caution_edge(canvas: &mut Cv, r: Rect, t: f32) {
    gfx::hazard(canvas, r, t * 0.15, Palette::TIMER, Palette::BLACK);
}
