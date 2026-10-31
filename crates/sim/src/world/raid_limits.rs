//! §player-styles (Peter, Oct 31 2026): the daily loot cap. A corporation loses
//! at most a quarter of its storage a day to other corporations' raids and
//! blockades, so a raided player still grows. Once its owner has given no order
//! for three days the cap no longer applies: an abandoned colony is a farm.
//! Pirates keep their own limits (`pirate::HOME_RAID_MAX_LOOT`), and freight
//! taken in space and a captured system's stockpile don't count.
use serde::{Deserialize, Serialize};

use super::*;

/// The share of a corporation's storage it can lose in a day. Tunable.
pub const RAID_LOSS_CAP_FRAC: f64 = 0.25;
/// The cap's rolling window.
pub const RAID_LOSS_WINDOW_S: f64 = 24.0 * 3_600.0;
/// No order for this long and the cap no longer applies.
pub const RAID_CAP_INACTIVE_S: f64 = 3.0 * 24.0 * 3_600.0;

const HOUR_S: f64 = 3_600.0;
const WINDOW_HOURS: u32 = (RAID_LOSS_WINDOW_S / HOUR_S) as u32;

/// Units a corporation lost to raids and blockades, in hourly buckets over
/// the last day.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RaidLosses {
    buckets: Vec<(u32, f64)>,
}

impl RaidLosses {
    fn hour(now: f64) -> u32 {
        (now.max(0.0) / HOUR_S) as u32
    }

    /// Units lost within the window ending at `now`.
    pub fn taken(&self, now: f64) -> f64 {
        let h = Self::hour(now);
        self.buckets.iter().filter(|(b, _)| b + WINDOW_HOURS > h).map(|(_, n)| n).sum()
    }

    fn add(&mut self, now: f64, units: f64) {
        let h = Self::hour(now);
        self.buckets.retain(|(b, _)| b + WINDOW_HOURS > h);
        match self.buckets.last_mut() {
            Some((b, n)) if *b == h => *n += units,
            _ => self.buckets.push((h, units)),
        }
    }
}

impl World {
    /// The units `victim` may lose to raids and blockades in a day, or None
    /// when no cap applies (its owner has been away three days).
    pub fn raid_loss_cap(&self, victim: PlayerId) -> Option<f64> {
        let corp = self.players.get(&victim)?;
        if self.time - corp.last_active >= RAID_CAP_INACTIVE_S {
            return None;
        }
        let storage: f64 = self.systems.iter().filter(|s| s.owner == Some(victim)).map(|s| s.storage_cap()).sum();
        Some(storage * RAID_LOSS_CAP_FRAC)
    }

    /// What `victim` may still lose today (infinite without a cap).
    pub fn raid_loss_left(&self, victim: PlayerId) -> f64 {
        match self.raid_loss_cap(victim) {
            Some(cap) => (cap - self.players.get(&victim).map_or(0.0, |c| c.raid_losses.taken(self.time))).max(0.0),
            None => f64::INFINITY,
        }
    }

    pub(super) fn note_raid_loss(&mut self, victim: PlayerId, units: f64) {
        let now = self.time;
        if let Some(corp) = self.players.get_mut(&victim) {
            corp.raid_losses.add(now, units);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losses_roll_out_of_the_day_by_the_hour() {
        let mut l = RaidLosses::default();
        l.add(10.0, 5.0);
        l.add(100.0, 7.0);
        l.add(2.0 * HOUR_S, 3.0);
        assert_eq!(l.taken(2.0 * HOUR_S), 15.0);
        assert_eq!(l.taken(RAID_LOSS_WINDOW_S + 10.0), 3.0, "the first hour has rolled out");
        assert_eq!(l.taken(RAID_LOSS_WINDOW_S + 3.0 * HOUR_S), 0.0);
    }
}
