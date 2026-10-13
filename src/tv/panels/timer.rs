//! 活動限界まで: the activity-limit timer (battery time left), power source and mode lamps.

use femtovg::Align;

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::{BatStatus, PowerMode};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

/// Horizontal shear of the timer group (plan REVISION 2: about −10°).
pub const SKEW_DEG: f32 = -10.0;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    let (base_layer, live_layer) = (ctx.layer.base(), ctx.layer.live());
    if base_layer {
        plate::plate(ctx.canvas, r, Cuts::TL_BR, "ACTIVITY LIMIT", Some("活動限界まで"), Palette::ORANGE, ctx.fonts);
    }
    let power = ctx.state.power.clone();
    let bat = power.as_ref().and_then(|p| p.battery.clone());
    let counting = bat.as_ref().is_some_and(|b| matches!(b.status, BatStatus::Discharging | BatStatus::Charging));
    let left = bat.as_ref().and_then(|b| b.seconds_left).filter(|_| counting);
    let elapsed = ctx.anim.since_power_sample(left, ctx.t);
    let text = timer_text(left, elapsed);
    let fonts = ctx.fonts;
    let what = match bat.as_ref().map(|b| b.status) {
        Some(BatStatus::Charging) => "TIME TO FULL CHARGE",
        _ => "ACTIVE TIME REMAINING",
    };
    let on_ac = power.as_ref().is_some_and(|p| p.ac_online);
    let mode = power.as_ref().map(|p| p.profile.mode());
    let has_power = power.is_some();

    let group_top = r.y + plate::PLATE_TOP + 8.0;
    let group_bottom = r.y + 330.0;
    let pivot = (group_top + group_bottom) / 2.0;
    let sprites = &mut *ctx.sprites;
    plate::skewed(ctx.canvas, SKEW_DEG, pivot, |c| {
        let x0 = r.x + 40.0;
        if base_layer {
            let kp = gfx::text_paint(&fonts.mincho(), 34.0, Palette::ORANGE);
            let kw = gfx::text(c, x0, group_top + 34.0, "活動限界まで", &kp);
            let cp = gfx::text_paint(&fonts.display(), 24.0, Palette::ORANGE).with_letter_spacing(3.0);
            gfx::text(c, x0 + kw + 16.0, group_top + 32.0, what, &cp);
        }

        // 7-segment readout ≤ 150 px, scaled down if the ghost would not fit
        let max_w = r.w - 110.0;
        let mut seg = gfx::text_paint(&fonts.segment(), 150.0, Palette::TIMER);
        let ghost_w = gfx::text_width(c, "8:88:88.88", &seg);
        if ghost_w > max_w {
            seg = seg.with_font_size(150.0 * max_w / ghost_w);
        }
        let base = group_top + 196.0;
        if base_layer {
            gfx::text(c, x0, base, "8:88:88.88", &seg.clone().with_color(gfx::alpha(Palette::TIMER, 0.10)));
        }
        if live_layer {
            let shown = if left.is_some() { Palette::TIMER } else { gfx::alpha(Palette::TIMER, 0.45) };
            crate::tv::sprites::glow_text(c, sprites, "timer", x0, base, &text, &seg, shown, gfx::alpha(Palette::TIMER, 0.7), 14.0);
        }
        if !base_layer {
            return;
        }

        // power source plates + mode buttons
        let row_y = base + 22.0;
        let mut x = x0;
        for (kanji, latin, active) in [("内部", "INTERNAL", !on_ac && has_power), ("外部", "EXTERNAL", on_ac)] {
            let b = Rect::new(x, row_y, 196.0, 50.0);
            if active {
                gfx::fill_rect(c, b, Palette::ORANGE);
            } else {
                gfx::stroke_rect(c, b, gfx::alpha(Palette::ORANGE, 0.45), 2.0);
            }
            let fg = if active { Palette::BLACK } else { gfx::alpha(Palette::ORANGE, 0.5) };
            let w = gfx::text(c, b.x + 10.0, b.y + 37.0, kanji, &gfx::text_paint(&fonts.mincho(), 30.0, fg));
            gfx::text(c, b.x + 18.0 + w, b.y + 35.0, latin, &gfx::text_paint(&fonts.display(), 24.0, fg).with_letter_spacing(1.5));
            x += 196.0 + 10.0;
        }
        x += 18.0;
        for m in PowerMode::ALL {
            let b = Rect::new(x, row_y, 112.0, 50.0);
            let lit = mode == Some(m);
            if lit {
                gfx::fill_rect(c, b, Palette::RED);
            } else {
                gfx::stroke_rect(c, b, gfx::alpha(Palette::ORANGE, 0.6), 2.0);
            }
            let fg = if lit { Palette::BLACK } else { gfx::alpha(Palette::ORANGE, 0.7) };
            gfx::text_center(c, b, m.label(), &gfx::text_paint(&fonts.display(), 24.0, fg).with_letter_spacing(1.5));
            x += 112.0 + 8.0;
        }
    });

    if !base_layer {
        return;
    }
    let profile = power.as_ref().map_or("--".to_string(), |p| match &p.profile {
        crate::state::Profile::Unknown(s) if s.is_empty() => "NONE".into(),
        crate::state::Profile::Unknown(s) => s.to_uppercase(),
        other => format!("{other:?}").to_uppercase(),
    });
    let pp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.75)).with_text_align(Align::Right);
    gfx::text(ctx.canvas, r.right() - 40.0, r.y + 28.0, &format!("PROFILE {profile}"), &pp);
    battery_table(ctx, Rect::new(r.x + 40.0, r.y + 346.0, r.w - 100.0, r.h - 346.0 - 20.0));
}

/// Two columns of label/value pairs from `state.power` + `state.powerx`, and a charge bar.
fn battery_table(ctx: &mut Ctx, r: Rect) {
    let st = ctx.state;
    let bat = st.power.as_ref().and_then(|p| p.battery.as_ref());
    let x = st.powerx.as_ref();
    let pct = bat.map(|b| ctx.anim.roll("bat_pct", b.percent, ctx.t));
    let c = &mut *ctx.canvas;
    let fonts = ctx.fonts;
    let dash = || "--".to_string();
    let status = bat.map_or("NO BATTERY".to_string(), |b| match b.status {
        BatStatus::Charging => "CHARGING".into(),
        BatStatus::Discharging => "DISCHARGING".into(),
        BatStatus::Full => "FULL".into(),
        BatStatus::NotCharging => "NOT CHARGING".into(),
        BatStatus::Unknown => "UNKNOWN".into(),
    });
    let cells: [(&str, String); 8] = [
        ("CHARGE", pct.map_or_else(dash, |p| format!("{p:.1} %"))),
        ("POWER", bat.and_then(|b| b.watts).map_or_else(dash, |w| format!("{w:.1} W"))),
        ("VOLTAGE", x.and_then(|x| x.voltage_v).map_or_else(dash, |v| format!("{v:.2} V"))),
        ("CURRENT", x.and_then(|x| x.current_a).map_or_else(dash, |a| format!("{a:.2} A"))),
        ("HEALTH", x.and_then(|x| x.health_pct).map_or_else(dash, |h| format!("{h:.1} %"))),
        ("CYCLES", x.and_then(|x| x.cycles).map_or_else(dash, |n| n.to_string())),
        ("STATUS", status),
        ("MODEL", x.and_then(|x| x.model.clone()).unwrap_or_else(dash)),
    ];
    // charge bar across the top
    let bar = Rect::new(r.x, r.y, r.w, 18.0);
    let frac = pct.unwrap_or(0.0) / 100.0;
    let on = if frac < 0.15 { Palette::RED } else if frac < 0.3 { Palette::AMBER } else { Palette::LCD };
    gfx::seg_bar(c, bar, frac, 40, on, gfx::alpha(Palette::DIM, 0.35));

    let lp = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::ORANGE, 0.75)).with_letter_spacing(1.5);
    let vp = gfx::text_paint(&fonts.mono(), 26.0, Palette::WHITE);
    let col_w = r.w / 2.0;
    let row_h = ((r.h - 30.0) / 4.0).min(40.0);
    for (i, (label, value)) in cells.iter().enumerate() {
        let (col, rowi) = (i % 2, i / 2);
        let x0 = r.x + col as f32 * col_w;
        let y = r.y + 30.0 + row_h * rowi as f32 + row_h * 0.72;
        gfx::text(c, x0, y, label, &lp);
        let v = gfx::ellipsize(c, &vp, value, col_w - 150.0);
        gfx::text(c, x0 + 130.0, y, &v, &vp);
        if rowi < 3 {
            let ly = r.y + 30.0 + row_h * (rowi + 1) as f32;
            gfx::line(c, x0, ly, x0 + col_w - 24.0, ly, gfx::alpha(Palette::ORANGE, 0.12), 1.0);
        }
    }
}

/// `H:MM:SS.cc` of `seconds_left − elapsed` (elapsed = seconds since the power sample).
pub fn timer_text(seconds_left: Option<u64>, elapsed: f32) -> String {
    let Some(s) = seconds_left else { return "-:--:--.--".into() };
    let left = (s as f64 - elapsed.max(0.0) as f64).max(0.0);
    let cs = (left * 100.0 + 1e-6).floor() as u64;
    let (h, m, sec, c) = (cs / 360_000, cs / 6000 % 60, cs / 100 % 60, cs % 100);
    format!("{h}:{m:02}:{sec:02}.{c:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_text_formats() {
        assert_eq!(timer_text(Some(4 * 3600 + 59 * 60 + 56), 0.25), "4:59:55.75");
        assert_eq!(timer_text(Some(0), 0.0), "0:00:00.00");
        assert_eq!(timer_text(None, 0.0), "-:--:--.--");
    }
}
