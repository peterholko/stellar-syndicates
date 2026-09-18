//! Distinctive pirate prizes use the existing recovery, shipping and research
//! clocks. No expedition planner, remote inventory grant or combat-stat tier.
use super::*;
use crate::operation::{FollowUpKind, OperationBriefing, OperationIssuer, OperationKind,
    OperationReward, OperationScope, OperationState};
use crate::pirate::SitePrize;

impl World {
    pub(super) fn pirate_prize_briefing(&self, player: PlayerId, system: EntityId, tier: u32) -> OperationBriefing {
        let mut briefing = pirate_progression::site_briefing(tier);
        // Geology is static, but not automatically known with a fortification
        // sighting. Publish this prospect only from the corporation's survey;
        // the immutable briefing then rides the site's normal delayed report.
        if self.players.get(&player).is_some_and(|c| c.surveyed.contains(&system))
            && let Some(sys) = self.systems.iter().find(|s| s.id == system)
            && let Some((body, deposit)) = sys.bodies.iter().flat_map(|b| b.deposits.iter()
                .filter(|d| d.resource.is_mineable_mineral())
                .map(move |d| (b, d)))
                .max_by(|(a, da), (b, db)| crate::explore::natural_extraction_rate(a, da, sys.trait_)
                    .total_cmp(&crate::explore::natural_extraction_rate(b, db, sys.trait_))) {
            briefing.summary.push_str(&format!(" Settlement prospect: {} · {}.", body.name, deposit.resource.display_name()));
        }
        briefing
    }

    pub(super) fn offer_pirate_prize(&mut self, player: PlayerId, system: EntityId,
        tier: u32, pos: Vec2, events: &mut Vec<Event>) {
        let Some(prize) = SitePrize::for_tier(tier) else { return; };
        // The permanent site's cleared bit prevents re-minting after reload.
        // Keep the operation-level guard too: abandoned/expired caches are not
        // new stock, and another corporation cannot receive a second copy.
        if self.operations.values().any(|o| matches!(o.kind,
            OperationKind::PrizeRecovery { system: s, .. } if s == system)) { return; }
        let id = self.insert_operation(OperationIssuer::SalvageOffice, OperationScope::Private { player },
            OperationKind::PrizeRecovery { pos, system, prize }, 1, OperationReward::default(),
            pos, 24.0 * 60.0 * 60.0, events);
        self.operations.get_mut(&id).unwrap().briefing = Some(OperationBriefing {
            follow_up: FollowUpKind::SiteRecovery, title: format!("Recover {}", prize.title()),
            difficulty: "Equipment recovery".into(), suitable_fleets: "Freighter".into(), variant: 0,
            summary: format!("{} Dock at an owned system to store the modules. Data radios home after recovery; research gates still apply.", prize.summary()),
        });
    }

    pub(super) fn recover_pirate_prize(&mut self, player: PlayerId, id: OperationId,
        fleet_id: EntityId, events: &mut Vec<Event>) {
        let Some((pos, prize)) = self.operations.get(&id).and_then(|o| {
            if o.state != OperationState::Active || !o.participants.contains(&player) { return None; }
            match o.kind { OperationKind::PrizeRecovery { pos, prize, .. } => Some((pos, prize)), _ => None }
        }) else { return; };
        if self.engagements.values().any(|e| e.attackers.contains(&fleet_id) || e.defenders.contains(&fleet_id)) { return; }
        let Some(fleet) = self.fleets.get_mut(&fleet_id) else { return; };
        let modules = prize.modules();
        let needed: u32 = modules.values().sum();
        // Module crates already have their own transport berths, separate from
        // bulk commodity capacity. A cache is atomic; a full Freighter leaves it
        // on-site rather than silently dropping equipment or paying partial data.
        let capacity = fleet.freighter_count().saturating_mul(crate::module::MODULE_CONVOY_BERTHS);
        let aboard: u32 = fleet.modules.values().sum();
        if fleet.owner != player || fleet.pos.distance(pos) > crate::operation::SALVAGE_RECOVERY_RADIUS
            || capacity.saturating_sub(aboard) < needed { return; }
        for (kind, count) in modules { *fleet.modules.entry(kind).or_default() += count; }
        let operation = self.operations.get_mut(&id).unwrap();
        operation.assigned_fleets.insert(player, fleet_id);
        operation.contribution_mut(player).goods += needed;
        operation.contribution_mut(player).progress += 1;
        self.complete_operation(id, Some(player), pos, events);
    }

    /// Physical docking lands recovered crates, just like an existing module
    /// transfer arriving. Never move the hull or create a transport; crates lost
    /// with it stay lost. Remote inventory/manifest changes use ordinary served
    /// snapshots and ModulesDelivered news, not the recovery report's clock.
    pub(super) fn land_recovered_modules(&mut self, events: &mut Vec<Event>) {
        let arrivals: Vec<_> = self.fleets.values().filter(|f| !f.modules.is_empty() && f.mission.is_none())
            .filter_map(|f| match self.dock_of(f.id) {
                Some(DockSite::System(sid)) if self.systems.iter().any(|s| s.id == sid && s.owner == Some(f.owner))
                    => Some((f.id, f.owner, sid)),
                _ => None,
            }).collect();
        for (fleet, owner, system) in arrivals {
            let manifest = std::mem::take(&mut self.fleets.get_mut(&fleet).unwrap().modules);
            let sys = self.systems.iter_mut().find(|s| s.id == system).unwrap();
            for (&kind, &count) in &manifest { *sys.modules.entry(kind).or_default() += count; }
            events.push(Event::new(self.time, EventPayload::ModulesDelivered { owner, system, manifest }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Loadout, ModuleKind};

    fn cleared_site(tier: u32) -> (World, PlayerId, EntityId, Vec2, OperationId) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let player = PlayerId(95_071);
        w.step(&[Command::AddPlayer { id: player, name: "Prize hunters".into() }]);
        let e = w.enclaves.values().find(|e| e.tier == tier).unwrap();
        let (site, pack) = (e.system, e.pack.unwrap());
        let pos = w.systems.iter().find(|s| s.id == site).unwrap().pos;
        // The real clearance path, fed the tactical engine's victory end-state.
        w.fleets.remove(&pack);
        w.systems.iter_mut().find(|s| s.id == site).unwrap().set_tier(crate::build::StructureKind::DefensePlatform, 0);
        fleet(&mut w, player, ShipKind::Cruiser, pos);
        w.pirate_ai(&mut Vec::new());
        let id = w.operations.values().find(|o| matches!(o.kind,
            OperationKind::PrizeRecovery { system, .. } if system == site)).unwrap().id;
        (w, player, site, pos, id)
    }

    fn fleet(w: &mut World, owner: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None));
        id
    }

    fn reports_arrive(w: &mut World) {
        w.time = w.pending_operation_reports.iter().map(|r| r.arrive_at).fold(w.time, f64::max);
        w.deliver_operation_reports();
    }

    #[test]
    fn a_site_prize_is_finite_reported_and_physically_hauled() {
        for (tier, freight_kind) in [pirate::DEPOT_TIER, pirate::STRONGHOLD_TIER].into_iter()
            .flat_map(|tier| crate::ship::PLAYER_FREIGHTERS.map(|kind| (tier, kind))) {
            let (mut w, p, site, pos, id) = cleared_site(tier);
            let prize = SitePrize::for_tier(tier).unwrap();
            let home = w.players[&p].home_system.unwrap();
            let home_pos = w.players[&p].home;
            let before = w.systems.iter().find(|s| s.id == home).unwrap().modules.clone();
            assert!(!w.operations[&id].is_visible_to(p));
            w.apply_accept_operation(p, id);
            assert_eq!(w.operations[&id].state, OperationState::Offered, "unarrived prize cannot be accepted");
            reports_arrive(&mut w);
            w.apply_accept_operation(p, id);
            let hull = fleet(&mut w, p, freight_kind, home_pos);
            w.apply_recover_operation(p, id, hull, &mut Vec::new());
            assert!(w.fleets[&hull].modules.is_empty(), "no remote recovery");
            let fighter = fleet(&mut w, p, ShipKind::Raider, pos);
            w.apply_recover_operation(p, id, fighter, &mut Vec::new());
            assert!(w.fleets[&fighter].modules.is_empty(), "combatants cannot carry the cache");
            let rival = fleet(&mut w, PlayerId(999), ShipKind::Convoy, pos);
            w.apply_recover_operation(PlayerId(999), id, rival, &mut Vec::new());
            w.apply_recover_operation(p, id, rival, &mut Vec::new());
            assert!(w.fleets[&rival].modules.is_empty(), "scope and hull ownership both enforced");
            let f = w.fleets.get_mut(&hull).unwrap();
            f.pos = pos;
            f.modules.insert(ModuleKind::TorpedoRack, crate::module::MODULE_CONVOY_BERTHS);
            w.apply_recover_operation(p, id, hull, &mut Vec::new());
            assert_eq!(w.operations[&id].state, OperationState::Active, "full crate berths leave the cache intact");
            w.fleets.get_mut(&hull).unwrap().modules.clear();
            w.apply_assign_operation_fleet(p, id, hull, None);
            w.tick_operations(&mut Vec::new());
            assert_eq!(w.fleets[&hull].modules, prize.modules(), "assigned Freighter automatically recovers on site");
            assert_eq!(w.operations[&id].state, OperationState::Completed);
            assert!(w.players[&p].research.recovered_data.is_empty(), "pickup isn't CC knowledge");
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules, before);
            w.land_recovered_modules(&mut Vec::new());
            assert_eq!(w.fleets[&hull].modules, prize.modules(), "unowned site is not a home inventory");
            w.apply_recover_operation(p, id, hull, &mut Vec::new());
            assert_eq!(w.fleets[&hull].modules, prize.modules(), "repeated pickup cannot mint another cache");

            let mut saved: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
            saved.fixup_after_load();
            saved.offer_pirate_prize(p, site, tier, pos, &mut Vec::new());
            assert_eq!(saved.operations.values().filter(|o| matches!(o.kind,
                OperationKind::PrizeRecovery { system, .. } if system == site)).count(), 1);
            saved.fleets.get_mut(&hull).unwrap().pos = home_pos;
            let mut events = Vec::new();
            saved.land_recovered_modules(&mut events);
            assert!(saved.fleets[&hull].modules.is_empty());
            assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ModulesDelivered { .. })));
            let deposited = saved.systems.iter().find(|s| s.id == home).unwrap().modules.clone();
            for (kind, n) in prize.modules() { assert_eq!(deposited[&kind], before.get(&kind).copied().unwrap_or(0) + n); }
            saved.land_recovered_modules(&mut events);
            assert_eq!(saved.systems.iter().find(|s| s.id == home).unwrap().modules, deposited);
            assert!(saved.fleets.contains_key(&hull), "docking never consumes the player's Freighter");
        }
    }

    #[test]
    fn recovered_data_waits_for_light_and_stays_with_its_named_technology() {
        let (mut w, p, _, pos, id) = cleared_site(pirate::DEPOT_TIER);
        reports_arrive(&mut w);
        w.apply_accept_operation(p, id);
        let hull = fleet(&mut w, p, ShipKind::Convoy, pos);
        w.players.get_mut(&p).unwrap().research.active = Some("hull_line_iv_destroyer".into());
        w.players.get_mut(&p).unwrap().research.progress = 17.0;
        w.apply_recover_operation(p, id, hull, &mut Vec::new());
        let arrival = w.pending_operation_reports.iter().find(|r| r.operation == id
            && r.snapshot.state == OperationState::Completed).unwrap().arrive_at;
        assert!(arrival > w.time);
        w.time = arrival - 1e-4;
        w.deliver_operation_reports();
        assert!(w.players[&p].research.recovered_data.is_empty());
        w.time = arrival;
        w.deliver_operation_reports();
        let target = SitePrize::BreacherCache.programme();
        let credit = crate::research::cost_of(target) * SitePrize::DOSSIER_FRACTION;
        assert_eq!(w.players[&p].research.recovered_data[target], credit);
        assert_eq!(w.players[&p].research.progress, 17.0, "dossier cannot fund an unrelated programme");
        w.deliver_operation_reports();
        assert_eq!(w.players[&p].research.recovered_data[target], credit);

        let r = &mut w.players.get_mut(&p).unwrap().research;
        r.active = Some(target.into()); r.progress = 0.0;
        w.resolve_research(&mut Vec::new());
        assert_eq!(w.players[&p].research.progress, 0.0, "dossier cannot bypass the Line ladder");
        assert_eq!(w.players[&p].research.recovered_data[target], credit);
        w.players.get_mut(&p).unwrap().research.completed.insert("hull_line_iv_destroyer".into());
        w.resolve_research(&mut Vec::new());
        assert_eq!(w.players[&p].research.progress, credit);
        assert!(!w.players[&p].research.has(target), "the remaining 70% still needs normal work");
        w.players.get_mut(&p).unwrap().research.recover_dossier(target, 0.30);
        w.resolve_research(&mut Vec::new());
        assert_eq!(w.players[&p].research.progress, credit, "duplicate data never stacks");
    }

    #[test]
    fn dossier_credit_never_overflows_and_capital_gates_still_apply() {
        let mut r = crate::research::ResearchState::default();
        let target = SitePrize::ScreenCache.programme();
        r.recover_dossier(target, SitePrize::DOSSIER_FRACTION);
        r.completed.insert("hull_line_v_cruiser".into());
        assert!(!crate::research::is_available(target, &r, &|_| 0.0, 0.0), "Battleship still needs its martial proof");
        r.add_verb(crate::research::Verb::BattlesWon, 25.0);
        assert!(crate::research::is_available(target, &r, &|_| 0.0, 0.0));
        r.active = Some(target.into());
        r.progress = crate::research::cost_of(target) - 1.0;
        r.queue = vec!["hull_line_vii_dreadnought".into()];
        r.apply_recovered_data();
        assert_eq!(r.try_complete().as_deref(), Some(target));
        assert_eq!(r.progress, 0.0, "unused dossier work cannot spill into Dreadnought");
        assert!(r.recovered_data.is_empty());
        r.recover_dossier(target, 0.30);
        assert!(r.recovered_data.is_empty(), "completed technology pays no generic substitute");
    }

    #[test]
    fn destroyed_recovery_freighter_does_not_deliver_its_equipment() {
        let (mut w, p, site, pos, id) = cleared_site(pirate::STRONGHOLD_TIER);
        reports_arrive(&mut w); w.apply_accept_operation(p, id);
        let hull = fleet(&mut w, p, ShipKind::Convoy, pos);
        w.apply_recover_operation(p, id, hull, &mut Vec::new());
        w.fleets.remove(&hull); // ordinary destruction removes its carried modules too
        w.land_recovered_modules(&mut Vec::new());
        let home = w.players[&p].home_system.unwrap();
        assert!(w.systems.iter().find(|s| s.id == home).unwrap().modules.is_empty());
        w.offer_pirate_prize(p, site, pirate::STRONGHOLD_TIER, pos, &mut Vec::new());
        assert_eq!(w.operations.values().filter(|o| matches!(o.kind, OperationKind::PrizeRecovery { .. })).count(), 1);
    }

    #[test]
    fn cache_fits_are_legal_sidegrades_not_new_combat_multipliers() {
        let breacher = Loadout::new(SitePrize::BreacherCache.modules().keys().copied().collect());
        let screen = Loadout::new(SitePrize::ScreenCache.modules().keys().copied().collect());
        assert!(breacher.validate(ShipKind::Destroyer));
        assert!(screen.validate(ShipKind::Corvette));
        assert!(!breacher.has_pd());
        assert!(screen.has_pd());
        assert!(screen.offense().1 < Loadout::default().offense().1);
        assert!(SitePrize::for_tier(3).is_none(), "respawning hideouts cannot farm permanent prizes");
    }

    #[test]
    fn crate_only_return_haul_keeps_the_players_freighter() {
        let (mut w, p, _, _, _) = cleared_site(pirate::DEPOT_TIER);
        let home = w.players[&p].home_system.unwrap();
        let pos = w.players[&p].home;
        let hull = fleet(&mut w, p, ShipKind::Convoy, pos);
        let manifest = SitePrize::BreacherCache.modules();
        w.fleets.get_mut(&hull).unwrap().modules = manifest.clone();
        w.fleets.get_mut(&hull).unwrap().mission = Some(TradeMission::DeliverToSystem { system: home });
        w.resolve_trade_arrivals(&mut Vec::new());
        assert!(w.fleets.contains_key(&hull), "a module-only return is not an expendable transfer convoy");
        assert!(w.fleets[&hull].modules.is_empty());
        assert!(w.fleets[&hull].mission.is_none());
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules, manifest);

        // Real player hauls target the Warehouse, not the legacy disposable
        // SellAtHub mission. Bulk goods unload; equipment stays aboard, with no
        // surprise home redirect and no deletion of a persistent hull.
        w.fleets.get_mut(&hull).unwrap().pos = w.hub;
        w.fleets.get_mut(&hull).unwrap().modules = SitePrize::ScreenCache.modules();
        w.fleets.get_mut(&hull).unwrap().add_cargo(crate::cargo::Commodity::Alloys, 7);
        w.fleets.get_mut(&hull).unwrap().mission = Some(TradeMission::DeliverToWarehouse { sell_on_arrival: false });
        w.resolve_trade_arrivals(&mut Vec::new());
        assert!(w.fleets.contains_key(&hull));
        assert_eq!(w.fleets[&hull].modules, SitePrize::ScreenCache.modules());
        assert_eq!(w.fleets[&hull].cargo_amount(crate::cargo::Commodity::Alloys), 0);
        assert!(w.fleets[&hull].mission.is_none());
        assert!(matches!(w.fleets[&hull].order, FleetOrder::Idle));
        assert_eq!(w.players[&p].warehouse[&crate::cargo::Commodity::Alloys], 7);
    }

    #[test]
    fn prize_briefings_do_not_disclose_unsurveyed_deposits() {
        let (mut w, p, site, _, _) = cleared_site(pirate::STRONGHOLD_TIER);
        w.players.get_mut(&p).unwrap().surveyed.remove(&site);
        let unknown = w.pirate_prize_briefing(p, site, pirate::STRONGHOLD_TIER);
        assert!(!unknown.summary.contains("Settlement prospect:"));
        assert!(unknown.summary.contains("Battleship dossier"), "authored prize terms visible before acceptance");
        w.players.get_mut(&p).unwrap().surveyed.insert(site);
        let known = w.pirate_prize_briefing(p, site, pirate::STRONGHOLD_TIER);
        assert!(known.summary.contains("Settlement prospect:"));
        assert!(w.systems.iter().find(|s| s.id == site).unwrap().all_deposits()
            .any(|d| known.summary.contains(&d.resource.display_name())));
    }
}
