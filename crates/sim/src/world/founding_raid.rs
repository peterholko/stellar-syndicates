//! §charter-v5: the privateer's raid on a founder's colony. As the mine comes
//! online, the Rogue Privateer flies in from its den's side, takes every
//! semiconductor and Armament in the home stockpile, carries them to its den
//! and lurks between the den and the colony. No battle unless the founder
//! already has a warship: the colony can only watch. Clearing the den later
//! offers the loot back with the den's other stores.
use super::*;
use crate::founding::{FoundingRaid, RaidPhase};

impl World {
    /// Once a tick for a founder in the opening: launch the raid when the
    /// charter reaches it, then fly it.
    pub(super) fn advance_founding_raid(&mut self, owner: PlayerId, out: &mut Vec<Event>) {
        let f = &self.players[&owner].founding;
        let Some(raid) = f.raid.clone() else {
            if f.stage == crate::founding::FoundingStage::PirateRaid && !f.reward_granted {
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
            return;
        };
        let pos = self.fleets[&pirate].pos;
        if f.raid_seen_at.is_none() && (raid.phase != RaidPhase::Inbound || self.founding_raid_detected(owner, pirate)) {
            let seen = self.time + crate::transit::delay(pos, cc, self.config.c);
            self.players.get_mut(&owner).unwrap().founding.raid_seen_at = Some(seen);
        }
        let engaged = self.engagements.values().any(|e| e.attackers.contains(&pirate) || e.defenders.contains(&pirate));
        match raid.phase {
            RaidPhase::Inbound => {
                // In the colony's well: the mine it raced comes online.
                if pos.distance(home) <= crate::transit::HYPERLIMIT
                    && self.players[&owner].founding.stage == crate::founding::FoundingStage::BuildMine
                {
                    self.finish_charter_mine(owner, home_id);
                }
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
                if let Some(enclave) = den.and_then(|d| self.enclaves.get_mut(&d)).filter(|e| e.active(self.time))
                    && let Some(fleet) = self.fleets.get_mut(&pirate)
                {
                    for good in fleet.take_cargo() {
                        *enclave.plunder.entry(good.commodity).or_default() += good.units;
                    }
                }
                if let Some(fleet) = self.fleets.get_mut(&pirate) {
                    fleet.order = FleetOrder::MoveTo { dest: raid.lurk };
                }
                self.players.get_mut(&owner).unwrap().founding.raid.as_mut().unwrap().phase = RaidPhase::Lurking;
            }
            RaidPhase::Lurking => {}
        }
    }

    /// §charter-gates (Oct 26 2026, Peter): the privateer sets out the moment
    /// the charter's mine starts going up, from where its flight into the
    /// colony's system takes [`CHARTER_MINE_SECS`](crate::founding::CHARTER_MINE_SECS):
    /// the drive's spool, then warp to the well edge (nearer, if its den is).
    /// That is the mine's build time; the mine is finished as the privateer
    /// drops into the well either way (`finish_charter_mine`). §raid-course:
    /// its course is the colony's world, so it crosses the edge on that line,
    /// then comes on at impulse.
    pub(super) fn raid_on_mine_start(&mut self, owner: PlayerId, system: EntityId) -> Option<f64> {
        let corp = self.players.get(&owner)?;
        let f = &corp.founding;
        if !f.enabled || !f.local_opening || f.stage != crate::founding::FoundingStage::BuildMine
            || f.raid.is_some() || f.reward_granted || corp.home_system != Some(system)
        {
            return None;
        }
        self.launch_founding_raid(owner);
        let sys = self.systems.iter().find(|s| s.id == system)?;
        let (home, world) = (sys.pos, sys.main_world().map_or(sys.pos, |b| sys.body_pos(b, self.time)));
        let pirate = self.fleets.get_mut(&self.players[&owner].founding.privateer?)?;
        let warp = pirate.transit_speed() * crate::transit::WARP_FACTOR;
        // How far short of the world its line in crosses the well's edge.
        let (dir, d) = ((pirate.pos - world).normalized(), world - home);
        let along = dir.dot(d);
        let edge = -along + (along * along - d.length_sq() + crate::transit::HYPERLIMIT.powi(2)).max(0.0).sqrt();
        let flight = edge + warp * (crate::founding::CHARTER_MINE_SECS - crate::transit::WARP_SPOOL_S);
        let out = pirate.pos.distance(world);
        if out > flight {
            pirate.pos = world + (pirate.pos - world) * (flight / out);
        }
        Some(crate::founding::CHARTER_MINE_SECS)
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

    /// The charter's mine, going up during the privateer's flight, comes
    /// online as it drops into the colony's well.
    fn finish_charter_mine(&mut self, owner: PlayerId, system: EntityId) {
        let tick = self.tick;
        let mine = crate::build::BuildKind::Upgrade { upgrade: crate::build::StructureKind::MiningComplex };
        if let Some(job) = self.build_queue.iter_mut()
            .find(|j| j.owner == owner && j.system == system && j.what == mine
                && j.structure_work.as_ref().is_some_and(|work| work.rate > 0.0))
            && let Some(work) = job.structure_work.as_mut()
        {
            work.required = work.completed_at(tick);
            job.complete_tick = tick;
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
        let den = corp.founding.den.and_then(|d| self.systems.iter().find(|s| s.id == d)).map(|s| s.pos);
        let outward = if home.distance(self.hub) > 1.0 { (home - self.hub).normalized() } else { Vec2::new(1.0, 0.0) };
        let reach = self.config.sensor_range * crate::founding::RAID_START_SENSOR_RANGES;
        let (start, base) = match den {
            Some(den) if den.distance(home) > 1.0 => {
                (home + (den - home).normalized() * reach.min(den.distance(home)), den)
            }
            _ => (home + outward * reach, home + outward * reach),
        };
        let lurk = home + (base - home).normalized() * self.config.sensor_range * crate::founding::RAID_LURK_SENSOR_RANGES;
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
        assert_eq!(w.players[&owner].founding.stage, S::BuildBioharvester);
        // Nothing rushes the Shipyard ahead of the raid.
        assert_eq!(refusal(&mut w, K::OrbitalDefense).as_deref(), Some("The charter opens this at step 6"),
            "§gun-platform: not before the first raid");
        assert_eq!(refusal(&mut w, K::PowerPlant).as_deref(), Some("The charter opens this at step 8"));
        assert_eq!(refusal(&mut w, K::Shipyard).as_deref(), Some("The charter opens this at step 9"));
        assert_eq!(refusal(&mut w, K::Academy).as_deref(), Some("The charter opens this at step 14"));
        assert_eq!(refusal(&mut w, K::MiningComplex).as_deref(), Some("The charter opens this at step 4"));
        assert_eq!(refusal(&mut w, K::Bioharvester), None, "the step's own structure");
        assert_eq!(stock(&w, home, C::Electronics), crate::founding::KIT_SEMICONDUCTORS, "the kit's chips wait for the raid");
        // A step opens its structure and every earlier one.
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.set_stage(S::BuildShipyard, 0.0);
        assert!(f.allows(K::Shipyard) && f.allows(K::PowerPlant) && f.allows(K::MiningComplex));
        assert!(!f.allows(K::Academy));
        f.set_stage(S::Complete, 0.0);
        assert!(K::ALL.iter().all(|k| f.allows(*k)), "the charter fulfilled, nothing waits on it");
    }

    #[test]
    fn the_privateer_sets_out_as_the_mine_starts_and_arrives_as_it_is_built() {
        use crate::build::StructureKind as K;
        let (mut w, owner, home) = founder();
        let body = landing(&w, home);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildMine, w.time);
        w.step(&[]);
        assert!(w.players[&owner].founding.raid.is_none(), "not before the mine");
        let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: K::MiningComplex }]);
        assert!(!events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, .. } if *o == owner)), "{events:?}");
        run_until(&mut w, 2.0, |w| w.players[&owner].founding.raid.is_some());
        let started = w.tick;
        let pirate = w.players[&owner].founding.privateer.expect("it sets out with the mine");
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        assert!(w.fleets[&pirate].pos.distance(home_pos) > w.config.sensor_range, "unseen as it sets out");
        // §raid-course: its course is the colony's world, not the star.
        w.step(&[]);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let world = sys.body_pos(sys.main_world().unwrap(), w.time);
        assert!(matches!(w.fleets[&pirate].order, FleetOrder::MoveTo { dest }
            if dest.distance(world) < 20.0 && dest.distance(home_pos) > 100.0), "{:?}", w.fleets[&pirate].order);
        let job = w.build_queue.iter().find(|j| j.owner == owner
            && j.what == (crate::build::BuildKind::Upgrade { upgrade: K::MiningComplex })).unwrap();
        let planned = job.complete_tick;
        let begun = job.started_tick.unwrap();
        let opening = (crate::founding::OPENING_BUILD_SECS * crate::config::TICK_HZ as f64) as u64;
        assert!(planned - started > opening, "longer than an opening build: its flight");
        let mine = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == body).unwrap().tier(K::MiningComplex);
        run_until(&mut w, 120.0, |w| w.fleets[&pirate].pos.distance(home_pos) <= crate::transit::HYPERLIMIT);
        let arrived = w.tick;
        let hz = crate::config::TICK_HZ as u64;
        assert_eq!(planned - begun, (crate::founding::CHARTER_MINE_SECS * hz as f64) as u64, "its timer: the flight");
        assert!(planned.abs_diff(arrived) <= hz / 2, "the privateer keeps it: {planned} vs {arrived}");
        assert_eq!(mine(&w), 0, "not before the privateer is in the colony's well");
        run_until(&mut w, 2.0, |w| mine(w) == 1);
        assert!(w.tick - arrived <= 1, "the mine comes online as it arrives: {} vs {arrived}", w.tick);
        // §raid-course (Oct 30 2026): it loots over the colony's world, not at
        // the well's edge.
        let phase = |w: &World| w.players[&owner].founding.raid.as_ref().unwrap().phase;
        assert_eq!(phase(&w), RaidPhase::Inbound, "it still has the well to cross");
        run_until(&mut w, 60.0, |w| phase(w) != RaidPhase::Inbound);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let world = sys.body_pos(sys.main_world().unwrap(), w.time);
        assert!(w.fleets[&pirate].pos.distance(world) <= crate::founding::RAID_OVER_COLONY_SU, "over the colony's world");
        // Nothing could spend the kit's chips or Armaments first: it takes them all.
        run_until(&mut w, 120.0, |w| w.players[&owner].founding.raid.as_ref().is_some_and(|r| r.phase == RaidPhase::Returning));
        let stolen = &w.players[&owner].founding.raid.as_ref().unwrap().stolen;
        assert_eq!(stolen.get(&C::Electronics).copied(), Some(crate::founding::KIT_SEMICONDUCTORS as u32));
        assert_eq!(stolen.get(&C::Armaments).copied(), Some(crate::founding::KIT_ARMAMENTS as u32));
    }

    #[test]
    fn the_raid_takes_every_semiconductor_and_armament_and_carries_them_to_the_den() {
        let (mut w, owner, home) = founder();
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
        let stolen = BTreeMap::from([(C::Armaments, 15), (C::Electronics, 40)]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::FoundingRaided { owner: o, stolen: s, .. }
            if *o == owner && *s == stolen)), "one report of what it took");
        assert_eq!((stock(&w, home, C::Electronics), stock(&w, home, C::Armaments)), (0.0, 0.0));
        assert!(stock(&w, home, C::MetallicOre) >= ore, "it takes nothing else");
        assert_eq!(w.fleets[&pirate].cargo_units(), 55, "the loot is aboard, not teleported");

        // The report hands on to the Gun Platform (§gun-platform); the trader is called.
        w.time = w.time.max(report);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::BuildGunPlatform);
        assert!(w.merchants.values().any(|v| v.customer == owner), "the free trader is called");

        // Home to its den, where the loot joins the den's stores; then it lurks.
        run_until(&mut w, 300.0, |w| w.players[&owner].founding.raid.as_ref()
            .is_some_and(|r| r.phase == RaidPhase::Lurking));
        assert_eq!(w.fleets[&pirate].cargo_units(), 0);
        let plunder = &w.enclaves[&den].plunder;
        assert_eq!((plunder[&C::Electronics], plunder[&C::Armaments]),
            (crate::founding::DEN_PLUNDER[1].1 + 40, 15), "the den holds what it took");
        run_until(&mut w, 300.0, |w| matches!(w.fleets[&pirate].order, FleetOrder::Idle));
        let lurk = w.fleets[&pirate].pos;
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        assert!(lurk.distance(home_pos) > w.config.sensor_range, "it lurks beyond the colony's sensors");
        assert!(lurk.distance(den_pos) > pirate::PIRATE_ASSAULT_RADIUS * 4.0, "and clear of the den's garrison");
        // §privateer-hunt (Peter, Oct 27 2026): just outside them, on the side
        // it left by, quiet enough that only a hunter on the spot finds it.
        assert!(lurk.distance(home_pos) < w.config.sensor_range * 1.2, "just outside: {:.0} su", lurk.distance(home_pos));
        assert!((lurk - home_pos).dot(den_pos - home_pos) > 0.0, "on its den's side");
        let quiet = w.fleets[&pirate].detection_signature();
        let cc = w.players[&owner].command_center;
        assert!(!crate::detection::detected(quiet, &[(cc, w.config.sensor_range)], lurk), "the colony can't see it");
        assert!(crate::detection::detected(quiet, &[(lurk + Vec2::new(500.0, 0.0), w.config.sensor_range)], lurk),
            "a hunter's sensors on the spot can");
    }

    #[test]
    fn a_raid_survives_a_reload_without_repeating_or_refilling() {
        let (mut w, owner, home) = founder();
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
        assert_eq!(took(&w).values().sum::<u32>(), 55, "the allowance froze on arrival");
        assert_eq!(w.fleets.values().filter(|f| f.founding_privateer && f.owner.is_pirate()
            && f.ships.len() == 1).count(), 1, "one privateer");
    }
}
