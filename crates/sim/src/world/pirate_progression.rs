//! Fixed-strength, persistent conquest prizes. Templates are public contract
//! terms; discovery and outcomes still travel through scout/operation reports.
use super::*;
use crate::cargo::Commodity;
use crate::operation::{FollowUpKind, OperationBriefing, OperationIssuer, OperationKind,
    OperationReward, OperationScope};

impl World {
    pub(super) fn seed_progression_sites(&mut self) {
        for tier in [pirate::DEPOT_TIER, pirate::STRONGHOLD_TIER] {
            if self.enclaves.values().any(|e| e.tier == tier) { continue; }
            // Safe also on old saves: no homes, colonies, existing structures,
            // nearby fleets or engagements. Never relocate an existing object.
            let radius = self.config.galaxy_radius;
            let candidate = self.systems.iter().filter(|s| s.owner.is_none()
                && !self.enclaves.contains_key(&s.id)
                && (radius * 0.30..=radius * 0.80).contains(&s.pos.length())
                && s.bodies.iter().all(|b| b.structures.values().all(|n| *n == 0))
                && self.home_slots.iter().all(|h| h.pos.distance(s.pos) >= 30_000.0)
                && self.fleets.values().all(|f| f.pos.distance(s.pos) > 20_000.0)
                && self.engagements.values().all(|e| e.pos.distance(s.pos) > 20_000.0))
                // Guard a genuine industrial opportunity, not a random empty
                // system. Pick from existing geology; don't buff every resource.
                .max_by(|a, b| {
                    let value = |s: &StarSystem| s.bodies.iter().flat_map(|body|
                        body.deposits.iter().filter(|d| d.resource.is_mineable_mineral())
                            .map(move |d| crate::explore::natural_extraction_rate(body, d, s.trait_)))
                        .fold(0.0, f64::max);
                    value(a).total_cmp(&value(b)).then_with(|| b.id.cmp(&a.id))
                }).map(|s| (s.id, s.pos));
            let Some((sid, pos)) = candidate else { continue; };
            self.systems.iter_mut().find(|s| s.id == sid).unwrap()
                .set_tier(crate::build::StructureKind::DefensePlatform, pirate::base_defense_tiers(tier));
            let pack = self.spawn_fixed_pirates(pirate::formation(tier), pos, FleetOrder::Idle);
            self.enclaves.insert(sid, Enclave { system: sid, tier,
                plunder: BTreeMap::from([(Commodity::Alloys, if tier == pirate::STRONGHOLD_TIER { 240 } else { 100 }),
                    (Commodity::Electronics, if tier == pirate::STRONGHOLD_TIER { 100 } else { 40 })]),
                next_launch_at: 0.0, next_grow_at: 0.0, dormant_until: 0.0,
                pack: Some(pack), cleared: false });
        }
    }

    pub(super) fn spawn_fixed_pirates(&mut self,
        formation: Vec<(ShipKind, crate::module::Loadout, u32)>, pos: Vec2, order: FleetOrder,
    ) -> EntityId {
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, PlayerId::PIRATE, formation[0].0, pos, order, None);
        fleet.reset_to(formation[0].0, 0);
        for (kind, loadout, count) in formation {
            assert!(loadout.validate(kind), "illegal authored pirate fitting");
            fleet.add_fitted(kind, &loadout, count);
        }
        self.fleets.insert(id, fleet);
        id
    }

    /// Goods stay AT the defeated base until a cargo fleet recovers them. The
    /// offer and eventual payout use the same delayed report path as salvage.
    pub(super) fn offer_site_plunder(&mut self, player: PlayerId, site: EntityId, pos: Vec2,
        plunder: &BTreeMap<Commodity, u32>, events: &mut Vec<Event>) {
        let link = self.operations.values().rev().find_map(|o| {
            if matches!(o.scope, OperationScope::Private { player: p } if p == player)
                && !o.state.terminal() { o.counter_raid.filter(|c| c.source == site) } else { None }
        });
        for (&commodity, &units) in plunder {
            if units == 0 { continue; }
            let id = self.insert_operation(OperationIssuer::SalvageOffice, OperationScope::Private { player },
                OperationKind::RescueSalvage { pos, commodity, units, source_fleet: EntityId(0) },
                units, OperationReward::default(), pos, 24.0 * 60.0 * 60.0, events);
            let o = self.operations.get_mut(&id).unwrap();
            o.counter_raid = link;
            o.briefing = Some(OperationBriefing { follow_up: if link.is_some() {
                FollowUpKind::CounterRaidRecovery } else { FollowUpKind::SiteRecovery },
                title: format!("{}Recover {}", if link.is_some() { "3/3 · " } else { "" }, commodity.slug().replace('_', " ")),
                difficulty: "Cargo recovery".into(), suitable_fleets: "Freighter; escort if the route is unsafe".into(), variant: 0,
                summary: format!("{units} {} at the cleared hideout. Load on-site, then haul home or to Market.", commodity.slug().replace('_', " ")) });
        }
    }
}

pub(super) fn site_briefing(tier: u32) -> OperationBriefing {
    let stronghold = tier == pirate::STRONGHOLD_TIER;
    OperationBriefing { follow_up: if stronghold { FollowUpKind::Stronghold } else { FollowUpKind::Depot },
        title: pirate::site_name(tier).into(), variant: 0,
        difficulty: if stronghold { "Very high · fixed garrison" } else { "High · fortified" }.into(),
        suitable_fleets: if stronghold { "Prepared example: 4 Cruisers + 2 Destroyers + 2 screening Corvettes" }
            else { "Prepared example: 4 Destroyers + 2 screening Corvettes" }.into(),
        summary: format!("{} Prize: {}", if stronghold {
            "6 defense tiers; armored Cruiser, torpedo Destroyer, 2 point-defense Corvettes. Scout for support-site weaknesses. Clear permanently to open settlement and recover remaining stores on-site."
        } else {
            "3 defense tiers; driver Destroyer + point-defense Corvette. Scout for support-site weaknesses, then bring armor and sustained firepower. Clear permanently; recover remaining stores on-site."
        }, pirate::SitePrize::for_tier(tier).unwrap().summary()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Loadout, ModuleKind::*};
    use crate::tactical::ProjSetup;

    fn setup() -> (World, PlayerId) {
        let mut world = World::new(SimConfig::for_players(123, 4));
        let player = PlayerId(95001);
        world.step(&[Command::AddPlayer { id: player, name: "Conquest".into() }]);
        (world, player)
    }
    fn site(w: &World, tier: u32) -> (EntityId, Vec2) {
        let e = w.enclaves.values().find(|e| e.tier == tier).unwrap();
        (e.system, w.systems.iter().find(|s| s.id == e.system).unwrap().pos)
    }
    fn friendly(w: &mut World, owner: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None));
        id
    }
    fn arrive(w: &mut World) {
        w.time = w.pending_operation_reports.iter().map(|r| r.arrive_at).fold(w.time, f64::max);
        w.deliver_operation_reports();
    }

    #[test]
    fn fixed_sites_are_seeded_safe_fitted_and_saved_without_reinforcements() {
        for seed in [1, 42, 123, 992] {
            let mut w = World::new(SimConfig::for_players(seed, 5));
            for tier in [pirate::DEPOT_TIER, pirate::STRONGHOLD_TIER] {
                let (sid, pos) = site(&w, tier);
                let pack = w.enclaves[&sid].pack.unwrap();
                let f = &w.fleets[&pack];
                assert!(w.home_slots.iter().all(|h| pos.distance(h.pos) >= 30_000.0));
                assert!(f.ships.iter().all(|s| s.hp == s.max_hp() && s.loadout.validate(s.kind)));
                assert!(!f.operation_privateer, "ordinary full-strength opponents");
                w.fleets.get_mut(&pack).unwrap().ships[0].hp *= 0.5;
                let hp = w.fleets[&pack].ships[0].hp;
                w.time += pirate::PIRATE_GROW_PERIOD * 10.0;
                w.pirate_ai(&mut Vec::new());
                assert_eq!(w.fleets[&pack].ships[0].hp, hp, "no timer repair");
                assert!(matches!(w.fleets[&pack].order, FleetOrder::Idle));
            }
            let count = w.fleets.len();
            let mut saved: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
            saved.fixup_after_load();
            assert_eq!(saved.fleets.len(), count, "reload doesn't replenish the garrison");
        }
    }

    #[test]
    fn heavy_assaults_need_no_interceptor_and_platform_death_is_not_victory() {
        let (mut w, p) = setup();
        let (sid, pos) = site(&w, pirate::DEPOT_TIER);
        let destroyer = friendly(&mut w, p, ShipKind::Destroyer, pos);
        let pirate = w.enclaves[&sid].pack.unwrap();
        w.pirate_ai(&mut Vec::new());
        assert!(w.engagements.values().any(|e| e.platform_system == Some(sid) && e.attackers.contains(&destroyer)));
        w.systems.iter_mut().find(|s| s.id == sid).unwrap().set_tier(crate::build::StructureKind::DefensePlatform, 0);
        let mut events = Vec::new();
        w.pirate_ai(&mut events);
        assert!(!w.enclaves[&sid].cleared);
        assert!(w.fleets.contains_key(&pirate), "cannot delete live defenders on platform death");
        assert!(!events.iter().any(|e| matches!(e.payload, EventPayload::PirateEnclaveCleared { .. })));
    }

    #[test]
    fn clearing_a_stronghold_opens_settlement_and_physical_plunder_not_a_home_teleport() {
        let (mut w, p) = setup();
        let (sid, pos) = site(&w, pirate::STRONGHOLD_TIER);
        let colony = friendly(&mut w, p, ShipKind::Colony, pos);
        w.resolve_colony_arrivals(&mut Vec::new());
        assert!(w.fleets.contains_key(&colony));
        assert!(w.systems.iter().find(|s| s.id == sid).unwrap().owner.is_none());
        let cruiser = friendly(&mut w, p, ShipKind::Cruiser, pos);
        let bounty = w.insert_operation(OperationIssuer::Authority, OperationScope::Private { player: p },
            OperationKind::PirateBounty { system: sid, tier: pirate::STRONGHOLD_TIER }, 1,
            OperationReward { credits: 8000.0, ..Default::default() }, pos, 1800.0, &mut Vec::new());
        arrive(&mut w);
        w.apply_accept_operation(p, bounty);
        let before = w.players[&p].credits;
        let home = w.players[&p].home_system.unwrap();
        let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        // The combat engine's end-state; suppression must consume it exactly once.
        w.fleets.remove(&w.enclaves[&sid].pack.unwrap());
        w.systems.iter_mut().find(|s| s.id == sid).unwrap().set_tier(crate::build::StructureKind::DefensePlatform, 0);
        let mut events = Vec::new();
        w.pirate_ai(&mut events);
        w.process_operation_events(&events.clone(), &mut events);
        assert!(w.enclaves[&sid].cleared);
        assert_eq!(w.players[&p].credits, before, "reward must wait for light");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
        let loot: Vec<_> = w.operations.values().filter(|o| matches!(o.kind, OperationKind::RescueSalvage { .. })).map(|o| o.id).collect();
        assert_eq!(loot.len(), 2);
        assert!(loot.iter().all(|id| !w.operations[id].is_visible_to(p)));
        arrive(&mut w);
        assert_eq!(w.players[&p].credits, before + 8000.0);
        w.deliver_operation_reports();
        assert_eq!(w.players[&p].credits, before + 8000.0);
        let freighter = friendly(&mut w, p, ShipKind::Convoy, pos);
        let alloys = *loot.iter().find(|id| matches!(w.operations[id].kind,
            OperationKind::RescueSalvage { commodity: Commodity::Alloys, .. })).unwrap();
        w.apply_accept_operation(p, alloys);
        w.apply_recover_operation(p, alloys, freighter, &mut Vec::new());
        assert_eq!(w.fleets[&freighter].cargo_amount(Commodity::Alloys), 240);
        w.resolve_colony_arrivals(&mut Vec::new());
        assert_eq!(w.systems.iter().find(|s| s.id == sid).unwrap().owner, Some(p));
        w.time += pirate::PIRATE_DORMANCY * 10.0;
        w.pirate_ai(&mut Vec::new());
        let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        restored.fixup_after_load();
        assert!(restored.enclaves[&sid].cleared);
        assert!(restored.enclaves[&sid].pack.is_none());
        assert!(restored.fleets.contains_key(&cruiser));
    }

    fn ships(formation: Vec<(ShipKind, Loadout, u32)>, fleet: u64) -> Vec<(EntityId, crate::ship::Ship)> {
        let mut n = 0;
        formation.into_iter().flat_map(|(kind, loadout, count)| (0..count).map(move |_| (kind, loadout.clone())))
            .map(|(kind, loadout)| { n += 1; (EntityId(fleet), crate::ship::Ship::new(n, kind, loadout)) }).collect()
    }

    #[test]
    fn a_stale_offer_for_a_cleared_site_fails_only_when_its_new_report_arrives() {
        let (mut w, p) = setup();
        let (sid, pos) = site(&w, pirate::DEPOT_TIER);
        let id = w.insert_operation(OperationIssuer::Authority, OperationScope::Private { player: p },
            OperationKind::PirateBounty { system: sid, tier: pirate::DEPOT_TIER }, 1,
            OperationReward { credits: 3000.0, ..Default::default() }, pos, 1800.0, &mut Vec::new());
        arrive(&mut w);
        let before = w.players[&p].credits;
        w.enclaves.get_mut(&sid).unwrap().cleared = true;
        w.tick_operations(&mut Vec::new());
        assert_eq!(w.operations[&id].state, crate::operation::OperationState::Failed);
        assert_eq!(w.operations[&id].known[&p].state, crate::operation::OperationState::Offered);
        arrive(&mut w);
        assert_eq!(w.operations[&id].known[&p].state, crate::operation::OperationState::Failed);
        assert_eq!(w.players[&p].credits, before);
    }

    #[test]
    fn a_corvette_guard_detects_an_inbound_heavy_threat_before_contact() {
        let (mut w, p) = setup();
        let pos = w.players[&p].home + Vec2::new(10_000.0, 0.0);
        let charge = friendly(&mut w, p, ShipKind::Convoy, pos);
        let guard = friendly(&mut w, p, ShipKind::Corvette, pos);
        w.fleets.get_mut(&guard).unwrap().add(ShipKind::Corvette, 2);
        w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: charge };
        let enemy = w.spawn_fixed_pirates(vec![(ShipKind::Destroyer, Loadout::default(), 1)],
            pos + Vec2::new(3000.0, 0.0), FleetOrder::Intercept { target: charge });
        w.fleets.get_mut(&enemy).unwrap().vel = Vec2::new(-1000.0, 0.0);
        w.autonomous_defense();
        assert!(w.fleets[&guard].defense.as_ref().is_some_and(|d| d.target == enemy),
            "guarding must work against a Destroyer flagship, not only Raider identifiers");
    }

    #[test]
    fn authored_combat_progression_has_real_fitting_and_heavy_fleet_payoffs() {
        for tier in [pirate::DEPOT_TIER, pirate::STRONGHOLD_TIER] {
            let light = ships(vec![(ShipKind::Raider, Loadout::default(), 2)], 1);
            let heavy = ships(if tier == pirate::DEPOT_TIER {
                vec![(ShipKind::Destroyer, Loadout::new(vec![MassDriver, WhippleArmor, ReflectivePlating]), 4),
                    (ShipKind::Corvette, Loadout::new(vec![PointDefenseScreen, ReflectivePlating]), 2)]
            } else {
                vec![(ShipKind::Cruiser, Loadout::new(vec![MassDriver, WhippleArmor, ReflectivePlating]), 4),
                    (ShipKind::Destroyer, Loadout::new(vec![MassDriver, WhippleArmor, ReflectivePlating]), 2),
                    (ShipKind::Corvette, Loadout::new(vec![PointDefenseScreen, ReflectivePlating]), 2)]
            }, 1);
            let mut light_wins = 0;
            let mut heavy_wins = 0;
            for seed in 0..5 {
                let mut setup = ProjSetup { a: light.clone(), d: ships(pirate::formation(tier), 2),
                    platform_tiers: pirate::base_defense_tiers(tier), ..Default::default() };
                light_wins += u32::from(crate::tactical::simulate_engagement(&setup, seed).a_won);
                setup.a = heavy.clone();
                let result = crate::tactical::simulate_engagement(&setup, seed);
                heavy_wins += u32::from(result.a_won);
                eprintln!("{} seed {seed}: prepared fleet won={} losses={:?}, rounds={}",
                    pirate::site_name(tier), result.a_won, result.a_losses, result.steps);
            }
            assert_eq!(light_wins, 0, "two stock Interceptors cannot trivialize a fortified objective");
            assert!(heavy_wins >= 4, "prepared heavy fleet must make the opportunity achievable");
        }
        let keys: std::collections::BTreeSet<_> = (0..3).map(|v| {
            let fit = pirate::patrol_fitting(v);
            assert!(fit.validate(ShipKind::Raider));
            fit.key()
        }).collect();
        assert_eq!(keys.len(), 3, "three genuinely different patrol fittings");
        for (variant, counter) in [WhippleArmor, PointDefenseScreen, ReflectivePlating].into_iter().enumerate() {
            let mut stock_wins = 0;
            let mut fitted_wins = 0;
            for seed in 0..20 {
                let mut setup = ProjSetup { a: ships(vec![(ShipKind::Raider, Loadout::default(), 1)], 1),
                    d: ships(vec![(ShipKind::Raider, pirate::patrol_fitting(variant as u8), 1)], 2),
                    ..Default::default() };
                stock_wins += u32::from(crate::tactical::simulate_engagement(&setup, seed).a_won);
                setup.a = ships(vec![(ShipKind::Raider, Loadout::new(vec![counter]), 1)], 1);
                fitted_wins += u32::from(crate::tactical::simulate_engagement(&setup, seed).a_won);
            }
            eprintln!("patrol {variant}: stock wins {stock_wins}/20, counter-fitted wins {fitted_wins}/20");
            assert!(fitted_wins > stock_wins, "the published counter must improve the actual fight");
        }
    }
}
