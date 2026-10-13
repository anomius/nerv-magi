//! Post effects (plan REVISION 3/6): scanline looks off / subtle / heavy / crt, the rolling refresh
//! band, CRT flicker, animated film grain and the vignette, plus the 2 s SCANLINES toast when the
//! look is cycled.

use femtovg::imgref::Img;
use femtovg::rgb::RGBA8;
use femtovg::{ImageFlags, ImageId, ImageSource, Paint, Path};

use super::gfx::{self, Cv, Palette};
use super::layout::{Layout, Rect, DESIGN_H, DESIGN_W};
use crate::config::{ScanlineMode, TvConfig};

/// `--scanlines` wins; otherwise the config (Off when its master switch is off).
pub fn effective_mode(arg: Option<ScanlineMode>, tv: &TvConfig) -> ScanlineMode {
    arg.unwrap_or_else(|| tv.scanline_mode())
}

pub fn toast_text(m: ScanlineMode) -> String {
    format!("SCANLINES · {}", m.label())
}

/// One scanline pattern in device pixels: a dark line of `line_px` every `period_px`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pattern {
    pub period_px: f32,
    pub line_px: f32,
    pub alpha: f32,
}

/// The design-unit pattern of each look (subtle 2/4 @ 6 %, heavy 2/3 @ 14 %, crt 1/2 @ 18 %),
/// snapped to whole device pixels at `scale` so lines stay crisp at any window size.
pub fn pattern(m: ScanlineMode, scale: f32) -> Option<Pattern> {
    let (period, line, alpha) = match m {
        ScanlineMode::Off => return None,
        ScanlineMode::Subtle => (4.0, 2.0, 0.06),
        ScanlineMode::Heavy => (3.0, 2.0, 0.14),
        ScanlineMode::Crt => (2.0, 1.0, 0.18),
    };
    let period_px = f32::max(2.0, (period * scale).round());
    let line_px = (line * scale).round().clamp(1.0, period_px - 1.0);
    Some(Pattern { period_px, line_px, alpha })
}

/// Height of the rolling band and its period.
pub const ROLL_H: f32 = 120.0;
pub const ROLL_S: f32 = 8.0;

/// Top of the rolling band (design units) at `t`: moves top → bottom once every ROLL_S.
pub fn roll_y(t: f32, screen_h: f32) -> f32 {
    -ROLL_H + (screen_h + ROLL_H) * (t / ROLL_S).rem_euclid(1.0)
}

/// Darkening (0..0.03) of the CRT flicker at `t`, constant within each 1/12 s step.
pub fn flicker(t: f32) -> f32 {
    let step = (t * 12.0).floor() as i64 as u32;
    let h = step.wrapping_mul(2_654_435_761) >> 16;
    (h & 0xffff) as f32 / 65_535.0 * 0.03
}

/// Film grain: GRAIN_TILES noise tiles of GRAIN_TILE² device pixels, one picked per 1/GRAIN_FPS.
pub const GRAIN_TILE: usize = 256;
pub const GRAIN_TILES: usize = 4;
const GRAIN_FPS: f32 = 30.0;
/// Peak noise alpha at strength 1.0.
const GRAIN_PEAK: f32 = 0.08;

/// Effective grain strength: the config value (clamped to 0..1), ×1.4 in the CRT look.
pub fn grain_strength(cfg: f32, mode: ScanlineMode) -> f32 {
    cfg.clamp(0.0, 1.0) * if mode == ScanlineMode::Crt { 1.4 } else { 1.0 }
}

fn xorshift(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

/// One `size`² noise tile: random gray per pixel, alpha = strength × GRAIN_PEAK × u³ (u uniform),
/// so most pixels are near transparent and a few reach the peak. Deterministic per seed.
pub fn grain_tile(seed: u32, size: usize, strength: f32) -> Vec<RGBA8> {
    let peak = (strength * GRAIN_PEAK * 255.0).round().clamp(0.0, 255.0);
    let mut s = seed.wrapping_mul(0x9E37_79B9) | 1;
    (0..size * size)
        .map(|_| {
            let v = (xorshift(&mut s) >> 24) as u8;
            let u = (xorshift(&mut s) >> 8) as f32 / (1u32 << 24) as f32;
            RGBA8::new(v, v, v, (peak * u * u * u).round() as u8)
        })
        .collect()
}

/// Tile index and integer device-pixel offset at `t`; both change once per 1/GRAIN_FPS.
pub fn grain_step(t: f32) -> (usize, (u32, u32)) {
    let step = (t * GRAIN_FPS).floor().max(0.0) as u32;
    let a = step.wrapping_mul(2_654_435_761);
    let b = (a ^ (a >> 15)).wrapping_mul(2_246_822_519);
    let n = GRAIN_TILE as u32;
    (step as usize % GRAIN_TILES, ((a >> 8) % n, (b >> 8) % n))
}

/// Runtime grain switch (starts from `tv.grain`; `g` / tv-ctl grain toggle it).
#[derive(Debug, Clone, Copy)]
pub struct Grain {
    pub on: bool,
}

impl Grain {
    pub fn new(on: bool) -> Self {
        Self { on }
    }

    /// Flip, returning the toast text.
    pub fn toggle(&mut self) -> String {
        self.on = !self.on;
        super::input::grain_toast(self.on)
    }
}

/// Cached 1-tile images for the scanline and phosphor patterns and the grain tiles (one draw call
/// each per frame).
#[derive(Default)]
pub struct Images {
    scan: Option<(Pattern, ImageId)>,
    mask: Option<ImageId>,
    grain: Option<(f32, [ImageId; GRAIN_TILES])>,
}

impl Images {
    fn scan(&mut self, c: &mut Cv, p: Pattern) -> Option<ImageId> {
        if let Some((cached, id)) = self.scan {
            if cached == p {
                return Some(id);
            }
            c.delete_image(id);
            self.scan = None;
        }
        let h = p.period_px as usize;
        let a = (p.alpha * 255.0).round() as u8;
        let px: Vec<RGBA8> = (0..h).map(|y| if (y as f32) < p.line_px { RGBA8::new(0, 0, 0, a) } else { RGBA8::new(0, 0, 0, 0) }).collect();
        let id = c.create_image(ImageSource::Rgba(Img::new(px.as_slice(), 1, h)), pattern_flags()).ok()?;
        self.scan = Some((p, id));
        Some(id)
    }

    /// The grain tiles for `strength`, generated once (again only when the strength changes).
    fn grain(&mut self, c: &mut Cv, strength: f32) -> Option<[ImageId; GRAIN_TILES]> {
        if let Some((s, ids)) = self.grain {
            if s == strength {
                return Some(ids);
            }
            for id in ids {
                c.delete_image(id);
            }
            self.grain = None;
        }
        let mut ids = Vec::with_capacity(GRAIN_TILES);
        for i in 0..GRAIN_TILES {
            let px = grain_tile(0x5EED + i as u32, GRAIN_TILE, strength);
            match c.create_image(ImageSource::Rgba(Img::new(px.as_slice(), GRAIN_TILE, GRAIN_TILE)), pattern_flags()) {
                Ok(id) => ids.push(id),
                Err(_) => {
                    for id in ids {
                        c.delete_image(id);
                    }
                    return None;
                }
            }
        }
        let ids: [ImageId; GRAIN_TILES] = ids.try_into().ok()?;
        self.grain = Some((strength, ids));
        Some(ids)
    }

    fn mask(&mut self, c: &mut Cv) -> Option<ImageId> {
        if self.mask.is_none() {
            let a = (0.03f32 * 255.0).round() as u8;
            let px = [RGBA8::new(255, 0, 0, a), RGBA8::new(0, 255, 0, a), RGBA8::new(0, 0, 255, a)];
            self.mask = c.create_image(ImageSource::Rgba(Img::new(&px[..], 3, 1)), pattern_flags()).ok();
        }
        self.mask
    }
}

fn pattern_flags() -> ImageFlags {
    ImageFlags::REPEAT_X | ImageFlags::REPEAT_Y | ImageFlags::NEAREST
}

/// Draw the post effects over the finished view, in device pixels (so lines stay crisp). `grain`
/// is the runtime switch; `red` tints the grain (EMERGENCY).
#[allow(clippy::too_many_arguments)]
pub fn draw(c: &mut Cv, images: &mut Images, layout: &Layout, mode: ScanlineMode, grain: bool, red: bool, tv: &TvConfig, t: f32) {
    let screen = layout.to_px(Rect::new(0.0, 0.0, DESIGN_W, DESIGN_H));
    c.save();
    c.reset_transform();
    c.reset_scissor();
    let mut all = Path::new();
    all.rect(screen.x, screen.y, screen.w, screen.h);
    if let Some(p) = pattern(mode, layout.scale) {
        if let Some(id) = images.scan(c, p) {
            c.fill_path(&all, &Paint::image(id, screen.x, screen.y.floor(), 1.0, p.period_px, 0.0, 1.0));
        }
        if mode == ScanlineMode::Crt {
            if let Some(id) = images.mask(c) {
                c.fill_path(&all, &Paint::image(id, screen.x.floor(), 0.0, 3.0, 1.0, 0.0, 1.0));
            }
        }
        if tv.scanline_roll {
            let top = screen.y + roll_y(t, DESIGN_H) * layout.scale;
            let h = ROLL_H * layout.scale;
            let paint = Paint::linear_gradient_stops(
                0.0,
                top,
                0.0,
                top + h,
                [(0.0, gfx::alpha(Palette::WHITE, 0.0)), (0.5, gfx::alpha(Palette::WHITE, 0.04)), (1.0, gfx::alpha(Palette::WHITE, 0.0))],
            );
            let mut band = Path::new();
            band.rect(screen.x, top.max(screen.y), screen.w, (top + h).min(screen.bottom()) - top.max(screen.y));
            c.fill_path(&band, &paint);
        }
        if tv.crt_flicker {
            c.fill_path(&all, &Paint::color(gfx::alpha(Palette::BLACK, flicker(t))));
        }
    }
    let strength = grain_strength(tv.grain_strength, mode);
    if grain && strength > 0.0 {
        if let Some(ids) = images.grain(c, strength) {
            let (i, (ox, oy)) = grain_step(t);
            let (x, y) = (screen.x.floor() - ox as f32, screen.y.floor() - oy as f32);
            let n = GRAIN_TILE as f32;
            let paint = if red {
                Paint::image_tint(ids[i], x, y, n, n, 0.0, femtovg::Color::rgbf(1.0, 0.3, 0.3))
            } else {
                Paint::image(ids[i], x, y, n, n, 0.0, 1.0)
            };
            c.fill_path(&all, &paint);
        }
    }
    if tv.vignette {
        let (cx, cy) = (screen.x + screen.w / 2.0, screen.y + screen.h / 2.0);
        let outer = (screen.w * screen.w + screen.h * screen.h).sqrt() / 2.0;
        let paint = Paint::radial_gradient(cx, cy, outer * 0.55, outer, gfx::alpha(Palette::BLACK, 0.0), gfx::alpha(Palette::BLACK, 0.35));
        c.fill_path(&all, &paint);
    }
    c.restore();
}

/// The 2 s toast after the look is cycled: a box at the right end of the status-strip row.
pub fn toast(c: &mut Cv, fonts: &super::fonts::Fonts, text: &str, layout: &Layout) {
    let row = layout.alert;
    let p = gfx::text_paint(&fonts.bold(), 32.0, Palette::BLACK).with_letter_spacing(4.0);
    let w = gfx::text_width(c, text, &p) + 60.0;
    let b = Rect::new(row.right() - w, row.y - 4.0, w, row.h + 8.0);
    gfx::fill_rect(c, b, Palette::CYAN);
    gfx::text_center(c, b, text, &p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_resolution() {
        let mut tv = TvConfig::default();
        assert_eq!(effective_mode(None, &tv), ScanlineMode::Subtle);
        assert_eq!(effective_mode(Some(ScanlineMode::Crt), &tv), ScanlineMode::Crt);
        tv.scanlines = false;
        assert_eq!(effective_mode(None, &tv), ScanlineMode::Off);
        assert_eq!(effective_mode(Some(ScanlineMode::Heavy), &tv), ScanlineMode::Heavy); // explicit flag wins
        tv.scanlines = true;
        tv.scanline_style = ScanlineMode::Heavy;
        assert_eq!(effective_mode(None, &tv), ScanlineMode::Heavy);
    }

    #[test]
    fn cycling_from_the_effective_mode_and_toast() {
        let order: Vec<ScanlineMode> = std::iter::successors(Some(ScanlineMode::Off), |m| Some(m.next())).take(5).collect();
        assert_eq!(order, [ScanlineMode::Off, ScanlineMode::Subtle, ScanlineMode::Heavy, ScanlineMode::Crt, ScanlineMode::Off]);
        assert_eq!(toast_text(ScanlineMode::Heavy), "SCANLINES · HEAVY");
        assert_eq!(toast_text(ScanlineMode::Off), "SCANLINES · OFF");
    }

    #[test]
    fn patterns_match_spec_at_4k_and_stay_crisp_when_scaled() {
        assert_eq!(pattern(ScanlineMode::Off, 1.0), None);
        assert_eq!(pattern(ScanlineMode::Subtle, 1.0), Some(Pattern { period_px: 4.0, line_px: 2.0, alpha: 0.06 }));
        assert_eq!(pattern(ScanlineMode::Heavy, 1.0), Some(Pattern { period_px: 3.0, line_px: 2.0, alpha: 0.14 }));
        assert_eq!(pattern(ScanlineMode::Crt, 1.0), Some(Pattern { period_px: 2.0, line_px: 1.0, alpha: 0.18 }));
        for m in [ScanlineMode::Subtle, ScanlineMode::Heavy, ScanlineMode::Crt] {
            for scale in [0.5, 0.333, 0.75] {
                let p = pattern(m, scale).unwrap();
                assert!(p.period_px >= 2.0 && p.line_px >= 1.0 && p.line_px < p.period_px, "{m:?} {scale} {p:?}");
                assert_eq!(p.period_px.fract(), 0.0);
                assert_eq!(p.line_px.fract(), 0.0);
            }
        }
    }

    #[test]
    fn roll_moves_down_and_wraps() {
        let h = 2160.0;
        assert!(roll_y(1.0, h) < roll_y(2.0, h));
        assert!((roll_y(0.5, h) - roll_y(0.5 + ROLL_S, h)).abs() < 0.01);
        for t in [0.0, 3.3, 7.99, 123.4] {
            let y = roll_y(t, h);
            assert!((-ROLL_H..=h).contains(&y), "{t}: {y}");
        }
    }

    #[test]
    fn grain_tile_is_deterministic_per_seed() {
        let a = grain_tile(7, 64, 0.35);
        assert_eq!(a.len(), 64 * 64);
        assert_eq!(a, grain_tile(7, 64, 0.35));
        assert_ne!(a, grain_tile(8, 64, 0.35));
    }

    #[test]
    fn grain_alpha_is_capped_and_mostly_near_transparent() {
        for strength in [0.1, 0.35, 1.0] {
            let cap = (strength * 0.08 * 255.0f32).round() as u8;
            let tile = grain_tile(3, 128, strength);
            assert!(tile.iter().all(|p| p.a <= cap), "strength {strength}: max {}", tile.iter().map(|p| p.a).max().unwrap());
            assert!(tile.iter().any(|p| p.a == cap || p.a + 1 >= cap), "noise reaches its peak");
            // biased toward 0: most pixels are under a third of the peak
            let low = tile.iter().filter(|p| (p.a as f32) < cap as f32 / 3.0).count();
            assert!(low * 2 > tile.len(), "{low} of {}", tile.len());
        }
        assert!(grain_tile(1, 32, 0.0).iter().all(|p| p.a == 0));
    }

    #[test]
    fn grain_strength_follows_the_look() {
        assert!((grain_strength(0.35, ScanlineMode::Subtle) - 0.35).abs() < 1e-6);
        assert!((grain_strength(0.35, ScanlineMode::Crt) - 0.49).abs() < 1e-6);
        assert_eq!(grain_strength(2.0, ScanlineMode::Off), 1.0); // clamped
    }

    #[test]
    fn grain_steps_at_30_fps_through_the_tiles() {
        let (t0, o0) = grain_step(1.0);
        assert_eq!(grain_step(1.0 + 0.01), (t0, o0)); // same 1/30 s step
        let (t1, o1) = grain_step(1.0 + 1.0 / 30.0 + 0.001);
        assert_eq!(t1, (t0 + 1) % GRAIN_TILES);
        assert_ne!(o0, o1);
        for i in 0..100 {
            let (t, (x, y)) = grain_step(i as f32 * 0.021);
            assert!(t < GRAIN_TILES && (x as usize) < GRAIN_TILE && (y as usize) < GRAIN_TILE);
        }
    }

    #[test]
    fn grain_toggle_shows_a_toast() {
        let mut g = Grain::new(true);
        assert!(g.on);
        assert_eq!(g.toggle(), "GRAIN · OFF");
        assert!(!g.on);
        assert_eq!(g.toggle(), "GRAIN · ON");
        assert!(Grain::new(false).on == false);
    }

    #[test]
    fn flicker_is_small_and_steps_at_12hz() {
        for i in 0..200 {
            let f = flicker(i as f32 * 0.013);
            assert!((0.0..=0.03).contains(&f));
        }
        assert_eq!(flicker(1.001), flicker(1.0 + 1.0 / 12.0 - 0.001));
    }
}
