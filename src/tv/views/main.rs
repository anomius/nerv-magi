//! MAIN: the operations board (plan REVISION 2 anchors).

use crate::tv::gfx::Palette;
use crate::tv::layout::{Layout, Rect, DESIGN_W};
use crate::tv::panels::{alert, header, jobs, magi, plate, procs, sdat, sectors, telemetry, ticker, timer, units, Ctx};

/// Time a panel under `--bench` (separately for the base and the live layer).
macro_rules! timed {
    ($ctx:expr, $name:literal, $e:expr) => {
        if $ctx.layer == crate::tv::layers::Layer::Live {
            crate::tv::bench::span(concat!("live ", $name), || $e)
        } else {
            crate::tv::bench::span(concat!("base ", $name), || $e)
        }
    };
}

pub fn draw(ctx: &mut Ctx, layout: &Layout) {
    timed!(ctx, "header", header::draw(ctx, layout.header));
    timed!(ctx, "strip", alert::status_strip(ctx, layout.alert));
    timed!(ctx, "units", units::draw(ctx, layout.units, layout.band));
    timed!(ctx, "magi", magi::draw(ctx, layout.magi));
    timed!(ctx, "timer", timer::draw(ctx, layout.timer));
    timed!(ctx, "sectors", sectors::draw(ctx, layout.sectors));
    timed!(ctx, "telemetry", telemetry::draw(ctx, layout.telemetry));
    timed!(ctx, "sdat", sdat::draw(ctx, layout.sdat));
    timed!(ctx, "jobs", jobs::draw(ctx, layout.jobs));
    timed!(ctx, "procs", procs::draw(ctx, layout.procs));
    timed!(ctx, "ticker", ticker::draw(ctx, layout.ticker));
    timed!(ctx, "traces", magi::traces(ctx, layout));
    if ctx.layer.base() {
        frame(ctx, layout);
    }
}

/// Structural edges of the board (plan REVISION 2): tick rulers down the left margin and along
/// the bottom margin, yellow/black hazard strips on the right margin and in the gap between the
/// timer and S-DAT plates. All in margins and gaps, never over a panel.
fn frame(ctx: &mut Ctx, l: &Layout) {
    let c = &mut *ctx.canvas;
    let top = l.units.y;
    let bottom = l.jobs.bottom();
    plate::ruler_ticks(c, 2.0, top, bottom - top, true, 16.0, Palette::ORANGE);
    plate::ruler_ticks(c, l.ticker.x + 24.0, l.ticker.bottom() + 20.0, DESIGN_W - 48.0, false, 14.0, Palette::ORANGE);
    plate::structural_hazard(c, Rect::new(l.procs.right() + 6.0, top, 12.0, bottom - top));
    let gap = Rect::new(l.timer.x + 40.0, l.timer.bottom() + 5.0, l.timer.w - 80.0, l.sdat.y - l.timer.bottom() - 10.0);
    plate::structural_hazard(c, gap);
}
