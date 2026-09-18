//! Onboard routes and local industry. No remote inventory/ownership read can
//! dispatch a leg: cargo work happens at a real berth, and progress is carried
//! in the same immutable fleet/site reports as all other physical work.
use super::*;
use crate::Commodity as C;
use crate::industry::FreightRun;
use crate::industry::*;

impl World {
    fn industry_reject(&self, owner: PlayerId, fleet: EntityId, events: &mut Vec<Event>) {
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
        }
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
        let valid = (2..=MAX_STOPS).contains(&route.stops.len())
            && !route.name.trim().is_empty()
            && route.name.chars().count() <= 48
            && route.fuel_reserve.is_finite()
            && route.fuel_reserve >= 0.0
            && route.fuel_reserve <= self.fleets[&id].fuel_capacity()
            && route.escort != Some(id)
            && (route.stops.len() <= 2
                || self.research_flag(owner, crate::research::Cap::AutonomousFreight))
            && route.stops.iter().all(|s| {
                self.freight_port_pos(s.port).is_some()
                    && (!s.sell || s.port == FreightPort::Hub)
                    && s.load.values().map(|n| *n as u64).sum::<u64>() <= capacity
                    && s.unload.values().map(|n| *n as u64).sum::<u64>() <= capacity
            })
            && route.stops.windows(2).all(|s| s[0].port != s[1].port);
        if !valid {
            self.industry_reject(owner, id, events);
            return;
        }
        let f = self.fleets.get_mut(&id).unwrap();
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
                    b.id == body
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
    pub(super) fn start_colony_project(
        &mut self,
        owner: PlayerId,
        id: EntityId,
        body: u32,
        kind: ColonyProjectKind,
        commodity: C,
    ) -> Result<(), &'static str> {
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
            ColonyProjectKind::OrbitalAssembly => b.tier(crate::StructureKind::Shipyard) >= 2,
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
        if s.blockade.is_some() {
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
                        let mut amount = SKIM_FUEL_PER_S * DT;
                        let tank = (f.fuel_capacity() - f.fuel).max(0.0).min(amount);
                        f.fuel += tank;
                        f.stalled = false;
                        amount -= tank;
                        harvested += tank;
                        if f.has_freighter() && f.cargo_units() < f.cargo_capacity() {
                            cargo_fraction += amount;
                            let n = (cargo_fraction.floor() as u32)
                                .min(f.cargo_capacity() - f.cargo_units());
                            f.add_cargo(C::Fuel, n);
                            cargo_fraction -= n as f64;
                            harvested += n as f64;
                        }
                        if f.fuel + 1e-9 >= f.fuel_capacity()
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
            FreightPort::Hub => None,
        };
        let access = system.is_none_or(|sid| {
            self.systems
                .iter()
                .any(|s| s.id == sid && s.owner == Some(owner) && s.blockade.is_none())
        });
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
        }
        if run.phase == RoutePhase::Unloading {
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
                if stop.sell {
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
            if run.remaining_unload.values().all(|n| *n == 0) {
                run.phase = RoutePhase::Loading;
            }
        }
        if run.phase == RoutePhase::Loading {
            for (&c, left) in &mut run.remaining_load {
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

    fn freight_departure_hold(
        &self,
        id: EntityId,
        route: &FreightRoute,
        dest: Vec2,
    ) -> Option<String> {
        let f = &self.fleets[&id];
        // Price every intersected public gravity well, not just the endpoints.
        // This uses static geography and onboard tanks/cargo, never a remote
        // port's unarrived ownership/stock report. Leave a 15% travel margin.
        let distance = f.pos.distance(dest);
        let direction = if distance > 0.0 {
            (dest - f.pos) / distance
        } else {
            Vec2::ZERO
        };
        let mut intervals = Vec::new();
        for center in self
            .systems
            .iter()
            .map(|s| s.pos)
            .chain(std::iter::once(self.hub))
        {
            let d = center - f.pos;
            let along = d.x * direction.x + d.y * direction.y;
            let lateral2 = (d.x * d.x + d.y * d.y - along * along).max(0.0);
            let radius = crate::transit::HYPERLIMIT;
            if lateral2 < radius * radius {
                let reach = (radius * radius - lateral2).sqrt();
                let start = (along - reach).max(0.0);
                let end = (along + reach).min(distance);
                if end > start {
                    intervals.push((start, end));
                }
            }
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut impulse = 0.0;
        let mut end = 0.0_f64;
        for (a, b) in intervals {
            impulse += (b - a.max(end)).max(0.0);
            end = end.max(b);
        }
        let budget = crate::fuel::fuel_cost(
            impulse + (distance - impulse) / crate::transit::WARP_FACTOR,
            f.mass(),
        ) * 1.15
            + route.fuel_reserve;
        let reason = if budget > f.fuel_capacity() {
            "Leg exceeds tank range; shorten route or reduce cargo"
        } else if f.fuel + 1e-9 < budget {
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
            !g.supplied
                || g.fuel + 1e-9 < (budget - route.fuel_reserve) * g.mass() / f.mass().max(1.0)
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
            let safe = s.blockade.is_none();
            if let Some(mut outpost) = s.industry.outpost.take() {
                let inputs = [
                    (C::Provisions, 0.03),
                    (C::Fuel, 0.02),
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
            name: "Mixed outward / alloy return".into(),
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
    fn pump(w: &mut World, ticks: u32) {
        for _ in 0..ticks {
            w.time += DT;
            w.tick_industrial_expansion(&mut vec![]);
        }
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
        site_mut(&mut w, home)
            .stockpile
            .insert(C::MetallicOre, cap - 2.0);
        w.fleets.get_mut(&id).unwrap().add_cargo(C::Alloys, 7);
        w.set_freight_route(owner, id, Some(route(home)), &mut vec![]);
        pump(&mut w, 1);
        assert_eq!(w.fleets[&id].cargo_amount(C::Alloys), 5);
        assert_eq!(run(&w, id).remaining_unload[&C::Alloys], 5);
        site_mut(&mut w, home).owner = Some(PlayerId(912));
        pump(&mut w, 2);
        assert_eq!(run(&w, id).remaining_unload[&C::Alloys], 5);
        let mut w: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        site_mut(&mut w, home).owner = Some(owner);
        site_mut(&mut w, home).stockpile.insert(C::MetallicOre, 0.0);
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
        s.stockpile.extend([(C::Composites, 20.0), (C::Fuel, 20.0)]);
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
        assert!(s.stockpile[&C::HullSections] > 0.0 && s.stockpile[&C::DriveAssemblies] > 0.0);
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
            (C::Fuel, 10.0),
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
            (C::Fuel, 100.0),
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
        pump(&mut w, 90);
        let f = &w.fleets[&id];
        assert_eq!(f.fuel, f.fuel_capacity());
        assert_eq!(
            f.cargo_amount(C::Fuel),
            2,
            "excess harvested Fuel is physical cargo"
        );
        let mut restored: World =
            serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        pump(&mut restored, 30);
        pump(&mut w, 30);
        assert_eq!(
            restored.fleets[&id].industry, w.fleets[&id].industry,
            "fractional harvest survives checkpoints"
        );
    }
}
