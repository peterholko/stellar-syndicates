//! Home defense becomes a reason to scout, refit, and go after the source.
//! Do not mutate an already-visible operation's kind/briefing: those are public
//! terms, not part of KnownOperation. Separate, immutable offers ensure the next
//! stage and its intelligence cannot appear before the report that earned it.

use super::*;
use crate::operation::{CounterRaid, FollowUpKind, OperationBriefing, OperationIssuer,
    OperationKind, OperationReward, OperationScope};

/// Give an asynchronous commander time to repair, scout and assemble a fleet.
const COUNTER_RAID_LIFETIME_S: f64 = 24.0 * 60.0 * 60.0;
const COUNTER_RAID_BOUNTY_PER_TIER: f64 = 400.0;

impl World {
    pub(super) fn offer_counter_raid(&mut self, fleet: EntityId, origin: Vec2, events: &mut Vec<Event>) {
        let Some(raid) = self.pirate_home_raids.get(&fleet) else { return; };
        if raid.counter_raid_offered || self.time < raid.depart_at { return; }
        let (player, link) = (raid.owner, CounterRaid { raid: fleet, home: raid.system, source: raid.base });
        self.pirate_home_raids.get_mut(&fleet).unwrap().counter_raid_offered = true;
        // Deduplicate even offers whose light is still in flight. Never test
        // unseen source survival here: this lead comes from the pirates' earlier
        // deliberate warning transmission, not omniscient enemy tracking.
        if self.operations.values().any(|o| !o.state.terminal()
            && matches!(o.scope, OperationScope::Private { player: p } if p == player)
            && o.counter_raid.is_some_and(|c| c.source == link.source)) { return; }
        let id = self.insert_operation(OperationIssuer::SurveyOffice, OperationScope::Private { player },
            OperationKind::SurveyExpedition { system: link.source }, 1,
            OperationReward { captain_xp: 30, research_insight: 20.0, ..Default::default() },
            origin, COUNTER_RAID_LIFETIME_S, events);
        let o = self.operations.get_mut(&id).unwrap();
        o.counter_raid = Some(link);
        o.briefing = Some(OperationBriefing { follow_up: FollowUpKind::CounterRaidTrace,
            title: "1/3 · Trace the home raiders".into(), difficulty: "Reconnaissance".into(),
            suitable_fleets: "Scout".into(), variant: 0,
            summary: "Survey the source of their warning transmission. Identify the hideout before committing warships.".into() });
    }

    pub(super) fn advance_counter_raid(&mut self, id: OperationId, origin: Vec2, events: &mut Vec<Event>) {
        let o = &self.operations[&id];
        let Some(link) = o.counter_raid else { return; };
        if !matches!(o.kind, OperationKind::SurveyExpedition { .. }) { return; }
        let OperationScope::Private { player } = o.scope else { return; };
        // Called only on physical survey completion. Freeze exactly the on-site
        // observation NOW; later views must never consult a newer garrison.
        let Some(site) = self.systems.iter().find(|s| s.id == link.source) else { return; };
        let pos = site.pos;
        let (active, tier, pack) = self.enclaves.get(&link.source)
            .map(|e| (e.active(self.time), e.tier, e.pack)).unwrap_or((false, 0, None));
        let defense = if active { site.tier_sum(crate::build::StructureKind::DefensePlatform) } else { 0 };
        let ships = if active { pack.and_then(|f| self.fleets.get(&f))
            .filter(|f| f.pos.distance(pos) <= pirate::PIRATE_ASSAULT_RADIUS)
            .map_or(0, Fleet::total_count) } else { 0 };
        // The Scout's ordinary gather_intel path handles map intelligence.
        // This immutable briefing adds no replacement/shorter map report leg.
        let bounty = if active { COUNTER_RAID_BOUNTY_PER_TIER * defense.max(1) as f64 } else { 0.0 };
        let next = self.insert_operation(OperationIssuer::Authority, OperationScope::Private { player },
            OperationKind::PirateBounty { system: link.source, tier }, 1,
            OperationReward { credits: bounty, captain_xp: if active { 90 } else { 0 }, ..Default::default() },
            origin, COUNTER_RAID_LIFETIME_S, events);
        let o = self.operations.get_mut(&next).unwrap();
        o.counter_raid = Some(link);
        o.briefing = Some(OperationBriefing { follow_up: FollowUpKind::CounterRaidAssault,
            title: if active { "2/3 · Strike the raiders' hideout" } else { "Hideout already suppressed" }.into(),
            difficulty: if defense >= 3 { "High · fortified" } else { "Moderate · fortified" }.into(),
            suitable_fleets: if defense >= 3 { "Destroyer-led fleet + screening Corvettes; repair first" }
                else { "Several fitted Interceptors + a screening Corvette; repair first" }.into(), variant: 0,
            summary: if active { format!("Scouted: {defense} defense tiers, {ships} ships on-site. Clear the hideout to stop its raids for {}m. Bring a Freighter for the loot.",
                (pirate::PIRATE_DORMANCY / 60.0).round()) }
                else { "The survey found no active hideout. No assault needed.".into() } });
        if !active {
            // Close the debrief on the SAME survey wavefront. No fabricated
            // bounty, and no stale assault destination that silently does nothing.
            self.complete_operation(next, Some(player), origin, events);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind::DefensePlatform;
    use crate::cargo::Commodity;
    use crate::operation::OperationState;

    fn scene() -> (World, PlayerId, EntityId, EntityId, Vec2) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let player = PlayerId(95103);
        w.step(&[Command::AddPlayer { id: player, name: "Counterattack".into() }]);
        w.time = pirate::HOME_RAID_GRACE_S + 1.0;
        w.tick = (w.time / DT) as u64;
        w.players.get_mut(&player).unwrap().founding.enabled = false;
        let home = w.players[&player].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile
            .insert(Commodity::MetallicOre, 500.0);
        let source = *w.enclaves.iter().find(|(_, e)| !pirate::permanent_site(e.tier)).unwrap().0;
        w.enclaves.retain(|id, _| *id == source);
        w.fleets.clear();
        let pos = w.players[&player].home + Vec2::new(40_000.0, 0.0);
        let sys = w.systems.iter_mut().find(|s| s.id == source).unwrap();
        sys.pos = pos;
        sys.set_tier(DefensePlatform, pirate::base_defense_tiers(1));
        let pack = w.spawn_pirate_pack(1, pos);
        let e = w.enclaves.get_mut(&source).unwrap();
        e.pack = Some(pack);
        e.tier = 1;
        e.plunder = BTreeMap::from([(Commodity::Electronics, 11)]);
        e.next_grow_at = w.time + 1e9;
        assert!(w.try_launch_home_raid(source, pack, &mut Vec::new()));
        w.time = w.pirate_home_raids[&pack].depart_at + 1.0;
        (w, player, source, pack, pos)
    }

    fn arrive(w: &mut World) {
        w.time = w.pending_operation_reports.iter().map(|r| r.arrive_at).fold(w.time, f64::max);
        w.deliver_operation_reports();
    }

    fn stage(w: &World, follow_up: FollowUpKind) -> OperationId {
        w.operations.values().find(|o| o.briefing.as_ref().is_some_and(|b| b.follow_up == follow_up)).unwrap().id
    }

    fn survey(w: &mut World, player: PlayerId, source: EntityId, pos: Vec2) -> EntityId {
        let scout = w.alloc_entity_id();
        w.fleets.insert(scout, Fleet::single(scout, player, ShipKind::Scout, pos,
            FleetOrder::Survey { system: source, station: pos, dwell_since: Some(w.time - crate::explore::SURVEY_SECS - 1.0) }, None));
        let mut events = Vec::new();
        w.resolve_surveys(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SurveyCompleted { .. })));
        w.process_operation_events(&events.clone(), &mut events);
        scout
    }

    #[test]
    fn counter_raid_stages_wait_for_light_and_loot_needs_a_real_freighter() {
        let (mut w, player, source, pack, pos) = scene();
        let lead_origin = w.players[&player].home + Vec2::new(6_000.0, 0.0);
        w.offer_counter_raid(pack, lead_origin, &mut Vec::new());
        let trace = stage(&w, FollowUpKind::CounterRaidTrace);
        assert!(!w.operations[&trace].is_visible_to(player));
        assert_eq!(w.operations.len(), 1, "a warning lead must not expose defenses or loot");
        arrive(&mut w);
        w.apply_accept_operation(player, trace);
        survey(&mut w, player, source, pos);
        let assault = stage(&w, FollowUpKind::CounterRaidAssault);
        assert_eq!(w.operations[&trace].known[&player].state, OperationState::Active);
        assert!(!w.operations[&assault].is_visible_to(player));
        let report_times: Vec<_> = w.pending_operation_reports.iter()
            .filter(|r| r.operation == trace || r.operation == assault).map(|r| r.arrive_at).collect();
        assert!(report_times.iter().all(|t| (*t - report_times[0]).abs() < 1e-9), "one survey wavefront");
        let terms = serde_json::to_value(&w.operations[&assault].briefing).unwrap();
        // New truth cannot rewrite already-priced scout terms while in flight.
        w.fleets.get_mut(&pack).unwrap().add(ShipKind::Raider, 2);
        assert_eq!(serde_json::to_value(&w.operations[&assault].briefing).unwrap(), terms);
        arrive(&mut w);
        assert_eq!(w.operations[&trace].known[&player].state, OperationState::Completed);
        w.apply_accept_operation(player, assault);
        let credits = w.players[&player].credits;
        let home = w.players[&player].home_system.unwrap();
        let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        let winner = w.alloc_entity_id();
        w.fleets.insert(winner, Fleet::single(winner, player, ShipKind::Destroyer, pos, FleetOrder::Idle, None));
        // Feed the actual combat end state into the ordinary clearance pass.
        w.fleets.remove(&pack);
        w.systems.iter_mut().find(|s| s.id == source).unwrap().set_tier(DefensePlatform, 0);
        let cleared_at = w.time;
        let mut events = Vec::new();
        w.pirate_ai(&mut events);
        w.process_operation_events(&events.clone(), &mut events);
        assert_eq!(w.enclaves[&source].dormant_until, cleared_at + pirate::PIRATE_DORMANCY);
        assert_eq!(w.players[&player].credits, credits, "no early bounty");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock, "no cargo teleport");
        assert_eq!(w.operations[&assault].known[&player].state, OperationState::Active);
        let loot: Vec<_> = w.operations.values().filter(|o| o.briefing.as_ref()
            .is_some_and(|b| b.follow_up == FollowUpKind::CounterRaidRecovery)).map(|o| o.id).collect();
        assert_eq!(loot.len(), 2, "stolen Electronics plus finite platform wreckage");
        assert!(loot.iter().all(|id| !w.operations[id].is_visible_to(player)));
        w.pirate_ai(&mut Vec::new());
        assert_eq!(w.operations.len(), 4, "one clearance cannot duplicate loot");
        // Save mid-wave: neither rewards nor the chain restart on restore.
        w = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        arrive(&mut w);
        assert_eq!(w.players[&player].credits, credits + w.operations[&assault].reward.credits);
        let paid = w.players[&player].credits;
        w.deliver_operation_reports();
        assert_eq!(w.players[&player].credits, paid);
        let electronics = *loot.iter().find(|id| matches!(w.operations[id].kind,
            OperationKind::RescueSalvage { commodity: Commodity::Electronics, .. })).unwrap();
        w.apply_accept_operation(player, electronics);
        let freighter = w.alloc_entity_id();
        w.fleets.insert(freighter, Fleet::single(freighter, player, ShipKind::Convoy,
            w.players[&player].home, FleetOrder::Idle, None));
        w.apply_assign_operation_fleet(player, electronics, freighter, None);
        w.tick_operations(&mut Vec::new());
        assert_eq!(w.fleets[&freighter].cargo_units(), 0, "remote assignment is not recovery");
        w.fleets.get_mut(&freighter).unwrap().pos = pos;
        let hulls = w.fleets.len();
        w.tick_operations(&mut Vec::new());
        assert_eq!(w.fleets.len(), hulls, "real site loot must not spawn an authored salvage encounter");
        assert_eq!(w.fleets[&freighter].cargo_amount(Commodity::Electronics), 11);
        assert_eq!(w.operations[&electronics].known[&player].state, OperationState::Active);
        arrive(&mut w);
        assert_eq!(w.operations[&electronics].known[&player].state, OperationState::Completed);
        assert!(!w.enclaves[&source].active(cleared_at + pirate::PIRATE_DORMANCY - 0.01));
        assert!(w.enclaves[&source].active(cleared_at + pirate::PIRATE_DORMANCY));
    }

    #[test]
    fn counter_raid_deduplicates_and_does_not_claim_unseen_source_survival() {
        let (mut w, player, source, pack, pos) = scene();
        w.offer_counter_raid(pack, pos, &mut Vec::new());
        w.offer_counter_raid(pack, pos, &mut Vec::new());
        assert_eq!(w.operations.len(), 1);
        w = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        w.offer_counter_raid(pack, pos, &mut Vec::new());
        assert_eq!(w.operations.len(), 1, "save/load never reposts the lead");
        let trace = stage(&w, FollowUpKind::CounterRaidTrace);
        w.enclaves.get_mut(&source).unwrap().dormant_until = w.time + 1000.0;
        arrive(&mut w);
        assert_eq!(w.operations[&trace].known[&player].state, OperationState::Offered,
            "an unseen clearance must not erase a transmission lead");
        w.apply_accept_operation(player, trace);
        let credits = w.players[&player].credits;
        survey(&mut w, player, source, pos);
        let debrief = stage(&w, FollowUpKind::CounterRaidAssault);
        assert!(!w.operations[&debrief].is_visible_to(player));
        arrive(&mut w);
        assert_eq!(w.operations[&debrief].known[&player].state, OperationState::Completed);
        assert_eq!(w.players[&player].credits, credits, "already cleared is not a bounty exploit");
        assert!(w.operations[&debrief].briefing.as_ref().unwrap().summary.contains("No assault needed"));
    }

    #[test]
    fn a_destroyed_reinforcing_raider_keeps_its_counterattack_lead() {
        let (mut w, player, _source, pack, _) = scene();
        let pos = w.players[&player].home + Vec2::new(3000.0, 0.0);
        w.fleets.get_mut(&pack).unwrap().pos = pos;
        let other = w.spawn_pirate_pack(1, pos);
        let guard = w.alloc_entity_id();
        let mut defense = Fleet::single(guard, player, ShipKind::Destroyer, pos,
            FleetOrder::Attack { target: other }, None);
        defense.add(ShipKind::Destroyer, 3);
        w.fleets.insert(guard, defense);
        for id in [pack, other] {
            for hull in &mut w.fleets.get_mut(&id).unwrap().ships { hull.hp = 0.1; }
        }
        for _ in 0..3000 {
            w.resolve_raids(&mut Vec::new());
            if !w.fleets.contains_key(&pack) { break; }
            w.time += DT;
            w.tick += 1;
        }
        assert!(!w.fleets.contains_key(&pack));
        let trace = stage(&w, FollowUpKind::CounterRaidTrace);
        assert!(w.pirate_home_raids[&pack].counter_raid_offered);
        assert_eq!(w.operations[&trace].counter_raid.unwrap().raid, pack);
        assert!(!w.operations[&trace].is_visible_to(player));
        let report = w.pending_operation_reports.iter().find(|r| r.operation == trace).unwrap();
        assert!((report.arrive_at - report.snapshot.reported_at
            - crate::transit::delay(pos, w.players[&player].command_center, w.config.c)).abs() < 1e-8);
    }

    #[test]
    fn another_players_clearance_closes_a_stale_assault_only_with_new_light() {
        let (mut w, player, source, pack, pos) = scene();
        w.offer_counter_raid(pack, pos, &mut Vec::new());
        arrive(&mut w);
        let trace = stage(&w, FollowUpKind::CounterRaidTrace);
        w.apply_accept_operation(player, trace);
        survey(&mut w, player, source, pos);
        arrive(&mut w);
        let assault = stage(&w, FollowUpKind::CounterRaidAssault);
        let before = w.players[&player].credits;
        w.enclaves.get_mut(&source).unwrap().dormant_until = w.time + pirate::PIRATE_DORMANCY;
        w.tick_operations(&mut Vec::new());
        assert_eq!(w.operations[&assault].state, OperationState::Failed);
        assert_eq!(w.operations[&assault].known[&player].state, OperationState::Offered);
        arrive(&mut w);
        assert_eq!(w.operations[&assault].known[&player].state, OperationState::Failed);
        assert_eq!(w.players[&player].credits, before);
    }

    #[test]
    fn retreating_home_raider_posts_the_lead_from_its_actual_position() {
        let (mut w, player, _source, pack, _) = scene();
        let pos = w.players[&player].home;
        w.fleets.get_mut(&pack).unwrap().pos = pos;
        w.pirate_home_raids.get_mut(&pack).unwrap().arrived_at = Some(w.time
            - pirate::HOME_RAID_HOLD_BATTLE_MULT * w.config.battle_target_secs - 1.0);
        w.tick_pirate_home_raids(&mut Vec::new());
        assert!(w.pirate_home_raids[&pack].returning);
        let trace = stage(&w, FollowUpKind::CounterRaidTrace);
        let report = w.pending_operation_reports.iter().find(|r| r.operation == trace).unwrap();
        assert_eq!(report.arrive_at, w.time + crate::transit::delay(pos, w.players[&player].command_center, w.config.c));
        w.tick_pirate_home_raids(&mut Vec::new());
        assert_eq!(w.operations.len(), 1);
    }
}
