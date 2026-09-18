//! Persistent local threats and objective contracts. No UI reads this truth
//! sidecar: reconnaissance creates immutable offers on the scout's wavefront,
//! and progress/rewards use the existing arrived Operation reports.
use super::*;
use crate::pirate::{BaseNetwork, CombatObjective as Goal, PirateFaction, SupportSite};
use crate::operation::{FollowUpKind, OperationBriefing, OperationIssuer, OperationKind,
    OperationReward, OperationScope, OperationState};

impl World {
    pub(super) fn spawn_faction_patrol(&mut self, faction: PirateFaction, n: u32, pos: Vec2) -> EntityId {
        let id = self.spawn_fixed_pirates(faction.patrol(n), pos, FleetOrder::Idle);
        let f = self.fleets.get_mut(&id).unwrap();
        f.pirate_faction = Some(faction);
        f.mission_profile = faction.mission();
        id
    }

    pub(super) fn seed_pirate_campaign(&mut self) {
        if self.uses_legacy_test_bootstrap() { return; }
        let sites: Vec<_> = self.enclaves.iter().filter(|(id, e)| !e.cleared && !self.pirate_campaign.bases.contains_key(id))
            .filter_map(|(&id, _)| self.systems.iter().find(|s| s.id == id).map(|s| (id, s.pos))).collect();
        for (base, pos) in sites {
            let faction = PirateFaction::for_base(base);
            let mut supports = Vec::new();
            // Append-only migration: no existing hull is moved, refitted or
            // repaired. Supply/fire-control sites are fixed physical objectives.
            for (i, objective) in [Goal::DisableFireControl, Goal::DisableSupply].into_iter().enumerate() {
                let mut chosen = None;
                for turn in 0..12 {
                    let angle = (base.0 as f64 + i as f64 * 6.0 + turn as f64) * std::f64::consts::TAU / 12.0;
                    let p = pos + Vec2::new(angle.cos(), angle.sin()) * 14_000.0;
                    if self.home_slots.iter().all(|h| h.pos.distance(p) > 30_000.0)
                        && !self.in_sovereign_zone(p)
                        && self.fleets.values().all(|f| f.pos.distance(p) > 3_000.0)
                        && self.systems.iter().all(|s| s.pos.distance(p) > crate::transit::HYPERLIMIT) {
                        chosen = Some(p); break;
                    }
                }
                let Some(p) = chosen else { continue; };
                let id = self.alloc_entity_id();
                let guard = self.spawn_faction_patrol(faction, 1, p);
                supports.push(SupportSite { id, pos: p, objective, guards: vec![guard],
                    disabled_at: None, disabled_by: None, effect_at: None });
            }
            self.pirate_campaign.bases.insert(base, BaseNetwork { faction, supports, patrol: None,
                next_patrol_at: self.time + pirate::CAMPAIGN_PATROL_PERIOD_S, lost_at: None });
        }
    }

    fn campaign_engaged(&self, id: EntityId) -> bool {
        self.engagements.values().any(|e| e.attackers.contains(&id) || e.defenders.contains(&id))
    }

    pub(super) fn pirate_support_disabled(&self, base: EntityId, goal: Goal) -> bool {
        self.pirate_campaign.bases.get(&base).is_some_and(|b| b.supports.iter()
            .any(|s| s.objective == goal && s.effect_at.is_some_and(|t| self.time >= t)))
    }

    pub(super) fn pirate_fire_control(&self, owner: PlayerId, base: Option<EntityId>) -> f64 {
        if owner.is_pirate() && base.is_some_and(|id| self.pirate_support_disabled(id, Goal::DisableFireControl)) { 0.75 } else { 1.0 }
    }

    /// No global hunt: leashed patrols use only local sensor contacts. Scouts
    /// see the threat first and retain their ordinary automatic retreat logic.
    fn campaign_guard(&mut self, id: EntityId, anchor: Vec2, radius: f64) {
        if self.campaign_engaged(id) { return; }
        let Some(f) = self.fleets.get(&id) else { return; };
        let p = f.pos;
        if f.mission_profile.withdrawal.threshold().is_some_and(|limit|
            f.ships.iter().any(|h| h.hp > 0.0 && h.hp / h.max_hp() < limit)) {
            self.fleets.get_mut(&id).unwrap().order = if p.distance(anchor) > 100.0 {
                FleetOrder::MoveTo { dest: anchor }
            } else { FleetOrder::Idle };
            return; // surviving ambushers do not immediately re-enter the fight
        }
        let target = self.fleets.values().filter(|t| !t.owner.is_pirate()
            && (!t.owner.is_tca() || self.pirate_campaign.missions.values().any(|m| m.evacuees == Some(t.id)))
            && t.pos.distance(anchor) <= radius
            && crate::detection::detected(t.signature() * self.veil_factor(t.owner, t.pos)
                * self.nebula_signature_factor(t.pos)
                * if t.surveying() { crate::explore::SURVEY_SIGNATURE_FACTOR } else { 1.0 },
                &[(p, self.config.sensor_range * self.nebula_sensor_factor(p))], t.pos)
            && !self.in_sovereign_zone(t.pos)
            && self.players.get(&t.owner).is_none_or(|c| !c.founding.protected(self.time)
                // Founding protection does not turn an explicitly accepted
                // combat assignment into a risk-free contract. Unassigned
                // beginner traffic keeps the normal protection.
                || self.operations.values().any(|o| o.state == OperationState::Active
                    && o.assigned_fleets.get(&t.owner) == Some(&t.id)
                    && matches!(o.kind, OperationKind::CombatObjective { .. }))))
            .min_by(|a, b| p.distance(a.pos).total_cmp(&p.distance(b.pos)).then(a.id.cmp(&b.id))).map(|t| t.id);
        let next = if p.distance(anchor) > radius { FleetOrder::MoveTo { dest: anchor } }
            else if let Some(target) = target { FleetOrder::Attack { target } }
            else if p.distance(anchor) > 100.0 { FleetOrder::MoveTo { dest: anchor } } else { FleetOrder::Idle };
        self.fleets.get_mut(&id).unwrap().order = next;
    }

    pub(super) fn tick_pirate_campaign(&mut self, events: &mut Vec<Event>) {
        if self.uses_legacy_test_bootstrap() { return; }
        // Static supports are seeded once, not respawned by an offer or restart.
        if !self.tick.is_multiple_of(30) { return; }
        let bases: Vec<_> = self.pirate_campaign.bases.keys().copied().collect();
        for base in bases {
            let Some(pos) = self.systems.iter().find(|s| s.id == base).map(|s| s.pos) else { continue; };
            let alive = self.enclaves.get(&base).is_some_and(|e| e.active(self.time));
            let network = self.pirate_campaign.bases[&base].clone();
            if !alive {
                let lost = *self.pirate_campaign.bases.get_mut(&base).unwrap().lost_at.get_or_insert(self.time);
                // Clearing ends all new launches immediately. Distant surviving
                // detachments receive that news normally, return physically,
                // and retire at the wreck; no instantaneous disappearance.
                for id in network.supports.iter().flat_map(|s| s.guards.iter().copied()).chain(network.patrol) {
                    if self.campaign_engaged(id) { continue; }
                    if let Some(f) = self.fleets.get(&id)
                        && self.time >= lost + crate::transit::delay(pos, f.pos, self.config.c) {
                        if f.pos.distance(pos) <= pirate::PIRATE_ASSAULT_RADIUS { self.fleets.remove(&id); }
                        else { self.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: pos }; }
                    }
                }
                continue;
            }
            self.pirate_campaign.bases.get_mut(&base).unwrap().lost_at = None;
            for support in &network.supports {
                if support.disabled_at.is_none() {
                    for &guard in &support.guards { self.campaign_guard(guard, support.pos, 4_000.0); }
                }
            }
            if let Some(id) = network.patrol.filter(|id| self.fleets.contains_key(id)) {
                if self.fleets[&id].cargo_is_empty() { self.campaign_guard(id, pos, 24_000.0); }
                else if !self.campaign_engaged(id) {
                    if self.fleets[&id].pos.distance(pos) <= pirate::PIRATE_ASSAULT_RADIUS {
                        for cargo in self.fleets.get_mut(&id).unwrap().take_cargo() {
                            *self.enclaves.get_mut(&base).unwrap().plunder.entry(cargo.commodity).or_default() += cargo.units;
                        }
                    } else { self.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: pos }; }
                }
            } else if self.time >= network.next_patrol_at && !self.pirate_support_disabled(base, Goal::DisableSupply) {
                // A bounded extra patrol consumes actual stolen/seeded stores.
                // No free top-ups, no scaling to the player's fleet, no repair of
                // the permanent garrison. Clearing removes this local source.
                let stock = &mut self.enclaves.get_mut(&base).unwrap().plunder;
                let available: u64 = stock.values().map(|n| *n as u64).sum();
                if available >= pirate::CAMPAIGN_PATROL_COST as u64 {
                    let mut need = pirate::CAMPAIGN_PATROL_COST;
                    for n in stock.values_mut() { let used = (*n).min(need); *n -= used; need -= used; }
                    stock.retain(|_, n| *n > 0);
                    let patrol = self.spawn_faction_patrol(network.faction, 1, pos);
                    self.pirate_campaign.bases.get_mut(&base).unwrap().patrol = Some(patrol);
                }
                self.pirate_campaign.bases.get_mut(&base).unwrap().next_patrol_at = self.time + pirate::CAMPAIGN_PATROL_PERIOD_S;
            }
            // The scout, not the CC, observes these structures. Publishing the
            // optional strike contracts rides exactly that scout report's light.
            let viewers: Vec<_> = self.fleets.values().filter(|f| f.contains(ShipKind::Scout)
                && self.players.contains_key(&f.owner) && f.pos.distance(pos) <= crate::ship::SCOUT_INTEL_RANGE)
                .map(|f| (f.owner, f.pos)).collect();
            for (owner, scout_pos) in viewers {
                let e = &self.enclaves[&base];
                let tier = e.tier;
                let intel = format!("{} defense tiers; {} garrison hulls; {} goods stored. {}",
                    self.systems.iter().find(|s| s.id == base).unwrap().tier_sum(crate::build::StructureKind::DefensePlatform),
                    e.pack.and_then(|id| self.fleets.get(&id)).map_or(0, |f| f.total_count()),
                    e.plunder.values().map(|n| *n as u64).sum::<u64>(), network.faction.counter());
                let base_name = self.systems.iter().find(|s| s.id == base).unwrap().name.clone();
                for support in &network.supports {
                    if support.disabled_at.is_some() { continue; }
                    let title = if support.objective == Goal::DisableFireControl { "Disable fire-control relay" } else { "Disable supply depot" };
                    let effect = if support.objective == Goal::DisableFireControl {
                        "Hold unopposed for 20s to reduce base defensive fire by 25%."
                    } else if pirate::permanent_site(tier) { "Hold unopposed for 20s to stop new patrols." }
                    else { "Hold unopposed for 20s to stop new patrols and home-raider launches." };
                    self.offer_combat_objective(owner, support.objective, support.id, support.pos, None,
                        20, 500.0, &format!("{base_name} · {title}"), "Moderate · guarded installation", "Fitted Interceptors or Corvette escort",
                        format!("{} · {intel} {effect} Direct assault remains possible; damage to the main garrison persists.", network.faction.name()), scout_pos, events);
                }
            }
        }
        self.offer_campaign_contracts(events);
        self.tick_combat_objectives(events);
    }

    fn offer_combat_objective(&mut self, owner: PlayerId, objective: Goal, site: EntityId, pos: Vec2,
        destination: Option<Vec2>, goal: u32, credits: f64, title: &str, difficulty: &str,
        suitable: &str, summary: String, report_origin: Vec2, events: &mut Vec<Event>) -> Option<OperationId> {
        if self.operations.values().any(|o| matches!(o.scope, OperationScope::Private { player } if player == owner)
            && matches!(o.kind, OperationKind::CombatObjective { objective: g, site: s, .. } if g == objective && s == site)
            && (!o.state.terminal() || o.known.get(&owner).is_none_or(|k| !k.state.terminal())
                || self.time < o.completed_at.unwrap_or(o.expires_at) + crate::operation::FOLLOW_UP_COOLDOWN_S)) { return None; }
        let id = self.insert_operation(OperationIssuer::Authority, OperationScope::Private { player: owner },
            OperationKind::CombatObjective { objective, site, pos, destination }, goal,
            OperationReward { credits, captain_xp: 100, authority_standing: 2.0, research_insight: 15.0 },
            report_origin, 3600.0, events);
        self.operations.get_mut(&id).unwrap().briefing = Some(OperationBriefing { follow_up: FollowUpKind::CombatPreparation,
            title: title.into(), difficulty: difficulty.into(), suitable_fleets: suitable.into(), summary, variant: 0 });
        Some(id)
    }

    fn offer_campaign_contracts(&mut self, events: &mut Vec<Event>) {
        let players: Vec<_> = self.players.keys().copied().collect();
        for owner in players {
            let c = &self.players[&owner];
            if c.founding.enabled && !c.founding.reward_granted { continue; }
            let home = c.home;
            // The Authority's fixed rescue sites are advertised contract terms,
            // not generated from an unseen player movement or unseen casualty.
            let Some(home_id) = c.home_system else { continue; };
            let axis = (self.hub - home).normalized();
            let across = Vec2::new(-axis.y, axis.x);
            for (i, goal) in [Goal::HoldExtraction, Goal::ProtectEvacuation].into_iter().enumerate() {
                let pos = home + axis * 28_000.0 + across * (24_000.0 + 18_000.0 * i as f64);
                if self.in_sovereign_zone(pos) || self.home_slots.iter().any(|h| h.pos.distance(pos) < 20_000.0) { continue; }
                let extraction = goal == Goal::HoldExtraction;
                let prior = self.operations.values().rev().find(|o| matches!(o.scope, OperationScope::Private { player } if player == owner)
                    && matches!(o.kind, OperationKind::CombatObjective { objective, site, .. } if objective == goal && site == home_id)).map(|o| o.id);
                let old_actors = prior.and_then(|id| self.pirate_campaign.missions.get(&id)).cloned();
                let Some(id) = self.offer_combat_objective(owner, goal, home_id, pos, (!extraction).then_some(self.hub),
                    if extraction { pirate::EXTRACTION_HOLD_S } else { 1 }, if extraction { 1100.0 } else { 1500.0 },
                    if extraction { "Hold the recovery zone" } else { "Protect the evacuation" },
                    "Moderate · objective victory", "Healthy Interceptors; screening Corvette recommended",
                    if extraction { "Hold the zone unopposed for 60s while salvagers recover a cache. Leaving or renewed attack resets the hold; surviving pirates need not be hunted down. 40 Alloys remain on-site for collection." }
                    else { "Meet the stranded Authority liner. Your assigned escort takes up Guard automatically on arrival; protect it to Market. Enemy destruction is optional. Passenger loss fails the contract." }.into(), self.hub, events) else { continue; };
                let mut actors = old_actors.unwrap_or_default();
                actors.pirates.retain(|id| self.fleets.contains_key(id));
                if actors.pirates.is_empty() {
                    actors.pirates.push(self.spawn_faction_patrol(if extraction { PirateFaction::Ironclad } else { PirateFaction::Rift }, 1, pos + across * 6_000.0));
                }
                if !extraction && actors.evacuees.is_none_or(|f| !self.fleets.contains_key(&f)) {
                    let liner = self.alloc_entity_id();
                    self.fleets.insert(liner, Fleet::single(liner, PlayerId::TCA, ShipKind::Freighter, pos, FleetOrder::Idle, None));
                    actors.evacuees = Some(liner); actors.launched = false; actors.arrived = false;
                }
                self.pirate_campaign.missions.insert(id, actors);
                if let Some(old) = prior { self.pirate_campaign.missions.remove(&old); }
            }
            let blockades: Vec<_> = self.systems.iter().filter(|s| s.owner == Some(owner) && s.blockade.is_some()).map(|s| (s.id, s.pos)).collect();
            for (system, pos) in blockades {
                self.offer_combat_objective(owner, Goal::BreakBlockade, system, pos, None, 20, 1000.0,
                    "Break the blockade", "High · contested system", "Combat fleet suited to the reported blockaders",
                    "Drive the blockaders away and hold the system clear for 20s. They can retreat or withdraw; no kill quota. Ownership must remain yours.".into(), pos, events);
            }
        }
    }

    pub(super) fn contract_guard_authorized(&self, owner: PlayerId, target: EntityId) -> bool {
        self.pirate_campaign.missions.iter().any(|(id, m)| m.evacuees == Some(target)
            && self.operations.get(id).is_some_and(|o| o.participants.contains(&owner) && !o.state.terminal()))
    }

    fn tick_combat_objectives(&mut self, events: &mut Vec<Event>) {
        // Arrival belongs to the passenger hull, even if its guard subsequently
        // died, was reassigned, or the contract expired. Retire only at the real
        // berth; terminal contracts never pay again. The sidecar remains bounded
        // to the newest pair of repeatable objectives per corporation.
        let arrivals: Vec<_> = self.pirate_campaign.missions.iter().filter_map(|(&id, m)| {
            let liner = m.evacuees?;
            (m.launched && !m.arrived && self.fleets.get(&liner).is_some_and(|f|
                f.pos.distance(self.hub) <= crate::ship::DOCK_RADIUS) && !self.campaign_engaged(liner))
                .then_some((id, liner))
        }).collect();
        for (id, liner) in arrivals {
            self.pirate_campaign.missions.get_mut(&id).unwrap().arrived = true;
            if let Some(o) = self.operations.get(&id)
                && o.state == OperationState::Active && self.time <= o.expires_at
                && let OperationScope::Private { player } = o.scope {
                self.complete_operation(id, Some(player), self.hub, events);
            }
            self.fleets.remove(&liner);
        }
        let ids: Vec<_> = self.operations.values().filter(|o| matches!(o.kind, OperationKind::CombatObjective { .. }) && !o.state.terminal()).map(|o| o.id).collect();
        for id in ids {
            let op = self.operations[&id].clone();
            let OperationKind::CombatObjective { objective, site, pos, destination } = op.kind else { continue; };
            let OperationScope::Private { player } = op.scope else { continue; };
            if op.state != OperationState::Active || self.time > op.expires_at { continue; }
            if objective == Goal::ProtectEvacuation && self.pirate_campaign.missions.get(&id)
                .is_some_and(|m| m.evacuees.is_none_or(|f| !self.fleets.contains_key(&f))) {
                self.fail_combat_objective(id, player, pos, events); continue;
            }
            let Some(fid) = op.assigned_fleets.get(&player).copied() else { continue; };
            let Some(fleet) = self.fleets.get(&fid).filter(|f| f.owner == player && f.is_combatant()) else { continue; };
            let on_site = fleet.pos.distance(pos) <= crate::operation::SALVAGE_RECOVERY_RADIUS;
            let contested = self.campaign_engaged(fid) || self.fleets.values().any(|f| f.owner.is_pirate()
                && f.is_combatant() && f.pos.distance(pos) <= 1_500.0);
            if let Some(actors) = self.pirate_campaign.missions.get(&id).cloned() {
                for pirate in actors.pirates.iter().copied() { self.campaign_guard(pirate, pos, 10_000.0); }
                if objective == Goal::ProtectEvacuation {
                    let Some(liner) = actors.evacuees.filter(|f| self.fleets.contains_key(f)) else {
                        self.fail_combat_objective(id, player, pos, events); continue;
                    };
                    let end = destination.unwrap_or(self.hub);
                    if on_site && !actors.launched {
                        self.pirate_campaign.missions.get_mut(&id).unwrap().launched = true;
                        self.fleets.get_mut(&liner).unwrap().order = FleetOrder::MoveTo { dest: end };
                        self.fleets.get_mut(&fid).unwrap().order = FleetOrder::Guard { target: liner };
                    }
                    continue;
                }
            }
            let support = self.pirate_campaign.bases.iter().find_map(|(&base, b)| b.supports.iter().find(|s| s.id == site).map(|s| (base, s.clone())));
            if let Some((base, ref s)) = support {
                if !self.enclaves.get(&base).is_some_and(|e| e.active(self.time)) {
                    self.fail_combat_objective(id, player, pos, events); continue;
                }
                if s.disabled_at.is_some() {
                    if s.disabled_by == Some(player) { self.complete_operation(id, Some(player), pos, events); }
                    else { self.fail_combat_objective(id, player, pos, events); }
                    continue;
                }
            }
            let clear = on_site && !contested && if objective == Goal::BreakBlockade {
                self.systems.iter().any(|s| s.id == site && s.owner == Some(player) && s.blockade.is_none())
            } else { true };
            let elapsed = if clear { (self.time - op.hold_since.unwrap_or(self.time)).max(0.0) as u32 } else { 0 };
            let o = self.operations.get_mut(&id).unwrap();
            o.hold_since = if clear { Some(op.hold_since.unwrap_or(self.time)) } else { None };
            o.progress = elapsed.min(o.goal);
            if elapsed / 5 != op.progress / 5 || (!clear && op.progress > 0) {
                self.queue_operation_report(id, &[player], pos, events);
            }
            if elapsed < op.goal { continue; }
            if let Some((base, _)) = support {
                let base_pos = self.systems.iter().find(|s| s.id == base).unwrap().pos;
                let s = self.pirate_campaign.bases.get_mut(&base).unwrap().supports.iter_mut().find(|s| s.id == site).unwrap();
                s.disabled_at = Some(self.time); s.disabled_by = Some(player);
                s.effect_at = Some(self.time + crate::transit::delay(pos, base_pos, self.config.c));
            }
            self.complete_operation(id, Some(player), pos, events);
            if objective == Goal::HoldExtraction {
                self.insert_operation(OperationIssuer::SalvageOffice, OperationScope::Private { player },
                    OperationKind::RescueSalvage { pos, commodity: crate::Commodity::Alloys, units: 40, source_fleet: EntityId(0) },
                    40, OperationReward::default(), pos, 3600.0, events);
            }
        }
    }

    fn fail_combat_objective(&mut self, id: OperationId, owner: PlayerId, pos: Vec2, events: &mut Vec<Event>) {
        let o = self.operations.get_mut(&id).unwrap();
        if o.state.terminal() { return; }
        o.state = OperationState::Failed; o.completed_at = Some(self.time);
        self.queue_operation_report(id, &[owner], pos, events);
    }

    /// The v6 hulls first retreat across the actual arena under pursuit fire.
    /// Only then leave their engagement, preserving survivor accounting and their
    /// battle position. Other fleets on the same side keep fighting.
    pub(super) fn finish_mission_withdrawals(&mut self) {
        let leaving: std::collections::BTreeSet<_> = self.engagements.values().filter_map(|e| e.tactical.as_ref())
            .flat_map(|t| t.withdrawn_mission_fleets()).collect();
        for fid in leaving {
            let Some(f) = self.fleets.get(&fid) else { continue; };
            let owner = f.owner;
            let destination = self.players.get(&owner).map(|c| c.home).or_else(|| self.enclaves.iter()
                .find(|(_, e)| e.pack == Some(fid)).and_then(|(id, _)| self.systems.iter().find(|s| s.id == *id).map(|s| s.pos)))
                .unwrap_or(f.pos + Vec2::new(20_000.0, 0.0));
            let departure = self.aftermath_fleet(owner, fid, true, None);
            for e in self.engagements.values_mut() {
                let side = if e.attackers.contains(&fid) { 0 } else if e.defenders.contains(&fid) { 1 } else { continue; };
                if let Some(fleet) = &departure {
                    let start = if side == 0 { &mut e.a_start } else { &mut e.d_start };
                    for (kind, count) in &fleet.composition { if let Some(n) = start.get_mut(kind) { *n = n.saturating_sub(*count); } }
                    start.retain(|_, n| *n > 0);
                    e.departed[side].insert(fid, fleet.clone());
                }
                e.attackers.retain(|f| *f != fid); e.defenders.retain(|f| *f != fid);
                if side == 0 { e.a_fled = true; } else { e.d_fled = true; }
            }
            let f = self.fleets.get_mut(&fid).unwrap();
            f.order = FleetOrder::MoveTo { dest: destination }; f.defense = None; f.vel = Vec2::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{DamageWithdrawal, MissionProfile, TargetPriority};

    fn scene() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(991, 4));
        let p = PlayerId(91001);
        w.step(&[Command::AddPlayer { id: p, name: "Prepared".into() }]);
        w.players.get_mut(&p).unwrap().founding.enabled = false;
        (w, p)
    }
    fn fleet(w: &mut World, p: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, p, kind, pos, FleetOrder::Idle, None)); id
    }
    fn offer(w: &mut World, p: PlayerId, goal: Goal, site: EntityId, pos: Vec2) -> OperationId {
        w.offer_combat_objective(p, goal, site, pos, None, 20, 100.0, "Objective", "Moderate", "Combat fleet",
            "Complete on-site".into(), pos, &mut vec![]).unwrap()
    }
    fn assign(w: &mut World, p: PlayerId, op: OperationId, f: EntityId) {
        // The command-path test below separately covers delayed dispatch.
        let o = w.operations.get_mut(&op).unwrap();
        o.state = OperationState::Active; o.participants.insert(p); o.assigned_fleets.insert(p, f);
    }

    #[test]
    fn factions_have_distinct_legal_full_hull_fittings_and_survival_policies() {
        let (mut w, _) = scene();
        let mut forms = Vec::new();
        for faction in [PirateFaction::Ashwake, PirateFaction::Ironclad, PirateFaction::Rift] {
            let id = w.spawn_faction_patrol(faction, 2, Vec2::ZERO);
            let f = &w.fleets[&id];
            assert_eq!(f.pirate_faction, Some(faction));
            assert_eq!(f.ships.len(), 2);
            assert!(f.ships.iter().all(|s| s.loadout.validate(s.kind) && s.hp == s.max_hp()));
            forms.push(serde_json::to_value(&f.loadouts).unwrap());
        }
        assert!(forms[0] != forms[1] && forms[1] != forms[2] && forms[0] != forms[2]);
        assert!(ShipKind::Corvette.max_speed() < ShipKind::Raider.max_speed());
        assert_eq!(PirateFaction::Rift.mission().withdrawal, DamageWithdrawal::Hull50);
    }

    #[test]
    fn mission_profile_is_owner_only_and_delivered_not_immediate() {
        let (mut w, p) = scene();
        let f = fleet(&mut w, p, ShipKind::Raider, Vec2::new(120_000.0, 50_000.0));
        let requested = MissionProfile { priority: TargetPriority::MissileShips, withdrawal: DamageWithdrawal::Hull50, ..Default::default() };
        w.apply(&Command::SetFleetMission { player_id: PlayerId(999), fleet_id: f, mission: requested }, &mut vec![]);
        assert!(w.pending_orders.iter().all(|o| o.ship_id != f));
        w.apply(&Command::SetFleetMission { player_id: p, fleet_id: f, mission: requested }, &mut vec![]);
        let due = w.pending_orders.iter().find(|o| o.ship_id == f).unwrap().apply_time;
        assert!(due > w.time);
        assert_eq!(w.fleets[&f].mission_profile, MissionProfile::default());
        w.time = due - 1e-6; w.deliver_due_orders(&mut vec![]);
        assert_eq!(w.fleets[&f].mission_profile, MissionProfile::default());
        w.time = due + 1e-6; w.deliver_due_orders(&mut vec![]);
        assert_eq!(w.fleets[&f].mission_profile, requested);
        assert!(w.pending_echoes.iter().any(|e| e.fleet == f));
    }

    #[test]
    fn scout_weakness_reports_are_private_frozen_and_light_delayed() {
        let (mut w, p) = scene();
        let base = *w.pirate_campaign.bases.iter().find(|(_, b)| b.supports.len() == 2).unwrap().0;
        let pos = w.systems.iter().find(|s| s.id == base).unwrap().pos;
        fleet(&mut w, p, ShipKind::Scout, pos + Vec2::new(100.0, 0.0));
        w.tick = 30; w.tick_pirate_campaign(&mut vec![]);
        let ids: Vec<_> = w.operations.values().filter(|o| matches!(o.kind,
            OperationKind::CombatObjective { objective: Goal::DisableFireControl | Goal::DisableSupply, .. })).map(|o| o.id).collect();
        assert_eq!(ids.len(), 2);
        for id in &ids { assert!(!w.operations[id].is_visible_to(p)); }
        let brief = w.operations[&ids[0]].briefing.as_ref().unwrap().summary.clone();
        w.enclaves.get_mut(&base).unwrap().plunder.clear();
        let last = w.pending_operation_reports.iter().filter(|r| ids.contains(&r.operation)).map(|r| r.arrive_at).fold(0.0, f64::max);
        w.time = last + 1e-6; w.deliver_operation_reports();
        for id in ids { assert!(w.operations[&id].is_visible_to(p)); assert!(!w.operations[&id].is_visible_to(PlayerId(123))); }
        assert!(w.operations.values().any(|o| o.briefing.as_ref().is_some_and(|b| b.summary == brief)));
    }

    #[test]
    fn disabling_support_changes_the_base_after_its_own_signal_not_before() {
        let (mut w, p) = scene();
        let (&base, net) = w.pirate_campaign.bases.iter().find(|(_, b)| b.supports.len() == 2).unwrap();
        let support = net.supports[0].clone();
        for guard in &support.guards { w.fleets.remove(guard); }
        let id = offer(&mut w, p, Goal::DisableFireControl, support.id, support.pos);
        let f = fleet(&mut w, p, ShipKind::Raider, support.pos); assign(&mut w, p, id, f);
        let credits = w.players[&p].credits;
        w.tick_combat_objectives(&mut vec![]); w.time += 20.1; w.tick_combat_objectives(&mut vec![]);
        let s = &w.pirate_campaign.bases[&base].supports[0];
        assert!(s.disabled_at.is_some()); let effect = s.effect_at.unwrap();
        assert!(effect > w.time);
        assert_eq!(w.pirate_fire_control(PlayerId::PIRATE, Some(base)), 1.0);
        assert_eq!(w.players[&p].credits, credits);
        w.time = effect + 1e-6;
        assert_eq!(w.pirate_fire_control(PlayerId::PIRATE, Some(base)), 0.75);
        assert_eq!(w.pirate_fire_control(p, Some(base)), 1.0);
        let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        restored.fixup_after_load();
        assert_eq!(restored.pirate_campaign.bases[&base].supports[0].disabled_at, s.disabled_at);
    }

    #[test]
    fn stolen_stores_fund_one_patrol_and_clearance_sends_survivors_home_with_light() {
        let (mut w, _) = scene();
        let base = *w.enclaves.iter().find(|(_, e)| e.tier == pirate::STRONGHOLD_TIER).unwrap().0;
        let pos = w.systems.iter().find(|s| s.id == base).unwrap().pos;
        let before = w.enclaves[&base].plunder.values().sum::<u32>();
        w.time = 301.0; w.tick = 30; w.tick_pirate_campaign(&mut vec![]);
        let patrol = w.pirate_campaign.bases[&base].patrol.unwrap();
        assert_eq!(w.enclaves[&base].plunder.values().sum::<u32>(), before - pirate::CAMPAIGN_PATROL_COST);
        w.time += 500.0; w.tick_pirate_campaign(&mut vec![]);
        assert_eq!(w.pirate_campaign.bases[&base].patrol, Some(patrol));
        assert_eq!(w.enclaves[&base].plunder.values().sum::<u32>(), before - pirate::CAMPAIGN_PATROL_COST);
        w.fleets.get_mut(&patrol).unwrap().pos = pos + Vec2::new(20_000.0, 0.0);
        w.fleets.get_mut(&patrol).unwrap().order = FleetOrder::Idle;
        w.enclaves.get_mut(&base).unwrap().cleared = true;
        w.tick_pirate_campaign(&mut vec![]);
        assert!(matches!(w.fleets[&patrol].order, FleetOrder::Idle));
        w.time += 100.0; w.tick_pirate_campaign(&mut vec![]);
        assert!(matches!(w.fleets[&patrol].order, FleetOrder::MoveTo { dest } if dest == pos));
        w.fleets.get_mut(&patrol).unwrap().pos = pos; w.tick_pirate_campaign(&mut vec![]);
        assert!(!w.fleets.contains_key(&patrol));
        w.time += 10000.0; w.tick_pirate_campaign(&mut vec![]);
        assert!(!w.fleets.contains_key(&patrol), "clearance does not regenerate a local patrol");
    }

    #[test]
    fn extraction_resets_when_contested_and_wins_without_killing_distant_enemies() {
        let (mut w, p) = scene();
        let pos = Vec2::new(250_000.0, 180_000.0);
        let id = offer(&mut w, p, Goal::HoldExtraction, EntityId(987), pos);
        let f = fleet(&mut w, p, ShipKind::Raider, pos); assign(&mut w, p, id, f);
        w.tick_combat_objectives(&mut vec![]); w.time += 10.0; w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].progress, 10);
        let pirate = w.spawn_faction_patrol(PirateFaction::Ashwake, 1, pos + Vec2::new(500.0, 0.0));
        w.tick_combat_objectives(&mut vec![]); assert_eq!(w.operations[&id].progress, 0);
        w.fleets.get_mut(&pirate).unwrap().pos = pos + Vec2::new(5000.0, 0.0);
        w.tick_combat_objectives(&mut vec![]); w.time += 21.0; w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert!(w.fleets.contains_key(&pirate), "kills are not the win condition");
        assert!(w.operations.values().any(|o| matches!(o.kind, OperationKind::RescueSalvage { units: 40, .. })));
    }

    #[test]
    fn evacuation_dispatches_a_physical_liner_and_loss_fails_even_with_escort_lost() {
        let (mut w, p) = scene();
        w.offer_campaign_contracts(&mut vec![]);
        let id = w.operations.values().find(|o| matches!(o.kind, OperationKind::CombatObjective { objective: Goal::ProtectEvacuation, .. })).unwrap().id;
        let pos = w.operations[&id].kind.target_pos(&[], w.hub);
        let f = fleet(&mut w, p, ShipKind::Raider, pos); assign(&mut w, p, id, f);
        let liner = w.pirate_campaign.missions[&id].evacuees.unwrap();
        assert!(matches!(w.fleets[&liner].order, FleetOrder::Idle));
        w.tick_combat_objectives(&mut vec![]);
        assert!(matches!(w.fleets[&liner].order, FleetOrder::MoveTo { dest } if dest == w.hub));
        assert!(matches!(w.fleets[&f].order, FleetOrder::Guard { target } if target == liner));
        assert!(w.contract_guard_authorized(p, liner));
        assert!(!w.contract_guard_authorized(PlayerId(999), liner));
        assert_eq!(w.fleets[&liner].pos, pos, "dispatch cannot teleport a passenger hull");
        for _ in 0..60 { w.time += DT; w.integrate_movement(&mut vec![]); }
        assert!(w.fleets[&liner].pos.distance(pos) > 1.0);
        assert_eq!(w.operations[&id].state, OperationState::Active);
        w.fleets.remove(&f); w.fleets.remove(&liner); w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].state, OperationState::Failed);
    }

    #[test]
    fn blockade_relief_is_a_hold_objective_not_a_kill_counter() {
        let (mut w, p) = scene();
        let site = w.players[&p].home_system.unwrap(); let pos = w.players[&p].home;
        let id = offer(&mut w, p, Goal::BreakBlockade, site, pos);
        let f = fleet(&mut w, p, ShipKind::Raider, pos); assign(&mut w, p, id, f);
        w.tick_combat_objectives(&mut vec![]); w.time += 20.1; w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert_eq!(w.operations[&id].contributions.get(&p).map_or(0, |c| c.combat), 0);
    }

    #[test]
    fn mission_withdrawal_keeps_allies_fighting_and_preserves_the_departing_hull() {
        let (mut w, p) = scene();
        let base = *w.enclaves.keys().next().unwrap();
        let pos = w.systems.iter().find(|s| s.id == base).unwrap().pos;
        let f = fleet(&mut w, p, ShipKind::Raider, pos);
        let ally = fleet(&mut w, p, ShipKind::Raider, pos);
        let enemy = fleet(&mut w, PlayerId::PIRATE, ShipKind::Convoy, pos);
        w.fleets.get_mut(&f).unwrap().ships[0].hp *= 0.45;
        let hp = w.fleets[&f].ships[0].hp;
        w.open_pirate_assault(f, base, pos, vec![enemy], 0, w.time);
        let eid = *w.engagements.keys().next_back().unwrap();
        w.engagements.get_mut(&eid).unwrap().attackers.push(ally);
        w.engagements.get_mut(&eid).unwrap().a_start = w.side_comp(&[f, ally]);
        let mut tac = crate::tactical::TacticalState::open_current(11, eid.0,
            &w.side_ships(&[f, ally]), &w.side_ships(&[enemy]), 0, 0.0, Vec2::new(1.0, 0.0));
        tac.set_missions(BTreeMap::from([(f, MissionProfile { withdrawal: DamageWithdrawal::Hull50, ..Default::default() })]));
        for _ in 0..500 { tac.step(false, [crate::tactical::SideMods { damage_mult: 0.0, ..Default::default() }; 2]); }
        assert_eq!(tac.withdrawn_mission_fleets(), vec![f]);
        w.engagements.get_mut(&eid).unwrap().tactical = Some(tac);
        w.finish_mission_withdrawals();
        let e = &w.engagements[&eid];
        assert_eq!(e.attackers, vec![ally]);
        assert!(e.departed[0].contains_key(&f));
        assert!(matches!(w.fleets[&f].order, FleetOrder::MoveTo { dest } if dest == w.players[&p].home));
        assert_eq!(w.fleets[&f].pos, pos, "return starts at the battle, not home");
        assert_eq!(w.fleets[&f].ships[0].hp, hp);
    }

    #[test]
    fn evacuation_success_uses_the_arrival_report_and_cannot_pay_twice() {
        let (mut w, p) = scene();
        w.offer_campaign_contracts(&mut vec![]);
        let id = w.operations.values().find(|o| matches!(o.kind, OperationKind::CombatObjective { objective: Goal::ProtectEvacuation, .. })).unwrap().id;
        let pos = w.operations[&id].kind.target_pos(&[], w.hub);
        let f = fleet(&mut w, p, ShipKind::Raider, pos); assign(&mut w, p, id, f);
        let liner = w.pirate_campaign.missions[&id].evacuees.unwrap();
        let credits = w.players[&p].credits;
        w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].state, OperationState::Active);
        w.fleets.get_mut(&liner).unwrap().pos = w.hub;
        w.tick_combat_objectives(&mut vec![]);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert_eq!(w.players[&p].credits, credits);
        assert!(!w.fleets.contains_key(&liner));
        let due = w.pending_operation_reports.iter().filter(|r| r.operation == id).map(|r| r.arrive_at).fold(0.0, f64::max);
        w.time = due + 1e-6; w.deliver_operation_reports();
        let paid = w.players[&p].credits;
        assert!(paid > credits);
        w.tick_combat_objectives(&mut vec![]); w.deliver_operation_reports();
        assert_eq!(w.players[&p].credits, paid);
    }

    #[test]
    fn a_supply_strike_stops_replacements_without_erasing_the_garrison() {
        let (mut w, p) = scene();
        let (&base, network) = w.pirate_campaign.bases.iter().find(|(_, b)| b.supports.len() == 2).unwrap();
        let support = network.supports.iter().find(|s| s.objective == Goal::DisableSupply).unwrap().clone();
        for guard in &support.guards { w.fleets.remove(guard); }
        let garrison = w.enclaves[&base].pack.and_then(|id| w.fleets.get(&id)).cloned();
        let id = offer(&mut w, p, Goal::DisableSupply, support.id, support.pos);
        let f = fleet(&mut w, p, ShipKind::Raider, support.pos); assign(&mut w, p, id, f);
        w.tick_combat_objectives(&mut vec![]); w.time += 20.1; w.tick_combat_objectives(&mut vec![]);
        w.time += 500.0; w.tick = 30;
        w.enclaves.get_mut(&base).unwrap().plunder.insert(crate::Commodity::Alloys, 100);
        w.tick_pirate_campaign(&mut vec![]);
        assert!(w.pirate_support_disabled(base, Goal::DisableSupply));
        assert!(w.pirate_campaign.bases[&base].patrol.is_none());
        assert_eq!(w.enclaves[&base].plunder[&crate::Commodity::Alloys], 100);
        if let Some(g) = garrison { assert_eq!(w.fleets[&g.id].ships, g.ships); }
    }
}
