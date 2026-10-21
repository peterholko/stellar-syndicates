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
                if pos.distance(home) > crate::transit::HYPERLIMIT {
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

    /// The privateer sets out from beyond the colony's sensors on its den's
    /// side, its den rising with it. A home without a den gets one from the
    /// dark side, away from the Market Hub.
    fn launch_founding_raid(&mut self, owner: PlayerId) {
        self.ensure_founding_den(owner);
        let corp = &self.players[&owner];
        let Some(home) = corp.home_system.and_then(|id| self.systems.iter().find(|s| s.id == id)).map(|s| s.pos) else {
            return;
        };
        let den = corp.founding.den.and_then(|d| self.systems.iter().find(|s| s.id == d)).map(|s| s.pos);
        let outward = if home.distance(self.hub) > 1.0 { (home - self.hub).normalized() } else { Vec2::new(1.0, 0.0) };
        let reach = self.config.sensor_range * crate::founding::RAID_START_SENSOR_RANGES;
        let (start, base) = match den {
            Some(den) if den.distance(home) > 1.0 => {
                (home + (den - home).normalized() * reach.min(den.distance(home)), den)
            }
            _ => (home + outward * reach, home + outward * reach),
        };
        let lurk = base + (home - base) * crate::founding::RAID_LURK_FRACTION;
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, start, FleetOrder::MoveTo { dest: home }, None);
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

        // The report hands on to the ore sale; the trader is called.
        w.time = w.time.max(report);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::SellOre);
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

