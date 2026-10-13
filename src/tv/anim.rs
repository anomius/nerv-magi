//! Time-based animation state for the TV view: eased counters, MAGI vote flicker, the red flash
//! when an agent becomes blocked, and the boot clock. Times are seconds since the view started.

use std::collections::HashMap;

use crate::state::AgentStatus;

/// Full-screen red flash length when an agent becomes blocked.
pub const FLASH_S: f32 = 0.5;
/// Peak alpha of that flash.
pub const FLASH_ALPHA: f32 = 0.35;
/// MAGI verdict flicker after a vote changes.
pub const FLICKER_S: f32 = 0.4;
/// A dot travels a MAGI circuit trace this long after that unit's vote changes.
pub const TRACE_S: f32 = 1.2;
/// Length of the 1 Hz history rings.
pub const RING_CAP: usize = 120;
/// Counter easing rate (1/s): ~95 % of the way in 0.4 s.
const ROLL_RATE: f32 = 7.5;

/// One cached value, recomputed when its key changes.
#[derive(Debug)]
pub struct Memo<K, V>(Option<(K, V)>);

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Self(None)
    }
}

impl<K: PartialEq, V> Memo<K, V> {
    pub fn get(&mut self, key: K, compute: impl FnOnce() -> V) -> &V {
        if self.0.as_ref().is_none_or(|(k, _)| *k != key) {
            self.0 = Some((key, compute()));
        }
        &self.0.as_ref().expect("just set").1
    }
}

pub struct Anim {
    /// ACQUIRING overlays recorded by the base pass, drawn by every live pass: (plot rect, colour).
    pub overlays: Vec<(crate::tv::layout::Rect, femtovg::Color)>,
    rolls: HashMap<String, (f32, f32)>, // key → (value, last update time)
    rolls64: HashMap<String, (f64, f32)>,
    seen_transitions: u64,
    flash_until: f32,
    votes: [Option<bool>; 3],
    flicker_until: [f32; 3],
    vote_changed_at: [f32; 3],
    /// (seconds_left as last seen, when it last changed) for the timer's sub-second countdown.
    power_sample: Option<(Option<u64>, f32)>,
    peaks: Vec<f32>,
    peaks_at: f32,
    rings: HashMap<String, (Vec<f32>, f32)>,
    /// Ticker item widths per (transitions, item count, window scale bits).
    pub ticker_widths: Memo<(u64, usize, u32), Vec<f32>>,
}

impl Anim {
    pub fn new(now: f32) -> Self {
        Self {
            overlays: Vec::new(),
            rolls: HashMap::new(),
            rolls64: HashMap::new(),
            seen_transitions: 0,
            flash_until: f32::NEG_INFINITY,
            votes: [None; 3],
            flicker_until: [f32::NEG_INFINITY; 3],
            vote_changed_at: [f32::NEG_INFINITY; 3],
            power_sample: None,
            peaks: Vec::new(),
            peaks_at: now,
            rings: HashMap::new(),
            ticker_widths: Memo::default(),
        }
    }

    /// Ease the counter `key` toward `target`; the first value seen for a key is shown as is.
    pub fn roll(&mut self, key: &str, target: f32, now: f32) -> f32 {
        match self.rolls.get_mut(key) {
            Some((value, last)) => {
                let dt = (now - *last).max(0.0);
                *value += (target - *value) * (1.0 - (-ROLL_RATE * dt).exp());
                *last = now;
                *value
            }
            None => {
                self.rolls.insert(key.to_string(), (target, now));
                target
            }
        }
    }

    pub fn seen_transitions(&self) -> u64 {
        self.seen_transitions
    }

    /// Feed the state's transition counter and the newest transition's target status.
    pub fn observe(&mut self, transitions: u64, newest_to: Option<AgentStatus>, now: f32) {
        if transitions > self.seen_transitions && newest_to == Some(AgentStatus::Blocked) {
            self.flash_until = now + FLASH_S;
        }
        self.seen_transitions = transitions;
    }

    /// Alpha of the full-screen red flash at `now` (0 when none).
    pub fn flash_alpha(&self, now: f32) -> f32 {
        let remaining = self.flash_until - now;
        if remaining > 0.0 {
            FLASH_ALPHA * (remaining / FLASH_S).min(1.0)
        } else {
            0.0
        }
    }

    /// Record MAGI unit `i`'s vote; a change starts a flicker. Returns whether the verdict is
    /// visible this frame (it blinks at 15 Hz while flickering).
    pub fn vote(&mut self, i: usize, approve: bool, now: f32) -> bool {
        if let Some(prev) = self.votes[i] {
            if prev != approve {
                self.flicker_until[i] = now + FLICKER_S;
                self.vote_changed_at[i] = now;
            }
        }
        self.votes[i] = Some(approve);
        now >= self.flicker_until[i] || ((now * 30.0) as i64) % 2 == 0
    }

    /// 0..1 position of the dot on unit `i`'s circuit trace, while one is travelling.
    pub fn trace_pulse(&self, i: usize, now: f32) -> Option<f32> {
        let f = (now - self.vote_changed_at[i]) / TRACE_S;
        (0.0..1.0).contains(&f).then_some(f)
    }

    /// Seconds since `seconds_left` last changed value, capped just under one sample period so
    /// a stale estimate never counts down further than a second.
    pub fn since_power_sample(&mut self, seconds_left: Option<u64>, now: f32) -> f32 {
        match self.power_sample {
            Some((v, at)) if v == seconds_left => (now - at).clamp(0.0, 0.99),
            _ => {
                self.power_sample = Some((seconds_left, now));
                0.0
            }
        }
    }

    /// `roll` for large counters: eases in f64 and lands exactly on `target`.
    pub fn roll_u64(&mut self, key: &str, target: u64, now: f32) -> u64 {
        let goal = target as f64;
        let Some((value, last)) = self.rolls64.get_mut(key) else {
            self.rolls64.insert(key.to_string(), (goal, now));
            return target;
        };
        let dt = (now - *last).max(0.0) as f64;
        *value += (goal - *value) * (1.0 - (-(ROLL_RATE as f64) * dt).exp());
        *last = now;
        if (goal - *value).abs() < 0.5 {
            *value = goal;
        }
        value.round() as u64
    }

    /// Spectrum peak caps: each holds the highest bar seen and falls at 1 unit/s.
    pub fn peaks(&mut self, bars: &[f32], now: f32) -> &[f32] {
        let dt = (now - self.peaks_at).max(0.0);
        self.peaks_at = now;
        if self.peaks.len() != bars.len() {
            self.peaks = bars.to_vec();
        } else {
            for (p, &b) in self.peaks.iter_mut().zip(bars) {
                *p = (*p - dt).max(b).max(0.0);
            }
        }
        &self.peaks
    }

    /// 1 Hz history ring for `key` (last 120 samples, oldest first); `v` is recorded at most once
    /// per second.
    pub fn ring(&mut self, key: &str, v: f32, now: f32) -> &[f32] {
        if !self.rings.contains_key(key) {
            self.rings.insert(key.to_string(), (Vec::with_capacity(RING_CAP), f32::NEG_INFINITY));
        }
        let (values, last) = self.rings.get_mut(key).expect("inserted above");
        if now - *last >= 1.0 - 1e-4 {
            values.push(v);
            if values.len() > RING_CAP {
                values.remove(0);
            }
            *last = now;
        }
        values
    }

    /// The samples of ring `key` so far (empty if never sampled).
    pub fn ring_values(&self, key: &str) -> &[f32] {
        self.rings.get(key).map_or(&[], |(v, _)| v.as_slice())
    }

    /// Anything time-limited still running (flash, flicker)?
    pub fn busy(&self, now: f32) -> bool {
        now < self.flash_until || self.vote_changed_at.iter().any(|&c| now < c + TRACE_S)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_recomputes_only_when_the_key_changes() {
        let mut m: Memo<(u64, u32), Vec<f32>> = Memo::default();
        let mut calls = 0;
        let mut get = |m: &mut Memo<(u64, u32), Vec<f32>>, k| {
            m.get(k, || {
                calls += 1;
                vec![k.0 as f32]
            })
            .clone()
        };
        assert_eq!(get(&mut m, (1, 7)), vec![1.0]);
        assert_eq!(get(&mut m, (1, 7)), vec![1.0]);
        assert_eq!(get(&mut m, (2, 7)), vec![2.0]);
        drop(get);
        assert_eq!(calls, 2);
    }

    #[test]
    fn roll_eases_toward_target() {
        let mut a = Anim::new(0.0);
        assert_eq!(a.roll("cpu", 100.0, 0.0), 100.0); // first sight snaps
        let v = a.roll("cpu", 0.0, 0.1);
        assert!(v < 100.0 && v > 0.0);
        assert!(a.roll("cpu", 0.0, 5.0) < 0.5);
    }

    #[test]
    fn flash_starts_on_new_blocked_transition_only() {
        let mut a = Anim::new(0.0);
        a.observe(3, Some(crate::state::AgentStatus::Blocked), 10.0);
        assert!(a.flash_alpha(10.1) > 0.0);
        assert_eq!(a.flash_alpha(10.6), 0.0);
        a.observe(3, Some(crate::state::AgentStatus::Blocked), 20.0); // same count: no new flash
        assert_eq!(a.flash_alpha(20.1), 0.0);
        a.observe(4, Some(crate::state::AgentStatus::Done), 30.0);
        assert_eq!(a.flash_alpha(30.1), 0.0);
    }

    #[test]
    fn roll_u64_is_exact_for_big_counters() {
        let mut a = Anim::new(0.0);
        assert_eq!(a.roll_u64("tok", 62_840_356, 0.0), 62_840_356);
        let mid = a.roll_u64("tok", 62_900_000, 0.05);
        assert!(mid > 62_840_356 && mid < 62_900_000);
        assert_eq!(a.roll_u64("tok", 62_900_000, 10.0), 62_900_000);
    }

    #[test]
    fn peaks_hold_then_fall_one_unit_per_second() {
        let mut a = Anim::new(0.0);
        assert_eq!(a.peaks(&[1.0, 0.2], 0.0), &[1.0, 0.2]);
        let p = a.peaks(&[0.0, 0.5], 0.5).to_vec();
        assert!((p[0] - 0.5).abs() < 1e-4 && (p[1] - 0.5).abs() < 1e-4, "{p:?}");
        assert!((a.peaks(&[0.0, 0.0], 2.0)[0]).abs() < 1e-4);
        assert_eq!(a.peaks(&[0.3, 0.3, 0.3], 2.1).len(), 3); // bar count change resets
    }

    #[test]
    fn ring_samples_once_per_second_and_caps() {
        let mut a = Anim::new(0.0);
        a.ring("gpu", 1.0, 0.0);
        assert_eq!(a.ring("gpu", 2.0, 0.5), &[1.0]);
        assert_eq!(a.ring("gpu", 3.0, 1.0), &[1.0, 3.0]);
        for i in 2..300 {
            a.ring("gpu", i as f32, i as f32);
        }
        assert_eq!(a.ring("gpu", 0.0, 299.5).len(), 120);
    }

    #[test]
    fn vote_change_flickers_and_sends_a_trace_pulse() {
        let mut a = Anim::new(0.0);
        assert!(a.vote(0, true, 1.0)); // first sight: steady, no pulse
        assert_eq!(a.trace_pulse(0, 1.1), None);
        a.vote(0, false, 2.0);
        assert!(a.busy(2.1));
        let p = a.trace_pulse(0, 2.3).unwrap();
        assert!(p > 0.0 && p < 1.0);
        assert_eq!(a.trace_pulse(0, 2.0 + TRACE_S + 0.1), None);
        assert!(a.vote(0, false, 3.0)); // flicker over
    }
}
