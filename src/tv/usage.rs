//! Agent usage on the TV (plan REVISION 4): sync ratio colours, token and reset formatting, the
//! compact plan-limits line, and the SYNC CRITICAL rule.

use std::time::{Duration, SystemTime};

use femtovg::Color;

use crate::state::PlanLimits;
use crate::tv::gfx::Palette;

/// Context ≥ this % flags the agent `SYNC CRITICAL`.
pub const CRITICAL_CTX_PCT: f32 = 85.0;

/// Sync ratio (free context %) → green > 50, amber 20–50, red < 20, dim when unknown.
pub fn sync_color(ratio: Option<f32>) -> Color {
    match ratio {
        None => Palette::DIM,
        Some(r) if r > 50.0 => Palette::GREEN,
        Some(r) if r >= 20.0 => Palette::AMBER,
        Some(_) => Palette::RED,
    }
}

pub fn sync_critical(context_pct: Option<f32>) -> bool {
    context_pct.is_some_and(|p| p >= CRITICAL_CTX_PCT)
}

/// "950", "48K", "1.2M", "3.4B"
pub fn fmt_tokens(n: u64) -> String {
    let f = n as f64;
    if f >= 1e9 {
        format!("{:.1}B", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1}M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.0}K", f / 1e3)
    } else {
        n.to_string()
    }
}

/// Time until a reset: "45m", "2h14m", "4d03h"; "now" when past.
pub fn fmt_reset(left: Duration) -> String {
    let s = left.as_secs();
    if s == 0 {
        "now".into()
    } else if s >= 86_400 {
        format!("{}d{:02}h", s / 86_400, s / 3600 % 24)
    } else if s >= 3600 {
        format!("{}h{:02}m", s / 3600, s / 60 % 60)
    } else {
        format!("{}m", (s / 60).max(1))
    }
}

/// "CLAUDE 5H 37% (2h14m) · 7D 12% (4d00h) │ CODEX WEEK 17% (5d01h)"
pub fn limits_line(limits: &[PlanLimits], now: SystemTime) -> String {
    limits
        .iter()
        .filter(|l| !l.windows.is_empty())
        .map(|l| {
            let windows: Vec<String> = l
                .windows
                .iter()
                .map(|w| {
                    let reset =
                        w.resets_at.map(|r| format!(" ({})", fmt_reset(r.duration_since(now).unwrap_or_default())));
                    format!("{} {:.0}%{}", w.label, w.used_pct, reset.unwrap_or_default())
                })
                .collect();
            format!("{} {}", l.provider.to_uppercase(), windows.join(" · "))
        })
        .collect::<Vec<_>>()
        .join(" │ ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LimitWindow;

    #[test]
    fn sync_colors_and_critical() {
        assert_eq!(sync_color(Some(80.0)), Palette::GREEN);
        assert_eq!(sync_color(Some(50.0)), Palette::AMBER);
        assert_eq!(sync_color(Some(20.0)), Palette::AMBER);
        assert_eq!(sync_color(Some(19.9)), Palette::RED);
        assert_eq!(sync_color(None), Palette::DIM);
        assert!(sync_critical(Some(85.0)));
        assert!(!sync_critical(Some(84.9)));
        assert!(!sync_critical(None));
    }

    #[test]
    fn token_and_reset_formats() {
        assert_eq!(fmt_tokens(950), "950");
        assert_eq!(fmt_tokens(48_200), "48K");
        assert_eq!(fmt_tokens(1_234_567), "1.2M");
        assert_eq!(fmt_tokens(3_400_000_000), "3.4B");
        assert_eq!(fmt_reset(Duration::from_secs(45 * 60)), "45m");
        assert_eq!(fmt_reset(Duration::from_secs(2 * 3600 + 14 * 60)), "2h14m");
        assert_eq!(fmt_reset(Duration::from_secs(4 * 86400 + 3 * 3600)), "4d03h");
        assert_eq!(fmt_reset(Duration::ZERO), "now");
    }

    #[test]
    fn limits_line_lists_providers_and_countdowns() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let w = |label: &str, pct: f32, secs: u64| LimitWindow {
            label: label.into(),
            used_pct: pct,
            resets_at: Some(now + Duration::from_secs(secs)),
            window_minutes: None,
        };
        let limits = vec![
            PlanLimits {
                provider: "claude".into(),
                windows: vec![w("5H", 37.4, 2 * 3600 + 14 * 60), w("7D", 12.0, 4 * 86400)],
                ..Default::default()
            },
            PlanLimits {
                provider: "codex".into(),
                windows: vec![LimitWindow { resets_at: None, ..w("WEEK", 17.0, 0) }],
                ..Default::default()
            },
        ];
        assert_eq!(limits_line(&limits, now), "CLAUDE 5H 37% (2h14m) · 7D 12% (4d00h) │ CODEX WEEK 17%");
        assert_eq!(limits_line(&[], now), "");
    }
}
