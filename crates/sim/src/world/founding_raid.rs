//! §charter-v5: the privateer's raid on a founder's colony. §habitat-first
//! (Peter, Nov 8 2026: "the first pirate raid should arrive after the smelter
//! is built"); §smelter-raid (Peter, Nov 9 2026: "start moving towards the
//! player's system when they immediately start building their smelter"):
//! once the Smelter's order is in, the Rogue Privateer flies in from its
//! den's side (seen as the Smelter stands, over the colony after it), takes
//! the freighter's engines and every semiconductor and
//! Armament in the home stockpile and carries them to its den. §den-retreat
//! (Peter, Nov 12 2026: "let's have the first pirate raid retreat back to
//! their den instead of the hunting area"): there it joins the den's garrison,
//! so the hunt is one assault on the den. No battle unless the founder
//! already has a warship: the colony can only watch. Clearing the den later
//! offers the loot back with the den's other stores.
use super::*;
use crate::founding::{FoundingRaid, RaidPhase};

impl World {
    /// Once a tick for a founder in the opening: launch the raid when the
    /// Smelter's order is in ([`World::founding_raid_due`]), then fly it.
    pub(super) fn advance_founding_raid(&mut self, owner: PlayerId, out: &mut Vec<Event>) {
        let f = &self.players[&owner].founding;
        let Some(raid) = f.raid.clone() else {
            if !f.reward_granted && self.founding_raid_due(owner) {
                self.launch_founding_raid(owner);
            }
            return;
        };
        let cc = self.players[&owner].command_center;
        let Some((home_id, home)) = self.players[&owner].home_system
            .and_then(|id| self.systems.iter().find(|s| s.id == id).map(|s| (id, s.pos)))
        else {
            return;
        };
        let Some(pirate) = f.privateer.filter(|id| self.fleets.contains_key(id)) else {
            // Destroyed mid-raid (its kill report pays the bounty): the raid
            // ends with what it took.
            if f.raid_report_at.is_none() {
                self.end_founding_raid(owner, home_id, home, out);
            }
            // §trader-call: gone, it is off the map too.
            let f = &mut self.players.get_mut(&owner).unwrap().founding;
            if f.raid_report_at.is_some_and(|at| at <= self.time) && f.raid_lost_at.is_none() {
                f.raid_lost_at = Some(self.time);
            }
            return;
        };
        let pos = self.fleets[&pirate].pos;
        if f.raid_seen_at.is_none() && (raid.phase != RaidPhase::Inbound || self.founding_raid_detected(owner, pirate)) {
            let seen = self.time + crate::transit::delay(pos, cc, self.config.c);
            self.players.get_mut(&owner).unwrap().founding.raid_seen_at = Some(seen);
        }
        // §trader-call: once the raid's report is in, the colony tracks it
        // only by its own sensors (the server's `tracked_contacts`); the
        // moment they lose it, it drops off the map into its hunting area.
        let f = &self.players[&owner].founding;
        if f.raid_lost_at.is_none() && f.raid_report_at.is_some_and(|at| at <= self.time)
            && !self.founding_raid_detected(owner, pirate)
        {
            let lost = self.time + crate::transit::delay(pos, cc, self.config.c);
            self.players.get_mut(&owner).unwrap().founding.raid_lost_at = Some(lost);
        }
        let engaged = self.engagements.values().any(|e| e.attackers.contains(&pirate) || e.defenders.contains(&pirate));
        match raid.phase {
            RaidPhase::Inbound => {
                if engaged || !self.steer_raid_at_colony(pirate, home_id) {
                    return;
                }
                // Over the colony: everything it came for, as it stands now.
                let stock = &self.systems.iter().find(|s| s.id == home_id).unwrap().stockpile;
                let allowance = crate::founding::RAID_TAKES.iter()
                    .map(|c| (*c, stock.get(c).copied().unwrap_or(0.0).max(0.0).floor() as u32))
                    .filter(|(_, n)| *n > 0)
                    .collect();
                let r = self.players.get_mut(&owner).unwrap().founding.raid.as_mut().unwrap();
                r.phase = RaidPhase::Looting;
                r.arrived_at = Some(self.time);
                r.allowance = allowance;
            }
            RaidPhase::Looting => {
                if !engaged {
                    self.load_founding_raid(owner, pirate, home_id);
                }
                let r = self.players[&owner].founding.raid.as_ref().unwrap();
                let stock = &self.systems.iter().find(|s| s.id == home_id).unwrap().stockpile;
                let left = r.allowance.iter().any(|(c, n)| *n > 0 && stock.get(c).copied().unwrap_or(0.0) >= 1.0);
                let held = r.arrived_at.is_some_and(|at| self.time - at >= crate::founding::RAID_MAX_HOLD_S);
                if !left || held {
                    self.end_founding_raid(owner, home_id, home, out);
                }
            }
            RaidPhase::Returning => {
                if engaged || pos.distance(raid.base) > pirate::PIRATE_ASSAULT_RADIUS {
                    return;
                }
                // Home at its den: the loot joins the den's stores.
                let den = self.players[&owner].founding.den;
                let mut pack = None;
                if let Some(enclave) = den.and_then(|d| self.enclaves.get_mut(&d)).filter(|e| e.active(self.time))
                    && let Some(fleet) = self.fleets.get_mut(&pirate)
                {
                    for good in fleet.take_cargo() {
                        *enclave.plunder.entry(good.commodity).or_default() += good.units;
                    }
                    pack = enclave.pack;
                }
                // §den-retreat: it joins the garrison, and the den's fleet is
                // the privateer from now on: its fall is his.
                if let Some(pack) = pack.filter(|p| *p != pirate && self.fleets.contains_key(p)) {
                    let ships = self.fleets.remove(&pirate).map(|f| f.ships).unwrap_or_default();
                    self.fleets.get_mut(&pack).unwrap().absorb_ships(ships);
                    self.players.get_mut(&owner).unwrap().founding.privateer = Some(pack);
                } else if let Some(fleet) = self.fleets.get_mut(&pirate) {
                    fleet.order = FleetOrder::MoveTo { dest: raid.lurk };
                }
                self.players.get_mut(&owner).unwrap().founding.raid.as_mut().unwrap().phase = RaidPhase::Lurking;
            }
            RaidPhase::Lurking => {}
        }
    }

    /// §raid-course (Oct 30 2026, Peter: "The first pirate raiders seem to fly
    /// towards the player's star system instead of their planet"): a raider
    /// bound for a colony steers for the colony's world, not its star, and
    /// names the system once inside the well, so it settles over that world
    /// and rides it. Whether it is over the world now.
    pub(super) fn steer_raid_at_colony(&mut self, fleet: EntityId, home_id: EntityId) -> bool {
        let Some(sys) = self.systems.iter().find(|s| s.id == home_id) else { return false };
        let Some(f) = self.fleets.get(&fleet) else { return false };
        let star = sys.pos;
        let in_well = f.pos.distance(star) <= crate::transit::HYPERLIMIT;
        let over = in_well && self.besieged_distance(sys, f.pos) <= crate::founding::RAID_OVER_COLONY_SU;
        let dest = if in_well { star } else { sys.main_world().map_or(star, |b| sys.body_pos(b, self.time)) };
        if !(over && matches!(f.order, FleetOrder::Idle)) {
            self.fleets.get_mut(&fleet).unwrap().order = FleetOrder::MoveTo { dest };
        }
        over
    }

    /// §smelter-raid (Peter, Nov 9 2026: "I want the Pirates to start moving
    /// towards the player's system when they immediately start building their
    /// smelter and then the warning & watch occurs at the same distance right
    /// now"): the privateer sets out the moment the Smelter's order is in at
    /// home (or once one stands), not once it runs. It starts where it always
    /// did, so the colony sees it at the same range, about as the Smelter's
    /// build ends, and it is over the colony a little after. A raid due at its
    /// own step (an older charter) still sets out there.
    fn founding_raid_due(&self, owner: PlayerId) -> bool {
        use crate::build::{BuildKind, StructureKind as K};
        use crate::founding::FoundingStage as S;
        let corp = &self.players[&owner];
        match corp.founding.stage {
            S::PirateRaid => true,
            S::BuildSmelter => {
                let Some(home) = corp.home_system else { return false };
                self.build_queue.iter().any(|j| j.owner == owner && j.system == home
                    && j.what == (BuildKind::Upgrade { upgrade: K::Smelter }))
                    || self.systems.iter().find(|s| s.id == home)
                        .is_some_and(|s| s.bodies.iter().any(|b| b.tier(K::Smelter) >= 1))
            }
            _ => false,
        }
    }

    /// The privateer sets out from beyond the colony's sensors on its den's
    /// side, its den rising with it. A home without a den gets one from the
    /// dark side, away from the Market Hub.
    fn launch_founding_raid(&mut self, owner: PlayerId) {
        self.ensure_founding_den(owner);
        let corp = &self.players[&owner];
        let Some(sys) = corp.home_system.and_then(|id| self.systems.iter().find(|s| s.id == id)) else {
            return;
        };
        // §raid-course: its course is the colony's world from the first tick
        // (in warp nothing steers, so a late turn would drop it out).
        let (home, world) = (sys.pos, sys.main_world().map_or(sys.pos, |b| sys.body_pos(b, self.time)));
        // §den-retreat: home is the den's garrison, on its belt.
        let den = corp.founding.den.and_then(|d| {
            let pack = self.enclaves.get(&d).and_then(|e| e.pack).and_then(|p| self.fleets.get(&p)).map(|f| f.pos);
            pack.or_else(|| self.systems.iter().find(|s| s.id == d).map(|s| s.pos))
        });
        let outward = if home.distance(self.hub) > 1.0 { (home - self.hub).normalized() } else { Vec2::new(1.0, 0.0) };
        let reach = self.config.sensor_range * crate::founding::RAID_START_SENSOR_RANGES;
        let (start, base) = match den {
            Some(den) if den.distance(home) > 1.0 => {
                (home + (den - home).normalized() * reach.min(den.distance(home)), den)
            }
            _ => (home + outward * reach, home + outward * reach),
        };
        // §den-retreat: it waits at its den; without one, just outside the colony's sensors.
        let lurk = if den.is_some() { base }
            else { home + (base - home).normalized() * self.config.sensor_range * crate::founding::RAID_LURK_SENSOR_RANGES };
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, start, FleetOrder::MoveTo { dest: world }, None);
        // The tutorial privateer: slow hull, weak guns, full hull.
        fleet.founding_privateer = true;
        if let Some(ship) = fleet.ships.first_mut() {
            ship.hp = ship.max_hp() * crate::founding::PRIVATEER_HULL_FRAC;
        }
        self.fleets.insert(id, fleet);
        let f = &mut self.players.get_mut(&owner).unwrap().founding;
        f.privateer = Some(id);
        f.raid = Some(FoundingRaid { phase: RaidPhase::Inbound, base, lurk, arrived_at: None,
            allowance: BTreeMap::new(), stolen: BTreeMap::new(), loading_credit: 0.0 });
    }

    /// Whether the colony's sensors see the privateer now: the Command Center
    /// and any Sensor Arrays, by the shared detection rule.
    fn founding_raid_detected(&self, owner: PlayerId, pirate: EntityId) -> bool {
        let Some(fleet) = self.fleets.get(&pirate) else {
            return false;
        };
        let mut sources = vec![(self.players[&owner].command_center, self.config.sensor_range)];
        sources.extend(self.array_sensor_sources(owner));
        crate::detection::detected(fleet.detection_signature(), &sources, fleet.pos)
    }

    /// Load a tick's worth of the allowance into the privateer's hold, the
    /// dearest good first.
    fn load_founding_raid(&mut self, owner: PlayerId, pirate: EntityId, home: EntityId) {
        let raid = self.players.get_mut(&owner).unwrap().founding.raid.as_mut().unwrap();
        raid.loading_credit += crate::founding::RAID_LOOT_PER_SEC * DT;
        let Some(sys) = self.systems.iter_mut().find(|s| s.id == home) else {
            return;
        };
        while raid.loading_credit >= 1.0 {
            let good = raid.allowance.iter()
                .filter(|(c, n)| **n > 0 && sys.stockpile.get(c).copied().unwrap_or(0.0) >= 1.0)
                .max_by(|a, b| crate::market::base_price(*a.0).total_cmp(&crate::market::base_price(*b.0))
                    .then(b.0.cmp(a.0)))
                .map(|(c, _)| *c);
            let Some(good) = good else {
                break;
            };
            *sys.stockpile.get_mut(&good).unwrap() -= 1.0;
            *raid.allowance.get_mut(&good).unwrap() -= 1;
            *raid.stolen.entry(good).or_default() += 1;
            raid.loading_credit -= 1.0;
            if let Some(fleet) = self.fleets.get_mut(&pirate) {
                fleet.add_cargo(good, 1);
            }
        }
        raid.loading_credit = raid.loading_credit.min(1.0);
    }

    /// The raid breaks off: the privateer heads for its den, and the report
    /// of what it took starts home.
    fn end_founding_raid(&mut self, owner: PlayerId, system: EntityId, home: Vec2, out: &mut Vec<Event>) {
        let cc = self.players[&owner].command_center;
        let pirate = self.players[&owner].founding.privateer;
        let pos = pirate.and_then(|id| self.fleets.get(&id)).map_or(home, |f| f.pos);
        let Some(raid) = self.players.get_mut(&owner).unwrap().founding.raid.as_mut() else {
            return;
        };
        raid.phase = RaidPhase::Returning;
        let (base, stolen) = (raid.base, raid.stolen.clone());
        if let Some(fleet) = pirate.and_then(|id| self.fleets.get_mut(&id)) {
            fleet.order = FleetOrder::MoveTo { dest: base };
        }
        let f = &mut self.players.get_mut(&owner).unwrap().founding;
        f.raid_report_at.get_or_insert(self.time + crate::transit::delay(pos, cc, self.config.c));
        f.raid_seen_at.get_or_insert(self.time + crate::transit::delay(pos, cc, self.config.c));
        out.push(Event::new(self.time, EventPayload::FoundingRaided { owner, system, pos, stolen }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Commodity as C;
    use crate::founding::FoundingStage as S;

    fn founder() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        let owner = PlayerId(88301);
        w.step(&[Command::AddPlayer { id: owner, name: "Raided".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        (w, owner, home)
    }

    fn stock(w: &World, home: EntityId, c: C) -> f64 {
        w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }

    /// §charter-v6: step 4's Solar Array stands and has spent its
    /// semiconductors, so the raid finds the kit's twenty.
    fn plant_built(w: &mut World, home: EntityId) {
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let land = sys.bodies.iter().find(|b| b.tier(crate::build::StructureKind::CommandCenter) > 0).unwrap().id;
        sys.bodies.iter_mut().find(|b| b.id == land).unwrap().structures.insert(crate::build::StructureKind::PowerPlant, 1);
        *sys.stockpile.get_mut(&C::Electronics).unwrap() -= crate::founding::PLANT_SEMICONDUCTORS;
    }

    /// Run until `done`, at most `secs` of sim time, collecting events.
    fn run_until(w: &mut World, secs: f64, done: impl Fn(&World) -> bool) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..(secs * crate::config::TICK_HZ as f64) as usize {
            events.extend(w.step(&[]));
            if done(w) {
                break;
            }
        }
        events
    }

    fn landing(w: &World, home: EntityId) -> u32 {
        w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.tier(crate::build::StructureKind::CommandCenter) > 0).unwrap().id
    }

    #[test]
    fn the_charter_opens_its_structures_step_by_step() {
        use crate::build::StructureKind as K;
        let (mut w, owner, home) = founder();
        let body = landing(&w, home);
        let refusal = |w: &mut World, kind: K| {
            let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: kind }]);
            events.into_iter().find_map(|e| match e.payload {
                EventPayload::IndustryRejected { owner: o, reason, .. } if o == owner => Some(reason),
                _ => None,
            })
        };
        assert_eq!(w.players[&owner].founding.stage, S::BuildHabitat);
        // Nothing rushes the Shipyard ahead of the raid.
        assert_eq!(refusal(&mut w, K::OrbitalDefense).as_deref(), Some("The charter opens this at step 8"),
            "§gun-platform: not before the first raid");
        // §charter-v6 (Nov 7 2026): the Solar Array at 4, the mine at 5, the
        // Smelter at 6, before the raid; the Shipyard after the ore sale.
        // §habitat-first (Nov 8 2026): the Habitat at 1, the Solar Array at
        // 2, then the food chain on its power.
        assert_eq!(refusal(&mut w, K::PowerPlant).as_deref(), Some("The charter opens this at step 2"));
        assert_eq!(refusal(&mut w, K::Bioharvester).as_deref(), Some("The charter opens this at step 3"));
        assert_eq!(refusal(&mut w, K::Agroplex).as_deref(), Some("The charter opens this at step 4"));
        assert_eq!(refusal(&mut w, K::Shipyard).as_deref(), Some("The charter opens this at step 10"));
        // §freight-frames (Nov 6 2026): the Academy's research step is 19.
        assert_eq!(refusal(&mut w, K::Academy).as_deref(), Some("The charter opens this at step 19"));
        assert_eq!(refusal(&mut w, K::Smelter).as_deref(), Some("The charter opens this at step 6"));
        assert_eq!(refusal(&mut w, K::MiningComplex).as_deref(), Some("The charter opens this at step 5"));
        assert_eq!(refusal(&mut w, K::Habitat), None, "the step's own structure");
        assert_eq!(stock(&w, home, C::Electronics), crate::founding::KIT_SEMICONDUCTORS + crate::founding::PLANT_SEMICONDUCTORS,
            "the kit's chips: the Solar Array's, and the raid's");
        // A step opens its structure and every earlier one.
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.set_stage(S::BuildShipyard, 0.0);
        assert!(f.allows(K::Shipyard) && f.allows(K::PowerPlant) && f.allows(K::MiningComplex) && f.allows(K::Smelter));
        assert!(!f.allows(K::Academy));
        f.set_stage(S::Complete, 0.0);
        assert!(K::ALL.iter().all(|k| f.allows(*k)), "the charter fulfilled, nothing waits on it");
    }

    /// §habitat-first (Peter, Nov 8 2026: "the first pirate raid should
    /// arrive after the smelter is built"): the mine is an ordinary opening
    /// build. §smelter-raid (Peter, Nov 9 2026): the privateer sets out the
    /// moment the Smelter's order is in, from where it always did: seen at
    /// the same range about as the Smelter stands, over the colony after it.
    #[test]
    fn the_privateer_sets_out_on_the_smelters_order_and_arrives_after_it_stands() {
        use crate::build::StructureKind as K;
        let (mut w, owner, home) = founder();
        let body = landing(&w, home);
        plant_built(&mut w, home);
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildMine, w.time);
        let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: K::MiningComplex }]);
        assert!(!events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, .. } if *o == owner)), "{events:?}");
        let job = w.build_queue.iter().find(|j| j.owner == owner
            && j.what == (crate::build::BuildKind::Upgrade { upgrade: K::MiningComplex })).unwrap();
        let opening = (crate::founding::OPENING_BUILD_SECS * crate::config::TICK_HZ as f64) as u64;
        assert_eq!(job.complete_tick - job.started_tick.unwrap(), opening, "an opening build, no flight to race");
        let mine = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == body).unwrap().tier(K::MiningComplex);
        run_until(&mut w, 30.0, |w| mine(w) == 1);
        assert_eq!(mine(&w), 1);
        assert!(w.players[&owner].founding.raid.is_none(), "no raid with the mine");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildSmelter, w.time);
        run_until(&mut w, 10.0, |_| false);
        assert!(w.players[&owner].founding.raid.is_none(), "nor before the Smelter's order");
        // The Smelter's order: the privateer sets out at once, from beyond the
        // colony's sensors, while the step still waits on the Smelter.
        let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: K::Smelter }]);
        assert!(!events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, .. } if *o == owner)), "{events:?}");
        let stands = w.build_queue.iter().find(|j| j.owner == owner
            && j.what == (crate::build::BuildKind::Upgrade { upgrade: K::Smelter })).unwrap().complete_tick as f64 / crate::config::TICK_HZ as f64;
        run_until(&mut w, 2.0, |w| w.players[&owner].founding.raid.is_some());
        assert_eq!(w.players[&owner].founding.stage, S::BuildSmelter);
        let pirate = w.players[&owner].founding.privateer.expect("it sets out on the Smelter's order");
        let den = w.players[&owner].founding.den.and_then(|d| w.systems.iter().find(|s| s.id == d)).map(|s| s.pos).unwrap();
        let start = w.fleets[&pirate].pos.distance(home_pos);
        let reach = (w.config.sensor_range * crate::founding::RAID_START_SENSOR_RANGES).min(den.distance(home_pos));
        assert!((start - reach).abs() < 100.0, "from where it always set out: {start:.0} su, not {reach:.0}");
        assert!(start > w.config.sensor_range, "unseen as it sets out");
        // §raid-course: its course is the colony's world, not the star.
        w.step(&[]);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let world = sys.body_pos(sys.main_world().unwrap(), w.time);
        assert!(matches!(w.fleets[&pirate].order, FleetOrder::MoveTo { dest }
            if dest.distance(world) < 20.0 && dest.distance(home_pos) > 100.0), "{:?}", w.fleets[&pirate].order);
        // Seen about as the Smelter stands.
        run_until(&mut w, 60.0, |w| w.players[&owner].founding.raid_seen_at.is_some());
        let seen = w.players[&owner].founding.raid_seen_at.expect("the colony sees it coming");
        assert!((seen - stands).abs() < 10.0, "seen {:.1} s from the Smelter standing", seen - stands);
        // The Smelter is staffed: the step hands on to the raid already under way.
        run_until(&mut w, 30.0, |w| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .any(|b| b.tier(K::Smelter) >= 1));
        let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
        b.assignments.insert(K::Smelter, crate::production::Assignment::crew(1));
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::PirateRaid);
        assert_eq!(w.players[&owner].founding.privateer, Some(pirate), "one privateer");
        // §raid-course (Oct 30 2026): it loots over the colony's world, not at
        // the well's edge, after the Smelter stands.
        let phase = |w: &World| w.players[&owner].founding.raid.as_ref().unwrap().phase;
        run_until(&mut w, 90.0, |w| phase(w) != RaidPhase::Inbound);
        assert_eq!(phase(&w), RaidPhase::Looting);
        assert!(w.time - stands > 10.0, "it arrives {:.0} s after the Smelter stands", w.time - stands);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let world = sys.body_pos(sys.main_world().unwrap(), w.time);
        assert!(w.fleets[&pirate].pos.distance(world) <= crate::founding::RAID_OVER_COLONY_SU, "over the colony's world");
        // Nothing could spend the kit's chips or Armaments first: it takes them all.
        run_until(&mut w, 120.0, |w| w.players[&owner].founding.raid.as_ref().is_some_and(|r| r.phase == RaidPhase::Returning));
        let stolen = &w.players[&owner].founding.raid.as_ref().unwrap().stolen;
        assert_eq!(stolen.get(&C::Electronics).copied(), Some(crate::founding::KIT_SEMICONDUCTORS as u32));
        assert_eq!(stolen.get(&C::Armaments).copied(), Some(crate::founding::KIT_ARMAMENTS as u32));
    }

    /// §smelter-raid: a founder slow to staff the Smelter meets the raid
    /// first; the steps then hand on as soon as it is staffed, the Smelter's
    /// stipend paid on the way.
    #[test]
    fn a_raid_over_before_the_smelter_is_staffed_hands_straight_on() {
        use crate::build::StructureKind as K;
        let (mut w, owner, home) = founder();
        let body = landing(&w, home);
        plant_built(&mut w, home);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildSmelter, w.time);
        w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: K::Smelter }]);
        run_until(&mut w, 200.0, |w| w.players[&owner].founding.raid_report_at.is_some_and(|at| at <= w.time));
        assert!(w.players[&owner].founding.raid_report_at.is_some_and(|at| at <= w.time), "the raid's report is in");
        assert_eq!(w.players[&owner].founding.stage, S::BuildSmelter, "the step still waits on the Smelter's crew");
        let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
        assert!(b.tier(K::Smelter) >= 1, "the Smelter stands");
        b.assignments.insert(K::Smelter, crate::production::Assignment::crew(1));
        run_until(&mut w, 1.0, |w| w.players[&owner].founding.stage == S::BuildGunPlatform);
        assert_eq!(w.players[&owner].founding.stage, S::BuildGunPlatform, "through the raid's step at once");
        assert!(w.players[&owner].founding.milestones.contains_key(&S::PirateRaid), "the raid's step was reached");
        assert!(w.players[&owner].founding.stipends.contains_key(&S::PirateRaid), "and the Smelter's stipend paid");
    }

    #[test]
    fn the_raid_takes_every_semiconductor_and_armament_and_carries_them_to_the_den() {
        let (mut w, owner, home) = founder();
        plant_built(&mut w, home);
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        assert_eq!(stock(&w, home, C::Electronics), crate::founding::KIT_SEMICONDUCTORS);
        assert_eq!(stock(&w, home, C::Armaments), crate::founding::KIT_ARMAMENTS);
        let ore = stock(&w, home, C::MetallicOre);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PirateRaid, w.time);
        let launched = w.time;
        w.step(&[]);
        let f = &w.players[&owner].founding;
        let pirate = f.privateer.expect("the privateer sets out");
        let den = f.den.expect("its den rises with it");
        let start = w.fleets[&pirate].pos.distance(home_pos);
        assert!(start > w.config.sensor_range, "it sets out unseen: {start}");
        assert!(w.fleets[&pirate].founding_privateer);

        // Seen within seconds, over the colony within a minute.
        run_until(&mut w, 60.0, |w| w.players[&owner].founding.raid_seen_at.is_some());
        let seen = w.players[&owner].founding.raid_seen_at.expect("the colony sees it coming");
        assert!(seen - launched < 30.0, "seen {:.0} s after launch", seen - launched);
        let events = run_until(&mut w, 120.0, |w| w.players[&owner].founding.raid_report_at.is_some());
        let report = w.players[&owner].founding.raid_report_at.expect("the raid breaks off");
        assert!(report - launched < 75.0, "over {:.0} s after launch", report - launched);
        // §hull-ladder (Peter, Nov 7 2026: "obviously the most valuable
        // items"): the charter's six reactors and seven drives go too.
        let stolen = BTreeMap::from([(C::Armaments, crate::founding::KIT_ARMAMENTS as u32),
            (C::Electronics, crate::founding::KIT_SEMICONDUCTORS as u32), (C::FissionReactor, 6), (C::ZephyrDrive, 7)]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::FoundingRaided { owner: o, stolen: s, .. }
            if *o == owner && *s == stolen)), "one report of what it took");
        assert_eq!((stock(&w, home, C::Electronics), stock(&w, home, C::Armaments), stock(&w, home, C::FissionReactor)), (0.0, 0.0, 0.0));
        assert_eq!(stock(&w, home, C::PulseLaser), 6.0, "it leaves the Interceptor's guns");
        assert_eq!(stock(&w, home, C::ZephyrDrive), 0.0, "and takes every charter hull's drive");
        assert_eq!(stock(&w, home, C::Uranium), crate::fuel::URANIUM_HOME_SEED, "and the Uranium");
        assert!(stock(&w, home, C::MetallicOre) >= ore, "it takes nothing else");
        // Peter (Nov 5 2026: "only take 25 units"), and the thirteen engines.
        assert_eq!(w.fleets[&pirate].cargo_units(), 38, "the loot is aboard, not teleported");

        // The report hands on to the Laser Battery (§gun-platform).
        w.time = w.time.max(report);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::BuildGunPlatform);
        // §trader-call (Peter, Nov 5 2026): the trader sets out once the
        // privateer drops off the colony's sensors into its hunting area.
        assert!(w.founding_raid_detected(owner, pirate), "still in sight as it leaves");
        assert!(w.merchants.is_empty(), "no trader while it can still be seen");
        run_until(&mut w, 300.0, |w| w.players[&owner].founding.raid_lost_at.is_some());
        let lost = w.players[&owner].founding.raid_lost_at.expect("the colony loses it");
        assert!(!w.founding_raid_detected(owner, pirate), "out of sight");
        assert!(lost - report > 1.0 && lost - report < 240.0, "lost {:.0} s after the report", lost - report);
        // §trader-battery (Peter, Nov 9 2026): and not before its Laser Battery
        // is under construction.
        run_until(&mut w, 5.0, |_| false);
        assert!(!w.merchants.values().any(|v| v.customer == owner), "no trader before the Laser Battery");
        let landing = landing(&w, home);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == landing).unwrap()
            .structures.insert(crate::build::StructureKind::OrbitalDefense, 1);
        run_until(&mut w, 30.0, |w| w.merchants.values().any(|v| v.customer == owner));
        assert!(w.merchants.values().any(|v| v.customer == owner), "then the free trader is called");
        assert!(w.time + 1e-6 >= lost, "not before the colony lost it");

        // §den-retreat (Peter, Nov 12 2026: "let's have the first pirate raid
        // retreat back to their den instead of the hunting area"): home to its
        // den, where the loot joins the den's stores and he joins its crew.
        let pack = w.enclaves[&den].pack.expect("the den's garrison");
        let raid = w.players[&owner].founding.raid.clone().unwrap();
        assert_eq!(raid.lurk, raid.base, "it waits where it went home");
        assert!(raid.base.distance(w.fleets[&pack].pos) < 1.0, "home is the garrison on its belt");
        run_until(&mut w, 300.0, |w| w.players[&owner].founding.raid.as_ref()
            .is_some_and(|r| r.phase == RaidPhase::Lurking));
        assert!(!w.fleets.contains_key(&pirate), "its own fleet is gone");
        assert_eq!(w.players[&owner].founding.privateer, Some(pack), "the den's fleet is the privateer now");
        assert_eq!(w.fleets[&pack].ships.len(), crate::founding::DEN_GARRISON_RAIDERS as usize + 1, "he joined its crew");
        assert_eq!(w.fleets[&pack].cargo_units(), 0, "the loot went into the den's stores");
        // §stash: what it took, §den-stores beside the den's stores: two
        // Tiny Freighter loads (Peter, Nov 6 2026: "max 100 units of loot so
        // 2 trips").
        let mut held: BTreeMap<C, u32> = crate::founding::DEN_STORES.into_iter().collect();
        for (&c, &n) in &stolen {
            *held.entry(c).or_default() += n;
        }
        assert_eq!(w.enclaves[&den].plunder, held, "the den holds what it took and its stores");
        let loads = w.enclaves[&den].plunder.values().sum::<u32>().div_ceil(ShipKind::TinyFreighter.cargo_units());
        assert_eq!(loads, 2, "two trips");
        let at = w.fleets[&pack].pos;
        run_until(&mut w, 120.0, |_| false);
        assert!(w.fleets[&pack].pos.distance(at) < 1.0, "and it holds its den");
    }

    #[test]
    fn a_raid_survives_a_reload_without_repeating_or_refilling() {
        let (mut w, owner, home) = founder();
        plant_built(&mut w, home);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PirateRaid, w.time);
        run_until(&mut w, 120.0, |w| w.players[&owner].founding.raid.as_ref()
            .is_some_and(|r| r.stolen.values().sum::<u32>() >= 10));
        let mut loaded: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        loaded.fixup_after_load();
        // A fresh shipment lands mid-raid: this raid's allowance stays frozen.
        for world in [&mut w, &mut loaded] {
            world.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::Electronics, 100.0);
        }
        for world in [&mut w, &mut loaded] {
            run_until(world, 120.0, |w| w.players[&owner].founding.raid_report_at.is_some());
        }
        let took = |w: &World| w.players[&owner].founding.raid.as_ref().unwrap().stolen.clone();
        assert_eq!(took(&w), took(&loaded));
        // Peter (Nov 5 2026: "only take 25 units"); §hull-ladder: and the six
        // reactors and seven drives.
        assert_eq!(took(&w).values().sum::<u32>(), 38, "the allowance froze on arrival");
        assert_eq!(w.fleets.values().filter(|f| f.founding_privateer && f.owner.is_pirate()
            && f.ships.len() == 1).count(), 1, "one privateer");
    }
}
