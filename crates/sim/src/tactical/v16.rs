//! Deterministic math. Every sin, cos, atan2 and powf the battle engine
//! evaluates is software (`Math::Soft`, `crate::dmath`), so a battle fights
//! the same in a debug build and a release build, on every platform, and an
//! archived replay rebuilds after a server rebuild or an OS update. Before
//! v16 the platform's libm, as the optimizer rewrote it, made debug and
//! release builds diverge in about half of all fights (see `Math`).
//! Everything else is v15.

use super::v1::{Distribution, ProjSetup, SimOutcome, TacticalState, PROJECTION_BATTLE_ID};
use super::{v3, Rules};
use crate::doctrine::MissionProfile;
use crate::ship::Ship;
use crate::station::MountSpec;
use crate::{EntityId, Vec2};
use std::collections::BTreeMap;

impl TacticalState {
    /// New battles open on the current rules. Saved battles keep the version
    /// they opened with; the frozen constructors stay available for tests.
    pub fn open_current(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v15(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V15 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V16 { maneuver_seed };
        state
    }
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V16 { maneuver_seed }
}

pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    super::v1::simulate_engagement_with_rules(setup, seed, rules(seed))
}

pub fn project_distribution(setup: &ProjSetup, seed: u64, k: u32) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, simulate_engagement)
}

/// Estimation receives only profiles in the observer's arrived picture (v6).
pub fn project_distribution_with_missions(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, None, &[Vec::new(), Vec::new()])
    })
}

/// The estimate for a fight at a world held by side `home` (0 attacker, 1
/// defender), with each side's station hardpoints as the viewer knows them.
pub fn project_distribution_at_world(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>, home: u8, stations: &[Vec<MountSpec>; 2]) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, Some(home), stations)
    })
}
