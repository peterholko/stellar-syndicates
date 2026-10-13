//! Habitable worlds are the scarce resource of expansion: a habitable system
//! usually holds one, and a system with two or more should feel special.
use super::*;
use crate::galaxy::MULTI_HABITABLE_SHARE;
use crate::rng::Rng;
use std::collections::BTreeSet;

impl World {
    /// NEW galaxies only, once every full system exists (frontier, homes and
    /// neighbourhoods) and before the starter opportunities read them. Homes
    /// already hold exactly one. A seeded quota of the systems that rolled
    /// several habitable worlds keeps them; every other system keeps only the
    /// world colonists rate best, the rest reading Marginal.
    pub(super) fn thin_habitable_worlds(&mut self) {
        let homes: BTreeSet<EntityId> = self.home_slots.iter().filter_map(|h| h.system).collect();
        let worlds = |s: &StarSystem| s.bodies.iter().filter(|b| b.habitable).count();
        let candidates = self.systems.iter().filter(|s| !homes.contains(&s.id));
        let habitable = candidates.clone().filter(|s| worlds(s) >= 1).count();
        let mut several: Vec<EntityId> = candidates.filter(|s| worlds(s) >= 2).map(|s| s.id).collect();
        // Id order, then a keyed shuffle: the same seed picks the same systems.
        let mut rng = Rng::keyed(self.config.seed, "multi-habitable-v1");
        for i in (1..several.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            several.swap(i, j);
        }
        let quota = ((habitable as f64 * MULTI_HABITABLE_SHARE).round() as usize).max(1);
        let special: BTreeSet<EntityId> = several.iter().take(quota).copied().collect();
        for system in self.systems.iter_mut() {
            if homes.contains(&system.id) || special.contains(&system.id) || worlds(system) < 2 {
                continue;
            }
            let best = system.best_habitable_body();
            system.clear_habitability_except(best);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;

    fn habitable_worlds(system: &StarSystem) -> usize {
        system.bodies.iter().filter(|b| b.habitable).count()
    }

    #[test]
    fn a_home_holds_exactly_one_habitable_world_its_biomass_garden() {
        for seed in 0..40u64 {
            let w = World::new(SimConfig::for_players(seed, 4));
            for slot in &w.home_slots {
                let home = w.systems.iter().find(|s| Some(s.id) == slot.system).unwrap();
                let habitable: Vec<_> = home.bodies.iter().filter(|b| b.habitable).collect();
                assert_eq!(habitable.len(), 1, "seed {seed}: {}", home.name);
                assert!(habitable[0].deposits.iter().any(|d| d.resource == crate::cargo::Commodity::Biomass));
                assert!(home.bodies.iter().filter(|b| !b.habitable)
                    .all(|b| !b.profile.environment.naturally_habitable()), "no Gaia or Terran world left unsettleable");
            }
        }
    }

    #[test]
    fn two_or_more_habitable_worlds_are_rare_but_real() {
        let (mut habitable, mut multi, mut starter_gardens, mut starter_multi) = (0usize, 0usize, 0usize, 0usize);
        for seed in 0..80u64 {
            let w = World::new(SimConfig::for_players(seed, 4));
            let homes: BTreeSet<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
            let gardens: BTreeSet<_> = w.home_slots.iter().filter_map(|h| h.founding_opportunities.first().copied()).collect();
            for system in w.systems.iter().filter(|s| !homes.contains(&s.id)) {
                let n = habitable_worlds(system);
                if n == 0 { continue; }
                habitable += 1;
                multi += (n >= 2) as usize;
                if gardens.contains(&system.id) {
                    starter_gardens += 1;
                    starter_multi += (n >= 2) as usize;
                }
            }
        }
        let share = multi as f64 / habitable as f64;
        assert!((0.05..=0.12).contains(&share), "{multi} of {habitable} habitable systems hold 2+ ({share:.3})");
        // The starter garden upgrades a world colonists already rate best; it
        // no longer adds a second habitable world as a side effect.
        let starter_share = starter_multi as f64 / starter_gardens as f64;
        assert!(starter_share <= 0.20, "{starter_multi} of {starter_gardens} starter gardens hold 2+");
    }

    #[test]
    fn every_galaxy_has_a_few_special_systems() {
        for seed in 0..40u64 {
            for players in [2, 4, 8] {
                let w = World::new(SimConfig::for_players(seed, players));
                let homes: BTreeSet<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
                let special = w.systems.iter().filter(|s| !homes.contains(&s.id) && habitable_worlds(s) >= 2).count();
                let habitable = w.systems.iter().filter(|s| !homes.contains(&s.id) && habitable_worlds(s) >= 1).count();
                assert!(special >= 1, "seed {seed} players {players}: at least one special system");
                assert!(special * 6 <= habitable.max(6), "seed {seed} players {players}: {special} of {habitable} is not rare");
            }
        }
    }

    #[test]
    fn thinning_keeps_the_world_colonists_rate_best() {
        use crate::body::{BodyKind, BodySize, BodySpecial, Environment};
        let w = World::new(SimConfig::for_players(7, 4));
        let mut system = w.systems.iter()
            .find(|s| s.bodies.iter().filter(|b| b.parent.is_none() && b.kind != BodyKind::GasGiant).count() >= 2)
            .unwrap().clone();
        let planets: Vec<u32> = system.bodies.iter()
            .filter(|b| b.parent.is_none() && b.kind != BodyKind::GasGiant).map(|b| b.id).take(2).collect();
        for body in system.bodies.iter_mut() {
            body.habitable = false;
            body.profile.environment = Environment::Hostile;
        }
        let set = |system: &mut StarSystem, id: u32, size, environment, special| {
            let body = system.bodies.iter_mut().find(|b| b.id == id).unwrap();
            body.profile.size = size;
            body.profile.environment = environment;
            body.profile.special = special;
            body.habitable = true;
        };
        set(&mut system, planets[0], BodySize::Small, Environment::Terran, Some(BodySpecial::FertileBiosphere));
        set(&mut system, planets[1], BodySize::Huge, Environment::Gaia, None);
        assert_eq!(system.best_habitable_body(), Some(planets[1]), "a huge Gaia world outranks a small fertile one");
        system.clear_habitability_except(system.best_habitable_body());
        assert_eq!(habitable_worlds(&system), 1);
        let demoted = system.bodies.iter().find(|b| b.id == planets[0]).unwrap();
        assert_eq!(demoted.profile.environment, Environment::Marginal);
        assert_eq!(demoted.profile.special, None, "a fertile biosphere needs a habitable world");
        assert_eq!(demoted.profile.size, BodySize::Small, "only the air changes");
    }
}
