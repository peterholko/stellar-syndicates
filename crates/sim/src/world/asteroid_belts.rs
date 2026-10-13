//! §belts: seeding each galaxy's asteroid belts (`crate::belts`).
use super::*;

impl World {
    /// Roll every system's belts from its own keyed stream, so no other roll
    /// moves. Homes get the one shared home belt; sparse exploration stars
    /// get bare belts. Idempotent: run once for the full systems before the
    /// richness bands are fixed, and again after the sparse stars exist.
    pub(super) fn seed_asteroid_belts(&mut self) {
        let seed = self.config.seed;
        let radius = self.config.galaxy_radius;
        let homes: std::collections::BTreeSet<EntityId> =
            self.home_slots.iter().filter_map(|h| h.system).collect();
        for sys in &mut self.systems {
            if sys.bodies.iter().any(crate::Body::is_belt) {
                continue;
            }
            let belts = if homes.contains(&sys.id) {
                vec![crate::belts::home_belt()]
            } else {
                let mut rng = crate::rng::Rng::keyed_id(seed, "belts-v1", sys.id.0);
                let frontier = crate::belts::frontier_at(sys.pos, radius);
                let full = !self.exploration_systems.contains(&sys.id);
                crate::belts::roll_belts(&mut rng, frontier, full)
            };
            crate::belts::add_belts(sys, belts);
        }
    }

    /// §pockets: roll every belt's hidden pockets from a keyed stream of their
    /// own, once every belt exists, so no other roll moves. Each home belt gets
    /// the shared Cuprite pocket. A belt that already has pockets keeps them.
    pub(super) fn seed_belt_pockets(&mut self) {
        let seed = self.config.seed;
        let radius = self.config.galaxy_radius;
        let homes: std::collections::BTreeSet<EntityId> =
            self.home_slots.iter().filter_map(|h| h.system).collect();
        for sys in &mut self.systems {
            let home = homes.contains(&sys.id);
            let frontier = crate::belts::frontier_at(sys.pos, radius);
            let mut rng = crate::rng::Rng::keyed_id(seed, "belt-pockets-v1", sys.id.0);
            for b in sys.bodies.iter_mut().filter(|b| b.is_belt()) {
                let Some(shape) = b.belt else { continue };
                // Every belt draws, filled or not, so one belt's state never
                // shifts another's roll.
                let rolled = crate::belts::roll_pockets(&mut rng, shape.kind, frontier, b.deposits.is_empty());
                if !b.pockets.is_empty() {
                    continue;
                }
                let pockets = if home { vec![crate::belts::home_pocket()] } else { rolled };
                b.pockets = pockets
                    .into_iter()
                    .map(|deposit| crate::body::Pocket { deposit, found_by: Default::default() })
                    .collect();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind as K;
    use crate::cargo::Commodity as C;
    use crate::production::Assignment;

    fn galaxy(seed: u64) -> World {
        World::new(crate::config::SimConfig::for_players(seed, 4))
    }

    #[test]
    fn every_home_gets_the_same_belt_and_the_founding_mine_stays_on_ferrite() {
        let w = galaxy(41);
        for slot in &w.home_slots {
            let home = w.systems.iter().find(|s| Some(s.id) == slot.system).unwrap();
            let belts: Vec<_> = home.bodies.iter().filter(|b| b.is_belt()).collect();
            assert_eq!(belts.len(), 1, "{} has exactly one belt", home.name);
            let belt = belts[0];
            assert_eq!(belt.belt.unwrap().kind, crate::BeltKind::Stony);
            assert_eq!(belt.deposits.len(), 1);
            assert_eq!(belt.deposits[0].resource, C::CrystallineOre);
            assert_eq!(belt.deposits[0].richness, crate::belts::HOME_BELT_RICHNESS);
            assert!(!belt.habitable && belt.population == 0.0);
            assert_eq!((belt.industrial_slots(), belt.infrastructure_slots(), belt.resource_slots()), (0, 0, 1));
            let mine = home.site_for(K::MiningComplex).unwrap();
            let site = home.bodies.iter().find(|b| b.id == mine).unwrap();
            assert!(site.deposits.iter().any(|d| d.resource == C::MetallicOre),
                "the founding mine lands on the Ferrite world, not the belt");
            for kind in [K::Smelter, K::Habitat, K::Warehouse, K::Shipyard, K::SensorArray, K::FuelRefinery] {
                assert_ne!(home.site_for(kind), Some(belt.id), "{kind:?} never sites on a belt");
            }
        }
    }

    /// The local survey stop that carries ore: exploration stars stay
    /// sparse, but each home's rocky stop keeps a belt for the founding barge.
    fn lesson_stops(w: &World) -> Vec<(EntityId, f64)> {
        w.systems
            .iter()
            .filter(|s| w.exploration_systems.contains(&s.id))
            .filter(|s| s.bodies.iter().any(|b| b.is_belt() && !b.deposits.is_empty()))
            .map(|s| {
                let home = w.home_slots.iter().map(|h| h.pos.distance(s.pos)).fold(f64::INFINITY, f64::min);
                (s.id, home)
            })
            .collect()
    }

    #[test]
    fn every_home_has_a_belt_with_ore_at_a_local_survey_stop_within_a_barges_reach() {
        for seed in [3, 41, 77, 1_201, 8_002] {
            let w = galaxy(seed);
            let stops = lesson_stops(&w);
            assert_eq!(stops.len(), w.home_slots.len(), "seed {seed}: one ore belt per home's local stops");
            // A barge flies out empty and must come home with a full hold on
            // one tank, by the conservative dispatch estimate.
            let mut barge = Fleet::single(EntityId(1), PlayerId(1), ShipKind::MiningBarge, Vec2::ZERO,
                FleetOrder::Idle, None);
            let empty = barge.mass();
            barge.add_cargo(C::MetallicOre, barge.cargo_capacity());
            for (id, distance) in stops {
                assert!(distance <= 34_000.0, "seed {seed}: {id:?} is a local stop ({distance:.0} su)");
                let there = Vec2::new(distance, 0.0);
                let fuel = crate::fuel::route_estimate(Vec2::ZERO, there, empty, 1.0, [Vec2::ZERO, there])
                    + crate::fuel::route_estimate(there, Vec2::ZERO, barge.mass(), 1.0, [Vec2::ZERO, there]);
                assert!(fuel <= barge.fuel_capacity(), "seed {seed}: {fuel:.1} > {:.1}", barge.fuel_capacity());
            }
        }
    }

    #[test]
    fn belts_follow_every_planet_and_moon_and_sparse_belts_are_bare() {
        let w = galaxy(7);
        let mut ores = std::collections::BTreeSet::new();
        for sys in &w.systems {
            let first_belt = sys.bodies.iter().position(|b| b.is_belt()).unwrap_or(sys.bodies.len());
            assert!(sys.bodies[first_belt..].iter().all(|b| b.is_belt()), "belt ids come after the roster");
            assert!(sys.bodies[..first_belt].iter().all(|b| !b.is_belt()));
            for belt in sys.bodies.iter().filter(|b| b.is_belt()) {
                assert!(belt.belt.is_some() && belt.parent.is_none() && belt.profile.special.is_none());
                if w.exploration_systems.contains(&sys.id) && !lesson_stops(&w).iter().any(|(id, _)| *id == sys.id) {
                    assert!(belt.deposits.is_empty(), "sparse stars stay sparse");
                }
                ores.extend(belt.deposits.iter().map(|d| d.resource));
            }
        }
        for ore in [C::OlivineOre, C::BorniteOre, C::IlmeniteOre] {
            assert!(ores.contains(&ore), "{ore:?} occurs in this galaxy's belts");
        }
        assert!(!ores.contains(&C::TitaniumOre) && !ores.contains(&C::RareMetalOre),
            "scarce ores come only as pockets, never as belt base ore");
    }

    #[test]
    fn a_belt_mining_complex_extracts_and_strip_rigs_boost_it() {
        let mut w = galaxy(316);
        w.legacy_test_bootstrap = true;
        w.enclaves.clear();
        let owner = PlayerId(77);
        w.step(&[Command::AddPlayer { id: owner, name: "Belt works".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        let mined = |w: &mut World| {
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            for b in &mut s.bodies {
                b.assignments.clear();
                b.structures.clear();
            }
            s.set_population(0.05);
            s.seed_warehouse();
            s.stockpile = [(C::Provisions, 50.0)].into();
            let belt = s.bodies.iter_mut().find(|b| b.is_belt()).unwrap();
            belt.set_tier(K::MiningComplex, 1);
            belt.assignments.insert(K::MiningComplex, Assignment::crew(1));
            w.accrue_production(&mut Vec::new());
            w.systems.iter().find(|s| s.id == home).unwrap().stockpile
                .get(&C::CrystallineOre).copied().unwrap_or(0.0)
        };
        let base = mined(&mut w);
        assert!(base > 0.0, "the belt's mine produces Crystalline");
        w.players.get_mut(&owner).unwrap().research.completed.insert("mat_deepcrust_iii_strip_rigs".into());
        let boosted = mined(&mut w);
        assert!((boosted / base - 1.25).abs() < 1e-6, "Strip Rigs: ×1.25 on belts ({boosted} vs {base})");
    }

    #[test]
    fn every_home_belt_hides_the_same_cuprite_pocket() {
        let w = galaxy(41);
        for slot in &w.home_slots {
            let home = w.systems.iter().find(|s| Some(s.id) == slot.system).unwrap();
            let belt = home.bodies.iter().find(|b| b.is_belt()).unwrap();
            assert_eq!(belt.pockets.len(), 1, "{}: one hidden pocket", home.name);
            assert_eq!(belt.pockets[0].deposit, crate::belts::home_pocket());
            assert!(belt.pockets[0].found_by.is_empty() && belt.prospected.is_empty(), "hidden until prospected");
            assert!(!belt.deposits.iter().any(|d| d.resource == C::CupriteOre), "a pocket is never a deposit");
        }
    }

    #[test]
    fn pockets_keep_to_their_type_and_frontier_windows() {
        let mut seen = std::collections::BTreeSet::new();
        for seed in [7, 41, 316, 2026] {
            let w = galaxy(seed);
            let homes: std::collections::BTreeSet<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
            for sys in w.systems.iter().filter(|s| !homes.contains(&s.id)) {
                let frontier = crate::belts::frontier_at(sys.pos, w.config.galaxy_radius);
                for b in sys.bodies.iter().filter(|b| b.is_belt()) {
                    let kind = b.belt.unwrap().kind;
                    let cap = if b.deposits.is_empty() { 1 } else if kind == crate::BeltKind::Shattered { 3 } else { 2 };
                    assert!(b.pockets.len() <= cap, "{}: {} pockets", b.name, b.pockets.len());
                    let mut ores: Vec<C> = b.pockets.iter().map(|p| p.deposit.resource).collect();
                    ores.dedup();
                    assert_eq!(ores.len(), b.pockets.len(), "never two pockets of one ore");
                    for p in &b.pockets {
                        let row = crate::belts::pocket_ores(kind).iter().find(|r| r.0 == p.deposit.resource)
                            .expect("a pocket holds its belt type's pocket ores");
                        assert!(crate::belts::ore_weight(*row, frontier) > 0.0, "{:?} outside its window", p.deposit.resource);
                        assert!(p.deposit.reserves.is_some_and(|r| r > 0.0), "pockets are finite");
                        assert!(p.found_by.is_empty());
                        seen.insert(p.deposit.resource);
                    }
                    for d in &b.deposits {
                        assert!(!matches!(d.resource, C::IridiumOre | C::Helium3Ice | C::VoidCrystal),
                            "rare ores come only as pockets");
                    }
                }
            }
        }
        for ore in [C::CupriteOre, C::TitaniumOre, C::IridiumOre, C::Helium3Ice, C::VoidCrystal] {
            assert!(seen.contains(&ore), "{ore:?} hides somewhere in four galaxies");
        }
    }

    #[test]
    fn pockets_come_from_their_own_stream() {
        let a = galaxy(77);
        let b = galaxy(77);
        let pockets = |w: &World| -> Vec<_> {
            w.systems.iter().flat_map(|s| s.bodies.iter().map(|b| b.pockets.clone())).collect()
        };
        assert_eq!(pockets(&a), pockets(&b), "the same seed hides the same pockets");
        assert_ne!(pockets(&a), pockets(&galaxy(78)), "another seed hides others");
        // Re-seeding leaves pockets alone, and a cleared galaxy rerolls the same.
        let mut again = galaxy(77);
        let before = serde_json::to_string(&again.systems).unwrap();
        again.seed_belt_pockets();
        assert_eq!(serde_json::to_string(&again.systems).unwrap(), before);
        for s in &mut again.systems {
            for b in &mut s.bodies {
                b.pockets.clear();
            }
        }
        again.seed_belt_pockets();
        assert_eq!(serde_json::to_string(&again.systems).unwrap(), before, "a reroll gives the same pockets");
    }
}
