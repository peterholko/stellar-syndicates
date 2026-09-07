//! The small, optional bridge between the first privateer and the wider board.
//! Offers use arrived home reports; accepting never teleports or dispatches a
//! fleet. Encounter actors are ordinary sensor-gated, full-hull pirate fleets.

use super::*;
use crate::cargo::Commodity;
use crate::operation::{FollowUpEncounter, FollowUpKind, OperationBriefing, OperationIssuer,
    OperationKind, OperationReward, OperationScope, OperationState};

impl World {
    pub(super) fn refresh_followup_operations(&mut self, player: PlayerId, events: &mut Vec<Event>) {
        let Some(corp) = self.players.get(&player) else { return; };
        // reward_granted is the ARRIVED privateer result, not the true kill.
        // Old saves with disabled founding already graduated from that chapter.
        if corp.founding.enabled && !corp.founding.reward_granted { return; }
        let Some(home_id) = corp.home_system else { return; };
        let known = self.information.systems_for(self, player);
        let Some(home) = known.iter().find(|s| s.id == home_id && s.owner == Some(player)) else { return; };
        let origin = home.pos;
        let along = (self.hub - origin).normalized();
        let across = Vec2::new(-along.y, along.x);
        let site = origin + along * origin.distance(self.hub).min(60_000.0) * 0.45
            + across * 16_000.0;
        // Pick a product from the best KNOWN local feedstock. No arbitrary
        // advanced-good lottery, and no unseen colony deposit informs the offer.
        let feedstock = home.bodies.iter().flat_map(|body| body.deposits.iter().map(move |d|
            (d.resource, crate::explore::natural_extraction_rate(body, d, home.trait_))))
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(good, _)| good).unwrap_or(Commodity::MetallicOre);
        let (product, chain) = match feedstock {
            Commodity::Biomass => (Commodity::Provisions, "Biomass → Agroplex → Provisions"),
            Commodity::Volatiles => (Commodity::Fuel, "Volatiles → Fuel Refinery → Fuel"),
            Commodity::RareElements | Commodity::Silicates => (Commodity::Electronics,
                "Rare Elements + Silicates → Electronics Fabricator → Electronics"),
            _ => (Commodity::Alloys, "Metallic Ore + Fuel → Smelter → Alloys"),
        };
        for chapter in [FollowUpKind::Escort, FollowUpKind::Salvage, FollowUpKind::Production] {
            // Do not renew on an unseen completion/failure; even the appearance
            // of the next offer would otherwise disclose the previous outcome.
            if self.operations.values().any(|o|
                matches!(o.scope, OperationScope::Private { player: p } if p == player)
                && o.briefing.as_ref().is_some_and(|b| b.follow_up == chapter)
                && (!o.state.terminal() || o.known.get(&player).is_none_or(|k| !k.state.terminal())
                    || self.time < o.completed_at.unwrap_or(o.expires_at) + crate::operation::FOLLOW_UP_COOLDOWN_S)) {
                continue;
            }
            let (issuer, kind, goal, reward, title, difficulty, suitable, summary) = match chapter {
                FollowUpKind::Escort => (OperationIssuer::Authority,
                    OperationKind::FreightEscort { origin, destination: self.hub }, 1,
                    OperationReward { credits: 900.0, captain_xp: 80, authority_standing: 2.0, research_insight: 0.0 },
                    "Guarded market run", "Moderate", "1 healthy Interceptor + 1 Freighter",
                    "Guard a Freighter from home to the Market Hub. Expect a slow two-ship pirate pack.".to_string()),
                FollowUpKind::Salvage => (OperationIssuer::SalvageOffice,
                    OperationKind::RescueSalvage { pos: site, commodity: Commodity::Alloys, units: 24,
                        source_fleet: EntityId(0) }, 24,
                    OperationReward { credits: 600.0, captain_xp: 40, authority_standing: 0.0, research_insight: 15.0 },
                    "Quiet wreck, armed neighbours", "Low · optional combat", "1 Freighter · 24 free cargo; Interceptor optional",
                    "Recover 24 Alloys. A nearby pirate Corvette can be avoided or challenged for combat experience.".to_string()),
                FollowUpKind::Production => (OperationIssuer::Market,
                    OperationKind::MarketDelivery { commodity: product, units: 40 }, 40,
                    OperationReward { credits: 800.0, captain_xp: 40, authority_standing: 1.0, research_insight: 0.0 },
                    "First specialist export", "Low", "1 Freighter · 40 free cargo",
                    format!("Home opportunity: {chain}. Deliver 40 to the Market Hub; selling is separate.")),
            };
            let id = self.insert_operation(issuer, OperationScope::Private { player }, kind, goal,
                reward, self.hub, crate::operation::PRIVATE_OFFER_LIFETIME_S, events);
            let operation = self.operations.get_mut(&id).unwrap();
            operation.briefing = Some(OperationBriefing { follow_up: chapter, title: title.into(),
                difficulty: difficulty.into(), suitable_fleets: suitable.into(), summary });
            operation.encounter = Some(FollowUpEncounter::default());
            // A renewed contract reuses surviving opponents, in place. Neither
            // acceptance nor expiry despawns a visible ship, and repeated offers
            // cannot accumulate another pirate formation every ten minutes.
            let survivors: std::collections::BTreeSet<_> = self.operations.values()
                .filter(|o| matches!(o.scope, OperationScope::Private { player: p } if p == player)
                    && o.briefing.as_ref().is_some_and(|b| b.follow_up == chapter))
                .filter_map(|o| o.encounter.as_ref()).flat_map(|e| e.pirates.iter().copied())
                .filter(|id| self.fleets.contains_key(id)).collect();
            self.operations.get_mut(&id).unwrap().encounter.as_mut().unwrap().pirates
                = survivors.into_iter().collect();
        }
    }

    fn contract_pirates(&mut self, kind: ShipKind, count: u32, pos: Vec2, order: FleetOrder,
        events: &mut Vec<Event>) -> EntityId {
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, PlayerId::PIRATE, kind, pos, order, None);
        fleet.reset_to(kind, count); // full hull; weaker guns, never pre-damage
        fleet.operation_privateer = true;
        self.fleets.insert(id, fleet);
        events.push(Event::new(self.time, EventPayload::ShipSpawned { id, owner: PlayerId::PIRATE, kind }));
        id
    }

    pub(super) fn tick_followup_encounters(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<_> = self.operations.values().filter(|o| o.briefing.is_some()
            && o.state == OperationState::Active && self.time <= o.expires_at).map(|o| o.id).collect();
        for id in ids {
            let op = self.operations[&id].clone();
            let OperationScope::Private { player } = op.scope else { continue; };
            let Some(assigned) = op.assigned_fleets.get(&player).copied() else { continue; };
            let Some(ship) = self.fleets.get(&assigned).filter(|f| f.owner == player) else { continue; };
            let encounter = op.encounter.unwrap_or_default();
            match op.kind {
                OperationKind::FreightEscort { origin, destination } => {
                    let Some(charge_id) = encounter.protected_fleet else { continue; };
                    let Some(charge) = self.fleets.get(&charge_id).filter(|f| f.owner == player) else { continue; };
                    let charge_pos = charge.pos;
                    let escort_near = ship.pos.distance(charge_pos) <= crate::operation::ESCORT_RADIUS;
                    let guarding = matches!(ship.order, FleetOrder::Guard { target } if target == charge_id)
                        || ship.defense.as_ref().is_some_and(|d| d.guard == Some(charge_id));
                    if !encounter.staged_at_home && charge_pos.distance(origin) <= 1_200.0
                        && escort_near && guarding {
                        self.operations.get_mut(&id).unwrap().encounter.as_mut().unwrap().staged_at_home = true;
                    }
                    if encounter.staged_at_home && !encounter.launched
                        && charge_pos.distance(origin) >= 5_000.0 {
                        let along = (destination - charge_pos).normalized();
                        let across = Vec2::new(-along.y, along.x);
                        let pos = charge_pos + along * charge_pos.distance(destination).min(50_000.0) * 0.5
                            + across * 20_000.0;
                        let pirate = if let Some(existing) = encounter.pirates.iter()
                            .find(|id| self.fleets.contains_key(id)).copied() {
                            self.fleets.get_mut(&existing).unwrap().order = FleetOrder::Intercept { target: charge_id };
                            existing
                        } else {
                            self.contract_pirates(ShipKind::Raider, 2, pos,
                                FleetOrder::Intercept { target: charge_id }, events)
                        };
                        let e = self.operations.get_mut(&id).unwrap().encounter.as_mut().unwrap();
                        e.launched = true;
                        e.pirates = vec![pirate];
                    }
                    // A stationary ship already at the hub cannot cash an escort.
                    // It must stage at home, physically depart, and arrive alive
                    // alongside its actual escort. Combat is not a mandatory kill.
                    if encounter.launched && escort_near && guarding
                        && charge_pos.distance(destination) <= crate::ship::DOCK_RADIUS {
                        self.operations.get_mut(&id).unwrap().contribution_mut(player).escort += 100;
                        self.complete_operation(id, Some(player), destination, events);
                    }
                }
                OperationKind::RescueSalvage { pos, .. } => {
                    if !encounter.launched && ship.pos.distance(pos) <= 25_000.0 {
                        // The optional opponent holds OFF the salvage approach.
                        // It does not intercept the cargo ship or gate recovery.
                        let approach = (pos - self.players[&player].home).normalized();
                        let pirate = encounter.pirates.iter().find(|id| self.fleets.contains_key(id))
                            .copied().unwrap_or_else(|| self.contract_pirates(ShipKind::Corvette, 1,
                                pos + Vec2::new(-approach.y, approach.x) * 14_000.0,
                                FleetOrder::Idle, events));
                        let e = self.operations.get_mut(&id).unwrap().encounter.as_mut().unwrap();
                        e.launched = true;
                        e.pirates = vec![pirate];
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (World, PlayerId) {
        let mut world = World::new(SimConfig::for_players(123, 4));
        let player = PlayerId(91_001);
        world.step(&[Command::AddPlayer { id: player, name: "Follow-ups".into() }]);
        (world, player)
    }

    fn unlock(world: &mut World, player: PlayerId) {
        world.players.get_mut(&player).unwrap().founding.reward_granted = true;
        world.refresh_followup_operations(player, &mut Vec::new());
        world.time = world.pending_operation_reports.iter().map(|r| r.arrive_at)
            .fold(world.time, f64::max);
        world.deliver_operation_reports();
    }

    fn job(world: &World, kind: FollowUpKind) -> OperationId {
        world.operations.values().find(|o| o.briefing.as_ref()
            .is_some_and(|b| b.follow_up == kind)).unwrap().id
    }

    fn hull(world: &mut World, player: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = world.alloc_entity_id();
        world.fleets.insert(id, Fleet::single(id, player, kind, pos, FleetOrder::Idle, None));
        id
    }

    #[test]
    fn followups_wait_for_defeat_news_and_offer_reports() {
        let (mut w, p) = setup();
        w.refresh_followup_operations(p, &mut Vec::new());
        assert!(w.operations.values().all(|o| o.briefing.is_none()),
            "true combat progress alone must not unlock the board");
        w.players.get_mut(&p).unwrap().founding.reward_granted = true;
        w.refresh_followup_operations(p, &mut Vec::new());
        assert_eq!(w.operations.values().filter(|o| o.briefing.is_some()).count(), 3);
        for o in w.operations.values().filter(|o| o.briefing.is_some()) {
            assert!(!o.is_visible_to(p), "hub's offer is still in flight");
            assert!(o.reward.credits > 0.0);
            assert!(!o.briefing.as_ref().unwrap().suitable_fleets.is_empty());
        }
        assert!(!w.fleets.values().any(|f| f.operation_privateer));
        unlock(&mut w, p);
        w.refresh_followup_operations(p, &mut Vec::new());
        assert_eq!(w.operations.values().filter(|o| o.briefing.is_some()).count(), 3);
        let serialized = serde_json::to_string(&w).unwrap();
        let mut restored: World = serde_json::from_str(&serialized).unwrap();
        restored.refresh_followup_operations(p, &mut Vec::new());
        assert_eq!(restored.operations.len(), w.operations.len(), "restart must not duplicate offers");
    }

    #[test]
    fn escort_requires_a_real_guarded_departure_and_completion_light() {
        let (mut w, p) = setup();
        unlock(&mut w, p);
        let id = job(&w, FollowUpKind::Escort);
        let origin = w.players[&p].home;
        let freighter = hull(&mut w, p, ShipKind::Convoy, origin);
        let guard = hull(&mut w, p, ShipKind::Raider, origin);
        let stranger = hull(&mut w, PlayerId(999), ShipKind::Convoy, origin);
        w.apply_accept_operation(p, id);
        w.apply_assign_operation_fleet(p, id, guard, Some(stranger));
        assert!(w.operations[&id].assigned_fleets.is_empty(), "cannot bind a rival freighter");
        w.apply_assign_operation_fleet(p, id, guard, Some(freighter));
        let mut events = Vec::new();
        w.tick_followup_encounters(&mut events);
        assert!(!w.operations[&id].encounter.as_ref().unwrap().staged_at_home);
        w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: freighter };
        w.tick_followup_encounters(&mut events);
        assert!(w.operations[&id].encounter.as_ref().unwrap().staged_at_home);
        let departed = origin + (w.hub - origin).normalized() * 6_000.0;
        for fid in [guard, freighter] { w.fleets.get_mut(&fid).unwrap().pos = departed; }
        w.tick_followup_encounters(&mut events);
        let pirate = w.operations[&id].encounter.as_ref().unwrap().pirates[0];
        assert_eq!(w.fleets[&pirate].count(ShipKind::Raider), 2);
        assert!(w.fleets[&pirate].ships.iter().all(|s| s.hp == s.max_hp()));
        assert_eq!(w.combat_damage_mult_for(PlayerId::PIRATE, &[pirate]),
            crate::operation::FOLLOW_UP_PIRATE_DAMAGE_MULT);
        assert!(w.fleets[&pirate].pos.distance(departed) > 20_000.0, "time to react, not a point-blank spawn");
        assert_ne!(w.operations[&id].state, OperationState::Completed);
        for fid in [guard, freighter] { w.fleets.get_mut(&fid).unwrap().pos = w.hub; }
        let before = w.players[&p].credits;
        w.tick_followup_encounters(&mut events);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert_ne!(w.operations[&id].known[&p].state, OperationState::Completed);
        assert_eq!(w.players[&p].credits, before, "physical success isn't an immediate payout");
        w.time = w.pending_operation_reports.iter().map(|r| r.arrive_at).fold(w.time, f64::max);
        w.deliver_operation_reports();
        assert_eq!(w.players[&p].credits, before + 900.0);
        // Renewing cannot create an ever-growing crowd of surviving pirates.
        w.time += crate::operation::FOLLOW_UP_COOLDOWN_S + 1.0;
        w.refresh_followup_operations(p, &mut events);
        let renewed = w.operations.values().filter(|o| o.briefing.as_ref()
            .is_some_and(|b| b.follow_up == FollowUpKind::Escort)).max_by_key(|o| o.id).unwrap();
        assert_eq!(renewed.encounter.as_ref().unwrap().pirates, vec![pirate]);
    }

    #[test]
    fn salvage_is_optional_combat_and_recovers_only_at_the_site_with_room() {
        let (mut w, p) = setup();
        unlock(&mut w, p);
        let id = job(&w, FollowUpKind::Salvage);
        let OperationKind::RescueSalvage { pos, .. } = w.operations[&id].kind else { panic!() };
        let origin = w.players[&p].home;
        let cargo = hull(&mut w, p, ShipKind::Convoy, origin);
        w.apply_accept_operation(p, id);
        w.apply_assign_operation_fleet(p, id, cargo, None);
        let mut events = Vec::new();
        w.tick_operations(&mut events);
        assert_eq!(w.operations[&id].progress, 0);
        w.fleets.get_mut(&cargo).unwrap().pos = pos;
        let cap = w.fleets[&cargo].cargo_capacity();
        w.fleets.get_mut(&cargo).unwrap().add_cargo(Commodity::Provisions, cap);
        w.tick_operations(&mut events);
        assert_eq!(w.operations[&id].progress, 0, "a full hold cannot recover wreck cargo");
        let pirate = w.operations[&id].encounter.as_ref().unwrap().pirates[0];
        assert!(matches!(w.fleets[&pirate].order, FleetOrder::Idle));
        assert!(w.fleets[&pirate].pos.distance(pos) >= 14_000.0 - 1e-6);
        w.fleets.get_mut(&cargo).unwrap().take_cargo();
        w.tick_operations(&mut events);
        assert_eq!(w.fleets[&cargo].cargo_amount(Commodity::Alloys), 24);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert!(w.fleets.contains_key(&pirate), "salvage never requires the optional kill");
    }

    #[test]
    fn specialist_export_counts_physical_delivery_not_a_sale_or_double_receipt() {
        let (mut w, p) = setup();
        unlock(&mut w, p);
        let id = job(&w, FollowUpKind::Production);
        let OperationKind::MarketDelivery { commodity, .. } = w.operations[&id].kind else { panic!() };
        let sale = Event::new(w.time, EventPayload::Trade(TradeEvent::Sold {
            player: p, commodity, units: 40, unit_price: 100.0, penalty: 0.0,
        }));
        let delivered = |units| Event::new(w.time, EventPayload::Trade(TradeEvent::Delivered {
            player: p, commodity, units, system: None,
        }));
        let half = delivered(20);
        let mut events = Vec::new();
        w.process_operation_events(&[half.clone()], &mut events);
        assert_eq!(w.operations[&id].progress, 0, "offers do not score before acceptance");
        w.apply_accept_operation(p, id);
        w.process_operation_events(&[sale.clone(), half.clone(), sale], &mut events);
        assert_eq!(w.operations[&id].progress, 20, "unloading and selling the same cargo counts once");
        w.process_operation_events(&[half], &mut events);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert_ne!(w.operations[&id].known[&p].state, OperationState::Completed);
    }

    #[test]
    fn export_briefing_uses_received_home_resources_not_unreported_changes() {
        let (mut w, p) = setup();
        unlock(&mut w, p);
        let before = w.operations[&job(&w, FollowUpKind::Production)].briefing.clone().unwrap().summary;
        let home = w.players[&p].home_system.unwrap();
        for body in &mut w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies {
            for deposit in &mut body.deposits {
                deposit.resource = Commodity::Volatiles;
                deposit.richness = 1000.0;
            }
        }
        w.operations.clear();
        w.pending_operation_reports.clear();
        w.refresh_followup_operations(p, &mut Vec::new());
        assert_eq!(w.operations[&job(&w, FollowUpKind::Production)].briefing.as_ref().unwrap().summary, before);
    }
}
