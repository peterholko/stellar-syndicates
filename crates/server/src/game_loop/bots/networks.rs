//! Owned hulls join a network of reported ports; the Market is one destination,
//! not the only destination. No route reserves goods that have not arrived.
use super::*;
use sim::industry::{FleetIndustry, FreightPort, FreightRoute, RoutePhase, RouteStop};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SupplyDispatch {
    from: EntityId, to: EntityId, goods: BTreeMap<C, u32>, pub until: f64,
}

impl Observation {
    pub(super) fn colony_pickups(&self, good: C, outbound: bool) -> u32 {
        let mut sources: BTreeMap<_, _> = self.sites.iter().map(|s|
            (s.system.id, s.system.free_stock(good).floor().max(0.0) as u32)).collect();
        let mut total = 0;
        for f in self.own().filter(|f| f.kind.is_player_freighter() && !f.stalled && !self.fighting.contains(&f.id)) {
            let Some(FleetIndustry::Route { run }) = &f.industry else { continue; };
            if run.phase == RoutePhase::Complete || run.hold.is_some() { continue; }
            // Only a future physical pickup at an owned source counts, bounded
            // by its REPORTED goods and the hull's hold. Allocate each source
            // once across carriers, rather than promising its last crate twice.
            for (index, stop) in run.route.stops.iter().enumerate().skip(run.stop) {
                let FreightPort::System { id: from } = stop.port else { continue; };
                let Some(destination) = run.route.stops.iter().skip(index + 1).find(|s|
                    matches!(s.port, FreightPort::System { id } if id != from) && s.unload.contains_key(&good)) else { continue; };
                let FreightPort::System { id: to } = destination.port else { continue; };
                let n = if index == run.stop && matches!(run.phase, RoutePhase::Loading | RoutePhase::Fuel | RoutePhase::Escort) {
                    run.remaining_load.get(&good).copied().unwrap_or(0)
                } else { stop.load.get(&good).copied().unwrap_or(0) };
                let available = sources.entry(from).or_default();
                let n = n.min(*available).min(capacity(f));
                *available -= n;
                if if outbound { from == self.home.system.id } else { to == self.home.system.id } { total += n; }
            }
        }
        total
    }
}

impl Bot {
    pub(super) fn pending_supply(&self, p: &Observation, good: C, outbound: bool) -> u32 {
        self.supply_dispatches.values().filter(|d| d.until > p.now
            && if outbound { d.from == p.home.system.id } else { d.to == p.home.system.id })
            .map(|d| d.goods.get(&good).copied().unwrap_or(0)).sum()
    }

    pub(super) fn reserved_supply(&self, p: &Observation, good: C) -> u32 {
        p.colony_pickups(good, true) + self.pending_supply(p, good, true)
    }

    pub(super) fn expected_supply(&self, p: &Observation, good: C) -> u32 {
        p.colony_pickups(good, false) + self.pending_supply(p, good, false)
    }

    pub(super) fn refresh_network(&mut self, p: &Observation) {
        // Pending dispatches bridge command/report latency only. Once the
        // route itself is visible, its remaining pickup replaces the promise;
        // rejection/loss/timeout releases it rather than inventing cargo.
        self.supply_dispatches.retain(|id, d| d.until > p.now && p.own().any(|f|
            f.id == *id && !matches!(&f.industry, Some(FleetIndustry::Route { run })
                if run.phase != RoutePhase::Complete && run.hold.is_none() && run.route.name == "Inter-colony supply"
                    && run.route.stops.first().is_some_and(|s| s.port == FreightPort::System { id: d.from })
                    && run.route.stops.get(1).is_some_and(|s| s.port == FreightPort::System { id: d.to }))));
        self.freight_homes.retain(|fleet, system| p.own().any(|f| f.id == *fleet)
            && p.sites.iter().any(|s| s.system.id == *system));
        for site in &p.sites {
            if self.freight_homes.values().any(|id| *id == site.system.id) { continue; }
            let donor = p.own().filter(|f| f.kind.is_player_freighter() && f.docked.is_some()
                && !self.belt_haulers.contains_key(&f.id)
                && f.vel.length_sq() < 1.0 && !p.fighting.contains(&f.id)
                && self.ready(&format!("fleet:{}", f.id), p.now)
                && !matches!(&f.industry, Some(FleetIndustry::Route { run }) if run.phase != RoutePhase::Complete))
                .find(|f| self.freight_homes.get(&f.id).is_some_and(|port|
                    self.freight_homes.values().filter(|id| *id == port).count() > 1)).map(|f| f.id);
            if let Some(id) = donor { self.freight_homes.insert(id, site.system.id); }
        }
        for f in p.own().filter(|f| f.kind.is_player_freighter()) {
            if self.freight_homes.contains_key(&f.id) || self.belt_haulers.contains_key(&f.id) { continue; }
            let site = p.sites.iter().min_by(|a, b| {
                let score = |site: &sim::information::SiteReport| {
                    let assigned = self.freight_homes.values().filter(|id| **id == site.system.id).count();
                    (assigned, f.pos.distance(site.system.pos))
                };
                let (ac, ad) = score(a); let (bc, bd) = score(b);
                ac.cmp(&bc).then_with(|| ad.total_cmp(&bd)).then(a.system.id.cmp(&b.system.id))
            });
            if let Some(site) = site { self.freight_homes.insert(f.id, site.system.id); }
        }
    }

    pub(super) fn site_project(&self, p: &Observation, site: &sim::information::SiteReport) -> Option<Project> {
        if site.system.id == p.home.system.id { self.project(p) }
        else { self.strategic_project(&p.at_site(site)) }
    }

    fn intercolony_route(&mut self, p: &Observation) -> Option<Command> {
        for fleet in p.own().filter(|f| f.kind.is_player_freighter() && !f.stalled && !self.belt_haulers.contains_key(&f.id)) {
            let key = format!("fleet:{}", fleet.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&fleet.id) || self.returns.contains_key(&fleet.id) { continue; }
            if matches!(&fleet.industry, Some(FleetIndustry::Route { run })
                if run.phase != RoutePhase::Complete && run.hold.is_none()) { continue; }
            let Some(source) = p.sites.iter().find(|s| fleet.docked.as_deref() == Some(s.system.id.to_string().as_str())) else { continue; };
            let local = p.at_site(source);
            let reserve = self.desired_stock(&local, self.site_project(p, source));
            let mut economy = fleet.clone(); economy.transit = Some(p.freight_transit());
            let mut best = None;
            for dest in p.sites.iter().filter(|s| s.system.id != source.system.id && s.system.blockade.is_none()) {
                if p.threatened(dest.system.pos) { continue; }
                let remote = p.at_site(dest);
                let wanted = self.desired_stock(&remote, self.site_project(p, dest));
                let mut room = local.safe_load(&economy, source.system.pos, dest.system.pos)
                    .min(dest.system.storage_headroom().floor().max(0.0) as u32);
                let mut manifest = BTreeMap::new();
                let mut goods: Vec<_> = wanted.iter().collect();
                goods.sort_by_key(|(c, _)| (**c != C::Provisions && !FUELS.contains(*c), **c));
                for (&good, &target) in goods {
                    let spare = (source.system.free_stock(good) - reserve.get(&good).copied().unwrap_or(0.0)).floor().max(0.0) as u32;
                    let need = (target - remote.stock(good) - remote.inbound(good) as f64 - remote.carried_home(good) as f64
                        - self.expected_supply(&remote, good) as f64).ceil().max(0.0) as u32;
                    let n = spare.min(need).min(room);
                    if n > 0 { manifest.insert(good, n); room -= n; }
                }
                let units = manifest.values().sum::<u32>();
                if units == 0 { continue; }
                let outbound = p.leg(&economy, source.system.pos, dest.system.pos, units);
                let return_burn = p.leg(&economy, dest.system.pos, source.system.pos, 0);
                let escort = p.own().find(|f| f.guard_target == Some(fleet.id));
                let have = store(&economy);
                if have < outbound + p.arrival_reserve(fleet)
                    || dest.system.free_stock(regime_fuel(&economy)) < return_burn && have < outbound + return_burn + p.arrival_reserve(fleet)
                    || escort.is_some_and(|g| {
                        let (fuel, mult) = p.escort_burn(g, on_uranium(&economy));
                        fuel < p.fuel_per_mass(source.system.pos, dest.system.pos) * hull_mass(g) * mult * 2.0 + p.arrival_reserve(g)
                    }) { continue; }
                let value = manifest.iter().map(|(&c, &n)| p.price(c) * n as f64).sum::<f64>()
                    / (1.0 + source.system.pos.distance(dest.system.pos) / 80_000.0);
                if best.as_ref().is_none_or(|(v, _, _)| value > *v) { best = Some((value, dest, manifest)); }
            }
            if let Some((_, dest, manifest)) = best {
                // Source unload first makes retained imported cargo explicit;
                // never add a fresh full manifest on top of an occupied hold.
                let unload = fleet.cargo_manifest.iter().map(|c| (c.commodity, c.units)).collect();
                self.wait(key, p, fleet.pos);
                self.supply_dispatches.insert(fleet.id, SupplyDispatch { from: source.system.id, to: dest.system.id,
                    goods: manifest.clone(), until: p.now + p.round_trip(fleet.pos) });
                self.status = "hauling a specialized colony's surplus to an actual shortage".into();
                return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id,
                    route: Some(FreightRoute { name: "Inter-colony supply".into(), repeat: false,
                        shopping_budget: None,
                        transit: p.freight_transit(), fuel_reserve: p.arrival_reserve(fleet),
                        escort: p.own().find(|g| g.guard_target == Some(fleet.id)).map(|g| g.id),
                        stops: vec![RouteStop { port: FreightPort::System { id: source.system.id },
                            load: manifest.clone(), unload, sell: false },
                            RouteStop { port: FreightPort::System { id: dest.system.id },
                                load: BTreeMap::new(), unload: manifest, sell: false }],
                    }) });
            }
        }
        None
    }

    pub(super) fn network_routes(&mut self, p: &Observation) -> Option<Command> {
        if let Some(command) = self.intercolony_route(p) { return Some(command); }
        for site in &p.sites {
            let local = p.at_site(site);
            let wanted = self.desired_stock(&local, self.site_project(p, site));
            if let Some(command) = self.routes(&local, &wanted) { return Some(command); }
        }
        // Rebase spare/returning freighters to their assigned port. Only idle
        // reported berths qualify; don't overwrite an in-flight delivery.
        for fleet in p.own().filter(|f| f.kind.is_player_freighter() && !f.stalled && !self.belt_haulers.contains_key(&f.id)) {
            let key = format!("fleet:{}", fleet.id);
            if !self.ready(&key, p.now) || fleet.vel.length_sq() > 1.0 || p.fighting.contains(&fleet.id)
                || matches!(&fleet.industry, Some(FleetIndustry::Route { run }) if run.phase != RoutePhase::Complete && run.hold.is_none()) { continue; }
            let Some(site) = self.freight_homes.get(&fleet.id).and_then(|id| p.sites.iter().find(|s| s.system.id == *id)) else { continue; };
            if fleet.pos.distance(site.system.pos) > sim::transit::HYPERLIMIT && p.enough_fuel(fleet, site.system.pos) {
                self.wait(key, p, fleet.pos);
                return Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: site.system.pos });
            }
        }
        None
    }

    pub(super) fn network_economy(&mut self, p: &Observation, home_wanted: &BTreeMap<C, f64>) -> Option<Command> {
        let count = p.sites.len();
        for offset in 0..count {
            let index = (self.economy_turn + offset) % count;
            let site = &p.sites[index];
            let local = p.at_site(site);
            let project = self.site_project(p, site);
            let wanted = if site.system.id == p.home.system.id { home_wanted.clone() }
                else { self.desired_stock(&local, project) };
            let key = format!("build:{}", site.system.id);
            if site.system.id != p.home.system.id && self.ready(&key, p.now) && site.builds.is_empty()
                && !self.recovering(p) {
                if let Some(project) = project.filter(|j| local.project_costs(*j).iter().all(|(&c, n)| local.stock(c) >= *n)) {
                    self.wait(key, p, site.system.pos);
                    self.economy_turn = index + 1;
                    return Some(match project.what {
                        BuildKind::Upgrade { upgrade } => Command::DevelopSystem { player_id: p.owner, system_id: site.system.id, upgrade, body_id: Some(project.body) },
                        BuildKind::Ship { ship } => Command::BuildShip { player_id: p.owner, system_id: site.system.id, ship_kind: ship, join: None, loadout: Default::default() },
                        _ => continue,
                    });
                }
            }
            if let Some(command) = self.market(&local, &wanted) {
                self.economy_turn = index + 1;
                return Some(command);
            }
            if site.builds.is_empty() && self.ready(&key, p.now) {
                if let Some(specialist) = self.specialist_need(&local).filter(|_| sim::specialist::ACADEMY_TRAIN_COSTS.iter()
                    .all(|(c, n)| local.stock(*c) >= wanted.get(c).copied().unwrap_or(*n))) {
                    self.wait(key, p, site.system.pos);
                    self.economy_turn = index + 1;
                    return Some(Command::TrainSpecialist { player_id: p.owner, system_id: site.system.id, specialist });
                }
            }
        }
        None
    }
}
