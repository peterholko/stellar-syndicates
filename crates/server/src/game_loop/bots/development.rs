//! A funded intention survives replenishment, travel and delayed reports. This
//! is CC bookkeeping only: no extra resources, accelerated research or truth reads.
use super::*;

impl Bot {
    pub(super) fn recovery_project(&self, p: &Observation) -> Option<Project> {
        // Losing the last earning hull or Interceptor changes the plan. Count
        // paid replacement jobs too; a stale casualty picture must not order
        // the same replacement at every shipyard.
        let hull = if !sim::ship::ALL_SHIP_KINDS.into_iter()
            .any(|h| h.is_player_freighter() && p.count(h) > 0) { H::TinyFreighter }
            else if p.count(H::Raider) == 0 { H::Raider } else { return None; };
        p.ship(hull, 1).or_else(|| p.structure(K::Shipyard, 1))
    }

    pub(super) fn next_research(&self, p: &Observation, wanted: &BTreeMap<C, f64>) -> Option<&'static str> {
        let available = |id: &str| p.available_research.iter().any(|s| s == id);
        let safe = |s: &&Prospect| !self.avoided.contains_key(&s.id) && !p.threatened(s.pos);
        let range_blocked = p.expansion_unlocked
            && p.prospects.iter().filter(safe).next().is_some()
            && !p.prospects.iter().filter(safe).any(|s| Self::reachable_prospect(p, s));
        // A fuel-range blocker is a concrete reason to progress to Tier II.
        // Still use the player's ordinary prerequisite/metric availability;
        // no technology, larger tank or jump capability is granted for free.
        if range_blocked && available("prop_efficient_burns") { return Some("prop_efficient_burns"); }
        // §fuel-overhaul: freight before anything optional. Freight Frames opens
        // the 150-unit Small Freighter (and satisfies the charter's research
        // step); Fusion Freight, once its convoy count is in, makes full-speed
        // freight cost what slow freight does.
        if let Some(id) = ["prop_freight_frames", "prop_line_fusion_freight"].into_iter()
            .find(|id| available(id) && !p.research.has(id)) {
            return Some(id);
        }
        // §belts phase 6: barges at work and an unowned belt worth a station.
        if p.wants_station() && available("mat_belt_stations") { return Some("mat_belt_stations"); }
        // §power: a planet that has outgrown a tier IV plant needs fusion.
        if p.power_shortfall().is_some_and(|(body, _)| p.home.system.bodies.iter()
            .any(|b| b.id == body && b.tier(K::PowerPlant) >= sim::build::BASE_MAX_STRUCTURE_TIER))
            && available(sim::power::FUSION_RESEARCH)
        {
            return Some(sim::power::FUSION_RESEARCH);
        }
        // Score EVERY legal catalogue entry. Completed openings never exhaust
        // the controller's technology list; new tiers compete by useful effects,
        // colony role and the corporation's investment preferences.
        let score = |r: &sim::research::Programme| {
            let mut value = self.research_value(p, r);
            for kind in K::ALL {
                let unlock = kind.research_prerequisite() == Some(r.id)
                    || r.effects.iter().any(|e| matches!(e, sim::research::Effect::UnlockStructureTier(k, _) if *k == kind));
                if !unlock { continue; }
                // Prefer research which can replace a material shortage in the
                // funded plan, not merely whichever tier costs the least.
                for (&good, &n) in wanted.iter().filter(|(_, n)| **n > 0.0) {
                    let makes = sim::production::extraction_structure(good) == Some(kind)
                        || sim::production::converter_for(kind).is_some_and(|c| c.outputs().any(|(g, _)| g == good));
                    if makes { value += ((n - p.stock(good) - p.inbound(good) as f64) / n).clamp(0.0, 1.0)
                        * 100.0 * self.priorities().industry; }
                }
            }
            value
        };
        p.available_research.iter().filter_map(|id| sim::research::programme(id))
            .filter(|r| !r.hidden && !p.research.has(r.id))
            .max_by(|a, b| score(a).total_cmp(&score(b))
                .then_with(|| b.id.cmp(a.id))).map(|r| r.id)
    }

    pub(super) fn refresh_development(&mut self, p: &Observation) {
        // A restored pre-fix intention cannot bypass the founding gate. A
        // settlement report also satisfies the goal if its consumed hull was
        // never seen between construction and arrival; don't buy a duplicate.
        let colony_done_or_locked = !p.expansion_unlocked
            || self.expansion.is_some_and(|id| !p.prospects.iter().any(|s| s.id == id));
        if colony_done_or_locked { self.expansion = None; }
        if self.development.as_ref().is_some_and(|goal| {
            match goal.project.what {
                BuildKind::Ship { ship: H::Colony } if colony_done_or_locked => true,
                BuildKind::Upgrade { upgrade } => p.home.system.bodies.iter()
                    .find(|b| b.id == goal.project.body).is_none_or(|b| b.tier(upgrade) >= goal.target),
                BuildKind::Ship { ship } => p.own().flat_map(|f| f.composition.iter().flatten())
                    .filter(|s| s.kind == ship).map(|s| s.count).sum::<u32>() >= goal.target,
                _ => true,
            }
        }) { self.development = None; }
        // Preserve funded construction and orders whose outcomes have not yet
        // arrived. An UNPAID optional ambition can yield to casualty recovery;
        // merely needing another fuel delivery does not churn the chosen goal.
        if p.home.builds.is_empty() && self.ready("build", p.now) {
            if let Some(project) = self.recovery_project(p) {
                if self.development.as_ref().is_some_and(|g| g.project != project) {
                    self.development = None;
                }
            }
        }
        if self.development.is_none() && p.home.builds.is_empty() {
            if let Some(project) = self.recovery_project(p).or_else(|| self.next_project(p)) {
                let target = match project.what {
                    BuildKind::Upgrade { upgrade } => p.home.system.bodies.iter()
                        .find(|b| b.id == project.body).map_or(0, |b| b.tier(upgrade)) + 1,
                    BuildKind::Ship { ship } => p.count(ship) + 1,
                    _ => return,
                };
                self.development = Some(DevelopmentGoal { project, target, since: p.now });
            }
        }
    }

    pub(super) fn explain_blocker(&mut self, p: &Observation, project: Option<Project>) {
        self.status = if !p.home.builds.is_empty() {
            "development: awaiting reported construction completion".into()
        } else if self.recovering(p) {
            "development: protecting operating cash before investment".into()
        } else if p.fuel_critical() {
            "development: awaiting operating fuel".into()
        } else if let Some(project) = project {
            let missing: Vec<_> = p.project_costs(project).into_iter()
                .filter(|(c, n)| p.stock(*c) + 1e-9 < *n)
                .map(|(c, n)| format!("{c:?} {:.0}", (n - p.stock(c)).ceil())).collect();
            format!("development {:?}: {}", project.what, if missing.is_empty() {
                "awaiting command/report".into()
            } else { format!("awaiting {}", missing.join(", ")) })
        } else if p.research.active.is_some() {
            if p.home.academies.iter().any(|a| a.rate > 0.0 && a.supplied) {
                "research: funded work in progress".into()
            } else { "research: awaiting supplies or workforce".into() }
        } else if !p.expansion_unlocked {
            format!("expansion: completing founding {:?}; {} candidate reports outstanding", p.founding_stage,
                p.survey_candidates.iter().filter(|id| !p.surveyed.contains(id)).count())
        } else if p.prospects.is_empty() {
            "expansion: awaiting a safe surveyed colony prospect".into()
        } else { "operating trade, surveys and colony supply".into() };
    }
}
