//! §fuel-overhaul (docs/fuel-overhaul.md): Uranium for slow warp, Deuterium
//! for full warp, Helium-3 for range, Antimatter for jumps; the Uranium Mill,
//! Fusion Freight, empty launches, Hub bunkerage by regime, depot rules, the
//! Authority's cores and the Jump Drive.

use super::*;
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
fn the_founding_fleet_arrives_with_full_stores_and_the_kit_carries_both_fuels() {
    let (w, owner, home) = founded();
    let fleets: Vec<&Fleet> = w.fleets.values().filter(|f| f.owner == owner).collect();
    let tiny = fleets.iter().find(|f| f.contains(ShipKind::TinyFreighter)).unwrap();
    let interceptor = fleets.iter().find(|f| f.contains(ShipKind::Raider)).unwrap();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(near(tiny.fuel, 52.5) && near(tiny.uranium, 52.5) && tiny.antimatter == 0.0);
    assert!(near(interceptor.fuel, 7.0) && near(interceptor.uranium, 7.0) && near(interceptor.antimatter, 2.0));
    assert_eq!(stock(&w, home, C::Uranium), crate::fuel::URANIUM_HOME_SEED);
    assert_eq!(stock(&w, home, C::Deuterium), crate::fuel::DEUTERIUM_HOME_SEED);
    assert_eq!(stock(&w, home, C::WarpCore), 1.0, "the charter's barge needs a Warp Core");
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
    assert_eq!(w.fleets[&raider].cruise_fuel_mult(), 1.0, "warships are unaffected");
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
    assert_eq!((mill.rate, mill.inputs), (0.05, &[(C::MetallicOre, 0.5)][..]), "0.05 a second at tier I");
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
    for (c, n) in [(C::Alloys, 100.0), (C::Machinery, 40.0), (C::Polymers, 40.0), (C::WarpCore, 2.0),
        (C::Uranium, 100.0), (C::Deuterium, 100.0)] {
        sys.stockpile.insert(c, n);
    }
    let before: std::collections::BTreeSet<EntityId> = w.fleets.keys().copied().collect();
    w.apply_build(owner, home, Some(yard_body), BuildKind::Ship { ship: ShipKind::TinyFreighter }, None,
        Loadout::default(), &mut Vec::new());
    assert_eq!(stock(&w, home, C::WarpCore), 1.0, "a Tiny takes one Warp Core");
    w.tick = w.build_queue[0].complete_tick;
    w.time = w.tick as f64 * DT;
    w.resolve_builds(&mut Vec::new());
    let id = *w.fleets.keys().find(|id| !before.contains(id)).expect("the hull launched");
    let f = &w.fleets[&id];
    assert_eq!((f.fuel, f.uranium, f.antimatter), (0.0, 0.0, 0.0), "no free fuel off the slip");
    assert_eq!(w.fleets[&id].transit, TransitMode::Slow, "a civilian hull launches Slow");
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!((f.fuel, f.uranium), (0.0, f.uranium_capacity()),
        "a Slow fleet takes Uranium, never the colony's Deuterium");
    assert!((stock(&w, home, C::Uranium) - (100.0 - 52.5)).abs() < 1e-9);
    assert_eq!(stock(&w, home, C::Deuterium), 100.0);
    w.fleets.get_mut(&id).unwrap().transit = TransitMode::Full;
    w.refuel_docked(&mut Vec::new());
    let f = &w.fleets[&id];
    assert_eq!(f.fuel, f.fuel_capacity(), "switched to Full, it fills the fusion tank at the dock");
}

#[test]
fn hub_bunkerage_sells_the_regimes_fuel_and_antimatter_for_a_jump_bottle() {
    let (mut w, owner, _) = founded();
    w.fleets.retain(|_, f| f.owner != owner);
    let hub = w.hub;
    let tiny = w.alloc_entity_id();
    w.fleets.insert(tiny, Fleet::single(tiny, owner, ShipKind::TinyFreighter, hub, FleetOrder::Idle, None));
    let raider = w.alloc_entity_id();
    w.fleets.insert(raider, Fleet::single(raider, owner, ShipKind::Raider, hub, FleetOrder::Idle, None));
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    for id in [tiny, raider] {
        w.fleets.get_mut(&id).unwrap().launch_empty();
        assert_eq!(w.dock_of(id), Some(crate::ship::DockSite::Hub));
    }
    w.fleets.get_mut(&tiny).unwrap().transit = TransitMode::Slow;
    let mut events = Vec::new();
    w.refuel_docked(&mut events);
    let (t, r) = (&w.fleets[&tiny], &w.fleets[&raider]);
    assert!(t.fuel == 0.0 && (t.uranium - 52.5).abs() < 1e-9,
        "a Slow fleet buys Uranium only, rounded up to whole units (the spill is under one)");
    assert!((r.fuel - 7.0).abs() < 1e-9 && (r.uranium - 7.0).abs() < 1e-9 && (r.antimatter - 2.0).abs() < 1e-9,
        "Full: Deuterium, the Uranium store an escort may need, and the bottle");
    let bought = |c: C| events.iter().any(|e| matches!(e.payload,
        EventPayload::HubFuelPurchased { commodity, .. } if commodity == c));
    assert!(bought(C::Uranium) && bought(C::Deuterium) && bought(C::Antimatter));
}

#[test]
fn a_depot_rule_books_the_shortfall_once_counting_what_is_already_inbound() {
    let (mut w, owner, home) = founded();
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    // A site report the Hub can see.
    w.information.record(&w.clone(), Vec::new());
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
    assert_eq!(booked, 100 - crate::fuel::DEUTERIUM_HOME_SEED as u32, "the shortfall, bought and booked");
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
fn fusion_reactors_open_the_drive_works_and_a_line_can_make_fusion_cores() {
    assert_eq!(K::DriveWorks.research_prerequisite(), Some(crate::power::FUSION_RESEARCH));
    let mut fusion_line = Assignment::crew(1);
    assert_eq!(crate::production::assigned_converter(K::DriveWorks, Some(&fusion_line)).unwrap().output, C::WarpCore);
    fusion_line.refining_ore = Some(C::FusionCore);
    assert_eq!(crate::production::assigned_converter(K::DriveWorks, Some(&fusion_line)).unwrap().output, C::FusionCore);

    let (mut w, owner, home) = founded();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let body = sys.bodies[0].id;
    sys.bodies[0].set_tier(K::DriveWorks, 1);
    w.receive_admin_for_test(&[Command::SetAssignment {
        player_id: owner, system_id: home, structure: K::DriveWorks, workers: 1,
        specialists: Default::default(), body_id: Some(body), refining_ore: Some(C::FusionCore),
    }]);
    let line = &w.systems.iter().find(|s| s.id == home).unwrap().bodies[0].assignments[&K::DriveWorks];
    assert_eq!(line.refining_ore, Some(C::FusionCore));
    w.receive_admin_for_test(&[Command::SetAssignment {
        player_id: owner, system_id: home, structure: K::DriveWorks, workers: 1,
        specialists: Default::default(), body_id: Some(body), refining_ore: Some(C::Alloys),
    }]);
    let line = &w.systems.iter().find(|s| s.id == home).unwrap().bodies[0].assignments[&K::DriveWorks];
    assert_eq!(line.refining_ore, Some(C::FusionCore), "a Drive Works line makes cores, nothing else");
}

#[test]
fn a_jump_drive_lets_a_freighter_jump_from_its_own_bottle() {
    let jump = Loadout::new(vec![M::JumpDrive]);
    assert!(jump.validate(ShipKind::TinyFreighter));
    assert!(!jump.validate(ShipKind::Battleship), "no fold for a ship of the line");
    assert!(!jump.validate(ShipKind::Raider), "built-in drives need no module");
    let mut f = Fleet::single(EntityId(1), PlayerId(1), ShipKind::TinyFreighter, Vec2::ZERO, FleetOrder::Idle, None);
    assert!(!f.can_jump());
    assert_eq!(f.antimatter_capacity(), 0.0);
    f.set_fitted(ShipKind::TinyFreighter, &jump, 1);
    assert!(f.can_jump());
    assert!((f.antimatter_capacity() - 15.0).abs() < 1e-9, "two full-range jumps of the bare hull");
    let research = crate::research::programme("mat_foundry_vi_antimatter_synthesis").unwrap();
    assert!(research.effects.contains(&crate::research::Effect::UnlockModule(M::JumpDrive)));
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
    w.refuel_docked(&mut Vec::new());
    assert!((w.fleets[&id].fuel - 28.0).abs() < 1e-9, "it takes only what the plant can spare");
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
    assert_eq!((freight, alone), (crate::fuel::FUSION_FREIGHT_MULT, 1.0));
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
    assert!((burn(&merged) - (burn(&w.fleets[&tiny]) + burn(&w.fleets[&scout]))).abs() < 1e-9,
        "together or apart, the same fuel");
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
    b.set_tier(K::Shipyard, 2);
    b.assignments.insert(K::Shipyard, Assignment::crew(2));
    for (c, n) in [(C::Alloys, 500.0), (C::Machinery, 200.0), (C::Polymers, 200.0), (C::Electronics, 200.0),
        (C::WarpCore, 10.0), (C::Composites, 100.0)] {
        sys.stockpile.insert(c, n);
    }
    let mut launched = |w: &mut World, ship: ShipKind| {
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
    assert!((r.cruise_fuel_mult() - 1.40).abs() < 1e-12);
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
    let slow = f.signature();
    f.transit = TransitMode::Stealth;
    f.vel = Vec2::new(f.transit_speed(), 0.0);
    let stealth = f.signature();
    assert!((stealth - slow * crate::detection::STEALTH_QUIET).abs() < 1e-12, "{slow} {stealth}");
}

#[test]
fn a_tender_refills_a_bottle_and_a_fusion_tank_with_helium3() {
    let (mut w, owner, _) = founded();
    w.players.get_mut(&owner).unwrap().research.completed.insert(crate::power::FUSION_RESEARCH.into());
    w.refresh_fleet_fuel_research();
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
    r.fuel = 0.0;
    r.antimatter = 0.0;
    w.fleets.insert(tender, t);
    w.fleets.insert(raider, r);
    w.start_tender_transfer(owner, tender, raider, &mut Vec::new());
    for _ in 0..(30.0 / DT) as u32 {
        w.time += DT;
        w.tick += 1;
        w.integrate_movement(&mut vec![]);
        w.resolve_tender_transfers();
    }
    let r = &w.fleets[&raider];
    assert!((r.antimatter - 2.0).abs() < 1e-9, "the bottle refills away from a dock: {}", r.antimatter);
    assert!(r.helium3 > 0.0 && r.fuel > 0.0, "Helium-3 cargo tops up the fusion tank");
}
