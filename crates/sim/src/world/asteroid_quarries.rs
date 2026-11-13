//! §asteroid-quarry (Oct 2026, `docs/asteroid-quarry.md`): a belt's one
//! structure. It needs no workforce, power plant or fuel; each tick it pays
//! its tier's upkeep in Provisions and Machinery from its system's stock, all
//! or nothing, and while paid it works every deposit on its belt
//! (`accrue_production`). Unpaid, its line reads `no_supplies` and it stops;
//! nothing is lost.
use super::*;
use crate::build::StructureKind as K;
use crate::production::SuspendReason;

impl World {
    /// Each built quarry pays its upkeep, before the tick's production.
    pub(super) fn supply_quarries(&mut self, events: &mut Vec<Event>) {
        for sys in self.systems.iter_mut() {
            let Some(owner) = sys.owner else {
                continue;
            };
            let quarries: Vec<(usize, u32)> = sys.bodies.iter().enumerate()
                .filter(|(_, b)| b.is_belt())
                .map(|(i, b)| (i, b.tier(K::AsteroidQuarry)))
                .filter(|(_, tier)| *tier > 0)
                .collect();
            for (i, tier) in quarries {
                let need = crate::build::QUARRY_UPKEEP_PER_TIER.map(|(c, n)| (c, n * tier as f64 * DT));
                let paid = need.iter().all(|(c, n)| sys.free_stock(*c) + 1e-9 >= *n);
                if paid {
                    for (c, n) in need {
                        *sys.stockpile.entry(c).or_insert(0.0) -= n;
                        sys.tally_used(c, n);
                    }
                }
                let reason = (!paid).then_some(SuspendReason::NoSupplies);
                let line = sys.bodies[i].assignments.entry(K::AsteroidQuarry).or_default();
                if line.suspended != reason {
                    let was = line.suspended;
                    line.suspended = reason;
                    events.push(Event::new(self.time, match reason {
                        Some(reason) => EventPayload::ProductionSuspended { owner, system: sys.id, structure: K::AsteroidQuarry, reason },
                        None if was.is_some() => EventPayload::ProductionResumed { owner, system: sys.id, structure: K::AsteroidQuarry },
                        None => continue,
                    }));
                }
            }
        }
    }
}

/// Is this body's quarry working this tick (built and paid)?
pub(super) fn quarry_running(body: &crate::body::Body) -> bool {
    body.is_belt() && body.tier(K::AsteroidQuarry) > 0
        && body.assignments.get(&K::AsteroidQuarry).is_none_or(|line| line.suspended.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Commodity as C;

    /// A corporation whose home system has a belt, with a quarry of `tier`
    /// on it and `stock` of Provisions and Machinery at home.
    fn quarried(tier: u32, stock: f64) -> (World, PlayerId, EntityId, u32) {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        let owner = PlayerId(93_001);
        w.step(&[Command::AddPlayer { id: owner, name: "Quarrymen".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let belt = match sys.bodies.iter().position(|b| b.is_belt()) {
            Some(i) => i,
            None => {
                let mut rng = crate::rng::Rng::new(5);
                let belt = crate::belts::prospect_belt(&mut rng, 0.5);
                crate::belts::add_belts(sys, vec![belt]);
                sys.bodies.len() - 1
            }
        };
        let body = &mut sys.bodies[belt];
        body.deposits = vec![
            crate::galaxy::Deposit { resource: C::MetallicOre, richness: 1.0, reserves: None, accessibility: 0.5 },
            crate::galaxy::Deposit { resource: C::Volatiles, richness: 1.0, reserves: None, accessibility: 0.5 },
        ];
        body.set_tier(K::AsteroidQuarry, tier);
        let id = body.id;
        for c in [C::Provisions, C::Machinery] {
            sys.stockpile.insert(c, stock);
        }
        sys.stockpile.insert(C::MetallicOre, 0.0);
        sys.stockpile.insert(C::Volatiles, 0.0);
        (w, owner, home, id)
    }

    fn stock(w: &World, home: EntityId, c: C) -> f64 {
        w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }

    #[test]
    fn a_belt_takes_its_quarry_and_nothing_else() {
        let (w, _, home, belt) = quarried(1, 100.0);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let belt = sys.bodies.iter().find(|b| b.id == belt).unwrap();
        let planet = sys.bodies.iter().find(|b| !b.is_belt()).unwrap();
        for kind in K::ALL {
            assert_eq!(kind.suits(belt), kind == K::AsteroidQuarry, "{kind:?} on a belt");
            // §veil: the projector also wants a sky.
            assert_eq!(kind.suits(planet), kind != K::AsteroidQuarry && (kind != K::VeilProjector || planet.has_atmosphere()), "{kind:?} on a planet");
        }
        assert!(!K::AsteroidQuarry.takes_workforce() && !crate::power::powered(K::AsteroidQuarry));
        assert_eq!((belt.deposits.len(), belt.resource_slots()), (2, 1), "one site: the quarry works both deposits");
    }

    #[test]
    fn the_build_check_refuses_the_wrong_ground() {
        let (mut w, owner, home, belt) = quarried(0, 1_000.0);
        let planet = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| !b.is_belt()).unwrap().id;
        let refused = |w: &mut World, body: u32, upgrade: K| {
            let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade }]);
            events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, .. } if *o == owner))
        };
        assert!(refused(&mut w, belt, K::MiningComplex), "no Mining Complex on a belt");
        assert!(refused(&mut w, belt, K::PowerPlant), "no Power Plant on a belt");
        assert!(refused(&mut w, planet, K::AsteroidQuarry), "no quarry on a planet");
    }

    #[test]
    fn a_paid_quarry_works_every_deposit_on_its_belt_without_crew_or_power() {
        let (mut w, _, home, _) = quarried(2, 100.0);
        for _ in 0..(60 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(stock(&w, home, C::MetallicOre) > 0.0, "ore");
        assert!(stock(&w, home, C::Volatiles) > 0.0, "and volatiles, from one structure");
        // A minute at tier 2: twice an outpost's upkeep (at the output scale).
        let paid = 100.0 - stock(&w, home, C::Machinery);
        let scale = crate::production::OUTPUT_SCALE;
        assert!((paid - 2.0 * 0.3 * scale).abs() < 0.05 * scale, "Machinery paid: {paid}");
    }

    #[test]
    fn unpaid_it_stops_and_says_why_then_resumes() {
        let (mut w, owner, home, belt) = quarried(1, 0.0);
        let events = w.step(&[]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::ProductionSuspended {
            owner: o, structure: K::AsteroidQuarry, reason: SuspendReason::NoSupplies, .. } if *o == owner)));
        for _ in 0..(10 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert_eq!(stock(&w, home, C::MetallicOre), 0.0, "nothing mined unpaid");
        let line = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == belt).unwrap().assignments[&K::AsteroidQuarry].suspended;
        assert_eq!(line(&w), Some(SuspendReason::NoSupplies));
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.stockpile.insert(C::Provisions, 50.0);
        sys.stockpile.insert(C::Machinery, 50.0);
        let events = w.step(&[]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::ProductionResumed {
            structure: K::AsteroidQuarry, .. })));
        assert_eq!(line(&w), None);
    }
}
