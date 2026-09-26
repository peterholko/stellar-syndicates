use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Reconnaissance {
    pub system: EntityId, pub point: Vec2, pub since: f64, pub deadline: f64,
}

impl Observation {
    /// First physical leg of a civilian expedition, through supplied owned
    /// berths or the paid hub. This permits networks beyond the original
    /// home→hub triangle, without granting Colony Ships a jump drive.
    pub(super) fn colony_leg(&self, from: Vec2, to: Vec2, fuel: f64, tank: f64) -> Option<Vec2> {
        let burn = |a, b| self.fuel_per_mass(a, b) * H::Colony.hull_mass();
        let reserve = tank * 0.05;
        if burn(from, to) + reserve <= fuel { return Some(to); }
        let ports: Vec<_> = self.sites.iter().filter(|s| s.system.free_stock(C::Fuel) > 1.0
            && s.system.blockade.is_none()).map(|s| s.system.pos)
            .chain(std::iter::once(self.hub)).collect();
        let mut queue = std::collections::VecDeque::new();
        let mut visited = BTreeSet::new();
        for (i, &port) in ports.iter().enumerate() {
            if from.distance(port) > sim::transit::HYPERLIMIT && burn(from, port) + reserve <= fuel {
                visited.insert(i); queue.push_back((i, port));
            }
        }
        while let Some((index, first)) = queue.pop_front() {
            if burn(ports[index], to) + reserve <= tank { return Some(first); }
            for (i, &port) in ports.iter().enumerate() {
                if !visited.contains(&i) && burn(ports[index], port) + reserve <= tank {
                    visited.insert(i); queue.push_back((i, first));
                }
            }
        }
        None
    }
    pub(super) fn scout_leg(&self, fleet: &GhostView, from: Vec2, to: Vec2) -> f64 {
        let distance = from.distance(to);
        if distance <= 4_000.0 { return self.leg(fleet, from, to, 0); }
        let dir = (to - from).normalized();
        // Pay ordinary impulse/warp for both well approaches and the existing
        // jump tariff between them. Playtest jumps are free for EVERY player;
        // restoring the global tariff automatically changes this budget too.
        self.leg(fleet, from, from + dir * 2_000.0, 0)
            + self.leg(fleet, to - dir * 2_000.0, to, 0)
            + sim::fuel::fuel_cost(distance, hull_mass(fleet)) / sim::transit::JUMP_FUEL_FACTOR
                * sim::research::mod_of(&self.research, sim::research::ModKey::FuelConsumption) * 1.15
    }

    fn outside_wells(&self, pos: Vec2) -> bool {
        self.chart.iter().map(|(_, p)| *p).chain(std::iter::once(self.hub))
            .all(|well| well.distance(pos) > sim::transit::HYPERLIMIT + 100.0)
    }

    fn safe_departure(&self, fleet: &GhostView, dest: Vec2) -> bool {
        let direction = (dest - fleet.pos).normalized();
        !self.threatened(dest) && self.ghosts.iter().filter(|g| !g.own && !g.ally
            && !g.owner.is_tca() && g.kind.is_combatant() && g.pos.distance(fleet.pos) < 25_000.0)
            .all(|g| {
                let away = fleet.pos - g.pos;
                // A distant contact can close a sector without imprisoning a
                // Scout at its own dock. Leave only AWAY from every threat,
                // with enough space to clear the well and spool. Never use this
                // exception to investigate a threatened destination.
                away.length() > 8_000.0 && away.x * direction.x + away.y * direction.y > 0.0
            })
    }
}

impl Bot {
    fn recon_step(&mut self, p: &Observation, scout: &GhostView) -> (bool, Option<Command>) {
        if let Some(mission) = self.reconnaissance.get(&scout.id).cloned() {
            if p.military_sites.iter().find(|s| s.id == mission.system)
                .is_none_or(|s| s.intel.observed_at >= mission.since) {
                self.reconnaissance.remove(&scout.id);
            } else if p.now >= mission.deadline || p.threatened(scout.pos) || p.threatened(mission.point) {
                self.reconnaissance.remove(&scout.id);
                self.avoided.insert(mission.system, p.now + 600.0);
                return (true, self.return_for_fuel(p, scout));
            } else {
                let command = if scout.pos.distance(mission.point) > 100.0 {
                    self.scout_step(p, scout, mission.point)
                } else { None };
                return (true, command); // wait for the actual intelligence report
            }
        }
        if !p.expansion_unlocked || self.surveys.contains_key(&scout.id)
            || scout.vel.length_sq() > 1.0 || scout.damage.unwrap_or(0.0) > 0.15 { return (false, None); }
        let target = p.military_sites.iter().filter(|s|
            p.now - s.intel.observed_at > 1_800.0 && !self.avoided.contains_key(&s.id)
                && !self.reconnaissance.values().any(|m| m.system == s.id)
                && !p.threatened(s.pos) && p.safe_departure(scout, s.pos)
                && scout.fuel.unwrap_or(0.0) >= p.scout_leg(scout, scout.pos, s.pos)
                    + p.scout_leg(scout, s.pos, p.home.system.pos) + p.arrival_reserve(scout))
            .min_by(|a, b| scout.pos.distance(a.pos).total_cmp(&scout.pos.distance(b.pos)));
        let Some(site) = target else { return (false, None); };
        // Observe from the near edge of the public scout contact radius, not
        // the enclave's battle trigger. A visible threat always aborts; an old
        // report never justifies asking a civilian Scout to fight its way in.
        let mut direction = (scout.pos - site.pos).normalized();
        if direction.length_sq() < 0.5 { direction = Vec2::new(1.0, 0.0); }
        let point = site.pos + direction * (sim::ship::SCOUT_INTEL_RANGE * 0.9);
        self.reconnaissance.insert(scout.id, Reconnaissance { system: site.id, point, since: p.now,
            deadline: p.now + p.round_trip(site.pos) + scout.pos.distance(site.pos) / H::Scout.max_speed() + 600.0 });
        let command = self.scout_step(p, scout, point);
        self.status = "refreshing stale campaign intelligence with a threat-avoiding Scout".into();
        (true, command)
    }

    pub(super) fn scout_step(&mut self, p: &Observation, fleet: &GhostView, dest: Vec2) -> Option<Command> {
        if fleet.vel.length_sq() > 1.0 || fleet.jump_spool.is_some() || fleet.jump_presumed.is_some() { return None; }
        let delta = dest - fleet.pos;
        let distance = delta.length();
        let direction = delta.normalized();
        let key = format!("fleet:{}", fleet.id);
        let command = if distance <= 4_000.0 {
            Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest }
        } else if !p.outside_wells(fleet.pos) {
            let point = fleet.pos + direction * 2_000.0;
            if !p.outside_wells(point) { return None; }
            Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: point }
        } else {
            // Stop outside the target well, never spool inside one. The public
            // base range is conservative even in a range-extending nebula.
            let span = (distance - 2_000.0).min(sim::transit::JUMP_RANGE * 0.9);
            // A blocked forward fan does not close the expedition. Try wider,
            // then shorter, progress-making hops around observed threats; never
            // trade the Scout's safety gate for a more direct approach.
            let point = [span, span * 0.65, span * 0.35].into_iter().flat_map(|length| {
                [0.0_f64, 0.25, -0.25, 0.5, -0.5, 0.9, -0.9, 1.2, -1.2].into_iter().map(move |angle| {
                    let dir = Vec2::new(direction.x * angle.cos() - direction.y * angle.sin(),
                        direction.x * angle.sin() + direction.y * angle.cos());
                    fleet.pos + dir * length
                })
            }).find(|point| point.distance(dest) < distance - 1.0 && p.outside_wells(*point)
                && (self.returns.contains_key(&fleet.id) || !p.threatened(*point)))?;
            Command::JumpShip { player_id: p.owner, ship_id: fleet.id, dest: point }
        };
        // Wait through spool AND the destination report; an owner-only presumed
        // bookmark is not arrival evidence for issuing another jump.
        let step = match &command { Command::MoveShip { dest, .. } | Command::JumpShip { dest, .. } => *dest, _ => unreachable!() };
        self.waits.insert(key, p.now + p.round_trip(fleet.pos).max(p.round_trip(step))
            + sim::transit::JUMP_SPOOL_S + THINK_SECONDS);
        self.status = "staging a Scout expedition through observed jump waypoints".into();
        Some(command)
    }

    pub(super) fn reachable_prospect(p: &Observation, s: &Prospect) -> bool {
        let tank = H::Colony.hull_mass() * sim::fuel::FUEL_PER_HULL_MASS
            * sim::research::mod_of(&p.research, sim::research::ModKey::FuelCapacity);
        p.colony_leg(p.home.system.pos, s.pos, tank, tank).is_some()
    }
    pub(super) fn scouts(&mut self, p: &Observation) -> Option<Command> {
        if p.opening_local_work() { return None; }
        for scout in p.own().filter(|g| g.kind == H::Scout && !g.stalled) {
            let key = format!("fleet:{}", scout.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&scout.id) { continue; }
            let can_depart = p.chart.iter().any(|(id, pos)| !p.surveyed.contains(id)
                && !self.avoided.contains_key(id) && p.safe_departure(scout, *pos)
                && self.surveys.get(&scout.id).is_none_or(|target| target == id));
            if p.threatened(scout.pos) && !can_depart || scout.damage.unwrap_or(0.0) > 0.35 {
                if let Some(mission) = self.reconnaissance.remove(&scout.id) {
                    self.avoided.insert(mission.system, p.now + 600.0);
                }
                if let Some(target) = self.surveys.remove(&scout.id) {
                    self.survey_started.remove(&scout.id);
                    self.avoided.insert(target, p.now + 600.0);
                }
                if scout.pos.distance(p.home.system.pos) > sim::transit::HYPERLIMIT {
                    if let Some(trip) = self.returns.get(&scout.id) {
                        let dest = trip.destination;
                        if let Some(cmd) = self.scout_step(p, scout, dest) { return Some(cmd); }
                        continue;
                    }
                    if let Some(cmd) = self.return_for_fuel(p, scout) { return Some(cmd); }
                }
                continue;
            }
            if let Some(trip) = self.returns.get(&scout.id) {
                let dest = trip.destination;
                if let Some(cmd) = self.scout_step(p, scout, dest) { return Some(cmd); }
                continue;
            }
            let (recon, command) = self.recon_step(p, scout);
            if command.is_some() { return command; }
            if recon { continue; }
            if let Some(&target) = self.surveys.get(&scout.id) {
                if !p.surveyed.contains(&target) {
                    if !self.survey_started.contains(&scout.id) {
                        if let Some((_, pos)) = p.chart.iter().find(|(id, _)| *id == target) {
                            if scout.pos.distance(*pos) > 4_000.0 {
                                if let Some(cmd) = self.scout_step(p, scout, *pos) { return Some(cmd); }
                                continue;
                            }
                            if scout.vel.length_sq() > 1.0 || scout.jump_spool.is_some() || scout.jump_presumed.is_some() { continue; }
                            self.survey_started.insert(scout.id);
                            self.wait(key, p, scout.pos);
                            self.waits.insert(format!("survey:{}", scout.id), p.now + p.round_trip(*pos) + 600.0);
                            return Some(Command::SurveySystem { player_id: p.owner, fleet_id: scout.id, system_id: target });
                        }
                    }
                    // An idle old sighting is not a failed survey. Wait for the
                    // round trip, travel, investigation and its report. A real
                    // timeout backs off instead of reissuing the same mission.
                    if !self.ready(&format!("survey:{}", scout.id), p.now) { continue; }
                    self.avoided.insert(target, p.now + 600.0);
                }
                self.surveys.remove(&scout.id);
                self.survey_started.remove(&scout.id);
            } else if scout.survey_progress.is_some() || scout.vel.length_sq() > 1.0 { continue; }
            let target = p.chart.iter().filter(|(id, pos)| !p.surveyed.contains(id)
                && !self.avoided.contains_key(id) && !self.surveys.values().any(|other| other == id)
                && !p.threatened(*pos)
                && (!p.threatened(scout.pos) || p.safe_departure(scout, *pos))
                && scout.fuel.unwrap_or(0.0) >= p.scout_leg(scout, scout.pos, *pos)
                    + p.scout_leg(scout, *pos, p.home.system.pos) + p.arrival_reserve(scout))
                // The founder's two named surveys unlock expansion; arbitrary
                // nearby surveys do not. Prioritize those PLAYER-KNOWN leads,
                // but only after the same threat and return-fuel filters above.
                .min_by(|(a_id, a), (b_id, b)| {
                    let priority = |id| p.expansion_unlocked || !p.survey_candidates.contains(id);
                    priority(a_id).cmp(&priority(b_id))
                        .then_with(|| scout.pos.distance(*a).total_cmp(&scout.pos.distance(*b)))
                });
            if let Some(&(system_id, pos)) = target {
                // A partial berth fill is allowed only if the whole excursion
                // fits; the return leg/reserve above is never traded for reach.
                self.wait(key, p, scout.pos);
                self.waits.insert(format!("survey:{}", scout.id), p.now + p.round_trip(pos)
                    + scout.pos.distance(pos) / H::Scout.max_speed() + 600.0);
                self.surveys.insert(scout.id, system_id);
                if scout.pos.distance(pos) > 4_000.0 { return self.scout_step(p, scout, pos); }
                self.survey_started.insert(scout.id);
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
        // This is the actual founding unlock, not a substitute "two surveys"
        // heuristic. Keep the ordinary build validator's gate unchanged.
        let stable = p.sites.iter().filter(|s| s.system.id != p.home.system.id).all(|s|
            s.system.tier(K::MiningComplex) + s.system.tier(K::VolatileHarvester) + s.system.tier(K::Bioharvester) > 0
            && s.system.food_state == sim::colony::FoodState::WellSupplied
            && s.system.blockade.is_none());
        p.expansion_unlocked && stable && p.exported && p.research.completed.len() >= 2
            && p.home.system.tier(K::Academy) > 0 && p.home.system.tier(K::MiningComplex) > 0
            && p.count(H::Scout) > 0 && p.own().any(|g| g.kind.is_player_freighter())
            && !p.fuel_critical() && p.stock(C::Provisions) >= 20.0
            && p.own().filter(|g| g.kind.is_player_freighter()).count() >= p.sites.len()
            && self.cash(p) >= p.operating_reserve() * (1.5 + p.sites.len() as f64 / self.priorities().expansion) + (H::Colony.hull_mass()
                * sim::fuel::FUEL_PER_HULL_MASS * sim::research::mod_of(&p.research, sim::research::ModKey::FuelCapacity)
                * p.price(C::Fuel) * 1.25).max(5_000.0)
            && p.prospects.iter().any(|s| !self.avoided.contains_key(&s.id) && !p.threatened(s.pos) && Self::reachable_prospect(p, s))
    }

    pub(super) fn colonies(&mut self, p: &Observation) -> Option<Command> {
        if !p.expansion_unlocked { self.expansion = None; }
        if self.expansion.is_none() && self.expansion_ready(p) {
            self.expansion = p.prospects.iter().filter(|s| !self.avoided.contains_key(&s.id) && !p.threatened(s.pos) && Self::reachable_prospect(p, s))
                .max_by(|a, b| {
                    let score = |s: &Prospect| s.score / (1.0 + p.sites.iter()
                        .map(|port| port.system.pos.distance(s.pos)).min_by(f64::total_cmp).unwrap_or(f64::INFINITY) / 80_000.0);
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
            if !self.ready(&key, p.now) || p.fighting.contains(&fleet.id)
                || self.returns.contains_key(&fleet.id) { continue; }
            // A new claim or pirate report can redirect an already-built hull.
            // Refill at a reachable berth rather than waiting forever for a
            // tank that cannot refill in open space at the discarded target.
            if fleet.vel.length_sq() > 1.0 { continue; }
            let step = p.colony_leg(fleet.pos, target.pos, fleet.fuel.unwrap_or(0.0), p.tank(fleet));
            if step.is_none() {
                if let Some(cmd) = self.return_for_fuel(p, fleet) { return Some(cmd); }
            }
            if let Some(dest) = step.filter(|dest| {
                let bunker = if dest.distance(p.hub) < sim::transit::HYPERLIMIT {
                    p.tank(fleet) * p.price(C::Fuel) * 1.25
                } else { 0.0 };
                self.cash(p) > p.operating_reserve() + bunker + 3_000.0
            }) {
                self.wait(key, p, fleet.pos);
                self.status = "staging a funded colony expedition through reported refuelling ports".into();
                return Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id,
                    dest });
            }
        }
        None
    }

}
