//! Full-screen views (plan REVISION 2): MAIN, LOAD, EMERGENCY and BOOT, chosen per frame by the
//! scene Director, with a hard Eva cut (horizontal wipe + glitch band) between them.

pub mod boot;
pub mod emergency;
pub mod load;
pub mod main;
pub mod sync;

use femtovg::{Paint, Path};

use super::gfx::{self, Palette};
use super::layout::{Layout, Rect, DESIGN_H, DESIGN_W};
use super::panels::{alert, plate, Ctx};
use super::scene::Scene;
use crate::state::AgentStatus;

/// Seconds after a cut during which text draws are flushed in batches (gfx::ColdBatcher).
pub const COLD_S: f32 = 0.2;

pub fn draw(ctx: &mut Ctx, layout: &Layout) {
    gfx::set_cold(ctx.scene.since < COLD_S);
    let live = ctx.layer.live();
    if ctx.layer == crate::tv::layers::Layer::Base {
        ctx.anim.overlays.clear();
    }
    if live {
        // once per frame (the live pass runs every frame; the base pass does not)
        observe_transitions(ctx);
        sample_history(ctx);
    }
    match ctx.scene.scene {
        Scene::Main => main::draw(ctx, layout),
        Scene::Load => load::draw(ctx, layout),
        Scene::Sync => sync::draw(ctx, layout),
        Scene::Emergency(cause) => emergency::draw(ctx, cause),
        Scene::Boot { reconnecting } => boot::draw(ctx, reconnecting),
    }
    if !live {
        return;
    }
    if ctx.layer == crate::tv::layers::Layer::Live {
        let recorded = ctx.anim.overlays.clone();
        for (r, color) in recorded {
            crate::tv::bench::span("live acquiring", || crate::tv::panels::telemetry::acquiring_overlay(ctx.canvas, r, ctx.t, ctx.fonts, color));
        }
    }
    let red = matches!(ctx.scene.scene, Scene::Emergency(_)) && !ctx.scene.preview;
    crate::tv::bench::span("fx", || crate::tv::fx::draw(ctx.canvas, ctx.fx, layout, ctx.scanlines, ctx.grain, red, &ctx.config.tv, ctx.t));
    wipe(ctx);
    alert::flash(ctx);
    if ctx.help {
        help_overlay(ctx);
    }
    if let Some(text) = ctx.toast {
        crate::tv::fx::toast(ctx.canvas, ctx.fonts, text, layout);
    }
}

/// Remote words for the help footer (scripts/tv-ctl.sh).
const REMOTE_WORDS: &str = "tv-ctl.sh main|load|sync|emergency|next|prev|auto|pin|fullscreen|scanlines|grain|help";

/// The `?` overlay: a centered plate listing control::KEY_HELP, the current view mode and look.
fn help_overlay(ctx: &mut Ctx) {
    let rows = crate::control::KEY_HELP;
    let row_h = 64.0;
    let w = 1640.0;
    let h = plate::PLATE_TOP + 40.0 + rows.len() as f32 * row_h + 150.0;
    let r = Rect::new((DESIGN_W - w) / 2.0, (DESIGN_H - h) / 2.0, w, h);
    let fonts = ctx.fonts;
    let c = &mut *ctx.canvas;
    gfx::fill_rect(c, Rect::new(0.0, 0.0, DESIGN_W, DESIGN_H), gfx::alpha(Palette::BLACK, 0.6));
    plate::plate(c, r, plate::Cuts::TL_BR, "CONTROL", Some("操作"), Palette::ORANGE, fonts);
    // keys in Oswald; Fira Code covers the arrows Oswald lacks
    let key_fonts = [fonts.display()[0], fonts.mono()[0], fonts.mincho()[0]];
    let kp = gfx::text_paint(&key_fonts, 34.0, Palette::CYAN).with_letter_spacing(3.0);
    let tp = gfx::text_paint(&fonts.label(), 30.0, Palette::WHITE).with_letter_spacing(1.0);
    let x = r.x + 70.0;
    let mut y = r.y + plate::PLATE_TOP + 40.0 + 30.0;
    for (i, (k, d)) in rows.iter().enumerate() {
        if i % 2 == 0 {
            gfx::fill_rect(c, Rect::new(r.x + 40.0, y - 44.0, r.w - 80.0, row_h - 4.0), gfx::alpha(Palette::ORANGE, 0.05));
        }
        gfx::text(c, x, y, k, &kp);
        gfx::text(c, x + 400.0, y, &d.to_uppercase(), &tp);
        y += row_h;
    }
    y += 16.0;
    gfx::line(c, r.x + 40.0, y - 30.0, r.right() - 40.0, y - 30.0, gfx::alpha(Palette::ORANGE, 0.5), 1.5);
    let sp = gfx::text_paint(&fonts.mono(), 26.0, Palette::GREEN);
    let state = format!("VIEW {} · SCANLINES {} · GRAIN {}", ctx.scene.mode.label(), ctx.scanlines.label(), if ctx.grain { "ON" } else { "OFF" });
    gfx::text(c, x, y + 10.0, &state, &sp);
    let rp = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::WHITE, 0.55));
    let fit = gfx::ellipsize(c, &rp, &format!("REMOTE  {REMOTE_WORDS}"), r.w - 140.0);
    gfx::text(c, x, y + 56.0, &fit, &rp);
}

/// Tell the animator about new status transitions; flash if any of them went to Blocked.
fn observe_transitions(ctx: &mut Ctx) {
    let new = ctx.state.transitions.saturating_sub(ctx.anim.seen_transitions()) as usize;
    let newest = if ctx.state.ticker.iter().take(new).any(|e| e.to == AgentStatus::Blocked) {
        Some(AgentStatus::Blocked)
    } else {
        ctx.state.ticker.front().map(|e| e.to)
    };
    ctx.anim.observe(ctx.state.transitions, newest, ctx.t);
}

/// Per-core 1 Hz rings, sampled whatever the scene so LOAD has history when it cuts in.
pub const CORE_KEYS: [&str; 32] = [
    "core00", "core01", "core02", "core03", "core04", "core05", "core06", "core07", "core08", "core09", "core10", "core11", "core12", "core13", "core14", "core15",
    "core16", "core17", "core18", "core19", "core20", "core21", "core22", "core23", "core24", "core25", "core26", "core27", "core28", "core29", "core30", "core31",
];

fn sample_history(ctx: &mut Ctx) {
    if let Some(sys) = &ctx.state.sys {
        for (k, v) in CORE_KEYS.iter().zip(&sys.cpu_cores) {
            ctx.anim.ring(k, *v, ctx.t);
        }
    }
}

/// Hard cut: the new scene is revealed left→right over CUT_S behind a glitch band, tinted red
/// into EMERGENCY and orange otherwise.
fn wipe(ctx: &mut Ctx) {
    let p = ctx.scene.cut;
    if p >= 1.0 {
        return;
    }
    let tint = if matches!(ctx.scene.scene, Scene::Emergency(_)) { Palette::RED } else { Palette::ORANGE };
    let edge = DESIGN_W * p;
    let c = &mut *ctx.canvas;
    gfx::fill_rect(c, Rect::new(edge, 0.0, DESIGN_W - edge, DESIGN_H), Palette::BLACK);
    gfx::fill_rect(c, Rect::new(edge, 0.0, DESIGN_W - edge, DESIGN_H), gfx::alpha(tint, 0.10));
    // glitch band: scanline slices with per-row offsets
    let band_w = 160.0;
    let mut stripes = Path::new();
    let mut y = 0.0;
    let mut k = (ctx.t * 977.0) as u32;
    while y < DESIGN_H {
        k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let jitter = (k >> 16) as f32 / 65_536.0;
        let h = 6.0 + 18.0 * jitter;
        stripes.rect(edge - band_w * (0.3 + jitter), y, band_w * (0.4 + jitter), h * 0.6);
        y += h;
    }
    c.fill_path(&stripes, &Paint::color(gfx::alpha(tint, 0.55)));
    gfx::line(c, edge, 0.0, edge, DESIGN_H, tint, 4.0);
}
