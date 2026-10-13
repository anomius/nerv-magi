//! S-DAT 音声: tape reels, track, progress, output device, cava spectrum.

use femtovg::{Align, Paint, Path};

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::{PlayStatus, Source};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

/// Both tape reels (outer ring, hub, three spokes each) in two paths.
/// Tape reels: the rings never move (base layer), the spokes turn (live layer).
fn reels(c: &mut gfx::Cv, centers: [(f32, f32); 2], radius: f32, angles: [f32; 2], live: bool, layer: crate::tv::layers::Layer) {
    let color = if live { Palette::ORANGE } else { gfx::alpha(Palette::ORANGE, 0.45) };
    if layer.base() {
        let mut rings = Path::new();
        for &(cx, cy) in &centers {
            rings.circle(cx, cy, radius);
            rings.circle(cx, cy, radius * 0.28);
        }
        c.stroke_path(&rings, &Paint::color(color).with_line_width(3.0));
    }
    if !layer.live() {
        return;
    }
    let mut spokes = Path::new();
    for (&(cx, cy), &angle) in centers.iter().zip(&angles) {
        for k in 0..3 {
            let a = angle + k as f32 * std::f32::consts::TAU / 3.0;
            spokes.move_to(cx + a.cos() * radius * 0.28, cy + a.sin() * radius * 0.28);
            spokes.line_to(cx + a.cos() * radius * 0.86, cy + a.sin() * radius * 0.86);
        }
    }
    c.stroke_path(&spokes, &Paint::color(color).with_line_width(5.0));
}

pub fn draw(ctx: &mut Ctx, r: Rect) {
    let (base_layer, live_layer) = (ctx.layer.base(), ctx.layer.live());
    if base_layer {
        plate::plate(ctx.canvas, r, Cuts::TR_BL, "S-DAT", Some("音声"), Palette::ORANGE, ctx.fonts);
    }
    let fonts = ctx.fonts;
    let media = ctx.state.media.clone();
    let audio = ctx.state.audio.clone();
    let playing = media.as_ref().is_some_and(|m| m.status == PlayStatus::Playing);
    let top = r.y + plate::PLATE_TOP + 10.0;

    // tape reels, 0.5 rev/s while playing
    let angle = if playing { ctx.t * std::f32::consts::PI } else { 0.0 };
    let rr = 62.0;
    let (c1, c2) = ((r.x + 40.0 + rr, top + rr), (r.x + 40.0 + rr * 3.0 + 24.0, top + rr));
    let c = &mut *ctx.canvas;
    reels(c, [c1, c2], rr, [angle, angle * 1.3], playing, ctx.layer);
    let tx = c2.0 + rr + 40.0;
    let tw = r.right() - 40.0 - tx;
    // geometry shared by both layers
    let y_rail = top + 2.0 * rr + 30.0;
    let y_out = y_rail + 52.0;
    let area = Rect::new(r.x + 40.0, y_out + 22.0, r.w - 80.0, r.bottom() - 30.0 - y_out - 22.0);
    let no_signal = ctx.state.offline.contains_key(&Source::Spectrum) || ctx.state.spectrum.is_empty();
    if live_layer && !no_signal {
        cava(ctx, area);
    }
    if !base_layer {
        return;
    }
    let c = &mut *ctx.canvas;
    gfx::line(c, c1.0, c1.1 + rr, c2.0, c2.1 + rr, gfx::alpha(Palette::ORANGE, 0.6), 2.0);

    match &media {
        None => {
            let p = gfx::text_paint(&fonts.bold(), 52.0, Palette::DIM).with_letter_spacing(6.0);
            gfx::text(c, tx, top + 80.0, "NO TAPE", &p);
            let q = gfx::text_paint(&fonts.mono(), 22.0, Palette::DIM);
            gfx::text(c, tx, top + 116.0, "NO MPRIS PLAYER", &q);
        }
        Some(m) => {
            let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.5);
            let status = match m.status {
                PlayStatus::Playing => ("▶ PLAYING", Palette::GREEN),
                PlayStatus::Paused => ("▮▮ PAUSED", Palette::AMBER),
                PlayStatus::Stopped => ("■ STOPPED", Palette::DIM),
            };
            let w = gfx::text(c, tx, top + 18.0, "TRACK", &lp);
            gfx::text(c, tx + w + 16.0, top + 18.0, status.0, &gfx::text_paint(&fonts.mono(), 22.0, status.1));
            let tp = gfx::text_paint(&fonts.display(), 38.0, Palette::WHITE);
            gfx::text(c, tx, top + 60.0, &gfx::ellipsize(c, &tp, &m.title.to_uppercase(), tw), &tp);
            let ap = gfx::text_paint(&fonts.label(), 30.0, Palette::ORANGE);
            gfx::text(c, tx, top + 96.0, &gfx::ellipsize(c, &ap, &m.artist, tw), &ap);
            let bp = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.55));
            let album = format!("{} · {}", if m.album.is_empty() { "--" } else { &m.album }, m.player.to_uppercase());
            gfx::text(c, tx, top + 126.0, &gfx::ellipsize(c, &bp, &album, tw), &bp);
        }
    }

    // progress rail with times
    let y = top + 2.0 * rr + 30.0;
    let rail = Rect::new(r.x + 40.0, y, r.w - 80.0 - 250.0, 10.0);
    gfx::stroke_rect(c, rail, gfx::alpha(Palette::CYAN, 0.6), 1.5);
    if let Some(m) = &media {
        let pos = m.position_at(ctx.state.now);
        if let Some(len) = m.length_s.filter(|l| *l > 0.0) {
            gfx::fill_rect(c, Rect::new(rail.x, rail.y, rail.w * (pos / len).clamp(0.0, 1.0) as f32, rail.h), Palette::CYAN);
        }
        let tp = gfx::text_paint(&fonts.mono(), 26.0, Palette::WHITE).with_text_align(Align::Right);
        let len = m.length_s.map_or("--:--".to_string(), mmss);
        gfx::text(c, r.right() - 40.0, y + 12.0, &format!("{} / {len}", mmss(pos)), &tp);
    }

    // output device and volume
    let y = y + 52.0;
    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.8)).with_letter_spacing(1.5);
    let w = gfx::text(c, r.x + 40.0, y, "OUT", &lp);
    match &audio {
        Some(a) => {
            let op = gfx::text_paint(&fonts.mono(), 26.0, Palette::WHITE);
            let name = gfx::ellipsize(c, &op, &a.sink_name, 380.0);
            let nw = gfx::text(c, r.x + 52.0 + w, y, &name, &op);
            let vx = r.x + 52.0 + w + nw + 40.0;
            let vw = gfx::text(c, vx, y, "VOL", &lp);
            let vol = if a.muted { "MUTED".to_string() } else { format!("{:.0}%", a.volume * 100.0) };
            let vp = gfx::text_paint(&fonts.mono(), 26.0, if a.muted { Palette::RED } else { Palette::WHITE });
            let vtw = gfx::text(c, vx + vw + 12.0, y, &vol, &vp);
            let bx = vx + vw + vtw + 32.0;
            if r.right() - 40.0 - bx > 60.0 {
                gfx::seg_bar(c, Rect::new(bx, y - 18.0, r.right() - 40.0 - bx, 18.0), (a.volume / 1.5).clamp(0.0, 1.0), 30, if a.muted { gfx::alpha(Palette::RED, 0.5) } else { Palette::LCD }, gfx::alpha(Palette::DIM, 0.3));
            }
        }
        None => {
            gfx::text(c, r.x + 52.0 + w, y, "NO SIGNAL", &gfx::text_paint(&fonts.mono(), 26.0, Palette::DIM));
        }
    }

    // cava baseline (bars are live)
    let _ = y;
    let c = &mut *ctx.canvas;
    gfx::line(c, area.x, area.bottom(), area.right(), area.bottom(), gfx::alpha(Palette::CYAN, 0.5), 1.5);
    if no_signal {
        let p = gfx::text_paint(&fonts.bold(), 44.0, Palette::DIM).with_letter_spacing(6.0);
        gfx::text_center(c, area, "NO SIGNAL", &p);
    }
}

/// cava bars (one path, one cyan→magenta gradient by height) with falling peak caps.
fn cava(ctx: &mut Ctx, area: Rect) {
    let bars = ctx.state.spectrum.clone();
    let peaks = ctx.anim.peaks(&bars, ctx.t).to_vec();
    let c = &mut *ctx.canvas;
    let n = bars.len() as f32;
    let slot = area.w / n;
    let bw = slot * 0.7;
    let mut body = Path::new();
    let mut caps = Path::new();
    for (i, (&v, &pk)) in bars.iter().zip(&peaks).enumerate() {
        let x = area.x + slot * i as f32 + (slot - bw) / 2.0;
        let h = area.h * v.clamp(0.0, 1.0);
        body.rect(x, area.bottom() - h, bw, h);
        let py = area.bottom() - area.h * pk.clamp(0.0, 1.0);
        caps.rect(x, py - 4.0, bw, 4.0);
    }
    c.fill_path(&body, &Paint::linear_gradient(0.0, area.bottom(), 0.0, area.y, Palette::CYAN, Palette::MAGENTA));
    c.fill_path(&caps, &Paint::color(Palette::WHITE));
}

/// "01:12" (minutes may exceed 59: "60:05").
pub fn mmss(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmss_formats() {
        assert_eq!(mmss(72.4), "01:12");
        assert_eq!(mmss(3600.0 + 5.0), "60:05");
    }
}
