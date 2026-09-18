//! Local escort fire control. A datalink does not see outside this battle,
//! strengthen beams/armor, or accelerate command-center reports. Old archives
//! never enter these rules; unfitted battles retain v3's arithmetic and dice.

use std::collections::BTreeMap;

use super::{Rules, rules_v1::{attack_weight, hull_affinity}, v1, v3};
use crate::{EntityId, ShipKind, Vec2, module::{Family, Loadout}, ship::Ship};
use v1::{Combatant, Role, SideMods, TacticalState};
#[cfg(test)]
use v1::{Distribution, ProjSetup, SimOutcome};

/// Tunable, battle-local arena units, NOT galaxy su. Ordinary PD still has
/// its 180-unit bubble. A linked screen gets ONE extra attempt/step within
/// 360, prioritizing its charge's incoming torpedoes, then earliest impact.
/// The finite volley makes saturation possible; another link never multiplies
/// this hull's stats or grants immunity to a whole side.
const DATALINK_RADIUS: f64 = v1::PD_RADIUS * 2.0;

fn rules(seed: u64, battle: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, battle) else { unreachable!() };
    Rules::V4 { maneuver_seed }
}

impl TacticalState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_v4(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        platform_tiers: u32, platform_pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v3(seed, battle, a, d, platform_tiers, platform_pool, bearing);
        state.rules = rules(seed, battle);
        state
    }

    pub(crate) fn set_escorts(&mut self, guards: BTreeMap<EntityId, EntityId>) -> bool {
        if !matches!(self.rules, Rules::V4 { .. } | Rules::V5 { .. } | Rules::V6 { .. }) || self.escorts == guards { return false; }
        self.escorts = guards;
        true
    }
}

fn linked(c: &Combatant) -> bool {
    let fit = Loadout::from_key(&c.stack);
    c.kind == ShipKind::Corvette && !c.platform && c.hp > 0.0
        && c.role == Role::Screen && fit.has_pd() && fit.has_datalink()
}

fn charge<'a>(state: &'a TacticalState, escort: &Combatant) -> Option<&'a Combatant> {
    let fleet = escort.origin?.0;
    let named = state.escorts.get(&fleet);
    state.combatants.iter().filter(|c| c.side == escort.side && c.hp > 0.0
        && !c.platform && c.cid != escort.cid
        && if let Some(target) = named { c.origin.is_some_and(|origin| origin.0 == *target) }
            else { attack_weight(c.kind) == 0.0 })
        .min_by(|a, b| {
            // Guard is authoritative. Without one, protect a civilian in this
            // fleet first, then the nearest friendly civilian actually present.
            let same = |c: &Combatant| c.origin.is_some_and(|origin| origin.0 == fleet);
            same(b).cmp(&same(a))
                .then_with(|| escort.pos.distance(a.pos).total_cmp(&escort.pos.distance(b.pos)))
                .then(a.cid.cmp(&b.cid))
        })
}

pub(super) fn desired_point(state: &TacticalState, i: usize, seed: u64) -> Vec2 {
    let escort = &state.combatants[i];
    if linked(escort) && let Some(charge) = charge(state, escort) {
        let threat = state.torpedo_threat_centroid(1 - escort.side)
            .or_else(|| state.nearest_enemy(i)).unwrap_or(charge.pos + state.bearing);
        let axis = threat - charge.pos;
        let direction = if axis.length() > 1e-9 { axis.normalized() } else { state.bearing };
        // Close enough to put the civilian inside even the ordinary PD bubble.
        // No warp/teleport: the existing acceleration and speed caps get us here.
        return charge.pos + direction * (v1::PD_RADIUS * 0.9);
    }
    v3::desired_point(state, i, seed)
}

fn priority_target(state: &TacticalState, escort: &Combatant) -> Option<usize> {
    let charge = charge(state, escort)?;
    if escort.pos.distance(charge.pos) > DATALINK_RADIUS { return None; }
    state.torpedoes.iter().enumerate().filter_map(|(index, t)| {
        if t.side == escort.side { return None; }
        let target = state.combatants.iter().find(|c| c.cid == t.target && c.hp > 0.0 && c.side == escort.side)?;
        // Use exactly the upcoming torpedo movement, not an earlier position.
        let delta = target.pos - t.pos;
        let distance = delta.length();
        let next = t.pos + if distance > 1e-9 { delta.normalized() * v1::TORP_SPEED.min(distance) } else { Vec2::ZERO };
        if escort.pos.distance(next) > DATALINK_RADIUS { return None; }
        let guards_target = target.origin.is_some_and(|origin| Some(origin.0) == charge.origin.map(|p| p.0));
        Some((index, !guards_target, next.distance(target.pos)))
    }).min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)).then(a.0.cmp(&b.0)))
        .map(|entry| entry.0)
}

pub(super) fn intercept_linked(state: &mut TacticalState, mods: [SideMods; 2]) {
    // Stable cid order. Recompute after each successful shot so two escorts
    // never waste their priority volley on an already-intercepted torpedo.
    let escorts: Vec<_> = state.combatants.iter().filter(|c| linked(c)).map(|c| c.cid).collect();
    for cid in escorts {
        let escort = state.combatants.iter().find(|c| c.cid == cid).unwrap();
        let Some(target) = priority_target(state, escort) else { continue; };
        let chance = (v1::PD_ROLL_BASE * hull_affinity(escort.kind, Family::Interception)
            * mods[escort.side as usize].flak()).min(0.95);
        if state.rng.next_f64() < chance { state.torpedoes.remove(target); }
    }
}

#[cfg(test)]
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    v1::simulate_engagement_with_rules(setup, seed, rules(seed, v1::PROJECTION_BATTLE_ID))
}

#[cfg(test)]
pub fn project_distribution(setup: &ProjSetup, base_seed: u64, k: u32) -> Distribution {
    v1::project_distribution_using(setup, base_seed, k, simulate_engagement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::rules_v1::Rng;
    use v1::Torpedo;

    fn hull(id: u64, kind: ShipKind, fit: &str) -> (EntityId, Ship) {
        (EntityId(id), Ship::new(0, kind, Loadout::from_key(fit)))
    }

    fn scene() -> TacticalState {
        let a = [hull(1, ShipKind::Raider, "torpedo_rack")];
        let d = [hull(2, ShipKind::Corvette, "point_defense_screen+escort_datalink"),
            hull(3, ShipKind::Convoy, ""), hull(4, ShipKind::Titan, "")];
        let mut state = TacticalState::open_v4(17, 2, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        for c in &mut state.combatants {
            c.pos = match c.origin.unwrap().0.0 {
                1 => Vec2::new(1200.0, 0.0), 2 => Vec2::new(-160.0, 0.0),
                3 => Vec2::ZERO, _ => Vec2::new(0.0, 70.0),
            };
            c.vel = Vec2::ZERO;
        }
        state.set_escorts(BTreeMap::from([(EntityId(2), EntityId(3))]));
        state
    }

    fn index(state: &TacticalState, fleet: u64) -> usize {
        state.combatants.iter().position(|c| c.origin.unwrap().0 == EntityId(fleet)).unwrap()
    }

    fn torp(state: &TacticalState, fleet: u64, pos: Vec2) -> Torpedo {
        Torpedo { side: 0, target: state.combatants[index(state, fleet)].cid, pos, dmg: 100.0 }
    }

    #[test]
    fn datalink_screens_its_named_fleet_not_the_largest_hull() {
        let mut state = scene();
        let i = index(&state, 2);
        let expected = Vec2::new(v1::PD_RADIUS * 0.9, 0.0);
        assert_eq!(desired_point(&state, i, 1), expected);
        assert!(v3::desired_point(&state, i, 1).distance(expected) > 50.0);
        state.escorts.insert(EntityId(2), EntityId(4));
        assert_eq!(desired_point(&state, i, 1), v3::desired_point(&state, i, 1));
        state.escorts.insert(EntityId(2), EntityId(999));
        assert!(charge(&state, &state.combatants[i]).is_none(), "absent charge has no position to follow");
        state.escorts.insert(EntityId(2), EntityId(1));
        assert!(charge(&state, &state.combatants[i]).is_none(), "never screen an enemy");
        state.escorts.clear();
        assert_eq!(charge(&state, &state.combatants[i]).unwrap().kind, ShipKind::Convoy);
    }

    #[test]
    fn priority_volley_favors_charge_at_extended_reach_and_is_finite() {
        let mut state = scene();
        state.torpedoes = vec![torp(&state, 4, Vec2::new(250.0, 70.0)),
            torp(&state, 3, Vec2::new(300.0, 0.0)), torp(&state, 3, Vec2::new(330.0, 0.0))];
        let escort = &state.combatants[index(&state, 2)];
        assert_eq!(priority_target(&state, escort), Some(1), "charge outranks earlier-impact threat to Titan");
        assert!(escort.pos.distance(Vec2::new(160.0, 0.0)) > v1::PD_RADIUS);
        let seed = (0..100).find(|seed| Rng::new(*seed).next_f64() < 0.95).unwrap();
        state.rng = Rng::new(seed);
        intercept_linked(&mut state, [SideMods { flak_mult: 100.0, ..Default::default() }; 2]);
        assert_eq!(state.torpedoes.len(), 2, "only one priority attempt per fitted hull/step");
        assert_eq!(state.torpedoes[0].pos, Vec2::new(250.0, 70.0));
        assert_eq!(state.torpedoes[1].pos, Vec2::new(330.0, 0.0));
    }

    #[test]
    fn no_remote_unarmed_dead_or_withdrawing_datalink_screen() {
        for case in 0..6 {
            let mut state = scene();
            state.torpedoes.push(torp(&state, 3, Vec2::new(300.0, 0.0)));
            let i = index(&state, 2);
            match case {
                0 => state.combatants[i].stack = "escort_datalink".into(),
                1 => state.combatants[i].hp = 0.0,
                2 => state.combatants[i].role = Role::Withdraw,
                3 => { let j = index(&state, 3); state.combatants[j].pos.x = 1000.0; },
                4 => state.torpedoes[0].pos.x = 1000.0,
                _ => { let j = index(&state, 3); state.combatants[j].hp = 0.0; },
            }
            let before = state.clone();
            intercept_linked(&mut state, [SideMods::default(); 2]);
            assert_eq!(state, before, "case {case}: no effect and not even an RNG draw");
        }
    }

    #[test]
    fn linked_screen_reduces_incoming_charge_torpedoes_without_guaranteeing_safety() {
        let mut stopped = 0;
        for seed in 0..256 {
            let mut state = scene();
            state.rng = Rng::new(seed);
            state.torpedoes.push(torp(&state, 3, Vec2::new(300.0, 0.0)));
            let mut plain = state.clone();
            let i = index(&plain, 2);
            plain.combatants[i].stack = "point_defense_screen".into();
            intercept_linked(&mut plain, [SideMods::default(); 2]);
            intercept_linked(&mut state, [SideMods::default(); 2]);
            assert_eq!(plain.torpedoes.len(), 1, "ordinary PD has no long-range volley");
            stopped += usize::from(state.torpedoes.is_empty());
        }
        eprintln!("escort probe: {stopped}/256 incoming torpedoes stopped at extended range (ordinary PD: 0/256)");
        assert!(stopped > 64 && stopped < 192, "protection is useful but saturable, not immunity");
    }

    #[test]
    fn the_actual_combat_step_runs_linked_steering_and_extended_interception() {
        let mut stopped = 0;
        for seed in 0..128 {
            let mut new = scene();
            let i = index(&new, 2);
            let heavy = index(&new, 4);
            new.combatants[heavy].pos = Vec2::new(0.0, 3000.0);
            new.combatants[i].pos = Vec2::new(162.0, 0.0);
            new.rng = Rng::new(seed);
            new.torpedoes.push(torp(&new, 3, Vec2::new(0.0, 450.0)));
            let mut old = new.clone();
            old.rules = v3::rules(17, 2);
            old.escorts.clear();
            old.step(false, [SideMods::default(); 2]);
            new.step(false, [SideMods::default(); 2]);
            let i = index(&new, 2);
            assert_eq!(new.combatants[i].vel.y, 0.0, "linked escort holds beside its charge");
            assert!(old.combatants[index(&old, 2)].vel.y > 0.0, "legacy screen drifts toward the Titan");
            assert!(old.torpedoes.iter().any(|t| t.dmg == 100.0));
            stopped += usize::from(!new.torpedoes.iter().any(|t| t.dmg == 100.0));
        }
        assert!(stopped > 0 && stopped < 128, "actual step must execute the extra volley, not just its test helper");
    }

    #[test]
    fn unfitted_v4_combat_retains_v3_geometry_damage_and_random_stream() {
        for weapon in ["", "mass_driver", "torpedo_rack", "point_defense_screen"] {
            let a = [hull(1, ShipKind::Raider, weapon)];
            let d = [hull(2, ShipKind::Corvette, "point_defense_screen"), hull(3, ShipKind::Convoy, "")];
            let mut old = TacticalState::open_v3(123, 8, &a, &d, 1, 0.0, Vec2::new(1.0, 0.0));
            let mut new = TacticalState::open_v4(123, 8, &a, &d, 1, 0.0, Vec2::new(1.0, 0.0));
            for _ in 0..60 {
                old.step(false, [SideMods::default(); 2]);
                new.step(false, [SideMods::default(); 2]);
                let mut normalized = new.clone();
                normalized.rules = old.rules;
                assert_eq!(normalized, old, "no datalink: {weapon} combat must not change");
            }
        }
    }
}
