//! §fuel-overhaul (docs/fuel-overhaul.md) and §shipbuilding-overhaul
//! (docs/shipbuilding-overhaul-plan.md): one fuel per core grade — Uranium for
//! Fission Cores, Deuterium for Fusion, Helium-3 for Helium, Antimatter for Fold
//! (which jump); the Uranium Mill, Fusion Freight, empty launches, Hub
//! bunkerage per tank, depot rules and the Drive Works' six products.

use super::*;
use std::collections::BTreeMap;
use crate::build::{BuildKind, StructureKind as K};
use crate::cargo::Commodity as C;
use crate::module::{Loadout, ModuleKind as M};
use crate::production::Assignment;
use crate::ship::TransitMode;

fn founded() -> (World, PlayerId, EntityId) {
    let mut w = World::new(SimConfig::for_players(2026, 4));
    let owner = PlayerId(4242);
    w.step(&[Command::AddPlayer { id: owner, name: "Fuel".into() }]);
    w.enclaves.clear();
    let home = w.players[&owner].home_system.unwrap();
    (w, owner, home)
}

fn open_space(w: &mut World, owner: PlayerId, kind: ShipKind) -> EntityId {
    let id = w.alloc_entity_id();
    w.fleets.insert(id, Fleet::single(id, owner, kind, Vec2::new(900_000.0, 900_000.0), FleetOrder::Idle, None));
    id
}

fn stock(w: &World, sys: EntityId, c: C) -> f64 {
    w.systems.iter().find(|s| s.id == sys).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
}

#[test]
fn the_charter_carries_both_fuels_and_its_warp_cores() {
    let (w, owner, home) = founded();
    assert!(!w.fleets.values().any(|f| f.owner == owner), "§charter-v4: no founding fleet");
    // §charter-crates: the home's seed at landing. §den-stores (Nov 5 2026):
    // the trader sells the first Freighter's fuel, the Reach crate the
    // barge's; the landing, the den and the Reach crate hold the cores.
    assert_eq!(stock(&w, home, C::Uranium), crate::fuel::URANIUM_HOME_SEED);
    assert_eq!(stock(&w, home, C::Deuterium), 0.0, "§shipbuilding-overhaul: no Deuterium seed, no charter hull is a fusion hull");
    let supplies = crate::founding::charter_supplies();
    assert_eq!(supplies[&C::Uranium], 50.0, "new hulls fill at the dock: the Reach crate fuels the barge");
    let trader: std::collections::BTreeMap<C, u32> = crate::merchant::FOUNDING_STOCK.into_iter().collect();
    let tank = ShipKind::TinyFreighter.hull_mass() * crate::fuel::FUEL_PER_HULL_MASS;
    assert!(trader[&C::Uranium] as f64 >= tank, "and the trader the Freighter");
    let den: std::collections::BTreeMap<C, u32> = crate::founding::DEN_STORES.into_iter().collect();
    assert_eq!(supplies[&C::FissionReactor], 6.0, "the charter's six engines land with the colony");
    assert!(!den.contains_key(&C::FissionReactor), "the den's cores are the raid's take");
    assert_eq!(trader[&C::FissionReactor], 4, "the trader sells the Interceptor's, the Tiny Freighter's pair and the barge's fourth (§hull-ladder)");
    assert_eq!(crate::founding::RAID_TAKES[0], C::FissionReactor, "the raid takes the engines first");
}

#[test]
fn a_slow_trip_costs_a_tenth_of_a_full_trip_at_hub_prices_until_fusion_freight() {
    // Per distance: Slow burns a quarter of the energy from Uranium at 5.6 Cr;
    // Full burns it all from Deuterium at 14 Cr. Fusion Freight cuts a freight
    // formation's fusion burn to a tenth, so the two cost the same.
    let slow = crate::fuel::SLOW_FUEL_MULT * crate::market::base_price(C::Uranium);
    let full = crate::market::base_price(C::Deuterium);
    assert!((full / slow - 10.0).abs() < 1e-9);
    assert!((full * crate::fuel::FUSION_FREIGHT_MULT - slow).abs() < 1e-9);

    let (mut w, owner, _) = founded();
    let tiny = open_space(&mut w, owner, ShipKind::TinyFreighter);
    let raider = open_space(&mut w, owner, ShipKind::Raider);
    assert_eq!(w.fleets[&tiny].cruise_fuel_mult(), 1.0);
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_line_fusion_freight".into());
    w.refresh_fleet_fuel_research();
    assert_eq!(w.fleets[&tiny].cruise_fuel_mult(), crate::fuel::FUSION_FREIGHT_MULT,
        "Fusion Freight: freighters burn a tenth of the Deuterium");
    // §reactors-drives: a default Raider flies a Maelstrom, whose square rides every burn.
    assert_eq!(w.fleets[&raider].cruise_fuel_mult(), w.fleets[&raider].drive_energy_mult(), "warships are unaffected");
    let f = w.fleets.get_mut(&tiny).unwrap();
    f.transit = TransitMode::Slow;
    assert_eq!(f.cruise_fuel_mult(), crate::fuel::SLOW_FUEL_MULT, "Slow still burns a quarter on Uranium");
}

#[test]
fn a_jump_burns_antimatter_from_the_bottle_and_a_short_bottle_cancels_it() {
    let (mut w, owner, _) = founded();
    let id = open_space(&mut w, owner, ShipKind::Raider);
    let from = w.fleets[&id].pos;
    let dest = from + Vec2::new(50_000.0, 0.0);
    assert!((w.fleets[&id].jump_antimatter_cost(50_000.0) - 1.0).abs() < 1e-9,
        "an Interceptor's 50k su jump burns one Antimatter");
    {
        let f = w.fleets.get_mut(&id).unwrap();
        f.antimatter = 0.5;
        f.order = FleetOrder::Jump { dest, spool_started: Some(w.time - 100.0) };
    }
    let mut events = Vec::new();
    w.resolve_jumps(&mut events);
    assert_eq!(w.fleets[&id].pos, from, "half a unit is not a jump");
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::FuelShortfall {
        kind: crate::fuel::ShortfallKind::Jump, .. })));
    let f = &w.fleets[&id];
    assert!(matches!(f.order, FleetOrder::Idle) && !f.stalled,
        "waiting can't refill a bottle: the jump is cancelled, the fleet free to refuel");
    let (fuel, uranium) = (w.fleets[&id].fuel, w.fleets[&id].uranium);
    {
        let f = w.fleets.get_mut(&id).unwrap();
        f.antimatter = 2.0;
        f.order = FleetOrder::Jump { dest, spool_started: Some(w.time - 100.0) };
    }
    w.resolve_jumps(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!(f.pos, dest);
    assert!((f.antimatter - 1.0).abs() < 1e-9);
    assert_eq!((f.fuel, f.uranium), (fuel, uranium), "a jump burns no warp fuel");
}

#[test]
fn the_uranium_mill_needs_no_research_and_sifts_two_uranium_per_ore() {
    assert_eq!(K::UraniumMill.research_prerequisite(), None);
    let (mut w, _, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(5.0);
    // Room in storage: the mill nets units (two Uranium per ore).
    sys.stockpile.clear();
    sys.stockpile.insert(C::Provisions, 100.0);
    sys.stockpile.insert(C::MetallicOre, 100.0);
    let body = sys.bodies.iter().find(|b| !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant).unwrap().id;
    let b = sys.bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.set_tier(K::PowerPlant, 1); // §charter-v4: no free power
    b.set_tier(K::UraniumMill, 1);
    b.assignments.insert(K::UraniumMill, Assignment::crew(1));
    for _ in 0..30 {
        w.accrue_production(&mut Vec::new());
    }
    let made = stock(&w, home, C::Uranium);
    let ore = 100.0 - stock(&w, home, C::MetallicOre);
    assert!(made > 0.0);
    assert!((made - 2.0 * ore).abs() < 1e-9, "{made} Uranium from {ore} ore");
    let mill = crate::production::converter_for(K::UraniumMill).unwrap();
    assert_eq!((mill.rate, mill.inputs), (0.05 * crate::production::OUTPUT_SCALE, &[(C::MetallicOre, 0.5)][..]),
        "0.05 a second at tier I, at the output scale");
}

#[test]
fn new_hulls_leave_the_slip_empty_and_fill_at_the_yard_dock() {
    let (mut w, owner, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(0.05);
    let yard_body = sys.bodies.iter().find(|b| !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant).unwrap().id;
    let b = sys.bodies.iter_mut().find(|b| b.id == yard_body).unwrap();
    b.set_tier(K::Shipyard, 1);
    b.assignments.insert(K::Shipyard, Assignment::crew(1));
    for (c, n) in [(C::Alloys, 100.0), (C::Machinery, 40.0), (C::Polymers, 40.0), (C::FissionReactor, 4.0), (C::ZephyrDrive, 4.0),
        (C::NavComputer, 2.0), (C::HullSections, 4.0), (C::CargoBay, 2.0), (C::Composites, 10.0), (C::Electronics, 10.0),
        (C::Uranium, 100.0), (C::Deuterium, 100.0)] {
        sys.stockpile.insert(c, n);
    }
    let before: std::collections::BTreeSet<EntityId> = w.fleets.keys().copied().collect();
    w.apply_build(owner, home, Some(yard_body), BuildKind::Ship { ship: ShipKind::TinyFreighter }, None,
        Loadout::default(), &mut Vec::new());
    assert_eq!(stock(&w, home, C::FissionReactor), 2.0, "a Tiny takes two Fission Reactors (§reactors-drives)");
    assert_eq!(stock(&w, home, C::ZephyrDrive), 2.0, "and two Zephyr Drives");
    w.tick = w.build_queue[0].complete_tick;
    w.time = w.tick as f64 * DT;
    w.resolve_builds(&mut Vec::new());
    let id = *w.fleets.keys().find(|id| !before.contains(id)).expect("the hull launched");
    let f = &w.fleets[&id];
    assert_eq!((f.fuel, f.uranium, f.antimatter), (0.0, 0.0, 0.0), "no free fuel off the slip");
    assert_eq!(f.engines_aboard(), BTreeMap::from([((crate::fuel::ReactorGrade::Fission, crate::fuel::Drive::Zephyr), 1)]),
        "built on the recipe's Fission Reactor and Zephyr Drive");
    assert_eq!(w.fleets[&id].transit, TransitMode::Slow, "a civilian hull launches Slow");
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!((f.fuel, f.uranium), (0.0, f.uranium_capacity()),
        "a fission hull takes Uranium, never the colony's Deuterium");
    assert!((stock(&w, home, C::Uranium) - (100.0 - 52.5)).abs() < 1e-9);
    assert_eq!(stock(&w, home, C::Deuterium), 100.0);
    w.fleets.get_mut(&id).unwrap().transit = TransitMode::Full;
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!((f.fuel, f.fuel_capacity()), (0.0, 0.0), "the throttle changes nothing about the tank");
    assert!(!f.can_stealth(), "Stealth needs a Fusion Core or better");
}

#[test]
fn hub_bunkerage_sells_each_hull_the_fuel_its_core_burns() {
    let (mut w, owner, _) = founded();
    w.fleets.retain(|_, f| f.owner != owner);
    let hub = w.hub;
    let tiny = w.alloc_entity_id();
    w.fleets.insert(tiny, Fleet::single_on(tiny, owner, ShipKind::TinyFreighter, (crate::fuel::ReactorGrade::Fission, crate::fuel::Drive::Zephyr),
        hub, FleetOrder::Idle, None));
    let raider = w.alloc_entity_id();
    w.fleets.insert(raider, Fleet::single(raider, owner, ShipKind::Raider, hub, FleetOrder::Idle, None));
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    for id in [tiny, raider] {
        w.fleets.get_mut(&id).unwrap().launch_empty();
        assert_eq!(w.dock_of(id), Some(crate::ship::DockSite::Hub));
    }
    let mut events = Vec::new();
    w.refuel_docked(&mut events);
    let (t, r) = (&w.fleets[&tiny], &w.fleets[&raider]);
    assert!(t.fuel == 0.0 && (t.uranium - 52.5).abs() < 1e-9,
        "a fission hull buys Uranium only, rounded up to whole units (the spill is under one)");
    assert!(r.fuel == 0.0 && r.uranium == 0.0 && (r.antimatter - 2.0).abs() < 1e-9,
        "a Fold hull buys Antimatter only: its cruise tank is its jump bottle");
    let bought = |c: C| events.iter().any(|e| matches!(e.payload,
        EventPayload::HubFuelPurchased { commodity, .. } if commodity == c));
    assert!(bought(C::Uranium) && bought(C::Antimatter) && !bought(C::Deuterium));
}

#[test]
fn a_depot_rule_books_the_shortfall_once_counting_what_is_already_inbound() {
    let (mut w, owner, home) = founded();
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    // A site report the Hub can see.
    w.information.record(&w.clone(), Vec::new());
    // §reactors-drives: the Hub sells Deuterium once Bunkerage is done.
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_bunkerage".into());
    w.receive_admin_for_test(&[Command::SetDepotRule {
        player_id: owner, system: home, commodity: C::Deuterium, target: 100,
    }]);
    assert_eq!(w.players[&owner].depot_rules.len(), 1);
    let report_time = w.time;
    w.time = report_time + crate::transit::delay(
        w.systems.iter().find(|s| s.id == home).unwrap().pos, w.hub, w.config.c) + 1.0;
    let mut events = Vec::new();
    w.review_depot_rules(&mut events);
    let booked: u32 = w.freight_queue.values()
        .filter(|s| s.owner == owner && s.system == home && s.commodity == C::Deuterium)
        .map(|s| s.units).sum();
    assert_eq!(booked, 100, "the shortfall, bought and booked");
    assert_eq!(w.players[&owner].warehouse.get(&C::Deuterium).copied().unwrap_or(0), 0,
        "the lot left the warehouse for the freighter");
    w.review_depot_rules(&mut events);
    let again: u32 = w.freight_queue.values()
        .filter(|s| s.owner == owner && s.system == home && s.commodity == C::Deuterium)
        .map(|s| s.units).sum();
    assert_eq!(again, booked, "what is already inbound counts");
    w.receive_admin_for_test(&[Command::SetDepotRule {
        player_id: owner, system: home, commodity: C::Deuterium, target: 0,
    }]);
    assert!(w.players[&owner].depot_rules.is_empty(), "target 0 clears the rule");
    w.receive_admin_for_test(&[Command::SetDepotRule {
        player_id: owner, system: home, commodity: C::Alloys, target: 10,
    }]);
    assert!(w.players[&owner].depot_rules.is_empty(), "fuels only");
}

#[test]
fn autoforges_open_the_drive_works_and_a_line_makes_the_parts_its_owner_has_researched() {
    assert_eq!(K::DriveWorks.research_prerequisite(), Some("mat_autoforges"));
    let mut line = Assignment::crew(1);
    assert_eq!(crate::production::assigned_converter(K::DriveWorks, Some(&line)).unwrap().output, C::FissionReactor);
    for product in [C::ZephyrDrive, C::FusionReactor, C::GaleDrive, C::AdvancedFusionReactor, C::TempestDrive,
        C::AntimatterReactor, C::MaelstromDrive] {
        line.refining_ore = Some(product);
        assert_eq!(crate::production::assigned_converter(K::DriveWorks, Some(&line)).unwrap().output, product);
    }
    // §reactors-drives: reactor and drive of a rung share one gate.
    assert_eq!(crate::production::drive_works_gate(C::FusionReactor), (2, None), "rung 2 at Drive Works II");
    assert_eq!(crate::production::drive_works_gate(C::GaleDrive), (2, None));
    assert_eq!(crate::production::drive_works_gate(C::AdvancedFusionReactor), (1, Some(crate::power::FUSION_RESEARCH)));
    assert_eq!(crate::production::drive_works_gate(C::TempestDrive), (1, Some(crate::power::FUSION_RESEARCH)));
    assert_eq!(crate::production::drive_works_gate(C::AntimatterReactor), (1, Some("mat_foundry_vi_antimatter_synthesis")));
    assert_eq!(crate::production::drive_works_gate(C::MaelstromDrive), (1, Some("mat_foundry_vi_antimatter_synthesis")));

    let (mut w, owner, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let body = sys.bodies[0].id;
    sys.bodies[0].set_tier(K::DriveWorks, 1);
    let set = |w: &mut World, product: C| {
        w.receive_admin_for_test(&[Command::SetAssignment {
            player_id: owner, system_id: home, structure: K::DriveWorks, workers: 1,
            specialists: Default::default(), body_id: Some(body), refining_ore: Some(product),
        }]);
        w.systems.iter().find(|s| s.id == home).unwrap().bodies[0].assignments[&K::DriveWorks].refining_ore
    };
    assert_eq!(set(&mut w, C::ZephyrDrive), Some(C::ZephyrDrive));
    assert_eq!(set(&mut w, C::Alloys), Some(C::ZephyrDrive), "a Drive Works line makes reactors and drives, nothing else");
    assert_eq!(set(&mut w, C::TempestDrive), Some(C::ZephyrDrive), "Tempest Drives wait for Helium-3 Cycle");
    w.players.get_mut(&owner).unwrap().research.completed.insert(crate::power::FUSION_RESEARCH.into());
    assert_eq!(set(&mut w, C::TempestDrive), Some(C::TempestDrive));
}

#[test]
fn maelstrom_drives_let_any_hull_but_a_ship_of_the_line_jump_from_its_own_tank() {
    use crate::fuel::{Drive, ReactorGrade};
    let plain = Fleet::single(EntityId(1), PlayerId(1), ShipKind::TinyFreighter, Vec2::ZERO, FleetOrder::Idle, None);
    assert!(!plain.can_jump());
    assert_eq!(plain.antimatter_capacity(), 0.0);
    let fold = Fleet::single_on(EntityId(1), PlayerId(1), ShipKind::TinyFreighter, (crate::fuel::ReactorGrade::Antimatter, crate::fuel::Drive::Maelstrom), Vec2::ZERO,
        FleetOrder::Idle, None);
    assert!(fold.can_jump());
    assert!((fold.antimatter_capacity() - 15.0).abs() < 1e-9, "two full-range jumps of the bare hull");
    assert!((fold.antimatter - 15.0).abs() < 1e-9, "a spawned hull arrives with its tank full");
    assert!(!ShipKind::Battleship.can_mount(ReactorGrade::Antimatter, Drive::Maelstrom), "no Maelstrom for a ship of the line");
    assert!(ShipKind::Battleship.can_mount(ReactorGrade::Antimatter, Drive::Tempest), "but any reactor");
    assert!(ShipKind::Raider.can_mount(ReactorGrade::Antimatter, Drive::Maelstrom)
        && ShipKind::Raider.default_engine() == (ReactorGrade::Antimatter, Drive::Maelstrom),
        "the dark hulls spawn on Antimatter Reactors and Maelstrom Drives");
    // A mixed formation jumps only when every hull folds.
    let mut mixed = fold.clone();
    mixed.add_fitted_on(ShipKind::Scout, &Loadout::default(), (crate::fuel::ReactorGrade::Fusion, crate::fuel::Drive::Zephyr), 1);
    assert!(!mixed.can_jump());
    assert_eq!(mixed.fuels_aboard(), vec![C::Deuterium, C::Antimatter]);
}

#[test]
fn the_antimatter_plant_turns_deuterium_into_antimatter_at_ten_times_a_factorys_draw() {
    let (mut w, _, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(5.0);
    sys.stockpile.insert(C::Provisions, 1_000.0);
    sys.stockpile.insert(C::Deuterium, 100.0);
    let b = &mut sys.bodies[0];
    b.structures.clear();
    b.assignments.clear();
    b.set_tier(K::AntimatterPlant, 1);
    b.assignments.insert(K::AntimatterPlant, Assignment::crew(1));
    b.set_tier(K::PowerPlant, 1);
    let mut events = Vec::new();
    for _ in 0..30 {
        w.accrue_production(&mut events);
    }
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    assert!((sys.bodies[0].grid.demand - 10.0).abs() < 1e-9);
    let made = stock(&w, home, C::Antimatter);
    assert!(made > 0.0);
    assert!((100.0 - stock(&w, home, C::Deuterium) - 2.0 * made).abs() < 1e-9);
}

#[test]
fn a_scout_refills_a_fractional_bottle_at_the_hub() {
    // Two 50k jumps leave a Scout's one-unit bottle at 0.2: whole-unit rounding
    // down would sell it nothing and strand the jump drive.
    let (mut w, owner, _) = founded();
    w.fleets.retain(|_, f| f.owner != owner);
    let scout = w.alloc_entity_id();
    w.fleets.insert(scout, Fleet::single(scout, owner, ShipKind::Scout, w.hub, FleetOrder::Idle, None));
    w.players.get_mut(&owner).unwrap().credits = 10_000.0;
    let cost = w.fleets[&scout].jump_antimatter_cost(50_000.0);
    w.fleets.get_mut(&scout).unwrap().antimatter = 1.0 - 2.0 * cost;
    assert!((w.fleets[&scout].antimatter - 0.2).abs() < 1e-9);
    w.refuel_docked(&mut Vec::new());
    assert!((w.fleets[&scout].antimatter - 1.0).abs() < 1e-9);
    let paid = w.players[&owner].credits;
    w.refuel_docked(&mut Vec::new());
    assert_eq!(w.players[&owner].credits, paid, "a full store never buys another unit");
}

#[test]
fn a_docking_fleet_leaves_the_fusion_plants_an_hour_of_fuel() {
    let (mut w, owner, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let body = sys.bodies.iter_mut().find(|b| !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant).unwrap();
    body.set_tier(K::PowerPlant, 3);
    body.grid.burn = 0.02; // 72 Deuterium an hour
    let pos = sys.pos;
    sys.stockpile.insert(C::Deuterium, 100.0);
    w.fleets.retain(|_, f| f.owner != owner);
    let id = w.alloc_entity_id();
    w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Corvette, pos, FleetOrder::Idle, None));
    w.fleets.get_mut(&id).unwrap().launch_empty();
    // A fusion Corvette's tank: 800 mass × 0.035 × 2 (§reactors-drives: a
    // Fusion Reactor stores twice the energy) at four a unit.
    assert!((w.fleets[&id].fuel_capacity() - 14.0).abs() < 1e-9);
    w.refuel_docked(&mut Vec::new());
    assert!((w.fleets[&id].fuel - 14.0).abs() < 1e-9, "the plant can spare a whole tank");
    assert!((stock(&w, home, C::Deuterium) - 86.0).abs() < 1e-9);
    w.fleets.get_mut(&id).unwrap().fuel = 0.0;
    w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::Deuterium, 75.0);
    w.refuel_docked(&mut Vec::new());
    assert!((w.fleets[&id].fuel - 3.0).abs() < 1e-9, "it takes only what the plant can spare");
    assert!((stock(&w, home, C::Deuterium) - 72.0).abs() < 1e-9);
    w.fleets.get_mut(&id).unwrap().fuel = 0.0;
    w.refuel_docked(&mut Vec::new());
    assert_eq!(w.fleets[&id].fuel, 0.0, "the plant's hour stays put");
}

#[test]
fn ship_production_keeps_helium3_with_the_other_fuels() {
    let (mut w, owner, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for c in [C::Helium3, C::Deuterium, C::Uranium, C::Antimatter, C::Alloys] {
        sys.stockpile.insert(c, 30.0);
    }
    w.receive_admin_for_test(&[Command::ShipProduction { player_id: owner, system_id: home }]);
    let sent: Vec<C> = w.freight_queue.values().filter(|s| s.owner == owner && s.system == home)
        .map(|s| s.commodity).collect();
    assert!(sent.contains(&C::Alloys), "{sent:?}");
    assert!(sent.iter().all(|c| !crate::industry::is_depot_fuel(*c)),
        "a Fusion Plant's Helium-3 is operating fuel, like the rest: {sent:?}");
}

#[test]
fn fusion_freight_cuts_each_freighter_not_the_whole_formation() {
    let (mut w, owner, _) = founded();
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_line_fusion_freight".into());
    w.refresh_fleet_fuel_research();
    let tiny = open_space(&mut w, owner, ShipKind::TinyFreighter);
    let scout = open_space(&mut w, owner, ShipKind::Scout);
    w.refresh_fleet_fuel_research();
    let (freight, alone) = (w.fleets[&tiny].cruise_fuel_mult(), w.fleets[&scout].cruise_fuel_mult());
    assert_eq!((freight, alone), (crate::fuel::FUSION_FREIGHT_MULT, w.fleets[&scout].drive_energy_mult()),
        "a default Scout flies a Maelstrom (§reactors-drives)");
    // One Scout riding along costs its own share, as it would guarding apart.
    let merged = {
        let mut f = w.fleets[&tiny].clone();
        f.ships.extend(w.fleets[&scout].ships.iter().cloned());
        f.rebuild_cache();
        f
    };
    let (tm, sm) = (ShipKind::TinyFreighter.hull_mass(), ShipKind::Scout.hull_mass());
    let expected = (tm * crate::fuel::FUSION_FREIGHT_MULT + sm) / (tm + sm);
    assert!((merged.cruise_fuel_mult() - expected).abs() < 1e-12);
    let burn = |f: &Fleet| f.mass() * f.cruise_fuel_mult();
    // §reactors-drives: in formation the Scout rides the Tiny's Zephyr pace;
    // apart it flies its own Maelstrom, at that drive's square.
    assert!((burn(&merged) - (burn(&w.fleets[&tiny]) + burn(&w.fleets[&scout]) / w.fleets[&scout].drive_energy_mult())).abs() < 1e-9,
        "together or apart, the same fuel at the same pace");
}

#[test]
fn after_fusion_freight_only_freighters_launch_on_full() {
    let (mut w, owner, home) = founded();
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_line_fusion_freight".into());
    w.refresh_fleet_fuel_research();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(0.05);
    let yard = sys.bodies.iter().find(|b| !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant).unwrap().id;
    let b = sys.bodies.iter_mut().find(|b| b.id == yard).unwrap();
    b.set_tier(K::PowerPlant, 1); // §charter-v4: no free power
    b.set_tier(K::Shipyard, 2);
    b.assignments.insert(K::Shipyard, Assignment::crew(2));
    for (c, n) in [(C::Alloys, 500.0), (C::Machinery, 200.0), (C::Polymers, 200.0), (C::Electronics, 200.0),
        (C::FissionReactor, 10.0), (C::ZephyrDrive, 10.0), (C::Composites, 100.0), (C::NavComputer, 4.0), (C::HullSections, 20.0), (C::CargoBay, 8.0),
        (C::PrecisionComponents, 20.0), (C::PulseLaser, 12.0)] {
        sys.stockpile.insert(c, n);
    }
    let launched = |w: &mut World, ship: ShipKind| {
        let before: std::collections::BTreeSet<EntityId> = w.fleets.keys().copied().collect();
        w.apply_build(owner, home, Some(yard), BuildKind::Ship { ship }, None, Loadout::default(), &mut Vec::new());
        let job = w.build_queue.iter().find(|j| j.owner == owner).expect("queued").complete_tick;
        w.tick = job;
        w.time = w.tick as f64 * DT;
        w.resolve_builds(&mut Vec::new());
        let id = *w.fleets.keys().find(|id| !before.contains(id)).expect("launched");
        w.fleets[&id].transit
    };
    assert_eq!(launched(&mut w, ShipKind::TinyFreighter), TransitMode::Full);
    assert_eq!(launched(&mut w, ShipKind::MiningBarge), TransitMode::Slow,
        "a barge gets no Fusion Freight cut, so it stays on cheap Uranium");
}

#[test]
fn torch_regime_speeds_warships_and_taxes_only_their_cruise() {
    let (mut w, owner, _) = founded();
    let raider = open_space(&mut w, owner, ShipKind::Raider);
    let tiny = open_space(&mut w, owner, ShipKind::TinyFreighter);
    let before = (w.fleets[&raider].max_speed(), w.fleets[&tiny].max_speed(),
        w.fleets[&raider].jump_antimatter_cost(50_000.0));
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_expedition_iii_torch_regime".into());
    w.refresh_fleet_fuel_research();
    let (r, t) = (&w.fleets[&raider], &w.fleets[&tiny]);
    assert!((r.max_speed() - before.0 * 1.30).abs() < 1e-9, "the advertised warship speed");
    assert_eq!(t.max_speed(), before.1, "freighters are not warships");
    assert!((r.cruise_fuel_mult() - 1.40 * r.drive_energy_mult()).abs() < 1e-12, "the torch on a Maelstrom's square");
    assert_eq!(t.cruise_fuel_mult(), 1.0, "freighters never pay the torch");
    assert_eq!(r.jump_antimatter_cost(50_000.0), before.2, "nor does the jump fold");
    w.players.get_mut(&owner).unwrap().research.completed.insert("prop_drive_tuning".into());
    w.refresh_fleet_fuel_research();
    assert!((w.fleets[&tiny].max_speed() - before.1 * 1.10).abs() < 1e-9, "Drive Tuning speeds every hull");
}

#[test]
fn stealth_runs_quieter_than_slow() {
    let (mut w, owner, _) = founded();
    let id = open_space(&mut w, owner, ShipKind::Raider);
    let f = w.fleets.get_mut(&id).unwrap();
    f.transit = TransitMode::Slow;
    f.vel = Vec2::new(f.transit_speed(), 0.0);
    let slow = f.core_output();
    f.transit = TransitMode::Stealth;
    f.vel = Vec2::new(f.transit_speed(), 0.0);
    let stealth = f.core_output();
    assert!((stealth - slow * crate::detection::STEALTH_QUIET).abs() < 1e-12, "{slow} {stealth}");
    assert!(f.signature() < (slow).sqrt(), "and so is its signature");
}

#[test]
fn a_tender_refills_each_hull_with_the_fuel_its_reactor_burns() {
    let (mut w, owner, _) = founded();
    w.fleets.retain(|_, f| f.owner != owner);
    let pos = Vec2::new(900_000.0, 900_000.0);
    let tender = w.alloc_entity_id();
    let mut t = Fleet::single(tender, owner, ShipKind::Convoy, pos, FleetOrder::Idle, None);
    t.set_fitted(ShipKind::Convoy, &Loadout::new(vec![M::FuelTransferRig]), 1);
    t.add_cargo(C::Helium3, 10);
    t.add_cargo(C::Antimatter, 5);
    t.fuel_research = w.fuel_research_for(owner);
    let raider = w.alloc_entity_id();
    let mut r = Fleet::single(raider, owner, ShipKind::Raider, pos + Vec2::new(100.0, 0.0), FleetOrder::Idle, None);
    r.fuel_research = w.fuel_research_for(owner);
    r.launch_empty();
    w.fleets.insert(tender, t);
    w.fleets.insert(raider, r);
    let run = |w: &mut World, tender: EntityId, target: EntityId| {
        w.start_tender_transfer(owner, tender, target, &mut Vec::new());
        for _ in 0..(30.0 / DT) as u32 {
            w.time += DT;
            w.tick += 1;
            w.integrate_movement(&mut vec![]);
            w.resolve_tender_transfers();
        }
    };
    run(&mut w, tender, raider);
    let r = &w.fleets[&raider];
    assert!((r.antimatter - 2.0).abs() < 1e-9, "the Fold tank refills away from a dock: {}", r.antimatter);
    assert!(r.helium3 == 0.0 && r.fuel == 0.0, "a Fold hull takes nothing else");
    assert_eq!(w.fleets[&tender].cargo_amount(C::Helium3), 10, "the Helium-3 stays in the hold");
    // A Helium hull takes the Helium-3.
    let tiny = w.alloc_entity_id();
    let mut h = Fleet::single_on(tiny, owner, ShipKind::TinyFreighter, (crate::fuel::ReactorGrade::Helium, crate::fuel::Drive::Zephyr), pos + Vec2::new(100.0, 0.0),
        FleetOrder::Idle, None);
    h.fuel_research = w.fuel_research_for(owner);
    h.launch_empty();
    w.fleets.insert(tiny, h);
    run(&mut w, tender, tiny);
    let h = &w.fleets[&tiny];
    assert!(h.helium3 > 0.0 && (h.helium3 - h.helium3_capacity()).abs() < 1e-9, "Helium-3 fills the Helium tank: {}", h.helium3);
    assert!(h.antimatter == 0.0, "and no Antimatter");
}


/// §shipbuilding-overhaul Phase 5: magazines fill at a dock from the stockpile
/// like a tank (only what the guns aboard hold), Hardened Magazines widens the
/// rack and driver magazines but never the crystals, the Hub sells rounds like
/// fuel, and a tender reloads a fleet in the field.
#[test]
fn magazines_fill_at_the_dock_the_hub_and_from_a_tender() {
    let (mut w, owner, home) = founded();
    w.fleets.retain(|_, f| f.owner != owner);
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let pos = sys.pos;
    for (c, n) in [(C::Torpedoes, 100.0), (C::BeamCrystals, 10.0), (C::Slugs, 50.0)] {
        sys.stockpile.insert(c, n);
    }
    let id = w.alloc_entity_id();
    let mut f = Fleet::single(id, owner, ShipKind::Destroyer, pos, FleetOrder::Idle, None);
    f.set_fitted(ShipKind::Destroyer, &Loadout::new(vec![M::TorpedoRack]), 1);
    f.fuel_research = w.fuel_research_for(owner);
    f.launch_empty();
    assert!(f.has_magazines());
    assert_eq!((f.magazine_capacity(C::Torpedoes), f.magazine_capacity(C::Slugs), f.magazine_capacity(C::BeamCrystals)),
        (40.0, 0.0, 2.0), "one rack, no driver, a Pulse Laser and a Beam Battery");
    w.fleets.insert(id, f);
    assert_eq!(w.dock_of(id), Some(crate::ship::DockSite::System(home)));
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!((f.torpedoes, f.slugs, f.crystals), (40.0, 0.0, 2.0));
    assert_eq!((stock(&w, home, C::Torpedoes), stock(&w, home, C::BeamCrystals), stock(&w, home, C::Slugs)),
        (60.0, 8.0, 50.0), "the dock takes only what the guns aboard hold");
    assert_eq!(f.magazine(), crate::munitions::Magazine { torpedoes: 40.0, slugs: 0.0, crystals: 2.0, crystal_capacity: 2.0 });
    // Hardened Magazines: +25% rounds, the same crystals.
    w.players.get_mut(&owner).unwrap().research.completed.insert("weap_hardened_magazines".into());
    let research = w.fuel_research_for(owner);
    w.fleets.get_mut(&id).unwrap().fuel_research = research;
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!((f.torpedoes, f.crystals), (50.0, 2.0));
    assert_eq!(stock(&w, home, C::Torpedoes), 50.0);
    // Losing the rack's hull takes its rounds; a half-worn crystal set stays half worn.
    let mut m = f.magazine();
    m.crystals = 1.0;
    let g = w.fleets.get_mut(&id).unwrap();
    g.apply_magazine(&m);
    assert_eq!(g.crystals, 1.0, "the exact value back with the same guns");
    g.add_fitted_on(ShipKind::Destroyer, &Loadout::default(), (crate::fuel::ReactorGrade::Fusion, crate::fuel::Drive::Zephyr), 1);
    assert_eq!(g.crystals, 3.0, "a new hull brings fresh crystals for its two guns");
    g.apply_magazine(&crate::munitions::Magazine { torpedoes: 500.0, slugs: 0.0, crystals: 2.0, crystal_capacity: 4.0 });
    assert_eq!((g.torpedoes, g.crystals), (50.0, 2.0), "rounds clamp to the racks left; wear carries across the guns left");

    // The Hub sells rounds like fuel.
    let hub = w.hub;
    let at_hub = w.alloc_entity_id();
    let mut h = Fleet::single(at_hub, owner, ShipKind::Destroyer, hub, FleetOrder::Idle, None);
    h.set_fitted(ShipKind::Destroyer, &Loadout::new(vec![M::TorpedoRack]), 1);
    h.launch_empty();
    w.fleets.insert(at_hub, h);
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    let mut events = Vec::new();
    w.refuel_docked(&mut events);
    let h = &w.fleets[&at_hub];
    assert_eq!((h.torpedoes, h.crystals), (40.0, 2.0), "bought at the Hub");
    assert!(events.iter().any(|e| matches!(e.payload,
        EventPayload::HubFuelPurchased { commodity: C::Torpedoes, units: 40, .. })));
    assert!(w.players[&owner].credits < 100_000.0 - 40.0 * 72.0 * 0.9, "paid near the Torpedo price");

    // A tender reloads in the field.
    let far = Vec2::new(900_000.0, 900_000.0);
    let tender = w.alloc_entity_id();
    let mut t = Fleet::single(tender, owner, ShipKind::Convoy, far, FleetOrder::Idle, None);
    t.set_fitted(ShipKind::Convoy, &Loadout::new(vec![M::FuelTransferRig]), 1);
    t.add_cargo(C::Torpedoes, 20);
    t.add_cargo(C::BeamCrystals, 3);
    t.fuel_research = w.fuel_research_for(owner);
    let dry = w.alloc_entity_id();
    let mut d = Fleet::single(dry, owner, ShipKind::Destroyer, far + Vec2::new(100.0, 0.0), FleetOrder::Idle, None);
    d.set_fitted(ShipKind::Destroyer, &Loadout::new(vec![M::TorpedoRack]), 1);
    d.fuel_research = w.fuel_research_for(owner);
    d.fill_stores();
    d.torpedoes = 0.0;
    d.crystals = 0.0;
    w.fleets.insert(tender, t);
    w.fleets.insert(dry, d);
    w.start_tender_transfer(owner, tender, dry, &mut Vec::new());
    assert!(matches!(w.fleets[&tender].order, FleetOrder::Refuel { .. }), "a hold of rounds is a tender's cargo");
    for _ in 0..(90.0 / DT) as u32 {
        w.time += DT;
        w.tick += 1;
        w.integrate_movement(&mut vec![]);
        w.resolve_tender_transfers();
    }
    let d = &w.fleets[&dry];
    assert_eq!((d.torpedoes, d.crystals), (20.0, 2.0), "every torpedo aboard and two crystals for two guns");
    assert_eq!((w.fleets[&tender].cargo_amount(C::Torpedoes), w.fleets[&tender].cargo_amount(C::BeamCrystals)), (0, 1));
}

/// §shipbuilding-overhaul Phase 5: a fight draws the magazines down and the
/// rounds that are left go home with the hull damage, so the next fight
/// starts where this one stopped.
#[test]
fn a_battle_draws_down_the_magazines_and_writes_them_back() {
    let mut w = World::new(SimConfig::for_players(2026, 4));
    let (p, rival) = (PlayerId(4242), PlayerId(4243));
    w.step(&[Command::AddPlayer { id: p, name: "A".into() }, Command::AddPlayer { id: rival, name: "B".into() }]);
    w.enclaves.clear();
    w.fleets.clear();
    let pos = Vec2::new(900_000.0, 900_000.0);
    let own = w.alloc_entity_id();
    let mut f = Fleet::single(own, p, ShipKind::Destroyer, pos, FleetOrder::Idle, None);
    f.set_fitted(ShipKind::Destroyer, &Loadout::new(vec![M::TorpedoRack]), 1);
    f.fill_stores();
    assert_eq!((f.torpedoes, f.crystals), (40.0, 2.0), "a scripted spawn comes loaded");
    w.fleets.insert(own, f);
    let enemy = w.alloc_entity_id();
    let mut e = Fleet::single(enemy, rival, ShipKind::Corvette, pos + Vec2::new(50.0, 0.0), FleetOrder::Idle, None);
    e.add(ShipKind::Corvette, 2);
    w.fleets.insert(enemy, e);
    w.fleets.get_mut(&own).unwrap().order = FleetOrder::Attack { target: enemy };
    for _ in 0..300 {
        w.step(&[]);
        if w.fleets.get(&own).is_none_or(|f| f.torpedoes < 40.0) { break; }
    }
    let f = w.fleets.get(&own).expect("the destroyer outlasts three corvettes' opening");
    assert!(f.torpedoes < 40.0, "torpedoes flew: {}", f.torpedoes);
    assert!(f.torpedoes >= 0.0 && f.crystals <= 2.0);
    let (_, e) = w.engagements.iter().next().expect("the fight is on");
    let tac = e.tactical.as_ref().unwrap();
    assert_eq!(tac.rules_version(), 17);
    assert_eq!(tac.magazines()[&own].torpedoes, f.torpedoes, "the engine's magazine is the fleet's");
}
