//! Big ships keep maneuvering. A line hull (corvette, destroyer, cruiser) that
//! holds its firing band and a capital anchoring the line (battleship and up)
//! no longer park: each flies a small circle at about half its top speed. The
//! to-hit roll reads a target's CURRENT speed, so a parked hull was the easiest
//! target on the field. The circle is as tight as the hull can turn while
//! keeping most of its acceleration in reserve. Civilians, platforms, screens,
//! escorts, skirmishers and anything withdrawing move exactly as in v7, and
//! every weapon, roll and range is v7's. Frozen: new battles open on v9
//! (`open_current`), which keeps this weave.

use super::rules_v1::{max_speed, Rng};
use super::v1::{
    Distribution, ProjSetup, Role, SimOutcome, TacticalState, ACCEL_CAL, ARENA_RADIUS, ARENA_SCALE,
    PROJECTION_BATTLE_ID,
};
use super::{v3, v6, Rules};
use crate::{doctrine::MissionProfile, ship::Ship, EntityId, Vec2};
use std::collections::BTreeMap;

/// Weave speed as a fraction of the hull's top speed.
pub(super) const WEAVE_SPEED: f64 = 0.5;
/// Share of the hull's acceleration spent turning on its circle; the rest
/// keeps it on station as the line shifts.
const WEAVE_TURN: f64 = 0.6;
/// The kernel arrives at `dist / 1.5` per step, so leading the circle by this
/// many steps holds a hull on it at the weave speed.
const LEAD_STEPS: f64 = 1.5;
/// An independent stream: weaving never spends a targeting or damage roll.
const WEAVE_SALT: u64 = 0x5745_4156_4553_4950;

impl TacticalState {
    /// Frozen v8 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v8(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v7(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V7 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V8 { maneuver_seed };
        state
    }
}

/// One hull's weave: turn rate (radians per step), circle radius (arena
/// units; rate × radius is the weave speed), and its own phase and turning
/// direction.
struct Weave {
    rate: f64,
    radius: f64,
    phase: f64,
    sign: f64,
    math: super::Math,
}

impl Weave {
    fn of(state: &TacticalState, i: usize, seed: u64) -> Self {
        let c = &state.combatants[i];
        let speed = WEAVE_SPEED * max_speed(c.kind) * ARENA_SCALE;
        let accel = ACCEL_CAL / c.max_hp.max(1.0).sqrt();
        let mut rate = WEAVE_TURN * accel / speed.max(1e-9);
        // v9 hulls heavier than a destroyer come about slowly: a wider circle.
        if matches!(state.rules, Rules::V9 { .. } | Rules::V10 { .. } | Rules::V11 { .. } | Rules::V12 { .. } | Rules::V13 { .. } | Rules::V14 { .. } | Rules::V15 { .. } | Rules::V16 { .. }) && let Some(cap) = super::v9::turn_rate(c.kind) {
            rate = rate.min(0.8 * cap);
        }
        let mut rng = Rng::new(seed ^ WEAVE_SALT ^ u64::from(c.cid).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let phase = rng.next_f64() * std::f64::consts::TAU;
        let sign = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 };
        Self { rate, radius: speed / rate, phase, sign, math: state.math() }
    }

    /// Unit vector from the circle's centre to the hull, `ahead` steps from now.
    fn at(&self, step: u64, ahead: f64) -> Vec2 {
        let th = self.phase + self.sign * self.rate * (step as f64 + ahead);
        match self.math {
            super::Math::Platform => Vec2::new(th.cos(), th.sin()),
            super::Math::Soft => crate::dmath::polar(th, 1.0),
        }
    }
}

pub(super) fn desired_point(state: &TacticalState, i: usize, seed: u64) -> Vec2 {
    let base = v6::desired_point(state, i, seed);
    let c = &state.combatants[i];
    if c.platform || state.withdrawing[usize::from(c.side.min(1))] || c.kind.attack_weight() <= 0.0 {
        return base;
    }
    match c.role {
        // v1's line hull holds exactly where it reached its band.
        Role::Line if base == c.pos => line_weave(state, i, seed),
        // A capital on its plain anchor (not escorting, screening or leaving).
        Role::Anchor if base == state.desired_point(i) => {
            let w = Weave::of(state, i, seed);
            inside_arena(base + w.at(state.step, LEAD_STEPS) * w.radius)
        }
        _ => base,
    }
}

/// A line hull circles where it holds, the circle's centre kept a little
/// inside its firing band so the whole circle stays in range.
fn line_weave(state: &TacticalState, i: usize, seed: u64) -> Vec2 {
    let c = &state.combatants[i];
    let w = Weave::of(state, i, seed);
    let mut centre = c.pos - w.at(state.step, 0.0) * w.radius;
    let near = state.nearest_enemy(i).unwrap_or(Vec2::ZERO);
    let band = state.preferred_band(c);
    let out = centre - near;
    let d = out.length();
    if d > 1e-9 {
        centre = near + out * ((band - w.radius * 0.5).max(band * 0.6) / d);
    }
    inside_arena(centre + w.at(state.step, LEAD_STEPS) * w.radius)
}

fn inside_arena(p: Vec2) -> Vec2 {
    let cap = ARENA_RADIUS * 0.95;
    let r = p.length();
    if r > cap { p * (cap / r) } else { p }
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V8 { maneuver_seed }
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
    use super::super::v1::{SideMods, BEAM_RANGE};
    use crate::module::Loadout;
    use crate::ShipKind;

    fn hull(id: u64, kind: ShipKind, fit: &str) -> (EntityId, Ship) {
        (EntityId(id), Ship::new(0, kind, Loadout::from_key(fit)))
    }

    /// Attackers `a` against one stationary defense platform at the origin.
    fn versus_platform(a: &[(EntityId, Ship)], current: bool) -> TacticalState {
        if current {
            TacticalState::open_v8(5, 40, a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        } else {
            TacticalState::open_v7(5, 40, a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        }
    }

    /// Run `steps`, keeping every hull alive (the test is about motion), and
    /// return the speed and distance-from-`centre` of hull `cid` each step.
    fn track(state: &mut TacticalState, cid: u32, steps: usize, centre: impl Fn(&TacticalState) -> Vec2) -> Vec<(f64, f64)> {
        (0..steps).map(|_| {
            for c in &mut state.combatants { c.hp = c.max_hp; }
            state.step(false, [SideMods::default(); 2]);
            let c = state.combatants.iter().find(|c| c.cid == cid).unwrap();
            (c.vel.length(), (c.pos - centre(state)).length())
        }).collect()
    }

    #[test]
    fn a_line_hull_in_band_keeps_weaving_at_about_half_speed_and_in_range() {
        let mut state = versus_platform(&[hull(1, ShipKind::Cruiser, "")], true);
        let cid = state.combatants.iter().find(|c| !c.platform).unwrap().cid;
        let band = state.preferred_band(state.combatants.iter().find(|c| c.cid == cid).unwrap());
        let late = track(&mut state, cid, 140, |_| Vec2::ZERO).split_off(60);
        let mean = late.iter().map(|s| s.0).sum::<f64>() / late.len() as f64;
        let top = ShipKind::Cruiser.max_speed();
        assert!(mean > 0.4 * top && mean < 0.6 * top, "weaves near half speed: {mean}");
        assert!(late.iter().all(|s| s.0 > 0.25 * top), "never parks");
        assert!(late.iter().all(|s| s.1 <= band * 1.15 && s.1 >= band * 0.5), "stays in its firing band");
        let mut v7 = versus_platform(&[hull(1, ShipKind::Cruiser, "")], false);
        let parked = track(&mut v7, cid, 140, |_| Vec2::ZERO).split_off(60);
        assert!(parked.iter().all(|s| s.0 < 1e-6), "v7 parked it");
    }

    #[test]
    fn capitals_circle_their_anchor_and_civilians_still_park() {
        let a = [hull(1, ShipKind::Titan, ""), hull(2, ShipKind::Battleship, ""), hull(3, ShipKind::Convoy, "")];
        let state = versus_platform(&a, true);
        let rally = state.bearing * (BEAM_RANGE * 0.85);
        let ids: Vec<(u32, ShipKind)> = state.combatants.iter().filter(|c| !c.platform).map(|c| (c.cid, c.kind)).collect();
        for (cid, kind) in ids {
            let late = track(&mut state.clone(), cid, 220, |_| rally).split_off(140);
            let mean = late.iter().map(|s| s.0).sum::<f64>() / late.len() as f64;
            if kind == ShipKind::Convoy {
                assert!(mean < 1.0, "a convoy still tucks in and parks: {mean}");
            } else {
                let top = kind.max_speed();
                assert!(mean > 0.4 * top && mean < 0.6 * top, "{kind:?} weaves near half speed: {mean}");
                assert!(late.iter().all(|s| s.1 < 140.0), "{kind:?} stays on its anchor");
            }
        }
    }

    #[test]
    fn weave_circles_fit_each_hull_and_its_viewer_turn_rate() {
        // Radius (arena units) and turn rate (rad per step) per warship class.
        let state = versus_platform(&[hull(1, ShipKind::Corvette, ""), hull(2, ShipKind::Destroyer, ""),
            hull(3, ShipKind::Cruiser, ""), hull(4, ShipKind::Battleship, ""), hull(5, ShipKind::Dreadnought, ""),
            hull(6, ShipKind::Titan, "")], true);
        for (i, c) in state.combatants.iter().enumerate().filter(|(_, c)| !c.platform) {
            let w = Weave::of(&state, i, 9);
            assert!(w.radius > 50.0 && w.radius < 110.0, "{:?} circle radius {}", c.kind, w.radius);
            assert!(w.rate < 0.42, "{:?} turns {} rad a step", c.kind, w.rate);
            assert!((w.rate * w.radius - WEAVE_SPEED * c.kind.max_speed()).abs() < 1e-9);
        }
    }

    #[test]
    fn v8_battles_open_on_v8_and_saved_v7_battles_keep_their_rules() {
        let a = [hull(1, ShipKind::Cruiser, ""), hull(2, ShipKind::Battleship, "")];
        let current = versus_platform(&a, true);
        assert_eq!(current.rules_version(), 8);
        let restored: TacticalState = serde_json::from_slice(&serde_json::to_vec(&current).unwrap()).unwrap();
        assert_eq!(restored, current);
        let old = versus_platform(&a, false);
        let mut saved: TacticalState = serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(saved.rules_version(), 7);
        let mut live = old.clone();
        for _ in 0..60 {
            live.step(false, [SideMods::default(); 2]);
            saved.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(saved, live, "a saved v7 fight replays exactly");
        let mut again = current.clone();
        let mut first = current;
        for _ in 0..60 {
            first.step(false, [SideMods::default(); 2]);
            again.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(first, again, "the weave is deterministic");
    }

    #[test]
    fn v8_estimates_run_deterministically() {
        let setup = ProjSetup {
            a: (0..6).map(|i| hull(10 + i, ShipKind::Raider, "")).collect(),
            d: vec![hull(30, ShipKind::Battleship, "")],
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None,
        };
        let first = project_distribution(&setup, 17, 8);
        let again = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, again.a_win_pct);
        assert_eq!(first.median.steps, again.median.steps);
        assert_eq!(simulate_engagement(&setup, 3).steps, simulate_engagement(&setup, 3).steps);
    }
}

/// Manual balance comparison against the frozen v7 rules (prints, asserts
/// nothing): `cargo test -p sim --lib balance_probe_v8 -- --ignored --nocapture`.
#[cfg(test)]
mod balance_probe_v8 {
    use super::*;
    use crate::module::Loadout;
    use crate::ShipKind;

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
    fn print_v7_vs_v8_balance() {
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
            ("6 torp raider+4 torp destroyer v 2 cruiser+dread", vec![(K::Raider, "torpedo_rack", 6), (K::Destroyer, "torpedo_rack", 4)], vec![(K::Cruiser, "", 2), (K::Dreadnought, "", 1)], false),
            ("2 dread v 12 raider+6 corvette", vec![(K::Dreadnought, "", 2)], vec![(K::Raider, "", 12), (K::Corvette, "", 6)], false),
            ("titan+4 corv v 3 battleship", vec![(K::Titan, "", 1), (K::Corvette, "", 4)], vec![(K::Battleship, "", 3)], false),
            ("8 driver raider v 2 cruiser", vec![(K::Raider, "mass_driver", 8)], vec![(K::Cruiser, "", 2)], false),
            ("4 cruiser v 8 corvette", vec![(K::Cruiser, "", 4)], vec![(K::Corvette, "", 8)], false),
        ];
        for (name, a, d, raid) in cases {
            let setup = ProjSetup { a: side(100, &a), d: side(500, &d), platform_tiers: 0, raid, a_retreat: None, d_retreat: None };
            let old = super::super::v7::project_distribution(&setup, 7, 32);
            let new = project_distribution(&setup, 7, 32);
            let loss = |m: &BTreeMap<ShipKind, u32>| m.values().sum::<u32>();
            eprintln!("{name:48} v7 win {:5.1}% steps {:3} lossA {:2} lossD {:2} | v8 win {:5.1}% steps {:3} lossA {:2} lossD {:2}",
                old.a_win_pct * 100.0, old.median.steps, loss(&old.median.a_losses), loss(&old.median.d_losses),
                new.a_win_pct * 100.0, new.median.steps, loss(&new.median.a_losses), loss(&new.median.d_losses));
        }
    }
}
