//! §real-coordinates: a ship at a system is at one of its worlds, never at the
//! star. An order that names a star's position means "that system": the fleet
//! steers for the star until it drops into the gravity well, then for its own
//! spot there (its berth world when it docks, the main world when it visits,
//! its belt when it mines), a moving target, and on arrival rides that world
//! as it turns. docs/real-system-coordinates.md.

use super::*;
use crate::body::BodyKind;
use crate::industry::FleetIndustry;
use crate::orbits::AtWorld;
use crate::transit::HYPERLIMIT;

/// A fleet's steering for one tick when its order names a star.
pub(super) struct SystemAim {
    /// Where to steer this tick: the star until inside the well, then the spot.
    pub aim: Vec2,
    /// The star the order names, put back after the step.
    pub star: Vec2,
    /// The spot, once inside the well: where it is on arrival.
    pub at: Option<AtWorld>,
}

/// The one fixed point an order steers for (a patrol's current waypoint).
pub(super) fn order_point(order: &FleetOrder) -> Option<Vec2> {
    match order {
        FleetOrder::MoveTo { dest } => Some(*dest),
        FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => Some(*site),
        FleetOrder::Blockade { station, .. }
        | FleetOrder::Survey { station, .. }
        | FleetOrder::Expedition { station, .. } => Some(*station),
        FleetOrder::DefendSystem { assignment } => Some(assignment.station),
        FleetOrder::Patrol { waypoints, index, .. } if !waypoints.is_empty() => {
            Some(waypoints[index % waypoints.len()])
        }
        _ => None,
    }
}

/// Swap the order's fixed point for `to` (see [`order_point`]).
pub(super) fn set_order_point(order: &mut FleetOrder, to: Vec2) {
    match order {
        FleetOrder::MoveTo { dest } => *dest = to,
        FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => *site = to,
        FleetOrder::Blockade { station, .. }
        | FleetOrder::Survey { station, .. }
        | FleetOrder::Expedition { station, .. } => *station = to,
        FleetOrder::DefendSystem { assignment } => assignment.station = to,
        FleetOrder::Patrol { waypoints, index, .. } if !waypoints.is_empty() => {
            let at = *index % waypoints.len();
            waypoints[at] = to;
        }
        _ => {}
    }
}

/// Put back the points an order named before a step steered it at a spot:
/// the same kind of order keeps its own progress (a patrol's next waypoint, a
/// dwell clock) with the star it names.
pub(super) fn restore_order_points(order: &mut FleetOrder, original: &FleetOrder, star: Vec2) {
    match (order, original) {
        (FleetOrder::Patrol { waypoints, .. }, FleetOrder::Patrol { waypoints: before, .. }) => {
            *waypoints = before.clone();
        }
        (order, original) if std::mem::discriminant(order) == std::mem::discriminant(original) => {
            set_order_point(order, star);
        }
        _ => {}
    }
}

fn star_key(p: Vec2) -> (u64, u64) {
    (p.x.to_bits(), p.y.to_bits())
}

impl World {
    /// The system whose gravity well holds `pos` (the nearest star, then id).
    pub(crate) fn well_system(&self, pos: Vec2) -> Option<&StarSystem> {
        self.systems
            .iter()
            .filter(|s| s.pos.distance(pos) <= HYPERLIMIT)
            .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)).then(a.id.cmp(&b.id)))
    }

    /// Where `fleet` goes at `sys`: belt work at its belt, skimming beside the
    /// gas giant, a station at its anchor; blockaders over the besieged (main)
    /// world, defence posts over the defended world; a free trader at the main
    /// world; an owner's or ally's ship at its berth world; anyone else at the
    /// main world. A system of belts alone puts everyone on its belt; one with
    /// no bodies at all, at the star.
    pub(crate) fn spot_for(&self, id: EntityId, fleet: &Fleet, sys: &StarSystem) -> Option<u32> {
        let here = |system: EntityId| system == sys.id;
        match &fleet.industry {
            Some(FleetIndustry::Mine { system, body, .. } | FleetIndustry::Prospect { system, body, .. })
            | Some(FleetIndustry::Deploy { system, body, .. })
                if here(*system) && sys.body(*body).is_some() =>
            {
                return Some(*body);
            }
            Some(FleetIndustry::Skim { system, .. }) if here(*system) => {
                if let Some(giant) = sys.bodies.iter().find(|b| b.kind == BodyKind::GasGiant) {
                    return Some(giant.id);
                }
            }
            _ => {}
        }
        if let Some(anchor) = fleet.anchor
            && here(anchor.system)
            && sys.body(anchor.body).is_some()
        {
            return Some(anchor.body);
        }
        let world = match fleet.order {
            FleetOrder::Blockade { system, .. } if here(system) => sys.main_world(),
            FleetOrder::DefendSystem { assignment } if here(assignment.system) => sys.defended_world(),
            _ if self.merchants.contains_key(&id) => sys.main_world(),
            _ if sys.owner.is_some_and(|o| o == fleet.owner || self.are_allied(fleet.owner, o)) => {
                sys.berth_world(fleet.flagship_kind())
            }
            _ => sys.main_world(),
        };
        world.or_else(|| sys.first_belt())
    }

    /// A spot's point at sim time `t` for a fleet coming from `from`: a world
    /// where it is then, a belt the part of its ring facing the fleet.
    pub(crate) fn spot_pos(sys: &StarSystem, body: u32, from: Vec2, t: f64) -> Vec2 {
        if sys.body(body).is_some_and(|b| b.is_belt()) {
            sys.belt_point(body, from)
        } else {
            sys.body_pos(body, t)
        }
    }

    /// How far `p` is from a spot of `sys` at the current time: from a world,
    /// or from a belt's ring; from the star when there is no spot.
    pub(crate) fn spot_distance(&self, sys: &StarSystem, body: Option<u32>, p: Vec2) -> f64 {
        match body {
            Some(body) if sys.body(body).is_some() => p.distance(Self::spot_pos(sys, body, p, self.time)),
            _ => p.distance(sys.pos),
        }
    }

    /// Is `p` at the dock at `dock`: within the docking radius of the Hub (or
    /// any point), or anywhere in the gravity well of the system there?
    pub(crate) fn at_dock_point(&self, p: Vec2, dock: Vec2) -> bool {
        p.distance(dock) <= crate::ship::DOCK_RADIUS || self.same_system(p, dock)
    }

    /// Is `fleet` within `r` of its spot at `sys`?
    pub(crate) fn near_spot(&self, id: EntityId, fleet: &Fleet, sys: &StarSystem, r: f64) -> bool {
        self.spot_distance(sys, self.spot_for(id, fleet, sys), fleet.pos) <= r
    }

    /// How far `p` is from where a system's blockaders and sieges hold: over
    /// its main world (a belt-only system's belt).
    pub(crate) fn besieged_distance(&self, sys: &StarSystem, p: Vec2) -> f64 {
        self.spot_distance(sys, sys.main_world().or_else(|| sys.first_belt()), p)
    }

    /// Where a fleet at a world is at sim time `t`.
    #[cfg(test)]
    pub(crate) fn at_world_pos(&self, at: AtWorld, from: Vec2, t: f64) -> Option<Vec2> {
        let sys = self.systems.iter().find(|s| s.id == at.system)?;
        sys.body(at.body)?;
        Some(Self::spot_pos(sys, at.body, from, t))
    }

    /// This tick's steering, for the step from `self.time` to `t`: an aim for
    /// every fleet whose order names a star, and, for every idle fleet at a
    /// world, the point it rides to and that system's star.
    pub(super) fn system_aims(&self, t: f64) -> (BTreeMap<EntityId, SystemAim>, BTreeMap<EntityId, (Vec2, Vec2)>) {
        let stars: BTreeMap<(u64, u64), usize> =
            self.systems.iter().enumerate().map(|(i, s)| (star_key(s.pos), i)).collect();
        let by_id: BTreeMap<EntityId, usize> = self.systems.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        let mut aims = BTreeMap::new();
        let mut rides = BTreeMap::new();
        for (id, f) in &self.fleets {
            if matches!(f.order, FleetOrder::Idle) {
                if let Some(at) = f.at_world
                    && let Some(&i) = by_id.get(&at.system)
                    && self.systems[i].body(at.body).is_some()
                {
                    let sys = &self.systems[i];
                    rides.insert(*id, (Self::spot_pos(sys, at.body, f.pos, t), sys.pos));
                }
                continue;
            }
            let Some(star) = order_point(&f.order) else { continue };
            let Some(&i) = stars.get(&star_key(star)) else { continue };
            let sys = &self.systems[i];
            let aim = if f.pos.distance(sys.pos) > HYPERLIMIT {
                SystemAim { aim: star, star, at: None }
            } else {
                match self.spot_for(*id, f, sys) {
                    Some(body) => SystemAim {
                        aim: Self::spot_pos(sys, body, f.pos, t),
                        star,
                        at: Some(AtWorld { system: sys.id, body }),
                    },
                    None => SystemAim { aim: star, star, at: None },
                }
            };
            aims.insert(*id, aim);
        }
        (aims, rides)
    }

    /// §real-coordinates: no fleet idles at a star. One that appeared there (a
    /// new hull off the slip, a spawned pack, a refit leaving the yard) takes
    /// its place at its world at once. A fleet caught in a fight is left alone.
    pub(super) fn settle_idle_in_wells(&mut self) {
        let t = self.time;
        let engaged: std::collections::BTreeSet<EntityId> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        // New hulls and spawned packs appear exactly at the star.
        let stars: BTreeMap<(u64, u64), usize> =
            self.systems.iter().enumerate().map(|(i, s)| (star_key(s.pos), i)).collect();
        let mut snaps: Vec<(EntityId, AtWorld, Vec2)> = Vec::new();
        for (id, f) in &self.fleets {
            if !matches!(f.order, FleetOrder::Idle) || f.at_world.is_some() || engaged.contains(id) {
                continue;
            }
            let Some(&i) = stars.get(&star_key(f.pos)) else { continue };
            let sys = &self.systems[i];
            if let Some(body) = self.spot_for(*id, f, sys) {
                snaps.push((*id, AtWorld { system: sys.id, body }, Self::spot_pos(sys, body, f.pos, t)));
            }
        }
        for (id, at, pos) in snaps {
            let f = self.fleets.get_mut(&id).expect("listed above");
            f.pos = pos;
            f.vel = Vec2::ZERO;
            f.at_world = Some(at);
        }
    }

    /// §real-coordinates: has `fleet` come to rest at its own spot at `sys`
    /// (its point on the belt it works, the gas giant it skims)?
    pub(crate) fn fleet_at_spot(&self, id: EntityId, fleet: &Fleet, sys: &StarSystem) -> bool {
        fleet.at_world.is_some_and(|a| a.system == sys.id && Some(a.body) == self.spot_for(id, fleet, sys))
    }

    /// Test rig: put a fleet at rest at a world (or belt) of a system, as if
    /// it had flown there.
    #[cfg(test)]
    pub(crate) fn park_at(&mut self, fleet: EntityId, system: EntityId, body: u32) {
        let t = self.time;
        let sys = self.systems.iter().find(|s| s.id == system).expect("a system");
        let from = self.fleets[&fleet].pos;
        let pos = Self::spot_pos(sys, body, from, t);
        let f = self.fleets.get_mut(&fleet).expect("a fleet");
        f.pos = pos;
        f.vel = Vec2::ZERO;
        f.at_world = Some(AtWorld { system, body });
    }

    /// Test rig: put a fleet at rest at its own spot at a system, as if it had
    /// flown there.
    #[cfg(test)]
    pub(crate) fn park_at_system(&mut self, fleet: EntityId, system: EntityId) {
        let sys = self.systems.iter().find(|s| s.id == system).expect("a system");
        let body = self.spot_for(fleet, &self.fleets[&fleet], sys).expect("a system with bodies");
        self.park_at(fleet, system, body);
    }

    /// §real-coordinates: has `fleet` come to rest at the dock at `dock`: at
    /// one of the worlds of the system there, or within the docking radius of
    /// any other point (the Hub, a station)?
    pub(crate) fn fleet_at_dock(&self, fleet: &Fleet, dock: Vec2) -> bool {
        match self.systems.iter().find(|s| s.pos == dock) {
            Some(sys) => {
                fleet.at_world.is_some_and(|a| a.system == sys.id)
                    || (matches!(fleet.order, FleetOrder::Idle) && fleet.pos.distance(dock) <= crate::ship::DOCK_RADIUS)
            }
            None => fleet.pos.distance(dock) <= crate::ship::DOCK_RADIUS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;

    /// A founder and a fuelled Interceptor of theirs, out in space.
    fn scene() -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(5150, 4));
        let owner = PlayerId(7_100);
        w.step(&[Command::AddPlayer { id: owner, name: "Coordinates".into() }]);
        w.enclaves.clear();
        let home = w.players[&owner].home_system.unwrap();
        let id = w.alloc_entity_id();
        let at = w.players[&owner].home + Vec2::new(6_000.0, 2_000.0);
        let mut f = Fleet::single(id, owner, ShipKind::Raider, at, FleetOrder::Idle, None);
        f.fuel = f.fuel_capacity();
        w.fleets.insert(id, f);
        (w, owner, home, id)
    }

    fn sys(w: &World, id: EntityId) -> &StarSystem {
        w.systems.iter().find(|s| s.id == id).unwrap()
    }

    fn fly(w: &mut World, id: EntityId, seconds: f64) {
        for _ in 0..(seconds * crate::config::TICK_HZ as f64) as u32 {
            w.step(&[]);
            if matches!(w.fleets[&id].order, FleetOrder::Idle) {
                return;
            }
        }
    }

    #[test]
    fn a_fleet_sent_home_berths_at_its_world_and_rides_it_round() {
        let (mut w, owner, home, id) = scene();
        let star = sys(&w, home).pos;
        w.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: star };
        fly(&mut w, id, 180.0);
        let berth = sys(&w, home).berth_world(ShipKind::Raider).unwrap();
        let f = &w.fleets[&id];
        assert_eq!(f.at_world, Some(AtWorld { system: home, body: berth }), "docked at its berth world");
        assert!(f.pos.distance(star) > 150.0, "never at the star");
        assert_eq!(w.dock_of(id), Some(DockSite::System(home)));
        let fuel = f.fuel;
        let first = f.pos;
        for _ in 0..(40 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let f = &w.fleets[&id];
        assert!((f.pos - sys(&w, home).body_pos(berth, w.time)).length() < 1e-6, "it rides the world");
        assert!(f.pos.distance(first) > 30.0, "which has turned on");
        assert_eq!((f.vel, f.fuel), (Vec2::ZERO, fuel), "no thrust and no fuel");
        assert!(!matches!(f.core_state(), crate::detection::CoreState::UnderWay(_)), "not under way");
        assert_eq!(w.dock_of(id), Some(DockSite::System(home)));
        let _ = owner;
    }

    #[test]
    fn a_visitor_goes_to_the_main_world() {
        let (mut w, _, home, id) = scene();
        let target = w
            .systems
            .iter()
            .filter(|s| s.owner.is_none() && s.id != home && s.main_world().is_some())
            .min_by(|a, b| a.pos.distance(w.fleets[&id].pos).total_cmp(&b.pos.distance(w.fleets[&id].pos)))
            .unwrap()
            .id;
        let star = sys(&w, target).pos;
        w.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: star };
        fly(&mut w, id, 1_200.0);
        let main = sys(&w, target).main_world().unwrap();
        let f = &w.fleets[&id];
        assert_eq!(f.at_world, Some(AtWorld { system: target, body: main }));
        assert!((f.pos - sys(&w, target).body_pos(main, w.time)).length() < 1e-6);
    }

    #[test]
    fn warp_runs_to_the_well_without_dropping_short() {
        let (mut w, _, home, id) = scene();
        let star = sys(&w, home).pos;
        w.fleets.get_mut(&id).unwrap().pos = star + Vec2::new(40_000.0, -9_000.0);
        w.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: star };
        let mut warped = false;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            w.step(&[]);
            let f = &w.fleets[&id];
            warped |= f.drive_state == crate::ship::DriveState::Cruising(crate::transit::Regime::Warp);
            if f.pos.distance(star) > HYPERLIMIT + 100.0 && warped {
                assert!(
                    !matches!(f.drive_state, crate::ship::DriveState::Dropping { .. }),
                    "a moving world never drags the course off its lock outside the well"
                );
            }
            if matches!(f.order, FleetOrder::Idle) {
                break;
            }
        }
        assert!(warped && w.fleets[&id].at_world.is_some_and(|a| a.system == home));
    }

    #[test]
    fn a_point_in_a_well_means_the_system() {
        let (mut w, owner, home, id) = scene();
        let star = sys(&w, home).pos;
        w.step(&[Command::MoveShip { player_id: owner, ship_id: id, dest: star + Vec2::new(310.0, -220.0) }]);
        while matches!(w.fleets[&id].order, FleetOrder::Idle) {
            w.step(&[]); // the order's light leg
        }
        fly(&mut w, id, 180.0);
        assert_eq!(w.fleets[&id].at_world.map(|a| a.system), Some(home), "at a world of it, not the clicked point");
    }

    #[test]
    fn new_hulls_appear_at_their_berth_and_strays_fly_back() {
        let (mut w, _, home, id) = scene();
        let star = sys(&w, home).pos;
        w.fleets.get_mut(&id).unwrap().pos = star;
        w.step(&[]);
        let berth = sys(&w, home).berth_world(ShipKind::Raider).unwrap();
        assert_eq!(w.fleets[&id].at_world, Some(AtWorld { system: home, body: berth }), "off the slip to its berth");
        // Held 300 su off its world (as through a fight), it flies back.
        let f = w.fleets.get_mut(&id).unwrap();
        f.pos = f.pos + Vec2::new(300.0, 0.0);
        w.step(&[]);
        assert!(matches!(w.fleets[&id].order, FleetOrder::MoveTo { dest } if dest == star));
        fly(&mut w, id, 60.0);
        assert_eq!(w.fleets[&id].at_world, Some(AtWorld { system: home, body: berth }));
    }

    #[test]
    fn blockaders_hold_over_the_besieged_world() {
        let (mut w, _, home, id) = scene();
        // An undefended colony of a rival's, at war (the mechanics sandbox).
        let rival = PlayerId(7_200);
        w.step(&[Command::AddPlayer { id: rival, name: "Besieged".into() }]);
        w.legacy_test_bootstrap = true;
        let target = w
            .systems
            .iter()
            .find(|s| s.owner.is_none() && s.id != home && s.main_world().is_some())
            .unwrap()
            .id;
        w.systems.iter_mut().find(|s| s.id == target).unwrap().owner = Some(rival);
        let star = sys(&w, target).pos;
        let f = w.fleets.get_mut(&id).unwrap();
        f.pos = star + Vec2::new(2_000.0, 0.0);
        f.order = FleetOrder::Blockade { system: target, station: star, raid: false, repeat: false };
        for _ in 0..(90 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(w.engagements.is_empty());
        let main = sys(&w, target).main_world().unwrap();
        let near = |w: &World| w.fleets.get(&id).is_none_or(|f| f.pos.distance(sys(w, target).body_pos(main, w.time)) < 2.0);
        assert!(near(&w), "on station over the main world");
        assert!(matches!(w.fleets[&id].order, FleetOrder::Blockade { station, .. } if station == star), "the order still names the system");
    }
}
