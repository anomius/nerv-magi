//! Window-side control state (plan REVISION 5/6): winit keys → `control::key_command` names, the
//! pointer latch, the help overlay timer and the toast texts for view / grain changes.

use winit::keyboard::{Key, NamedKey};

use crate::control::{TvCommand, ViewMode};

/// How long the help overlay stays up without a second press.
pub const HELP_S: f32 = 10.0;

/// winit's logical key as the text `control::key_command` expects: named keys by their winit
/// name ("Tab", "ArrowRight", "F11"; Space as " "), characters as typed with ASCII letters
/// lower-cased (so caps lock or shift does not swallow `f`, `g`, `s` …).
pub fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Character(c) => Some(c.to_ascii_lowercase()),
        Key::Named(n) => Some(
            match n {
                NamedKey::Tab => "Tab",
                NamedKey::ArrowRight => "ArrowRight",
                NamedKey::ArrowLeft => "ArrowLeft",
                NamedKey::Escape => "Escape",
                NamedKey::F11 => "F11",
                NamedKey::Space => " ",
                _ => return None,
            }
            .to_string(),
        ),
        _ => None,
    }
}

/// Toggles must not flip again on key auto-repeat (holding `f` would flicker fullscreen and
/// reallocate the base layer each time); stepping (Next/Prev) and idempotent picks may repeat.
pub fn repeat_ignored(cmd: TvCommand) -> bool {
    matches!(cmd, TvCommand::TogglePin | TvCommand::ToggleFullscreen | TvCommand::Scanlines | TvCommand::ToggleGrain | TvCommand::ToggleHelp)
}

/// Seconds after the window appears or regains focus during which key presses are dropped.
pub const KEY_GRACE_S: f32 = 1.0;

/// A new or refocused window gets the keystrokes someone was typing into the window that had
/// focus a moment ago (on Wayland they arrive as ordinary presses, not synthetic ones); `1`–`4`
/// would pin a view and `q` / Escape would quit. The gate drops presses for KEY_GRACE_S after
/// each focus gain.
#[derive(Debug, Default)]
pub struct KeyGate {
    focused_at: Option<f32>,
}

impl KeyGate {
    pub fn focused(&mut self, t: f32) {
        self.focused_at = Some(t);
    }

    pub fn allows(&self, t: f32) -> bool {
        self.focused_at.is_none_or(|f| t - f >= KEY_GRACE_S)
    }
}

/// Remembers what the pointer was last set to, so winit is only called when it changes.
#[derive(Debug, Default)]
pub struct CursorLatch(Option<bool>);

impl CursorLatch {
    /// `Some(visible)` when the window must be told, `None` when nothing changed.
    pub fn update(&mut self, visible: bool) -> Option<bool> {
        if self.0 == Some(visible) {
            return None;
        }
        self.0 = Some(visible);
        Some(visible)
    }
}

/// The `?` overlay: a press shows it for HELP_S, a second press while shown hides it.
#[derive(Debug, Default)]
pub struct Help {
    until: Option<f32>,
}

impl Help {
    pub fn toggle(&mut self, t: f32) {
        self.until = if self.visible(t) { None } else { Some(t + HELP_S) };
    }

    pub fn visible(&self, t: f32) -> bool {
        self.until.is_some_and(|u| t < u)
    }
}

pub fn view_toast(mode: ViewMode) -> String {
    match mode {
        ViewMode::Auto => "VIEW · AUTO".into(),
        ViewMode::Pinned(v) => format!("VIEW · {} (PINNED)", v.label()),
    }
}

pub fn grain_toast(on: bool) -> String {
    format!("GRAIN · {}", if on { "ON" } else { "OFF" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{key_command, ViewName};

    #[test]
    fn keys_map_to_control_names() {
        assert_eq!(key_name(&Key::Named(NamedKey::Tab)).as_deref(), Some("Tab"));
        assert_eq!(key_name(&Key::Named(NamedKey::ArrowRight)).as_deref(), Some("ArrowRight"));
        assert_eq!(key_name(&Key::Named(NamedKey::ArrowLeft)).as_deref(), Some("ArrowLeft"));
        assert_eq!(key_name(&Key::Named(NamedKey::Escape)).as_deref(), Some("Escape"));
        assert_eq!(key_name(&Key::Named(NamedKey::F11)).as_deref(), Some("F11"));
        assert_eq!(key_name(&Key::Named(NamedKey::Space)).as_deref(), Some(" "));
        assert_eq!(key_name(&Key::Character("2".into())).as_deref(), Some("2"));
        assert_eq!(key_name(&Key::Character("?".into())).as_deref(), Some("?"));
        // caps lock / shift on letters still reach the command
        assert_eq!(key_name(&Key::Character("F".into())).as_deref(), Some("f"));
        assert_eq!(key_name(&Key::Named(NamedKey::Shift)), None);
        assert_eq!(key_command(&key_name(&Key::Named(NamedKey::Space)).unwrap(), false), Some(TvCommand::TogglePin));
        assert_eq!(key_command(&key_name(&Key::Character("G".into())).unwrap(), true), Some(TvCommand::ToggleGrain));
        assert_eq!(key_command(&key_name(&Key::Character("4".into())).unwrap(), false), Some(TvCommand::View(ViewName::Emergency)));
    }

    #[test]
    fn keys_are_dropped_for_a_grace_period_after_focus() {
        let mut g = KeyGate::default();
        g.focused(10.0);
        assert!(!g.allows(10.0));
        assert!(!g.allows(10.0 + KEY_GRACE_S - 0.01));
        assert!(g.allows(10.0 + KEY_GRACE_S));
        g.focused(20.0); // focus came back: typing meant for the other window is dropped again
        assert!(!g.allows(20.5));
        assert!(g.allows(21.5));
    }

    #[test]
    fn auto_repeat_never_flips_a_toggle() {
        for cmd in [TvCommand::TogglePin, TvCommand::ToggleFullscreen, TvCommand::Scanlines, TvCommand::ToggleGrain, TvCommand::ToggleHelp] {
            assert!(repeat_ignored(cmd), "{cmd:?}");
        }
        // stepping and idempotent picks may repeat
        for cmd in [TvCommand::Next, TvCommand::Prev, TvCommand::View(ViewName::Load), TvCommand::Auto] {
            assert!(!repeat_ignored(cmd), "{cmd:?}");
        }
    }

    #[test]
    fn cursor_latch_reports_only_changes() {
        let mut l = CursorLatch::default();
        assert_eq!(l.update(true), Some(true)); // first frame always applies
        assert_eq!(l.update(true), None);
        assert_eq!(l.update(false), Some(false));
        assert_eq!(l.update(false), None);
        assert_eq!(l.update(true), Some(true));
    }

    #[test]
    fn help_toggles_and_hides_after_ten_seconds() {
        let mut h = Help::default();
        assert!(!h.visible(0.0));
        h.toggle(5.0);
        assert!(h.visible(5.0) && h.visible(14.9));
        assert!(!h.visible(15.1));
        h.toggle(20.0);
        h.toggle(21.0); // second press hides it
        assert!(!h.visible(21.0));
        h.toggle(30.0); // after auto-hide, a press shows it again
        assert!(h.visible(30.5));
    }

    #[test]
    fn toast_texts() {
        assert_eq!(view_toast(ViewMode::Pinned(ViewName::Load)), "VIEW · LOAD (PINNED)");
        assert_eq!(view_toast(ViewMode::Auto), "VIEW · AUTO");
        assert_eq!(grain_toast(true), "GRAIN · ON");
        assert_eq!(grain_toast(false), "GRAIN · OFF");
    }
}
