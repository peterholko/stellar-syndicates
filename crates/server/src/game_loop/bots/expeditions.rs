use super::*;

impl Bot {
    fn reachable_prospect(p: &Observation, s: &Prospect) -> bool {
        let tank = H::Colony.hull_mass() * sim::fuel::FUEL_PER_HULL_MASS
            * sim::research::mod_of(&p.research, sim::research::ModKey::FuelCapacity);
        p.fuel_per_mass(p.home.system.pos, s.pos) * H::Colony.hull_mass() < tank * 0.95
    }
    pub(super) fn scouts(&mut self, p: &Observation) -> Option<Command> {
        for scout in p.own().filter(|g| g.kind == H::Scout && !g.stalled) {
            let key = format!("fleet:{}", scout.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&scout.id) { continue; }
            if p.threatened(scout.pos) || scout.damage.unwrap_or(0.0) > 0.35 {
                if let Some(target) = self.surveys.remove(&scout.id) {
                    self.avoided.insert(target, p.now + 600.0);
                }
                if scout.pos.distance(p.home.system.pos) > sim::transit::HYPERLIMIT {
                    if let Some(cmd) = self.return_for_fuel(p, scout) { return Some(cmd); }
                }
                continue;
            }
            if self.returns.contains_key(&scout.id) { continue; }
            if let Some(&target) = self.surveys.get(&scout.id) {
                if !p.surveyed.contains(&target) {
                    // An idle old sighting is not a failed survey. Wait for the
                    // round trip, travel, investigation and its report. A real
                    // timeout backs off instead of reissuing the same mission.
                    if !self.ready(&format!("survey:{}", scout.id), p.now) { continue; }
                    self.avoided.insert(target, p.now + 600.0);
                }
                self.surveys.remove(&scout.id);
            } else if scout.survey_progress.is_some() || scout.vel.length_sq() > 1.0 { continue; }
            let target = p.chart.iter().filter(|(id, pos)| !p.surveyed.contains(id)
                && !self.avoided.contains_key(id) && !self.surveys.values().any(|other| other == id)
                && !p.threatened(*pos)
                && scout.fuel.unwrap_or(0.0) >= p.leg(scout, scout.pos, *pos, 0)
                    + p.leg(scout, *pos, p.home.system.pos, 0) + p.arrival_reserve(scout))
                .min_by(|(_, a), (_, b)| scout.pos.distance(*a).total_cmp(&scout.pos.distance(*b)));
            if let Some(&(system_id, pos)) = target {
                // A partial berth fill is allowed only if the whole excursion
                // fits; the return leg/reserve above is never traded for reach.
                self.wait(key, p, scout.pos);
                self.waits.insert(format!("survey:{}", scout.id), p.now + p.round_trip(pos)
                    + scout.pos.distance(pos) / H::Scout.max_speed() + 600.0);
                self.surveys.insert(scout.id, system_id);
                self.status = "surveying an unvisited star with fuel to return".into();
                return Some(Command::SurveySystem { player_id: p.owner, fleet_id: scout.id, system_id });
            }
            if scout.docked.is_none() {
                if let Some(cmd) = self.return_for_fuel(p, scout) { return Some(cmd); }
            }
            self.status = "scout waiting for refuel or a safe reachable prospect".into();
        }
        None
    }

    pub(super) fn expansion_ready(&self, p: &Observation) -> bool {
        // Expansion is a later investment, not a deadline in the opening. A
        // profitable home, learned technology, support hull and a funded supply
        // kit all precede a colony hull. No hidden geology or ownership is read.
        p.sites.len() == 1 && p.exported && p.research.completed.len() >= 2
            && p.home.system.tier(K::Academy) > 0 && p.home.system.tier(K::MiningComplex) > 0
            && p.count(H::Scout) > 0 && p.own().any(|g| g.kind.is_player_freighter())
            && !p.fuel_critical() && p.stock(C::Provisions) >= 20.0
            && self.cash(p) >= p.operating_reserve() * 2.0 + 5_000.0
            && p.prospects.iter().any(|s| !self.avoided.contains_key(&s.id) && !p.threatened(s.pos) && Self::reachable_prospect(p, s))
    }

    pub(super) fn colonies(&mut self, p: &Observation) -> Option<Command> {
        if p.sites.len() > 1 { self.expansion = None; }
        if self.expansion.is_none() && self.expansion_ready(p) {
            self.expansion = p.prospects.iter().filter(|s| !self.avoided.contains_key(&s.id) && !p.threatened(s.pos) && Self::reachable_prospect(p, s))
                .max_by(|a, b| {
                    let score = |s: &Prospect| s.score / (1.0 + p.home.system.pos.distance(s.pos) / 80_000.0);
                    score(a).total_cmp(&score(b))
                }).map(|s| s.id);
        }
        let target_id = self.expansion?;
        let Some(target) = p.prospects.iter().find(|s| s.id == target_id) else {
            // A newly arrived rival claim (or our own settlement) invalidates
            // the intention. Never colonize from an obsolete ownership report.
            self.expansion = None;
            return None;
        };
        if p.threatened(target.pos) { return None; }
        for fleet in p.own().filter(|g| g.kind == H::Colony) {
            let key = format!("fleet:{}", fleet.id);
            if self.ready(&key, p.now) && !p.fighting.contains(&fleet.id)
                && fleet.vel.length_sq() <= 1.0 && p.enough_fuel(fleet, target.pos)
                && self.cash(p) > p.operating_reserve() + 3_000.0 {
                self.wait(key, p, fleet.pos);
                self.status = "sending a funded colony expedition to surveyed resources".into();
                return Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: target.pos });
            }
        }
        None
    }

    pub(super) fn support_colonies(&mut self, p: &Observation) -> Option<Command> {
        let account = p.account.as_ref()?;
        for site in p.sites.iter().filter(|s| s.system.id != p.home.system.id) {
            let mut local = p.clone();
            local.home = site.clone();
            let project = local.structure(K::VolatileHarvester, 1).or_else(|| local.structure(K::MiningComplex, 1));
            let key = format!("build:{}", site.system.id);
            if self.ready(&key, p.now) && site.builds.is_empty() {
                if let Some(project) = project {
                    if local.project_costs(project).iter().all(|(c, n)| local.stock(*c) >= *n) {
                        if let BuildKind::Upgrade { upgrade } = project.what {
                            self.wait(key, p, site.system.pos);
                            return Some(Command::DevelopSystem { player_id: p.owner, system_id: site.system.id,
                                upgrade, body_id: Some(project.body) });
                        }
                    }
                }
            }
            if !self.ready("market", p.now) { continue; }
            let mut kit = project.map(|goal| local.project_costs(goal)).unwrap_or_default();
            // A new colony has no free food chain: pay for a longer first buffer
            // and its initial extraction plant, not merely a claim flag.
            kit.insert(C::Provisions, 80.0);
            kit.insert(C::Fuel, 20.0);
            for (good, target) in kit {
                let inbound: u32 = p.shipments.iter().filter(|s| s.system == site.system.id
                    && s.direction == sim::tca::ShipmentDir::Outbound && s.commodity == good).map(|s| s.units).sum();
                let need = (target - site.system.free_stock(good) - inbound as f64).ceil().max(0.0) as u32;
                if need < 10 { continue; }
                let stored = account.warehouse.get(&good).copied().unwrap_or(0);
                let n = need.min(stored);
                let fee = sim::tca::freight_fee(n, p.price(good), site.system.pos.distance(p.hub)) * 1.2;
                if n > 0 && self.spendable(p, false) >= fee {
                    self.spend(p, fee);
                    self.status = "supplying a reported new colony".into();
                    return Some(Command::BookFreightOut { player_id: p.owner, system: site.system.id, commodity: good, units: n });
                }
                let cost = need as f64 * p.price(good) * 1.3;
                if stored == 0 && self.spendable(p, false) >= cost + fee {
                    self.spend(p, cost);
                    return Some(Command::MarketBuy { player_id: p.owner, commodity: good, units: need, max_unit_price: Some(p.price(good) * 1.2) });
                }
            }
        }
        None
    }
}
