//! §gaia: Living Terraces house people and grow Provisions without Biomass,
//! on Gaia worlds only.

use super::*;
use crate::body::Environment;
use crate::build::StructureKind as K;
use crate::cargo::Commodity;
use crate::production::Assignment;

/// A home with its garden world cleared down to one Warehouse, a little food
/// and building goods — no Biomass anywhere.
fn garden_home() -> (World, PlayerId, EntityId, u32) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    w.legacy_test_bootstrap = true;
    w.enclaves.clear();
    let owner = PlayerId(5710);
    w.step(&[Command::AddPlayer { id: owner, name: "Gardeners".into() }]);
    let home = w.players[&owner].home_system.unwrap();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for body in &mut sys.bodies {
        body.structures.clear();
        body.assignments.clear();
        body.population = 0.0;
    }
    let garden = sys.bodies.iter().find(|b| b.habitable).unwrap().id;
    let world = sys.bodies.iter_mut().find(|b| b.id == garden).unwrap();
    world.set_tier(K::Warehouse, 1);
    world.population = 0.002;
    sys.stockpile.clear();
    for (c, n) in [(Commodity::Alloys, 100.0), (Commodity::Polymers, 100.0), (Commodity::Machinery, 40.0),
        (Commodity::Biomass, 40.0), (Commodity::Provisions, 50.0)] {
        sys.stockpile.insert(c, n);
    }
    (w, owner, home, garden)
}

fn world_mut(w: &mut World, home: EntityId, body: u32) -> &mut crate::body::Body {
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap()
}

fn stock(w: &World, home: EntityId, c: Commodity) -> f64 {
    w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
}

#[test]
fn living_terraces_grow_only_on_gaia_worlds() {
    let (mut w, owner, home, garden) = garden_home();
    assert_ne!(world_mut(&mut w, home, garden).profile.environment, Environment::Gaia, "a home garden is Terran");
    let develop = Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(garden), upgrade: K::LivingTerraces };
    let before = w.build_queue.len();
    let events = w.step(std::slice::from_ref(&develop));
    assert_eq!(w.build_queue.len(), before, "a Terran world is refused");
    assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, reason, .. }
        if *o == owner && reason.contains("Gaia"))), "the refusal names the rule: {events:?}");
    assert_eq!(stock(&w, home, Commodity::Biomass), 40.0, "a refused order costs nothing");

    world_mut(&mut w, home, garden).profile.environment = Environment::Gaia;
    w.step(&[develop]);
    assert_eq!(w.build_queue.len(), before + 1, "a Gaia world takes the order");
    assert_eq!(stock(&w, home, Commodity::Biomass), 20.0, "paid in living soil among the rest");
}

#[test]
fn living_terraces_house_people_beside_a_habitat() {
    let (mut w, _, home, garden) = garden_home();
    let world = world_mut(&mut w, home, garden);
    world.profile.environment = Environment::Gaia;
    world.set_tier(K::Habitat, 2);
    let habitat_only = world.population_capacity();
    world.set_tier(K::LivingTerraces, 3);
    let mult = world.habitat_capacity_mult();
    let added = world.population_capacity() - habitat_only;
    assert!((added - 3.0 * crate::colony::TERRACE_POP_PER_TIER * mult).abs() < 1e-12,
        "each tier houses 20,000 × size × Gaia: {added}");
    assert!(crate::colony::TERRACE_POP_PER_TIER < crate::colony::POP_CAP_PER_HABITAT_TIER,
        "fewer people per tier than a Habitat; their job is food");
}

#[test]
fn crewed_living_terraces_grow_provisions_without_biomass() {
    let (mut w, _, home, garden) = garden_home();
    w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.remove(&Commodity::Biomass);
    let world = world_mut(&mut w, home, garden);
    world.profile.environment = Environment::Gaia;
    world.set_tier(K::LivingTerraces, 1);

    // Unstaffed, they grow nothing: the colony only eats.
    let start = stock(&w, home, Commodity::Provisions);
    for _ in 0..TICK_HZ * 10 { w.step(&[]); }
    let unstaffed = stock(&w, home, Commodity::Provisions);
    assert!(unstaffed < start, "nothing produces unstaffed ({start} → {unstaffed})");

    // One crew farms tier I at the Agroplex's rate, with no input basket.
    world_mut(&mut w, home, garden).assignments.insert(K::LivingTerraces, Assignment { workers: 1, ..Default::default() });
    for _ in 0..TICK_HZ * 10 { w.step(&[]); }
    let staffed = stock(&w, home, Commodity::Provisions);
    let eaten_10s = start - unstaffed;
    let grown = staffed - unstaffed + eaten_10s;
    assert!((grown - 12.0).abs() < 0.5, "≈1.2 Provisions/s for 10 s at tier I: {grown}");
    assert_eq!(stock(&w, home, Commodity::Biomass), 0.0, "no Biomass drawn or needed");
}

#[test]
fn full_storage_idles_living_terraces() {
    let (mut w, _, home, garden) = garden_home();
    let world = world_mut(&mut w, home, garden);
    world.profile.environment = Environment::Gaia;
    world.set_tier(K::LivingTerraces, 1);
    world.assignments.insert(K::LivingTerraces, Assignment { workers: 1, ..Default::default() });
    // Fill the warehouse to its cap with anything but food.
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let room = sys.storage_headroom();
    *sys.stockpile.entry(Commodity::Alloys).or_insert(0.0) += room;
    assert!(sys.storage_headroom() <= 1e-9);
    let start = stock(&w, home, Commodity::Provisions);
    for _ in 0..TICK_HZ * 10 { w.step(&[]); }
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    assert!(sys.storage_headroom() >= -1e-9, "terraces never grow past the storage cap");
    assert!(stock(&w, home, Commodity::Provisions) <= start, "a full warehouse idles the terraces");
}
