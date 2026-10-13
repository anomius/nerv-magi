//! BOOT: the loading screen on launch, and the reconnecting variant while herdr is unreachable.
//! An original title card (not an episode-card copy), check lines appearing every 150 ms with a
//! lamp that turns green when that source really has data, and a LOADING bar.

use femtovg::{Align, Paint, Path};

use crate::state::{HerdrLink, Source, State};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::{Rect, DESIGN_H, DESIGN_W};
use crate::tv::panels::{plate, Ctx};

/// (label, ready?) for each boot line after the first.
pub fn checks(s: &State) -> Vec<(&'static str, bool)> {
    vec![
        ("LINK HERDR", s.herdr.link == HerdrLink::Linked),
        ("SENSORS", s.sys.is_some()),
        ("POWER", s.power.is_some()),
        ("AUDIO", s.audio.is_some()),
        ("SPECTRUM", !s.spectrum.is_empty() && !s.offline.contains_key(&Source::Spectrum)),
        ("PILOTS", !s.herdr.agents.is_empty()),
    ]
}

pub fn draw(ctx: &mut Ctx, reconnecting: bool) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    gfx::fill_rect(c, Rect::new(0.0, 0.0, DESIGN_W, DESIGN_H), Palette::BLACK);
    let since = ctx.scene.since;

    // title card, left: compressed Mincho, white on black, thin red rule
    c.save();
    c.translate(220.0, 0.0);
    c.scale(0.72, 1.0);
    let big = gfx::text_paint(&fonts.mincho(), 340.0, Palette::WHITE);
    gfx::glow_text(c, 0.0, 900.0, "MAGI 起動", &big, gfx::alpha(Palette::WHITE, 0.35), 16.0);
    c.restore();
    gfx::fill_rect(c, Rect::new(220.0, 960.0, 1520.0, 4.0), Palette::RED);
    let sub = gfx::text_paint(&fonts.bold(), 92.0, Palette::WHITE).with_letter_spacing(14.0);
    gfx::text(c, 220.0, 1090.0, "NERV · AGENT CONTROL", &sub);
    let note = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::WHITE, 0.6)).with_letter_spacing(2.0);
    let host = ctx.state.sysx.as_ref().map_or("--".to_string(), |x| x.hostname.to_uppercase());
    gfx::text(c, 220.0, 1160.0, &format!("TERMINAL {host} · HERDR PLUGIN NERV.MAGI · 3840×2160"), &note);
    plate::ruler(c, 220.0, 1240.0, 1520.0, false, Palette::RED, fonts);

    let checks = checks(ctx.state);
    let ready = checks.iter().filter(|c| c.1).count();
    let x = 2140.0;
    if reconnecting {
        let warn = gfx::text_paint(&fonts.bold(), 76.0, Palette::AMBER).with_letter_spacing(8.0);
        gfx::text(c, x, 560.0, "LINK LOST · RECONNECTING", &warn);
        let lost = ctx.scene.lost_for.unwrap_or(0.0);
        let seg = gfx::text_paint(&fonts.segment(), 150.0, Palette::AMBER);
        gfx::text(c, x, 800.0, "888.8", &seg.clone().with_color(gfx::alpha(Palette::AMBER, 0.1)));
        gfx::glow_text(c, x, 800.0, &format!("{:5.1}", lost.min(999.9)), &seg, gfx::alpha(Palette::AMBER, 0.7), 12.0);
        let lp = gfx::text_paint(&fonts.label(), 30.0, Palette::AMBER).with_letter_spacing(2.0);
        gfx::text(c, x + 560.0, 800.0, "SECONDS SINCE LINK LOST", &lp);
        let reason = match &ctx.state.herdr.link {
            HerdrLink::Lost { reason, .. } => reason.clone(),
            _ => String::new(),
        };
        let dp = gfx::text_paint(&fonts.mono(), 32.0, gfx::alpha(Palette::WHITE, 0.75));
        gfx::text(c, x, 900.0, &format!("LAST KNOWN PILOTS {} · SECTORS {}", ctx.state.herdr.agents.len(), ctx.state.herdr.workspaces.len()), &dp);
        let rp = gfx::text_paint(&fonts.mono(), 28.0, gfx::alpha(Palette::AMBER, 0.8));
        gfx::text(c, x, 950.0, &gfx::ellipsize(c, &rp, &format!("REASON {}", reason.to_uppercase()), 1580.0), &rp);
        gfx::text(c, x, 1000.0, "RETRY 1 s → 2 s → 5 s · FULL RE-SNAPSHOT ON RELINK", &rp);
        loading(ctx, 0.0, "LINK", ctx.t);
        return;
    }

    // check lines: one per 150 ms, lamp green when that source has data
    let shown = ((since / 0.15) as usize).min(checks.len() + 1);
    let head = gfx::text_paint(&fonts.mono(), 44.0, Palette::RED).with_letter_spacing(2.0);
    if shown >= 1 {
        gfx::text(c, x, 520.0, "MAGI SYSTEM BOOT", &head);
        gfx::text(c, x + 1100.0, 520.0, "MELCHIOR·1 BALTHASAR·2 CASPER·3", &gfx::text_paint(&fonts.mono(), 26.0, gfx::alpha(Palette::RED, 0.7)));
    }
    let lp = gfx::text_paint(&fonts.mono(), 40.0, Palette::RED);
    let dots = gfx::text_paint(&fonts.mono(), 40.0, gfx::alpha(Palette::RED, 0.45));
    for (i, (label, ok)) in checks.iter().enumerate().take(shown.saturating_sub(1)) {
        let y = 620.0 + i as f32 * 86.0;
        let lamp = Rect::new(x, y - 30.0, 30.0, 30.0);
        gfx::fill_rect(c, lamp, if *ok { Palette::GREEN } else { gfx::alpha(Palette::AMBER, 0.35 + 0.35 * (ctx.t * 4.0).sin().abs()) });
        let w = gfx::text(c, x + 56.0, y, label, &lp);
        let n = ((980.0 - w) / 24.0).max(3.0) as usize;
        gfx::text(c, x + 76.0 + w, y, &"·".repeat(n), &dots);
        let result = if *label == "PILOTS" {
            (format!("{} DETECTED", ctx.state.herdr.agents.len()), if *ok { Palette::GREEN } else { Palette::AMBER })
        } else if *ok {
            ("CHECK OK".to_string(), Palette::GREEN)
        } else {
            ("WAITING".to_string(), Palette::AMBER)
        };
        gfx::text(c, x + 1100.0, y, &result.0, &gfx::text_paint(&fonts.mono(), 40.0, result.1));
    }
    let frac = ready as f32 / checks.len() as f32;
    loading(ctx, frac, "LOADING", ctx.t);
}

/// Full-width segmented LOADING bar with its percentage; `frac` 0 animates an idle sweep.
fn loading(ctx: &mut Ctx, frac: f32, label: &str, t: f32) {
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let r = Rect::new(220.0, 1500.0, 3400.0, 70.0);
    let lp = gfx::text_paint(&fonts.bold(), 44.0, Palette::ORANGE).with_letter_spacing(6.0);
    gfx::text(c, r.x, r.y - 24.0, label, &lp);
    let pp = gfx::text_paint(&fonts.mono(), 44.0, Palette::ORANGE).with_text_align(Align::Right);
    gfx::text(c, r.right(), r.y - 24.0, &format!("{:3.0}%", frac * 100.0), &pp);
    gfx::seg_bar(c, r, frac, 60, Palette::ORANGE, gfx::alpha(Palette::ORANGE, 0.12));
    if frac <= 0.0 {
        // reconnect sweep: a lit block bouncing along the bar
        let k = (t * 0.6).rem_euclid(2.0);
        let pos = if k < 1.0 { k } else { 2.0 - k };
        gfx::fill_rect(c, Rect::new(r.x + (r.w - 300.0) * pos, r.y, 300.0, r.h), gfx::alpha(Palette::AMBER, 0.6));
    }
    plate::caution_edge(c, Rect::new(0.0, 1700.0, DESIGN_W, 40.0), t);
    let mut p = Path::new();
    p.rect(0.0, 2080.0, DESIGN_W, 4.0);
    c.fill_path(&p, &Paint::color(gfx::alpha(Palette::RED, 0.6)));
    let fp = gfx::text_paint(&fonts.mono(), 26.0, gfx::alpha(Palette::WHITE, 0.5));
    gfx::text(c, 220.0, 2050.0, "READ-ONLY OBSERVER · NEVER SENDS INPUT · q / Esc TO CLOSE", &fp);
}
