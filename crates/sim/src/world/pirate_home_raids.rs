//! Home-system piracy: a real inbound fleet, a contestable short blockade, and
//! physical stolen cargo. No conquest, bombardment, population loss, or timer
//! that directly deletes stock at a distance. Existing fixed heavy sites remain
//! defensive; only the three ambient hideouts send these ordinary raider packs.
use super::*;
use crate::pirate::HomeRaid;

impl World {
    fn home_raid_eligible(&self, owner: PlayerId) -> bool {
        self.players.get(&owner).is_some_and(|corp|
            !corp.founding.protected(self.time)
            && self.time - corp.joined_tick as f64 * DT >= pirate::HOME_RAID_GRACE_S)
    }

    pub(super) fn try_launch_home_raid(&mut self, base: EntityId, fleet: EntityId,
        events: &mut Vec<Event>) -> bool
    {
        let Some(enclave) = self.enclaves.get(&base) else { return false; };
        if !enclave.active(self.time) || pirate::permanent_site(enclave.tier)
            || self.pirate_support_disabled(base, pirate::CombatObjective::DisableSupply)
            || enclave.pack != Some(fleet) || self.pirate_home_raids.contains_key(&fleet) { return false; }
        let Some(pack) = self.fleets.get(&fleet) else { return false; };
        if !pack.owner.is_pirate() || !matches!(pack.order, FleetOrder::Idle) || !pack.cargo_is_empty()
            || self.engagements.values().any(|e| e.attackers.contains(&fleet) || e.defenders.contains(&fleet))
        { return false; }
        let from = pack.pos;
        // One expedition per corporation, including its return leg. The global
        // per-victim cooldown prevents three independent hideouts piling on.
        // Oldest eligible target first, distance/id only as deterministic ties:
        // a nearby rich home cannot monopolize every launch forever.
        let target = self.players.iter().filter_map(|(&owner, corp)| {
            if !self.home_raid_eligible(owner)
                || self.pirate_home_raid_after.get(&owner).is_some_and(|at| self.time < *at)
                || self.pirate_home_raids.values().any(|r| r.owner == owner) { return None; }
            let sys = self.systems.iter().find(|s| Some(s.id) == corp.home_system
                && s.owner == Some(owner) && s.blockade.is_none())?;
            if self.in_sovereign_zone(sys.pos) || !sys.stockpile.values()
                .any(|stock| (*stock - PLUNDER_FLOOR_UNITS).min(*stock * pirate::HOME_RAID_STOCK_FRAC) >= 1.0)
            { return None; }
            Some((owner, sys.id, sys.pos, self.pirate_home_raid_after.get(&owner).copied().unwrap_or(0.0)))
        }).min_by(|a, b| a.3.total_cmp(&b.3)
            .then(from.distance(a.2).total_cmp(&from.distance(b.2))).then(a.0.cmp(&b.0)));
        let Some((owner, system, pos, _)) = target else { return false; };
        let want = pirate::pack_size(enclave.tier);
        let pack = self.fleets.get_mut(&fleet).unwrap();
        let have = pack.count(ShipKind::Raider);
        if want > have && pack.pirate_faction.is_none() { pack.add(ShipKind::Raider, want - have); }
        // An intentional pirate transmission, not invisible CC omniscience.
        // Hold the real fleet at its base until the warning has had time to
        // arrive PLUS a reaction window. Even a nearby base cannot alpha-strike
        // the home before its warning. Orders to defenders still travel normally.
        let depart_at = self.time + crate::transit::delay(from,
            self.players[&owner].command_center, self.config.c) + pirate::HOME_RAID_WARNING_S;
        self.pirate_home_raids.insert(fleet, HomeRaid { base, owner, system, pos,
            depart_at, arrived_at: None, allowance: BTreeMap::new(), stolen: BTreeMap::new(),
            loading_credit: 0.0, returning: false, source_lost_at: None, counter_raid_offered: false });
        self.pirate_home_raid_after.insert(owner, self.time + pirate::HOME_RAID_COOLDOWN_S);
        events.push(Event::new(self.time, EventPayload::PirateRaidInbound {
            owner, system, fleet, ships: self.fleets[&fleet].total_count(), pos: from }));
        true
    }

    pub(super) fn tick_pirate_home_raids(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<_> = self.pirate_home_raids.keys().copied().collect();
        for fleet in ids {
            let raid = self.pirate_home_raids[&fleet].clone();
            let Some(pack) = self.fleets.get(&fleet) else {
                self.pirate_home_raid_after.insert(raid.owner, self.time + pirate::HOME_RAID_COOLDOWN_S);
                self.pirate_home_raids.remove(&fleet);
                continue; // ordinary battle/loss light explains destruction
            };
            let base = self.systems.iter().find(|s| s.id == raid.base).map(|s| s.pos);
            let pack_pos = pack.pos;
            let engaged = self.engagements.values().any(|e|
                e.attackers.contains(&fleet) || e.defenders.contains(&fleet));
            if raid.returning {
                self.withdraw_home_raider(fleet);
                if !engaged && base.is_none_or(|p| pack_pos.distance(p) <= pirate::PIRATE_ASSAULT_RADIUS) {
                    if let Some(pack) = self.fleets.get_mut(&fleet) {
                        pack.order = FleetOrder::Idle;
                        // If the source has been cleared, its survivors can
                        // withdraw there but never resurrect a hostile base.
                        if let Some(enclave) = self.enclaves.get_mut(&raid.base).filter(|e| e.active(self.time)) {
                            for good in pack.take_cargo() { *enclave.plunder.entry(good.commodity).or_default() += good.units; }
                        }
                    }
                    self.pirate_home_raids.remove(&fleet);
                }
                continue;
            }
            let valid_home = self.home_raid_eligible(raid.owner)
                && self.players[&raid.owner].home_system == Some(raid.system) && self.systems.iter().any(|s|
                s.id == raid.system && s.owner == Some(raid.owner) && !self.in_sovereign_zone(s.pos));
            let active_base = self.enclaves.get(&raid.base).is_some_and(|e| e.active(self.time));
            // Source suppression is settled later in the tick, so recording its
            // first absence here is conservatively one tick late, never early.
            // Compare against the MOVING receiver's current distance: this is a
            // wavefront, not an instant truth-triggered turn visible at the home.
            let source_lost_at = if !active_base {
                Some(*self.pirate_home_raids.get_mut(&fleet).unwrap().source_lost_at.get_or_insert(self.time))
            } else { raid.source_lost_at };
            let loss_heard = source_lost_at.is_some_and(|at| base.is_none_or(|origin|
                self.time >= at + crate::transit::delay(origin, pack_pos, self.config.c)));
            if !valid_home || loss_heard {
                self.end_home_raid(fleet, events);
                continue;
            }
            if self.time < raid.depart_at { continue; }
            if let Some(pack) = self.fleets.get_mut(&fleet) {
                pack.order = FleetOrder::Blockade { system: raid.system, station: raid.pos };
            }
            let at_home = self.fleets[&fleet].pos.distance(raid.pos) <= BLOCKADE_STATION_RADIUS;
            if at_home && raid.arrived_at.is_none() {
                let stock = &self.systems.iter().find(|s| s.id == raid.system).unwrap().stockpile;
                let allowance = stock.iter().map(|(&c, &n)| (c, (n * pirate::HOME_RAID_STOCK_FRAC)
                    .min(n - PLUNDER_FLOOR_UNITS).max(0.0).floor() as u32)).collect();
                let r = self.pirate_home_raids.get_mut(&fleet).unwrap();
                r.arrived_at = Some(self.time);
                r.allowance = allowance;
            }
            if raid.arrived_at.is_some_and(|at| self.time - at
                >= pirate::HOME_RAID_HOLD_BATTLE_MULT * self.config.battle_target_secs)
                || raid.stolen.values().sum::<u32>() >= pirate::HOME_RAID_MAX_LOOT
            {
                self.end_home_raid(fleet, events);
            }
        }
    }

    fn end_home_raid(&mut self, fleet: EntityId, events: &mut Vec<Event>) {
        let Some(raid) = self.pirate_home_raids.get_mut(&fleet) else { return; };
        if !raid.returning {
            raid.returning = true;
            self.pirate_home_raid_after.insert(raid.owner, self.time + pirate::HOME_RAID_COOLDOWN_S);
            if let Some(pack) = self.fleets.get(&fleet) {
                events.push(Event::new(self.time, EventPayload::PirateRaidWithdrawn {
                    owner: raid.owner, system: raid.system, fleet, pos: pack.pos, plunder: raid.stolen.clone() }));
            }
        }
        if let Some(pos) = self.fleets.get(&fleet).map(|f| f.pos) {
            self.offer_counter_raid(fleet, pos, events);
        }
        self.withdraw_home_raider(fleet);
    }

    /// Withdrawing in a battle uses the SAME recorded tactical exit as normal
    /// retreat. Never delete an engagement, teleport, or make the pack invulnerable.
    fn withdraw_home_raider(&mut self, fleet: EntityId) {
        let Some(raid) = self.pirate_home_raids.get(&fleet) else { return; };
        let base = self.systems.iter().find(|s| s.id == raid.base).map(|s| s.pos);
        if let Some(pack) = self.fleets.get_mut(&fleet) {
            pack.order = base.map_or(FleetOrder::Idle, |dest| FleetOrder::MoveTo { dest });
        }
        for (&id, e) in &mut self.engagements {
            let side = if e.attackers.contains(&fleet) { 0 }
                else if e.defenders.contains(&fleet) { 1 } else { continue; };
            if let Some(tac) = e.tactical.as_mut() && !tac.withdrawing[side] {
                self.battle_records.get_mut(&id).expect("live battle recorded")
                    .withdraw_tactical(self.tick, tac, side as u8);
                if side == 0 { e.a_fled = true; } else { e.d_fled = true; }
            }
        }
    }

    pub(super) fn home_raid_battle_ended(&mut self, e: &Engagement, events: &mut Vec<Event>) {
        for (side, members) in [(0, &e.attackers), (1, &e.defenders)] {
            for &fleet in members {
                if !self.pirate_home_raids.contains_key(&fleet) { continue; }
                let withdrew = if side == 0 { e.a_fled } else { e.d_fled };
                let opponents_live = if side == 0 {
                    e.d_fled || e.defenders.iter().any(|f| self.fleets.contains_key(f))
                        || e.platform_system.is_some_and(|s| self.systems.iter().any(|sys|
                            sys.id == s && sys.tier_sum(crate::build::StructureKind::DefensePlatform) > 0))
                } else { e.a_fled || e.attackers.iter().any(|f| self.fleets.contains_key(f))
                    || self.platform_state(e.attacker_platform_system).0 > 0 };
                if withdrew || opponents_live { self.end_home_raid(fleet, events); }
            }
        }
    }

    pub(super) fn plunder_home_raid(&mut self, fleet: EntityId, events: &mut Vec<Event>) {
        let raid = &self.pirate_home_raids[&fleet];
        if raid.returning || raid.arrived_at.is_none()
            || self.engagements.values().any(|e| e.platform_system == Some(raid.system)
                || e.attacker_platform_system == Some(raid.system)
                || e.attackers.contains(&fleet) || e.defenders.contains(&fleet)) { return; }
        let Some(sys) = self.systems.iter().find(|s| s.id == raid.system) else { return; };
        // A home raid MUST beat the standing defense first. The generic PvP
        // blockade drain is unchanged; this bounded NPC path never steals while
        // the opening engagement is still running or a fresh defense is present.
        if sys.tier_sum(crate::build::StructureKind::DefensePlatform) > 0
            || self.fleets.values().any(|f| f.owner == raid.owner && f.is_combatant()
                && f.pos.distance(sys.pos) <= crate::build::DEFENSE_PLATFORM_RADIUS) { return; }
        let room = pirate::HOME_RAID_MAX_LOOT.saturating_sub(self.fleets[&fleet].cargo_units());
        let raid = self.pirate_home_raids.get_mut(&fleet).unwrap();
        raid.loading_credit += PLUNDER_UNITS_PER_SEC * DT;
        let mut remaining = (raid.loading_credit.floor() as u32).min(room);
        while remaining > 0 {
            let sys = self.systems.iter_mut().find(|s| s.id == raid.system).unwrap();
            let good = raid.allowance.iter().filter(|(c, n)| **n > 0
                && sys.stockpile.get(c).copied().unwrap_or(0.0) >= PLUNDER_FLOOR_UNITS + 1.0)
                .max_by(|a, b| crate::market::base_price(*a.0).total_cmp(&crate::market::base_price(*b.0))
                    .then(b.0.cmp(a.0))).map(|(c, _)| *c);
            let Some(good) = good else { break; };
            *sys.stockpile.get_mut(&good).unwrap() -= 1.0;
            *raid.allowance.get_mut(&good).unwrap() -= 1;
            *raid.stolen.entry(good).or_default() += 1;
            raid.loading_credit -= 1.0;
            remaining -= 1;
            self.fleets.get_mut(&fleet).unwrap().add_cargo(good, 1);
            events.push(Event::new(self.time, EventPayload::SystemPlundered {
                by: PlayerId::PIRATE, owner: raid.owner, system: raid.system,
                commodity: good, units: 1, pos: raid.pos }));
        }
        // No banked loading bursts if stock was spent below its protected floor.
        raid.loading_credit = raid.loading_credit.fract();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Commodity;
    use crate::build::StructureKind;

    fn scene() -> (World, PlayerId, EntityId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let owner = PlayerId(95002);
        w.step(&[Command::AddPlayer { id: owner, name: "Home defender".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        let home_pos = w.players[&owner].home;
        let base = *w.enclaves.iter().find(|(_, e)| !pirate::permanent_site(e.tier)).unwrap().0;
        w.enclaves.retain(|id, _| *id == base);
        w.fleets.clear();
        w.time = pirate::HOME_RAID_GRACE_S + 1.0;
        w.tick = (w.time / DT).round() as u64;
        w.players.get_mut(&owner).unwrap().founding.enabled = false;
        w.players.get_mut(&owner).unwrap().joined_tick = 0;
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.stockpile = BTreeMap::from([(Commodity::Electronics, 1000.0),
            (Commodity::MetallicOre, 41.0), (Commodity::Fuel, 40.0), (Commodity::Provisions, 40.0)]);
        let from = home_pos + Vec2::new(20_000.0, 0.0);
        w.systems.iter_mut().find(|s| s.id == base).unwrap().pos = from;
        let fleet = w.spawn_pirate_pack(1, from);
        let e = w.enclaves.get_mut(&base).unwrap();
        e.pack = Some(fleet);
        e.tier = 1;
        e.next_launch_at = w.time;
        e.next_grow_at = w.time + 1e9;
        (w, owner, home, base, fleet)
    }

    fn launch(w: &mut World, base: EntityId, fleet: EntityId) -> Event {
        let mut events = Vec::new();
        assert!(w.try_launch_home_raid(base, fleet, &mut events));
        assert_eq!(events.len(), 1);
        events.remove(0)
    }

    fn reach_home(w: &mut World, fleet: EntityId) {
        for _ in 0..(600 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.pirate_home_raids[&fleet].arrived_at.is_some() { return; }
        }
        panic!("physical pack must reach its target");
    }

    #[test]
    fn home_raid_respects_founder_shield_grace_and_one_expedition_per_victim() {
        let (mut w, owner, _, base, fleet) = scene();
        let corp = w.players.get_mut(&owner).unwrap();
        corp.founding.enabled = true;
        assert!(!w.try_launch_home_raid(base, fleet, &mut Vec::new()), "existing founder shield wins");
        let corp = w.players.get_mut(&owner).unwrap();
        corp.founding.enabled = false;
        corp.joined_tick = w.tick;
        assert!(!w.try_launch_home_raid(base, fleet, &mut Vec::new()), "legacy/new joins get a grace clock too");
        w.players.get_mut(&owner).unwrap().joined_tick = 0;
        launch(&mut w, base, fleet);
        // Give another hideout a real separate pack. Even after its cooldown
        // would allow a launch, the still-live mission reserves this victim.
        let other_base = w.systems.iter().find(|s| s.id != base && s.owner.is_none()).unwrap().id;
        let mut other = w.enclaves[&base].clone();
        other.system = other_base;
        let second = w.spawn_pirate_pack(1, w.fleets[&fleet].pos);
        other.pack = Some(second);
        w.enclaves.insert(other_base, other);
        w.time += pirate::HOME_RAID_COOLDOWN_S + 1.0;
        assert!(!w.try_launch_home_raid(other_base, second, &mut Vec::new()));
        w.end_home_raid(fleet, &mut Vec::new());
        w.tick_pirate_home_raids(&mut Vec::new()); // already at the base
        assert!(!w.try_launch_home_raid(other_base, second, &mut Vec::new()), "break-off refreshes rest period");
        w.time += pirate::HOME_RAID_COOLDOWN_S + 1.0;
        assert!(w.try_launch_home_raid(other_base, second, &mut Vec::new()));
    }

    #[test]
    fn home_raids_launch_without_a_contract_and_fixed_garrisons_do_not_raid() {
        let (mut w, owner, _, base, fleet) = scene();
        assert!(w.operations.values().all(|o| o.participants.is_empty()));
        let events = w.step(&[]);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::PirateRaidInbound { owner: p, .. } if p == owner)));
        assert!(w.pirate_home_raids.contains_key(&fleet));
        assert!(!w.try_launch_home_raid(base, fleet, &mut Vec::new()), "the same pack cannot overwrite an expedition");
        assert!(!w.step(&[]).iter().any(|e| matches!(e.payload, EventPayload::PirateRaidInbound { .. })));
        w.pirate_home_raids.clear();
        w.pirate_home_raid_after.clear();
        w.enclaves.get_mut(&base).unwrap().tier = pirate::STRONGHOLD_TIER;
        assert!(!w.try_launch_home_raid(base, fleet, &mut Vec::new()), "heavy conquest sites retain fixed garrisons");
    }

    #[test]
    fn home_raid_warning_precedes_physical_departure_and_loading_is_capped() {
        let (mut w, owner, home, base, fleet) = scene();
        let event = launch(&mut w, base, fleet);
        let from = w.fleets[&fleet].pos;
        let arrival = event.time + crate::transit::delay(from, w.players[&owner].command_center, w.config.c);
        assert_eq!(event.physical_origin(&w), Some(from));
        assert!((w.pirate_home_raids[&fleet].depart_at - arrival - pirate::HOME_RAID_WARNING_S).abs() < 1e-9);
        let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Electronics];
        while w.time + DT < w.pirate_home_raids[&fleet].depart_at {
            w.step(&[]);
            assert_eq!(w.fleets[&fleet].pos, from, "real attackers stage before departure");
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Electronics], stock);
        }
        reach_home(&mut w, fleet);
        let at = w.pirate_home_raids[&fleet].arrived_at.unwrap();
        assert!(at > arrival + pirate::HOME_RAID_WARNING_S, "travel is real too");
        let before = w.systems.iter().find(|s| s.id == home).unwrap().clone();
        let mut stolen = 0;
        let mut withdrawal = None;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            for event in w.step(&[]) {
                if let EventPayload::SystemPlundered { by, system, units, .. } = event.payload
                    && by.is_pirate() && system == home { stolen += units; }
                if matches!(event.payload, EventPayload::PirateRaidWithdrawn { .. }) { withdrawal = Some(event); }
            }
            if withdrawal.is_some() { break; }
        }
        assert!(withdrawal.is_some(), "a raid always breaks off");
        assert_eq!(stolen, pirate::HOME_RAID_MAX_LOOT);
        assert_eq!(w.fleets[&fleet].cargo_units(), stolen, "no teleport to pirate inventory");
        let after = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(after.stockpile[&Commodity::Electronics], stock - stolen as f64);
        assert_eq!(after.owner, Some(owner));
        for (a, b) in before.bodies.iter().zip(&after.bodies) {
            assert_eq!(a.structures, b.structures, "no civilian building damage");
            assert!(b.population >= a.population, "no population losses");
        }
        assert!(after.blockade.as_ref().is_none_or(|b| b.siege_since.is_none()), "home can never be conquered");
        w.step(&[]);
        assert!(w.systems.iter().find(|s| s.id == home).unwrap().blockade.is_none());
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { dest } if dest == from));
        assert!(w.enclaves[&base].plunder.is_empty(), "loot is still in flight");
        for _ in 0..(300 * crate::config::TICK_HZ) {
            w.step(&[]);
            if !w.pirate_home_raids.contains_key(&fleet) { break; }
        }
        assert!(!w.pirate_home_raids.contains_key(&fleet));
        assert_eq!(w.enclaves[&base].plunder.values().sum::<u32>(), stolen);
    }

    #[test]
    fn home_raid_cannot_loot_during_defense_and_a_garrison_can_repel_it() {
        let (mut w, owner, home, base, fleet) = scene();
        let pos = w.players[&owner].home;
        let guard = w.alloc_entity_id();
        let mut g = Fleet::single(guard, owner, ShipKind::Cruiser, pos, FleetOrder::Idle, None);
        g.reset_to(ShipKind::Cruiser, 2);
        w.fleets.insert(guard, g);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(StructureKind::DefensePlatform, 2);
        launch(&mut w, base, fleet);
        reach_home(&mut w, fleet);
        let e = w.engagements.values().find(|e| e.attackers.contains(&fleet)).expect("standing defense battles the raid");
        assert!(e.defenders.contains(&guard));
        assert_eq!(e.platform_system, Some(home));
        let mut battle_ended = false;
        for _ in 0..(240 * crate::config::TICK_HZ) {
            for event in w.step(&[]) {
                assert!(!matches!(event.payload, EventPayload::SystemPlundered { owner: o, .. } if o == owner));
                if matches!(event.payload, EventPayload::RaidResolved { attacker, .. } if attacker.is_pirate()) {
                    battle_ended = true;
                }
            }
            if battle_ended { break; }
        }
        assert!(battle_ended, "uses a real recorded battle, not a strength subtraction");
        assert!(w.fleets.contains_key(&guard));
        assert!(w.battle_records.values().any(|r| r.ended_tick.is_some()));
        assert!(w.pirate_home_raids.get(&fleet).is_none_or(|r| r.returning));
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().owner, Some(owner));
    }

    #[test]
    fn home_raid_freezes_per_good_allowance_protects_floor_and_survives_save_load() {
        let (mut w, _, home, base, fleet) = scene();
        launch(&mut w, base, fleet);
        // Isolate loading arithmetic; the separate full-step test covers travel.
        w.time = w.pirate_home_raids[&fleet].depart_at;
        w.fleets.get_mut(&fleet).unwrap().pos = w.pirate_home_raids[&fleet].pos;
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile
            = BTreeMap::from([(Commodity::Electronics, 50.0), (Commodity::MetallicOre, 41.0), (Commodity::Fuel, 40.0)]);
        w.tick_pirate_home_raids(&mut Vec::new());
        w.resolve_blockades(&mut Vec::new());
        assert_eq!(w.pirate_home_raids[&fleet].allowance[&Commodity::Electronics], 5);
        // An import cannot make THIS raid's ten-percent allowance grow.
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(Commodity::Electronics, 500.0);
        for _ in 0..35 { w.plunder_home_raid(fleet, &mut Vec::new()); }
        let saved = serde_json::to_string(&w).unwrap();
        let mut resumed: World = serde_json::from_str(&saved).unwrap();
        resumed.fixup_after_load();
        for _ in 0..300 {
            w.plunder_home_raid(fleet, &mut Vec::new());
            resumed.plunder_home_raid(fleet, &mut Vec::new());
        }
        assert_eq!(w.fleets[&fleet].cargo_units(), 6);
        let stock = &w.systems.iter().find(|s| s.id == home).unwrap().stockpile;
        assert_eq!(stock[&Commodity::Fuel], 40.0);
        assert_eq!(stock[&Commodity::MetallicOre], 40.0);
        assert_eq!(stock[&Commodity::Electronics], 495.0);
        assert_eq!(serde_json::to_value(&w.pirate_home_raids).unwrap(), serde_json::to_value(&resumed.pirate_home_raids).unwrap());
        assert_eq!(w.pirate_home_raid_after, resumed.pirate_home_raid_after);
        // Running out the clock while loading never creates a permanent siege.
        w.time += pirate::HOME_RAID_HOLD_BATTLE_MULT * w.config.battle_target_secs + DT;
        w.tick_pirate_home_raids(&mut Vec::new());
        assert!(w.pirate_home_raids[&fleet].returning);
    }

    #[test]
    fn home_raid_deadline_withdraws_through_the_recorded_battle_not_a_teleport() {
        let (mut w, owner, _, base, fleet) = scene();
        let guard = w.alloc_entity_id();
        w.fleets.insert(guard, Fleet::single(guard, owner, ShipKind::Raider,
            w.players[&owner].home, FleetOrder::Idle, None));
        launch(&mut w, base, fleet);
        reach_home(&mut w, fleet);
        w.step(&[]); // opens the real tactical state and recorder
        let battle = *w.engagements.keys().next().expect("raid is contested");
        let pos = w.fleets[&fleet].pos;
        // The deadline is mission state, independent of battle initialization.
        // Exercise a save that resumes with the raid's allowed hold exhausted.
        w.pirate_home_raids.get_mut(&fleet).unwrap().arrived_at = Some(w.time
            - pirate::HOME_RAID_HOLD_BATTLE_MULT * w.config.battle_target_secs);
        let mut events = Vec::new();
        w.tick_pirate_home_raids(&mut events);
        assert!(w.pirate_home_raids[&fleet].returning);
        assert_eq!(w.fleets[&fleet].pos, pos);
        assert!(w.engagements[&battle].tactical.as_ref().unwrap().withdrawing[0]);
        assert!(w.battle_records[&battle].ended_tick.is_none(), "must finish the exit animation/pursuit");
        assert_eq!(events.iter().filter(|e| matches!(e.payload, EventPayload::PirateRaidWithdrawn { .. })).count(), 1);
        for _ in 0..(240 * crate::config::TICK_HZ) {
            let events = w.step(&[]);
            assert!(!events.iter().any(|e| matches!(e.payload, EventPayload::SystemPlundered { .. })));
            if !w.engagements.contains_key(&battle) { break; }
        }
        assert!(!w.engagements.contains_key(&battle));
        assert!(w.battle_records[&battle].ended_tick.is_some());
    }

    #[test]
    fn clearing_the_source_aborts_a_home_raid_without_erasing_its_fleet() {
        let (mut w, owner, _, base, fleet) = scene();
        launch(&mut w, base, fleet);
        let from = w.fleets[&fleet].pos;
        let away = from + Vec2::new(2000.0, 0.0);
        w.fleets.get_mut(&fleet).unwrap().pos = away;
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Blockade {
            system: w.pirate_home_raids[&fleet].system, station: w.pirate_home_raids[&fleet].pos };
        w.systems.iter_mut().find(|s| s.id == base).unwrap().set_tier(StructureKind::DefensePlatform, 0);
        let victor = w.alloc_entity_id();
        w.fleets.insert(victor, Fleet::single(victor, owner, ShipKind::Cruiser, from, FleetOrder::Idle, None));
        w.pirate_ai(&mut Vec::new());
        assert!(!w.enclaves[&base].active(w.time));
        assert_eq!(w.fleets[&fleet].pos, away, "clearing a base cannot erase a remote pack");
        w.tick_pirate_home_raids(&mut Vec::new());
        assert!(!w.pirate_home_raids[&fleet].returning, "source destruction must not turn a remote pack FTL");
        let lost_at = w.pirate_home_raids[&fleet].source_lost_at.unwrap();
        w.time = lost_at + crate::transit::delay(from, away, w.config.c) - 1e-6;
        w.tick_pirate_home_raids(&mut Vec::new());
        assert!(!w.pirate_home_raids[&fleet].returning);
        w.time += 1e-6;
        w.tick_pirate_home_raids(&mut Vec::new());
        assert!(w.pirate_home_raids[&fleet].returning);
        assert_eq!(w.fleets[&fleet].pos, away);
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { dest } if dest == from));
    }
}
