//! Onboard routes and local industry. No remote inventory/ownership read can
//! dispatch a leg: cargo work happens at a real berth, and progress is carried
//! in the same immutable fleet/site reports as all other physical work.
use super::*;
use crate::Commodity as C;
use crate::industry::FreightRun;
use crate::industry::*;

impl World {
    pub(super) fn industry_reject(&self, owner: PlayerId, fleet: EntityId, events: &mut Vec<Event>) {
        events.push(Event::new(
            self.time,
            EventPayload::OrderRejected {
                owner,
                fleet,
                target: None,
                reason: crate::event::OrderRejectReason::DeliveryConditionsChanged,
            },
        ));
    }
    fn freight_port_pos(&self, port: FreightPort) -> Option<Vec2> {
        match port {
            FreightPort::Hub => Some(self.hub),
            FreightPort::System { id } => self.systems.iter().find(|s| s.id == id).map(|s| s.pos),
            FreightPort::Station { id } => self.fleets.get(&id).map(|f| f.pos),
        }
    }
    /// §belts: a station stop names one of the route owner's stations.
    fn owns_station(&self, owner: PlayerId, id: EntityId) -> bool {
        self.fleets.get(&id).is_some_and(|f| f.owner == owner && f.contains(ShipKind::MiningBeltStation))
    }
    pub(super) fn set_freight_route(
        &mut self,
        owner: PlayerId,
        id: EntityId,
        route: Option<FreightRoute>,
        events: &mut Vec<Event>,
    ) {
        if !self
            .fleets
            .get(&id)
            .is_some_and(|f| f.owner == owner && !f.disposable && f.has_freighter())
        {
            self.industry_reject(owner, id, events);
            return;
        }
        let Some(route) = route else {
            let f = self.fleets.get_mut(&id).unwrap();
            f.industry = None;
            f.mission = None;
            f.order = FleetOrder::Idle;
            tenders::hold(f);
            return;
        };
        let capacity = self.fleets[&id].cargo_capacity() as u64;
        let return_trip = !route.repeat && route.stops.len() == 3
            && route.stops[0].port == route.stops[2].port;
        let valid = (2..=MAX_STOPS).contains(&route.stops.len())
            && (route.transit != crate::ship::TransitMode::Slow
                || self.fleets[&id].can_slow())
            && !route.name.trim().is_empty()
            && route.name.chars().count() <= 48
            && route.fuel_reserve.is_finite()
            && route.fuel_reserve >= 0.0
            && route.fuel_reserve <= self.fleets[&id].fuel_capacity()
            && route.escort != Some(id)
            && route.shopping_budget.is_none_or(|budget| budget.is_finite() && (0.0..=1_000_000_000.0).contains(&budget))
            && (route.stops.len() <= 2
                || return_trip
                || self.research_flag(owner, crate::research::Cap::AutonomousFreight))
            && route.stops.iter().all(|s| {
                self.freight_port_pos(s.port).is_some()
                    && (!s.sell || s.port == FreightPort::Hub)
                    && match s.port {
                        FreightPort::Station { id } => self.owns_station(owner, id) && s.unload.is_empty(),
                        _ => true,
                    }
                    && s.load.values().map(|n| *n as u64).sum::<u64>() <= capacity
                    && s.unload.values().map(|n| *n as u64).sum::<u64>() <= capacity
            })
            && route.stops.windows(2).all(|s| s[0].port != s[1].port);
        if !valid {
            self.industry_reject(owner, id, events);
            return;
        }
        let f = self.fleets.get_mut(&id).unwrap();
        f.transit = route.transit;
        f.mission = None;
        f.defense = None;
        f.pursuit_plan = None;
        f.industry = Some(FleetIndustry::Route {
            run: FreightRun {
                route,
                stop: 0,
                phase: RoutePhase::Dispatch,
                remaining_load: Default::default(),
                remaining_unload: Default::default(),
                visits: 0,
                hold: None,
                shopping_spent: 0.0,
            },
        });
        f.order = FleetOrder::Idle;
    }
    pub(super) fn reserve_project(
        &mut self,
        owner: PlayerId,
        id: EntityId,
        target: ProjectTarget,
        reserve: bool,
    ) {
        let Some(s) = self
            .systems
            .iter_mut()
            .find(|s| s.id == id && s.owner == Some(owner))
        else {
            return;
        };
        s.industry.reservations.retain(|r| r.target != target);
        if reserve && s.industry.reservations.len() < MAX_RESERVATIONS {
            s.industry.reservations.push(Reservation {
                target,
                goods: target.costs().iter().copied().collect(),
            });
        }
    }
    pub(super) fn deploy_outpost(
        &mut self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        body: u32,
        kind: OutpostKind,
        commodity: C,
        events: &mut Vec<Event>,
    ) {
        let site = self.systems.iter().find(|s| s.id == system);
        let valid = site.is_some_and(|s| {
            s.owner.is_none()
                && !self.enclaves.contains_key(&system)
                && !s.bodies.iter().any(|b| b.habitable)
                && s.bodies.iter().any(|b| {
                    // §belts: outposts stay on planets for now.
                    b.id == body
                        && !b.is_belt()
                        && (kind == OutpostKind::Research
                            || b.deposits.iter().any(|d| d.resource == commodity))
                })
        }) && self
            .players
            .get(&owner)
            .is_some_and(|c| c.surveyed.contains(&system) && c.founding.expansion_unlocked())
            && self.fleets.get(&fleet).is_some_and(|f| {
                f.owner == owner
                    && f.has_freighter()
                    && outpost_costs()
                        .iter()
                        .all(|(c, n)| f.cargo_amount(*c) >= *n)
            });
        if !valid {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let pos = site.unwrap().pos;
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.mission = None;
        f.defense = None;
        f.industry = Some(FleetIndustry::Deploy {
            system,
            body,
            outpost: kind,
            commodity,
            work: 0.0,
        });
        f.order = FleetOrder::MoveTo { dest: pos };
    }
    pub(super) fn start_skimming(
        &mut self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        events: &mut Vec<Event>,
    ) {
        let site = self.systems.iter().find(|s| {
            s.id == system && s.bodies.iter().any(|b| b.kind == crate::BodyKind::GasGiant)
        });
        if site.is_none()
            || !self.research_flag(owner, crate::research::Cap::Ramscoop)
            || !self
                .fleets
                .get(&fleet)
                .is_some_and(|f| f.owner == owner && !f.disposable)
        {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let pos = site.unwrap().pos;
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.mission = None;
        f.defense = None;
        f.industry = Some(FleetIndustry::Skim {
            system,
            harvested: 0.0,
            cargo_fraction: 0.0,
            status: "travelling".into(),
        });
        f.order = FleetOrder::MoveTo { dest: pos };
    }
    /// §belts: why `fleet` may not mine `ore` from belt `body` of `system`,
    /// or `None` when it may. The system must be unowned, yours or an ally's,
    /// and charted (surveyed or owned); the belt must hold the ore, or
    /// (§pockets) hide a pocket of it that your corporation has found. A pirate
    /// enclave is not refused: that would reveal it, and the barge learns the
    /// hard way.
    pub fn belt_mining_refusal(
        &self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        body: u32,
        (ore, pocket): (C, bool),
    ) -> Option<&'static str> {
        let Some(f) = self.fleets.get(&fleet).filter(|f| f.owner == owner && !f.disposable) else {
            return Some("Fleet unavailable");
        };
        if !f.contains(ShipKind::MiningBarge) {
            return Some("Only Mining Barges mine belts");
        }
        let Some(s) = self.systems.iter().find(|s| s.id == system) else {
            return Some("Unknown system");
        };
        if s.owner.is_some_and(|o| !self.friendly_to(owner, o)) {
            return Some("Another corporation holds this system");
        }
        let charted = s.owner == Some(owner)
            || self.players.get(&owner).is_some_and(|c| c.surveyed.contains(&system));
        if !charted {
            return Some("Survey the system first");
        }
        let Some(belt) = s.bodies.iter().find(|b| b.id == body && b.is_belt()) else {
            return Some("No such belt");
        };
        if pocket {
            if belt.found_pocket(ore, owner).is_none() {
                return Some("No pocket of that ore found");
            }
        } else if !belt.deposits.iter().any(|d| d.resource == ore) {
            return Some("The belt holds no such ore");
        }
        None
    }

    pub(super) fn start_belt_mining(
        &mut self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        body: u32,
        (ore, pocket): (C, bool),
        events: &mut Vec<Event>,
    ) {
        if self.belt_mining_refusal(owner, fleet, system, body, (ore, pocket)).is_some() {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let Some(pos) = self.systems.iter().find(|s| s.id == system).map(|s| s.pos) else {
            return;
        };
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.mission = None;
        f.defense = None;
        f.industry = Some(FleetIndustry::Mine {
            system,
            body,
            ore,
            pocket,
            to_station: false,
            mined: 0.0,
            fraction: 0.0,
            status: "travelling".into(),
        });
        f.order = FleetOrder::MoveTo { dest: pos };
    }

    /// §belts: a battle near the star or a blockade of the system makes belt
    /// work unsafe: it pauses until the danger passes.
    fn belt_unsafe(&self, s: &StarSystem) -> bool {
        s.blockade.is_some()
            || self.engagements.values().any(|e| {
                e.pos.distance(s.pos) <= SKIM_RANGE_SU
                    && e.started_at + crate::transit::delay(e.pos, s.pos, self.config.c) <= self.time
            })
    }

    /// §belts: one tick of a barge fleet's belt job. Ore goes to the stockpile
    /// when the system is yours, else into your station anchored at the belt,
    /// else into the fleet's hold. Every barge on a deposit shares
    /// `SHIP_YIELD_MULT` × its natural rate (`crews` counts them as of last
    /// tick), and finite reserves deplete. §pockets: `pocket` works a found
    /// pocket of the ore, and cutting may strike a hidden one.
    #[allow(clippy::too_many_arguments)]
    fn step_belt_mining(
        &mut self,
        id: EntityId,
        owner: PlayerId,
        (system, body, ore, pocket): (EntityId, u32, C, bool),
        mut mined: f64,
        mut fraction: f64,
        crews: &BTreeMap<(EntityId, u32, C, bool), u32>,
        events: &mut Vec<Event>,
    ) {
        let barges = self.fleets[&id].count(ShipKind::MiningBarge);
        let Some(s) = self.systems.iter().find(|s| s.id == system) else {
            self.fleets.get_mut(&id).unwrap().industry = None;
            return;
        };
        if barges == 0 {
            self.fleets.get_mut(&id).unwrap().industry = None;
            return;
        }
        let pos = s.pos;
        if self.fleets[&id].pos.distance(pos) > SKIM_RANGE_SU {
            return; // still on the way
        }
        let hostile = self.belt_unsafe(s);
        let deposit = s.bodies.iter().find(|b| b.id == body).and_then(|b| {
            let d = if pocket {
                b.found_pocket(ore, owner).map(|p| &p.deposit)
            } else {
                b.deposits.iter().find(|d| d.resource == ore)
            }?;
            Some((crate::explore::natural_extraction_rate(b, d, s.trait_), d.reserves))
        });
        let own = s.owner == Some(owner);
        let claimed = s.owner.is_some_and(|o| !self.friendly_to(owner, o));
        let headroom = if own { s.storage_headroom().max(0.0) } else { 0.0 };
        let mods = self.production_mods(owner);
        // §belts: your station anchored at this belt takes the ore next.
        let station = self.anchored_station_at(owner, system, body);
        let station_room = station.map_or(0, |(_, room)| room);
        let f = self.fleets.get_mut(&id).unwrap();
        f.order = FleetOrder::Idle;
        tenders::hold(f);
        let free = f.cargo_capacity().saturating_sub(f.cargo_units());
        let mut cut = 0.0;
        let mut to_stock = 0.0;
        let mut to_station = 0;
        let status = match deposit {
            _ if claimed => "claimed",
            _ if hostile => "unsafe",
            None => "depleted",
            Some((_, Some(reserves))) if reserves <= 1e-9 => "depleted",
            Some((natural, reserves)) => {
                let crew = crews.get(&(system, body, ore, pocket)).copied().unwrap_or(0).max(barges) as f64;
                let share = (SHIP_YIELD_MULT * natural / (crew * BARGE_ORE_PER_S)).min(1.0);
                let placeable = headroom + (station_room as f64 + free as f64 - fraction).max(0.0);
                cut = (barges as f64
                    * BARGE_ORE_PER_S
                    * share
                    * crate::production::ore_bulk_ratio(ore)
                    * mods.extraction
                    * mods.extraction_moons
                    * DT)
                    .min(placeable)
                    .min(reserves.unwrap_or(f64::INFINITY));
                if cut <= 1e-12 {
                    if station.is_some() { "station full" } else { "full" }
                } else {
                    to_stock = cut.min(headroom);
                    fraction += cut - to_stock;
                    // A whole unit lands even when float dust leaves it a hair
                    // short: the station first, then the barge's own hold.
                    let units = ((fraction + 1e-9).floor() as u32).min(station_room + free);
                    to_station = units.min(station_room);
                    let to_hold = units - to_station;
                    if to_hold > 0 {
                        f.add_cargo(ore, to_hold);
                    }
                    fraction -= units as f64;
                    mined += cut;
                    "mining"
                }
            }
        };
        f.industry = Some(FleetIndustry::Mine {
            system,
            body,
            ore,
            pocket,
            to_station: station_room > 0,
            mined,
            fraction,
            status: status.into(),
        });
        if to_station > 0
            && let Some((station_id, _)) = station
        {
            self.fleets.get_mut(&station_id).unwrap().add_cargo(ore, to_station);
        }
        if cut <= 0.0 {
            return;
        }
        if let Some(s) = self.systems.iter_mut().find(|s| s.id == system) {
            if to_stock > 0.0 {
                *s.stockpile.entry(ore).or_insert(0.0) += to_stock;
            }
            let worked = s.bodies.iter_mut().find(|b| b.id == body).and_then(|b| {
                if pocket {
                    b.pockets
                        .iter_mut()
                        .find(|p| p.deposit.resource == ore && p.found_by.contains(&owner))
                        .map(|p| &mut p.deposit)
                } else {
                    b.deposits.iter_mut().find(|d| d.resource == ore)
                }
            });
            if let Some(reserves) = worked.and_then(|d| d.reserves.as_mut()) {
                *reserves = (*reserves - cut).max(0.0);
            }
        }
        if let Some(corp) = self.players.get_mut(&owner) {
            *corp.mining.barge.entry(ore).or_default() += cut;
            if pocket {
                *corp.mining.pocket.entry(ore).or_default() += cut;
            }
        }
        // §research R3: mined units count like a mine's, in bulk-equivalent units.
        let bulk = cut / crate::production::ore_bulk_ratio(ore);
        self.add_research_verb_at(pos, owner, crate::research::Verb::UnitsExtracted, bulk);
        self.add_research_verb_at(pos, owner, crate::research::Verb::UnitsThroughIndustry, bulk);
        self.roll_pocket_strike(id, owner, (system, body), barges, events);
    }

    /// §pockets: hidden pockets of a belt that `owner` has neither found nor
    /// has a find of on its way home.
    fn unfound_pockets(&self, owner: PlayerId, system: EntityId, body: u32) -> Vec<C> {
        let pending: Vec<C> = self
            .pending_pocket_reports
            .iter()
            .filter(|r| r.recipient == owner && r.system == system && r.body == body)
            .flat_map(|r| r.found.iter().copied())
            .collect();
        self.systems
            .iter()
            .find(|s| s.id == system)
            .and_then(|s| s.bodies.iter().find(|b| b.id == body))
            .map(|b| {
                b.pockets
                    .iter()
                    .filter(|p| !p.found_by.contains(&owner) && !pending.contains(&p.deposit.resource))
                    .map(|p| p.deposit.resource)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// §pockets: a barge that cut ore this tick may strike a hidden pocket —
    /// `STRIKE_CHANCE_PER_BARGE_HOUR` per barge. The find heads home by light.
    fn roll_pocket_strike(
        &mut self,
        fleet: EntityId,
        owner: PlayerId,
        (system, body): (EntityId, u32),
        barges: u32,
        events: &mut Vec<Event>,
    ) {
        // One draw per working tick, keyed by fleet and tick: cheap, and the
        // same whatever the belt holds.
        let mut rng = crate::rng::Rng::keyed_id(
            self.config.seed,
            "pocket-strike-v1",
            fleet.0.rotate_left(32) ^ self.tick,
        );
        let chance = STRIKE_CHANCE_PER_BARGE_HOUR * barges as f64 * DT / 3_600.0;
        if rng.next_f64() >= chance {
            return;
        }
        let unfound = self.unfound_pockets(owner, system, body);
        if unfound.is_empty() {
            return;
        }
        let ore = unfound[((rng.next_f64() * unfound.len() as f64) as usize).min(unfound.len() - 1)];
        let pos = self.fleets[&fleet].pos;
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.mining.strikes += 1;
        }
        events.push(Event::new(self.time, EventPayload::PocketStruck { owner, system, body, ore, pos }));
        self.send_pocket_report(owner, (system, body), vec![ore], pos, true);
    }

    /// §pockets: a find sets out for the corporation's command center at light
    /// speed. It is private: no ally relay.
    fn send_pocket_report(&mut self, owner: PlayerId, (system, body): (EntityId, u32), found: Vec<C>, pos: Vec2, strike: bool) {
        if let Some(cc) = self.players.get(&owner).map(|p| p.command_center) {
            self.pending_pocket_reports.push(crate::world::PocketReport {
                recipient: owner,
                system,
                body,
                found,
                arrive_at: self.time + crate::transit::delay(pos, cc, self.config.c),
                strike,
            });
        }
    }

    /// §pockets: why `fleet` may not prospect belt `body` of `system`, or
    /// `None` when it may. Any charted belt is fair game, as for a survey; the
    /// fleet needs a Scout or a Mining Barge.
    pub fn prospect_refusal(&self, owner: PlayerId, fleet: EntityId, system: EntityId, body: u32) -> Option<&'static str> {
        let Some(f) = self.fleets.get(&fleet).filter(|f| f.owner == owner && !f.disposable) else {
            return Some("Fleet unavailable");
        };
        if !(f.contains(ShipKind::Scout) || f.contains(ShipKind::MiningBarge)) {
            return Some("Only Scouts and Mining Barges prospect");
        }
        let Some(s) = self.systems.iter().find(|s| s.id == system) else {
            return Some("Unknown system");
        };
        let charted = s.owner == Some(owner)
            || self.players.get(&owner).is_some_and(|c| c.surveyed.contains(&system));
        if !charted {
            return Some("Survey the system first");
        }
        if !s.bodies.iter().any(|b| b.id == body && b.is_belt()) {
            return Some("No such belt");
        }
        None
    }

    pub(super) fn start_prospecting(
        &mut self,
        owner: PlayerId,
        fleet: EntityId,
        system: EntityId,
        body: u32,
        events: &mut Vec<Event>,
    ) {
        if self.prospect_refusal(owner, fleet, system, body).is_some() {
            self.industry_reject(owner, fleet, events);
            return;
        }
        let Some(pos) = self.systems.iter().find(|s| s.id == system).map(|s| s.pos) else {
            return;
        };
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.mission = None;
        f.defense = None;
        f.industry = Some(FleetIndustry::Prospect { system, body, dwell: 0.0, status: "travelling".into() });
        f.order = FleetOrder::MoveTo { dest: pos };
    }

    /// §pockets: one tick of a prospecting pass. On station the dwell banks;
    /// leaving station restarts the pass and danger pauses it. At
    /// `PROSPECT_SECS` every hidden pocket is rolled at `POCKET_FIND_CHANCE`,
    /// keyed per (belt, corporation, pass), and the report heads home by light.
    fn step_prospecting(
        &mut self,
        id: EntityId,
        owner: PlayerId,
        (system, body): (EntityId, u32),
        mut dwell: f64,
        events: &mut Vec<Event>,
    ) {
        let Some(s) = self.systems.iter().find(|s| s.id == system) else {
            self.fleets.get_mut(&id).unwrap().industry = None;
            return;
        };
        let f = &self.fleets[&id];
        if !(f.contains(ShipKind::Scout) || f.contains(ShipKind::MiningBarge)) {
            self.fleets.get_mut(&id).unwrap().industry = None;
            return;
        }
        let pos = s.pos;
        let travelling = f.pos.distance(pos) > SKIM_RANGE_SU;
        let hostile = !travelling && self.belt_unsafe(s);
        let f = self.fleets.get_mut(&id).unwrap();
        let status = if travelling {
            dwell = 0.0;
            "travelling"
        } else {
            f.order = FleetOrder::Idle;
            tenders::hold(f);
            if hostile {
                "unsafe"
            } else {
                dwell += DT;
                "prospecting"
            }
        };
        if dwell + 1e-9 < PROSPECT_SECS {
            f.industry = Some(FleetIndustry::Prospect { system, body, dwell, status: status.into() });
            return;
        }
        f.industry = None;
        let at = f.pos;
        let unfound = self.unfound_pockets(owner, system, body);
        let seed = self.config.seed;
        let Some(belt) = self
            .systems
            .iter_mut()
            .find(|s| s.id == system)
            .and_then(|s| s.bodies.iter_mut().find(|b| b.id == body))
        else {
            return;
        };
        let record = belt.prospected.entry(owner).or_default();
        let pass = record.passes;
        record.passes += 1;
        let mut rng = crate::rng::Rng::keyed(
            seed,
            &format!("pocket-prospect-v1:{}:{}:{}:{}", system.0, body, owner.0, pass),
        );
        // Every pocket draws, found or not, so earlier finds never shift a roll.
        let found: Vec<C> = belt
            .pockets
            .iter()
            .filter_map(|p| {
                let hit = rng.next_f64() < POCKET_FIND_CHANCE;
                (hit && unfound.contains(&p.deposit.resource)).then_some(p.deposit.resource)
            })
            .collect();
        self.grant_captain_xp(owner, id, crate::captain::CAPTAIN_XP_SURVEY);
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.mining.prospect_passes += 1;
            corp.mining.pockets_found += found.len() as u32;
        }
        events.push(Event::new(
            self.time,
            EventPayload::ProspectCompleted { owner, system, body, found: found.clone(), pos: at },
        ));
        self.send_pocket_report(owner, (system, body), found, at, false);
    }

    /// §pockets: deliver prospecting and strike reports whose light has reached
    /// command. Each find joins the corporation's `found_by`, and a pass counts
    /// toward the belt's "Prospected N×".
    pub(super) fn deliver_pocket_reports(&mut self) {
        let now = self.time;
        let mut i = 0;
        while i < self.pending_pocket_reports.len() {
            if self.pending_pocket_reports[i].arrive_at > now {
                i += 1;
                continue;
            }
            let r = self.pending_pocket_reports.remove(i);
            let Some(belt) = self
                .systems
                .iter_mut()
                .find(|s| s.id == r.system)
                .and_then(|s| s.bodies.iter_mut().find(|b| b.id == r.body))
            else {
                continue;
            };
            for p in belt.pockets.iter_mut().filter(|p| r.found.contains(&p.deposit.resource)) {
                p.found_by.insert(r.recipient);
            }
            if !r.strike {
                belt.prospected.entry(r.recipient).or_default().reported += 1;
            }
        }
    }

    pub(super) fn start_colony_project(
        &mut self,
        owner: PlayerId,
        id: EntityId,
        body: u32,
        kind: ColonyProjectKind,
        commodity: C,
    ) -> Result<(), &'static str> {
        // §fuel-overhaul: Orbital Assembly makes Warp Cores, which colonies
        // learn to build with Fusion Reactors.
        let fusion = self.players.get(&owner)
            .is_some_and(|c| c.research.has(crate::power::FUSION_RESEARCH));
        let Some(s) = self
            .systems
            .iter_mut()
            .find(|s| s.id == id && s.owner == Some(owner))
        else {
            return Err("Site unavailable");
        };
        let Some(b) = s.bodies.iter().find(|b| b.id == body) else {
            return Err("World unavailable");
        };
        let suitable = match kind {
            ColonyProjectKind::OrbitalAssembly => fusion && b.tier(crate::StructureKind::Shipyard) >= 2,
            ColonyProjectKind::AgriculturalExport => {
                b.habitable && b.deposits.iter().any(|d| d.resource == C::Biomass)
            }
            ColonyProjectKind::DeepExtraction => b.deposits.iter().any(|d| {
                d.resource == commodity
                    && commodity.is_mineable_mineral()
            }),
        };
        let target = ProjectTarget::Development { project: kind };
        if !suitable || s.industry.outpost.is_some() {
            return Err("This world cannot support that project");
        }
        if s.blockade_held() {
            return Err("Construction access blocked");
        }
        if s.industry
            .projects
            .iter()
            .any(|p| p.kind == kind || p.work < p.kind.build_secs())
        {
            return Err("Project already built or another project still under construction");
        }
        if !kind
            .costs()
            .iter()
            .all(|(c, n)| s.project_stock(*c, Some(target)) + 1e-9 >= *n)
        {
            return Err("Not enough unreserved construction materials");
        }
        for (c, n) in kind.costs() {
            *s.stockpile.get_mut(c).unwrap() -= n;
        }
        s.industry.reservations.retain(|r| r.target != target);
        s.industry.projects.push(ColonyProject {
            kind,
            body,
            commodity,
            work: 0.0,
            active: true,
            supplied: false,
            outputs: vec![],
        });
        Ok(())
    }
    fn industry_engaged(&self, id: EntityId) -> bool {
        self.engagements
            .values()
            .any(|e| e.attackers.contains(&id) || e.defenders.contains(&id))
    }
    pub(super) fn tick_industrial_expansion(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<_> = self
            .fleets
            .iter()
            .filter(|(_, f)| f.industry.is_some())
            .map(|(id, _)| *id)
            .collect();
        // §belts: barges at work on each deposit as of last tick, for the cap.
        let mut crews: BTreeMap<(EntityId, u32, C, bool), u32> = BTreeMap::new();
        for f in self.fleets.values() {
            if let Some(FleetIndustry::Mine { system, body, ore, pocket, status, .. }) = &f.industry
                && matches!(status.as_str(), "mining" | "travelling")
                && self
                    .systems
                    .iter()
                    .find(|s| s.id == *system)
                    .is_some_and(|s| f.pos.distance(s.pos) <= SKIM_RANGE_SU)
            {
                *crews.entry((*system, *body, *ore, *pocket)).or_insert(0) += f.count(ShipKind::MiningBarge);
            }
        }
        for id in ids {
            if self.industry_engaged(id)
                || self.refit_queue.iter().any(|j| j.fleet == id && j.in_place)
            {
                continue;
            }
            let owner = self.fleets[&id].owner;
            let task = self.fleets[&id].industry.clone().unwrap();
            match task {
                FleetIndustry::Route { run } => self.step_freight_route(id, owner, run, events),
                FleetIndustry::Deploy {
                    system,
                    body,
                    outpost,
                    commodity,
                    mut work,
                } => {
                    let Some(s) = self.systems.iter().find(|s| s.id == system) else {
                        continue;
                    };
                    if self.fleets[&id].pos.distance(s.pos) > crate::ship::DOCK_RADIUS {
                        continue;
                    }
                    if s.owner.is_some() || s.blockade.is_some() {
                        self.fleets.get_mut(&id).unwrap().industry = None;
                        continue;
                    }
                    let f = self.fleets.get_mut(&id).unwrap();
                    f.order = FleetOrder::Idle;
                    tenders::hold(f);
                    if work == 0.0 {
                        if !outpost_costs()
                            .iter()
                            .all(|(c, n)| f.cargo_amount(*c) >= *n)
                        {
                            f.industry = None;
                            continue;
                        }
                        for (c, n) in outpost_costs() {
                            f.remove_cargo(*c, *n);
                        }
                    }
                    work += DT;
                    if work < OUTPOST_BUILD_S {
                        f.industry = Some(FleetIndustry::Deploy {
                            system,
                            body,
                            outpost,
                            commodity,
                            work,
                        });
                        continue;
                    }
                    f.industry = None;
                    let s = self.systems.iter_mut().find(|s| s.id == system).unwrap();
                    s.owner = Some(owner);
                    s.claimed_at = Some(self.time);
                    s.seed_warehouse();
                    s.industry.outpost = Some(Outpost {
                        kind: outpost,
                        body,
                        commodity,
                        supplied: false,
                        rate: 0.0,
                    });
                    for b in &mut s.bodies {
                        b.migration_policy = crate::migration::MigrationPolicy::Closed;
                    }
                    events.push(Event::new(
                        self.time,
                        EventPayload::SystemClaimed {
                            owner,
                            system,
                            pos: s.pos,
                        },
                    ));
                    self.apply_logistics_unload(owner, id, Some(system), events);
                }
                FleetIndustry::Skim {
                    system,
                    mut harvested,
                    mut cargo_fraction,
                    ..
                } => {
                    let Some(s) = self.systems.iter().find(|s| s.id == system) else {
                        continue;
                    };
                    if self.fleets[&id].pos.distance(s.pos) > SKIM_RANGE_SU {
                        continue;
                    }
                    let hostile = s.blockade.is_some()
                        || self.engagements.values().any(|e| {
                            e.pos.distance(s.pos) <= SKIM_RANGE_SU
                                && e.started_at + crate::transit::delay(e.pos, s.pos, self.config.c)
                                    <= self.time
                        });
                    let f = self.fleets.get_mut(&id).unwrap();
                    f.order = FleetOrder::Idle;
                    tenders::hold(f);
                    let status = if hostile {
                        "unsafe"
                    } else {
                        // §fuel-overhaul: gas giants yield Helium-3. It tops
                        // up the fusion tank once the corporation can burn it
                        // (Fusion Reactors); freighters hold the rest.
                        let mut amount = SKIM_HELIUM3_PER_S * DT;
                        let tank = f.refuel_helium3(amount);
                        if tank > 0.0 {
                            f.stalled = false;
                        }
                        amount -= tank;
                        harvested += tank;
                        if f.has_freighter() && f.cargo_units() < f.cargo_capacity() {
                            cargo_fraction += amount;
                            let n = (cargo_fraction.floor() as u32)
                                .min(f.cargo_capacity() - f.cargo_units());
                            f.add_cargo(C::Helium3, n);
                            cargo_fraction -= n as f64;
                            harvested += n as f64;
                        }
                        let tank_full = !f.fuel_research.helium3 || f.fusion_room() <= 1e-9;
                        if tank_full
                            && (!f.has_freighter()
                                || f.cargo_units() >= f.cargo_capacity())
                        {
                            "full"
                        } else {
                            "skimming"
                        }
                    };
                    f.industry = Some(FleetIndustry::Skim {
                        system,
                        harvested,
                        cargo_fraction,
                        status: status.into(),
                    });
                }
                FleetIndustry::Mine {
                    system,
                    body,
                    ore,
                    pocket,
                    mined,
                    fraction,
                    ..
                } => self.step_belt_mining(id, owner, (system, body, ore, pocket), mined, fraction, &crews, events),
                FleetIndustry::Prospect { system, body, dwell, .. } => {
                    self.step_prospecting(id, owner, (system, body), dwell, events)
                }
            }
        }
        self.tick_specialized_sites();
    }

    fn step_freight_route(
        &mut self,
        id: EntityId,
        owner: PlayerId,
        mut run: FreightRun,
        events: &mut Vec<Event>,
    ) {
        if run.phase == RoutePhase::Complete {
            return;
        }
        let stop = run.route.stops[run.stop].clone();
        let Some(pos) = self.freight_port_pos(stop.port) else {
            return;
        };
        if self.fleets[&id].pos.distance(pos) > crate::ship::DOCK_RADIUS {
            // A local manual order clears this sidecar; combat simply pauses it.
            // Do not inspect a distant port's true owner, blockade or stock.
            if run.phase == RoutePhase::Dispatch {
                run.hold = self.freight_departure_hold(id, &run.route, pos);
                let f = self.fleets.get_mut(&id).unwrap();
                if run.hold.is_some() {
                    f.order = FleetOrder::Idle;
                    tenders::hold(f);
                } else {
                    run.phase = RoutePhase::Travelling;
                    f.order = FleetOrder::MoveTo { dest: pos };
                }
                f.industry = Some(FleetIndustry::Route { run });
                return;
            }
            let f = self.fleets.get_mut(&id).unwrap();
            if f.supplied {
                f.order = FleetOrder::MoveTo { dest: pos };
            }
            return;
        }
        let system = match stop.port {
            FreightPort::System { id } => Some(id),
            FreightPort::Hub | FreightPort::Station { .. } => None,
        };
        let station = match stop.port {
            FreightPort::Station { id } => Some(id),
            _ => None,
        };
        let access = match station {
            Some(sid) => self.owns_station(owner, sid) && !self.industry_engaged(sid),
            None => system.is_none_or(|sid| {
                self.systems
                    .iter()
                    .any(|s| s.id == sid && s.owner == Some(owner) && !s.blockade_held())
            }),
        };
        if !access {
            // Keep the transfer cursor: a blockade mid-unload must never make
            // already-transferred units load/sell again after access returns.
            run.hold = Some("Port unavailable".into());
            let f = self.fleets.get_mut(&id).unwrap();
            f.order = FleetOrder::Idle;
            tenders::hold(f);
            f.industry = Some(FleetIndustry::Route { run });
            return;
        }
        run.hold = None;
        let f = self.fleets.get_mut(&id).unwrap();
        f.order = FleetOrder::Idle;
        tenders::hold(f);
        if matches!(
            run.phase,
            RoutePhase::Dispatch | RoutePhase::Travelling | RoutePhase::Blocked
        ) {
            run.remaining_load = stop.load.clone();
            run.remaining_unload = stop
                .unload
                .iter()
                .map(|(c, n)| (*c, (*n).min(f.cargo_amount(*c))))
                .collect();
            run.phase = RoutePhase::Unloading;
            run.shopping_spent = 0.0;
        }
        if run.phase == RoutePhase::Unloading {
            self.route_unload_pass(id, owner, system, stop.sell, &mut run, events);
            // A full warehouse cannot take this stop's deliveries, but the cargo
            // this stop LOADS frees exactly that room. Load first instead of
            // waiting forever beside a full stockpile (the founding export used
            // to stall here with the recovered cache still aboard).
            if let Some(sid) = system
                && run.remaining_unload.values().any(|n| *n > 0)
                && run.remaining_load.values().any(|n| *n > 0)
                && self.systems.iter().find(|s| s.id == sid).is_some_and(|s| s.storage_headroom() < 1.0)
            {
                self.route_system_load_pass(id, owner, sid, &mut run, events);
                self.route_unload_pass(id, owner, system, stop.sell, &mut run, events);
            }
            if run.remaining_unload.values().all(|n| *n == 0) {
                run.phase = RoutePhase::Loading;
            } else if let Some(s) = system.and_then(|sid| self.systems.iter().find(|s| s.id == sid)) {
                run.hold = Some(format!(
                    "Storage full at {} ({:.0}/{:.0}); unloading waits for space",
                    s.name,
                    s.storage_used(),
                    s.storage_cap()
                ));
            }
        }
        if run.phase == RoutePhase::Loading
            && let Some(sid) = station
        {
            // §belts: a station stop takes ore straight from the station's hold
            // and waits there until the manifest is aboard.
            for (&c, left) in &mut run.remaining_load {
                let room = {
                    let f = &self.fleets[&id];
                    f.cargo_capacity().saturating_sub(f.cargo_units())
                };
                let n = (*left).min(room).min(self.fleets[&sid].cargo_amount(c));
                if n > 0 {
                    self.fleets.get_mut(&sid).unwrap().remove_cargo(c, n);
                    self.fleets.get_mut(&id).unwrap().add_cargo(c, n);
                    *left -= n;
                    if let Some(corp) = self.players.get_mut(&owner) {
                        corp.mining.station_hauled += n as f64;
                    }
                }
            }
            if run.remaining_load.values().all(|n| *n == 0) {
                run.phase = RoutePhase::Fuel;
            } else {
                run.hold = Some(
                    if self.fleets[&id].cargo_units() >= self.fleets[&id].cargo_capacity() {
                        "Cargo space full; revise the unload manifest"
                    } else {
                        "Waiting for ore at the station"
                    }
                    .into(),
                );
            }
        }
        if run.phase == RoutePhase::Loading && station.is_none() {
            // After selling, leave cash for the ordinary paid tank service on
            // the next tick. Only physically present escorts count here; no
            // remote fleet telemetry is consulted by an onboard shopping job.
            let bunker_cash = if system.is_none() && run.route.shopping_budget.is_some() {
                // §fuel-overhaul: the Hub sells each hull the fuel its regime
                // burns — Uranium on Slow, else Deuterium.
                let f = &self.fleets[&id];
                let escort = run.route.escort.and_then(|escort| self.fleets.get(&escort))
                    .filter(|g| g.owner == owner && g.pos.distance(f.pos) <= 1_000.0);
                let need = |g: &crate::ship::Fleet| if g.burns_uranium() {
                    (C::Uranium, (g.uranium_capacity() - g.uranium).max(0.0).floor() as u32)
                } else {
                    (C::Deuterium, g.fusion_room().floor() as u32)
                };
                let mut quote = 0.0;
                for (c, n) in std::iter::once(need(f)).chain(escort.map(need)) {
                    quote += self.market.quote_buy(c, n).total;
                }
                (quote + self.exchange_penalty(owner,quote)) * 1.05
            } else { 0.0 };
            for (&c, left) in &mut run.remaining_load {
                if system.is_none() && let Some(budget) = run.route.shopping_budget {
                    let stocked = self.players[&owner].warehouse.get(&c).copied().unwrap_or(0);
                    let room = self.fleets[&id].cargo_capacity().saturating_sub(self.fleets[&id].cargo_units());
                    let need = (*left).min(room).saturating_sub(stocked);
                    let cash = (budget - run.shopping_spent).max(0.0).min((self.players[&owner].credits - bunker_cash).max(0.0));
                    let mut lo = 0;
                    let mut hi = need.min(self.market.available_to_buy(c));
                    while lo < hi {
                        let n = lo + (hi - lo).div_ceil(2);
                        let cost = self.market.quote_buy(c, n).total;
                        if cost + self.exchange_penalty(owner, cost) <= cash { lo = n; } else { hi = n - 1; }
                    }
                    if lo > 0 {
                        let before = self.players[&owner].credits;
                        self.apply_now(&Command::MarketBuy { player_id: owner, commodity: c, units: lo, max_unit_price: Some(self.market.quote_buy(c, lo).unit_price) }, events);
                        run.shopping_spent += (before - self.players[&owner].credits).max(0.0);
                    }
                }
                let available = system
                    .map(|sid| {
                        self.systems
                            .iter()
                            .find(|s| s.id == sid)
                            .unwrap()
                            .free_stock(c)
                            .floor() as u32
                    })
                    .unwrap_or_else(|| {
                        self.players[&owner].warehouse.get(&c).copied().unwrap_or(0)
                    });
                let f = &self.fleets[&id];
                let n = (*left)
                    .min(available)
                    .min(f.cargo_capacity().saturating_sub(f.cargo_units()));
                if n > 0 {
                    let before = self.fleets[&id].cargo_amount(c);
                    self.apply_logistics_load(owner, id, system, c, n, events);
                    *left -= self.fleets[&id].cargo_amount(c).saturating_sub(before);
                }
                // A bounded shopping trip returns affordable/available goods.
                // An unaffordable wishlist must not strand a new player's hull.
                if system.is_none() && run.route.shopping_budget.is_some() { *left = 0; }
            }
            if run.remaining_load.values().all(|n| *n == 0) {
                run.phase = RoutePhase::Fuel;
            } else {
                run.hold = Some(
                    if self.fleets[&id].cargo_units() >= self.fleets[&id].cargo_capacity() {
                        "Cargo space full; revise the unload manifest"
                    } else {
                        "Waiting for unreserved cargo"
                    }
                    .into(),
                );
            }
        }
        if matches!(run.phase, RoutePhase::Fuel | RoutePhase::Escort) {
            let next = (run.stop + 1) % run.route.stops.len();
            if next == 0 && !run.route.repeat {
                run.phase = RoutePhase::Complete;
            } else {
                let dest = self.freight_port_pos(run.route.stops[next].port).unwrap();
                run.hold = self.freight_departure_hold(id, &run.route, dest);
                if let Some(reason) = &run.hold {
                    run.phase = if reason.contains("escort") {
                        RoutePhase::Escort
                    } else {
                        RoutePhase::Fuel
                    };
                } else {
                    run.stop = next;
                    run.visits = run.visits.saturating_add(1);
                    run.phase = RoutePhase::Travelling;
                    self.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest };
                }
            }
        }
        self.fleets.get_mut(&id).unwrap().industry = Some(FleetIndustry::Route { run });
    }

    /// Deliver what this stop unloads, as far as the port's storage allows.
    fn route_unload_pass(
        &mut self,
        id: EntityId,
        owner: PlayerId,
        system: Option<EntityId>,
        sell: bool,
        run: &mut FreightRun,
        events: &mut Vec<Event>,
    ) {
        for (&c, left) in &mut run.remaining_unload {
            // Cargo lost during a fight cannot be unloaded later. Preserve
            // real surviving cargo without waiting forever for stolen units.
            *left = (*left).min(self.fleets[&id].cargo_amount(c));
            let room = system
                .map(|sid| {
                    self.systems
                        .iter()
                        .find(|s| s.id == sid)
                        .unwrap()
                        .storage_headroom()
                        .floor() as u32
                })
                .unwrap_or(u32::MAX);
            let n = (*left).min(room).min(self.fleets[&id].cargo_amount(c));
            if n == 0 {
                continue;
            }
            self.fleets.get_mut(&id).unwrap().remove_cargo(c, n);
            *left -= n;
            if let Some(sid) = system {
                *self
                    .systems
                    .iter_mut()
                    .find(|s| s.id == sid)
                    .unwrap()
                    .stockpile
                    .entry(c)
                    .or_default() += n as f64;
            } else {
                *self
                    .players
                    .get_mut(&owner)
                    .unwrap()
                    .warehouse
                    .entry(c)
                    .or_default() += n;
            }
            events.push(Event::new(
                self.time,
                EventPayload::Trade(TradeEvent::Unloaded {
                    player: owner,
                    commodity: c,
                    units: n,
                    system,
                }),
            ));
            if sell {
                self.apply_now(
                    &Command::MarketSell {
                        player_id: owner,
                        commodity: c,
                        units: n,
                        min_unit_price: None,
                    },
                    events,
                );
            }
        }
    }

    /// Take this stop's pickups from an owned system's unreserved stock.
    fn route_system_load_pass(
        &mut self,
        id: EntityId,
        owner: PlayerId,
        sid: EntityId,
        run: &mut FreightRun,
        events: &mut Vec<Event>,
    ) {
        for (&c, left) in &mut run.remaining_load {
            let available = self
                .systems
                .iter()
                .find(|s| s.id == sid)
                .map_or(0, |s| s.free_stock(c).floor() as u32);
            let f = &self.fleets[&id];
            let n = (*left)
                .min(available)
                .min(f.cargo_capacity().saturating_sub(f.cargo_units()));
            if n > 0 {
                let before = self.fleets[&id].cargo_amount(c);
                self.apply_logistics_load(owner, id, Some(sid), c, n, events);
                *left -= self.fleets[&id].cargo_amount(c).saturating_sub(before);
            }
        }
    }

    fn freight_departure_hold(
        &self,
        id: EntityId,
        route: &FreightRoute,
        dest: Vec2,
    ) -> Option<String> {
        let f = &self.fleets[&id];
        // Same public-geometry estimate as CC planning; onboard tank/cargo
        // checks are local physical decisions, not remote information shortcuts.
        let consumption = f.propulsion_fuel_mult();
        let budget = crate::fuel::route_estimate(f.pos, dest, f.mass(), consumption,
            self.systems.iter().map(|s| s.pos).chain(std::iter::once(self.hub))) + route.fuel_reserve;
        // §fuel-overhaul: the store the route's throttle burns (Uranium on Slow).
        let reason = if budget > f.propulsion_capacity() {
            "Leg exceeds tank range; shorten route or reduce cargo"
        } else if f.propulsion_store() + 1e-9 < budget {
            "Waiting for fuel and arrival reserve"
        } else if !f.supplied {
            "Waiting for ship supplies"
        } else if route.escort.is_some_and(|escort| {
            !self.fleets.get(&escort).is_some_and(|g| {
                g.owner == f.owner
                    && g.is_combatant()
                    && g.pos.distance(f.pos) <= 1_000.0
                    && matches!(g.order, FleetOrder::Guard { target } if target == id)
            })
        }) {
            "Waiting for escort"
        } else if route.escort.is_some_and(|escort| {
            let g = &self.fleets[&escort];
            // §fuel-overhaul: an escort burns its charge's Uranium regime when
            // it can fly it, else its own fusion tank. Price that hull itself.
            let (uranium, escort_consumption) = g.formation_burn(f.burns_uranium());
            let escort_budget = crate::fuel::route_estimate(f.pos, dest, g.mass(), escort_consumption,
                self.systems.iter().map(|s| s.pos).chain(std::iter::once(self.hub)));
            !g.supplied
                || (if uranium { g.uranium } else { g.fuel }) + 1e-9 < escort_budget
        }) {
            "Waiting for escort fuel or supplies"
        } else {
            return None;
        };
        Some(reason.into())
    }

    fn tick_specialized_sites(&mut self) {
        for s in &mut self.systems {
            if s.owner.is_none() {
                continue;
            }
            let safe = !s.blockade_held();
            if let Some(mut outpost) = s.industry.outpost.take() {
                // §fuel-overhaul: an outpost burns no fuel.
                let inputs = [
                    (C::Provisions, 0.03),
                    (C::Machinery, 0.005),
                ];
                outpost.supplied = safe
                    && inputs
                        .iter()
                        .all(|(c, n)| s.free_stock(*c) + 1e-9 >= n * DT);
                outpost.rate = 0.0;
                if outpost.supplied {
                    for (c, n) in inputs {
                        *s.stockpile.get_mut(&c).unwrap() -= n * DT;
                    }
                    if outpost.kind == OutpostKind::Extraction {
                        outpost.rate =
                            extract_site(s, outpost.body, outpost.commodity, 0.8, DT) / DT;
                    }
                }
                s.industry.outpost = Some(outpost);
            }
            let share = s.staffing_share();
            // Temporarily take projects to allow atomic baskets against the
            // shared stock. Workers still include all active project teams.
            let mut projects = std::mem::take(&mut s.industry.projects);
            for p in &mut projects {
                p.outputs.clear();
                p.supplied = false;
                if !p.active || !safe || share <= 0.0 {
                    continue;
                }
                if p.work < p.kind.build_secs() {
                    p.work = (p.work + DT * share).min(p.kind.build_secs());
                    continue;
                }
                let fraction = p
                    .kind
                    .inputs()
                    .iter()
                    .map(|(c, n)| s.free_stock(*c) / (n * DT))
                    .fold(share, f64::min)
                    .clamp(0.0, 1.0);
                if fraction <= 1e-9 {
                    continue;
                }
                let outputs = p.kind.outputs();
                let room = s.storage_headroom();
                let net = outputs.iter().map(|(_, n)| n).sum::<f64>()
                    - p.kind.inputs().iter().map(|(_, n)| n).sum::<f64>();
                let fraction = if net > 0.0 {
                    fraction.min(room / (net * DT))
                } else {
                    fraction
                };
                if p.kind == ColonyProjectKind::DeepExtraction {
                    let n = extract_site(s, p.body, p.commodity, 2.0 * fraction, DT);
                    if n <= 0.0 {
                        continue;
                    }
                    p.outputs.push((p.commodity, n / DT));
                } else {
                    if fraction <= 1e-9 {
                        continue;
                    }
                    for (c, n) in outputs {
                        *s.stockpile.entry(*c).or_default() += n * DT * fraction;
                        p.outputs.push((*c, n * fraction));
                    }
                }
                for (c, n) in p.kind.inputs() {
                    *s.stockpile.get_mut(c).unwrap() -= n * DT * fraction;
                }
                p.supplied = true;
            }
            s.industry.projects = projects;
        }
    }
}

fn extract_site(s: &mut StarSystem, body: u32, commodity: C, mult: f64, dt: f64) -> f64 {
    let room = s.storage_headroom();
    let Some(b) = s.bodies.iter_mut().find(|b| b.id == body) else {
        return 0.0;
    };
    let Some(i) = b.deposits.iter().position(|d| d.resource == commodity) else {
        return 0.0;
    };
    let amount = (crate::explore::natural_extraction_rate(b, &b.deposits[i], s.trait_)
        * crate::production::ore_bulk_ratio(commodity)
        * mult
        * dt)
        .min(room)
        .min(b.deposits[i].reserves.unwrap_or(f64::INFINITY))
        .max(0.0);
    if let Some(reserves) = &mut b.deposits[i].reserves {
        *reserves -= amount;
    }
    *s.stockpile.entry(commodity).or_default() += amount;
    amount
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(812, 4));
        let owner = PlayerId(911);
        w.step(&[Command::AddPlayer {
            id: owner,
            name: "Industrial routes".into(),
        }]);
        w.enclaves.clear();
        w.fleets.clear();
        w.nebulas.clear();
        let home = w.players[&owner].home_system.unwrap();
        w.hub = Vec2::new(10_000.0, 0.0);
        w.players.get_mut(&owner).unwrap().command_center = Vec2::ZERO;
        let s = site_mut(&mut w, home);
        s.pos = Vec2::ZERO;
        s.stockpile.clear();
        for b in &mut s.bodies {
            b.structures.clear();
            b.assignments.clear();
        }
        s.set_population(0.02);
        s.seed_warehouse(); // routes need real storage after clearing other buildings
        let fleet = w.alloc_entity_id();
        w.fleets.insert(
            fleet,
            Fleet::single(
                fleet,
                owner,
                ShipKind::Convoy,
                Vec2::ZERO,
                FleetOrder::Idle,
                None,
            ),
        );
        (w, owner, home, fleet)
    }
    fn site(w: &World, id: EntityId) -> &StarSystem {
        w.systems.iter().find(|s| s.id == id).unwrap()
    }
    fn site_mut(w: &mut World, id: EntityId) -> &mut StarSystem {
        w.systems.iter_mut().find(|s| s.id == id).unwrap()
    }
    fn route(home: EntityId) -> FreightRoute {
        FreightRoute {
            shopping_budget: None,
            name: "Mixed outward / alloy return".into(),
            transit: crate::ship::TransitMode::Full,
            fuel_reserve: 5.0,
            escort: None,
            repeat: true,
            stops: vec![
                RouteStop {
                    port: FreightPort::System { id: home },
                    load: [(C::MetallicOre, 10), (C::Biomass, 5)].into(),
                    unload: [(C::Alloys, 7)].into(),
                    sell: false,
                },
                RouteStop {
                    port: FreightPort::Hub,
                    load: [(C::Alloys, 7)].into(),
                    unload: [(C::MetallicOre, 10), (C::Biomass, 5)].into(),
                    sell: false,
                },
            ],
        }
    }
    fn run(w: &World, id: EntityId) -> &FreightRun {
        let Some(FleetIndustry::Route { run }) = &w.fleets[&id].industry else {
            panic!("route lost")
        };
        run
    }

    #[test]
    fn slow_routes_remember_their_regime_and_keep_legacy_routes_at_full_speed() {
        let (mut w, owner, home, fleet) = scene();
        let mut r = route(home);
        let mut legacy = serde_json::to_value(&r).unwrap();
        legacy.as_object_mut().unwrap().remove("transit");
        assert_eq!(serde_json::from_value::<FreightRoute>(legacy).unwrap().transit,
            crate::ship::TransitMode::Full);
        r.transit = crate::ship::TransitMode::Slow;
        w.set_freight_route(owner, fleet, Some(r), &mut vec![]);
        assert_eq!(w.fleets[&fleet].transit, crate::ship::TransitMode::Slow);
        assert_eq!(run(&w, fleet).route.transit, crate::ship::TransitMode::Slow);
        let full_budget = crate::fuel::route_estimate(w.fleets[&fleet].pos, w.hub,
            w.fleets[&fleet].mass(), 1.0, w.systems.iter().map(|s| s.pos).chain(std::iter::once(w.hub)));
        // A slow leg is priced against the Uranium store at a quarter of the energy.
        let reserve = run(&w, fleet).route.fuel_reserve;
        let f = w.fleets.get_mut(&fleet).unwrap();
        f.uranium = full_budget * 0.3 + reserve;
        f.fuel = 0.0;
        assert!(w.freight_departure_hold(fleet, &run(&w, fleet).route, w.hub).is_none());
        w.fleets.get_mut(&fleet).unwrap().transit = crate::ship::TransitMode::Full;
        assert!(w.freight_departure_hold(fleet, &run(&w, fleet).route, w.hub).unwrap().contains("fuel"));
        w.step(&[Command::SetFleetTransit { player_id: owner, fleet_id: fleet, mode: crate::ship::TransitMode::Full }]);
        assert_eq!(run(&w, fleet).route.transit, crate::ship::TransitMode::Full,
            "a received mode change updates the remembered route too");
    }
    fn pump(w: &mut World, ticks: u32) {
        for _ in 0..ticks {
            w.time += DT;
            w.tick_industrial_expansion(&mut vec![]);
        }
    }

    #[test]
    fn opening_return_trip_needs_no_automation_but_recurring_multi_stop_does() {
        let (mut w, owner, home, id) = scene();
        let mut r = route(home);
        r.stops.push(RouteStop { port: FreightPort::System { id: home }, load: Default::default(), unload: [(C::Alloys,7)].into(), sell: false });
        w.set_freight_route(owner,id,Some(r.clone()),&mut vec![]);
        assert!(w.fleets[&id].industry.is_none(), "recurring multi-stop needs research");
        r.repeat = false;
        w.set_freight_route(owner,id,Some(r),&mut vec![]);
        assert_eq!(run(&w,id).route.stops.len(),3);
        let mut old = serde_json::to_value(route(home)).unwrap();
        old.as_object_mut().unwrap().remove("shopping_budget");
        assert!(serde_json::from_value::<FreightRoute>(old).unwrap().shopping_budget.is_none());
    }

    #[test]
    fn opening_shopping_sells_real_cargo_then_buys_within_cap_and_returns_partial() {
        let (mut w, owner, home, id) = scene();
        site_mut(&mut w,home).stockpile.insert(C::MetallicOre,50.0);
        w.players.get_mut(&owner).unwrap().credits = 0.0;
        let r = FreightRoute { name:"One trip".into(), repeat:false, escort:None,
            transit:crate::ship::TransitMode::Slow, fuel_reserve:0.0, shopping_budget:Some(100.0),
            stops:vec![
                RouteStop { port:FreightPort::System{id:home},load:[(C::MetallicOre,50)].into(),unload:Default::default(),sell:false },
                RouteStop { port:FreightPort::Hub,load:[(C::Alloys,40)].into(),unload:[(C::MetallicOre,50)].into(),sell:true },
                RouteStop { port:FreightPort::System{id:home},load:Default::default(),unload:[(C::Alloys,40)].into(),sell:false },
            ] };
        w.set_freight_route(owner,id,Some(r),&mut vec![]);
        pump(&mut w,1);
        assert_eq!(w.players[&owner].credits,0.0, "no advance credit for unarrived ore");
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys),0);
        w.fleets.get_mut(&id).unwrap().pos = w.hub;
        let mut events=vec![];
        w.tick_industrial_expansion(&mut events);
        let bought=w.fleets[&id].cargo_amount(C::Alloys);
        assert!(bought>0 && bought<40);
        assert!(run(&w,id).shopping_spent>0.0 && run(&w,id).shopping_spent<=100.0);
        assert!(w.players[&owner].credits>0.0);
        assert_eq!(run(&w,id).stop,2, "an unaffordable remainder must not strand the hull");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::Trade(TradeEvent::Bought { .. }))));
        let before=w.players[&owner].credits;
        let mut w: World=serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        pump(&mut w,10);
        assert_eq!(w.players[&owner].credits,before, "reload never buys the same manifest twice");
        w.fleets.get_mut(&id).unwrap().pos=Vec2::ZERO;
        pump(&mut w,1);
        assert_eq!(site(&w,home).stockpile[&C::Alloys],f64::from(bought));
        assert_eq!(w.fleets[&id].cargo_units(),0);
        assert_eq!(run(&w,id).phase,RoutePhase::Complete);
        pump(&mut w,10);
        assert_eq!(site(&w,home).stockpile[&C::Alloys],f64::from(bought));
    }

    #[test]
    fn opening_shopping_leaves_cash_for_ordinary_tank_service() {
        let (mut w, owner, home, id) = scene();
        let f = w.fleets.get_mut(&id).unwrap();
        f.pos = w.hub;
        f.fuel = f.fuel_capacity() - 10.0;
        let fuel = w.market.quote_buy(C::Deuterium, 10).total;
        let buffer = (fuel + w.exchange_penalty(owner, fuel)) * 1.05;
        let alloys = w.market.quote_buy(C::Alloys, 2).total;
        w.players.get_mut(&owner).unwrap().credits = buffer + alloys + w.exchange_penalty(owner, alloys);
        let route = FreightRoute {
            name: "Limited-cash return".into(), repeat: false, escort: None,
            transit: crate::ship::TransitMode::Full, fuel_reserve: 0.0, shopping_budget: Some(10_000.0),
            stops: vec![
                RouteStop { port: FreightPort::Hub, load: [(C::Alloys, 40)].into(), unload: Default::default(), sell: false },
                RouteStop { port: FreightPort::System { id: home }, load: Default::default(), unload: [(C::Alloys, 40)].into(), sell: false },
            ],
        };
        w.set_freight_route(owner, id, Some(route), &mut vec![]);
        pump(&mut w, 1);
        assert!(w.players[&owner].credits + 1e-9 >= buffer);
        assert!((1..=2).contains(&w.fleets[&id].cargo_amount(C::Alloys)));
        assert_eq!(run(&w, id).stop, 1, "partial shopping does not keep the carrier at the Hub");
    }

    #[test]
    fn assigned_hull_runs_mixed_return_manifests_once_per_visit_and_survives_save() {
        let (mut w, owner, home, id) = scene();
        site_mut(&mut w, home)
            .stockpile
            .extend([(C::MetallicOre, 100.0), (C::Biomass, 100.0)]);
        w.players
            .get_mut(&owner)
            .unwrap()
            .warehouse
            .insert(C::Alloys, 21);
        let count = w.fleets.len();
        w.set_freight_route(owner, id, Some(route(home)), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(w.fleets[&id].cargo_amount(C::MetallicOre), 10);
        assert_eq!(w.fleets[&id].cargo_amount(C::Biomass), 5);
        assert_eq!(run(&w, id).stop, 1);
        pump(&mut w, 20);
        assert_eq!(
            w.fleets[&id].cargo_units(),
            15,
            "travelling never loads twice"
        );
        w.fleets.get_mut(&id).unwrap().pos = w.hub;
        pump(&mut w, 1);
        assert_eq!(w.players[&owner].warehouse[&C::MetallicOre], 10);
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys), 7);
        let saved = serde_json::to_string(&w).unwrap();
        let mut w: World = serde_json::from_str(&saved).unwrap();
        pump(&mut w, 3);
        assert_eq!(w.players[&owner].warehouse[&C::MetallicOre], 10);
        w.fleets.get_mut(&id).unwrap().pos = Vec2::ZERO;
        pump(&mut w, 1);
        assert_eq!(site(&w, home).stockpile[&C::Alloys], 7.0);
        assert_eq!(site(&w, home).stockpile[&C::MetallicOre], 80.0);
        assert_eq!(w.fleets.len(), count, "routes never spawn abstract freight");
    }

    #[test]
    fn partial_route_unload_keeps_its_cursor_across_blockade_and_restart() {
        let (mut w, owner, home, id) = scene();
        let cap = site(&w, home).storage_cap();
        // Fill with a good this stop does not load, so loading cannot make room.
        site_mut(&mut w, home)
            .stockpile
            .insert(C::Provisions, cap - 2.0);
        w.fleets.get_mut(&id).unwrap().add_cargo(C::Alloys, 7);
        w.set_freight_route(owner, id, Some(route(home)), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys), 5);
        assert_eq!(run(&w, id).remaining_unload[&C::Alloys], 5);
        assert!(run(&w, id).hold.as_deref().is_some_and(|h| h.starts_with("Storage full at")),
            "a storage-bound unload names its blocker");
        site_mut(&mut w, home).owner = Some(PlayerId(912));
        pump(&mut w, 2);
        assert_eq!(run(&w, id).remaining_unload[&C::Alloys], 5);
        let mut w: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        site_mut(&mut w, home).owner = Some(owner);
        site_mut(&mut w, home).stockpile.insert(C::Provisions, 0.0);
        pump(&mut w, 2);
        assert_eq!(site(&w, home).stockpile[&C::Alloys], 7.0);
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys), 0);
        assert_eq!(
            run(&w, id).phase,
            RoutePhase::Loading,
            "wait for the exact missing outbound manifest"
        );
    }

    #[test]
    fn a_full_warehouse_loads_this_stops_pickups_before_unloading() {
        // The founding export stalled at "Unloading" forever: the camp store
        // was full, yet the ore it was about to load would have made room.
        let (mut w, owner, home, id) = scene();
        let cap = site(&w, home).storage_cap();
        site_mut(&mut w, home).stockpile.insert(C::MetallicOre, cap);
        w.fleets.get_mut(&id).unwrap().add_cargo(C::Alloys, 7);
        w.set_freight_route(owner, id, Some(route(home)), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(w.fleets[&id].cargo_amount(C::MetallicOre), 10);
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys), 0);
        assert_eq!(site(&w, home).stockpile[&C::Alloys], 7.0);
        assert_eq!(site(&w, home).stockpile[&C::MetallicOre], cap - 10.0);
        assert_ne!(run(&w, id).phase, RoutePhase::Unloading);
        assert!(run(&w, id).hold.as_deref().is_none_or(|h| !h.starts_with("Storage full")));
    }

    #[test]
    fn routes_wait_for_fuel_and_real_guard_orders_including_initial_dispatch() {
        let (mut w, owner, home, id) = scene();
        let escort = w.alloc_entity_id();
        w.fleets.insert(
            escort,
            Fleet::single(
                escort,
                owner,
                ShipKind::Raider,
                Vec2::ZERO,
                FleetOrder::Idle,
                None,
            ),
        );
        let mut r = route(home);
        r.escort = Some(escort);
        for s in &mut r.stops {
            s.load.clear();
            s.unload.clear();
        }
        w.fleets.get_mut(&id).unwrap().fuel = 0.0;
        w.set_freight_route(owner, id, Some(r.clone()), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(run(&w, id).phase, RoutePhase::Fuel);
        let f = w.fleets.get_mut(&id).unwrap();
        f.fuel = f.fuel_capacity();
        pump(&mut w, 1);
        assert_eq!(
            run(&w, id).phase,
            RoutePhase::Escort,
            "nearby warship is not permission to commandeer it"
        );
        w.fleets.get_mut(&escort).unwrap().order = FleetOrder::Guard { target: id };
        pump(&mut w, 1);
        assert_eq!(run(&w, id).phase, RoutePhase::Travelling);
        let f = w.fleets.get_mut(&id).unwrap();
        f.pos = Vec2::new(5_000.0, 0.0);
        f.fuel = 0.0;
        w.set_freight_route(owner, id, Some(r), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(run(&w, id).phase, RoutePhase::Dispatch);
        assert!(matches!(w.fleets[&id].order, FleetOrder::Idle));
    }

    #[test]
    fn freight_route_assignment_and_manual_cancellation_wait_for_order_delivery() {
        let (mut w, owner, home, id) = scene();
        w.fleets.get_mut(&id).unwrap().pos = Vec2::new(40_000.0, 0.0);
        w.apply(
            &Command::SetFreightRoute {
                player_id: owner,
                fleet_id: id,
                route: Some(route(home)),
            },
            &mut vec![],
        );
        assert!(w.fleets[&id].industry.is_none());
        w.time = w.pending_orders[0].apply_time;
        w.deliver_due_orders(&mut vec![]);
        assert!(w.fleets[&id].industry.is_some());
        w.apply(
            &Command::HoldFleet {
                player_id: owner,
                ship_id: id,
            },
            &mut vec![],
        );
        assert!(
            w.fleets[&id].industry.is_some(),
            "cancel at receipt, not issue"
        );
        w.time = w.pending_orders[0].apply_time;
        w.deliver_due_orders(&mut vec![]);
        assert!(w.fleets[&id].industry.is_none());
    }

    #[test]
    fn malformed_routes_and_missing_multi_stop_research_are_rejected_without_overwrite() {
        let (mut w, owner, home, id) = scene();
        let mut r = route(home);
        r.stops.push(r.stops[0].clone());
        w.set_freight_route(owner, id, Some(r.clone()), &mut vec![]);
        assert!(w.fleets[&id].industry.is_none());
        w.players
            .get_mut(&owner)
            .unwrap()
            .research
            .completed
            .insert("prop_line_autonomous_freight".into());
        w.set_freight_route(owner, id, Some(r.clone()), &mut vec![]);
        assert!(w.fleets[&id].industry.is_some());
        let before = w.fleets[&id].industry.clone();
        r.fuel_reserve = f64::NAN;
        w.set_freight_route(owner, id, Some(r), &mut vec![]);
        assert_eq!(w.fleets[&id].industry, before);
        w.set_freight_route(PlayerId(999), id, None, &mut vec![]);
        assert_eq!(w.fleets[&id].industry, before);
    }

    #[test]
    fn reservation_protects_loading_but_matching_build_can_spend_it() {
        let (mut w, owner, home, id) = scene();
        let target = ProjectTarget::Academy;
        for (c, n) in target.costs() {
            site_mut(&mut w, home).stockpile.insert(*c, *n);
        }
        w.reserve_project(owner, home, target, true);
        let (c, n) = target.costs()[0];
        assert_eq!(site(&w, home).free_stock(c), 0.0);
        w.apply_logistics_load(owner, id, Some(home), c, 1, &mut vec![]);
        assert_eq!(w.fleets[&id].cargo_units(), 0);
        assert_eq!(site(&w, home).project_stock(c, Some(target)), n);
        let body = site(&w, home)
            .site_for(crate::StructureKind::Academy)
            .unwrap();
        w.apply_now(
            &Command::DevelopSystem {
                player_id: owner,
                system_id: home,
                body_id: Some(body),
                upgrade: crate::StructureKind::Academy,
            },
            &mut vec![],
        );
        assert_eq!(w.build_queue.len(), 1);
        assert!(site(&w, home).industry.reservations.is_empty());
        assert_eq!(site(&w, home).stockpile[&c], 0.0);
    }

    #[test]
    fn colony_project_needs_workers_atomic_imports_and_affects_only_its_specialty() {
        let (mut w, owner, home, _) = scene();
        let kind = ColonyProjectKind::OrbitalAssembly;
        // §fuel-overhaul: its Warp Cores need Fusion Reactors.
        let body0 = site(&w, home).bodies[0].id;
        site_mut(&mut w, home).bodies[0].set_tier(crate::StructureKind::Shipyard, 2);
        assert_eq!(w.start_colony_project(owner, home, body0, kind, C::MetallicOre).err(),
            Some("This world cannot support that project"));
        w.players.get_mut(&owner).unwrap().research.completed.insert(crate::power::FUSION_RESEARCH.into());
        let s = site_mut(&mut w, home);
        let body = s.bodies[0].id;
        s.bodies[0].set_tier(crate::StructureKind::Shipyard, 2);
        for (c, n) in kind.costs() {
            s.stockpile.insert(*c, *n);
        }
        w.reserve_project(
            owner,
            home,
            ProjectTarget::Development { project: kind },
            true,
        );
        w.start_colony_project(owner, home, body, kind, C::MetallicOre)
            .unwrap();
        assert_eq!(site(&w, home).industry.projects.len(), 1);
        site_mut(&mut w, home).set_population(0.0);
        pump(&mut w, 30);
        assert_eq!(site(&w, home).industry.projects[0].work, 0.0);
        let s = site_mut(&mut w, home);
        s.set_population(0.02);
        s.industry.projects[0].work = kind.build_secs();
        s.stockpile.clear();
        s.stockpile.extend([(C::Composites, 20.0), (C::Deuterium, 20.0), (C::Iridium, 20.0)]);
        pump(&mut w, 30);
        assert_eq!(
            site(&w, home).stockpile[&C::Composites],
            20.0,
            "missing electronics branch stalls the whole basket"
        );
        site_mut(&mut w, home)
            .stockpile
            .insert(C::PrecisionComponents, 20.0);
        pump(&mut w, 30);
        let s = site(&w, home);
        assert!(s.stockpile[&C::HullSections] > 0.0 && s.stockpile[&C::WarpCore] > 0.0);
        assert_eq!(
            s.stockpile.get(&C::Alloys),
            None,
            "no universal production buff"
        );
        assert!(s.industry.projects[0].supplied);
    }

    #[test]
    fn outpost_is_an_uninhabitable_supplied_claim_not_a_free_colony() {
        let (mut w, owner, home, id) = scene();
        let target = w
            .systems
            .iter()
            .find(|s| s.id != home && !s.bodies.is_empty())
            .unwrap()
            .id;
        let s = site_mut(&mut w, target);
        s.owner = None;
        s.pos = Vec2::new(2_000.0, 0.0);
        for b in &mut s.bodies {
            b.habitable = false;
            b.population = 0.0;
        }
        let body = s.bodies[0].id;
        let pos = s.pos;
        w.players.get_mut(&owner).unwrap().surveyed.insert(target);
        w.legacy_test_bootstrap = true;
        // Only the onboarding state is advanced in the fixture, not a tunable.
        w.players.get_mut(&owner).unwrap().founding.stage =
            crate::founding::FoundingStage::Complete;
        for (c, n) in outpost_costs() {
            w.fleets.get_mut(&id).unwrap().add_cargo(*c, *n);
        }
        w.deploy_outpost(
            owner,
            id,
            target,
            body,
            OutpostKind::Research,
            C::MetallicOre,
            &mut vec![],
        );
        assert!(site(&w, target).owner.is_none());
        w.fleets.get_mut(&id).unwrap().pos = pos;
        pump(&mut w, (OUTPOST_BUILD_S / DT).ceil() as u32 + 2);
        assert_eq!(site(&w, target).owner, Some(owner));
        assert_eq!(site(&w, target).population(), 0.0);
        assert!(w.fleets.contains_key(&id));
        assert!(!site(&w, target).industry.outpost.as_ref().unwrap().supplied);
        site_mut(&mut w, target).stockpile.extend([
            (C::Provisions, 10.0),
            (C::Deuterium, 10.0),
            (C::Machinery, 10.0),
        ]);
        pump(&mut w, 1);
        assert!(site(&w, target).industry.outpost.as_ref().unwrap().supplied);
        assert!(site(&w, target).stockpile[&C::Provisions] < 10.0);
    }

    #[test]
    fn project_reservations_wait_for_outbound_commands_and_returning_system_light() {
        let (mut w, owner, home, _) = scene();
        site_mut(&mut w, home).pos = Vec2::new(40_000.0, 0.0);
        w.time = 1_000.0;
        w.information = crate::information::InformationHistory::default();
        w.record_information();
        let command = Command::ReserveProject {
            player_id: owner,
            system_id: home,
            target: ProjectTarget::Academy,
            reserve: true,
        };
        w.apply(&command, &mut vec![]);
        assert!(site(&w, home).industry.reservations.is_empty());
        let arrival = w.pending_administration[0].0;
        assert!(arrival > w.time);
        w.time = arrival;
        w.step(&[]);
        assert_eq!(site(&w, home).industry.reservations.len(), 1);
        let delay = crate::transit::delay(site(&w, home).pos, Vec2::ZERO, w.config.c);
        let old = w
            .information
            .site(home, Vec2::ZERO, w.config.c, arrival + delay - DT)
            .unwrap();
        assert!(
            old.system.industry.reservations.is_empty(),
            "no truth shortcut to the planner"
        );
        let new = w
            .information
            .site(home, Vec2::ZERO, w.config.c, arrival + delay + DT)
            .unwrap();
        assert_eq!(new.system.industry.reservations.len(), 1);
        let saved = serde_json::to_string(&w).unwrap();
        let w: World = serde_json::from_str(&saved).unwrap();
        assert!(
            w.information
                .site(home, Vec2::ZERO, w.config.c, arrival + delay - DT)
                .unwrap()
                .system
                .industry
                .reservations
                .is_empty()
        );
    }

    #[test]
    fn a_remote_research_outpost_uses_the_existing_programme_round_trip() {
        let (mut w, owner, home, _) = scene();
        let s = site_mut(&mut w, home);
        s.pos = Vec2::new(20_000.0, 0.0);
        s.industry.outpost = Some(Outpost {
            kind: OutpostKind::Research,
            body: s.bodies[0].id,
            commodity: C::MetallicOre,
            supplied: true,
            rate: 0.0,
        });
        s.stockpile.extend([
            (C::Provisions, 100.0),
            (C::Deuterium, 100.0),
            (C::Machinery, 100.0),
            (C::Electronics, 100.0),
            (C::Alloys, 100.0),
        ]);
        w.fleets.clear();
        let issued = w.time;
        let delay = crate::transit::delay(site(&w, home).pos, Vec2::ZERO, w.config.c);
        w.step(&[Command::SetResearchQueue {
            player_id: owner,
            queue: vec!["prop_drive_tuning".into()],
        }]);
        while w.time < issued + 2.0 * delay - DT {
            assert_eq!(w.players[&owner].research.progress, 0.0);
            w.step(&[]);
        }
        while w.time < issued + 2.0 * delay + 1.0 {
            w.step(&[]);
        }
        assert!(w.players[&owner].research.progress > 0.0);
        assert!(
            w.research_contributions(owner)
                .iter()
                .any(|c| c.system == home && c.system_name.contains("outpost"))
        );
    }

    #[test]
    fn skimming_requires_research_and_station_time_then_fills_tanks_and_real_cargo() {
        // §fuel-overhaul: Ramscoop skims Helium-3. Freighters hold it as cargo;
        // it tops up the fusion tank once the corporation can burn it.
        let (mut w, owner, home, id) = scene();
        site_mut(&mut w, home).bodies[0].kind = crate::BodyKind::GasGiant;
        w.start_skimming(owner, id, home, &mut vec![]);
        assert!(w.fleets[&id].industry.is_none());
        w.players
            .get_mut(&owner)
            .unwrap()
            .research
            .completed
            .insert("prop_expedition_v_ramscoop".into());
        let f = w.fleets.get_mut(&id).unwrap();
        f.fuel = f.fuel_capacity() - 0.5;
        f.pos = Vec2::new(SKIM_RANGE_SU + 100.0, 0.0);
        let before = f.fuel;
        w.start_skimming(owner, id, home, &mut vec![]);
        pump(&mut w, 30);
        assert_eq!(w.fleets[&id].fuel, before, "no remote fuel creation");
        w.fleets.get_mut(&id).unwrap().pos = Vec2::ZERO;
        // 18 s on station: 3 Helium-3, all cargo before Fusion Reactors.
        pump(&mut w, 540);
        let f = &w.fleets[&id];
        assert_eq!(f.fuel, before, "no Helium-3 in the tank before Fusion Reactors");
        assert_eq!(f.cargo_amount(C::Helium3), 2, "harvested Helium-3 is physical cargo");
        w.players.get_mut(&owner).unwrap().research.completed.insert(crate::power::FUSION_RESEARCH.into());
        w.refresh_fleet_fuel_research();
        pump(&mut w, 120);
        let f = &w.fleets[&id];
        assert!(f.helium3 > 0.0 && f.fusion_room() < 1e-9, "the tank fills first once it can burn Helium-3");
        let mut restored: World =
            serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        pump(&mut restored, 30);
        pump(&mut w, 30);
        assert_eq!(
            restored.fleets[&id].industry, w.fleets[&id].industry,
            "fractional harvest survives checkpoints"
        );
    }

    /// §belts: `barges` Mining Barges at the star of a charted, unowned
    /// system with a belt: (world, owner, system, belt, its first ore, fleet).
    fn belt_scene(barges: u32) -> (World, PlayerId, EntityId, u32, C, EntityId) {
        let (mut w, owner, home, convoy) = scene();
        w.fleets.remove(&convoy);
        w.engagements.clear();
        let (system, body, ore) = w
            .systems
            .iter()
            .filter(|s| s.owner.is_none() && s.id != home && s.blockade.is_none())
            .find_map(|s| {
                s.bodies
                    .iter()
                    .find(|b| b.is_belt() && !b.deposits.is_empty())
                    .map(|b| (s.id, b.id, b.deposits[0].resource))
            })
            .expect("a galaxy has belts with ore");
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        let fleet = w.alloc_entity_id();
        let mut f = Fleet::single(fleet, owner, ShipKind::MiningBarge, site(&w, system).pos, FleetOrder::Idle, None);
        f.add(ShipKind::MiningBarge, barges - 1);
        w.fleets.insert(fleet, f);
        (w, owner, system, body, ore, fleet)
    }
    /// Units a second that `barges` barges alone may cut from the deposit.
    fn barge_rate(w: &World, system: EntityId, body: u32, ore: C, barges: f64) -> f64 {
        let s = site(w, system);
        let b = s.bodies.iter().find(|b| b.id == body).unwrap();
        let d = b.deposits.iter().find(|d| d.resource == ore).unwrap();
        let natural = crate::explore::natural_extraction_rate(b, d, s.trait_);
        (barges * BARGE_ORE_PER_S).min(SHIP_YIELD_MULT * natural) * crate::production::ore_bulk_ratio(ore)
    }
    fn mine_job(w: &World, fleet: EntityId) -> (f64, String) {
        match &w.fleets[&fleet].industry {
            Some(FleetIndustry::Mine { mined, status, .. }) => (*mined, status.clone()),
            other => panic!("not mining: {other:?}"),
        }
    }
    fn seconds(s: f64) -> u32 {
        (s / DT).round() as u32
    }

    #[test]
    fn a_barge_cuts_a_charted_belt_into_its_hold_loudly_and_stops_when_full() {
        let (mut w, owner, system, body, ore, fleet) = belt_scene(1);
        w.start_belt_mining(owner, fleet, system, body, (ore, false), &mut vec![]);
        assert_eq!(mine_job(&w, fleet).1, "travelling");
        pump(&mut w, seconds(20.0));
        let (mined, status) = mine_job(&w, fleet);
        let expected = barge_rate(&w, system, body, ore, 1.0) * 20.0;
        assert_eq!(status, "mining");
        assert!((mined - expected).abs() < 1e-6 * expected.max(1.0), "{mined} vs {expected}");
        assert_eq!(w.fleets[&fleet].cargo_amount(ore), (mined + 1e-6).floor() as u32, "whole units in the hold");
        assert!(w.fleets[&fleet].loud_work(), "mining is loud, like a survey");

        let free = w.fleets[&fleet].cargo_capacity() - w.fleets[&fleet].cargo_units();
        w.fleets.get_mut(&fleet).unwrap().add_cargo(ore, free - 1);
        pump(&mut w, seconds(120.0));
        let f = &w.fleets[&fleet];
        assert_eq!(f.cargo_units(), f.cargo_capacity(), "the hold fills");
        assert_eq!(mine_job(&w, fleet).1, "full");
        assert!(!w.fleets[&fleet].loud_work(), "a full barge sits quiet");

        let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(restored.fleets[&fleet].industry, w.fleets[&fleet].industry, "the job survives checkpoints");
        pump(&mut restored, 1);
    }

    #[test]
    fn barges_share_a_deposits_capped_yield_whoever_flies_them() {
        let (mut w, owner, system, body, ore, fleet) = belt_scene(4);
        let pos = site(&w, system).pos;
        let second = w.alloc_entity_id();
        let mut f = Fleet::single(second, owner, ShipKind::MiningBarge, pos, FleetOrder::Idle, None);
        f.add(ShipKind::MiningBarge, 3);
        w.fleets.insert(second, f);
        w.start_belt_mining(owner, fleet, system, body, (ore, false), &mut vec![]);
        w.start_belt_mining(owner, second, system, body, (ore, false), &mut vec![]);
        pump(&mut w, seconds(10.0));
        let (a, b) = (mine_job(&w, fleet).0, mine_job(&w, second).0);
        let cap = barge_rate(&w, system, body, ore, f64::INFINITY) * 10.0;
        assert!((a - b).abs() < 1e-9, "equal crews share equally");
        assert!(a + b <= cap + 1e-6, "eight barges never beat the cap: {} > {cap}", a + b);
        assert!(a + b >= barge_rate(&w, system, body, ore, 8.0) * 10.0 - 1e-6);
    }

    #[test]
    fn belt_orders_need_a_barge_a_charted_belt_and_no_rival_claim() {
        let (mut w, owner, system, body, ore, fleet) = belt_scene(1);
        let other = C::ALL.into_iter().find(|c| {
            !site(&w, system).bodies.iter().find(|b| b.id == body).unwrap().deposits.iter().any(|d| d.resource == *c)
        }).unwrap();
        assert_eq!(w.belt_mining_refusal(owner, fleet, system, body, (other, false)), Some("The belt holds no such ore"));
        w.players.get_mut(&owner).unwrap().surveyed.remove(&system);
        assert_eq!(w.belt_mining_refusal(owner, fleet, system, body, (ore, false)), Some("Survey the system first"));
        w.start_belt_mining(owner, fleet, system, body, (ore, false), &mut vec![]);
        assert!(w.fleets[&fleet].industry.is_none(), "an uncharted belt is refused");
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        site_mut(&mut w, system).owner = Some(PlayerId(4_242));
        assert_eq!(w.belt_mining_refusal(owner, fleet, system, body, (ore, false)), Some("Another corporation holds this system"));
        site_mut(&mut w, system).owner = None;
        let convoy = w.alloc_entity_id();
        w.fleets.insert(convoy, Fleet::single(convoy, owner, ShipKind::Convoy, Vec2::ZERO, FleetOrder::Idle, None));
        assert_eq!(w.belt_mining_refusal(owner, convoy, system, body, (ore, false)), Some("Only Mining Barges mine belts"));
        assert_eq!(w.belt_mining_refusal(owner, fleet, system, body, (ore, false)), None);

        // A rival claiming the system idles the job rather than stealing ore.
        w.start_belt_mining(owner, fleet, system, body, (ore, false), &mut vec![]);
        site_mut(&mut w, system).owner = Some(PlayerId(4_242));
        pump(&mut w, seconds(5.0));
        assert_eq!(mine_job(&w, fleet), (0.0, "claimed".into()));
    }

    #[test]
    fn a_barge_at_home_fills_the_stockpile_and_finite_reserves_run_dry() {
        let (mut w, owner, home, convoy) = scene();
        w.fleets.remove(&convoy);
        let (belt, ore) = {
            let b = site(&w, home).bodies.iter().find(|b| b.is_belt()).expect("every home has a belt");
            (b.id, b.deposits[0].resource)
        };
        let fleet = w.alloc_entity_id();
        w.fleets.insert(fleet, Fleet::single(fleet, owner, ShipKind::MiningBarge, Vec2::ZERO, FleetOrder::Idle, None));
        w.start_belt_mining(owner, fleet, home, belt, (ore, false), &mut vec![]);
        pump(&mut w, seconds(10.0));
        let stocked = site(&w, home).stockpile.get(&ore).copied().unwrap_or(0.0);
        assert!(stocked > 0.0 && (stocked - mine_job(&w, fleet).0).abs() < 1e-9, "your own belt fills your stockpile");
        assert_eq!(w.fleets[&fleet].cargo_units(), 0);

        site_mut(&mut w, home).bodies.iter_mut().find(|b| b.id == belt).unwrap().deposits[0].reserves = Some(0.5);
        pump(&mut w, seconds(30.0));
        assert_eq!(mine_job(&w, fleet).1, "depleted");
        assert_eq!(site(&w, home).bodies.iter().find(|b| b.id == belt).unwrap().deposits[0].reserves, Some(0.0));
    }

    /// §pockets: an unowned belt with base ore and at least one hidden pocket,
    /// charted by `owner`, and a fleet of `kind` parked at its star.
    fn pocket_scene(kind: ShipKind) -> (World, PlayerId, EntityId, u32, EntityId) {
        let (mut w, owner, home, convoy) = scene();
        w.fleets.remove(&convoy);
        w.engagements.clear();
        let (system, body) = w
            .systems
            .iter()
            .filter(|s| s.owner.is_none() && s.id != home && s.blockade.is_none())
            .find_map(|s| {
                s.bodies
                    .iter()
                    .find(|b| b.is_belt() && !b.deposits.is_empty() && !b.pockets.is_empty())
                    .map(|b| (s.id, b.id))
            })
            .expect("a galaxy has belts with pockets");
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        let fleet = w.alloc_entity_id();
        w.fleets.insert(fleet, Fleet::single(fleet, owner, kind, site(&w, system).pos, FleetOrder::Idle, None));
        (w, owner, system, body, fleet)
    }
    fn belt(w: &World, system: EntityId, body: u32) -> &crate::Body {
        site(w, system).bodies.iter().find(|b| b.id == body).unwrap()
    }
    fn deliver(w: &mut World) {
        let last = w.pending_pocket_reports.iter().map(|r| r.arrive_at).fold(w.time, f64::max);
        w.time = last;
        w.deliver_pocket_reports();
    }

    #[test]
    fn prospecting_finds_pockets_loudly_privately_and_by_light() {
        let (mut w, owner, system, body, scout) = pocket_scene(ShipKind::Scout);
        assert_eq!(w.prospect_refusal(owner, scout, system, body), None);
        let mut events = Vec::new();
        w.start_prospecting(owner, scout, system, body, &mut events);
        pump(&mut w, 1);
        assert!(matches!(&w.fleets[&scout].industry, Some(FleetIndustry::Prospect { status, .. }) if status == "prospecting"));
        assert!(w.fleets[&scout].loud_work(), "prospecting is loud, like a survey");
        pump(&mut w, seconds(PROSPECT_SECS) - 1);
        assert!(w.fleets[&scout].industry.is_none(), "the pass is done");
        assert_eq!(belt(&w, system, body).prospected[&owner].passes, 1);
        assert_eq!(belt(&w, system, body).prospected[&owner].reported, 0, "the report is still on its way");
        assert_eq!(w.pending_pocket_reports.len(), 1);
        assert!(belt(&w, system, body).pockets.iter().all(|p| p.found_by.is_empty()), "nothing is known before the light lands");

        // Pass after pass, every pocket turns up; each report lands by light.
        let total = belt(&w, system, body).pockets.len();
        let mut found = 0;
        for pass in 1..24 {
            found += w.pending_pocket_reports.iter().map(|r| r.found.len()).sum::<usize>();
            deliver(&mut w);
            assert_eq!(belt(&w, system, body).prospected[&owner].reported, pass);
            if belt(&w, system, body).pockets.iter().all(|p| p.found_by.contains(&owner)) {
                break;
            }
            w.start_prospecting(owner, scout, system, body, &mut events);
            pump(&mut w, seconds(PROSPECT_SECS));
        }
        let b = belt(&w, system, body);
        assert!(b.pockets.iter().all(|p| p.found_by == [owner].into()), "found, and only by the prospector");
        assert_eq!(found, total, "each pocket is reported once");
    }

    #[test]
    fn prospecting_rolls_are_keyed_and_need_a_prospector_and_a_chart() {
        let (mut w, owner, system, body, scout) = pocket_scene(ShipKind::Scout);
        let mut twin: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        for world in [&mut w, &mut twin] {
            for _ in 0..3 {
                world.start_prospecting(owner, scout, system, body, &mut vec![]);
                pump(world, seconds(PROSPECT_SECS));
            }
        }
        let finds = |w: &World| w.pending_pocket_reports.iter().map(|r| r.found.clone()).collect::<Vec<_>>();
        assert_eq!(finds(&w), finds(&twin), "the same passes find the same pockets");

        let convoy = w.alloc_entity_id();
        w.fleets.insert(convoy, Fleet::single(convoy, owner, ShipKind::Convoy, Vec2::ZERO, FleetOrder::Idle, None));
        assert_eq!(w.prospect_refusal(owner, convoy, system, body), Some("Only Scouts and Mining Barges prospect"));
        w.players.get_mut(&owner).unwrap().surveyed.remove(&system);
        assert_eq!(w.prospect_refusal(owner, scout, system, body), Some("Survey the system first"));
        w.start_prospecting(owner, scout, system, body, &mut vec![]);
        assert!(w.fleets[&scout].industry.is_none(), "an uncharted belt is refused");
        w.players.get_mut(&owner).unwrap().surveyed.insert(system);
        let barge = w.alloc_entity_id();
        w.fleets.insert(barge, Fleet::single(barge, owner, ShipKind::MiningBarge, Vec2::ZERO, FleetOrder::Idle, None));
        assert_eq!(w.prospect_refusal(owner, barge, system, body), None, "a barge prospects too");
    }

    #[test]
    fn only_found_pockets_are_mined_and_barges_strike_them_while_cutting() {
        let (mut w, owner, system, body, barge) = pocket_scene(ShipKind::MiningBarge);
        let ore = belt(&w, system, body).pockets[0].deposit.resource;
        assert_eq!(w.belt_mining_refusal(owner, barge, system, body, (ore, true)), Some("No pocket of that ore found"));

        // A strike: the barge finds a pocket while cutting, and the report
        // lands by light without counting as a prospecting pass.
        let mut events = Vec::new();
        let mut struck = None;
        for tick in 0..20_000 {
            w.tick = tick;
            w.roll_pocket_strike(barge, owner, (system, body), 5_000, &mut events);
            if let Some(r) = w.pending_pocket_reports.first() {
                struck = Some(r.found[0]);
                break;
            }
        }
        let struck = struck.expect("a big enough crew strikes soon");
        assert!(w.pending_pocket_reports[0].strike);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::PocketStruck { ore, .. } if ore == struck)));
        for tick in 20_000..20_100 {
            w.tick = tick;
            w.roll_pocket_strike(barge, owner, (system, body), 1_000_000, &mut events);
        }
        assert_eq!(w.pending_pocket_reports.iter().filter(|r| r.found.contains(&struck)).count(), 1,
            "a find on its way home is not struck twice");
        deliver(&mut w);
        assert!(belt(&w, system, body).found_pocket(struck, owner).is_some());
        assert!(belt(&w, system, body).prospected.get(&owner).is_none(), "a strike is not a prospecting pass");

        // Mining the found pocket draws its reserves, not the belt's ore.
        let base = belt(&w, system, body).deposits.clone();
        let before = belt(&w, system, body).found_pocket(struck, owner).unwrap().deposit.reserves.unwrap();
        assert_eq!(w.belt_mining_refusal(owner, barge, system, body, (struck, true)), None);
        w.start_belt_mining(owner, barge, system, body, (struck, true), &mut vec![]);
        pump(&mut w, seconds(30.0));
        let (mined, status) = mine_job(&w, barge);
        assert_eq!(status, "mining");
        let after = belt(&w, system, body).found_pocket(struck, owner).unwrap().deposit.reserves.unwrap();
        assert!((before - after - mined).abs() < 1e-6, "the pocket gives up exactly what was cut");
        assert_eq!(belt(&w, system, body).deposits, base, "the belt's own ore is untouched");

        // A rival who hasn't found it may not mine it.
        let rival = PlayerId(5_151);
        let theirs = w.alloc_entity_id();
        w.fleets.insert(theirs, Fleet::single(theirs, rival, ShipKind::MiningBarge, Vec2::ZERO, FleetOrder::Idle, None));
        w.players.insert(rival, w.players[&owner].clone());
        assert_eq!(w.belt_mining_refusal(rival, theirs, system, body, (struck, true)), Some("No pocket of that ore found"));
    }

    #[test]
    fn a_home_mine_picks_up_the_cuprite_pocket_once_it_is_found() {
        let (mut w, owner, home, _) = scene();
        let belt_id = site(&w, home).bodies.iter().find(|b| b.is_belt()).unwrap().id;
        let run = |w: &mut World| {
            let s = site_mut(w, home);
            s.stockpile = [(C::Provisions, 50.0)].into();
            let b = s.bodies.iter_mut().find(|b| b.id == belt_id).unwrap();
            b.set_tier(crate::StructureKind::MiningComplex, 1);
            b.assignments.insert(crate::StructureKind::MiningComplex, crate::production::Assignment::crew(1));
            w.accrue_production(&mut Vec::new());
            let s = site(w, home);
            [C::CrystallineOre, C::CupriteOre].map(|c| s.stockpile.get(&c).copied().unwrap_or(0.0))
        };
        let [crystal, cuprite] = run(&mut w);
        assert!(crystal > 0.0 && cuprite == 0.0, "an unfound pocket is not mined");
        let reserves = |w: &World| belt(w, home, belt_id).pockets[0].deposit.reserves.unwrap();
        let full = reserves(&w);
        site_mut(&mut w, home).bodies.iter_mut().find(|b| b.id == belt_id).unwrap().pockets[0].found_by.insert(owner);
        let [_, cuprite] = run(&mut w);
        assert!(cuprite > 0.0, "the found Cuprite pocket joins the mine's output");
        assert!((full - reserves(&w) - cuprite).abs() < 1e-9, "and runs down by what it gave");
    }
}
