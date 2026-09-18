use super::*;
use crate::build::{BuildKind, StructureKind as K};
use crate::cargo::Commodity as C;
use crate::module::{Loadout, ModuleKind as M};

fn yard() -> (World, PlayerId, EntityId, EntityId) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    let owner = PlayerId(91_123);
    w.step(&[Command::AddPlayer { id: owner, name: "Utility test".into() }]);
    w.enclaves.clear();
    let home = w.players[&owner].home_system.unwrap();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(0.05);
    for b in &mut sys.bodies {
        b.assignments.clear();
        for kind in [K::Shipyard, K::OrdnanceFoundry, K::ArmamentsComplex] { b.structures.remove(&kind); }
    }
    sys.bodies[0].structures.insert(K::Shipyard, 1);
    sys.bodies[0].assignments.insert(K::Shipyard, crate::production::Assignment::crew(1));
    for m in [M::ExtendedTanks, M::ReconSuite, M::CargoPods, M::EscortDatalink] {
        sys.modules.insert(m, 3);
        for (c, _) in crate::build::module_recipe(m).costs { sys.stockpile.insert(*c, 1000.0); }
    }
    let pos = sys.pos;
    let id = w.alloc_entity_id();
    let mut f = Fleet::single(id, owner, ShipKind::Convoy, pos, FleetOrder::Idle, None);
    f.fuel = 20.0;
    f.add_cargo(C::MetallicOre, 42);
    w.fleets.insert(id, f);
    (w, owner, home, id)
}

fn finish_refit(w: &mut World) {
    for _ in 0..(4 * TICK_HZ) {
        w.tick += 1;
        w.time += DT;
        w.resolve_refits(&mut vec![]);
    }
    assert!(w.refit_queue.is_empty());
}

#[test]
fn utilities_have_real_early_unlocks_and_strict_hull_limits() {
    use crate::research::{ResearchState, is_available, has_module};
    let mut r = ResearchState::default();
    for (id, module) in [("comp_shadow_iv_recon_suite", M::ReconSuite), ("prop_expedition_iii_extended_tanks", M::ExtendedTanks), ("hull_cargo_pods", M::CargoPods), ("hull_line_iii_escort_datalink", M::EscortDatalink)] {
        assert!(is_available(id, &r, &|_| 0.0, 0.0));
        assert!(!has_module(&r, module));
        r.completed.insert(id.into()); // old completed save ids also unlock manufacturing
        assert!(has_module(&r, module));
    }
    assert!(Loadout::new(vec![M::ExtendedTanks]).validate(ShipKind::Convoy));
    for m in crate::module::MODULE_KINDS {
        assert_eq!(Loadout::new(vec![m]).validate(ShipKind::Convoy), matches!(m, M::ExtendedTanks | M::CargoPods | M::FuelTransferRig));
    }
    for hull in [ShipKind::Freighter, ShipKind::Colony, ShipKind::Titan, ShipKind::Builder] {
        assert!(!Loadout::new(vec![M::ExtendedTanks]).validate(hull));
    }
    assert!(!Loadout::new(vec![M::ExtendedTanks, M::ExtendedTanks]).validate(ShipKind::Raider));
    assert!(Loadout::new(vec![M::ExtendedTanks, M::ReconSuite]).validate(ShipKind::Raider));
}

#[test]
fn escort_datalink_requires_pd_and_uses_both_corvette_slots() {
    let link = Loadout::new(vec![M::EscortDatalink]);
    let fit = Loadout::new(vec![M::PointDefenseScreen, M::EscortDatalink]);
    assert!(!link.validate(ShipKind::Corvette), "fire control alone has nothing to fire");
    assert!(fit.validate(ShipKind::Corvette));
    assert_eq!(fit.len() as u32, ShipKind::Corvette.module_slots());
    assert_eq!(fit.fitting_cost(), 4);
    for hull in crate::ship::ALL_SHIP_KINDS {
        assert_eq!(fit.validate(hull), hull == ShipKind::Corvette);
    }
    for extra in crate::module::MODULE_KINDS {
        assert!(!Loadout::new(vec![M::PointDefenseScreen, M::EscortDatalink, extra]).validate(ShipKind::Corvette));
    }
    let pd = Loadout::new(vec![M::PointDefenseScreen]);
    assert_eq!(fit.offense(), pd.offense());
    assert_eq!(fit.fuel_capacity_mult(), pd.fuel_capacity_mult());
    assert!(!fit.has_recon());
    assert!(pd.utility_change_to(&fit));
    assert!(!Loadout::default().utility_change_to(&fit), "installing the PD gun still needs the military workshop");
    assert_eq!(Loadout::from_key(&fit.key()), fit);
}

#[test]
fn datalink_blueprint_makes_physical_crates_with_staffed_shipyard_work() {
    let (mut w, owner, home, _) = yard();
    let what = BuildKind::Module { module: M::EscortDatalink };
    let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
    w.players.get_mut(&owner).unwrap().research.completed.insert("hull_line_iii_escort_datalink".into());
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert_eq!(w.build_queue.len(), 1);
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for (c, cost) in crate::build::module_recipe(M::EscortDatalink).costs {
        assert_eq!(sys.stockpile[c], stock[c] - cost);
    }
    sys.bodies[0].assignments.clear();
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert_eq!(w.build_queue.len(), 1);
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies[0].assignments.insert(K::Shipyard, crate::production::Assignment::crew(1));
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::EscortDatalink], 4);
}

#[test]
fn fitting_and_removing_a_datalink_preserves_the_gun_hull_and_command_delay() {
    let (mut w, owner, home, id) = yard();
    let pos = w.fleets[&id].pos;
    let mut fleet = Fleet::single(id, owner, ShipKind::Corvette, pos, FleetOrder::Idle, None);
    let pd = Loadout::new(vec![M::PointDefenseScreen]);
    let linked = Loadout::new(vec![M::PointDefenseScreen, M::EscortDatalink]);
    fleet.set_fitted(ShipKind::Corvette, &pd, 1);
    fleet.fuel = 20.0;
    fleet.ships[0].hp -= 15.0;
    w.fleets.insert(id, fleet);
    let before = w.fleets[&id].clone();
    w.players.get_mut(&owner).unwrap().command_center = pos - Vec2::new(10_000.0, 0.0);
    w.apply(&Command::RefitShips { player_id: owner, fleet_id: id, ship: ShipKind::Corvette,
        from: pd.clone(), to: linked.clone(), n: 1 }, &mut vec![]);
    assert_eq!(w.pending_orders.len(), 1);
    assert!(w.refit_queue.is_empty());
    assert_eq!(w.fleets[&id].ships, before.ships);
    let due = w.pending_orders[0].apply_time;
    w.time = due;
    w.deliver_due_orders(&mut vec![]);
    assert_eq!(w.refit_queue.len(), 1, "fitting a bought/recovered crate needs no blueprint");
    assert_eq!(w.fleets[&id].ships, before.ships, "not fitted until work finishes");
    let mut w: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].ships[0].loadout, linked);
    assert_eq!(w.fleets[&id].ships[0].hp, before.ships[0].hp);
    assert_eq!(w.fleets[&id].ships[0].id, before.ships[0].id);
    assert_eq!(w.fleets[&id].fuel, before.fuel);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::EscortDatalink], 2);
    w.apply_refit(owner, id, ShipKind::Corvette, linked, pd.clone(), 1, &mut vec![]);
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].ships[0].loadout, pd);
    let modules = &w.systems.iter().find(|s| s.id == home).unwrap().modules;
    assert_eq!(modules[&M::EscortDatalink], 3);
    assert_eq!(modules.get(&M::PointDefenseScreen).copied().unwrap_or(0), 0, "never duplicate the retained PD gun");
}

#[test]
fn fitted_capacity_and_sensors_belong_to_hulls_not_every_ship_in_the_fleet() {
    let (mut w, _, _, id) = yard();
    let f = w.fleets.get_mut(&id).unwrap();
    f.add(ShipKind::Convoy, 2);
    f.add(ShipKind::Scout, 1);
    let cap = f.fuel_capacity();
    let fuel = f.fuel;
    f.set_fitted(ShipKind::Convoy, &Loadout::new(vec![M::ExtendedTanks]), 1);
    assert!((f.fuel_capacity() - cap - ShipKind::Convoy.hull_mass() * 0.035 * 0.75).abs() < 1e-9);
    assert_eq!(f.fuel, fuel);
    f.set_fitted(ShipKind::Scout, &Loadout::new(vec![M::ReconSuite]), 1);
    assert_eq!(f.sensor_mult(), 0.5);
    assert_eq!(f.exploration_range_mult(), 2.0);
    let mut scout = Fleet::single(EntityId(9), f.owner, ShipKind::Scout, f.pos, FleetOrder::Idle, None);
    assert!(!scout.projects_sensor());
    let speed = scout.max_speed();
    scout.set_fitted(ShipKind::Scout, &Loadout::new(vec![M::ReconSuite]), 1);
    assert!(scout.projects_sensor());
    assert_eq!(scout.sensor_mult(), 0.5);
    assert_eq!(scout.max_speed(), speed);
}

#[test]
fn utilities_need_research_local_materials_and_live_shipyard_workforce() {
    let (mut w, owner, home, _) = yard();
    let what = BuildKind::Module { module: M::ExtendedTanks };
    let before = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, before);
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_expedition_iii_extended_tanks".into());
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert_eq!(w.build_queue.len(), 1);
    let debited = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies[0].assignments.clear();
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert_eq!(w.build_queue.len(), 1, "no work without workforce");
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies[0].assignments.insert(K::Shipyard, crate::production::Assignment::crew(1));
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert!(w.build_queue.is_empty());
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    assert_eq!(sys.modules[&M::ExtendedTanks], 4);
    assert_eq!(sys.stockpile, debited, "resuming work never debits a second recipe");
}

#[test]
fn utility_refits_preserve_identity_damage_manifest_and_fuel_and_survive_restart() {
    let (mut w, owner, home, id) = yard();
    let before = w.fleets[&id].clone();
    let to = Loadout::new(vec![M::ExtendedTanks]);
    w.fleets.get_mut(&id).unwrap().ships[0].hp -= 100.0;
    w.apply_refit(owner, id, ShipKind::Convoy, Loadout::default(), to.clone(), 1, &mut vec![]);
    assert_eq!(w.refit_queue.len(), 1, "recovered crate needs no research or foundry");
    assert_eq!(w.fleets[&id].fuel_capacity(), before.fuel_capacity());
    assert!(!w.fleet_at_owned_system(owner, id), "cannot split/merge while in the yard");
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::ExtendedTanks], 2);
    let mut w: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    finish_refit(&mut w);
    let f = &w.fleets[&id];
    assert_eq!(f.fuel, before.fuel);
    assert_eq!(f.cargo_stacks(), before.cargo_stacks());
    assert_eq!(f.ships[0].hp, before.ships[0].hp - 100.0);
    assert_eq!(f.ships[0].id, before.ships[0].id);
    assert!((f.fuel_capacity() - before.fuel_capacity() * 1.75).abs() < 1e-9);
    w.apply_refit(owner, id, ShipKind::Convoy, to, Loadout::default(), 1, &mut vec![]);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::ExtendedTanks], 2,
        "an installed crate cannot be lent to another hull during removal");
    finish_refit(&mut w);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::ExtendedTanks], 3);
    assert_eq!(w.fleets[&id].fuel, before.fuel);
}

#[test]
fn tank_removal_never_deletes_excess_fuel_and_basic_yards_cannot_refit_weapons() {
    let (mut w, owner, home, id) = yard();
    let tanks = Loadout::new(vec![M::ExtendedTanks]);
    w.fleets.get_mut(&id).unwrap().set_fitted(ShipKind::Convoy, &tanks, 1);
    w.fleets.get_mut(&id).unwrap().fuel = w.fleets[&id].fuel_capacity();
    w.apply_refit(owner, id, ShipKind::Convoy, tanks, Loadout::default(), 1, &mut vec![]);
    assert!(w.refit_queue.is_empty());
    let raider = w.alloc_entity_id();
    w.fleets.insert(raider, Fleet::single(raider, owner, ShipKind::Raider, w.fleets[&id].pos, FleetOrder::Idle, None));
    w.systems.iter_mut().find(|s| s.id == home).unwrap().modules.insert(M::MassDriver, 1);
    w.apply_refit(owner, raider, ShipKind::Raider, Loadout::default(), Loadout::new(vec![M::MassDriver]), 1, &mut vec![]);
    assert!(w.refit_queue.is_empty());
}

#[test]
fn equipment_split_and_merge_conserve_fuel_and_capacity() {
    let (mut w, owner, _, id) = yard();
    w.fleets.get_mut(&id).unwrap().add(ShipKind::Scout, 1);
    w.fleets.get_mut(&id).unwrap().set_fitted(ShipKind::Scout, &Loadout::new(vec![M::ExtendedTanks]), 1);
    let cap = w.fleets[&id].fuel_capacity();
    let fuel = w.fleets[&id].fuel;
    let mut events = vec![];
    w.apply_split_fleet(owner, id, &BTreeMap::from([(ShipKind::Scout, 1)]), &mut events);
    let split = events.iter().find_map(|e| if let EventPayload::ShipSpawned { id, .. } = e.payload { Some(id) } else { None }).unwrap();
    assert!((w.fleets[&id].fuel + w.fleets[&split].fuel - fuel).abs() < 1e-9);
    assert!((w.fleets[&id].fuel_capacity() + w.fleets[&split].fuel_capacity() - cap).abs() < 1e-9);
    w.apply_merge_fleets(owner, id, split, &mut []);
    assert!(!w.fleets.contains_key(&split));
    assert!((w.fleets[&id].fuel - fuel).abs() < 1e-9);
    assert_eq!(w.fleets[&id].fuel_capacity(), cap);
}

#[test]
fn deferred_research_is_hidden_without_erasing_saved_work() {
    let mut r = crate::research::ResearchState::default();
    r.active = Some("mat_beneficiation".into());
    r.progress = 123.0;
    r.queue = vec!["comp_gravimetric_survey".into(), "prop_expedition_iii_extended_tanks".into()];
    r.suspend_unimplemented();
    assert_eq!(r.active.as_deref(), Some("prop_expedition_iii_extended_tanks"));
    assert_eq!(r.progress, 0.0);
    assert_eq!(r.recovered_data["mat_beneficiation"], 123.0);
    r.suspend_unimplemented();
    assert_eq!(r.recovered_data["mat_beneficiation"], 123.0);
}

#[test]
fn utility_refit_starts_only_when_the_command_reaches_the_dock() {
    let (mut w, owner, _, id) = yard();
    w.players.get_mut(&owner).unwrap().command_center = w.fleets[&id].pos - Vec2::new(10_000.0, 0.0);
    let before = w.fleets[&id].fuel_capacity();
    w.apply(&Command::RefitShips { player_id: owner, fleet_id: id, ship: ShipKind::Convoy,
        from: Loadout::default(), to: Loadout::new(vec![M::ExtendedTanks]), n: 1 }, &mut vec![]);
    assert_eq!(w.pending_orders.len(), 1);
    let arrival = w.pending_orders[0].apply_time;
    assert!((arrival - w.time - 5.0).abs() < 1e-6);
    w.time = arrival - 1e-4;
    w.deliver_due_orders(&mut vec![]);
    assert!(w.refit_queue.is_empty());
    w.time = arrival;
    w.deliver_due_orders(&mut vec![]);
    assert_eq!(w.refit_queue.len(), 1);
    assert_eq!(w.fleets[&id].fuel_capacity(), before);
    finish_refit(&mut w);
    assert!(w.fleets[&id].fuel_capacity() > before);
}

#[test]
fn an_interrupted_utility_refit_returns_only_reserved_crates_not_a_remote_upgrade() {
    let (mut w, owner, home, id) = yard();
    let before = w.fleets[&id].fuel_capacity();
    w.apply_refit(owner, id, ShipKind::Convoy, Loadout::default(), Loadout::new(vec![M::ExtendedTanks]), 1, &mut vec![]);
    let fleet = w.fleets.get_mut(&id).unwrap();
    fleet.pos = fleet.pos + Vec2::new(50_000.0, 0.0);
    let mut events = vec![];
    w.resolve_refits(&mut events);
    assert!(w.refit_queue.is_empty());
    assert_eq!(w.fleets[&id].fuel_capacity(), before);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::ExtendedTanks], 3);
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::OrderRejected { .. }) && e.origin.is_some()));
    w.resolve_refits(&mut vec![]);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::ExtendedTanks], 3, "refund once");
}

#[test]
fn cargo_pods_double_only_fitted_freighters_and_compete_with_tanks() {
    let (mut w, _, _, id) = yard();
    let pods = Loadout::new(vec![M::CargoPods]);
    assert!(pods.validate(ShipKind::Convoy));
    for hull in [ShipKind::Scout, ShipKind::Raider, ShipKind::Corvette, ShipKind::Freighter, ShipKind::Titan] {
        assert!(!pods.validate(hull));
    }
    assert!(!Loadout::new(vec![M::CargoPods, M::ExtendedTanks]).validate(ShipKind::Convoy));
    assert!(!Loadout::new(vec![M::CargoPods, M::CargoPods]).validate(ShipKind::Convoy));
    let fleet = w.fleets.get_mut(&id).unwrap();
    fleet.add(ShipKind::Convoy, 2);
    fleet.add(ShipKind::Scout, 1);
    let before = (fleet.fuel, fleet.fuel_capacity(), fleet.max_speed(), fleet.cargo_stacks());
    assert_eq!(fleet.cargo_capacity(), 1200);
    fleet.set_fitted(ShipKind::Convoy, &pods, 1);
    assert_eq!(fleet.cargo_capacity(), 1600);
    assert_eq!((fleet.fuel, fleet.fuel_capacity(), fleet.max_speed(), fleet.cargo_stacks()), before);
}

#[test]
fn cargo_pods_are_manufactured_only_after_research_with_staffed_work() {
    let (mut w, owner, home, _) = yard();
    let what = BuildKind::Module { module: M::CargoPods };
    let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
    w.players.get_mut(&owner).unwrap().research.completed.insert("hull_cargo_pods".into());
    w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
    assert_eq!(w.build_queue.len(), 1);
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for (commodity, cost) in crate::build::module_recipe(M::CargoPods).costs {
        assert_eq!(sys.stockpile[commodity], stock[commodity] - cost);
    }
    sys.bodies[0].assignments.clear();
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::CargoPods], 3);
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies[0].assignments.insert(K::Shipyard, crate::production::Assignment::crew(1));
    w.update_ship_build_work();
    w.tick += 1000;
    w.resolve_builds(&mut vec![]);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::CargoPods], 4);
}

#[test]
fn cargo_pod_refits_preserve_goods_and_expand_real_loading_space_only_on_completion() {
    let (mut w, owner, home, id) = yard();
    let manifest = w.fleets[&id].cargo_stacks();
    let fuel = w.fleets[&id].fuel;
    w.apply_refit(owner, id, ShipKind::Convoy, Loadout::default(), Loadout::new(vec![M::CargoPods]), 1, &mut vec![]);
    assert_eq!(w.refit_queue.len(), 1, "a recovered crate requires no blueprint");
    assert_eq!(w.fleets[&id].cargo_capacity(), 400);
    let mut w: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].cargo_capacity(), 800);
    assert_eq!(w.fleets[&id].cargo_stacks(), manifest);
    assert_eq!(w.fleets[&id].fuel, fuel);
    w.apply_logistics_load(owner, id, Some(home), C::Polymers, 300, &mut vec![]);
    assert_eq!(w.fleets[&id].cargo_units(), 342);
    let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
    w.apply_logistics_load(owner, id, Some(home), C::Alloys, 459, &mut vec![]);
    assert_eq!(w.fleets[&id].cargo_units(), 342, "the 801st unit refuses the whole lot");
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
}

#[test]
fn loaded_pods_cannot_be_removed_or_swapped_and_partial_refits_respect_pooled_cargo() {
    let (mut w, owner, home, id) = yard();
    let pods = Loadout::new(vec![M::CargoPods]);
    let fleet = w.fleets.get_mut(&id).unwrap();
    fleet.set_fitted(ShipKind::Convoy, &pods, 1);
    fleet.add_cargo(C::Alloys, 359); // 401 across two commodity stacks
    let ledger = w.systems.iter().find(|s| s.id == home).unwrap().modules.clone();
    for to in [Loadout::default(), Loadout::new(vec![M::ExtendedTanks])] {
        w.apply_refit(owner, id, ShipKind::Convoy, pods.clone(), to, 1, &mut vec![]);
        assert!(w.refit_queue.is_empty());
        assert_eq!(w.fleets[&id].cargo_units(), 401);
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules, ledger);
    }
    w.fleets.get_mut(&id).unwrap().remove_cargo(C::Alloys, 1);
    w.apply_refit(owner, id, ShipKind::Convoy, pods.clone(), Loadout::new(vec![M::ExtendedTanks]), 1, &mut vec![]);
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].cargo_units(), 400);
    assert_eq!(w.fleets[&id].cargo_capacity(), 400);
    assert_eq!(w.fleets[&id].fuel, 20.0);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&M::CargoPods], 4);

    let fleet = w.fleets.get_mut(&id).unwrap();
    fleet.add_fitted(ShipKind::Convoy, &pods, 2);
    fleet.add_cargo(C::Polymers, 801); // 1201 / 2000; only one set can come off
    w.apply_refit(owner, id, ShipKind::Convoy, pods.clone(), Loadout::default(), 2, &mut vec![]);
    assert!(w.refit_queue.is_empty());
    w.apply_refit(owner, id, ShipKind::Convoy, pods, Loadout::default(), 1, &mut vec![]);
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].cargo_capacity(), 1600);
    assert_eq!(w.fleets[&id].cargo_units(), 1201);
}

#[test]
fn cargo_changes_during_refit_cancel_without_losing_goods_or_duplicating_pods() {
    let (mut w, owner, home, id) = yard();
    let pods = Loadout::new(vec![M::CargoPods]);
    w.fleets.get_mut(&id).unwrap().set_fitted(ShipKind::Convoy, &pods, 1);
    w.apply_refit(owner, id, ShipKind::Convoy, pods, Loadout::new(vec![M::ExtendedTanks]), 1, &mut vec![]);
    w.fleets.get_mut(&id).unwrap().add_cargo(C::Alloys, 359);
    finish_refit(&mut w);
    assert_eq!(w.fleets[&id].cargo_capacity(), 800);
    assert_eq!(w.fleets[&id].cargo_units(), 401);
    let modules = &w.systems.iter().find(|s| s.id == home).unwrap().modules;
    assert_eq!(modules[&M::ExtendedTanks], 3, "unused reservation is returned");
    assert_eq!(modules[&M::CargoPods], 3, "installed pods are not returned as loose crates");
}

#[test]
fn splitting_cannot_strand_cargo_but_a_safe_split_keeps_its_pods() {
    let (mut w, owner, _, id) = yard();
    let fleet = w.fleets.get_mut(&id).unwrap();
    fleet.add(ShipKind::Convoy, 1);
    fleet.set_fitted(ShipKind::Convoy, &Loadout::new(vec![M::CargoPods]), 1);
    fleet.add_cargo(C::Alloys, 408); // 450; removing the podded hull would leave 400
    let before = serde_json::to_value(&w.fleets).unwrap();
    w.apply_split_fleet(owner, id, &BTreeMap::from([(ShipKind::Convoy, 1)]), &mut vec![]);
    assert_eq!(serde_json::to_value(&w.fleets).unwrap(), before);
    w.fleets.get_mut(&id).unwrap().remove_cargo(C::Alloys, 408);
    let mut events = vec![];
    w.apply_split_fleet(owner, id, &BTreeMap::from([(ShipKind::Convoy, 1)]), &mut events);
    let split = events.iter().find_map(|e| if let EventPayload::ShipSpawned { id, .. } = e.payload { Some(id) } else { None }).unwrap();
    assert_eq!(w.fleets[&split].cargo_capacity(), 800);
    assert_eq!(w.fleets[&split].cargo_units(), 0);
    assert_eq!(w.fleets[&id].cargo_capacity(), 400);
    assert_eq!(w.fleets[&id].cargo_units(), 42);
    w.apply_merge_fleets(owner, id, split, &mut []);
    assert_eq!(w.fleets[&id].cargo_capacity(), 1200);
    assert_eq!(w.fleets[&id].cargo_units(), 42);
}
