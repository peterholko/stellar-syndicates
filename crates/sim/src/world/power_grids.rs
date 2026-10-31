//! §power: per-planet grids — the Command Center's reactor, the plant,
//! brownouts and fusion. §fuel-overhaul: Solar Arrays (I–II) burn nothing;
//! Fusion Plants burn Deuterium (III–IV) and Helium-3 (V–VI). §charter-v4: no
//! planet has free power; a Command Center powers its own planet with 1 MW.

use super::*;
use crate::body::{Body, BodyKind};
use crate::build::StructureKind as K;
use crate::cargo::Commodity as C;
use crate::power::{FUEL_PER_MW_S, Grid};
use crate::production::Assignment;

/// A home with every structure cleared (the Command Center too), crews to
/// spare and a full larder, on v4 power.
fn grid_home() -> (World, PlayerId, EntityId, Vec<u32>) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    w.legacy_test_bootstrap = true;
    w.strict_power = true;
    w.enclaves.clear();
    let owner = PlayerId(5720);
    w.step(&[Command::AddPlayer { id: owner, name: "Grid".into() }]);
    let home = w.players[&owner].home_system.unwrap();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for body in &mut sys.bodies {
        body.structures.clear();
        body.assignments.clear();
    }
    sys.set_population(5.0);
    sys.stockpile.clear();
    sys.stockpile.insert(C::Provisions, 1e6);
    let solid = sys.bodies.iter().filter(|b| !b.is_belt() && b.kind != BodyKind::GasGiant).map(|b| b.id).collect();
    (w, owner, home, solid)
}

fn body_mut(w: &mut World, home: EntityId, id: u32) -> &mut Body {
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == id).unwrap()
}

/// Build `kind` at `tier` on a body, fully crewed.
fn place(w: &mut World, home: EntityId, id: u32, kind: K, tier: u32) {
    let b = body_mut(w, home, id);
    b.set_tier(kind, tier);
    b.assignments.insert(kind, Assignment::crew(tier));
}

fn grid(w: &World, home: EntityId, id: u32) -> Grid {
    w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| b.id == id).unwrap().grid
}

fn stock(w: &World, home: EntityId, c: C) -> f64 {
    w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
}

fn set_stock(w: &mut World, home: EntityId, c: C, n: f64) {
    w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(c, n);
}

fn run(w: &mut World) -> Vec<Event> {
    let mut events = Vec::new();
    w.accrue_production(&mut events);
    events
}

#[test]
fn no_planet_has_free_power_but_a_command_center_runs_a_mine() {
    let (mut w, owner, home, solid) = grid_home();
    // The landing world: its Command Center's reactor runs a Mining Complex I.
    body_mut(&mut w, home, solid[1]).set_tier(K::CommandCenter, 1);
    place(&mut w, home, solid[1], K::MiningComplex, 1);
    place(&mut w, home, solid[1], K::Agroplex, 1);
    // A sibling world with work but no power source: nothing powered runs.
    place(&mut w, home, solid[0], K::MiningComplex, 1);
    let events = run(&mut w);
    assert_eq!(grid(&w, home, solid[1]).demand, 1.0, "food draws nothing");
    assert_eq!(grid(&w, home, solid[1]).power, 1.0);
    assert_eq!(grid(&w, home, solid[0]).power, 0.0, "no Command Center, no plant");
    assert!(events.iter().any(|e| matches!(e.payload,
        EventPayload::Brownout { owner: o, body, .. } if o == owner && body == solid[0])));
    // A Shipyard beside the reactor's mine needs a plant.
    place(&mut w, home, solid[1], K::Shipyard, 1);
    run(&mut w);
    assert_eq!(grid(&w, home, solid[1]).power, 0.5);
}

#[test]
fn a_planet_past_its_base_browns_out_until_a_solar_array_lifts_it() {
    let (mut w, owner, home, solid) = grid_home();
    let id = solid[0];
    body_mut(&mut w, home, id).set_tier(K::CommandCenter, 1);
    place(&mut w, home, id, K::MiningComplex, 2);
    place(&mut w, home, id, K::Shipyard, 2);
    let events = run(&mut w);
    let g = grid(&w, home, id);
    assert!((g.demand - 4.4).abs() < 1e-9);
    assert_eq!(g.power, 0.22, "the reactor's 1 MW of 4.4");
    assert!(events.iter().any(|e| matches!(e.payload,
        EventPayload::Brownout { owner: o, body, no_fuel: false, .. } if o == owner && body == id)));
    // A Solar Array needs no fuel at all: full power, nothing burned.
    body_mut(&mut w, home, id).set_tier(K::PowerPlant, 1);
    let events = run(&mut w);
    let g = grid(&w, home, id);
    assert_eq!((g.power, g.burn, g.no_fuel), (1.0, 0.0, false));
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::PowerRestored { body, .. } if body == id)));
}

#[test]
fn a_fusion_plant_browns_out_until_it_has_deuterium() {
    let (mut w, owner, home, solid) = grid_home();
    let id = solid[0];
    // 6 + 6 + 2 × 6 (the Smelter draws double) = 24 MW against a dry III.
    place(&mut w, home, id, K::MiningComplex, 4);
    place(&mut w, home, id, K::Shipyard, 4);
    place(&mut w, home, id, K::Smelter, 4);
    body_mut(&mut w, home, id).set_tier(K::PowerPlant, 3);
    let events = run(&mut w);
    let g = grid(&w, home, id);
    assert!((g.demand - 24.0).abs() < 1e-9, "{}", g.demand);
    assert!(g.no_fuel && g.power == 0.73, "only the kept solar fields: 17.6 / 24");
    assert!(events.iter().any(|e| matches!(e.payload,
        EventPayload::Brownout { owner: o, body, no_fuel: true, .. } if o == owner && body == id)));
    set_stock(&mut w, home, C::Deuterium, 100.0);
    let events = run(&mut w);
    let g = grid(&w, home, id);
    assert_eq!(g.power, 1.0);
    assert!((g.burn - 6.4 * FUEL_PER_MW_S).abs() < 1e-9, "burns only the load above the solar fields");
    assert!((stock(&w, home, C::Deuterium) - (100.0 - 6.4 * FUEL_PER_MW_S * DT)).abs() < 1e-9);
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::PowerRestored { body, .. } if body == id)));
}

#[test]
fn planets_never_share_power() {
    let (mut w, _, home, solid) = grid_home();
    place(&mut w, home, solid[0], K::MiningComplex, 2);
    place(&mut w, home, solid[0], K::Shipyard, 2);
    body_mut(&mut w, home, solid[1]).set_tier(K::PowerPlant, 3);
    set_stock(&mut w, home, C::Deuterium, 100.0);
    run(&mut w);
    assert_eq!(grid(&w, home, solid[0]).power, 0.0, "a plant next door doesn't help");
    assert_eq!(grid(&w, home, solid[1]).burn, 0.0, "an idle plant burns nothing");
    assert_eq!(stock(&w, home, C::Deuterium), 100.0);
}

#[test]
fn a_brownout_slows_the_planets_factories_by_its_share() {
    // The same Fuel Refinery, browned out and then fully powered: its input
    // draw (output per Volatile is fixed) falls by exactly the grid's share.
    let draw = |powered: bool| {
        let (mut w, _, home, solid) = grid_home();
        body_mut(&mut w, home, solid[0]).set_tier(K::CommandCenter, 1);
        place(&mut w, home, solid[0], K::FuelRefinery, 2);
        place(&mut w, home, solid[0], K::MiningComplex, 2);
        if powered {
            body_mut(&mut w, home, solid[0]).set_tier(K::PowerPlant, 1);
        }
        set_stock(&mut w, home, C::Volatiles, 1_000.0);
        run(&mut w);
        assert_eq!(grid(&w, home, solid[0]).power, if powered { 1.0 } else { 0.22 });
        1_000.0 - stock(&w, home, C::Volatiles)
    };
    let (dim, full) = (draw(false), draw(true));
    assert!(full > 0.0);
    assert!((dim / full - 0.22).abs() < 1e-9, "{dim} / {full}");
}

#[test]
fn a_dry_colony_still_refines_its_own_fuel() {
    // No deadlock: the kept solar fields keep the refinery turning, and the
    // fusion plant burns the Deuterium it makes.
    // §fuel-overhaul: 23 MW, past the plant's 17.6 MW of kept solar.
    let (mut w, _, home, solid) = grid_home();
    place(&mut w, home, solid[0], K::FuelRefinery, 6);
    place(&mut w, home, solid[0], K::MiningComplex, 6);
    body_mut(&mut w, home, solid[0]).set_tier(K::PowerPlant, 3);
    set_stock(&mut w, home, C::Volatiles, 1_000.0);
    run(&mut w);
    let first = grid(&w, home, solid[0]);
    assert!(first.no_fuel && first.power > 0.0 && first.power < 1.0);
    for _ in 0..30 { run(&mut w); }
    assert!(grid(&w, home, solid[0]).power > first.power, "its own Deuterium lifts the grid");
}

#[test]
fn the_fusion_tiers_need_fusion_reactors_and_burn_helium3() {
    use crate::build::max_buildable_tier;
    assert_eq!(max_buildable_tier(K::PowerPlant, 0), 4, "Deuterium plants to tier IV");
    let mut research = crate::research::ResearchState::default();
    research.completed.insert(crate::power::FUSION_RESEARCH.into());
    let unlocked = crate::research::unlocked_structure_tier(&research, K::PowerPlant);
    assert_eq!(max_buildable_tier(K::PowerPlant, unlocked), 6, "Fusion Reactors opens V–VI");
    assert!(crate::build::recipe_for_tier(crate::build::BuildKind::Upgrade { upgrade: K::PowerPlant }, 5)
        .costs.iter().any(|(c, _)| *c == C::Iridium), "fusion walls take Iridium");

    let (mut w, _, home, solid) = grid_home();
    // 5 × 11.5 + the Smelter's double 23 = 80.5 MW.
    for kind in [K::Smelter, K::PrecisionWorks, K::ElectronicsFabricator,
        K::CompositeWorks, K::HullFabricator, K::ArmamentsComplex] {
        place(&mut w, home, solid[0], kind, 6);
    }
    body_mut(&mut w, home, solid[0]).set_tier(K::PowerPlant, 6);
    set_stock(&mut w, home, C::Deuterium, 100.0);
    set_stock(&mut w, home, C::Helium3, 100.0);
    run(&mut w);
    let g = grid(&w, home, solid[0]);
    assert!((g.demand - 80.5).abs() < 1e-9, "{}", g.demand);
    assert_eq!(g.power, 1.0);
    assert_eq!(stock(&w, home, C::Deuterium), 100.0, "Helium-3 fusion burns no Deuterium");
    let burned = (80.5 - 17.6) * FUEL_PER_MW_S / crate::fuel::HELIUM3_FUEL_EQUIV * DT;
    assert!((100.0 - stock(&w, home, C::Helium3) - burned).abs() < 1e-9);
}

#[test]
fn a_plant_takes_no_slot_and_stands_on_gas_giants_but_not_belts() {
    let (mut w, owner, home, solid) = grid_home();
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    let belt = sys.bodies.iter().find(|b| b.is_belt()).unwrap().id;
    // §asteroid-quarry: a belt's power is its quarry's own.
    let giant = match sys.bodies.iter().find(|b| b.kind == BodyKind::GasGiant) {
        Some(b) => b.id,
        None => {
            body_mut(&mut w, home, solid[1]).kind = BodyKind::GasGiant;
            solid[1]
        }
    };
    for (c, n) in [(C::Alloys, 100.0), (C::Machinery, 40.0), (C::Electronics, 40.0)] {
        set_stock(&mut w, home, c, n);
    }
    let develop = |w: &mut World, body| {
        let mut events = Vec::new();
        w.apply_now(&Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: K::PowerPlant }, &mut events);
        events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { .. } | EventPayload::IndustryRejected { .. }))
    };
    assert!(develop(&mut w, belt), "a belt refuses a plant");
    assert!(!develop(&mut w, giant), "a gas giant takes one");
    assert!(w.build_queue.iter().any(|j| j.body_id == giant
        && j.what == crate::build::BuildKind::Upgrade { upgrade: K::PowerPlant }));
    assert!(!w.build_queue.iter().any(|j| j.body_id == belt));
    assert!(!K::PowerPlant.uses_slot());
}

#[test]
fn the_ladder_is_solar_then_fusion_with_the_authoritys_cores() {
    use crate::build::{recipe_for_tier, BuildKind};
    let plant = BuildKind::Upgrade { upgrade: K::PowerPlant };
    let cores = |tier| recipe_for_tier(plant, tier).costs.iter()
        .find(|(c, _)| *c == C::FusionCore).map(|(_, n)| *n).unwrap_or(0.0);
    assert_eq!((cores(1), cores(2), cores(3), cores(4), cores(5), cores(6)), (0.0, 0.0, 1.0, 1.0, 1.0, 1.0));
    assert_eq!(K::PowerPlant.tier_title(2), "Solar Array");
    assert_eq!(K::PowerPlant.tier_title(3), "Fusion Plant");
    assert_eq!(crate::power::plant_fuel(2), None);
    assert_eq!(crate::power::plant_fuel(4), Some(C::Deuterium));
    assert_eq!(crate::power::plant_fuel(6), Some(C::Helium3));
}
