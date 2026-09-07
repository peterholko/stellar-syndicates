//! Close-pass gun Interceptors. Weapon reach, damage, cooldowns, deployment,
//! and all other roles keep v1's rules; only these skirmish maneuver targets
//! change. The selected rules live in the saved state and private replay.

use super::v1;
use crate::module::{DamageType, Loadout};
use crate::{ShipKind, Vec2};
use v1::{Role, TacticalState};
#[cfg(test)]
use v1::{Distribution, ProjSetup, SimOutcome};

/// Preferred gun-fighting distance, in battle-local arena units (not galaxy
/// su). The inherited 0.95 orbit factor makes the desired ring ~209 units,
/// down from ~525 for stock beams. Torpedo boats still keep their long range.
const INTERCEPTOR_GUN_STANDOFF: f64 = 220.0;
/// A larger lead on the smaller circle preserves fast circling, rather than
/// making Interceptors slow to a crawl at each nearby maneuver point.
const CLOSE_PASS_LEAD_RAD: f64 = 0.8;

pub(super) fn desired_point(state: &TacticalState, i: usize) -> Vec2 {
    let c = &state.combatants[i];
    let weapon = super::rules_v1::offense(&Loadout::from_key(&c.stack)).0;
    if c.kind != ShipKind::Raider
        || c.role != Role::Skirmish
        || c.platform
        || weapon == DamageType::Torpedo
    {
        return state.desired_point(i);
    }
    // Corporate Interceptors and gun-equipped Privateers share the light
    // skirmisher class. Maximum firing range stays intact: they may shoot on
    // approach, but then commit to close passes instead of circling at reach.
    let near = state.nearest_enemy(i).unwrap_or(Vec2::ZERO);
    let from = c.pos - near;
    let dir = if from.length() > 1e-9 {
        from.normalized()
    } else {
        state.bearing
    };
    let sign = if c.cid.is_multiple_of(2) { 1.0 } else { -1.0 };
    let (s, co) = (CLOSE_PASS_LEAD_RAD * sign).sin_cos();
    let ahead = Vec2::new(dir.x * co - dir.y * s, dir.x * s + dir.y * co);
    let band = state.preferred_band(c).min(INTERCEPTOR_GUN_STANDOFF);
    v1::inside_ring_point(near, ahead, band * 0.95, sign)
}

#[cfg(test)]
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    v1::simulate_engagement_with_rules(setup, seed, super::Rules::V2)
}

#[cfg(test)]
pub fn project_distribution(setup: &ProjSetup, base_seed: u64, k: u32) -> Distribution {
    v1::project_distribution_using(setup, base_seed, k, simulate_engagement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityId, ship::Ship};
    use v1::SideMods;

    fn fleet(id: u64, kind: ShipKind, fit: &str) -> Vec<(EntityId, Ship)> {
        vec![(EntityId(id), Ship::new(0, kind, Loadout::from_key(fit)))]
    }

    fn duel(seed: u64, current: bool, kind: ShipKind, fit: &str) -> TacticalState {
        let a = fleet(1, kind, fit);
        let d = fleet(2, kind, fit);
        let mut state = TacticalState::open(seed, 99, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        if current { state.rules = crate::tactical::Rules::V2; }
        state
    }

    #[test]
    fn archived_v2_duels_keep_their_exact_movement() {
        let mut state = duel(0, true, ShipKind::Raider, "");
        for _ in 0..60 {
            state.step(false, [SideMods::default(); 2]);
            if state.combatants.len() != 2 { break; }
        }
        // Captured before v3. Do not rebaseline archived battle geometry/dice.
        assert_eq!(crate::tactical::replay::state_checksum(&state), 3_527_743_176_992_133_831);
    }

    #[test]
    fn gun_interceptors_choose_close_passes_without_shortening_weapons() {
        for fit in ["", "mass_driver"] {
            let mut state = duel(0, true, ShipKind::Raider, fit);
            state.combatants[0].pos = Vec2::new(-300.0, 0.0);
            state.combatants[1].pos = Vec2::ZERO;
            let target = desired_point(&state, 0);
            assert!((target.length() - INTERCEPTOR_GUN_STANDOFF * 0.95).abs() < 1e-9);
            assert!(state.desired_point(0).length() > target.length());
        }
        // Torpedo boats, PD escorts and non-Interceptor hulls retain exactly
        // the old targets, not merely targets somewhere within weapon range.
        for (kind, fit) in [
            (ShipKind::Raider, "torpedo_rack"),
            (ShipKind::Raider, "point_defense_screen"),
            (ShipKind::Corvette, ""),
            (ShipKind::Battleship, ""),
        ] {
            let state = duel(0, true, kind, fit);
            for i in 0..state.combatants.len() {
                assert_eq!(desired_point(&state, i), state.desired_point(i));
            }
        }
    }

    #[test]
    fn interceptor_duels_actually_close_in_the_recorded_geometry() {
        let median = |current| {
            let mut distances = Vec::new();
            for seed in 0..16 {
                let mut state = duel(seed, current, ShipKind::Raider, "");
                for step in 0..60 {
                    state.step(false, [SideMods::default(); 2]);
                    if state.combatants.len() != 2 {
                        break;
                    }
                    if step >= 15 {
                        distances
                            .push((state.combatants[0].pos - state.combatants[1].pos).length());
                    }
                }
            }
            assert!(
                !distances.is_empty(),
                "duels must reach the close-pass phase"
            );
            distances.sort_by(f64::total_cmp);
            distances[distances.len() / 2]
        };
        let old = median(false);
        let current = median(true);
        eprintln!(
            "Interceptor duel separation: v1 {old:.1} → v2 {current:.1} battle units (16 seeds)"
        );
        assert!(
            current < old * 0.65,
            "new passes must be substantially closer: {current} vs {old}"
        );
    }

    #[test]
    fn old_untagged_battles_keep_their_gun_maneuvers() {
        let initial = duel(0, false, ShipKind::Raider, "");
        let value = serde_json::to_value(&initial).unwrap();
        assert!(
            value.get("rules").is_none(),
            "legacy JSON shape is unchanged"
        );
        let mut old: TacticalState = serde_json::from_value(value).unwrap();
        assert_eq!(old.rules_version(), 1);
        for _ in 0..60 {
            old.step(false, [SideMods::default(); 2]);
            if old.combatants.len() != 2 {
                break;
            }
        }
        // Captured before v2 existed. Never rebaseline archived battle truth.
        assert_eq!(
            crate::tactical::replay::state_checksum(&old),
            3_397_473_367_274_835_693
        );
        let current = duel(0, true, ShipKind::Raider, "");
        let resumed: TacticalState =
            serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
        assert_eq!(resumed, current);
        assert_eq!(resumed.rules_version(), 2);
    }

    #[test]
    fn frozen_v2_projection_keeps_close_pass_rules() {
        let setup = ProjSetup {
            a: fleet(1, ShipKind::Raider, ""),
            d: fleet(2, ShipKind::Raider, ""),
            ..Default::default()
        };
        let distribution = project_distribution(&setup, 55, 1);
        assert_eq!(distribution.median, simulate_engagement(&setup, 55));
        assert!(
            (0..16)
                .any(|seed| simulate_engagement(&setup, seed)
                    != v1::simulate_engagement(&setup, seed))
        );
    }
}
