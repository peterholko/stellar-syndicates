//! §leave (R4, Oct 22 2026): vacation mode, after Travian's.
//!
//! A corporation on leave can't be attacked, raided, blockaded or hit by pirate
//! home raids, and it gives no orders: everything it sends is refused except
//! ending the leave. Its automation (production, freight routes, standing rules)
//! keeps running, so an absence costs progress but never the colony.
//!
//! Leave can't begin mid-battle or under a blockade, lasts at least
//! [`LEAVE_MIN_S`] before an early return, ends by itself after
//! [`LEAVE_MAX_S`], and can't be taken again for [`LEAVE_COOLDOWN_S`] after it
//! ends, so it can't be toggled to dodge a fight.

use serde::{Deserialize, Serialize};

/// The shortest leave: an early return is possible only after this (2 days).
pub const LEAVE_MIN_S: f64 = 2.0 * 86_400.0;
/// The longest: leave ends by itself after this (15 days, Travian's x1 cap).
pub const LEAVE_MAX_S: f64 = 15.0 * 86_400.0;
/// After a leave ends, the next can start only this much later (7 days).
pub const LEAVE_COOLDOWN_S: f64 = 7.0 * 86_400.0;

/// A corporation's current or last leave. Default: never taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Leave {
    /// When the current (or last) leave began.
    #[serde(default)]
    pub started: Option<f64>,
    /// When it ends (or ended): [`LEAVE_MAX_S`] after the start, or the early
    /// return.
    #[serde(default)]
    pub until: Option<f64>,
}

impl Leave {
    /// On leave at `now`.
    pub fn active(&self, now: f64) -> bool {
        self.started.is_some_and(|s| now >= s) && self.until.is_some_and(|u| now < u)
    }

    /// The earliest moment the current leave can end early.
    pub fn earliest_return(&self) -> Option<f64> {
        self.started.map(|s| s + LEAVE_MIN_S)
    }

    /// May the current leave end now?
    pub fn may_end(&self, now: f64) -> bool {
        self.active(now) && self.earliest_return().is_some_and(|t| now >= t)
    }

    /// When the next leave may start (0 if none was ever taken).
    pub fn next_start(&self) -> f64 {
        self.until.map_or(0.0, |u| u + LEAVE_COOLDOWN_S)
    }

    /// May a new leave start now?
    pub fn may_start(&self, now: f64) -> bool {
        !self.active(now) && now >= self.next_start()
    }

    pub fn start(&mut self, now: f64) {
        self.started = Some(now);
        self.until = Some(now + LEAVE_MAX_S);
    }

    pub fn end(&mut self, now: f64) {
        self.until = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leave_runs_two_to_fifteen_days_then_cools_down_a_week() {
        let day = 86_400.0;
        let mut leave = Leave::default();
        assert!(!leave.active(0.0) && leave.may_start(0.0), "never taken: free to start");
        leave.start(10.0);
        assert!(leave.active(10.0) && leave.active(10.0 + 14.9 * day));
        assert!(!leave.may_end(10.0 + day), "two days at least");
        assert!(leave.may_end(10.0 + 2.0 * day));
        assert!(!leave.active(10.0 + 15.0 * day), "ends by itself after fifteen days");
        assert!(!leave.may_start(10.0 + 20.0 * day), "a week's cooldown after it ends");
        assert!(leave.may_start(10.0 + 22.0 * day));
        let mut early = Leave::default();
        early.start(0.0);
        early.end(3.0 * day);
        assert!(!early.active(3.0 * day), "an early return ends it at once");
        assert_eq!(early.next_start(), 10.0 * day, "the cooldown runs from the return");
    }
}
