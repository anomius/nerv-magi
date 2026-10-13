//! Glyph sprites for live text that femtovg would otherwise draw as outline paths every frame:
//! glyphs under a skew (the −10° timer group) or bigger than 92 device px. Each character of a
//! small charset is rendered once, halo included, into an atlas image at device resolution; a
//! string is then one textured quad per character, transformed (skew included) like any fill.

use femtovg::{Align, Color, ImageFlags, ImageId, Paint, Path, PixelFormat, RenderTarget};

use super::gfx::{self, Cv};

/// Characters a sprite font holds (7-segment readouts).
pub const DIGITS: &str = "0123456789:.-";

/// Pen x of every drawable character of `s` (those `advance` knows), aligned at `x`, and the total
/// advance width.
pub fn pen_positions(s: &str, align: Align, x: f32, advance: impl Fn(char) -> Option<f32>) -> (Vec<(char, f32)>, f32) {
    let mut pens = Vec::new();
    let mut w = 0.0;
    for ch in s.chars() {
        if let Some(a) = advance(ch) {
            pens.push((ch, w));
            w += a;
        }
    }
    let x0 = match align {
        Align::Left => x,
        Align::Right => x - w,
        Align::Center => x - w / 2.0,
    };
    (pens.into_iter().map(|(c, p)| (c, x0 + p)).collect(), w)
}

/// Left edge of each atlas cell (pad + whole-pixel advance + pad, side by side) and the atlas width.
pub fn atlas_cells(advances_px: &[f32], pad_px: f32) -> (Vec<f32>, f32) {
    let mut x = 0.0;
    let mut xs = Vec::with_capacity(advances_px.len());
    for a in advances_px {
        xs.push(x);
        x += pad_px * 2.0 + a.ceil();
    }
    (xs, x)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    name: &'static str,
    size: f32,
    color: Color,
    glow: Color,
    blur: f32,
    bloom: f32,
    scale: f32,
}

struct SpriteFont {
    key: Key,
    image: ImageId,
    /// Atlas size, pad around each glyph and ascent, in device pixels.
    atlas: (f32, f32),
    pad: f32,
    ascent: f32,
    /// (char, cell x, advance) in device pixels.
    chars: Vec<(char, f32, f32)>,
}

/// Sprite fonts in use. `scale` (device px per design unit) and `target` (where the current pass
/// renders) are set by the view before each pass.
pub struct Cache {
    fonts: Vec<SpriteFont>,
    pub scale: f32,
    pub target: RenderTarget,
}

impl Default for Cache {
    fn default() -> Self {
        Self { fonts: Vec::new(), scale: 1.0, target: RenderTarget::Screen }
    }
}

impl Cache {
    fn get(&mut self, c: &mut Cv, key: Key, paint: &Paint) -> Option<usize> {
        if let Some(i) = self.fonts.iter().position(|f| f.key == key) {
            return Some(i);
        }
        // a new window scale makes every older font useless
        let scale = self.scale;
        self.fonts.retain(|f| {
            let keep = f.key.scale == scale;
            if !keep {
                c.delete_image(f.image);
            }
            keep
        });
        let f = build(c, key, paint, self.target)?;
        self.fonts.push(f);
        Some(self.fonts.len() - 1)
    }
}

/// Render DIGITS once at device size, halo included, into a premultiplied atlas image.
fn build(c: &mut Cv, key: Key, paint: &Paint, target: RenderTarget) -> Option<SpriteFont> {
    c.save();
    c.reset_transform();
    c.reset_scissor();
    let built = (|| {
        let p = paint.clone().with_font_size(key.size * key.scale).with_color(key.color).with_text_align(Align::Left);
        let m = c.measure_font(&p).ok()?;
        let ascent = m.ascender().ceil();
        let descent = (-m.descender()).ceil().max(0.0);
        let pad = (key.blur * key.scale * 0.3 * key.bloom).ceil() + 2.0;
        let advances: Vec<f32> = DIGITS.chars().map(|ch| gfx::text_width(c, &ch.to_string(), &p)).collect();
        let (xs, w) = atlas_cells(&advances, pad);
        let h = ascent + descent + 2.0 * pad;
        let image = c
            .create_image_empty(
                w as usize,
                h as usize,
                PixelFormat::Rgba8,
                ImageFlags::FLIP_Y | ImageFlags::PREMULTIPLIED,
            )
            .ok()?;
        c.set_render_target(RenderTarget::Image(image));
        c.clear_rect(0, 0, w as u32, h as u32, Color::rgbaf(0.0, 0.0, 0.0, 0.0));
        for (ch, x) in DIGITS.chars().zip(&xs) {
            gfx::glow_text(c, x + pad, pad + ascent, &ch.to_string(), &p, key.glow, key.blur * key.scale);
        }
        c.set_render_target(target);
        let chars = DIGITS.chars().zip(xs).zip(advances).map(|((ch, x), a)| (ch, x, a)).collect();
        Some(SpriteFont { key, image, atlas: (w, h), pad, ascent, chars })
    })();
    c.restore();
    built
}

/// `gfx::glow_text` from sprites: same placement (the paint's font, size and alignment, baseline
/// `y`, design units, current transform), one quad per character, in `color`. `name` tells the
/// uses apart. Characters outside DIGITS are skipped; if the atlas cannot be built it falls back
/// to plain glow text.
#[allow(clippy::too_many_arguments)]
pub fn glow_text(
    c: &mut Cv,
    cache: &mut Cache,
    name: &'static str,
    x: f32,
    y: f32,
    s: &str,
    paint: &Paint,
    color: Color,
    glow: Color,
    blur: f32,
) -> f32 {
    let key = Key { name, size: paint.font_size(), color, glow, blur, bloom: gfx::bloom(), scale: cache.scale };
    let Some(i) = cache.get(c, key, paint) else {
        return gfx::glow_text(c, x, y, s, &paint.clone().with_color(color), glow, blur);
    };
    let f = &cache.fonts[i];
    let k = cache.scale;
    let lookup = |ch: char| f.chars.iter().find(|e| e.0 == ch).copied();
    let (pens, w) = pen_positions(s, paint.text_align(), x, |ch| lookup(ch).map(|e| e.2 / k));
    let (aw, ah) = (f.atlas.0 / k, f.atlas.1 / k);
    let top = y - (f.ascent + f.pad) / k;
    for (ch, px) in pens {
        let Some((_, cx, adv)) = lookup(ch) else { continue };
        let left = px - f.pad / k;
        let mut quad = Path::new();
        quad.rect(left, top, (adv.ceil() + 2.0 * f.pad) / k, ah);
        c.fill_path(&quad, &Paint::image(f.image, left - cx / k, top, aw, ah, 0.0, 1.0));
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adv(c: char) -> Option<f32> {
        match c {
            '0'..='9' => Some(10.0),
            ':' | '.' => Some(4.0),
            _ => None,
        }
    }

    #[test]
    fn pens_follow_advances_and_alignment() {
        let (pens, w) = pen_positions("12:3", Align::Left, 100.0, adv);
        assert_eq!(pens, vec![('1', 100.0), ('2', 110.0), (':', 120.0), ('3', 124.0)]);
        assert_eq!(w, 34.0);
        let (pens, _) = pen_positions("12:3", Align::Right, 100.0, adv);
        assert_eq!(pens[0], ('1', 66.0));
        let (pens, _) = pen_positions("12", Align::Center, 100.0, adv);
        assert_eq!(pens[0], ('1', 90.0));
    }

    #[test]
    fn unknown_characters_are_skipped() {
        let (pens, w) = pen_positions("1x2", Align::Left, 0.0, adv);
        assert_eq!(pens, vec![('1', 0.0), ('2', 10.0)]);
        assert_eq!(w, 20.0);
    }

    #[test]
    fn atlas_cells_are_padded_and_whole_pixels() {
        let (xs, w) = atlas_cells(&[10.4, 4.0, 10.4], 3.0);
        // each cell: pad + ceil(advance) + pad, starting on whole pixels
        assert_eq!(xs, vec![0.0, 17.0, 27.0]);
        assert_eq!(w, 44.0);
    }
}
