//! Magazines. A player fleet brings its munitions into the fight (rules v17,
//! §shipbuilding-overhaul Phase 5, `crate::munitions`): every torpedo a rack
//! launches and every slug a driver fires leaves the fleet's magazine, and a
//! hull whose magazine is dry goes silent on that weapon and falls back to
//! its kit beams. Beam guns wear their crystals a little with every shot;
//! worn crystals hit softer, down to [`crate::munitions::CRYSTAL_FLOOR`] of
//! fresh, never nothing. A fleet the battle knows no magazine for — a pirate
//! pack, the Authority, a platform, every battle opened before v17 — fights
//! exactly as v16. Everything else is v16.

use super::v1::{Distribution, ProjSetup, SimOutcome, TacticalState, PROJECTION_BATTLE_ID};
use super::{v3, Rules};
use crate::doctrine::MissionProfile;
use crate::module::{DamageType, Loadout};
use crate::munitions::Magazine;
use crate::ship::{Ship, ShipKind};
use crate::station::MountSpec;
use crate::{EntityId, Vec2};
use std::collections::BTreeMap;

impl TacticalState {
    /// New battles open on the current rules. Saved battles keep the version
    /// they opened with; the frozen constructors stay available for tests.
    pub fn open_current(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v16(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V16 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V17 { maneuver_seed };
        state
    }

    /// The fleets' magazines as the world reports them this tick. An input
    /// like a roster change: recorded, so a replay draws the same rounds.
    /// Returns whether anything changed.
    pub(crate) fn set_magazines(&mut self, magazines: BTreeMap<EntityId, Magazine>) -> bool {
        if !matches!(self.rules, Rules::V17 { .. }) || self.magazines == magazines {
            return false;
        }
        self.magazines = magazines;
        true
    }

    /// What every magazine holds after the rounds so far: the world writes it
    /// back onto the fleets beside the hull damage.
    pub fn magazines(&self) -> &BTreeMap<EntityId, Magazine> {
        &self.magazines
    }

    /// The magazine combatant `i` draws on: its fleet's, when this battle
    /// carries one (v17, a player fleet). Platforms and hardpoints never do.
    pub(super) fn magazine_of(&self, i: usize) -> Option<EntityId> {
        if !matches!(self.rules, Rules::V17 { .. }) {
            return None;
        }
        let c = &self.combatants[i];
        if c.platform {
            return None;
        }
        c.origin.map(|(fleet, _)| fleet).filter(|fleet| self.magazines.contains_key(fleet))
    }

    /// No round left for `family` in this magazine. Beams are never dry.
    pub(super) fn magazine_dry(&self, magazine: Option<EntityId>, family: DamageType) -> bool {
        magazine.and_then(|f| self.magazines.get(&f)).is_some_and(|m| m.dry(family))
    }

    /// One shot of `family` from a hull with `guns` beam guns: a torpedo or
    /// slug leaves the magazine, the beams wear their crystals. False when the
    /// rack or driver had no round (the volley stops). Always true with no
    /// magazine.
    pub(super) fn fire_round(&mut self, magazine: Option<EntityId>, family: DamageType, guns: f64) -> bool {
        match magazine.and_then(|f| self.magazines.get_mut(&f)) {
            Some(m) => m.fire(family, guns),
            None => true,
        }
    }

    /// The beams' damage multiple from the magazine's crystals; 1 with none.
    pub(super) fn crystal_factor(&self, magazine: Option<EntityId>) -> f64 {
        magazine.and_then(|f| self.magazines.get(&f)).map_or(1.0, Magazine::crystal_factor)
    }
}

/// The beam guns a hull fires with: its kit's Pulse Lasers and Beam Batteries
/// plus any beam module (`crate::munitions::beam_guns`), as the wear on one
/// shot is counted. Platforms carry none.
pub(super) fn beam_guns(kind: ShipKind, fit: &Loadout) -> f64 {
    crate::munitions::beam_guns(kind, fit)
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V17 { maneuver_seed }
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::v1::SideMods;
    use crate::module::ModuleKind;

    fn ships(fleet: u64, kind: ShipKind, fit: &[ModuleKind], n: u32) -> Vec<(EntityId, Ship)> {
        (0..n).map(|i| (EntityId(fleet), Ship::new(i, kind, Loadout::new(fit.to_vec())))).collect()
    }

    fn run(state: &mut TacticalState, steps: usize) -> (f64, usize, usize) {
        let (mut dealt, mut beams, mut torps) = (0.0, 0, 0);
        for _ in 0..steps {
            let out = state.step(false, [SideMods::default(), SideMods::default()]);
            dealt += out.dealt[0];
            beams += out.gunfire.iter().filter(|g| g.side == 0 && g.weapon == DamageType::Beam).count();
            torps += out.torp_ends.iter().filter(|t| t.side == 0).count();
            if state.combatants.iter().filter(|c| c.side == 1 && c.hp > 0.0).count() == 0 { break; }
        }
        (dealt, beams, torps)
    }

    #[test]
    fn without_magazines_v17_fights_as_v16() {
        let a = ships(1, ShipKind::Destroyer, &[ModuleKind::TorpedoRack], 2);
        let d = ships(2, ShipKind::Corvette, &[], 4);
        let mut v16 = TacticalState::open_v16(5, 9, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        let mut v17 = TacticalState::open_current(5, 9, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        assert_eq!(v17.rules_version(), 17);
        for _ in 0..60 {
            v16.step(false, [SideMods::default(), SideMods::default()]);
            v17.step(false, [SideMods::default(), SideMods::default()]);
        }
        assert_eq!(v16.hp_writeback(), v17.hp_writeback(), "no magazine, no difference");
    }

    #[test]
    fn a_dry_rack_falls_back_to_the_kit_beams() {
        let a = ships(1, ShipKind::Destroyer, &[ModuleKind::TorpedoRack], 1);
        let d = ships(2, ShipKind::Corvette, &[], 3);
        let mut dry = TacticalState::open_current(3, 11, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        assert!(dry.set_magazines(BTreeMap::from([(EntityId(1),
            Magazine { torpedoes: 0.0, slugs: 0.0, crystals: 2.0, crystal_capacity: 2.0 })])));
        let (dealt, beams, torps) = run(&mut dry, 400);
        assert_eq!(torps, 0, "a dry rack launches nothing");
        assert!(beams > 0 && dealt > 0.0, "the destroyer closed and fought with its beams ({beams} beam shots, {dealt} dealt)");

        let mut two = TacticalState::open_current(3, 11, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        two.set_magazines(BTreeMap::from([(EntityId(1),
            Magazine { torpedoes: 2.0, slugs: 0.0, crystals: 2.0, crystal_capacity: 2.0 })]));
        let (_, beams, torps) = run(&mut two, 400);
        assert!((1..=2).contains(&torps), "two torpedoes aboard: at most two launched ({torps})");
        assert_eq!(two.magazines()[&EntityId(1)].torpedoes, 0.0);
        assert!(beams > 0, "then the beams");
        assert!(two.magazines()[&EntityId(1)].crystals < 2.0, "the beams wore their crystals");
    }

    #[test]
    fn worn_crystals_hit_softer_and_a_fleet_is_never_disarmed() {
        let a = ships(1, ShipKind::Cruiser, &[], 2);
        let d = ships(2, ShipKind::Battleship, &[], 1);
        let fresh_mag = Magazine { torpedoes: 0.0, slugs: 0.0, crystals: 6.0, crystal_capacity: 6.0 };
        let worn_mag = Magazine { crystals: 0.0, ..fresh_mag };
        let mut fresh = TacticalState::open_current(8, 13, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        fresh.set_magazines(BTreeMap::from([(EntityId(1), fresh_mag)]));
        let mut worn = TacticalState::open_current(8, 13, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        worn.set_magazines(BTreeMap::from([(EntityId(1), worn_mag)]));
        let (dealt_fresh, ..) = run(&mut fresh, 40);
        let (dealt_worn, beams, _) = run(&mut worn, 40);
        assert!(beams > 0 && dealt_worn > 0.0, "worn crystals still fire");
        assert!(dealt_worn < dealt_fresh * 0.75, "worn {dealt_worn} vs fresh {dealt_fresh}");
        assert_eq!(worn.magazines()[&EntityId(1)].crystal_factor(), crate::munitions::CRYSTAL_FLOOR);
    }
}
