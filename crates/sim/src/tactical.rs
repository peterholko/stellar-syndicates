//! Versioned tactical combat and private deterministic battle recordings.
//!
//! New battles use v3 independent Interceptor maneuvers. Keep old movement and tables:
//! a saved battle's version selects its original rules, not the
//! newest balance. The server alone reconstructs archives; seeds/state are
//! never part of the player protocol.

pub mod replay;
mod rules_v1;
mod v1;
mod v2;
mod v3;

pub use v1::*;
pub use v3::{project_distribution, simulate_engagement};

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
}

impl Rules {
    fn is_v1(&self) -> bool { *self == Self::V1 }
}

impl TacticalState {
    pub fn rules_version(&self) -> u32 {
        match self.rules {
            Rules::V1 => 1,
            Rules::V2 => 2,
            Rules::V3 { .. } => 3,
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
                crate::ship::requires_hull_unlock(kind)
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
                assert_eq!(super::rules_v1::offense(&fit), fit.offense());
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
