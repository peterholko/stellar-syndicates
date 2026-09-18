use super::*;
use sim::industry::{FleetIndustry, FreightPort, FreightRoute, RoutePhase, RouteStop};

impl Observation {
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
        for (i, guard) in p.own().filter(|g| g.kind == H::Raider).enumerate() {
            let key = format!("fleet:{}", guard.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&guard.id) || guard.stalled { continue; }
            if guard.fuel.unwrap_or(0.0) < p.tank(guard) * 0.35 && guard.docked.is_none() {
                if let Some(cmd) = self.return_for_fuel(p, guard) { return Some(cmd); }
                continue;
            }
            if self.returns.contains_key(&guard.id) { continue; }
            if let Some(target) = freighters.get(i) {
                if guard.guard_target != Some(target.id) {
                    self.wait(key, p, guard.pos);
                    self.status = "assigning a persistent freight escort".into();
                    return Some(Command::GuardFleet { player_id: p.owner, interceptor_id: guard.id, target_id: target.id });
                }
            } else if guard.defend_system.is_none() {
                self.wait(key, p, guard.pos);
                return Some(Command::DefendSystem { player_id: p.owner, fleet_id: guard.id,
                    system_id: p.home.system.id, pursuit_radius: 20_000.0 });
            }
        }
        None
    }

    pub(super) fn routes(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>) -> Option<Command> {
        let account = p.account.as_ref()?;
        for fleet in p.own().filter(|g| g.kind.is_player_freighter()) {
            let key = format!("fleet:{}", fleet.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&fleet.id) || fleet.stalled { continue; }
            let run = match &fleet.industry { Some(FleetIndustry::Route { run }) => Some(run), _ => None };
            let at_hub = fleet.docked.as_deref() == Some("hub");
            let at_home = fleet.docked.as_deref() == Some(p.home.system.id.to_string().as_str());
            if !at_hub && !at_home { continue; }
            // A healthy persistent route needs no CC micro-orders. Only revise
            // a reported hold/complete state, never restart a moving transfer.
            if run.is_some_and(|r| r.hold.is_none() && r.phase != RoutePhase::Complete) { continue; }
            if run.is_some_and(|r| matches!(r.phase, RoutePhase::Fuel | RoutePhase::Escort)) { continue; }
            let limit = p.safe_load(fleet, p.home.system.pos, p.hub);
            let exports = p.exports(wanted, limit);
            let mut imports = BTreeMap::new();
            let retained: u32 = fleet.cargo_manifest.iter().filter(|c| p.export_reserve(c.commodity, wanted).is_none()).map(|c| c.units).sum();
            let mut room = p.safe_load(fleet, p.hub, p.home.system.pos).saturating_sub(retained);
            let mut goods: Vec<_> = wanted.iter().collect();
            goods.sort_by_key(|(c, _)| (**c != C::Fuel, **c != C::Provisions, **c));
            for (&good, &target) in goods {
                let carried: u32 = p.own().map(|f| cargo(f, good)).sum();
                let need = (target - p.home.system.free_stock(good) - p.inbound(good) as f64 - carried as f64).ceil().max(0.0) as u32;
                let n = need.min(account.warehouse.get(&good).copied().unwrap_or(0)).min(room);
                if n > 0 { imports.insert(good, n); room -= n; }
            }
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
            let mut hub_unload = exports.clone();
            for c in &fleet.cargo_manifest {
                if p.export_reserve(c.commodity, wanted).is_some() { hub_unload.insert(c.commodity, capacity(fleet)); }
            }
            let mut home_unload: BTreeMap<_, _> = imports.keys().map(|&c| (c, capacity(fleet))).collect();
            for c in &fleet.cargo_manifest { home_unload.insert(c.commodity, capacity(fleet)); }
            let home_stop = RouteStop { port: FreightPort::System { id: p.home.system.id },
                load: exports, unload: home_unload, sell: false };
            let hub_stop = RouteStop { port: FreightPort::Hub, load: imports, unload: hub_unload, sell: true };
            let stops = if at_hub { vec![hub_stop, home_stop] } else { vec![home_stop, hub_stop] };
            self.wait(key, p, fleet.pos);
            self.route_visits.insert(fleet.id, 0);
            self.status = "assigning an owned mixed-cargo route with escort and fuel reserve".into();
            return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, route: Some(FreightRoute {
                name: "Home / Market".into(), stops, fuel_reserve: p.arrival_reserve(fleet),
                escort: escort.map(|g| g.id), repeat: profitable,
            }) });
        }
        None
    }

    pub(super) fn market(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>) -> Option<Command> {
        if !self.ready("market", p.now) { return None; }
        let (account, ticker) = (p.account.as_ref()?, p.ticker.as_ref()?);
        let home = p.home.system.id;
        // Sell arrived surplus already in the warehouse first: no rescue magic,
        // no assumed future revenue, and never liquidate earmarked imports.
        for (&good, &stored) in &account.warehouse {
            if p.export_reserve(good, wanted).is_some() {
                let reserve = (wanted.get(&good).copied().unwrap_or(0.0) - p.stock(good)).ceil().max(0.0) as u32;
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
            let in_hold: u32 = p.own().filter(|g| g.docked.as_deref() != Some(home.to_string().as_str())).map(|g| cargo(g, good)).sum();
            let need = (target - p.stock(good) - in_hold as f64 - p.inbound(good) as f64).ceil().max(0.0) as u32;
            let stored = account.warehouse.get(&good).copied().unwrap_or(0);
            let booked = need.min(stored).min(100);
            let own_pickup = p.own().any(|g| g.kind.is_player_freighter() && match &g.industry {
                Some(FleetIndustry::Route { run }) => run.route.stops.iter().any(|s| s.port == FreightPort::Hub && s.load.contains_key(&good)),
                _ => g.docked.as_deref() == Some("hub") && !g.stalled,
            });
            let budget = self.spendable(p, essential);
            let fee = sim::tca::freight_fee(booked, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            if booked > 0 && !own_pickup && budget >= fee {
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
        // Authority is a genuine bulk/overflow alternative, not the first
        // consumer of every ore unit. Keep one load for a viable owned circuit.
        for (&good, &stock) in &p.home.system.stockpile {
            let Some(reserve) = p.export_reserve(good, wanted) else { continue; };
            let held = p.own().filter(|f| f.kind.is_player_freighter() && f.industry.is_some()).map(capacity).sum::<u32>();
            let fee_per_unit = sim::tca::freight_fee(1, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            let affordable = ((self.cash(p) - p.bunker_commitment()).max(0.0) / fee_per_unit.max(0.01)).floor() as u32;
            let n = ((stock - reserve - held as f64).floor().max(0.0).min(300.0) as u32).min(affordable);
            let fee = sim::tca::freight_fee(n, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            if n >= (if self.recovering(p) { 1 } else { 50 }) && n as f64 * p.price(good) * 0.95 > fee
                && self.cash(p) - p.bunker_commitment() >= fee {
                self.spend(p, fee);
                self.status = "booking profitable bulk surplus for income".into();
                return Some(Command::BookFreightIn { player_id: p.owner, system: home, commodity: good, units: n, sell_on_arrival: true });
            }
        }
        None
    }
}
