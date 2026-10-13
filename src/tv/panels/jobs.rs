//! JOBS 作業: herdr panes running non-agent commands (dev servers, evals, tunnels).

use femtovg::Paint;

use super::plate::{self, Cuts};
use super::Ctx;
use crate::state::{Agent, PaneJob};
use crate::tv::gfx::{self, Palette};
use crate::tv::layout::Rect;

const HEAD_H: f32 = 40.0;
const ROW_MIN: f32 = 34.0;
const ROW_MAX: f32 = 100.0;
const LINE: f32 = 28.0;

pub fn draw(ctx: &mut Ctx, r: Rect) {
    if !ctx.layer.base() {
        return;
    }
    plate::plate(ctx.canvas, r, Cuts::BR, "JOBS", Some("作業"), Palette::ORANGE, ctx.fonts);
    let fonts = ctx.fonts;
    let jobs = order_jobs(&ctx.state.jobs, &ctx.state.herdr.agents);
    let c = &mut *ctx.canvas;
    let count = gfx::text_paint(&fonts.mono(), 24.0, gfx::alpha(Palette::ORANGE, 0.9)).with_text_align(femtovg::Align::Right);
    let s = format!("{} RUNNING PANES", jobs.len());
    let w = gfx::text_width(c, &s, &count);
    gfx::fill_rect(c, Rect::new(r.right() - 60.0 - w, r.y + 2.0, w + 24.0, 30.0), Palette::PANEL);
    gfx::text(c, r.right() - 48.0, r.y + 26.0, &s, &count);

    let t = Rect::new(r.x + 26.0, r.y + plate::PLATE_TOP + 4.0, r.w - 52.0, r.h - plate::PLATE_TOP - 30.0);
    let cols = col_widths(t.w);
    let xs = [t.x, t.x + cols[0], t.x + cols[0] + cols[1], t.x + cols[0] + cols[1] + cols[2]];
    let hp = gfx::text_paint(&fonts.label(), 22.0, Palette::ORANGE).with_letter_spacing(2.0);
    for (x, h) in xs.iter().zip(["WORKSPACE / TAB", "LABEL", "COMMAND", "CWD"]) {
        gfx::text(c, *x, t.y + 24.0, h, &hp);
    }
    gfx::line(c, t.x, t.y + HEAD_H - 6.0, t.right(), t.y + HEAD_H - 6.0, gfx::alpha(Palette::ORANGE, 0.5), 1.5);
    if jobs.is_empty() {
        let p = gfx::text_paint(&fonts.bold(), 40.0, Palette::DIM).with_letter_spacing(5.0);
        gfx::text_center(c, Rect::new(t.x, t.y + HEAD_H, t.w, t.h - HEAD_H), "NO ACTIVE JOBS", &p);
        return;
    }
    let avail = t.h - HEAD_H;
    let fit = super::procs::fit_rows(avail, ROW_MIN, 0.0).max(1);
    let shown = jobs.len().min(fit);
    let row_h = (avail / shown as f32).clamp(ROW_MIN, ROW_MAX);
    let lines = (((row_h - 8.0) / LINE).floor() as usize).max(1);
    let ws_p = gfx::text_paint(&fonts.label(), 26.0, Palette::CYAN);
    let tab_p = gfx::text_paint(&fonts.label(), 22.0, gfx::alpha(Palette::WHITE, 0.55));
    let lab_p = gfx::text_paint(&fonts.display(), 26.0, Palette::WHITE);
    let cmd_p = gfx::text_paint(&fonts.mono(), 24.0, Palette::LCD);
    let cwd_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::WHITE, 0.55));
    let id_p = gfx::text_paint(&fonts.mono(), 22.0, gfx::alpha(Palette::ORANGE, 0.6));
    let mut y = t.y + HEAD_H;
    for (i, j) in jobs.iter().take(shown).enumerate() {
        if i % 2 == 1 {
            gfx::fill_rect(c, Rect::new(t.x - 8.0, y, t.w + 16.0, row_h), gfx::alpha(Palette::WHITE, 0.03));
        }
        // running lamp
        gfx::fill_rect(c, Rect::new(t.x - 18.0, y + 12.0, 8.0, 14.0), Palette::GREEN);
        let base = y + 25.0;
        let cell = |k: usize| cols[k] - 18.0;
        gfx::text(c, xs[0], base, &gfx::ellipsize(c, &ws_p, &j.workspace_label, cell(0)), &ws_p);
        if lines >= 2 {
            gfx::text(c, xs[0], base + LINE, &gfx::ellipsize(c, &tab_p, &j.tab_label, cell(0)), &tab_p);
        }
        if lines >= 3 {
            gfx::text(c, xs[0], base + 2.0 * LINE, &gfx::ellipsize(c, &id_p, &j.pane_id, cell(0)), &id_p);
        }
        gfx::text(c, xs[1], base, &gfx::ellipsize(c, &lab_p, &j.label, cell(1)), &lab_p);
        if lines >= 2 {
            let rp = gfx::text_paint(&fonts.mono(), 22.0, Palette::GREEN);
            gfx::text(c, xs[1], base + LINE, "● RUNNING", &rp);
        }
        let cmd = wrap_text(&j.command, lines, |s| gfx::text_width(c, s, &cmd_p) <= cell(2));
        for (k, l) in cmd.iter().enumerate() {
            gfx::text(c, xs[2], base + k as f32 * LINE, l, &cmd_p);
        }
        let cwd = wrap_text(&j.cwd, lines, |s| gfx::text_width(c, s, &cwd_p) <= cell(3));
        for (k, l) in cwd.iter().enumerate() {
            gfx::text(c, xs[3], base + k as f32 * LINE, l, &cwd_p);
        }
        y += row_h;
        gfx::line(c, t.x, y, t.right(), y, gfx::alpha(Palette::ORANGE, 0.12), 1.0);
    }
    if jobs.len() > shown {
        let p = gfx::text_paint(&fonts.bold(), 24.0, Palette::ORANGE);
        gfx::text(c, t.x, t.bottom() + 20.0, &format!("+{} MORE", jobs.len() - shown), &p);
    }
    let _ = Paint::color(Palette::BLACK);
}

/// WORKSPACE/TAB · LABEL · COMMAND · CWD at 22 % / 18 % / 40 % / 20 %.
pub fn col_widths(total_w: f32) -> [f32; 4] {
    let a = total_w * 0.22;
    let b = total_w * 0.18;
    let c = total_w * 0.40;
    [a, b, c, total_w - a - b - c]
}

/// Greedy wrap at spaces and `/` (then after `_` `-` `.` inside a long token, then anywhere) into
/// at most `max_lines`; the last line is ellipsized with `…` when text remains. `fits` measures a
/// candidate line.
/// End of the longest prefix of `rest` that ends at a break character (after it; a space is
/// dropped) and fits.
fn break_at(rest: &str, fits: &impl Fn(&str) -> bool, is_break: impl Fn(char) -> bool) -> Option<usize> {
    let mut cut = None;
    for (i, ch) in rest.char_indices() {
        if !is_break(ch) {
            continue;
        }
        let end = if ch == ' ' { i } else { i + ch.len_utf8() };
        let cand = rest[..end].trim_end();
        if cand.is_empty() {
            continue;
        }
        if fits(cand) {
            cut = Some(end);
        } else {
            break;
        }
    }
    cut
}

/// Shortest piece of a token a soft break may leave for the next line.
const MIN_TAIL: usize = 4;

/// Longest fitting prefix of `rest` ending just after a `_` `-` `.`, skipping breaks that would
/// leave fewer than MIN_TAIL chars of the token (up to the next `/` or space) behind.
fn soft_break(rest: &str, fits: &impl Fn(&str) -> bool) -> Option<usize> {
    let token_end = rest.find(['/', ' ']).unwrap_or(rest.len());
    let mut best = None;
    for (i, ch) in rest[..token_end].char_indices() {
        if !matches!(ch, '_' | '-' | '.') {
            continue;
        }
        let end = i + 1;
        if !fits(&rest[..end]) {
            break;
        }
        if rest[end..token_end].chars().count() >= MIN_TAIL || best.is_none() {
            best = Some(end);
        }
    }
    best
}

pub fn wrap_text(text: &str, max_lines: usize, fits: impl Fn(&str) -> bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() && lines.len() < max_lines {
        if fits(rest) {
            lines.push(rest.to_string());
            break;
        }
        if lines.len() + 1 == max_lines {
            lines.push(ellipsize_by(rest, &fits));
            break;
        }
        // longest prefix ending at a space or just after a '/' that fits; failing that, just after
        // a '_' '-' '.' (so a long path token breaks between words, not mid-word)
        let cut = break_at(rest, &fits, |ch| matches!(ch, '/' | ' ')).or_else(|| soft_break(rest, &fits));
        // no break point fits: hard split at the longest fitting char prefix (≥ 1 char)
        let end = cut.unwrap_or_else(|| {
            let mut end = rest.char_indices().nth(1).map_or(rest.len(), |(i, _)| i);
            for (i, _) in rest.char_indices().skip(1) {
                if fits(&rest[..i]) {
                    end = i;
                } else {
                    break;
                }
            }
            end
        });
        lines.push(rest[..end].trim_end().to_string());
        rest = rest[end..].trim_start();
    }
    lines
}

fn ellipsize_by(s: &str, fits: &impl Fn(&str) -> bool) -> String {
    let mut best = String::from("…");
    for (i, _) in s.char_indices().skip(1) {
        let cand = format!("{}…", s[..i].trim_end());
        if fits(&cand) {
            best = cand;
        } else {
            break;
        }
    }
    best
}

/// Jobs whose workspace has the most recently changed agent first; jobs in agent-less
/// workspaces last; stable otherwise.
pub fn order_jobs<'a>(jobs: &'a [PaneJob], agents: &[Agent]) -> Vec<&'a PaneJob> {
    let newest = |ws: &str| agents.iter().filter(|a| a.workspace_label == ws).map(|a| a.since).max();
    let mut v: Vec<&PaneJob> = jobs.iter().collect();
    // None sorts first in ascending order, so compare reversed: newest Some first, None last
    v.sort_by(|a, b| newest(&b.workspace_label).cmp(&newest(&a.workspace_label)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::{agent, t0};
    use crate::state::AgentStatus;
    use std::time::Duration;

    fn job(ws: &str, label: &str) -> PaneJob {
        PaneJob { pane_id: format!("{ws}:{label}"), workspace_label: ws.into(), tab_label: "t".into(), label: label.into(), command: "cmd".into(), cwd: "~".into() }
    }

    #[test]
    fn wrap_breaks_at_spaces_and_slashes_and_ellipsizes_the_last_line() {
        let fits = |s: &str| s.chars().count() <= 12;
        assert_eq!(wrap_text("uv run --no-sync python -m test", 2, fits), ["uv run", "--no-sync p…"]);
        assert_eq!(wrap_text("~/Desktop/store_fe", 3, fits), ["~/Desktop/", "store_fe"]);
        assert_eq!(wrap_text("short", 3, fits), ["short"]);
        assert_eq!(wrap_text("", 3, fits), Vec::<String>::new());
        // a single unbreakable token longer than a line is ellipsized, never dropped
        assert_eq!(wrap_text("abcdefghijklmnopq", 1, fits), ["abcdefghijk…"]);
    }

    #[test]
    fn long_path_tokens_break_after_underscore_dash_or_dot_before_a_hard_split() {
        let fits = |s: &str| s.chars().count() <= 16;
        assert_eq!(wrap_text("~/Desktop/metro_storefront_be/x", 3, fits), ["~/Desktop/", "metro_", "storefront_be/x"]);
        assert_eq!(wrap_text("feature-docs-1013-long", 2, fits), ["feature-docs-", "1013-long"]);
        // a soft break never leaves a 1-3 char orphan of its token for the next line
        let fits17 = |s: &str| s.chars().count() <= 17;
        assert_eq!(wrap_text("~/Desktop/metro_storefront_be", 3, fits17), ["~/Desktop/", "metro_", "storefront_be"]);
        // no separator at all: still a hard split
        assert_eq!(wrap_text("abcdefghijklmnopqrstu", 2, fits), ["abcdefghijklmnop", "qrstu"]);
    }

    #[test]
    fn col_widths_sum_to_total() {
        for w in [1258.0, 1316.0, 640.0] {
            let c = col_widths(w);
            assert!((c.iter().sum::<f32>() - w).abs() < 0.01);
            assert!((c[2] - w * 0.40).abs() < 0.01);
        }
    }

    #[test]
    fn newest_agent_workspace_first() {
        let mut old = agent("w1:p1", "a", AgentStatus::Working, "search");
        old.since = t0();
        let mut new = agent("w2:p1", "b", AgentStatus::Working, "web");
        new.since = t0() + Duration::from_secs(60);
        let jobs = vec![job("search", "eval"), job("lonely", "tunnel"), job("web", "api")];
        let order: Vec<&str> = order_jobs(&jobs, &[old, new]).iter().map(|j| j.label.as_str()).collect();
        assert_eq!(order, ["api", "eval", "tunnel"]);
    }
}
