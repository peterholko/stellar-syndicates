//! Versioned tactical combat and private deterministic battle recordings.
//!
//! New battles use v6 mission profiles, v5 discovery weapons, v4 escort datalinks
//! and v3 maneuvers. Keep old rules:
//! a saved battle's version selects its original rules, not the
//! newest balance. The server alone reconstructs archives; seeds/state are
//! never part of the player protocol.

pub mod replay;
mod rules_v1;
mod v1;
mod v2;
mod v3;
mod v4;
mod v5;
mod v6;

pub use v1::*;
pub use v6::{project_distribution, project_distribution_with_missions, simulate_engagement};

/// Captured once at battle open. Missing in pre-v2 saves means v1, including
/// an already-running fight; loading a save must never rebalance its replay.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Rules {
    #[default]
    V1,
    V2,
    /// One private, frozen maneuver seed per engagement, not per frame or viewer.
    V3 { maneuver_seed: u64 },
    /// Local datalink fire control; the rest of the kernel retains v3 motion.
    V4 { maneuver_seed: u64 },
    /// Discovery weapons; v4 steering, escort fire control and all old fittings unchanged.
    V5 { maneuver_seed: u64 },
    V6 { maneuver_seed: u64 },
}

impl Rules {
    fn is_v1(&self) -> bool { *self == Self::V1 }
}

impl TacticalState {
    /// Add the other side's stationary support BEFORE the first recording
    /// checkpoint. Use the archived constructor's exact platform HP/weapons;
    /// no changes to old rules, random streams, or pre-existing battle rosters.
    pub fn add_initial_attacker_platforms(&mut self, tiers: u32, pool: f64) {
        if tiers == 0 { return; }
        assert_eq!(self.step, 0);
        let support = Self::open(0, 0, &[], &[], tiers, pool, self.bearing);
        for mut platform in support.combatants {
            platform.cid = self.next_cid;
            self.next_cid += 1;
            platform.side = 0;
            platform.pos = platform.pos + self.bearing * SPAWN_DIST;
            self.combatants.push(platform);
        }
    }

    pub fn platform_tiers_for(&self, side: u8) -> u32 {
        self.combatants.iter().filter(|c| c.platform && c.side == side && c.hp > 0.0).count() as u32
    }

    pub fn platform_pool_for(&self, side: u8) -> f64 {
        self.combatants.iter().filter(|c| c.platform && c.side == side && c.hp > 0.0)
            .map(|c| (c.max_hp - c.hp).max(0.0)).sum()
    }

    pub fn rules_version(&self) -> u32 {
        match self.rules {
            Rules::V1 => 1,
            Rules::V2 => 2,
            Rules::V3 { .. } => 3,
            Rules::V4 { .. } => 4,
            Rules::V5 { .. } => 5,
            Rules::V6 { .. } => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_combat_tables_match_the_current_ship_and_module_catalog() {
        // When changing live balance, introduce/select the next combat version
        // and update this ACTIVE-version check. Never retune archived v1.
        for kind in crate::ship::ALL_SHIP_KINDS {
            assert_eq!(super::rules_v1::hull_mass(kind), kind.hull_mass());
            assert_eq!(super::rules_v1::max_speed(kind), kind.max_speed());
            assert_eq!(super::rules_v1::attack_weight(kind), kind.attack_weight());
            assert_eq!(
                super::rules_v1::featured(kind),
                kind.is_combatant() && crate::ship::requires_hull_unlock(kind)
            );
            for family in [
                crate::module::Family::Beam,
                crate::module::Family::Driver,
                crate::module::Family::Torpedo,
                crate::module::Family::Interception,
                crate::module::Family::Protection,
            ] {
                assert_eq!(
                    super::rules_v1::hull_affinity(kind, family),
                    crate::ship::hull_affinity(kind, family)
                );
            }
        }
        for first in crate::module::MODULE_KINDS {
            for second in crate::module::MODULE_KINDS {
                let fit = crate::Loadout::new(vec![first, second]);
                assert_eq!(super::v5::offense(&fit), fit.offense());
            }
        }
        for seed in [0, 1, 123, u64::MAX] {
            let mut frozen = super::rules_v1::Rng::new(seed);
            let mut live = crate::rng::Rng::new(seed);
            for _ in 0..100 {
                assert_eq!(frozen.next_u64(), live.next_u64());
            }
        }
    }
}
