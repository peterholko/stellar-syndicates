//! §belts phase 4: the Mining Belt Station (`docs/asteroid-belt-mining-plan.md`).
//! A slow ore depot that anchors at a charted belt in an unowned or allied
//! system. The corporation's barges at that belt unload into its hold, and its
//! freighters collect by hand (`StationLoad`) or on a freight route. Anchored,
//! it cannot move, merge or split; unanchoring takes as long as anchoring.
use super::*;
use crate::industry::{Anchor, AnchorPhase, ANCHOR_SECS, SKIM_RANGE_SU};

impl World {
    /// The owner's station anchored at a belt, and the room left in its hold.
    pub(super) fn anchored_station_at(&self, owner: PlayerId, system: EntityId, body: u32) -> Option<(EntityId, u32)> {
        self.fleets
            .iter()
            .find(|(_, f)| {
                f.owner == owner
                    && f.anchor.is_some_and(|a| a.phase == AnchorPhase::Anchored && a.system == system && a.body == body)
            })
            .map(|(id, f)| (*id, f.cargo_capacity().saturating_sub(f.cargo_units())))
    }

    /// Is `other` the owner or one of its syndicate allies?
    pub(super) fn friendly_to(&self, owner: PlayerId, other: PlayerId) -> bool {
        other == owner || self.allies_of(owner).contains(&other)
    }

    /// Why `fleet` may not anchor at belt `body` of `system`, or `None` when it
    /// may. The fleet must be one station on its own; the belt charted; the
    /// system unowned or an ally's (a colony stockpiles its own ore, and a
    /// rival's system is closed); and the corporation must not already have a
    /// station there. Rivals may anchor beside you. A pirate enclave is not
    /// refused: that would reveal it.
    pub fn anchor_refusal(&self, owner: PlayerId, fleet: EntityId, system: EntityId, body: u32) -> Option<&'static str> {
        let Some(f) = self.fleets.get(&fleet).filter(|f| f.owner == owner && !f.disposable) else {
            return Some("Fleet unavailable");
        };
        if f.count(ShipKind::MiningBeltStation) != 1 || f.total_count() != 1 {
            return Some("Only a lone Mining Belt Station anchors");
        }
        if f.anchor.is_some_and(|a| a.locked()) {
            return Some("The station is already anchored");
        }
        let Some(s) = self.systems.iter().find(|s| s.id == system) else {
            return Some("Unknown system");
        };
        if s.owner == Some(owner) {
            return Some("Your colony stockpiles its own ore");
        }
        if s.owner.is_some_and(|o| !self.friendly_to(owner, o)) {
            return Some("Another corporation holds this system");
        }
        let charted = self.players.get(&owner).is_some_and(|c| c.surveyed.contains(&system));
        if !charted {
            return Some("Survey the system first");
        }
        if !s.bodies.iter().any(|b| b.id == body && b.is_belt()) {
            return Some("No such belt");
        }
        if self.fleets.iter().any(|(id, g)| {
            *id != fleet && g.owner == owner && g.anchor.is_some_and(|a| a.system == system && a.body == body)
        }) {
            return Some("You already have a station at this belt");
        }
        None
    }

    pub(super) fn start_anchoring(
        &mut self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        body: u32,
        events: &mut Vec<Event>,
    ) {
        if self.anchor_refusal(owner, fleet, system, body).is_some() {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let Some(pos) = self.systems.iter().find(|s| s.id == system).map(|s| s.pos) else {
            return;
        };
        let now = self.time;
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.mission = None;
        f.defense = None;
        f.industry = None;
        f.anchor = Some(Anchor { system, body, phase: AnchorPhase::Approaching, since: now });
        f.order = FleetOrder::MoveTo { dest: pos };
    }

    /// Unanchor: a station on its way stops at once; an anchored one takes
    /// `ANCHOR_SECS` to come free.
    pub(super) fn start_unanchoring(&mut self, owner: PlayerId, fleet: EntityId, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(f) = self.fleets.get_mut(&fleet).filter(|f| f.owner == owner) else {
            return;
        };
        match f.anchor.map(|a| a.phase) {
            Some(AnchorPhase::Approaching) => {
                f.anchor = None;
                f.order = FleetOrder::Idle;
                tenders::hold(f);
            }
            Some(AnchorPhase::Anchoring | AnchorPhase::Anchored) => {
                let a = f.anchor.as_mut().unwrap();
                a.phase = AnchorPhase::Unanchoring;
                a.since = now;
            }
            _ => self.industry_reject(owner, fleet, events),
        }
    }

    /// One tick of every station's anchor. An approaching station anchors once
    /// it is within `SKIM_RANGE_SU` of the star; anchoring and unanchoring each
    /// take `ANCHOR_SECS`. When the system becomes the station's own colony,
    /// the station unloads into the new stockpile and comes free.
    pub(super) fn tick_belt_stations(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let ids: Vec<EntityId> = self.fleets.iter().filter(|(_, f)| f.anchor.is_some()).map(|(id, _)| *id).collect();
        for id in ids {
            let (owner, anchor, pos) = {
                let f = &self.fleets[&id];
                (f.owner, f.anchor.unwrap(), f.pos)
            };
            let Some((star, system_owner)) =
                self.systems.iter().find(|s| s.id == anchor.system).map(|s| (s.pos, s.owner))
            else {
                self.fleets.get_mut(&id).unwrap().anchor = None;
                continue;
            };
            let at = |phase| Anchor { phase, since: now, ..anchor };
            match anchor.phase {
                AnchorPhase::Approaching => {
                    let f = self.fleets.get_mut(&id).unwrap();
                    if pos.distance(star) <= SKIM_RANGE_SU {
                        f.order = FleetOrder::Idle;
                        tenders::hold(f);
                        f.anchor = Some(at(AnchorPhase::Anchoring));
                    } else if matches!(f.order, FleetOrder::Idle) {
                        f.order = FleetOrder::MoveTo { dest: star };
                    }
                }
                AnchorPhase::Anchoring | AnchorPhase::Anchored | AnchorPhase::Unanchoring => {
                    let f = self.fleets.get_mut(&id).unwrap();
                    f.order = FleetOrder::Idle;
                    tenders::hold(f);
                    let done = now - anchor.since + 1e-9 >= ANCHOR_SECS;
                    match anchor.phase {
                        AnchorPhase::Anchoring if done => {
                            f.anchor = Some(at(AnchorPhase::Anchored));
                            events.push(Event::new(now, EventPayload::StationAnchored {
                                owner, fleet: id, system: anchor.system, body: anchor.body, pos,
                            }));
                        }
                        AnchorPhase::Unanchoring if done => {
                            f.anchor = None;
                            events.push(Event::new(now, EventPayload::StationUnanchored {
                                owner, fleet: id, system: anchor.system, body: anchor.body, pos,
                            }));
                        }
                        _ => {}
                    }
                    if anchor.phase == AnchorPhase::Anchored && system_owner == Some(owner) {
                        self.release_station_to_colony(id, anchor, events);
                    }
                }
            }
        }
    }

    /// §belts: the station's belt became the corporation's own colony. The
    /// station unloads into the new stockpile, as far as storage allows, and
    /// comes free to move on; whatever doesn't fit stays aboard.
    fn release_station_to_colony(&mut self, id: EntityId, anchor: Anchor, events: &mut Vec<Event>) {
        let now = self.time;
        let (owner, pos, carried) = {
            let f = &self.fleets[&id];
            (f.owner, f.pos, f.cargo_stacks())
        };
        let mut moved = 0u32;
        for cargo in carried {
            let Some(s) = self.systems.iter_mut().find(|s| s.id == anchor.system) else {
                break;
            };
            let n = (cargo.units as f64).min(s.storage_headroom().max(0.0)).floor() as u32;
            if n == 0 {
                continue;
            }
            *s.stockpile.entry(cargo.commodity).or_insert(0.0) += n as f64;
            self.fleets.get_mut(&id).unwrap().remove_cargo(cargo.commodity, n);
            moved += n;
        }
        self.fleets.get_mut(&id).unwrap().anchor = None;
        events.push(Event::new(now, EventPayload::StationReleased {
            owner, fleet: id, system: anchor.system, units: moved, pos,
        }));
    }

    /// §belts: LOAD a freighter from a station by hand. Both are the player's;
    /// the freighter is idle within `DOCK_RADIUS` of the station and neither is
    /// in a battle. Takes up to `units` of what the station holds, as far as
    /// the hold allows.
    pub(super) fn apply_station_load(
        &mut self,
        (owner, fleet, station): (PlayerId, EntityId, EntityId),
        commodity: crate::cargo::Commodity,
        units: u32,
        events: &mut Vec<Event>,
    ) {
        let engaged = |w: &World, id: EntityId| {
            w.engagements.values().any(|e| e.attackers.contains(&id) || e.defenders.contains(&id))
        };
        let ready = fleet != station
            && self.fleets.get(&station).is_some_and(|s| s.owner == owner && s.contains(ShipKind::MiningBeltStation))
            && self.fleets.get(&fleet).is_some_and(|f| {
                f.owner == owner
                    && !f.disposable
                    && !f.contains(ShipKind::MiningBeltStation)
                    && matches!(f.order, FleetOrder::Idle)
                    && f.pos.distance(self.fleets[&station].pos) <= crate::ship::DOCK_RADIUS
            })
            && !engaged(self, fleet)
            && !engaged(self, station);
        if !ready {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let f = &self.fleets[&fleet];
        let room = f.cargo_capacity().saturating_sub(f.cargo_units());
        let n = units.min(room).min(self.fleets[&station].cargo_amount(commodity));
        if n == 0 {
            self.industry_reject(owner, fleet, events);
            return;
        }
        self.fleets.get_mut(&station).unwrap().remove_cargo(commodity, n);
        self.fleets.get_mut(&fleet).unwrap().add_cargo(commodity, n);
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.mining.station_hauled += n as f64;
        }
        let pos = self.fleets[&station].pos;
        events.push(Event::new(self.time, EventPayload::StationLoaded {
            owner, station, fleet, commodity, units: n, pos,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Commodity as C;
    use crate::industry::{FleetIndustry, FreightPort, FreightRoute, RouteStop};

    /// A home of the owner's and an unowned, charted belt with ore; a lone
    /// station and a barge parked at the belt's star.
    fn scene() -> (World, PlayerId, EntityId, (EntityId, u32, C), EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(812, 4));
        let owner = PlayerId(911);
        w.step(&[Command::AddPlayer { id: owner, name: "Belt depot".into() }]);
        w.enclaves.clear();
        w.fleets.clear();
        w.nebulas.clear();
        w.engagements.clear();
        let home = w.players[&owner].home_system.unwrap();
        let (system, body, ore) = w
            .systems
            .iter()
            .filter(|s| s.owner.is_none() && s.blockade.is_none())
            .find_map(|s| {
                s.bodies.iter().find(|b| b.is_belt() && !b.deposits.is_empty()).map(|b| (s.id, b.id, b.deposits[0].resource))
            })
            .unwrap();
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        let pos = w.systems.iter().find(|s| s.id == system).unwrap().pos;
        let station = w.alloc_entity_id();
        w.fleets.insert(station, Fleet::single(station, owner, ShipKind::MiningBeltStation, pos, FleetOrder::Idle, None));
        let barge = w.alloc_entity_id();
        w.fleets.insert(barge, Fleet::single(barge, owner, ShipKind::MiningBarge, pos, FleetOrder::Idle, None));
        (w, owner, home, (system, body, ore), station, barge)
    }
    fn pump(w: &mut World, seconds: f64, events: &mut Vec<Event>) {
        for _ in 0..(seconds / DT).round() as u32 {
            w.time += DT;
            w.tick += 1;
            w.tick_industrial_expansion(events);
            w.tick_belt_stations(events);
        }
    }
    fn anchored(w: &mut World, owner: PlayerId, station: EntityId, (system, body): (EntityId, u32)) {
        w.start_anchoring(owner, station, system, body, &mut vec![]);
        pump(w, ANCHOR_SECS + 1.0, &mut vec![]);
        assert_eq!(w.fleets[&station].anchor.map(|a| a.phase), Some(AnchorPhase::Anchored));
    }

    #[test]
    fn a_station_anchors_in_a_minute_and_takes_its_barges_ore() {
        let (mut w, owner, _, (system, body, ore), station, barge) = scene();
        let mut events = Vec::new();
        w.start_anchoring(owner, station, system, body, &mut events);
        assert_eq!(w.fleets[&station].anchor.map(|a| a.phase), Some(AnchorPhase::Approaching));
        pump(&mut w, 1.0, &mut events);
        assert_eq!(w.fleets[&station].anchor.map(|a| a.phase), Some(AnchorPhase::Anchoring));
        pump(&mut w, ANCHOR_SECS, &mut events);
        assert_eq!(w.fleets[&station].anchor.map(|a| a.phase), Some(AnchorPhase::Anchored));
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::StationAnchored { fleet, .. } if fleet == station)));

        w.start_belt_mining(owner, barge, system, body, (ore, false), &mut vec![]);
        pump(&mut w, 60.0, &mut vec![]);
        let held = w.fleets[&station].cargo_amount(ore);
        assert!(held > 0, "the barge unloads into the station");
        assert_eq!(w.fleets[&barge].cargo_units(), 0, "and keeps its own hold empty");
        assert!(matches!(&w.fleets[&barge].industry, Some(FleetIndustry::Mine { to_station: true, status, .. }) if status == "mining"));

        // A full station overflows into the barge's hold, then both stop.
        let room = w.fleets[&station].cargo_capacity() - w.fleets[&station].cargo_units();
        w.fleets.get_mut(&station).unwrap().add_cargo(ore, room);
        let free = w.fleets[&barge].cargo_capacity();
        w.fleets.get_mut(&barge).unwrap().add_cargo(ore, free - 1);
        pump(&mut w, 120.0, &mut vec![]);
        assert_eq!(w.fleets[&barge].cargo_units(), w.fleets[&barge].cargo_capacity());
        assert!(matches!(&w.fleets[&barge].industry, Some(FleetIndustry::Mine { status, .. }) if status == "station full"));
    }

    #[test]
    fn an_anchored_station_stays_put_until_it_unanchors() {
        let (mut w, owner, _, (system, body, _), station, barge) = scene();
        anchored(&mut w, owner, station, (system, body));
        let mut events = Vec::new();
        assert!(!w.fleet_unanchored_for_orders(station, owner, &mut events), "movement is refused");
        assert!(events.iter().any(|e| matches!(e.payload,
            EventPayload::OrderRejected { reason: crate::event::OrderRejectReason::StationAnchored, .. })));
        w.fleets.get_mut(&station).unwrap().add_cargo(C::MetallicOre, 10);
        w.apply_now(&Command::HaulToMarketHub { player_id: owner, fleet_id: station, sell_on_arrival: false }, &mut vec![]);
        assert!(matches!(w.fleets[&station].order, FleetOrder::Idle), "a haul can't drag it off the belt");
        w.apply_merge_fleets(owner, barge, station, &mut []);
        assert!(w.fleets.contains_key(&station), "nor can a merge");

        w.start_unanchoring(owner, station, &mut vec![]);
        assert_eq!(w.fleets[&station].anchor.map(|a| a.phase), Some(AnchorPhase::Unanchoring));
        pump(&mut w, ANCHOR_SECS - 1.0, &mut vec![]);
        assert!(w.fleets[&station].anchor.is_some(), "unanchoring takes a minute");
        pump(&mut w, 2.0, &mut events);
        assert!(w.fleets[&station].anchor.is_none());
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::StationUnanchored { .. })));
        assert!(w.fleet_unanchored_for_orders(station, owner, &mut vec![]), "free to move again");
    }

    #[test]
    fn stations_anchor_alone_in_charted_unowned_or_allied_systems_once_a_belt() {
        let (mut w, owner, home, (system, body, _), station, barge) = scene();
        assert_eq!(w.anchor_refusal(owner, barge, system, body), Some("Only a lone Mining Belt Station anchors"));
        let home_belt = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| b.is_belt()).unwrap().id;
        assert_eq!(w.anchor_refusal(owner, station, home, home_belt), Some("Your colony stockpiles its own ore"));
        w.players.get_mut(&owner).unwrap().surveyed.remove(&system);
        assert_eq!(w.anchor_refusal(owner, station, system, body), Some("Survey the system first"));
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        w.systems.iter_mut().find(|s| s.id == system).unwrap().owner = Some(PlayerId(4_242));
        assert_eq!(w.anchor_refusal(owner, station, system, body), Some("Another corporation holds this system"));
        w.systems.iter_mut().find(|s| s.id == system).unwrap().owner = None;
        assert_eq!(w.anchor_refusal(owner, station, system, body), None);
        anchored(&mut w, owner, station, (system, body));
        let second = w.alloc_entity_id();
        let pos = w.fleets[&station].pos;
        w.fleets.insert(second, Fleet::single(second, owner, ShipKind::MiningBeltStation, pos, FleetOrder::Idle, None));
        assert_eq!(w.anchor_refusal(owner, second, system, body), Some("You already have a station at this belt"));
        let rival = PlayerId(5_151);
        w.players.insert(rival, w.players[&owner].clone());
        let theirs = w.alloc_entity_id();
        w.fleets.insert(theirs, Fleet::single(theirs, rival, ShipKind::MiningBeltStation, pos, FleetOrder::Idle, None));
        assert_eq!(w.anchor_refusal(rival, theirs, system, body), None, "rivals may anchor beside you");
    }

    #[test]
    fn freighters_collect_from_a_station_by_hand_and_on_a_route() {
        let (mut w, owner, home, (system, body, ore), station, barge) = scene();
        w.fleets.remove(&barge);
        anchored(&mut w, owner, station, (system, body));
        w.fleets.get_mut(&station).unwrap().add_cargo(ore, 300);
        let pos = w.fleets[&station].pos;
        let hauler = w.alloc_entity_id();
        w.fleets.insert(hauler, Fleet::single(hauler, owner, ShipKind::Convoy, pos + Vec2::new(100.0, 0.0), FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.apply_station_load((owner, hauler, station), ore, 120, &mut events);
        assert_eq!(w.fleets[&hauler].cargo_amount(ore), 120);
        assert_eq!(w.fleets[&station].cargo_amount(ore), 180);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::StationLoaded { units: 120, .. })));
        w.fleets.get_mut(&hauler).unwrap().pos = pos + Vec2::new(5_000.0, 0.0);
        w.apply_station_load((owner, hauler, station), ore, 10, &mut vec![]);
        assert_eq!(w.fleets[&hauler].cargo_amount(ore), 120, "only a freighter beside the station loads");

        // A route stop at the station only loads, straight from its hold.
        w.fleets.get_mut(&hauler).unwrap().pos = pos;
        w.fleets.get_mut(&hauler).unwrap().take_cargo();
        let stop = |port, load: &[(C, u32)], unload: &[(C, u32)]| RouteStop {
            port, load: load.iter().copied().collect(), unload: unload.iter().copied().collect(), sell: false,
        };
        let route = |stops| FreightRoute {
            name: "Belt run".into(), stops, repeat: true, fuel_reserve: 0.0, escort: None,
            transit: Default::default(), shopping_budget: None,
        };
        let bad = route(vec![stop(FreightPort::Station { id: station }, &[], &[(ore, 10)]), stop(FreightPort::System { id: home }, &[], &[])]);
        w.set_freight_route(owner, hauler, Some(bad), &mut vec![]);
        assert!(w.fleets[&hauler].industry.is_none(), "a station stop can't take deliveries");
        let good = route(vec![stop(FreightPort::Station { id: station }, &[(ore, 150)], &[]),
            stop(FreightPort::System { id: home }, &[], &[(ore, 150)])]);
        w.set_freight_route(owner, hauler, Some(good), &mut vec![]);
        assert!(w.fleets[&hauler].industry.is_some());
        pump(&mut w, 1.0, &mut vec![]);
        assert_eq!(w.fleets[&hauler].cargo_amount(ore), 150, "the route takes its load from the station");
        assert_eq!(w.fleets[&station].cargo_amount(ore), 30);
        assert_eq!(w.players[&owner].warehouse.get(&ore).copied().unwrap_or(0), 0, "never from the Hub warehouse");
    }

    #[test]
    fn when_the_belt_becomes_your_colony_the_station_unloads_and_comes_free() {
        let (mut w, owner, _, (system, body, ore), station, _) = scene();
        anchored(&mut w, owner, station, (system, body));
        w.fleets.get_mut(&station).unwrap().add_cargo(ore, 200);
        let s = w.systems.iter_mut().find(|s| s.id == system).unwrap();
        s.owner = Some(owner);
        s.seed_warehouse();
        let mut events = Vec::new();
        pump(&mut w, DT, &mut events);
        assert!(w.fleets[&station].anchor.is_none(), "free to move on");
        let stocked = w.systems.iter().find(|s| s.id == system).unwrap().stockpile.get(&ore).copied().unwrap_or(0.0);
        assert!(stocked > 0.0 && (stocked + w.fleets[&station].cargo_amount(ore) as f64 - 200.0).abs() < 1e-9,
            "what fits lands in the stockpile; the rest stays aboard");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::StationReleased { .. })));
    }
}
