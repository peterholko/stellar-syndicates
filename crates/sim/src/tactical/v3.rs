//! Independent, seeded close passes for gun Interceptors and Privateers.
//! Movement is still authoritative and arrives in the existing keyframes;
//! never add client-only dodges or expose a seed that can predict future light.

use super::{Rules, rules_v1::Rng, v1, v2};
use crate::module::{DamageType, Loadout};
use crate::ship::Ship;
use crate::{EntityId, ShipKind, Vec2};
use v1::{Role, TacticalState};
#[cfg(test)]
use v1::{Distribution, ProjSetup, SimOutcome};

const GUN_STANDOFF: f64 = 220.0; // Retain v2's close-fighting center, not weapon reach.
const PASS_MIN_STEPS: u64 = 14;
const PASS_STEP_SPREAD: u64 = 9;
const TURN_BLEND_STEPS: u64 = 6;
const MANEUVER_SALT: u64 = 0x4D41_4E45_5556_4552;

pub(super) fn rules(world_seed: u64, battle_id: u64) -> Rules {
    // Independent stream: steering never spends a targeting/damage/world roll.
    Rules::V3 {
        maneuver_seed: v1::battle_rng(world_seed ^ MANEUVER_SALT, battle_id).next_u64(),
    }
}

impl TacticalState {
    /// Frozen v3 constructor. V4 preserves this maneuver seed and movement for
    /// every hull except the new datalink screen; saved v3 fights remain v3.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_v3(
        world_seed: u64,
        battle_id: u64,
        a: &[(EntityId, Ship)],
        d: &[(EntityId, Ship)],
        platform_tiers: u32,
        platform_pool: f64,
        bearing: Vec2,
    ) -> Self {
        let mut state = Self::open(
            world_seed,
            battle_id,
            a,
            d,
            platform_tiers,
            platform_pool,
            bearing,
        );
        state.rules = rules(world_seed, battle_id);
        state
    }
}

#[derive(Clone, Copy)]
struct Pass {
    radius: f64,
    lead: f64,
}

fn pass(seed: u64, epoch: u64) -> Pass {
    let mut rng = Rng::new(seed ^ epoch.wrapping_mul(0xD1B5_4A32_D192_ED03));
    let sign = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 };
    Pass {
        radius: 0.78 + rng.next_f64() * 0.38,
        lead: sign * (0.7 + rng.next_f64() * 0.4),
    }
}

fn maneuver(seed: u64, cid: u32, step: u64) -> Pass {
    let seed = seed ^ u64::from(cid).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rng = Rng::new(seed);
    let period = PASS_MIN_STEPS + rng.next_u64() % PASS_STEP_SPREAD;
    let clock = step.saturating_add(rng.next_u64() % period);
    let epoch = clock / period;
    let a = pass(seed, epoch);
    let b = pass(seed, epoch + 1);
    // Hold a pass, then ease into the next over six tactical steps. Each hull
    // has its own cadence/phase/radius/turn direction, NOT side/cid parity.
    // No per-frame noise; acceleration and speed caps remain the shared kernel's.
    let t = ((clock % period) as f64 - (period - TURN_BLEND_STEPS) as f64).max(0.0)
        / TURN_BLEND_STEPS as f64;
    let mix = t * t * (3.0 - 2.0 * t);
    Pass {
        radius: a.radius + (b.radius - a.radius) * mix,
        lead: a.lead + (b.lead - a.lead) * mix,
    }
}

pub(super) fn desired_point(state: &TacticalState, i: usize, seed: u64) -> Vec2 {
    let c = &state.combatants[i];
    if c.kind != ShipKind::Raider
        || c.role != Role::Skirmish
        || c.platform
        || super::rules_v1::offense(&Loadout::from_key(&c.stack)).0 == DamageType::Torpedo
    {
        return v2::desired_point(state, i);
    }
    let pass = maneuver(seed, c.cid, state.step);
    let near = state.nearest_enemy(i).unwrap_or(Vec2::ZERO);
    let from = c.pos - near;
    let dir = if from.length() > 1e-9 {
        from.normalized()
    } else {
        state.bearing
    };
    let (s, co) = pass.lead.sin_cos();
    let ahead = Vec2::new(dir.x * co - dir.y * s, dir.x * s + dir.y * co);
    let ring = state.preferred_band(c).min(GUN_STANDOFF) * 0.95 * pass.radius;
    v1::inside_ring_point(near, ahead, ring, if pass.lead >= 0.0 { 1.0 } else { -1.0 })
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
    use std::collections::BTreeSet;
    use v1::SideMods;

    fn fleet(id: u64, kind: ShipKind, fit: &str) -> Vec<(EntityId, Ship)> {
        vec![(EntityId(id), Ship::new(0, kind, Loadout::from_key(fit)))]
    }

    fn duel(seed: u64) -> TacticalState {
        TacticalState::open_v3(
            seed,
            99,
            &fleet(1, ShipKind::Raider, ""),
            &fleet(2, ShipKind::Raider, ""),
            0,
            0.0,
            Vec2::new(1.0, 0.0),
        )
    }

    #[test]
    fn archived_v3_escort_fixture() {
        let a = fleet(1, ShipKind::Raider, "torpedo_rack");
        let mut d = fleet(2, ShipKind::Corvette, "point_defense_screen");
        d.extend(fleet(3, ShipKind::Convoy, ""));
        let mut state = TacticalState::open_v3(98123, 27, &a, &d, 0, 0.0, Vec2::new(0.8, 0.6));
        for _ in 0..40 { state.step(false, [SideMods::default(); 2]); }
        // Captured before datalinks. Do not rebaseline historical rules.
        assert_eq!(crate::tactical::replay::state_checksum(&state), 8_041_580_474_454_380_878);
    }

    #[test]
    fn symmetric_duels_no_longer_fly_mirrored_tracks() {
        let mut openings = BTreeSet::new();
        let mut least_error = f64::INFINITY;
        for seed in 0..32 {
            let mut state = duel(seed);
            state.combatants[0].pos = Vec2::new(-200.0, 0.0);
            state.combatants[1].pos = Vec2::new(200.0, 0.0);
            let mut max_mirror_error = 0.0_f64;
            for step in 0..24 {
                state.step(false, [SideMods::default(); 2]);
                if state.combatants.len() != 2 {
                    break;
                }
                let a = state.combatants[0].pos;
                let b = state.combatants[1].pos;
                max_mirror_error = max_mirror_error.max(Vec2::new(a.x + b.x, a.y - b.y).length());
                if step == 0 {
                    openings.insert((
                        (a.x * 100.0) as i64,
                        (a.y * 100.0) as i64,
                        (b.x * 100.0) as i64,
                        (b.y * 100.0) as i64,
                    ));
                }
            }
            assert!(
                max_mirror_error > 20.0,
                "seed {seed} still flies a mirror duel"
            );
            least_error = least_error.min(max_mirror_error);
        }
        eprintln!(
            "maneuver probe: {} distinct openings / 32 seeds; minimum symmetry break {least_error:.1} arena units (v2: 0)",
            openings.len()
        );
        assert!(
            openings.len() >= 28,
            "different battles should not repeat one approach"
        );
    }

    #[test]
    fn passes_vary_gently_without_spending_combat_rng() {
        let state = duel(47);
        let Rules::V3 { maneuver_seed } = state.rules else {
            panic!("new battles must use v3")
        };
        let untouched = state.clone();
        let mut reversals = 0;
        for cid in 0..12 {
            let mut last = maneuver(maneuver_seed, cid, 0);
            for step in 1..240 {
                let next = maneuver(maneuver_seed, cid, step);
                assert!((0.78..=1.16).contains(&next.radius));
                assert!(next.lead.abs() <= 1.1);
                // Smoothstep derivative ≤ 1.5; even opposite break turns are
                // blended, not fresh left/right dice every tactical step.
                assert!(
                    (next.lead - last.lead).abs() <= 2.2 * 1.5 / TURN_BLEND_STEPS as f64 + 1e-9
                );
                assert!(
                    (next.radius - last.radius).abs()
                        <= 0.38 * 1.5 / TURN_BLEND_STEPS as f64 + 1e-9
                );
                reversals += usize::from(next.lead.signum() != last.lead.signum());
                last = next;
            }
        }
        assert!(
            reversals >= 12,
            "hulls must sometimes break off their initial orbit"
        );
        for i in 0..state.combatants.len() {
            desired_point(&state, i, maneuver_seed);
        }
        assert_eq!(
            state, untouched,
            "sampling a maneuver never advances any RNG or state"
        );
        let legacy = TacticalState::open(
            47,
            99,
            &fleet(1, ShipKind::Raider, ""),
            &fleet(2, ShipKind::Raider, ""),
            0,
            0.0,
            Vec2::new(1.0, 0.0),
        );
        assert_eq!(
            state.rng, legacy.rng,
            "opening a v3 battle spends no extra deployment/weapon rolls"
        );
    }

    #[test]
    fn varied_gun_passes_stay_close_and_other_roles_are_unchanged() {
        let mut distances = Vec::new();
        for seed in 0..32 {
            let mut state = duel(seed);
            for step in 0..60 {
                let before = state.clone();
                state.step(false, [SideMods::default(); 2]);
                for c in &state.combatants {
                    let old = before
                        .combatants
                        .iter()
                        .find(|old| old.cid == c.cid)
                        .unwrap();
                    assert!(
                        c.pos.length() < v1::WITHDRAW_EXIT_RADIUS,
                        "a combat pass is not a withdrawal"
                    );
                    assert!(c.vel.length() <= super::super::rules_v1::max_speed(c.kind) + 1e-9);
                    assert!((c.vel - old.vel).length() <= v1::ACCEL_CAL / c.max_hp.sqrt() + 1e-9);
                }
                if state.combatants.len() != 2 {
                    break;
                }
                if step >= 15 {
                    distances.push((state.combatants[0].pos - state.combatants[1].pos).length());
                }
            }
        }
        distances.sort_by(f64::total_cmp);
        let median = distances[distances.len() / 2];
        eprintln!("v3 median duel separation: {median:.1} arena units (32 seeds)");
        assert!(
            (100.0..v1::DRIVER_RANGE).contains(&median),
            "retain close gun fights"
        );
        for (kind, fit) in [
            (ShipKind::Raider, "torpedo_rack"),
            (ShipKind::Raider, "point_defense_screen"),
            (ShipKind::Corvette, ""),
            (ShipKind::Battleship, ""),
            (ShipKind::Convoy, ""),
        ] {
            let state = TacticalState::open_v3(
                1,
                99,
                &fleet(1, kind, fit),
                &fleet(2, kind, fit),
                1,
                0.0,
                Vec2::new(1.0, 0.0),
            );
            for i in 0..state.combatants.len() {
                assert_eq!(desired_point(&state, i, 10), v2::desired_point(&state, i));
            }
        }
        let mut state = duel(1);
        state.order_withdraw(0);
        assert_eq!(desired_point(&state, 0, 10), v2::desired_point(&state, 0));
    }

    #[test]
    fn current_projection_uses_the_same_maneuver_rules() {
        let setup = ProjSetup {
            a: fleet(1, ShipKind::Raider, ""),
            d: fleet(2, ShipKind::Raider, ""),
            ..Default::default()
        };
        assert_eq!(
            project_distribution(&setup, 55, 1).median,
            simulate_engagement(&setup, 55)
        );
        assert_eq!(
            crate::tactical::simulate_engagement(&setup, 55),
            simulate_engagement(&setup, 55)
        );
        let a = duel(1);
        let b =
            TacticalState::open_v3(1, 100, &setup.a, &setup.d, 0, 0.0, Vec2::new(1.0, 0.0));
        assert_ne!(
            a.rules, b.rules,
            "a new battle gets a different maneuver pattern"
        );
    }
}
