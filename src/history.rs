//! Fixed-size, time-bucketed series for the sparkline/trace panels.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Series {
    cap: usize,
    bucket_s: u64,
    /// (bucket index since epoch, value); oldest first.
    points: VecDeque<(u64, f32)>,
}

impl Series {
    pub fn new(cap: usize, bucket: Duration) -> Self {
        Self { cap, bucket_s: bucket.as_secs().max(1), points: VecDeque::with_capacity(cap) }
    }

    fn bucket_of(&self, t: SystemTime) -> u64 {
        t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs() / self.bucket_s
    }

    /// Record a sample; samples in the same bucket keep the maximum.
    pub fn record(&mut self, now: SystemTime, v: f32) {
        let b = self.bucket_of(now);
        match self.points.back_mut() {
            Some((last, val)) if *last == b => *val = val.max(v),
            _ => {
                self.points.push_back((b, v));
                while self.points.len() > self.cap {
                    self.points.pop_front();
                }
            }
        }
    }

    /// `cap` slots ending at `now`, oldest first; `None` where no sample landed.
    pub fn values(&self, now: SystemTime) -> Vec<Option<f32>> {
        let end = self.bucket_of(now);
        let start = end.saturating_sub(self.cap as u64 - 1);
        let mut out = vec![None; self.cap];
        for &(b, v) in &self.points {
            if b >= start && b <= end {
                out[(b - start) as usize] = Some(v);
            }
        }
        out
    }

    pub fn latest(&self) -> Option<f32> {
        self.points.back().map(|&(_, v)| v)
    }

    pub fn max(&self) -> f32 {
        self.points.iter().map(|&(_, v)| v).fold(0.0, f32::max)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Histories {
    /// Number of working agents, 60 min in 10 s buckets (the SYNC trace).
    pub working: Series,
    /// cpu_total 0..1, last 120 s.
    pub cpu: Series,
    /// bytes/s, last 120 s.
    pub net_rx: Series,
    pub net_tx: Series,
}

impl Histories {
    pub fn new() -> Self {
        Self {
            working: Series::new(360, Duration::from_secs(10)),
            cpu: Series::new(120, Duration::from_secs(1)),
            net_rx: Series::new(120, Duration::from_secs(1)),
            net_tx: Series::new(120, Duration::from_secs(1)),
        }
    }
}

impl Default for Histories {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000 + s)
    }

    #[test]
    fn same_bucket_keeps_max() {
        let mut s = Series::new(6, Duration::from_secs(10));
        s.record(t(0), 2.0);
        s.record(t(3), 5.0);
        s.record(t(7), 1.0);
        assert_eq!(s.latest(), Some(5.0));
    }

    #[test]
    fn values_align_to_now_with_gaps() {
        let mut s = Series::new(4, Duration::from_secs(1));
        s.record(t(0), 1.0);
        s.record(t(2), 3.0);
        assert_eq!(s.values(t(3)), vec![Some(1.0), None, Some(3.0), None]);
    }

    #[test]
    fn capacity_drops_oldest() {
        let mut s = Series::new(3, Duration::from_secs(1));
        for i in 0..5 {
            s.record(t(i), i as f32);
        }
        assert_eq!(s.values(t(4)), vec![Some(2.0), Some(3.0), Some(4.0)]);
        assert_eq!(s.max(), 4.0);
    }
}
