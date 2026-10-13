//! Big guns track small hulls poorly, tacklers worst of all, and every battle
//! has its fortunes. A battleship's, dreadnought's or titan's guns hit hulls
//! lighter than a corvette (interceptors and scouts) at three quarters of the
//! usual chance, and its torpedoes lock onto them three times in four. An
//! interceptor holding one of the side's hulls (tackling it) weaves inside
//! those guns: they hit it at 15% of the usual chance, and torpedoes lock on
//! as rarely, so tacklers live long enough to hold. Destroyer and cruiser
//! escorts track small hulls as before. Each side rolls
//! its fortune once per battle from the battle's private seed, and its
//! weapons deal that multiple of their damage for the whole fight, so a close
//! fight can go either way. Everything else is v12. New battles open on v14
//! (`open_current`), which keeps all of this and fights at worlds.

use super::rules_v1::{hull_mass, Rng};
use super::v1::{Distribution, ProjSetup, SimOutcome, TacticalState, PROJECTION_BATTLE_ID};
use super::{v3, Rules};
use crate::doctrine::MissionProfile;
use crate::ship::{Ship, ShipKind};
use crate::{EntityId, Vec2};
use std::collections::BTreeMap;

/// A capital's guns hit a small hull at this share of the usual chance, and
/// its torpedoes lock onto one this often...
pub const CAPITAL_TRACKING: f64 = 0.75;
/// ...and a small hull that is tackling one of the shooter's side at this.
pub const TACKLER_TRACKING: f64 = 0.15;
/// Small: lighter than this (a corvette's mass).
pub const TRACKING_MASS: f64 = 800.0;
/// Each side's fortune falls evenly between 1 − this and 1 + this: its
/// weapons deal 60–140% of their damage for the whole battle.
pub const FORTUNE: f64 = 0.4;
/// An independent stream: fortune never spends a targeting or damage roll.
const FORTUNE_SALT: u64 = 0x464f_5254_554e_4521;

impl TacticalState {
    /// Frozen v13 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v13(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v12(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V12 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V13 { maneuver_seed };
        state
    }
}

/// A capital's tracking against small hulls: (standing off, tackling).
pub(super) fn tracking() -> (f64, f64) {
    (CAPITAL_TRACKING, TACKLER_TRACKING)
}

/// Battleships and bigger track hulls lighter than a corvette poorly.
pub fn tracks_poorly(shooter: ShipKind, target: ShipKind) -> bool {
    matches!(shooter, ShipKind::Battleship | ShipKind::Dreadnought | ShipKind::Titan) && hull_mass(target) < TRACKING_MASS
}

/// Side `side`'s fortune in the battle with this private maneuver seed: the
/// multiple of their damage its weapons deal, even between 1 − [`FORTUNE`]
/// and 1 + [`FORTUNE`]. Rolled from its own stream, so it is fixed for the
/// battle and costs no other roll.
pub fn fortune(maneuver_seed: u64, side: u8) -> f64 {
    let mut rng = Rng::new(maneuver_seed ^ FORTUNE_SALT ^ (u64::from(side) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    1.0 + FORTUNE * (2.0 * rng.next_f64() - 1.0)
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V13 { maneuver_seed }
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
    use super::super::v1::SideMods;
    use crate::module::Loadout;
    use crate::ShipKind as K;

    fn fleet(id: u64, list: &[(K, &str, u32)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for (kind, fit, n) in list {
            for _ in 0..*n {
                out.push((EntityId(id), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit))));
            }
        }
        out
    }

    fn unkillable(state: &mut TacticalState) {
        for c in &mut state.combatants { c.hp = c.max_hp * 100.0; }
    }

    const DRIVERS: &str = "mass_driver+mass_driver+mass_driver+mass_driver";

    #[test]
    fn capitals_track_small_hulls_poorly_and_tacklers_worst() {
        assert!(tracks_poorly(K::Battleship, K::Raider) && tracks_poorly(K::Titan, K::Scout));
        assert!(!tracks_poorly(K::Cruiser, K::Raider), "escorts track small hulls");
        assert!(!tracks_poorly(K::Dreadnought, K::Corvette), "a corvette is big enough");
        // A stand-up fight: nobody tackles, so a battleship's drivers hit the
        // interceptors on the same rolls as under v12 at three quarters of the
        // chance (every hit now was a hit then; fortune scales the damage).
        let a = fleet(1, &[(K::Raider, "", 10)]);
        let d = fleet(2, &[(K::Battleship, DRIVERS, 1)]);
        let mut new = TacticalState::open_v13(4, 91, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        let mut old = new.clone();
        let Rules::V13 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V12 { maneuver_seed };
        let hits = |out: &super::super::v1::StepOutcome| -> Vec<(u32, u32)> {
            out.gunfire.iter().filter(|g| g.side == 1 && g.damage > 0.0).map(|g| (g.from, g.to)).collect()
        };
        let (mut hits_new, mut hits_old) = (0, 0);
        for step in 0..300 {
            unkillable(&mut new);
            unkillable(&mut old);
            let (n, o) = (hits(&new.step(false, [SideMods::default(); 2])), hits(&old.step(false, [SideMods::default(); 2])));
            assert!(n.iter().all(|h| o.contains(h)), "step {step}: {n:?} ⊄ {o:?}");
            (hits_new, hits_old) = (hits_new + n.len(), hits_old + o.len());
        }
        let share = hits_new as f64 / hits_old as f64;
        assert!(hits_old > 20 && (0.55..0.95).contains(&share), "{hits_new} of {hits_old}");
        // Two battleships run from twenty interceptors (about their cost).
        // Under v12 their separate driver shots clear the tacklers and they
        // get away; now the tacklers holding them are inside their guns, and
        // most of them live.
        let run = |v12: bool| {
            let a = fleet(1, &[(K::Raider, "", 20)]);
            let d = fleet(2, &[(K::Battleship, DRIVERS, 2)]);
            let mut state = if v12 { TacticalState::open_v12(4, 91, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0)) }
                else { TacticalState::open_v13(4, 91, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0)) };
            state.order_withdraw(1);
            for _ in 0..360 {
                state.step(false, [SideMods::default(); 2]);
            }
            let out = state.combatants.iter().filter(|c| c.side == 1 && c.pos.length() >= super::super::v1::WITHDRAW_EXIT_RADIUS).count();
            (out, state.alive(0))
        };
        let (gone, tacklers) = run(true);
        assert_eq!((gone, tacklers), (2, 0), "v12: the battleships kill the tacklers and leave");
        let (gone, tacklers) = run(false);
        assert_eq!(gone, 0, "v13: still held ({tacklers} tacklers alive)");
        assert!(tacklers >= 10, "most tacklers live: {tacklers}");
    }

    #[test]
    fn fortune_is_fixed_for_the_battle_and_scales_every_hit() {
        let rolls: Vec<f64> = (0..2_000u64).map(|s| fortune(s.wrapping_mul(0x2545_F491_4F6C_DD1D), (s % 2) as u8)).collect();
        assert!(rolls.iter().all(|f| (1.0 - FORTUNE..=1.0 + FORTUNE).contains(f)));
        let mean = rolls.iter().sum::<f64>() / rolls.len() as f64;
        assert!((mean - 1.0).abs() < 0.02, "even around 1: {mean}");
        assert!(rolls.iter().any(|f| *f < 1.0 - 0.9 * FORTUNE) && rolls.iter().any(|f| *f > 1.0 + 0.9 * FORTUNE));
        assert_eq!(fortune(77, 0), fortune(77, 0));
        assert_ne!(fortune(77, 0), fortune(77, 1), "each side rolls its own");
        // The same fight under v12 and v13: no small hulls, nobody dies, so
        // every roll and every move is identical and each side's damage
        // differs by exactly its fortune (guns, batteries and torpedoes).
        let a = fleet(1, &[(K::Cruiser, "", 2), (K::Destroyer, "", 2)]);
        let d = fleet(2, &[(K::Battleship, DRIVERS, 1), (K::Cruiser, "torpedo_rack+torpedo_rack", 1)]);
        let mut new = TacticalState::open_v13(5, 92, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        let mut old = new.clone();
        let Rules::V13 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V12 { maneuver_seed };
        let (mut dealt_new, mut dealt_old) = ([0.0; 2], [0.0; 2]);
        for _ in 0..90 {
            unkillable(&mut new);
            unkillable(&mut old);
            let (n, o) = (new.step(false, [SideMods::default(); 2]), old.step(false, [SideMods::default(); 2]));
            for s in 0..2 { dealt_new[s] += n.dealt[s]; dealt_old[s] += o.dealt[s]; }
            assert_eq!(new.combatants.iter().map(|c| c.pos).collect::<Vec<_>>(), old.combatants.iter().map(|c| c.pos).collect::<Vec<_>>());
        }
        for side in 0..2u8 {
            let s = usize::from(side);
            assert!(dealt_old[s] > 0.0);
            assert!((dealt_new[s] / dealt_old[s] - fortune(maneuver_seed, side)).abs() < 1e-9, "side {side}");
        }
    }

    #[test]
    fn v13_estimates_run_deterministically() {
        let setup = ProjSetup { a: fleet(1, &[(K::Raider, "", 6), (K::Destroyer, "", 2)]),
            d: fleet(2, &[(K::Battleship, DRIVERS, 1)]),
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
        let first = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, project_distribution(&setup, 17, 8).a_win_pct);
        assert_eq!(simulate_engagement(&setup, 3), simulate_engagement(&setup, 3));
    }
}
