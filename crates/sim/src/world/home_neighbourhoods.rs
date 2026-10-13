//! More to find on the doorstep. Each pre-generated home gets a ring of full
//! star systems inside its command center's sensor bubble, a third of them
//! habitable, so a first expansion is a choice among neighbours instead of a
//! long haul to whichever frontier star happened to fall nearest.
use super::*;
use crate::galaxy::{
    HOME_NEIGHBOURS, HOME_NEIGHBOURS_HABITABLE, NEIGHBOURHOOD_INNER_SU, NEIGHBOURHOOD_OUTER_SU,
};
use crate::rng::Rng;
use std::collections::BTreeSet;

/// Neighbours keep the spacing every other star keeps.
const NEIGHBOUR_SPACING_SU: f64 = 12_000.0;

impl World {
    /// NEW galaxies only: right after the home systems, before anything that
    /// reads the chart (starter opportunities, pirates, sites, richness bands,
    /// sparse stars). Keyed streams leave the frontier and home geology as the
    /// seed always made them. Deposits roll on the ordinary frontier gradient;
    /// only habitability is dealt.
    pub(super) fn seed_home_neighbourhoods(&mut self) {
        let taken: BTreeSet<String> = self.systems.iter().map(|s| s.name.clone()).collect();
        let wanted = self.home_slots.len() * HOME_NEIGHBOURS;
        let mut names_rng = Rng::keyed(self.config.seed, "home-neighbour-names-v1");
        let mut names = crate::galaxy::shuffled_system_names(&mut names_rng, taken.len() + wanted)
            .into_iter()
            .filter(|name| !taken.contains(name));
        let radius = self.config.galaxy_radius;
        for slot in 0..self.home_slots.len() {
            let home = self.home_slots[slot].pos;
            let mut rng = Rng::keyed_id(self.config.seed, "home-neighbourhood-v1", slot as u64);
            let turn = rng.range(0.0, TAU);
            let mut placed = Vec::with_capacity(HOME_NEIGHBOURS);
            for sector in 0..HOME_NEIGHBOURS {
                let Some(pos) = self.neighbour_position(home, turn, sector, &mut rng) else {
                    continue;
                };
                let Some(name) = names.next() else { break };
                let id = self.alloc_entity_id();
                let frontier = ((pos.length() / radius - 0.12) / 0.84).clamp(0.0, 1.0);
                let deposits = crate::galaxy::generate_deposits(&mut rng, frontier);
                let bodies = crate::body::generate_bodies(&id.0.to_string(), &name, &deposits);
                let claim = crate::galaxy::claim_cost_for(&deposits);
                self.systems
                    .push(crate::galaxy::unowned_system(id, pos, name, bodies, claim));
                self.home_neighbours.insert(id);
                placed.push(id);
            }
            self.deal_habitability(&placed, &mut rng);
        }
    }

    /// One neighbour per sector so the ring surrounds the home; any bearing
    /// when the sector is crowded (the hub margin can pinch the inner side).
    fn neighbour_position(&self, home: Vec2, turn: f64, sector: usize, rng: &mut Rng) -> Option<Vec2> {
        let width = TAU / HOME_NEIGHBOURS as f64;
        let (hub_margin, rim) = (self.config.galaxy_radius * 0.12, self.config.galaxy_radius * 0.96);
        (0..160).find_map(|attempt| {
            let angle = if attempt < 48 {
                turn + width * (sector as f64 + rng.range(0.1, 0.9))
            } else {
                rng.range(0.0, TAU)
            };
            // Area-uniform across the ring, not bunched at its inner edge.
            let r = rng
                .range(NEIGHBOURHOOD_INNER_SU.powi(2), NEIGHBOURHOOD_OUTER_SU.powi(2))
                .sqrt();
            let pos = home + Vec2::from_polar(angle, r);
            (pos.length() >= hub_margin
                && pos.length() <= rim
                && self.systems.iter().all(|s| s.pos.distance(pos) >= NEIGHBOUR_SPACING_SU))
                .then_some(pos)
        })
    }

    /// Exactly [`HOME_NEIGHBOURS_HABITABLE`] of a home's neighbours keep a
    /// world colonists can settle: naturally habitable ones first, in a seeded
    /// order, then ones with a terrestrial or ocean world to turn Terran. The
    /// rest are prospects, their Gaia and Terran worlds reading Marginal.
    fn deal_habitability(&mut self, placed: &[EntityId], rng: &mut Rng) {
        let habitable = |world: &World, id: EntityId| {
            world
                .systems
                .iter()
                .find(|s| s.id == id)
                .is_some_and(|s| s.bodies.iter().any(|b| b.habitable))
        };
        let (mut natural, mut others): (Vec<EntityId>, Vec<EntityId>) =
            placed.iter().copied().partition(|id| habitable(self, *id));
        shuffle(&mut natural, rng);
        shuffle(&mut others, rng);
        let mut keep = BTreeSet::new();
        for id in natural.into_iter().chain(others) {
            if keep.len() == HOME_NEIGHBOURS_HABITABLE {
                break;
            }
            if let Some(system) = self.systems.iter_mut().find(|s| s.id == id)
                && system.ensure_habitable()
            {
                keep.insert(id);
            }
        }
        for id in placed.iter().filter(|id| !keep.contains(id)) {
            if let Some(system) = self.systems.iter_mut().find(|s| s.id == *id) {
                system.clear_habitability();
            }
        }
    }
}

fn shuffle(ids: &mut [EntityId], rng: &mut Rng) {
    for i in (1..ids.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;

    fn habitable(system: &StarSystem) -> bool {
        system.bodies.iter().any(|b| b.habitable)
    }

    #[test]
    fn every_home_has_a_neighbourhood_a_third_habitable_inside_its_sensors() {
        let (mut rings, mut habitable_rings) = (0usize, 0usize);
        for seed in [0xC0FFEE, 971, 4242, 0x5EED] {
            for players in [2, 4, 8] {
                let w = World::new(SimConfig::for_players(seed, players));
                assert_eq!(w.home_neighbours.len(), w.home_slots.len() * HOME_NEIGHBOURS,
                    "seed {seed} players {players}: every home gets its full ring");
                // Rings are seeded home by home, so each home's six are consecutive.
                let neighbours: Vec<&StarSystem> = w.systems.iter().filter(|s| w.home_neighbours.contains(&s.id)).collect();
                for (home, ring) in w.home_slots.iter().zip(neighbours.chunks(HOME_NEIGHBOURS)) {
                    for system in ring {
                        let d = system.pos.distance(home.pos);
                        assert!((NEIGHBOURHOOD_INNER_SU - 1.0..=NEIGHBOURHOOD_OUTER_SU + 1.0).contains(&d),
                            "seed {seed} players {players}: neighbour at {d:.0} su");
                        assert!(d < w.config.sensor_range, "inside the command center's sensors");
                    }
                    // A ring whose stars hold no other terrestrial or ocean world
                    // keeps one habitable system; rocky worlds are never turned.
                    let count = ring.iter().filter(|s| habitable(s)).count();
                    assert!((1..=HOME_NEIGHBOURS_HABITABLE).contains(&count),
                        "seed {seed} players {players}: {count} habitable neighbours");
                    rings += 1;
                    habitable_rings += count;
                }
                for (i, a) in w.systems.iter().enumerate() {
                    for b in &w.systems[i + 1..] {
                        if w.home_neighbours.contains(&a.id) || w.home_neighbours.contains(&b.id) {
                            assert!(a.pos.distance(b.pos) >= NEIGHBOUR_SPACING_SU - 1e-6, "neighbours keep star spacing");
                        }
                    }
                }
            }
        }
        assert!(habitable_rings * 10 >= rings * HOME_NEIGHBOURS_HABITABLE * 9,
            "a third of neighbours habitable on average: {habitable_rings} over {rings} rings");
    }

    #[test]
    fn uninhabited_neighbours_keep_their_geology_and_lose_only_their_air() {
        let w = World::new(SimConfig::for_players(0xC0FFEE, 4));
        for system in w.systems.iter().filter(|s| w.home_neighbours.contains(&s.id) && !habitable(s)) {
            for body in &system.bodies {
                assert!(!body.profile.environment.naturally_habitable());
                assert_ne!(body.profile.special, Some(crate::body::BodySpecial::FertileBiosphere),
                    "a fertile biosphere needs a habitable world");
            }
            assert!(!system.all_deposits().collect::<Vec<_>>().is_empty(), "prospects still carry deposits");
        }
    }

    #[test]
    fn the_starter_survey_targets_and_pirate_bases_respect_the_neighbourhood() {
        for seed in [0xC0FFEE, 971, 4242, 0x5EED] {
            let w = World::new(SimConfig::for_players(seed, 4));
            for home in &w.home_slots {
                let [garden, mineral] = home.founding_opportunities[..] else {
                    panic!("seed {seed}: two starter opportunities");
                };
                let system = |id| w.systems.iter().find(|s| s.id == id).unwrap();
                assert!(habitable(system(garden))
                    && system(garden).pos.distance(home.pos) <= NEIGHBOURHOOD_OUTER_SU + 1.0,
                    "seed {seed}: the garden is a habitable system on the home's doorstep");
                assert!(w.home_neighbours.contains(&mineral) && !habitable(system(mineral)),
                    "seed {seed}: the mineral prospect is an uninhabited neighbour");
            }
            assert!(w.enclaves.keys().all(|id| !w.home_neighbours.contains(id)),
                "seed {seed}: no pirate base on a home's doorstep");
            let per_home: Vec<usize> = w.home_slots.iter().map(|home| w.systems.iter()
                .filter(|s| w.home_neighbours.contains(&s.id) && habitable(s)
                    && s.pos.distance(home.pos) <= NEIGHBOURHOOD_OUTER_SU + 1.0).count()).collect();
            assert!(per_home.iter().all(|n| *n >= HOME_NEIGHBOURS_HABITABLE), "seed {seed}: {per_home:?}");
        }
    }
}

