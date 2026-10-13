//! Two-layer rendering for the steady views (perf): a cached **base** image of everything that
//! changes at data rate, re-rendered at most every BASE_S, plus a **live** layer drawn every frame
//! on top (clock digits, timer centiseconds, rails, reels, cava, ticker lane, roll band).

use super::scene::Scene;

/// Base layer refresh period (the collectors deliver at 1 Hz).
pub const BASE_S: f32 = 1.0;

/// Which part of a view a draw call belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Everything (full render: snapshots of non-steady views, cuts, BOOT, EMERGENCY).
    All,
    /// Data-rate parts only (rendered into the cached image).
    Base,
    /// Per-frame motion only (drawn over the cached image).
    Live,
}

impl Layer {
    pub fn base(self) -> bool {
        self != Layer::Live
    }
    pub fn live(self) -> bool {
        self != Layer::Base
    }
}

/// Use the cached base layer this frame? For MAIN / LOAD / SYNC / EMERGENCY, with no cut running
/// and no flicker or trace pulse in flight. (ACQUIRING sweeps are drawn on the live layer.) BOOT
/// is drawn in full: it is short and every line animates.
pub fn layered(scene: Scene, cut: f32, busy: bool) -> bool {
    matches!(scene, Scene::Main | Scene::Load | Scene::Sync | Scene::Emergency(_)) && cut >= 1.0 && !busy
}

/// What the base layer must reflect at once rather than at the next BASE_S refresh: status
/// transitions, the agent set, the blocked count and the EMERGENCY preview flag (the live layer
/// lays the EMERGENCY card hexes out from the same data, so a stale base would put them over the
/// wrong cards).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch {
    transitions: u64,
    agents: usize,
    blocked: usize,
    preview: bool,
}

pub fn epoch(state: &crate::state::State, preview: bool) -> Epoch {
    Epoch { transitions: state.transitions, agents: state.herdr.agents.len(), blocked: state.counts().blocked, preview }
}

/// Re-render the base image this frame?
pub fn base_stale(age: f32, scene_changed: bool, size_changed: bool, forced: bool) -> bool {
    age >= BASE_S || scene_changed || size_changed || forced
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::EmergencyCause;

    #[test]
    fn only_steady_views_are_layered() {
        assert!(layered(Scene::Main, 1.0, false));
        assert!(layered(Scene::Load, 1.0, false));
        assert!(layered(Scene::Sync, 1.0, false));
        assert!(!layered(Scene::Main, 0.5, false)); // cut running
        assert!(!layered(Scene::Main, 1.0, true)); // flicker / pulse
        // EMERGENCY can stay up for minutes: its title and cards are base, hazard bars and pulses live
        assert!(layered(Scene::Emergency(EmergencyCause::Blocked), 1.0, false));
        assert!(!layered(Scene::Emergency(EmergencyCause::Blocked), 0.5, false));
        assert!(!layered(Scene::Boot { reconnecting: false }, 1.0, false));
    }

    #[test]
    fn base_refreshes_once_a_second() {
        // collectors deliver at 1 Hz; a faster base only redraws the same data (perf budget)
        assert!(!base_stale(0.9, false, false, false));
        assert!(base_stale(1.0, false, false, false));
    }

    #[test]
    fn base_refreshes_on_age_scene_size_or_force() {
        assert!(!base_stale(0.2, false, false, false));
        assert!(base_stale(BASE_S, false, false, false));
        assert!(base_stale(0.0, true, false, false));
        assert!(base_stale(0.0, false, true, false));
        assert!(base_stale(0.0, false, false, true));
    }

    #[test]
    fn epoch_changes_with_what_the_base_must_show_at_once() {
        use crate::state::tests::{agent, t0};
        use crate::state::{AgentStatus, State, Update};
        let mut s = State::new(t0());
        let a = |p: &str, st| agent(p, p, st, "w1");
        s.apply(Update::HerdrSnapshot { agents: vec![a("p1", AgentStatus::Blocked), a("p2", AgentStatus::Blocked)], workspaces: vec![] }, t0());
        let e0 = epoch(&s, false);
        assert_eq!(e0, epoch(&s, false));
        assert_ne!(e0, epoch(&s, true), "preview ↔ real");
        // one blocked pilot's pane goes away without a status transition
        let mut fewer = s.clone();
        fewer.apply(Update::HerdrSnapshot { agents: vec![a("p1", AgentStatus::Blocked)], workspaces: vec![] }, t0());
        assert_ne!(e0, epoch(&fewer, false));
        // a status transition
        let mut moved = s.clone();
        moved.apply(Update::AgentStatus { pane_id: "p2".into(), status: AgentStatus::Working, title: None }, t0());
        assert_ne!(e0, epoch(&moved, false));
    }

    #[test]
    fn layer_membership() {
        assert!(Layer::All.base() && Layer::All.live());
        assert!(Layer::Base.base() && !Layer::Base.live());
        assert!(!Layer::Live.base() && Layer::Live.live());
    }
}
