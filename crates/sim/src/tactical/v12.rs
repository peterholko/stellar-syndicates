//! Every gun fires. A hull fires one shot per weapon it carries: each Mass
//! Driver, Torpedo Rack or Prismatic Lance picks its own target, rolls to hit
//! and for damage, and meets armour on its own, instead of one shot carrying
//! the whole stack. A big hull no longer spends its whole broadside on one
//! small hull, and a stacked fit trades an all-or-nothing alpha for a steady
//! stream. A battle line holds no wider than its guns reach: a hull takes a
//! place in the line only where the enemy's centre is still in its weapon
//! range from its slot, and the rest fight on their own. Dreadnoughts carry
//! heavier broadsides (attack weight 15, was 12), and armour blunts more:
//! Reflective Plating takes half of every beam hit (was 35%), Whipple Armor
//! 60% of every driver hit (was 45%). Everything else is v11. New battles
//! open on v13 (`open_current`), which keeps all of this.

use super::rules_v1::{self, hull_affinity};
use super::v1::{
    Combatant, Distribution, ProjSetup, SimOutcome, StepOutcome, TacticalState, Torpedo, BEAM_COOLDOWN, BEAM_RANGE,
    DMG_VAR, DRIVER_COOLDOWN, DRIVER_FALLOFF, DRIVER_RANGE, HIT_DMG_CAL, PROJECTION_BATTLE_ID, TORP_COOLDOWN,
    TORP_LAUNCH_RANGE, to_hit,
};
use super::{v3, v5, v9, v10, v11, Rules};
use crate::doctrine::MissionProfile;
use crate::module::{DamageType, Family, Loadout, ModuleKind};
use crate::ship::{Ship, ShipKind};
use crate::{EntityId, Vec2};
use std::collections::BTreeMap;

/// Reflective Plating blunts this share of every beam hit, light batteries
/// included (scaled by the hull's protection affinity, as before)...
pub const REFLECT_BLUNT: f64 = 0.50;
/// ...and Whipple Armor this share of every driver hit.
pub const WHIPPLE_BLUNT: f64 = 0.60;
/// A battle-line hull's slot sits no further from the line's centre than this
/// share of its own weapon reach. A line holds its shortest band (0.8 of its
/// reach) from the enemy, so from there the enemy's centre is still in range:
/// a line of like hulls is at most 1.2 reaches wide.
pub const LINE_REACH: f64 = 0.6;

impl TacticalState {
    /// Frozen v12 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v12(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v11(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V11 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V12 { maneuver_seed };
        state
    }
}

/// Attack weights: v1's, with the dreadnought's raised to 15 (was 12).
pub fn attack_weight(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Dreadnought => 15.0,
        other => rules_v1::attack_weight(other),
    }
}

/// A hull's guns: how many shots it fires a round and what each carries. One
/// per Mass Driver, Torpedo Rack or Prismatic Lance in the weapon it fires; a
/// stock or point-defense beam is one gun.
pub fn guns(fit: &Loadout) -> (usize, f64) {
    let (family, mult) = v5::offense(fit);
    let count = |kind: ModuleKind| fit.modules().iter().filter(|m| **m == kind).count();
    let copies = match family {
        DamageType::Torpedo => count(ModuleKind::TorpedoRack),
        DamageType::Driver => count(ModuleKind::MassDriver),
        DamageType::Beam => count(ModuleKind::PrismaticLance),
    }.max(1);
    (copies, mult / copies as f64)
}

/// One hull's guns this round, before any target is known.
pub(super) struct Volley<'a> {
    pub family: DamageType,
    /// Shots this round: one per weapon carried.
    pub shots: usize,
    /// What each shot carries: one weapon's offense multiplier.
    pub per_gun: f64,
    /// The hull's attack weight (light guns cut it per target).
    pub aw: f64,
    /// Hull affinity, opening bonus, raid multiplier and research damage,
    /// applied in v11's order.
    pub aff: f64,
    pub opening: f64,
    pub raid: f64,
    pub research: f64,
    /// The hull's weapon reach multiple: drivers fall off beyond it.
    pub reach: f64,
    pub platform: bool,
    /// v13: a capital's guns hit small hulls it tracks poorly at this share
    /// of the usual chance, and its torpedoes lock onto them this often: the
    /// first while they stand off, the second while they tackle (`tackling`,
    /// by combatant index). 1 before v13.
    pub tracking: (f64, f64),
    pub tackling: &'a [bool],
    /// v13: the side's fortune for the battle, a multiple of every hit's
    /// damage (1 before v13).
    pub fortune: f64,
    /// v15: this hull's hits on stations and platforms (the siege class),
    /// and its side's Breaching Ordnance; 1 before v15.
    pub siege: f64,
    pub vs_defenses: f64,
    /// v15: a station hardpoint wears its armour like a ship.
    pub mount_armour: bool,
}

/// Fire combatant `i`'s guns at the in-range candidates (`cid`, v6 target
/// weight, distance, speed, platform). Each shot picks among the candidates
/// still alive, rolls to hit and for damage exactly as v11's single shot did
/// (so a one-gun hull fires as before), and records its own gunfire; each
/// torpedo flies on its own.
pub(super) fn volley(state: &mut TacticalState, i: usize, cands: &[(u32, f64, f64, f64, bool)], v: &Volley<'_>,
    out: &mut StepOutcome) {
    let math = state.math();
    let (side, from, pos, kind) = { let c = &state.combatants[i]; (c.side, c.cid, c.pos, c.kind) };
    let at = v11::candidate_indices(state, cands);
    for _ in 0..v.shots {
        let live: Vec<usize> = (0..cands.len()).filter(|&k| state.combatants[at[k]].hp > 0.0).collect();
        let Some(&last) = live.last() else { break };
        let total: f64 = live.iter().map(|&k| cands[k].1).sum();
        let mut roll = state.rng.next_f64() * total;
        let mut pick = last;
        for &k in &live {
            if roll < cands[k].1 {
                pick = k;
                break;
            }
            roll -= cands[k].1;
        }
        let (to, _, dist, speed, platform) = cands[pick];
        let (mass, target_kind, fit) = {
            let t = &state.combatants[at[pick]];
            (t.max_hp, t.kind, Loadout::from_key(&t.stack))
        };
        let aw = if v.platform { v.aw } else {
            let armed = platform || rules_v1::attack_weight(target_kind) > 0.0;
            v11::gun_weight(kind, v.family, armed.then_some((mass, platform)), v.aw, math)
        };
        let base = v.per_gun * aw * HIT_DMG_CAL * v.aff * v.opening * v.raid * v.research * v.fortune
            * if platform { v.siege * v.vs_defenses } else { 1.0 };
        let armoured = !platform || (v.mount_armour && state.combatants[at[pick]].mount.is_some());
        let track = if !v.platform && !platform && super::v13::tracks_poorly(kind, target_kind) {
            if v.tackling.get(at[pick]).copied().unwrap_or(false) { v.tracking.1 } else { v.tracking.0 }
        } else { 1.0 };
        match v.family {
            DamageType::Torpedo => {
                // v13: a capital's torpedo locks onto a small hull only so often.
                if track < 1.0 && state.rng.next_f64() >= track {
                    continue;
                }
                let dmg = base * state.rng.range(DMG_VAR.0, DMG_VAR.1);
                state.torpedoes.push(Torpedo { side, pos, target: to, dmg });
            }
            DamageType::Beam | DamageType::Driver => {
                let mut hit = to_hit(v.family, mass, speed, math);
                if v.family == DamageType::Driver && dist > DRIVER_RANGE * v.reach {
                    hit *= DRIVER_FALLOFF;
                }
                if track < 1.0 {
                    hit *= track;
                }
                let connects = state.rng.next_f64() < hit;
                let mut dmg = base * state.rng.range(DMG_VAR.0, DMG_VAR.1);
                if armoured {
                    // A hardpoint has no hull affinity: its armour is the module's.
                    let prot = if platform { 1.0 } else { hull_affinity(target_kind, Family::Protection) };
                    if v.family == DamageType::Beam && fit.reflects() {
                        dmg *= 1.0 - (REFLECT_BLUNT * prot).min(0.95);
                    }
                    if v.family == DamageType::Driver && fit.whipples() {
                        dmg *= 1.0 - (WHIPPLE_BLUNT * prot).min(0.95);
                    }
                }
                let damage = if connects { state.apply_damage(to, dmg, out, side) } else { 0.0 };
                out.gunfire.push(crate::combat::KfGunfire { side, from, to, weapon: v.family, damage: damage as f32 });
            }
        }
    }
    let c = &mut state.combatants[i];
    match v.family {
        DamageType::Beam => c.cooldowns.beam = BEAM_COOLDOWN,
        DamageType::Driver => c.cooldowns.driver = DRIVER_COOLDOWN,
        DamageType::Torpedo => c.cooldowns.torpedo = TORP_COOLDOWN,
    }
}

/// How far a hull's weapon reaches (its full-damage range).
fn weapon_reach(state: &TacticalState, c: &Combatant) -> f64 {
    let reach = state.reach_of(c.kind, c.platform);
    match v5::offense(&Loadout::from_key(&c.stack)).0 {
        DamageType::Beam => BEAM_RANGE * reach,
        DamageType::Driver => DRIVER_RANGE * reach,
        DamageType::Torpedo => TORP_LAUNCH_RANGE,
    }
}

/// The members (heaviest first) that hold a battle line. They take its slots
/// in order, alternating out to the left and right as v10 places them; an
/// armed hull whose slot would sit further from the line's centre than
/// [`LINE_REACH`] of its own weapon reach is left out and fights on its own,
/// and the next hull may still fit if it reaches further.
pub(super) fn line_members(state: &TacticalState, members: &[usize]) -> Vec<usize> {
    let math = state.math();
    let r = |i: usize| v9::body_radius(state.combatants[i].kind, false, math);
    let mut kept: Vec<usize> = Vec::with_capacity(members.len());
    // Each side's outermost slot and the radius of the hull in it.
    let mut ends = [(0.0, 0.0); 2];
    for &i in members {
        if kept.is_empty() {
            ends = [(0.0, r(i)); 2];
            kept.push(i);
            continue;
        }
        let side = usize::from(kept.len().is_multiple_of(2));
        let at = ends[side].0 + (ends[side].1 + r(i)) * v10::SLOT_SPACING;
        let armed = rules_v1::attack_weight(state.combatants[i].kind) > 0.0;
        if armed && at > LINE_REACH * weapon_reach(state, &state.combatants[i]) {
            continue;
        }
        ends[side] = (at, r(i));
        kept.push(i);
    }
    kept
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V12 { maneuver_seed }
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
    use crate::ShipKind as K;

    /// One fleet (`id`) of these hulls.
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

    fn open(seed: u64, a: &[(K, &str, u32)], d: &[(K, &str, u32)]) -> TacticalState {
        TacticalState::open_v12(seed, 90, &fleet(1, a), &fleet(2, d), 0, 0.0, Vec2::new(1.0, 0.0))
    }

    #[test]
    fn a_hull_with_one_gun_fires_exactly_as_in_v11() {
        // One weapon each, no armour, no dreadnought, lines narrower than
        // their reach: nothing v12 changes applies, so the fight is v11's to
        // the last bit.
        let a = [(K::Cruiser, "", 2), (K::Raider, "torpedo_rack", 3)];
        let d = [(K::Corvette, "mass_driver", 3), (K::Destroyer, "", 1), (K::Raider, "", 2)];
        let mut new = open(4, &a, &d);
        let mut old = new.clone();
        let Rules::V12 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V11 { maneuver_seed };
        for step in 0..120 {
            let (o, n) = (old.step(false, [SideMods::default(); 2]), new.step(false, [SideMods::default(); 2]));
            assert_eq!(o.gunfire, n.gunfire, "step {step}");
            let mut same = new.clone();
            same.rules = old.rules;
            assert_eq!(same, old, "step {step}");
        }
        assert!(old.combatants.len() < 11, "the fight drew blood");
    }

    #[test]
    fn every_weapon_carried_fires_its_own_shot() {
        assert_eq!(guns(&Loadout::default()), (1, 1.0));
        assert_eq!(guns(&Loadout::from_key("point_defense_screen")), (1, 0.5));
        let drivers = Loadout::from_key("mass_driver+mass_driver+mass_driver+mass_driver");
        assert_eq!(guns(&drivers).0, 4);
        assert!((guns(&drivers).1 - 1.3).abs() < 1e-12);
        let racks = Loadout::from_key("torpedo_rack+torpedo_rack+point_defense_screen");
        assert_eq!(guns(&racks).0, 2);
        assert!((guns(&racks).1 - 1.6).abs() < 1e-12);
        // Every stack carries the same total as v11's single shot.
        for fit in ["mass_driver+mass_driver", "prismatic_lance+prismatic_lance", "torpedo_rack+torpedo_rack+torpedo_rack"] {
            let fit = Loadout::from_key(fit);
            let (n, per) = guns(&fit);
            assert!((n as f64 * per - v5::offense(&fit).1).abs() < 1e-12);
        }
        // A six-driver titan fires six shots a volley, each at its own pick.
        let mut state = open(6, &[(K::Titan, "mass_driver+mass_driver+mass_driver+mass_driver+mass_driver+mass_driver", 1)],
            &[(K::Corvette, "", 12)]);
        let mut volleys = Vec::new();
        let mut targets = std::collections::BTreeSet::new();
        for _ in 0..90 {
            unkillable(&mut state);
            let out = state.step(false, [SideMods::default(); 2]);
            let titan = state.combatants.iter().find(|c| c.kind == K::Titan).unwrap().cid;
            let shots: Vec<_> = out.gunfire.iter().filter(|g| g.from == titan).collect();
            if !shots.is_empty() { volleys.push(shots.len()); }
            targets.extend(shots.iter().map(|g| g.to));
        }
        assert!(!volleys.is_empty() && volleys.iter().all(|&n| n == 6), "{volleys:?}");
        assert!(targets.len() > 3, "the shots spread over the swarm: {targets:?}");
        // A cruiser with four racks launches four torpedoes a volley.
        let mut state = open(7, &[(K::Cruiser, "torpedo_rack+torpedo_rack+torpedo_rack+torpedo_rack", 1)], &[(K::Battleship, "", 1)]);
        let mut launched = Vec::new();
        for _ in 0..40 {
            unkillable(&mut state);
            let before = state.torpedoes.len();
            state.step(false, [SideMods::default(); 2]);
            let ends = state.torpedoes.len() as i64 - before as i64;
            if ends > 0 { launched.push(ends); }
        }
        assert!(launched.contains(&4), "{launched:?}");
    }

    #[test]
    fn a_battle_line_is_no_wider_than_its_guns_reach() {
        let line = |fit: &str, n: u32| {
            let state = open(8, &[(K::Corvette, fit, n)], &[(K::Titan, "", 1)]);
            let members: Vec<usize> = (0..state.combatants.len()).filter(|&i| state.combatants[i].side == 0).collect();
            line_members(&state, &members).len()
        };
        // Corvette drivers reach 280: slots at 0, ±68 and ±136 fit inside
        // 0.6 of it; beams reach 520, so nine fit.
        assert_eq!(line("mass_driver", 20), 5);
        assert_eq!(line("", 20), 9);
        assert_eq!(line("", 4), 4, "a short line keeps everyone");
        // Driver titans can't hold a line at all: the second slot is out of reach.
        let titans = open(8, &[(K::Titan, "mass_driver", 2)], &[(K::Titan, "", 1)]);
        assert_eq!(line_members(&titans, &[0, 1]), vec![0]);
        // Against a lone titan, far more of a corvette swarm gets into range
        // than under v11's line, which stretched far past its reach.
        let in_range = |rules: Option<Rules>| {
            let mut state = open(9, &[(K::Corvette, "mass_driver", 30)], &[(K::Titan, "", 1)]);
            if let Some(r) = rules { state.rules = r; }
            let mut counts = Vec::new();
            for _ in 0..140 {
                unkillable(&mut state);
                state.step(false, [SideMods::default(); 2]);
                let titan = state.combatants.iter().find(|c| c.kind == K::Titan).unwrap().pos;
                counts.push(state.combatants.iter().filter(|c| c.kind == K::Corvette && c.pos.distance(titan) <= DRIVER_RANGE * 0.8).count());
            }
            counts[100..].iter().sum::<usize>() as f64 / 40.0
        };
        let Rules::V12 { maneuver_seed } = open(9, &[], &[]).rules else { unreachable!() };
        let (now, before) = (in_range(None), in_range(Some(Rules::V11 { maneuver_seed })));
        assert!(now > 3.0 * before, "{now} in range now, {before} under v11");
    }

    #[test]
    fn armour_blunts_half_a_beam_and_sixty_percent_of_a_driver() {
        // The same fight against a bare and an armoured target: the dice and
        // the motion are identical, so every hit differs by exactly the blunt
        // (scaled by the cruiser's protection affinity, 1.2).
        let dealt = |gun: &str, armour: &str| {
            let mut state = open(10, &[(K::Cruiser, gun, 1)], &[(K::Cruiser, armour, 1)]);
            let mut sum = 0.0;
            for _ in 0..60 {
                unkillable(&mut state);
                sum += state.step(false, [SideMods::default(); 2]).dealt[0];
            }
            sum
        };
        let beam = dealt("", "reflective_plating") / dealt("", "");
        assert!((beam - (1.0 - REFLECT_BLUNT * 1.2)).abs() < 1e-9, "{beam}");
        let driver = dealt("mass_driver", "whipple_armor") / dealt("mass_driver", "");
        assert!((driver - (1.0 - WHIPPLE_BLUNT * 1.2)).abs() < 1e-9, "{driver}");
        assert_eq!(attack_weight(K::Dreadnought), 15.0);
        for kind in crate::ship::ALL_SHIP_KINDS.into_iter().filter(|k| *k != K::Dreadnought) {
            assert_eq!(attack_weight(kind), rules_v1::attack_weight(kind));
        }
    }

    #[test]
    fn v12_estimates_run_deterministically() {
        let setup = ProjSetup { a: fleet(1, &[(K::Raider, "", 6), (K::Destroyer, "", 2)]),
            d: fleet(2, &[(K::Cruiser, "mass_driver+mass_driver", 3)]),
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
        let first = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, project_distribution(&setup, 17, 8).a_win_pct);
        assert_eq!(simulate_engagement(&setup, 3), simulate_engagement(&setup, 3));
    }
}
