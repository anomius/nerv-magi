//! Which full-screen view the TV shows (plan REVISION 2): EMERGENCY takes over, BOOT on launch and
//! while herdr is unreachable, otherwise MAIN ⇄ LOAD rotation. Pure and tested; times are seconds
//! since the view started.

use crate::config::Config;
use crate::control::{ViewMode, ViewName};
pub use crate::state::EmergencyCause;
use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Boot { reconnecting: bool },
    Main,
    Load,
    Sync,
    Emergency(EmergencyCause),
}

impl Scene {
    pub fn name(self) -> &'static str {
        match self {
            Scene::Boot { .. } => "BOOT",
            Scene::Main => "MAIN",
            Scene::Load => "LOAD",
            Scene::Sync => "SYNC",
            Scene::Emergency(_) => "EMERGENCY",
        }
    }

    /// The pickable view this scene is (BOOT counts as MAIN), for Next / Prev / pin.
    pub fn view_name(self) -> ViewName {
        match self {
            Scene::Boot { .. } | Scene::Main => ViewName::Main,
            Scene::Load => ViewName::Load,
            Scene::Sync => ViewName::Sync,
            Scene::Emergency(_) => ViewName::Emergency,
        }
    }

    fn rotating(self) -> bool {
        matches!(self, Scene::Main | Scene::Load | Scene::Sync)
    }

    /// Same view (an EMERGENCY whose cause changes is not a new cut).
    fn same_view(self, other: Scene) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// Length of the hard cut into a new scene.
pub const CUT_S: f32 = 0.35;

/// Seconds herdr must be unreachable before the reconnecting BOOT view replaces MAIN.
pub const LOST_GRACE_S: f32 = 3.0;

pub struct Director {
    started: f32,
    current: Scene,
    entered_at: f32,
    /// When the MAIN slot of the current rotation began.
    rot_start: f32,
    /// The emergency being shown (kept through the hold) and when its cause cleared.
    cause: Option<EmergencyCause>,
    cleared_at: Option<f32>,
    lost_since: Option<f32>,
    /// Rotation, or a view pinned by the viewer (keys, tv-ctl, `--scene`).
    mode: ViewMode,
    /// The launch BOOT is over (time ran out, or the viewer picked a view).
    booted: bool,
    /// Back to Auto: the next rotation frame starts a fresh MAIN slot.
    restart_rotation: bool,
    /// Pinned EMERGENCY with no real emergency: the view is a preview.
    preview: bool,
    /// `--scene boot`: always show this scene (debugging; BOOT cannot be pinned).
    pub forced: Option<Scene>,
}

impl Director {
    pub fn new(t0: f32) -> Self {
        Self {
            started: t0,
            current: Scene::Boot { reconnecting: false },
            entered_at: t0,
            rot_start: t0,
            cause: None,
            cleared_at: None,
            lost_since: None,
            mode: ViewMode::Auto,
            booted: false,
            restart_rotation: false,
            preview: false,
            forced: None,
        }
    }

    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    /// Switch between rotation and a pinned view. Pinning ends the launch BOOT; going back to
    /// Auto restarts the rotation at MAIN.
    pub fn set_mode(&mut self, mode: ViewMode) {
        if mode == self.mode {
            return;
        }
        match mode {
            ViewMode::Pinned(_) => self.booted = true,
            ViewMode::Auto => self.restart_rotation = true,
        }
        self.mode = mode;
    }

    /// The EMERGENCY on screen is a pinned preview, not a real one.
    pub fn preview(&self) -> bool {
        self.preview
    }

    pub fn current(&self) -> Scene {
        self.current
    }

    pub fn entered_at(&self) -> f32 {
        self.entered_at
    }

    /// When herdr was first seen unreachable (view time), while it still is.
    pub fn lost_since(&self) -> Option<f32> {
        self.lost_since
    }

    /// Decide the scene for this frame. Priority: Emergency > Boot(reconnecting) > Boot(launch) > rotation.
    pub fn scene(&mut self, state: &State, cfg: &Config, t: f32) -> Scene {
        let tv = &cfg.tv;
        // emergency with hold
        // the shared takeover rule (state.rs); a ≤ 1/3 MAGI tally is styling only, not a takeover
        match state.emergency(&cfg.thresholds) {
            Some(c) => {
                self.cause = Some(c);
                self.cleared_at = None;
            }
            None => {
                if self.cause.is_some() && self.cleared_at.is_none() {
                    self.cleared_at = Some(t);
                }
                if let Some(at) = self.cleared_at {
                    if t - at >= tv.emergency_hold_secs as f32 {
                        self.cause = None;
                        self.cleared_at = None;
                    }
                }
            }
        }
        // herdr reachability
        if matches!(state.herdr.link, crate::state::HerdrLink::Lost { .. }) {
            self.lost_since.get_or_insert(t);
        } else {
            self.lost_since = None;
        }

        if t - self.started >= tv.boot_secs as f32 {
            self.booted = true;
        }
        let takeover = match self.mode {
            ViewMode::Auto => true,
            ViewMode::Pinned(_) => tv.emergency_overrides_pin,
        };
        self.preview = false;
        let next = if let Some(f) = self.forced {
            f
        } else if let Some(c) = self.cause.filter(|_| takeover) {
            Scene::Emergency(c)
        } else if self.lost_since.is_some_and(|s| t - s > LOST_GRACE_S) {
            Scene::Boot { reconnecting: true }
        } else if let ViewMode::Pinned(v) = self.mode {
            match v {
                ViewName::Main => Scene::Main,
                ViewName::Load => Scene::Load,
                ViewName::Sync => Scene::Sync,
                ViewName::Emergency => match self.cause {
                    Some(c) => Scene::Emergency(c),
                    None => {
                        self.preview = true;
                        Scene::Emergency(EmergencyCause::Blocked)
                    }
                },
            }
        } else if !self.booted {
            Scene::Boot { reconnecting: false }
        } else {
            if self.restart_rotation || !self.current.rotating() {
                // leaving BOOT, EMERGENCY or a pin: a fresh MAIN slot, timed from when the view ended
                self.rot_start = match self.current {
                    Scene::Boot { reconnecting: false } if !self.restart_rotation => (self.started + tv.boot_secs as f32).min(t),
                    _ => t,
                };
                self.restart_rotation = false;
            }
            self.rotation(cfg, t).0
        };
        if !next.same_view(self.current) {
            self.entered_at = t;
        }
        self.current = next;
        next
    }

    /// (scene, seconds until the next rotation step) at `t` within the current rotation.
    fn rotation(&self, cfg: &Config, t: f32) -> (Scene, f32) {
        let tv = &cfg.tv;
        let main = tv.main_secs.max(1) as f32;
        if !tv.rotate {
            return (Scene::Main, f32::INFINITY);
        }
        let load = tv.load_secs as f32;
        let sync = tv.sync_secs as f32;
        let cycle = main + load + sync;
        let phase = (t - self.rot_start).max(0.0) % cycle;
        if phase < main {
            (Scene::Main, main - phase)
        } else if phase < main + load {
            (Scene::Load, main + load - phase)
        } else {
            (Scene::Sync, cycle - phase)
        }
    }

    /// 0..1 progress of the cut transition into the current scene.
    pub fn transition(&self, t: f32) -> f32 {
        ((t - self.entered_at) / CUT_S).clamp(0.0, 1.0)
    }

    /// While rotating: the next scene and the seconds until it.
    pub fn countdown(&self, cfg: &Config, t: f32) -> Option<(Scene, f32)> {
        if !self.current.rotating() || !cfg.tv.rotate || self.forced.is_some() || self.mode != ViewMode::Auto {
            return None;
        }
        let (now, left) = self.rotation(cfg, t);
        let tv = &cfg.tv;
        let next = match now {
            Scene::Main if tv.load_secs > 0 => Scene::Load,
            Scene::Main | Scene::Load if tv.sync_secs > 0 => Scene::Sync,
            Scene::Main => Scene::Main,
            _ => Scene::Main,
        };
        Some((next, left))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{ViewMode, ViewName};
    use crate::state::tests::{agent, t0};
    use crate::state::{AgentStatus, HerdrLink, SysState, Update};

    fn cfg() -> Config {
        Config::default() // main 45 s, load 15 s, boot 4 s, hold 5 s, mem critical 800 MB
    }

    fn state(blocked: bool) -> State {
        let mut s = State::new(t0());
        s.apply(Update::HerdrLink(HerdrLink::Linked), t0());
        let status = if blocked { AgentStatus::Blocked } else { AgentStatus::Working };
        s.apply(Update::HerdrSnapshot { agents: vec![agent("w1:p1", "a", status, "w1")], workspaces: vec![] }, t0());
        s.apply(Update::Sys(SysState { mem_total: 15_000_000_000, mem_available: 4_000_000_000, ..Default::default() }), t0());
        s
    }

    #[test]
    fn launch_shows_boot_then_main() {
        let mut d = Director::new(0.0);
        let s = state(false);
        assert_eq!(d.scene(&s, &cfg(), 0.0), Scene::Boot { reconnecting: false });
        assert_eq!(d.scene(&s, &cfg(), 3.9), Scene::Boot { reconnecting: false });
        assert_eq!(d.scene(&s, &cfg(), 4.1), Scene::Main);
    }

    #[test]
    fn rotates_main_then_load_then_sync_then_main() {
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &cfg(), 0.0);
        // rotation starts when boot ends (t = 4): MAIN 4..49, LOAD 49..64, SYNC 64..79, MAIN again
        assert_eq!(d.scene(&s, &cfg(), 5.0), Scene::Main);
        assert_eq!(d.scene(&s, &cfg(), 48.9), Scene::Main);
        assert_eq!(d.scene(&s, &cfg(), 49.1), Scene::Load);
        assert_eq!(d.scene(&s, &cfg(), 63.9), Scene::Load);
        assert_eq!(d.scene(&s, &cfg(), 64.1), Scene::Sync);
        assert_eq!(d.scene(&s, &cfg(), 78.9), Scene::Sync);
        assert_eq!(d.scene(&s, &cfg(), 79.1), Scene::Main);
    }

    #[test]
    fn sync_secs_zero_drops_the_sync_slot() {
        let mut c = cfg();
        c.tv.sync_secs = 0;
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &c, 0.0);
        assert_eq!(d.scene(&s, &c, 63.9), Scene::Load);
        assert_eq!(d.scene(&s, &c, 64.1), Scene::Main);
        assert_eq!(d.countdown(&c, 50.0).unwrap().0, Scene::Main);
    }

    #[test]
    fn countdown_names_the_next_scene() {
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &cfg(), 0.0);
        d.scene(&s, &cfg(), 10.0);
        let (next, secs) = d.countdown(&cfg(), 10.0).unwrap();
        assert_eq!(next, Scene::Load);
        assert!((secs - 39.0).abs() < 0.01, "{secs}");
        d.scene(&s, &cfg(), 50.0);
        assert_eq!(d.countdown(&cfg(), 50.0).unwrap().0, Scene::Sync);
        d.scene(&s, &cfg(), 70.0);
        assert_eq!(d.countdown(&cfg(), 70.0).unwrap(), (Scene::Main, 9.0));
    }

    #[test]
    fn blocked_agent_takes_over_immediately_and_holds() {
        let mut d = Director::new(0.0);
        d.scene(&state(false), &cfg(), 0.0);
        assert_eq!(d.scene(&state(false), &cfg(), 9.9), Scene::Main);
        assert_eq!(d.scene(&state(true), &cfg(), 10.0), Scene::Emergency(EmergencyCause::Blocked));
        assert!(d.transition(10.1) < 1.0); // the cut is running
        assert_eq!(d.transition(10.5), 1.0);
        // unblocked at t = 12 → holds until 17
        assert_eq!(d.scene(&state(false), &cfg(), 12.0), Scene::Emergency(EmergencyCause::Blocked));
        assert_eq!(d.scene(&state(false), &cfg(), 16.9), Scene::Emergency(EmergencyCause::Blocked));
        assert_eq!(d.scene(&state(false), &cfg(), 17.1), Scene::Main);
        // rotation restarts at MAIN with a fresh main_secs
        assert_eq!(d.scene(&state(false), &cfg(), 61.9), Scene::Main);
        assert_eq!(d.scene(&state(false), &cfg(), 62.1), Scene::Load);
    }

    #[test]
    fn memory_critical_is_emergency() {
        let mut d = Director::new(0.0);
        let mut s = state(false);
        s.sys.as_mut().unwrap().mem_available = 500_000_000;
        assert_eq!(d.scene(&s, &cfg(), 10.0), Scene::Emergency(EmergencyCause::MemoryCritical));
        let mut both = state(true);
        both.sys.as_mut().unwrap().mem_available = 500_000_000;
        assert_eq!(d.scene(&both, &cfg(), 11.0), Scene::Emergency(EmergencyCause::Both));
    }

    #[test]
    fn link_lost_shows_reconnecting_boot_after_3s() {
        let mut d = Director::new(0.0);
        let ok = state(false);
        d.scene(&ok, &cfg(), 0.0);
        assert_eq!(d.scene(&ok, &cfg(), 19.0), Scene::Main);
        let mut lost = state(false);
        lost.herdr.link = HerdrLink::Lost { since: t0(), reason: "socket".into() };
        assert_eq!(d.scene(&lost, &cfg(), 20.0), Scene::Main);
        assert_eq!(d.scene(&lost, &cfg(), 22.9), Scene::Main);
        assert_eq!(d.scene(&lost, &cfg(), 23.1), Scene::Boot { reconnecting: true });
        // relinked → back to rotation at MAIN
        assert_eq!(d.scene(&ok, &cfg(), 30.0), Scene::Main);
    }

    #[test]
    fn rotate_false_stays_main() {
        let mut c = cfg();
        c.tv.rotate = false;
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &c, 0.0);
        for t in [5.0, 50.0, 70.0, 500.0] {
            assert_eq!(d.scene(&s, &c, t), Scene::Main);
        }
        assert!(d.countdown(&c, 500.0).is_none());
    }

    fn pinned(v: ViewName) -> ViewMode {
        ViewMode::Pinned(v)
    }

    #[test]
    fn pinned_load_stays_load_across_rotation_time() {
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &cfg(), 0.0);
        d.scene(&s, &cfg(), 10.0);
        d.set_mode(pinned(ViewName::Load));
        for t in [10.0, 49.5, 64.5, 80.0, 500.0] {
            assert_eq!(d.scene(&s, &cfg(), t), Scene::Load, "t={t}");
        }
        assert!(d.countdown(&cfg(), 80.0).is_none(), "no rotation countdown while pinned");
    }

    #[test]
    fn pinned_view_with_blocked_agent_follows_emergency_overrides_pin() {
        let mut d = Director::new(0.0);
        d.set_mode(pinned(ViewName::Load));
        assert_eq!(d.scene(&state(true), &cfg(), 10.0), Scene::Emergency(EmergencyCause::Blocked));
        assert!(!d.preview());
        let mut c = cfg();
        c.tv.emergency_overrides_pin = false;
        let mut d = Director::new(0.0);
        d.set_mode(pinned(ViewName::Load));
        assert_eq!(d.scene(&state(true), &c, 10.0), Scene::Load);
    }

    #[test]
    fn pinned_emergency_without_emergency_is_a_preview() {
        let mut d = Director::new(0.0);
        d.set_mode(pinned(ViewName::Emergency));
        assert_eq!(d.scene(&state(false), &cfg(), 10.0), Scene::Emergency(EmergencyCause::Blocked));
        assert!(d.preview());
        // a real emergency is not a preview
        assert_eq!(d.scene(&state(true), &cfg(), 11.0), Scene::Emergency(EmergencyCause::Blocked));
        assert!(!d.preview());
        // the cut into the real one is not a new cut (same view)
        assert_eq!(d.entered_at(), 10.0);
    }

    #[test]
    fn auto_mode_rotates_as_before() {
        let mut d = Director::new(0.0);
        d.set_mode(ViewMode::Auto);
        let s = state(false);
        d.scene(&s, &cfg(), 0.0);
        assert_eq!(d.scene(&s, &cfg(), 48.9), Scene::Main);
        assert_eq!(d.scene(&s, &cfg(), 49.1), Scene::Load);
        assert!(!d.preview());
    }

    #[test]
    fn back_to_auto_restarts_rotation_at_main() {
        let mut d = Director::new(0.0);
        let s = state(false);
        d.scene(&s, &cfg(), 0.0);
        d.set_mode(pinned(ViewName::Sync));
        assert_eq!(d.scene(&s, &cfg(), 20.0), Scene::Sync);
        assert_eq!(d.entered_at(), 20.0);
        d.set_mode(ViewMode::Auto);
        assert_eq!(d.scene(&s, &cfg(), 100.0), Scene::Main);
        assert_eq!(d.entered_at(), 100.0);
        assert_eq!(d.scene(&s, &cfg(), 144.9), Scene::Main);
        assert_eq!(d.scene(&s, &cfg(), 145.1), Scene::Load);
    }

    #[test]
    fn pinned_view_skips_launch_boot_but_not_reconnect() {
        let mut d = Director::new(0.0);
        d.set_mode(pinned(ViewName::Main));
        let ok = state(false);
        assert_eq!(d.scene(&ok, &cfg(), 0.0), Scene::Main);
        let mut lost = state(false);
        lost.herdr.link = HerdrLink::Lost { since: t0(), reason: "socket".into() };
        assert_eq!(d.scene(&lost, &cfg(), 20.0), Scene::Main);
        assert_eq!(d.scene(&lost, &cfg(), 23.1), Scene::Boot { reconnecting: true });
    }

    #[test]
    fn view_name_of_each_scene() {
        assert_eq!(Scene::Load.view_name(), ViewName::Load);
        assert_eq!(Scene::Emergency(EmergencyCause::Both).view_name(), ViewName::Emergency);
        assert_eq!(Scene::Boot { reconnecting: true }.view_name(), ViewName::Main);
    }

}
