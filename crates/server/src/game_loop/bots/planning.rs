use super::*;
use sim::industry::FleetIndustry;

impl Observation {
    /// Conservative ordinary-carrier delivery window, including booking and
    /// returned light. This is a planning estimate, not an arrival assertion.
    pub(super) fn supply_lead(&self) -> f64 {
        sim::tca::TCA_DEPARTURE_PERIOD * 2.0
            + self.home.system.pos.distance(self.hub) / (H::Freighter.max_speed() * sim::transit::WARP_FACTOR)
            + self.round_trip(self.hub) * 2.0 + 120.0
    }

    pub(super) fn price(&self, good: C) -> f64 {
        self.ticker.as_ref().and_then(|m| m.prices.get(&good)).copied().unwrap_or(20.0)
    }

    pub(super) fn tank(&self, fleet: &GhostView) -> f64 {
        fleet.fuel_capacity.unwrap_or_else(|| hull_mass(fleet) * sim::fuel::FUEL_PER_HULL_MASS)
    }

    pub(super) fn arrival_reserve(&self, fleet: &GhostView) -> f64 { self.tank(fleet) * 0.05 }

    pub(super) fn leg(&self, fleet: &GhostView, from: Vec2, to: Vec2, units: u32) -> f64 {
        self.fuel_per_mass(from, to) * (hull_mass(fleet) + units as f64 * sim::ship::CARGO_MASS_PER_UNIT)
            * cruise_fuel_mult(fleet)
    }

    pub(super) fn safe_load(&self, fleet: &GhostView, from: Vec2, to: Vec2) -> u32 {
        let mut full = fleet.clone();
        full.pos = from;
        full.fuel = Some((self.tank(fleet) - self.arrival_reserve(fleet)).max(0.0));
        self.freight_limit(&full, to)
    }

    pub(super) fn uses_local_port(&self, fleet: &GhostView) -> bool {
        let id = self.home.system.id;
        if fleet.docked.as_deref() == Some(id.to_string().as_str())
            || fleet.defend_system.as_ref().is_some_and(|d| d.system == id) { return true; }
        let carrier = fleet.guard_target.and_then(|ward| self.own().find(|f| f.id == ward)).unwrap_or(fleet);
        if let Some(FleetIndustry::Route { run }) = &carrier.industry {
            if run.phase != sim::industry::RoutePhase::Complete {
                return run.route.stops.iter().any(|s| s.port == sim::industry::FreightPort::System { id });
            }
        }
        // Unassigned expeditions use their nearest reported owned berth. Every
        // new colony must not import a full tank for the corporation's ENTIRE
        // navy; an active route/defense order is the stronger depot assignment.
        self.sites.iter().min_by(|a, b| carrier.pos.distance(a.system.pos)
            .total_cmp(&carrier.pos.distance(b.system.pos)).then(a.system.id.cmp(&b.system.id)))
            .is_none_or(|s| s.system.id == id)
    }

    pub(super) fn fuel_target(&self) -> f64 {
        // A depot target covers the next dispatch/refill, not an arbitrary stock
        // floor. Fuel already in tanks is usable runway; an empty silo alone is
        // not a crisis. Hub bunkering is paid separately from imported fuel cargo.
        let home = self.home.system.pos;
        let mut target = 0.0;
        for fleet in self.own().filter(|f| self.uses_local_port(f)) {
            let cycle = if fleet.kind.is_player_freighter() {
                self.leg(fleet, home, self.hub, self.safe_load(fleet, home, self.hub))
            } else { self.tank(fleet) };
            let deficit = (self.tank(fleet) - fleet.fuel.unwrap_or(0.0)).max(0.0);
            target += cycle + if fleet.docked.as_deref() == Some(self.home.system.id.to_string().as_str()) { deficit } else { 0.0 };
        }
        // Refining is a standing fuel customer too. Size its buffer to the
        // replenishment lead time, with two coverage windows so the low-water
        // reorder point still covers a full buy/book/delivery cycle. Otherwise
        // unlocking a Smelter silently consumes the fleet's entire reserve.
        let system = &self.home.system;
        let processing = sim::research::mod_of(&self.research, sim::research::ModKey::ProcessingYield);
        let industrial = system.bodies.iter().flat_map(|b| b.assignments.iter().map(move |(&kind, a)| {
            let tier = b.tier(kind);
            let Some(recipe) = sim::production::assigned_converter(kind, Some(a)) else { return 0.0; };
            if tier == 0 { return 0.0; }
            let fuel = recipe.inputs.iter().find(|(c, _)| *c == C::Fuel).map_or(0.0, |(_, n)| *n);
            fuel * recipe.rate * sim::production::tier_throughput(tier)
                * a.workers.min(tier) as f64 / tier as f64 * system.staffing_share() * processing
        })).sum::<f64>();
        let lead = self.supply_lead();
        (target + industrial * lead * 2.0).min(system.storage_cap() * 0.6).max(20.0)
    }

    pub(super) fn fuel_critical(&self) -> bool {
        self.stock(C::Fuel) + (self.inbound(C::Fuel) as f64) < self.fuel_target() * 0.5
    }

    pub(super) fn operating_reserve(&self) -> f64 {
        // Protect one full working circuit, escort included, plus its normal
        // cargo fuel delivery fee. Never count a speculative future sale as cash.
        let home = self.home.system.pos;
        let freight = self.own().filter(|f| f.kind.is_player_freighter())
            .map(|f| self.leg(f, home, self.hub, self.safe_load(f, home, self.hub)) * 2.0)
            .min_by(f64::total_cmp).unwrap_or(0.0);
        let escort = self.own().filter(|f| f.kind == H::Raider)
            .map(|f| self.leg(f, home, self.hub, 0) * 2.0).next().unwrap_or(0.0);
        let fuel = freight + escort;
        fuel * self.price(C::Fuel) * 1.25
            + sim::tca::freight_fee(fuel.ceil() as u32, self.price(C::Fuel), home.distance(self.hub))
            + self.price(C::Provisions) * 10.0
    }

    pub(super) fn bunker_commitment(&self) -> f64 {
        // Automatic hub fills can spend money without a MarketBuy command. Hold
        // their conservative cost until returned account/fleet light catches up.
        self.own().filter(|f| (f.kind.is_player_freighter() || f.guard_target.is_some())
            && (f.docked.as_deref() == Some("hub") || f.docked.is_none() && f.vel.length_sq() > 1.0)
            && !matches!(&f.industry, Some(FleetIndustry::Route { run })
                if !run.route.stops.iter().any(|s| s.port == sim::industry::FreightPort::Hub)))
            .map(|f| {
                let burn = self.leg(f, f.pos, self.hub, cargo_used(f));
                (self.tank(f) - f.fuel.unwrap_or(0.0) + burn).clamp(0.0, self.tank(f))
            }).sum::<f64>() * self.price(C::Fuel) * 1.25
    }

    pub(super) fn threatened(&self, at: Vec2) -> bool {
        self.ghosts.iter().any(|g| !g.own && !g.ally && !g.owner.is_tca()
            && g.kind.is_combatant() && g.pos.distance(at) < 25_000.0)
    }

    pub(super) fn refuel_port(&self, fleet: &GhostView, hub_affordable: bool) -> Option<Vec2> {
        self.sites.iter().filter(|r| r.system.free_stock(C::Fuel) > 0.5)
            .map(|r| r.system.pos).chain(hub_affordable.then_some(self.hub))
            .filter(|p| self.enough_fuel(fleet, *p))
            .min_by(|a, b| fleet.pos.distance(*a).total_cmp(&fleet.pos.distance(*b)))
    }
}

impl Bot {
    pub(super) fn refresh(&mut self, p: &Observation) {
        self.waits.retain(|_, t| *t > p.now);
        self.avoided.retain(|_, t| *t > p.now);
        // `market_at` is the emission cutoff of the actual account lookup. This
        // is not an estimate timeout: that picture contains the action's result
        // (or rejection) before we release its reservation. Saves retain it.
        if p.account.is_some() { self.commitments.retain(|c| p.market_at < c.receiver_at); }
        self.surveys.retain(|fleet, _| p.own().any(|g| g.id == *fleet));
        self.reconnaissance.retain(|fleet, _| p.own().any(|g| g.id == *fleet));
        self.survey_started.retain(|id| self.surveys.contains_key(id));
        self.returns.retain(|id, trip| p.own().any(|g| g.id == *id
            && !(p.now - g.age >= trip.issued_at && g.docked.is_some()
                && g.pos.distance(trip.destination) < sim::transit::HYPERLIMIT)));
        for fleet in p.own() {
            // Replacing a completed route resets its visit counter locally, but
            // the old completion remains visible until the new order's report.
            // Do not count that same homecoming again while it is in flight.
            if !self.ready(&format!("fleet:{}", fleet.id), p.now) { continue; }
            if let Some(FleetIndustry::Route { run }) = &fleet.industry {
                let returned = !run.route.repeat && run.phase == sim::industry::RoutePhase::Complete
                    && fleet.docked.as_deref() == Some(p.home.system.id.to_string().as_str());
                let visits = run.visits + u32::from(returned);
                let previous = self.route_visits.entry(fleet.id).or_default();
                self.trade_cycles += (visits / 2).saturating_sub(*previous / 2) as u64;
                *previous = visits;
            }
        }
        self.status = if self.recovering(p) { "recovery: protecting the next earning trip" }
            else { "waiting for arrived work or supplies" }.into();
    }

    pub(super) fn cash(&self, p: &Observation) -> f64 {
        (p.account.as_ref().map_or(0.0, |a| a.credits)
            - self.commitments.iter().map(|c| c.amount).sum::<f64>()).max(0.0)
    }

    pub(super) fn spendable(&self, p: &Observation, essential: bool) -> f64 {
        (self.cash(p) - if essential { p.bunker_commitment() + 100.0 } else {
            p.bunker_commitment().max(p.operating_reserve())
        }).max(0.0)
    }

    pub(super) fn recovering(&self, p: &Observation) -> bool { self.cash(p) < p.operating_reserve() * 0.5 }

    pub(super) fn spend(&mut self, p: &Observation, amount: f64) {
        self.commitments.push(Commitment { amount, receiver_at:
            p.now + sim::transit::delay(p.cc, p.hub, p.c) + DT * 2.0 });
        self.wait("market".into(), p, p.hub);
    }

    pub(super) fn return_for_fuel(&mut self, p: &Observation, fleet: &GhostView) -> Option<Command> {
        let key = format!("fleet:{}", fleet.id);
        if !self.ready(&key, p.now) || self.returns.contains_key(&fleet.id) { return None; }
        let affordable = self.cash(p) >= p.tank(fleet) * p.price(C::Fuel) * 1.25;
        let scout_port = (fleet.kind == H::Scout).then(|| p.sites.iter()
            .map(|s| s.system.pos).chain(affordable.then_some(p.hub))
            .filter(|pos| fleet.fuel.unwrap_or(0.0) >= p.scout_leg(fleet, fleet.pos, *pos))
            .min_by(|a, b| fleet.pos.distance(*a).total_cmp(&fleet.pos.distance(*b)))).flatten();
        let dest = scout_port.or_else(|| p.refuel_port(fleet, affordable)).or_else(|| {
            // An empty depot is still a safe berth. Do not leave a threatened
            // Scout investigating in the open just because its home needs fuel.
            p.sites.iter().map(|s| s.system.pos).filter(|pos| p.enough_fuel(fleet, *pos))
                .min_by(|a, b| fleet.pos.distance(*a).total_cmp(&fleet.pos.distance(*b)))
        })?;
        if fleet.docked.is_some() && fleet.pos.distance(dest) < sim::transit::HYPERLIMIT { return None; }
        self.wait(key, p, fleet.pos);
        self.returns.insert(fleet.id, ReturnTrip { destination: dest, issued_at: p.now });
        if fleet.kind == H::Scout {
            if fleet.vel.length_sq() > 1.0 {
                let step = if fleet.pos.distance(dest) <= 4_000.0 { dest }
                    else { fleet.pos + (dest - fleet.pos).normalized() * 2_000.0 };
                return Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: step });
            }
            return self.scout_step(p, fleet, dest);
        }
        self.status = "returning a low-fuel fleet to a reachable berth".into();
        Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest })
    }

    pub(super) fn rescue(&mut self, p: &Observation) -> Option<Command> {
        for fleet in p.own().filter(|f| f.stalled && !f.rescue_inbound && f.docked.is_none()) {
            if !self.ready("market", p.now) || !self.ready(&format!("fleet:{}", fleet.id), p.now) { continue; }
            let quote = sim::tca::TCA_RESCUE_SERVICE_FEE + p.tank(fleet)
                * p.price(C::Fuel) * sim::tca::TCA_RESCUE_FUEL_PRICE_MULT * 1.25;
            if self.cash(p) >= quote + p.operating_reserve() * 0.5 {
                self.spend(p, quote);
                self.wait(format!("fleet:{}", fleet.id), p, fleet.pos);
                self.returns.remove(&fleet.id);
                self.status = "booking an affordable paid fuel rescue".into();
                return Some(Command::RequestFuelRescue { player_id: p.owner, fleet_id: fleet.id });
            }
            self.status = "recovery: stranded fleet; rescue is not yet affordable".into();
        }
        None
    }

    pub(super) fn staff(&mut self, p: &Observation) -> Option<Command> {
        for site in &p.sites {
            let key = format!("staff:{}", site.system.id);
            if !self.ready("staff", p.now) || !self.ready(&key, p.now) { continue; }
            let system = &site.system;
            let workforce = sim::colony::workforce_units(system.population());
            let mut jobs = Vec::new();
            for body in &site.system.bodies {
                for (&kind, &tier) in &body.structures {
                    if tier == 0 { continue; }
                    let priority = match kind {
                        K::Bioharvester | K::Agroplex => 0,
                        K::MiningComplex => 1,
                        K::Shipyard | K::NavalDrydock | K::CapitalSlipway | K::Garrison
                            if site.builds.iter().any(|job| job.body_id == body.id && matches!(job.what, BuildKind::Ship { ship } if sim::build::yard_for(ship).0 == kind)) => 2,
                        K::OrdnanceFoundry if p.own().any(|f| f.damage.unwrap_or(0.0) > 0.05) => 2,
                        K::Academy => 3,
                        K::VolatileHarvester | K::FuelRefinery => 4,
                        // Don't burn the expedition's fuel in an optional line.
                        K::Smelter if system.free_stock(C::Fuel) > if body.assignments.get(&kind).is_some_and(|a| a.workers > 0) { 10.0 } else { 40.0 } => 5,
                        K::Shipyard | K::NavalDrydock | K::CapitalSlipway | K::OrdnanceFoundry | K::Smelter => 9,
                        k if sim::production::converter_for(k).is_some() => 5,
                        _ => continue,
                    };
                    let specialists = body.assignments.get(&kind).map_or(0, |a| a.specialists.values().sum::<u32>());
                    jobs.push((priority, body.id, kind, tier.saturating_sub(specialists)));
                }
            }
            jobs.sort();
            let needed = jobs.iter().filter(|(priority, _, _, _)| *priority < 9).map(|(_, _, _, tier)| tier).sum::<u32>();
            // Managed migration needs overposted jobs. A budgeted workforce no
            // longer supplies that signal, so explicitly recruit up to the jobs
            // we intend to open, then return to Managed. Migrants still travel.
            let inbound = system.bodies.iter().map(|b| b.inbound_migrants).sum::<u32>() / 1_000;
            let policy = if workforce + inbound < needed && system.food_state == sim::colony::FoodState::WellSupplied {
                sim::MigrationPolicy::Open
            } else { sim::MigrationPolicy::Managed };
            if let Some(body) = system.bodies.iter().find(|b| b.population > 0.0 && b.migration_policy != policy) {
                self.wait(key, p, system.pos);
                return Some(Command::SetMigrationPolicy { player_id: p.owner, system_id: system.id, body_id: body.id, policy });
            }
            // The founding food chain and mine may briefly share the first two
            // cohorts. Beyond that, don't dilute the Academy with every optional
            // factory: allocate scarce cohorts by priority, releasing jobs first.
            let core = jobs.iter().filter(|(priority, _, _, _)| *priority <= 1).count() as u32;
            let budget = workforce.saturating_sub(system.industry.workforce()).max(core);
            // One cohort gets each essential line working, then spare cohorts
            // fill its researched tiers. Tier IV no longer operates forever at
            // one-quarter staffing just because the opening used one worker.
            let mut allocated = BTreeMap::new();
            let mut remaining = budget;
            for cohort in 0..jobs.iter().map(|j| j.3).max().unwrap_or(0) {
                for &(priority, body, kind, tier) in &jobs {
                    if remaining > 0 && priority < 9 && cohort < tier {
                        *allocated.entry((body, kind)).or_insert(0) += 1;
                        remaining -= 1;
                    }
                }
            }
            let mut changes = Vec::new();
            for &(_, body_id, kind, _) in &jobs {
                let wanted = allocated.get(&(body_id, kind)).copied().unwrap_or(0);
                let body = system.bodies.iter().find(|b| b.id == body_id).unwrap();
                let current = body.assignments.get(&kind).map_or(0, |a| a.workers);
                if current != wanted { changes.push((wanted > current, body_id, kind, wanted)); }
            }
            changes.sort();
            if let Some((_, body_id, kind, wanted)) = changes.first().copied() {
                self.wait(key, p, site.system.pos);
                self.status = "budgeting workforce for food, income, construction and science".into();
                let assignment = site.system.bodies.iter().find(|b| b.id == body_id)
                    .and_then(|b| b.assignments.get(&kind));
                return Some(Command::SetAssignment { player_id: p.owner, system_id: site.system.id,
                    structure: kind, workers: wanted, body_id: Some(body_id),
                    specialists: assignment.map(|a| a.specialists.clone()).unwrap_or_default(),
                    refining_ore: assignment.and_then(|a| a.refining_ore) });
            }
            // Post resident specialists to staffed lines of their profession.
            // Their ordinary training/transport remains physical; none are
            // conjured to open an activity-gated research school.
            for (&specialist, &resident) in &system.specialists {
                let posted: u32 = system.bodies.iter().flat_map(|b| b.assignments.values())
                    .map(|a| a.specialists.get(&specialist).copied().unwrap_or(0)).sum();
                if posted >= resident { continue; }
                for body in &system.bodies {
                    if let Some((&kind, a)) = body.assignments.iter().find(|(k, a)| specialist.affine(**k)
                        && a.workers > 0 && a.specialists.values().sum::<u32>() < body.tier(**k)) {
                        let mut specialists = a.specialists.clone();
                        *specialists.entry(specialist).or_default() += 1;
                        self.wait(key, p, system.pos);
                        return Some(Command::SetAssignment { player_id: p.owner, system_id: system.id,
                            structure: kind, body_id: Some(body.id), workers: a.workers.saturating_sub(1),
                            specialists, refining_ore: a.refining_ore });
                    }
                }
            }
        }
        None
    }
}
