use super::*;
use sim::industry::{FleetIndustry, FreightPort, FreightRoute, RoutePhase, RouteStop};

impl Observation {
    /// Count real homebound cargo, not an outbound ore hold that will be sold.
    pub(super) fn carried_home(&self, good: C) -> u32 {
        self.own().filter(|f| f.docked.as_deref() != Some(self.home.system.id.to_string().as_str()))
            .filter(|f| matches!(&f.industry, Some(FleetIndustry::Route { run })
                if run.phase != RoutePhase::Complete && run.route.stops.iter()
                    .skip(run.stop + usize::from(matches!(run.phase, RoutePhase::Loading | RoutePhase::Fuel | RoutePhase::Escort)))
                    .find(|s| s.unload.contains_key(&good))
                    .is_some_and(|s| s.port == FreightPort::System { id: self.home.system.id })))
            .map(|f| cargo(f, good)).sum()
    }

    /// A manifest is NOT inbound freight. Cover only its uncollected quantity,
    /// with warehouse stock, a working carrier and time to beat depletion. The
    /// surplus above that quantity needs another carrier (or paid Authority).
    fn pickup_coverage(&self, good: C) -> u32 {
        let mut warehouse = self.account.as_ref().and_then(|a| a.warehouse.get(&good)).copied().unwrap_or(0);
        let mut covered = 0;
        for f in self.own().filter(|f| f.kind.is_player_freighter() && !f.stalled && !self.fighting.contains(&f.id)) {
            let Some(FleetIndustry::Route { run }) = &f.industry else { continue; };
            if run.phase == RoutePhase::Complete || run.hold.is_some() { continue; }
            let home = FreightPort::System { id: self.home.system.id };
            let future: Vec<_> = run.route.stops.iter().enumerate().skip(run.stop).collect();
            let Some(&(index, hub)) = future.iter().find(|(_, s)| s.port == FreightPort::Hub) else { continue; };
            if !run.route.repeat && !future.iter().any(|(i, s)| *i > index && s.port == home) { continue; }
            let units = if index == run.stop && matches!(run.phase, RoutePhase::Loading | RoutePhase::Fuel | RoutePhase::Escort) {
                run.remaining_load.get(&good).copied().unwrap_or(0)
            } else { hub.load.get(&good).copied().unwrap_or(0) };
            let speed = f.kind.max_speed() * sim::transit::WARP_FACTOR
                * if run.route.transit == sim::ship::TransitMode::Economy { sim::ship::ECONOMY_SPEED_FRACTION } else { 1.0 };
            let eta = (f.pos.distance(self.hub) + self.hub.distance(self.home.system.pos)) / speed
                + self.round_trip(f.pos) + 120.0;
            let drain = self.research.active.as_deref().and_then(sim::research::programme).map_or(0.0, |r|
                sim::research::basket(r.field, r.tier).into_iter().find(|(c, _)| *c == good).map_or(0.0, |(_, n)| n)
                    * self.home.academies.iter().map(|a| a.rate).sum::<f64>().max(1.0));
            let deadline = if good == C::Fuel && self.fuel_critical() || good == C::Provisions && self.stock(good) < 10.0 {
                self.supply_lead()
            } else if drain > 0.0 { (self.home.system.free_stock(good) / drain).max(self.supply_lead()) }
            else { self.supply_lead() * 3.0 };
            if eta > deadline { continue; }
            let n = units.min(capacity(f)).min(warehouse);
            covered += n;
            warehouse -= n;
        }
        covered
    }

    pub(super) fn exports(&self, wanted: &BTreeMap<C, f64>, limit: u32) -> BTreeMap<C, u32> {
        let mut goods: Vec<_> = self.home.system.stockpile.keys().filter_map(|&good| {
            let reserve = self.export_reserve(good, wanted)?;
            Some((good, (self.home.system.free_stock(good) - reserve).floor().max(0.0) as u32))
        }).filter(|(_, n)| *n > 0).collect();
        goods.sort_by(|(a, _), (b, _)| self.price(*b).total_cmp(&self.price(*a)));
        let mut room = limit;
        goods.into_iter().filter_map(|(good, n)| {
            let n = n.min(room); room -= n; (n > 0).then_some((good, n))
        }).collect()
    }

    /// Compare replacement fuel (including the escort and return cargo), not
    /// just credits received at the hub. Starting fuel is not free recurring
    /// income. Negative-margin Tiny/low-value-ore circuits use paid bulk freight
    /// instead; no bot-only fuel discount hides a real balance problem.
    pub(super) fn route_margin(&self, fleet: &GhostView, exports: &BTreeMap<C, u32>, imports: &BTreeMap<C, u32>, escort: Option<&GhostView>) -> f64 {
        let home = self.home.system.pos;
        let mut fuel = self.leg(fleet, home, self.hub, exports.values().sum())
            + self.leg(fleet, self.hub, home, imports.values().sum());
        if let Some(guard) = escort { fuel += self.leg(guard, home, self.hub, 0) * 2.0; }
        // The return goods are needed regardless of carrier. Count the freight
        // fee saved, not their purchase price, as the backhaul's contribution.
        let saved = imports.iter().map(|(&c, &n)| sim::tca::freight_fee(n, self.price(c), home.distance(self.hub))).sum::<f64>();
        exports.iter().map(|(&c, &n)| n as f64 * self.price(c) * 0.95).sum::<f64>()
            + saved - fuel * self.price(C::Fuel) * 1.10
    }
}

impl Bot {
    pub(super) fn guards(&mut self, p: &Observation) -> Option<Command> {
        let freighters: Vec<_> = p.own().filter(|g| g.kind.is_player_freighter()).collect();
        for guard in p.own().filter(|g| g.kind == H::Raider
            && warfare::composition(g).values().sum::<u32>() == 1) {
            let key = format!("fleet:{}", guard.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&guard.id) || guard.stalled
                || self.campaign_fleet(guard.id) || guard.damage.unwrap_or(0.0) > 0.2 { continue; }
            if guard.fuel.unwrap_or(0.0) < p.tank(guard) * 0.35 && guard.docked.is_none() {
                if let Some(cmd) = self.return_for_fuel(p, guard) { return Some(cmd); }
                continue;
            }
            if self.returns.contains_key(&guard.id) { continue; }
            // Preserve assignments when a different formation merges or is
            // lost. Enumeration indices are not stable identities for escorts.
            if guard.guard_target.is_some_and(|id| freighters.iter().any(|f| f.id == id))
                || guard.defend_system.as_ref().is_some_and(|d| p.sites.iter().any(|s| s.system.id == d.system)) { continue; }
            if let Some(target) = freighters.iter().find(|f|
                self.ready(&format!("escort:{}", f.id), p.now) && !p.own().any(|g| g.guard_target == Some(f.id))) {
                self.wait(key, p, guard.pos);
                self.wait(format!("escort:{}", target.id), p, guard.pos);
                self.status = "assigning a persistent freight escort".into();
                return Some(Command::GuardFleet { player_id: p.owner, interceptor_id: guard.id, target_id: target.id });
            } else if let Some(site) = p.sites.iter().find(|s|
                self.ready(&format!("defend:{}", s.system.id), p.now)
                    && !p.own().any(|g| g.defend_system.as_ref().is_some_and(|d| d.system == s.system.id))) {
                self.wait(key, p, guard.pos);
                self.wait(format!("defend:{}", site.system.id), p, guard.pos);
                return Some(Command::DefendSystem { player_id: p.owner, fleet_id: guard.id,
                    system_id: site.system.id, pursuit_radius: 20_000.0 });
            }
        }
        None
    }

    pub(super) fn routes(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>) -> Option<Command> {
        if p.opening_local_work() { return None; }
        let account = p.account.as_ref()?;
        for fleet in p.own().filter(|g| g.kind.is_player_freighter()) {
            if self.freight_homes.get(&fleet.id).is_some_and(|id| *id != p.home.system.id) { continue; }
            let key = format!("fleet:{}", fleet.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&fleet.id) || fleet.stalled { continue; }
            let run = match &fleet.industry { Some(FleetIndustry::Route { run }) => Some(run), _ => None };
            let at_hub = fleet.docked.as_deref() == Some("hub");
            let at_home = fleet.docked.as_deref() == Some(p.home.system.id.to_string().as_str());
            if !at_hub && !at_home { continue; }
            // Replan finite legs only after their final dock transfer. Old
            // saved repeating routes migrate on a reported home return, never
            // midflight. This prevents yesterday's one-unit import manifest
            // from becoming a permanent bottleneck for today's project.
            if run.is_some_and(|r| r.hold.is_none() && r.phase != RoutePhase::Complete
                && !(r.route.repeat && at_home && r.visits >= 2)) { continue; }
            if run.is_some_and(|r| matches!(r.phase, RoutePhase::Fuel | RoutePhase::Escort)) { continue; }
            // Price this proposed route in the mode sent with it, not the
            // fleet's previous received setting. Never mutate the observation.
            let mut economy = fleet.clone();
            economy.transit = Some(sim::ship::TransitMode::Economy);
            let fleet = &economy;
            let limit = p.safe_load(fleet, p.home.system.pos, p.hub);
            let exports = p.exports(wanted, limit);
            let mut imports = BTreeMap::new();
            let retained: u32 = fleet.cargo_manifest.iter().filter(|c| p.export_reserve(c.commodity, wanted).is_none()).map(|c| c.units).sum();
            let mut room = p.safe_load(fleet, p.hub, p.home.system.pos).saturating_sub(retained);
            let mut goods: Vec<_> = wanted.iter().collect();
            goods.sort_by_key(|(c, _)| (**c != C::Fuel, **c != C::Provisions, **c));
            for (&good, &target) in goods {
                let need = (target - p.stock(good) - p.inbound(good) as f64 - p.carried_home(good) as f64
                    - self.expected_supply(p, good) as f64).ceil().max(0.0) as u32;
                let unpromised = account.warehouse.get(&good).copied().unwrap_or(0).saturating_sub(p.pickup_coverage(good));
                let n = need.min(unpromised).min(room);
                if n > 0 { imports.insert(good, n); room -= n; }
            }
            // The starter has only the ordinary TWO-stop route permission.
            // Finish exports at the hub, then price a fresh homebound manifest
            // there. Do not wait on a warehouse forecast made a whole trip ago.
            if at_home { imports.clear(); }
            let escort = p.own().find(|g| g.guard_target == Some(fleet.id));
            let first_export = !p.exported && self.trade_cycles == 0
                && p.own().find(|g| g.kind.is_player_freighter()).is_some_and(|g| g.id == fleet.id);
            let profitable = p.route_margin(fleet, &exports, &imports, escort) > 0.0;
            if at_home && (!profitable && !first_export || exports.values().sum::<u32>() < 10) {
                if run.is_some() {
                    self.wait(key, p, fleet.pos);
                    return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, route: None });
                }
                if cargo_used(fleet) > 0 && p.home.system.storage_headroom() >= 1.0 {
                    self.wait(key, p, fleet.pos);
                    return Some(Command::SystemUnload { player_id: p.owner, fleet_id: fleet.id, system: p.home.system.id });
                }
                self.status = "holding freight: waiting for a viable load or better margin".into();
                continue;
            }
            // Full tanks include safety margin, arrival reserve, and cargo mass.
            // The onboard route repeats this check before EVERY departure.
            let dest = if at_hub { p.home.system.pos } else { p.hub };
            let units = if at_hub { imports.values().sum::<u32>() + retained } else { exports.values().sum() };
            if fleet.fuel.unwrap_or(0.0) < p.leg(fleet, fleet.pos, dest, units) + p.arrival_reserve(fleet)
                || escort.is_some_and(|g| g.fuel.unwrap_or(0.0) < p.leg(g, fleet.pos, dest, 0) + p.arrival_reserve(g)) { continue; }
            if first_export && escort.is_none() { continue; }
            if at_home && self.cash(p) < p.operating_reserve() { continue; }
            // Unload only export goods at the hub; imported build materials must
            // never be mistaken for sale cargo after a blocked-route revision.
            // Unload quantities share the hold limit too. Writing `capacity`
            // for EACH commodity made mixed manifests illegal at the server.
            let hub_unload = if at_home { exports.clone() } else {
                fleet.cargo_manifest.iter().filter(|c| p.export_reserve(c.commodity, wanted).is_some())
                    .map(|c| (c.commodity, c.units)).collect()
            };
            let mut home_unload = imports.clone();
            for c in &fleet.cargo_manifest {
                if at_home || p.export_reserve(c.commodity, wanted).is_none() {
                    *home_unload.entry(c.commodity).or_default() += c.units;
                }
            }
            let finish = RouteStop { port: FreightPort::System { id: p.home.system.id },
                load: BTreeMap::new(), unload: home_unload.clone(), sell: false };
            let home_stop = RouteStop { port: FreightPort::System { id: p.home.system.id },
                load: exports, unload: home_unload, sell: false };
            let hub_stop = RouteStop { port: FreightPort::Hub, load: imports, unload: hub_unload, sell: true };
            let stops = if at_hub { vec![hub_stop, finish] } else { vec![home_stop, hub_stop] };
            self.wait(key, p, fleet.pos);
            // Give a hub pickup's load/sale and its account report one round
            // trip before booking the same warehouse goods on another carrier.
            if at_hub { self.wait("market".into(), p, p.hub); }
            self.route_visits.insert(fleet.id, 0);
            self.status = "assigning an owned mixed-cargo route with escort and fuel reserve".into();
            return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, route: Some(FreightRoute {
                shopping_budget: None,
                name: format!("{} / Market", p.home.system.name), stops, fuel_reserve: p.arrival_reserve(fleet),
                transit: sim::ship::TransitMode::Economy,
                escort: escort.map(|g| g.id), repeat: false,
            }) });
        }
        None
    }

    pub(super) fn market(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>) -> Option<Command> {
        if !self.ready("market", p.now) { return None; }
        let (account, ticker) = (p.account.as_ref()?, p.ticker.as_ref()?);
        let home = p.home.system.id;
        // Imports can't displace surplus already occupying the warehouse. Clear
        // actual storage pressure before another buy/book cycle; otherwise a
        // well-funded Academy can starve beside full ore and food silos.
        let space = wanted.iter().map(|(&c, &n)| (n - p.home.system.free_stock(c)).max(0.0)).sum::<f64>()
            .max(50.0).min(p.home.system.storage_cap() * 0.3);
        if p.home.system.storage_headroom() < space {
            if let Some(cmd) = self.export_surplus(p, wanted, true) { return Some(cmd); }
        }
        // Sell arrived surplus already in the warehouse first: no rescue magic,
        // no assumed future revenue, and never liquidate earmarked imports.
        for (&good, &stored) in &account.warehouse {
            let promised = p.own().any(|g| matches!(&g.industry, Some(FleetIndustry::Route { run })
                if run.phase != RoutePhase::Complete && run.hold.is_none()
                    && run.route.stops.iter().skip(run.stop).any(|s| s.port == FreightPort::Hub && s.load.contains_key(&good))));
            let liquidation = self.recovering(p) && !matches!(good, C::Fuel | C::Provisions) && !promised;
            if !promised && (liquidation || p.export_reserve(good, wanted).is_some()) {
                // A future optional build is deferrable when bankrupt. Cargo
                // promised to a live route, food and fuel are not liquidation.
                let reserve = if liquidation { 0 } else {
                    p.sites.iter().map(|site| {
                        let local = p.at_site(site);
                        let needs = self.desired_stock(&local, self.site_project(p, site));
                        (needs.get(&good).copied().unwrap_or(0.0) - local.stock(good) - local.inbound(good) as f64
                            - local.carried_home(good) as f64).ceil().max(0.0) as u32
                    }).sum::<u32>().max((wanted.get(&good).copied().unwrap_or(0.0) - p.stock(good)).ceil().max(0.0) as u32)
                };
                let units = stored.saturating_sub(reserve).min(ticker.available_sell.get(&good).copied().unwrap_or(0));
                if units > 0 {
                    self.wait("market".into(), p, p.hub);
                    self.status = "selling arrived surplus to rebuild working cash".into();
                    return Some(Command::MarketSell { player_id: p.owner, commodity: good, units, min_unit_price: None });
                }
            }
        }
        // Critical fuel/food precede materials. Normal top-ups have hysteresis,
        // preventing every consumed fraction from starving projects/exploration.
        let mut goods: Vec<_> = wanted.iter().collect();
        goods.sort_by_key(|(c, _)| (**c != C::Fuel || !p.fuel_critical(), **c != C::Provisions || p.stock(C::Provisions) >= 10.0, **c));
        for (&good, &target) in goods {
            let essential = good == C::Fuel || good == C::Provisions;
            let in_hold = p.carried_home(good);
            let pipeline = p.stock(good) + in_hold as f64 + p.inbound(good) as f64 + self.expected_supply(p, good) as f64;
            let project_need = self.project(p).map(|goal| p.project_costs(goal).get(&good).copied().unwrap_or(0.0)).unwrap_or(0.0);
            // Replenish continuous consumers in batches, not a new paid ship
            // for each fractional unit. Keep the project's recipe untouched;
            // reorder when its operating/research buffer is half consumed.
            if target > project_need && pipeline >= project_need + (target - project_need) * 0.5 { continue; }
            let need = (target - pipeline).ceil().max(0.0) as u32;
            let stored = account.warehouse.get(&good).copied().unwrap_or(0);
            let booked = need.min(stored).saturating_sub(p.pickup_coverage(good)).min(100);
            let budget = self.spendable(p, essential);
            let fee = sim::tca::freight_fee(booked, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            if booked > 0 && budget >= fee {
                self.spend(p, fee);
                self.status = if essential { "booking essential replenishment" } else { "booking imports without an available owned carrier" }.into();
                return Some(Command::BookFreightOut { player_id: p.owner, system: home, commodity: good, units: booked });
            }
            let missing = need.saturating_sub(stored);
            let ceiling = p.price(good) * 1.2;
            // Reserve delivery fees now too; buying a warehouseful with no cash
            // left to bring it home was another form of the bankruptcy trap.
            let delivery = sim::tca::freight_fee(missing.min(100), p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            let units = missing.min(100).min(ticker.available_buy.get(&good).copied().unwrap_or(0))
                .min(((budget - delivery).max(0.0) / (ceiling * 1.1)).floor() as u32);
            if units > 0 && (essential || !self.recovering(p)) {
                self.spend(p, units as f64 * ceiling * 1.1);
                self.status = if essential { "buying operating supplies before optional projects" } else { "funding the next affordable project" }.into();
                return Some(Command::MarketBuy { player_id: p.owner, commodity: good, units, max_unit_price: Some(ceiling) });
            }
        }
        self.export_surplus(p, wanted, false)
    }

    fn export_surplus(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>, clear_space: bool) -> Option<Command> {
        // Authority is a genuine bulk/overflow alternative, not the first
        // consumer of every ore unit. Keep one load for a viable owned circuit.
        for (&good, &stock) in &p.home.system.stockpile {
            let Some(reserve) = p.export_reserve(good, wanted) else { continue; };
            let held = if clear_space { 0 } else { p.own().filter(|f|
                f.kind.is_player_freighter() && f.industry.is_some() && p.uses_local_port(f)).map(capacity).sum::<u32>() };
            let fee_per_unit = sim::tca::freight_fee(1, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            let affordable = ((self.cash(p) - p.bunker_commitment()).max(0.0) / fee_per_unit.max(0.01)).floor() as u32;
            let n = ((stock - reserve - held as f64).floor().max(0.0).min(300.0) as u32).min(affordable);
            let fee = sim::tca::freight_fee(n, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            if n >= (if self.recovering(p) { 1 } else { 50 }) && n as f64 * p.price(good) * 0.95 > fee
                && self.cash(p) - p.bunker_commitment() >= fee {
                self.spend(p, fee);
                self.status = "booking profitable bulk surplus for income".into();
                return Some(Command::BookFreightIn { player_id: p.owner, system: p.home.system.id, commodity: good, units: n, sell_on_arrival: true });
            }
        }
        None
    }
}
