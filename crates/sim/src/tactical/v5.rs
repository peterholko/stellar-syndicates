//! Frozen discovery-weapon rules. Old fittings use the original arithmetic;
//! old recordings select their old kernel. Movement and escort PD remain v4.

use super::{Rules, rules_v1, v1, v3};
use crate::{EntityId, Vec2, module::{DamageType, Loadout, ModuleKind}, ship::Ship};
use v1::{Distribution, ProjSetup, SimOutcome, TacticalState};

pub(super) fn offense(fit: &Loadout) -> (DamageType, f64) {
    let n = fit.modules().iter().filter(|m| **m == ModuleKind::PrismaticLance).count();
    if n > 0 && !fit.modules().contains(&ModuleKind::TorpedoRack) && !fit.modules().contains(&ModuleKind::MassDriver) {
        (DamageType::Beam, 1.45 * n as f64)
    } else { rules_v1::offense(fit) }
}

fn rules(seed: u64, battle: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, battle) else { unreachable!() };
    Rules::V5 { maneuver_seed }
}

impl TacticalState {
    #[allow(clippy::too_many_arguments)]
    pub fn open_v5(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        platform_tiers: u32, platform_pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v4(seed, battle, a, d, platform_tiers, platform_pool, bearing);
        state.rules = rules(seed, battle);
        state
    }
}

#[allow(dead_code)] // archived rules remain executable for regression comparisons
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    v1::simulate_engagement_with_rules(setup, seed, rules(seed, v1::PROJECTION_BATTLE_ID))
}

#[allow(dead_code)]
pub fn project_distribution(setup: &ProjSetup, base_seed: u64, k: u32) -> Distribution {
    v1::project_distribution_using(setup, base_seed, k, simulate_engagement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShipKind;
    use v1::SideMods;

    fn scene(fit: &str) -> TacticalState {
        let a = [(EntityId(1), Ship::new(0, ShipKind::Destroyer, Loadout::from_key(fit)))];
        let d = [(EntityId(2), Ship::new(0, ShipKind::Titan, Loadout::default()))];
        let mut state = TacticalState::open_v5(11, 50, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        state.combatants[0].pos = Vec2::new(150.0, 0.0);
        state.combatants[1].pos = Vec2::ZERO;
        state
    }

    #[test]
    fn discovery_lance_reaches_real_combat_without_rebalancing_archived_fights() {
        let mut new = scene("prismatic_lance");
        let mut old = new.clone();
        let Rules::V5 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V4 { maneuver_seed };
        let mut damage = [0.0; 2];
        for _ in 0..12 {
            damage[0] += old.step(false, [SideMods::default(); 2]).dealt[0];
            damage[1] += new.step(false, [SideMods::default(); 2]).dealt[0];
        }
        assert!(damage[0] > 0.0);
        assert!((damage[1] / damage[0] - 1.45).abs() < 1e-10);
        let saved = serde_json::to_string(&old).unwrap();
        let mut restored: TacticalState = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.rules_version(), 4);
        old.step(false, [SideMods::default(); 2]);
        restored.step(false, [SideMods::default(); 2]);
        assert_eq!(restored, old);
    }

    #[test]
    fn previous_equipment_has_identical_damage_motion_and_rng_in_v5() {
        for fit in ["", "mass_driver", "torpedo_rack", "point_defense_screen", "reflective_plating"] {
            let mut new = scene(fit);
            let mut old = new.clone();
            let Rules::V5 { maneuver_seed } = new.rules else { unreachable!() };
            old.rules = Rules::V4 { maneuver_seed };
            for _ in 0..60 {
                old.step(false, [SideMods::default(); 2]);
                new.step(false, [SideMods::default(); 2]);
                let mut normalized = new.clone(); normalized.rules = old.rules;
                assert_eq!(normalized, old, "unchanged fitting {fit}");
            }
        }
        let setup = ProjSetup { a: vec![(EntityId(1), Ship::new(0, ShipKind::Raider, Loadout::default()))],
            d: vec![(EntityId(2), Ship::new(0, ShipKind::Corvette, Loadout::default()))],
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
        let old = super::super::v4::project_distribution(&setup, 42, 8);
        let new = project_distribution(&setup, 42, 8);
        assert_eq!(old.median, new.median);
        assert_eq!(old.a_win_pct, new.a_win_pct);
    }
}
