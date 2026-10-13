//! Hull-sized weapon reach. Light hulls carry short guns and must close to
//! fight; capitals mount long batteries. Each hull's beam and driver reach,
//! and the stand-off band it steers for, scale with its class. Torpedo launch
//! range, point defense and defense platforms are unchanged; movement, escort
//! datalinks, discovery weapons and mission profiles are exactly v6's. Frozen:
//! new battles open on v8 (`open_current`), which keeps this reach.

use super::{v3, Rules};
use super::v1::{Distribution, ProjSetup, SimOutcome, TacticalState, PROJECTION_BATTLE_ID};
use crate::{doctrine::MissionProfile, ship::Ship, EntityId, ShipKind, Vec2};
use std::collections::BTreeMap;

/// Beam and driver reach as a multiple of the stock ranges (`BEAM_RANGE`,
/// `DRIVER_RANGE`). Unarmed hulls never fire, so theirs is moot.
pub(super) fn reach(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Raider => 0.6,
        ShipKind::Corvette => 0.8,
        ShipKind::Destroyer => 0.9,
        ShipKind::Cruiser => 1.0,
        ShipKind::Battleship => 1.1,
        ShipKind::Dreadnought => 1.2,
        ShipKind::Titan => 1.3,
        _ => 1.0,
    }
}

impl TacticalState {
    /// Frozen v7 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v7(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v6(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V6 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V7 { maneuver_seed };
        state
    }
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V7 { maneuver_seed }
}

#[allow(dead_code)] // archived rules remain executable for regression comparisons
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    super::v1::simulate_engagement_with_rules(setup, seed, rules(seed))
}

#[allow(dead_code)]
pub fn project_distribution(setup: &ProjSetup, seed: u64, k: u32) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, simulate_engagement)
}

/// Estimation receives only profiles in the observer's arrived picture (v6).
#[allow(dead_code)]
pub fn project_distribution_with_missions(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_with_missions(s, seed, rules(seed), missions)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::v1::{SideMods, BEAM_RANGE, DRIVER_RANGE};
    use crate::module::Loadout;

    /// One shooter (side 0) `gap` units from a stationary defense platform.
    fn range_test(shooter: ShipKind, fit: &str, gap: f64, current: bool) -> TacticalState {
        let a = [(EntityId(1), Ship::new(0, shooter, Loadout::from_key(fit)))];
        let mut state = if current {
            TacticalState::open_v7(5, 40, &a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        } else {
            TacticalState::open_v6(5, 40, &a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        };
        for c in &mut state.combatants {
            c.pos = if c.side == 0 { Vec2::new(gap, 0.0) } else { Vec2::ZERO };
        }
        state
    }

    /// Does side 0 open fire within six steps, restarting from rest at the
    /// same spot each step (a hull moves at most its top speed per step)?
    fn fires(mut state: TacticalState) -> bool {
        let start: Vec<Vec2> = state.combatants.iter().map(|c| c.pos).collect();
        (0..6).any(|_| {
            for (c, p) in state.combatants.iter_mut().zip(&start) { c.pos = *p; c.vel = Vec2::ZERO; }
            state.step(false, [SideMods::default(); 2]).gunfire.iter().any(|shot| shot.side == 0)
        })
    }

    #[test]
    fn light_hulls_must_close_while_capitals_reach_further() {
        // 500: inside the stock 650 beam range, beyond an interceptor's 390
        // even after a full 100-unit step toward the target.
        assert!(fires(range_test(ShipKind::Raider, "", 500.0, false)), "v6 interceptor shoots at 500");
        assert!(!fires(range_test(ShipKind::Raider, "", 500.0, true)), "v7 interceptor must close first");
        assert!(fires(range_test(ShipKind::Raider, "", 380.0, true)));
        // 800: beyond the stock range, inside a titan's 845 even after its 23-unit step.
        assert!(!fires(range_test(ShipKind::Titan, "", 800.0, false)));
        assert!(fires(range_test(ShipKind::Titan, "", 800.0, true)), "v7 titan outranges");
        // A cruiser keeps the stock reach (it closes at most 45 a step).
        assert!(fires(range_test(ShipKind::Cruiser, "", 600.0, true)));
        assert!(!fires(range_test(ShipKind::Cruiser, "", 700.0, true)));
        // Drivers: a v7 corvette's falloff band starts at 280, not 350.
        let s = range_test(ShipKind::Corvette, "mass_driver", 300.0, true);
        assert!((s.reach_of(ShipKind::Corvette, false) * DRIVER_RANGE - 280.0).abs() < 1e-9);
        assert_eq!(s.reach_of(ShipKind::Corvette, true), 1.0, "platforms keep their reach");
    }

    #[test]
    fn stand_off_bands_scale_with_reach_and_torpedoes_do_not() {
        let band = |kind, fit: &str, current| {
            let s = range_test(kind, fit, 300.0, current);
            s.preferred_band(&s.combatants[0])
        };
        assert_eq!(band(ShipKind::Corvette, "", false), BEAM_RANGE * 0.85);
        assert!((band(ShipKind::Corvette, "", true) - BEAM_RANGE * 0.85 * 0.8).abs() < 1e-9);
        assert!((band(ShipKind::Corvette, "mass_driver", true) - DRIVER_RANGE * 0.85 * 0.8).abs() < 1e-9);
        assert!((band(ShipKind::Titan, "", true) - BEAM_RANGE * 0.85 * 1.3).abs() < 1e-9);
        assert_eq!(band(ShipKind::Raider, "torpedo_rack", true), band(ShipKind::Raider, "torpedo_rack", false));
        let ladder = [ShipKind::Raider, ShipKind::Corvette, ShipKind::Destroyer, ShipKind::Cruiser,
            ShipKind::Battleship, ShipKind::Dreadnought, ShipKind::Titan].map(reach);
        assert!(ladder.windows(2).all(|w| w[0] < w[1]), "reach grows with hull class");
    }

    #[test]
    fn v7_battles_open_on_v7_and_saved_v6_battles_keep_their_rules() {
        let current = range_test(ShipKind::Raider, "", 500.0, true);
        assert_eq!(current.rules_version(), 7);
        let restored: TacticalState = serde_json::from_slice(&serde_json::to_vec(&current).unwrap()).unwrap();
        assert_eq!(restored, current);
        let old = range_test(ShipKind::Raider, "", 500.0, false);
        let mut saved: TacticalState = serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(saved.rules_version(), 6);
        let mut live = old.clone();
        for _ in 0..40 {
            live.step(false, [SideMods::default(); 2]);
            saved.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(saved, live, "a saved v6 fight replays exactly");
    }

    #[test]
    fn v7_estimates_run_deterministically() {
        let setup = ProjSetup {
            a: (0..6).map(|i| (EntityId(10 + i), Ship::new(0, ShipKind::Raider, Loadout::default()))).collect(),
            d: vec![(EntityId(30), Ship::new(0, ShipKind::Battleship, Loadout::default()))],
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None,
        };
        let first = project_distribution(&setup, 17, 8);
        let again = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, again.a_win_pct);
        assert_eq!(first.median.steps, again.median.steps);
        assert_eq!(simulate_engagement(&setup, 3).steps, simulate_engagement(&setup, 3).steps);
    }
}

/// Manual balance comparison against the frozen v6 rules (prints, asserts
/// nothing): `cargo test -p sim --lib balance_probe -- --ignored --nocapture`.
#[cfg(test)]
mod balance_probe {
    use super::*;
    use crate::module::Loadout;
    use super::super::v1::ProjSetup;

    fn side(base: u64, list: &[(ShipKind, &str, u64)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        let mut id = base;
        for (kind, fit, n) in list {
            for _ in 0..*n { out.push((EntityId(id), Ship::new(0, *kind, Loadout::from_key(fit)))); id += 1; }
        }
        out
    }

    #[test]
    #[ignore]
    fn print_v6_vs_v7_balance() {
        use ShipKind as K;
        let cases: Vec<(&str, Vec<(K, &str, u64)>, Vec<(K, &str, u64)>, bool)> = vec![
            ("6 raider v 4 corvette", vec![(K::Raider, "", 6)], vec![(K::Corvette, "", 4)], false),
            ("10 raider v 1 battleship", vec![(K::Raider, "", 10)], vec![(K::Battleship, "", 1)], false),
            ("20 raider v 1 titan", vec![(K::Raider, "", 20)], vec![(K::Titan, "", 1)], false),
            ("4 corvette v 2 destroyer", vec![(K::Corvette, "", 4)], vec![(K::Destroyer, "", 2)], false),
            ("2 cruiser v 1 battleship", vec![(K::Cruiser, "", 2)], vec![(K::Battleship, "", 1)], false),
            ("5 cruiser mirror", vec![(K::Cruiser, "", 5)], vec![(K::Cruiser, "", 5)], false),
            ("10 raider mirror", vec![(K::Raider, "", 10)], vec![(K::Raider, "", 10)], false),
            ("3 raider v 5 corvette RAID", vec![(K::Raider, "", 3)], vec![(K::Corvette, "", 5)], true),
            ("1 raider v convoy RAID", vec![(K::Raider, "", 1)], vec![(K::Convoy, "", 1)], true),
            ("6 torp raider v 3 PD corvette+convoy", vec![(K::Raider, "torpedo_rack", 6)], vec![(K::Corvette, "point_defense_screen", 3), (K::Convoy, "", 1)], false),
            ("2 dread v 12 raider+6 corvette", vec![(K::Dreadnought, "", 2)], vec![(K::Raider, "", 12), (K::Corvette, "", 6)], false),
            ("titan+4 corv v 3 battleship", vec![(K::Titan, "", 1), (K::Corvette, "", 4)], vec![(K::Battleship, "", 3)], false),
            ("8 driver raider v 2 cruiser", vec![(K::Raider, "mass_driver", 8)], vec![(K::Cruiser, "", 2)], false),
        ];
        for variant in 0..3u8 {
            let patrol = crate::pirate::patrol_fitting(variant);
            for fit in ["", "whipple_armor", "point_defense_screen", "reflective_plating"] {
                let mut wins = [0u32; 2];
                for seed in 0..200 {
                    let setup = ProjSetup { a: side(1, &[(K::Raider, fit, 1)]),
                        d: vec![(EntityId(2), Ship::new(0, K::Raider, patrol.clone()))],
                        platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
                    wins[0] += u32::from(super::super::v6::simulate_engagement(&setup, seed).a_won);
                    wins[1] += u32::from(simulate_engagement(&setup, seed).a_won);
                }
                eprintln!("patrol {variant} ({}) vs raider[{fit}]: v6 {}/200, v7 {}/200", patrol.key(), wins[0], wins[1]);
            }
        }
        for (name, a, d, raid) in cases {
            let setup = ProjSetup { a: side(100, &a), d: side(500, &d), platform_tiers: 0, raid, a_retreat: None, d_retreat: None };
            let old = super::super::v6::project_distribution(&setup, 7, 32);
            let new = project_distribution(&setup, 7, 32);
            let loss = |m: &std::collections::BTreeMap<ShipKind, u32>| m.values().sum::<u32>();
            eprintln!("{name:40} v6 win {:5.1}% steps {:3} lossA {:2} lossD {:2} | v7 win {:5.1}% steps {:3} lossA {:2} lossD {:2}",
                old.a_win_pct * 100.0, old.median.steps, loss(&old.median.a_losses), loss(&old.median.d_losses),
                new.a_win_pct * 100.0, new.median.steps, loss(&new.median.a_losses), loss(&new.median.d_losses));
        }
    }
}
