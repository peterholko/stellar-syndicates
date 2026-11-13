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
            let speed = f.kind.max_speed() * sim::transit::WARP_FACTOR * run.route.transit.fraction();
            let eta = (f.pos.distance(self.hub) + self.hub.distance(self.home.system.pos)) / speed
                + self.round_trip(f.pos) + 120.0;
            let drain = self.research.active.as_deref().and_then(sim::research::programme).map_or(0.0, |r|
                sim::research::basket(r.field, r.tier).into_iter().find(|(c, _)| *c == good).map_or(0.0, |(_, n)| n)
                    * self.home.academies.iter().map(|a| a.rate).sum::<f64>().max(1.0));
            let deadline = if FUELS.contains(&good) && self.fuel_critical() || good == C::Provisions && self.stock(good) < 10.0 {
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

    /// §fuel: freighter holds at the Hub or bound for it. Each can bring fuel
    /// home on its return leg.
    pub(super) fn hub_bound_room(&self) -> u32 {
        self.own().filter(|f| f.kind.is_player_freighter() && (f.docked.as_deref() == Some("hub")
            || matches!(&f.industry, Some(FleetIndustry::Route { run }) if run.phase != RoutePhase::Complete
                && run.route.stops.iter().skip(run.stop).any(|s| s.port == FreightPort::Hub))))
            .map(capacity).sum()
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
        // §fuel-overhaul: each hull's burn at its own fuel's price; an escort
        // flies its charge's Uranium regime when it can.
        let home = self.home.system.pos;
        // §cargo-mass: the goods at their own weight, ore out and parts back.
        let weigh = |goods: &BTreeMap<C, u32>| goods.iter().map(|(c, n)| *n as f64 * c.mass_per_unit()).sum::<f64>();
        let mut fuel = (self.leg_mass(fleet, home, self.hub, weigh(exports))
            + self.leg_mass(fleet, self.hub, home, weigh(imports))) * self.price_per_energy(fleet);
        if let Some(guard) = escort {
            let (_, mult) = self.escort_burn(guard, flies_slow(fleet));
            fuel += self.fuel_per_mass(home, self.hub) * hull_mass(guard) * mult * 2.0 * self.price_per_energy(guard);
        }
        // The return goods are needed regardless of carrier. Count the freight
        // fee saved, not their purchase price, as the backhaul's contribution.
        let saved = imports.iter().map(|(&c, &n)| sim::tca::freight_fee(n, self.price(c), home.distance(self.hub))).sum::<f64>();
        exports.iter().map(|(&c, &n)| n as f64 * self.price(c) * 0.95).sum::<f64>()
            + saved - fuel * 1.10
    }
}

impl Bot {
    /// §fuel: the fuel the home still lacks after every delivery under way.
    pub(super) fn fuel_lacking(&self, p: &Observation, wanted: &BTreeMap<C, f64>, fuel: C) -> u32 {
        (wanted.get(&fuel).copied().unwrap_or(0.0) - p.stock(fuel) - p.inbound(fuel) as f64
            - p.carried_home(fuel) as f64 - self.expected_supply(p, fuel) as f64).ceil().max(0.0) as u32
    }

    pub(super) fn guards(&mut self, p: &Observation) -> Option<Command> {
        let freighters: Vec<_> = p.own().filter(|g| g.kind.is_player_freighter()).collect();
        for guard in p.own().filter(|g| g.kind == H::Raider
            && warfare::composition(g).values().sum::<u32>() == 1) {
            let key = format!("fleet:{}", guard.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&guard.id) || guard.stalled
                || self.campaign_fleet(guard.id) || guard.damage.unwrap_or(0.0) > 0.2
                // §stash: a hull the den's blast hurt stays in the yard until whole.
                || p.den_wounded.contains(&guard.id) { continue; }
            if store(guard) < p.tank(guard) * 0.35 && guard.docked.is_none() {
                if let Some(cmd) = self.return_for_fuel(p, guard) { return Some(cmd); }
                continue;
            }
            if self.returns.contains_key(&guard.id) { continue; }
            // §charter (Peter, Oct 30 2026): step 9's Interceptor waits at
            // rest at home for its captain; one already on duty comes home.
            if p.founding_stage == sim::FoundingStage::BuildInterceptor && guard.captain.is_none() {
                if guard.guard_target.is_some() || guard.defend_system.is_some() {
                    self.wait(key, p, guard.pos);
                    return Some(Command::MoveShip { player_id: p.owner, ship_id: guard.id, dest: p.home.system.pos });
                }
                continue;
            }
            // §charter-v4: the den step takes the Interceptor off escort duty.
            if p.den.is_some() { continue; }
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
        // §stash: the charter's first freighter fetches the den's stash first —
        // through the whole stash chapter, so no Hub run takes it before the
        // den's report names the stash (§shipbuilding-overhaul, Nov 7 2026:
        // with fuel no longer critical here, an ore export used to grab it at
        // "Full speed" and the charter stalled).
        if p.stash.is_some() || (p.local_opening && matches!(p.founding_stage, sim::FoundingStage::EliminateDen
            | sim::FoundingStage::RepairInterceptor | sim::FoundingStage::BuildTinyFreighter
            | sim::FoundingStage::FullSpeed | sim::FoundingStage::HaulLoot))
        {
            return None;
        }
        let account = p.account.as_ref()?;
        for fleet in p.own().filter(|g| g.kind.is_player_freighter()) {
            if self.freight_homes.get(&fleet.id).is_some_and(|id| *id != p.home.system.id) { continue; }
            if self.belt_haulers.contains_key(&fleet.id) { continue; } // §belts: on the station run
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
            let mut planned = fleet.clone();
            planned.transit = Some(p.freight_transit());
            let fleet = &planned;
            let limit = p.safe_load(fleet, p.home.system.pos, p.hub);
            let mut exports = p.exports(wanted, limit);
            // §charter-v5: until the charter's first export is done, a Hub
            // run carries it first. §charter-v6 (Nov 7 2026): Alloys, the
            // Smelter's — what the colony can spare past its buffer, or a
            // token ten so the lesson sails even when the buffer is short.
            if at_home && p.local_opening && p.founding_stage.step() <= sim::FoundingStage::PrepareExport.step() {
                let free = p.home.system.free_stock(C::Alloys).floor().max(0.0);
                let spare = (free - wanted.get(&C::Alloys).copied().unwrap_or(0.0)).floor().max(0.0) as u32;
                let ore = spare.max(free.min(10.0) as u32).min(limit);
                if ore > 0 && !exports.contains_key(&C::Alloys) {
                    let mut room = limit - ore;
                    let mut others: Vec<_> = std::mem::take(&mut exports).into_iter().collect();
                    others.sort_by(|(a, _), (b, _)| p.price(*b).total_cmp(&p.price(*a)));
                    exports.insert(C::Alloys, ore);
                    for (good, n) in others {
                        let n = n.min(room);
                        if n > 0 { exports.insert(good, n); room -= n; }
                    }
                }
            }
            let mut imports = BTreeMap::new();
            let retained: u32 = fleet.cargo_manifest.iter().filter(|c| p.export_reserve(c.commodity, wanted).is_none()).map(|c| c.units).sum();
            let mut room = p.safe_load(fleet, p.hub, p.home.system.pos).saturating_sub(retained);
            // §hold-room: the hold counts room, each good its own (a capital
            // part ten). `room` is in whole room units, so quarters here.
            let mut quarters = room as u64 * sim::cargo::ROOM_SCALE as u64;
            let mut goods: Vec<_> = wanted.iter().collect();
            goods.sort_by_key(|(c, _)| (!FUELS.contains(*c), **c != C::Provisions, **c));
            for (&good, &target) in goods {
                let need = (target - p.stock(good) - p.inbound(good) as f64 - p.carried_home(good) as f64
                    - self.expected_supply(p, good) as f64).ceil().max(0.0) as u32;
                let unpromised = account.warehouse.get(&good).copied().unwrap_or(0).saturating_sub(p.pickup_coverage(good));
                let fits = (quarters / good.room_quarters() as u64).min(u32::MAX as u64) as u32;
                let n = need.min(unpromised).min(fits);
                if n > 0 { imports.insert(good, n); quarters -= n as u64 * good.room_quarters() as u64; }
            }
            room = (quarters / sim::cargo::ROOM_SCALE as u64) as u32;
            // The starter has only the ordinary TWO-stop route permission.
            // Finish exports at the hub, then price a fresh homebound manifest
            // there. Do not wait on a warehouse forecast made a whole trip ago.
            if at_home { imports.clear(); }
            // §fuel: a hull heading home fills its spare hold with the Fuel the
            // home still lacks, bought at the Hub on arrival: no market slot and
            // no Authority lead time. One run per light round trip, until its
            // cargo shows in the reports.
            let mut shopping = None;
            if at_hub && self.ready("fuel-run", p.now) {
                let mut budget = 0.0;
                for fuel in FUELS {
                    let n = self.fuel_lacking(p, wanted, fuel)
                        .saturating_sub(imports.get(&fuel).copied().unwrap_or(0)).min(room);
                    let cost = n as f64 * p.price(fuel) * 1.3;
                    if n > 0 && self.spendable(p, true) >= budget + cost {
                        *imports.entry(fuel).or_default() += n;
                        room -= n;
                        budget += cost;
                    }
                }
                if budget > 0.0 { shopping = Some(budget); }
            }
            let escort = p.own().find(|g| g.guard_target == Some(fleet.id));
            // §fuel: with nothing worth selling, a home short of Fuel still
            // sends a hull, unless the holds already bound for the Hub cover it.
            let fuel_run = at_home && self.ready("fuel-run", p.now) && {
                let lacking: u32 = FUELS.into_iter().map(|fuel| self.fuel_lacking(p, wanted, fuel)).sum();
                lacking >= 10 && lacking > p.hub_bound_room()
            };
            let profitable = p.route_margin(fleet, &exports, &imports, escort) > 0.0;
            if at_home && !fuel_run && (!profitable || exports.values().sum::<u32>() < 10) {
                if run.is_some() {
                    self.wait(key, p, fleet.pos);
                    return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, revise: false, route: None });
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
            if store(fleet) < p.leg(fleet, fleet.pos, dest, units) + p.arrival_reserve(fleet)
                || escort.is_some_and(|g| {
                    let (fuel, mult) = p.escort_burn(g, flies_slow(fleet));
                    fuel < p.fuel_per_mass(fleet.pos, dest) * hull_mass(g) * mult + p.arrival_reserve(g)
                }) { continue; }
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
            if let Some(budget) = shopping { self.hold_for_route(p, budget); }
            if shopping.is_some() || fuel_run { self.wait("fuel-run".into(), p, p.hub); }
            self.route_visits.insert(fleet.id, 0);
            self.status = if shopping.is_some() { "bringing fuel home on the return leg" }
                else if fuel_run { "sending a freighter to the Hub for fuel" }
                else { "assigning an owned mixed-cargo route with escort and fuel reserve" }.into();
            return Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, revise: false, route: Some(FreightRoute {
                shopping_budget: shopping,
                name: format!("{} / Market", p.home.system.name), stops, fuel_reserve: p.arrival_reserve(fleet),
                transit: p.freight_transit(),
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
            let liquidation = self.recovering(p) && !matches!(good, C::Deuterium | C::Uranium | C::Provisions) && !promised;
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
        // Critical fuel/food precede materials, then whatever the current
        // project is still short of: each buy holds the market for a light
        // round trip, so buffer top-ups must not crowd out the project.
        // Normal top-ups have hysteresis, preventing every consumed fraction
        // from starving projects/exploration.
        let project_costs = self.project(p).map(|goal| p.project_costs(goal)).unwrap_or_default();
        let project_short = |c: C| project_costs.get(&c).is_some_and(|n|
            p.stock(c) + p.carried_home(c) as f64 + p.inbound(c) as f64 + 1e-9 < *n);
        let mut goods: Vec<_> = wanted.iter().collect();
        goods.sort_by_key(|(c, _)| (!FUELS.contains(*c) || !p.fuel_critical(), **c != C::Provisions || p.stock(C::Provisions) >= 10.0,
            !project_short(**c), **c));
        // §hull-ladder (Nov 7 2026): a kit is several Hub goods at once (the
        // Small Freighter's plating, frame, precision parts, Nav Computer and
        // bays). One order a light round trip booked them one at a time —
        // fifty minutes for one hull in the charter harness — so while the
        // project is still short of OTHER goods, an order holds its cash
        // without holding the market slot; only the last one waits.
        let project_shorts = project_costs.keys().filter(|c| project_short(**c)).count();
        // §habitat-first (Nov 8 2026): during the charter the home's own mine
        // feeds its Smelter. A Hub buy of that ore (95 Ferrite Ore, ~930 Cr,
        // right after the trader's sale) left the expansion without cash for
        // its fuel; the Smelter's buffer still keeps the ore from export.
        let in_charter = p.local_opening && p.founding_stage != sim::FoundingStage::Complete;
        let mined_at_home = |c: C| p.home.system.bodies.iter()
            .any(|b| b.tier(K::MiningComplex) > 0 && b.deposits.iter().any(|d| d.resource == c));
        for (&good, &target) in goods {
            if in_charter && !project_short(good) && mined_at_home(good) { continue; }
            let essential = FUELS.contains(&good) || good == C::Provisions;
            // §charter-v5: the docked trader covers the project at once; a
            // Hub import of the same goods only waits on the freight.
            if !essential && project_short(good) && p.trader_stocks(good) { continue; }
            let in_hold = p.carried_home(good);
            let pipeline = p.stock(good) + in_hold as f64 + p.inbound(good) as f64 + self.expected_supply(p, good) as f64;
            let project_need = project_costs.get(&good).copied().unwrap_or(0.0);
            // Replenish continuous consumers in batches, not a new paid ship
            // for each fractional unit. Keep the project's recipe untouched;
            // reorder when its operating/research buffer is half consumed.
            // §fuel: fuel reorders at three quarters, well clear of the critical half.
            let refill = if FUELS.contains(&good) { 0.75 } else { 0.5 };
            if target > project_need && pipeline >= project_need + (target - project_need) * refill { continue; }
            let need = (target - pipeline).ceil().max(0.0) as u32;
            let stored = account.warehouse.get(&good).copied().unwrap_or(0);
            let booked = need.min(stored).saturating_sub(p.pickup_coverage(good)).min(100);
            let budget = self.spendable(p, essential);
            let fee = sim::tca::freight_fee(booked, p.price(good), p.home.system.pos.distance(p.hub)) * 1.2;
            let more_for_project = project_short(good) && project_shorts > 1;
            if booked > 0 && budget >= fee {
                if more_for_project { self.spend_at_home(p, fee); } else { self.spend(p, fee); }
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
                if more_for_project { self.spend_at_home(p, units as f64 * ceiling * 1.1); } else { self.spend(p, units as f64 * ceiling * 1.1); }
                self.status = if essential { "buying operating supplies before optional projects" } else { "funding the next affordable project" }.into();
                return Some(Command::MarketBuy { player_id: p.owner, commodity: good, units, max_unit_price: Some(ceiling) });
            }
        }
        self.export_surplus(p, wanted, false)
    }

    /// §merchant: a docked merchant covers the current project's shortfall
    /// rather than waiting a Hub round trip. It is a last resort at its
    /// markup: the project's own goods only, never buffers, from spare cash.
    pub(super) fn merchant(&mut self, p: &Observation, project: Option<Project>, wanted: &BTreeMap<C, f64>) -> Option<Command> {
        let visit = p.merchant.as_ref().filter(|v| v.phase == sim::merchant::MerchantPhase::Docked
            && v.system == p.home.system.id)?;
        if !self.ready("merchant", p.now) { return None; }
        let buys: BTreeMap<C, u32> = p.project_costs(project?).into_iter().filter_map(|(c, n)| {
            let short = (n - p.stock(c) - p.inbound(c) as f64 - p.carried_home(c) as f64).ceil().max(0.0) as u32;
            let units = short.min(visit.stock.get(&c).copied().unwrap_or(0));
            (units > 0 && visit.sell_price(c).is_some()).then_some((c, units))
        }).collect();
        if buys.is_empty() { return None; }
        let spendable = self.spendable(p, false);
        // §charter-v5: short of cash, sell the trader just enough of what it
        // buys (§sell-alloys: Alloys) to cover the gap (what the home wants
        // stays); short of those too, buy what the cash covers, selling what
        // there is toward the rest.
        let mut buys = buys;
        let mut sells = BTreeMap::new();
        let cost = visit.quote(&buys, &BTreeMap::new()).ok()?.cost;
        let good = sim::merchant::BUYS;
        if cost > spendable && let Some(price) = visit.buy_price(good).filter(|p| *p > 0.0) {
            let keep = wanted.get(&good).copied().unwrap_or(0.0);
            let spare = ((p.home.system.free_stock(good) - keep).floor().max(0.0) as u32).min(visit.room());
            let need = ((cost - spendable) / price).ceil() as u32;
            let sold = need.min(spare);
            if sold > 0 { sells.insert(good, sold); }
            let mut budget = spendable + sold as f64 * price;
            if need > spare {
                for (c, n) in buys.iter_mut() {
                    let unit = visit.sell_price(*c).unwrap_or(f64::INFINITY);
                    *n = (*n).min((budget / unit).floor().max(0.0) as u32);
                    budget -= *n as f64 * unit;
                }
                buys.retain(|_, n| *n > 0);
                if buys.is_empty() && sells.is_empty() { return None; }
            }
        }
        let quote = visit.quote(&buys, &sells).ok()?;
        let net = quote.cost - quote.revenue;
        if net > spendable || quote.net_units as f64 > p.home.system.storage_headroom() {
            return None;
        }
        self.spend_at_home(p, net.max(0.0));
        self.wait("merchant".into(), p, p.home.system.pos);
        self.status = if sells.is_empty() { "buying the project's shortfall from the merchant" }
            else if buys.is_empty() { "selling Alloys to the merchant toward the project's shortfall" }
            else { "selling Alloys to the merchant for the project's shortfall" }.into();
        Some(Command::MerchantTrade { player_id: p.owner, buys, sells })
    }

    pub(super) fn export_surplus(&mut self, p: &Observation, wanted: &BTreeMap<C, f64>, clear_space: bool) -> Option<Command> {
        // Authority is a genuine bulk/overflow alternative, not the first
        // consumer of every ore unit.
        // §charter-v5: until the trader has made its first purchase, what it
        // buys (§sell-alloys: Alloys) waits for it.
        let held_for_trader = p.local_opening
            && p.founding_stage.step() <= sim::FoundingStage::SellOre.step();
        // §fuel: clearing room, the biggest pile goes first. Fifty-unit ore lots
        // never caught up with a full Agroplex, and they took the market slots
        // the fuel needed.
        let mut piles: Vec<(C, f64)> = p.home.system.stockpile.iter().map(|(&c, &n)| (c, n)).collect();
        if clear_space { piles.sort_by(|a, b| b.1.total_cmp(&a.1)); }
        for (good, stock) in piles {
            // §charter-v4: until the Agroplex runs, the kit's Provisions are
            // the colony's only food; never ship them out.
            if good == C::Provisions && p.local_opening
                && p.founding_stage.step() <= sim::FoundingStage::BuildAgroplex.step() { continue; }
            // Full storage still wins: imports bounce off a full colony.
            if held_for_trader && good == sim::merchant::BUYS && !clear_space { continue; }
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
