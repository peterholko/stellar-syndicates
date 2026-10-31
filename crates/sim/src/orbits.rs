//! §real-coordinates: where things are inside a star system. Every world
//! orbits its star ([`crate::body::Orbit`], laid out by
//! [`crate::body::layout_orbits`]); a ship at a system is at a world, never at
//! the star: at its berth world when it docks, at the main world when it
//! visits, and it rides that world as it turns. docs/real-system-coordinates.md.

use serde::{Deserialize, Serialize};

use crate::body::{Body, BodySize};
use crate::build::StructureKind;
use crate::galaxy::StarSystem;
use crate::ids::EntityId;
use crate::math::Vec2;
use crate::ship::ShipKind;

/// §real-coordinates: the world (or belt) of a system a fleet is at. A fleet
/// at a planet or moon rides it as it turns; one at a belt holds its point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtWorld {
    pub system: EntityId,
    pub body: u32,
}

/// Which facilities a hull berths beside, first match wins (the System View's
/// rule, now the sim's): warships by the yards that serve them, freighters by
/// the stores, troopers by the Garrison, the rest by the Shipyard.
fn berth_preferences(kind: ShipKind) -> &'static [StructureKind] {
    use StructureKind as S;
    const WARSHIP: [StructureKind; 5] =
        [S::CapitalSlipway, S::NavalDrydock, S::Shipyard, S::Garrison, S::CommandCenter];
    const FREIGHTER: [StructureKind; 4] = [S::OrbitalWarehouse, S::Warehouse, S::ServiceBerth, S::CommandCenter];
    const TRANSPORT: [StructureKind; 4] = [S::Garrison, S::CommandCenter, S::NavalDrydock, S::Shipyard];
    const OTHER: [StructureKind; 3] = [S::Shipyard, S::CommandCenter, S::Warehouse];
    match kind {
        ShipKind::Raider
        | ShipKind::Corvette
        | ShipKind::Destroyer
        | ShipKind::Cruiser
        | ShipKind::Battleship
        | ShipKind::Dreadnought
        | ShipKind::Titan => &WARSHIP,
        ShipKind::TinyFreighter
        | ShipKind::SmallFreighter
        | ShipKind::Convoy
        | ShipKind::LargeFreighter
        | ShipKind::HeavyFreighter
        | ShipKind::BulkFreighter
        | ShipKind::Freighter
        // §holds: by the stores it unloads into.
        | ShipKind::CombatFreighter => &FREIGHTER,
        ShipKind::Transport => &TRANSPORT,
        _ => &OTHER,
    }
}

fn size_rank(size: BodySize) -> u8 {
    match size {
        BodySize::Tiny => 0,
        BodySize::Small => 1,
        BodySize::Medium => 2,
        BodySize::Large => 3,
        BodySize::Huge => 4,
    }
}

impl StarSystem {
    /// §real-coordinates: lay out this system's orbits (idempotent).
    pub fn lay_out_orbits(&mut self) {
        crate::body::layout_orbits(&self.id.0.to_string(), &mut self.bodies);
    }

    pub fn body(&self, id: u32) -> Option<&Body> {
        self.bodies.iter().find(|b| b.id == id)
    }

    /// A body's offset from its star at sim time `t`, in su: a moon adds its
    /// planet's. A belt's is its ring's nearest point to nothing in
    /// particular, so belts are placed with [`StarSystem::belt_point`].
    pub fn body_offset(&self, id: u32, t: f64) -> Vec2 {
        let Some(body) = self.body(id) else { return Vec2::ZERO };
        if body.is_belt() {
            return Vec2::ZERO;
        }
        let own = body.orbit.map_or(Vec2::ZERO, |o| o.offset_at(t));
        match body.parent {
            Some(parent) => self.body_offset(parent, t) + own,
            None => own,
        }
    }

    /// Where a world is at sim time `t`.
    pub fn body_pos(&self, id: u32, t: f64) -> Vec2 {
        self.pos + self.body_offset(id, t)
    }

    /// The point of a belt's ring nearest `from` (a ship comes in to the
    /// part of the belt it faces). Belts don't turn, so it stays put.
    pub fn belt_point(&self, id: u32, from: Vec2) -> Vec2 {
        let radius = self
            .body(id)
            .and_then(|b| b.belt.map(|s| s.radius).or(b.orbit.map(|o| o.radius)))
            .unwrap_or(0.0)
            * crate::body::SYSTEM_UNIT_SU;
        let out = from - self.pos;
        let dir = if out.length_sq() > 1e-9 { out.normalized() } else { Vec2::new(1.0, 0.0) };
        self.pos + dir * radius
    }

    /// The main world: the most populated, then the Command Center's, then the
    /// first habitable planet, then the largest planet (then any moon). Never a
    /// belt; `None` for a system of belts alone.
    pub fn main_world(&self) -> Option<u32> {
        let worlds: Vec<&Body> = self.bodies.iter().filter(|b| !b.is_belt()).collect();
        if let Some(b) = worlds
            .iter()
            .filter(|b| b.population > 0.0)
            .min_by(|a, b| b.population.total_cmp(&a.population).then(a.id.cmp(&b.id)))
        {
            return Some(b.id);
        }
        if let Some(b) = worlds.iter().find(|b| b.tier(StructureKind::CommandCenter) > 0) {
            return Some(b.id);
        }
        if let Some(b) = worlds.iter().find(|b| b.parent.is_none() && b.habitable) {
            return Some(b.id);
        }
        let planets: Vec<&&Body> = worlds.iter().filter(|b| b.parent.is_none()).collect();
        let pool: Vec<&&Body> = if planets.is_empty() { worlds.iter().collect() } else { planets };
        pool.into_iter()
            .min_by(|a, b| size_rank(b.profile.size).cmp(&size_rank(a.profile.size)).then(a.id.cmp(&b.id)))
            .map(|b| b.id)
    }

    /// The world a hull of `kind` berths beside here: the body with the most
    /// tiers of the first facility its role looks for, else the main world.
    pub fn berth_world(&self, kind: ShipKind) -> Option<u32> {
        for &facility in berth_preferences(kind) {
            if let Some(host) = self
                .bodies
                .iter()
                .filter(|b| b.tier(facility) > 0)
                .min_by(|a, b| b.tier(facility).cmp(&a.tier(facility)).then(a.id.cmp(&b.id)))
            {
                return Some(host.id);
            }
        }
        self.main_world()
    }

    /// The world a system's defences stand on: the most Defense Platform
    /// tiers, then the most Orbital Defense, else the main world.
    pub fn defended_world(&self) -> Option<u32> {
        for kind in [StructureKind::DefensePlatform, StructureKind::OrbitalDefense] {
            if let Some(host) = self
                .bodies
                .iter()
                .filter(|b| b.tier(kind) > 0)
                .min_by(|a, b| b.tier(kind).cmp(&a.tier(kind)).then(a.id.cmp(&b.id)))
            {
                return Some(host.id);
            }
        }
        self.main_world()
    }

    /// The system's first belt, if it has one (a planetless system's only
    /// place to be).
    pub fn first_belt(&self) -> Option<u32> {
        self.bodies.iter().find(|b| b.is_belt()).map(|b| b.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::SYSTEM_UNIT_SU;

    fn system(id: u64) -> StarSystem {
        let bodies = crate::body::generate_bodies(&id.to_string(), "Testa", &[]);
        let mut sys = crate::galaxy::unowned_system(
            crate::ids::EntityId(id), Vec2::new(10_000.0, -4_000.0), "Testa".into(), bodies, 0.0);
        sys.lay_out_orbits();
        sys
    }

    #[test]
    fn worlds_orbit_inside_the_gravity_well_and_turn() {
        for id in 1..40 {
            let sys = system(id);
            for b in sys.bodies.iter().filter(|b| b.is_planet()) {
                let o = b.orbit.expect("laid out");
                assert!((0.2..=0.95).contains(&o.radius), "{} at {}", b.name, o.radius);
                let r = sys.body_offset(b.id, 0.0).length();
                assert!(r > 150.0 && r < crate::transit::HYPERLIMIT, "{} {r} su out", b.name);
                let later = sys.body_pos(b.id, o.period / 4.0);
                assert!(later.distance(sys.body_pos(b.id, 0.0)) > 100.0, "{} moves", b.name);
                assert!((sys.body_pos(b.id, o.period).distance(sys.body_pos(b.id, 0.0))) < 1e-6, "one turn per period");
            }
            for m in sys.bodies.iter().filter(|b| b.parent.is_some()) {
                let p = m.parent.unwrap();
                let gap = sys.body_pos(m.id, 33.0).distance(sys.body_pos(p, 33.0));
                assert!(gap > 0.04 * SYSTEM_UNIT_SU && gap < 0.2 * SYSTEM_UNIT_SU, "moon {} rides its planet", m.name);
            }
        }
    }

    #[test]
    fn laying_out_twice_changes_nothing() {
        let mut sys = system(7);
        crate::belts::add_belts(&mut sys, vec![crate::belts::home_belt()]);
        sys.lay_out_orbits();
        let once = sys.bodies.clone();
        sys.lay_out_orbits();
        assert_eq!(once, sys.bodies);
        let belt = sys.bodies.iter().find(|b| b.is_belt()).unwrap();
        let shape = belt.belt.unwrap();
        let mut orbits: Vec<f64> = sys.bodies.iter().filter(|b| b.is_planet()).map(|b| b.orbit.unwrap().radius).collect();
        orbits.sort_by(f64::total_cmp);
        assert!(orbits.windows(2).any(|w| ((w[0] + w[1]) / 2.0 - shape.radius).abs() < 1e-12), "a belt sits in a gap");
    }

    #[test]
    fn berths_follow_the_facilities_then_the_main_world() {
        let mut sys = system(11);
        let ids: Vec<u32> = sys.bodies.iter().filter(|b| b.is_planet()).map(|b| b.id).collect();
        let main = sys.main_world().unwrap();
        assert_eq!(sys.berth_world(ShipKind::Raider), Some(main), "nothing built: the main world");
        sys.bodies.iter_mut().find(|b| b.id == ids[0]).unwrap().set_tier(StructureKind::Shipyard, 2);
        sys.bodies.iter_mut().find(|b| b.id == ids[1]).unwrap().set_tier(StructureKind::Warehouse, 1);
        sys.bodies.iter_mut().find(|b| b.id == ids[2]).unwrap().population = 0.4;
        assert_eq!(sys.main_world(), Some(ids[2]), "the populated world leads");
        assert_eq!(sys.berth_world(ShipKind::Destroyer), Some(ids[0]));
        assert_eq!(sys.berth_world(ShipKind::SmallFreighter), Some(ids[1]));
        assert_eq!(sys.berth_world(ShipKind::Scout), Some(ids[0]), "the rest berth at the Shipyard");
        assert_eq!(sys.berth_world(ShipKind::Transport), Some(ids[0]), "no Garrison or CC: the Shipyard");
        assert_eq!(sys.berth_world(ShipKind::Colony), Some(ids[0]));
        sys.bodies.iter_mut().find(|b| b.id == ids[0]).unwrap().set_tier(StructureKind::Shipyard, 0);
        assert_eq!(sys.berth_world(ShipKind::Transport), Some(ids[2]), "nothing it looks for: the main world");
    }

    /// Values checked bit for bit against the client's `buildVisualSystem`
    /// (Oct 20 2026): the sim lays systems out exactly as the System View
    /// always drew them, so a system keeps its look.
    #[test]
    fn the_layout_is_the_system_views() {
        use crate::cargo::Commodity;
        use crate::galaxy::Deposit;
        let dep = |c: Commodity| Deposit { resource: c, richness: 0.4, reserves: None, accessibility: 0.1 };
        let mut bodies = crate::body::generate_bodies("29", "Testa", &[dep(Commodity::Biomass), dep(Commodity::MetallicOre)]);
        for b in &mut bodies {
            b.ensure_profile("29");
        }
        let mut sys = crate::galaxy::unowned_system(crate::ids::EntityId(29), Vec2::ZERO, "Testa".into(), bodies, 0.0);
        crate::belts::add_belts(&mut sys, vec![crate::belts::home_belt(), crate::belts::RolledBelt {
            shape: crate::body::BeltShape { kind: crate::body::BeltKind::Icy, radius: 0.81, width: 0.05 },
            deposits: vec![],
        }]);
        sys.lay_out_orbits();
        let orbit = |id: u32| sys.body(id).unwrap().orbit.unwrap();
        assert_eq!((orbit(0).radius, orbit(0).angle), (0.20984882129356267, 4.337252019567364));
        assert_eq!((orbit(1).radius, orbit(1).angle), (0.5869159414921887, 2.4383476144168665));
        assert_eq!((orbit(2).radius, orbit(2).angle), (0.95, 1.9755874724051905));
        assert_eq!((orbit(3).radius, orbit(3).angle, orbit(3).period), (0.07483886975377797, 4.2879429481380456, 150.0));
        assert_eq!((orbit(4).radius, orbit(4).angle), (0.06788531250924804, 4.607258278800413));
        assert_eq!(sys.body(5).unwrap().belt.unwrap().radius, 0.7684579707460943, "the home belt snaps to its gap");
        assert_eq!(sys.body(6).unwrap().belt.unwrap().radius, 0.3983823813928757);
        assert_eq!(sys.body(6).unwrap().belt.unwrap().width, 0.05);
    }
}
