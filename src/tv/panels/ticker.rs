//! TICKER 通信: status transitions scrolling left on an angled band.

use femtovg::{Paint, Path};

use super::{status_color, Ctx};
use crate::state::{AgentStatus, State};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

/// Scroll speed in design px per second.
pub const SPEED: f32 = 120.0;
const SLOPE: f32 = 50.0;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    let fonts = ctx.fonts;
    let (base_layer, live_layer) = (ctx.layer.base(), ctx.layer.live());
    let c = &mut *ctx.canvas;
    let label = Rect::new(r.x + 24.0, r.y + 14.0, 300.0, r.h - 28.0);
    let lane = Rect::new(label.right() + 60.0, r.y + 6.0, r.right() - 170.0 - label.right() - 60.0, r.h - 12.0);
    let base = r.y + r.h / 2.0 + 13.0;
    let items = ticker_items(ctx.state);
    if !base_layer {
        if !items.is_empty() {
            lane_items(ctx, lane, base, &items);
        }
        return;
    }
    // band with angled ends
    let mut band = Path::new();
    band.move_to(r.x + SLOPE, r.y);
    band.line_to(r.right() - 24.0, r.y);
    band.line_to(r.right() - 24.0 - SLOPE, r.bottom());
    band.line_to(r.x + 24.0, r.bottom());
    band.line_to(r.x + 24.0, r.y + SLOPE);
    band.close();
    c.fill_path(&band, &Paint::color(Palette::PANEL));
    c.stroke_path(&band, &Paint::color(Palette::ORANGE).with_line_width(3.0));

    // fixed label block
    let mut tag = Path::new();
    tag.move_to(label.x + 30.0, label.y);
    tag.line_to(label.right() + 30.0, label.y);
    tag.line_to(label.right(), label.bottom());
    tag.line_to(label.x, label.bottom());
    tag.line_to(label.x, label.y + 30.0);
    tag.close();
    c.fill_path(&tag, &Paint::color(Palette::ORANGE));
    let lp = gfx::text_paint(&fonts.bold(), 36.0, Palette::BLACK).with_letter_spacing(3.0);
    let w = gfx::text(c, label.x + 30.0, label.y + 52.0, "◀◀", &lp);
    gfx::text(c, label.x + 44.0 + w, label.y + 52.0, "通信 LOG", &lp);

    if items.is_empty() {
        let p = gfx::text_paint(&fonts.display(), 32.0, gfx::alpha(Palette::WHITE, 0.5)).with_letter_spacing(3.0);
        let s = format!(
            "NO TRANSITIONS YET · MONITORING {} PILOTS IN {} SECTORS",
            ctx.state.herdr.agents.len(),
            ctx.state.herdr.workspaces.len()
        );
        gfx::text(c, lane.x, base, &s, &p);
        return;
    }
    // right-end count
    let cp =
        gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.8)).with_text_align(femtovg::Align::Right);
    gfx::text(c, r.right() - 90.0, base, &format!("{}", ctx.state.transitions), &cp);
    if live_layer {
        lane_items(ctx, lane, base, &items);
    }
}

/// The scrolling part: one pass of all items, repeated to fill the lane while scrolling left.
fn lane_items(ctx: &mut Ctx, lane: Rect, base: f32, items: &[(String, String, AgentStatus)]) {
    let fonts = ctx.fonts;
    let c = &mut *ctx.canvas;
    let tp = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::ORANGE, 0.85));
    let np = gfx::text_paint(&fonts.display(), 34.0, Palette::WHITE).with_letter_spacing(1.0);
    let sep = gfx::text_paint(&fonts.mono(), 30.0, gfx::alpha(Palette::ORANGE, 0.5));
    // items only change with a new transition: measure them once per change, not every frame
    let key = (ctx.state.transitions, items.len(), ctx.sprites.scale.to_bits());
    let widths = ctx
        .anim
        .ticker_widths
        .get(key, || {
            items
                .iter()
                .map(|(agent, hhmm, st)| {
                    let sp = gfx::text_paint(&fonts.bold(), 34.0, status_color(*st)).with_letter_spacing(2.0);
                    gfx::text_width(c, hhmm, &tp)
                        + 16.0
                        + gfx::text_width(c, agent, &np)
                        + 16.0
                        + gfx::text_width(c, "→", &tp)
                        + 16.0
                        + gfx::text_width(c, st.label(), &sp)
                        + gfx::text_width(c, "  ··  ", &sep)
                })
                .collect()
        })
        .clone();
    // a short list scrolls through once per lane width instead of tiling itself
    let cycle: f32 = widths.iter().sum::<f32>().max(lane.w);
    let offset = (ctx.t * SPEED) % cycle;
    c.save();
    c.intersect_scissor(lane.x, lane.y, lane.w, lane.h);
    let mut start = lane.x - offset;
    while start < lane.right() {
        let mut x = start;
        for ((agent, hhmm, st), w) in items.iter().zip(&widths) {
            if x + w > lane.x && x < lane.right() {
                let sp = gfx::text_paint(&fonts.bold(), 34.0, status_color(*st)).with_letter_spacing(2.0);
                let mut cx = x;
                cx += gfx::text(c, cx, base, hhmm, &tp) + 16.0;
                cx += gfx::text(c, cx, base, agent, &np) + 16.0;
                cx += gfx::text(c, cx, base, "→", &tp) + 16.0;
                cx += gfx::text(c, cx, base, st.label(), &sp);
                gfx::text(c, cx, base, "  ··  ", &sep);
            }
            x += w;
        }
        start += cycle;
    }
    c.restore();
}

/// (agent, "HH:MM", status) newest first.
pub fn ticker_items(s: &State) -> Vec<(String, String, AgentStatus)> {
    s.ticker
        .iter()
        .map(|e| {
            let (.., h, m, _) = super::header::local_time(e.at);
            (e.agent.clone(), format!("{h:02}:{m:02}"), e.to)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_text_lists_newest_first() {
        use crate::state::{tests::t0, *};
        let mut s = State::new(t0());
        s.ticker.push_front(TickerEvent {
            at: t0(),
            agent: "a".into(),
            from: AgentStatus::Working,
            to: AgentStatus::Done,
        });
        s.ticker.push_front(TickerEvent {
            at: t0(),
            agent: "b".into(),
            from: AgentStatus::Working,
            to: AgentStatus::Blocked,
        });
        let items = ticker_items(&s);
        assert_eq!(items[0].0, "b");
        assert_eq!(items[1].0, "a");
    }
}
