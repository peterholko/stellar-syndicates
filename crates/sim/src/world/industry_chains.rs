//! Integration checks for component production, its unlocks, and physical goods.
use super::*;
use crate::build::{BuildKind, StructureKind as K};
use crate::cargo::Commodity as C;
use crate::production::{Assignment, CONVERTERS};

const FACTORIES: [K; 4] = [K::CompositeWorks, K::HullFabricator, K::PrecisionWorks, K::DriveWorks];
const GOODS: [C; 4] = [C::Composites, C::HullSections, C::PrecisionComponents, C::DriveAssemblies];

fn rig() -> (World, PlayerId, EntityId) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    w.legacy_test_bootstrap = true;
    w.enclaves.clear();
    let owner = PlayerId(5800);
    w.step(&[Command::AddPlayer { id: owner, name: "Component industry".into() }]);
    let home = w.players[&owner].home_system.unwrap();
    let s = site_mut(&mut w, home);
    for body in &mut s.bodies {
        body.assignments.clear();
        body.structures.clear();
        body.deposits.clear();
    }
    s.set_population(0.05);
    s.seed_warehouse(); // production/freight fixtures retain the founding store
    s.stockpile.clear();
    s.stockpile.insert(C::Provisions, 100.0);
    (w, owner, home)
}

fn site(w: &World, id: EntityId) -> &StarSystem {
    w.systems.iter().find(|s| s.id == id).unwrap()
}
fn site_mut(w: &mut World, id: EntityId) -> &mut StarSystem {
    w.systems.iter_mut().find(|s| s.id == id).unwrap()
}
fn stock(w: &World, id: EntityId, good: C) -> f64 {
    site(w, id).stockpile.get(&good).copied().unwrap_or(0.0)
}

#[test]
fn component_factories_require_workers_and_consume_the_exact_input_basket() {
    for kind in FACTORIES {
        let (mut w, _, home) = rig();
        let recipe = crate::production::converter_for(kind).unwrap();
        let s = site_mut(&mut w, home);
        s.set_tier(kind, 1);
        for (good, _) in recipe.inputs { s.stockpile.insert(*good, 50.0); }
        w.accrue_production(&mut Vec::new());
        assert_eq!(stock(&w, home, recipe.output), 0.0, "{kind:?} needs workers");
        for (good, _) in recipe.inputs { assert_eq!(stock(&w, home, *good), 50.0); }

        let s = site_mut(&mut w, home);
        s.assign(kind, Assignment::crew(1));
        let body = s.bodies.iter().find(|b| b.tier(kind) > 0).unwrap();
        let yield_mult = crate::explore::converter_site_mult(body, kind, s.trait_);
        w.accrue_production(&mut Vec::new());
        let output = stock(&w, home, recipe.output);
        assert!(output > 0.0, "{kind:?} must actually produce");
        for (good, per) in recipe.inputs {
            let drawn = 50.0 - stock(&w, home, *good);
            assert!((drawn - output / yield_mult * per).abs() < 1e-9, "{kind:?}: {good:?}");
        }

        // A missing input suspends the whole basket: no partial destruction of
        // the remaining inputs, and no free output. Restocking resumes it.
        let missing = recipe.inputs[0].0;
        site_mut(&mut w, home).stockpile.remove(&missing);
        w.accrue_production(&mut Vec::new());
        assert_eq!(stock(&w, home, recipe.output), output);
        site_mut(&mut w, home).stockpile.insert(missing, 50.0);
        w.accrue_production(&mut Vec::new());
        assert!(stock(&w, home, recipe.output) > output);
    }
}

#[test]
fn component_research_gates_factory_receipt_without_a_syndicate() {
    for kind in FACTORIES {
        let (mut w, owner, home) = rig();
        assert!(w.players[&owner].syndicate.is_none());
        let s = site_mut(&mut w, home);
        let body = s.site_for(kind).unwrap();
        for (good, _) in crate::build::recipe_for(BuildKind::Upgrade { upgrade: kind }).costs {
            s.stockpile.insert(*good, 1000.0);
        }
        let command = Command::DevelopSystem { player_id: owner, system_id: home,
            upgrade: kind, body_id: Some(body) };
        let before = site(&w, home).stockpile.clone();
        let mut events = Vec::new();
        // At receipt: the normal admin queue already has delay-specific tests.
        w.apply_now(&command, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
            reason: crate::event::BuildRejectReason::NeedsResearch, .. })), "{kind:?}");
        assert!(w.build_queue.is_empty());
        assert_eq!(site(&w, home).stockpile, before);

        w.players.get_mut(&owner).unwrap().research.completed
            .insert(kind.research_prerequisite().unwrap().into());
        w.apply_now(&command, &mut Vec::new());
        assert_eq!(w.build_queue.len(), 1, "{kind:?} unlock must admit its factory");
        assert_eq!(w.build_queue[0].what, BuildKind::Upgrade { upgrade: kind });
    }
}

#[test]
fn all_structure_unlocks_gate_receipt_until_research_is_completed() {
    let gated: Vec<_> = K::ALL.into_iter().filter(|kind| kind.research_prerequisite().is_some()).collect();
    assert_eq!(gated.len(), 18);
    for kind in gated {
        let (mut w, owner, home) = rig();
        let research = kind.research_prerequisite().unwrap();
        assert!(w.players[&owner].syndicate.is_none());
        let s = site_mut(&mut w, home);
        if kind == K::VolatileHarvester {
            s.bodies[0].deposits.push(crate::galaxy::Deposit {
                resource: C::Volatiles, richness: 1.0, reserves: None, accessibility: 0.0,
            });
        }
        if let Some((yard, tier)) = crate::build::yard_prereq(kind) { s.set_tier(yard, tier); }
        let body = s.site_for(kind).unwrap();
        // Isolate research from local slot scarcity: civic buildings may share
        // their target world with the founding ground Warehouse.
        if kind.slot_pool() == crate::build::SlotPool::Infrastructure {
            s.bodies.iter_mut().find(|b| b.id == body).unwrap().population = crate::build::POP_DEVELOPED;
        }
        for (good, _) in crate::build::recipe_for(BuildKind::Upgrade { upgrade: kind }).costs {
            s.stockpile.insert(*good, 1000.0);
        }
        let command = Command::DevelopSystem { player_id: owner, system_id: home,
            upgrade: kind, body_id: Some(body) };
        let stock_before = site(&w, home).stockpile.clone();
        // Queued/active research is not a blueprint. Completion at command
        // center, not its predicted finish clock, is the construction gate.
        let rs = &mut w.players.get_mut(&owner).unwrap().research;
        rs.active = Some(research.into());
        rs.progress = crate::research::cost_of(research) - 1.0;
        let mut events = Vec::new();
        w.apply_now(&command, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
            reason: crate::event::BuildRejectReason::NeedsResearch, .. })), "{kind:?}: no bypass");
        assert!(w.build_queue.is_empty(), "{kind:?}");
        assert_eq!(site(&w, home).stockpile, stock_before, "{kind:?}: rejected job spends nothing");

        let rs = &mut w.players.get_mut(&owner).unwrap().research;
        rs.progress = crate::research::cost_of(research);
        assert_eq!(rs.try_complete().as_deref(), Some(research));
        let mut events = Vec::new();
        w.apply_now(&command, &mut events);
        assert_eq!(w.build_queue.len(), 1, "{kind:?}: completed research admits construction: {events:?}");
        assert_eq!(w.build_queue[0].what, BuildKind::Upgrade { upgrade: kind });
        assert_ne!(site(&w, home).stockpile, stock_before, "{kind:?}: admitted job pays normally");
    }
}

#[test]
fn tier_research_does_not_bypass_the_initial_structure_blueprint() {
    let (mut w, owner, home) = rig();
    w.players.get_mut(&owner).unwrap().research.completed.insert("comp_deep_space_arrays".into());
    let s = site_mut(&mut w, home);
    for (good, _) in crate::build::recipe_for(BuildKind::Upgrade { upgrade: K::SensorArray }).costs {
        s.stockpile.insert(*good, 1000.0);
    }
    let command = Command::DevelopSystem { player_id: owner, system_id: home,
        upgrade: K::SensorArray, body_id: s.site_for(K::SensorArray) };
    let mut events = Vec::new();
    w.apply_now(&command, &mut events);
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
        reason: crate::event::BuildRejectReason::NeedsResearch, .. })));
    assert!(w.build_queue.is_empty());
    w.players.get_mut(&owner).unwrap().research.completed.insert("comp_sensor_gain".into());
    w.apply_now(&command, &mut Vec::new());
    assert_eq!(w.build_queue.len(), 1);
    assert_eq!(w.research_struct_tier(owner, K::SensorArray), 4, "later research still raises the ceiling");
}

#[test]
fn existing_unresearched_industry_survives_a_save_and_keeps_producing() {
    let (mut w, owner, home) = rig();
    let s = site_mut(&mut w, home);
    s.set_tier(K::Smelter, 1);
    s.assign(K::Smelter, Assignment::crew(1));
    s.stockpile.insert(C::MetallicOre, 100.0);
    s.stockpile.insert(C::Fuel, 100.0);
    w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    w.fixup_after_load();
    assert!(!w.players[&owner].research.has("mat_enrichment"));
    w.accrue_production(&mut Vec::new());
    assert!(stock(&w, home, C::Alloys) > 0.0, "the new gate never shuts existing industry down");
    assert_eq!(site(&w, home).tier(K::Smelter), 1);
}

#[test]
fn paid_structure_jobs_do_not_recheck_new_research_gates_on_load() {
    let (mut w, owner, home) = rig();
    w.players.get_mut(&owner).unwrap().research.completed.insert("mat_enrichment".into());
    let s = site_mut(&mut w, home);
    for (good, _) in crate::build::recipe_for(BuildKind::Upgrade { upgrade: K::Smelter }).costs {
        s.stockpile.insert(*good, 1000.0);
    }
    let command = Command::DevelopSystem { player_id: owner, system_id: home,
        upgrade: K::Smelter, body_id: s.site_for(K::Smelter) };
    w.apply_now(&command, &mut Vec::new());
    assert_eq!(w.build_queue.len(), 1);
    let paid_stock = site(&w, home).stockpile.clone();
    // Model a save with a paid job from before its new research gate existed.
    w.players.get_mut(&owner).unwrap().research.completed.remove("mat_enrichment");
    w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    w.fixup_after_load();
    w.tick = w.build_queue[0].complete_tick;
    w.time = w.tick as f64 / crate::config::TICK_HZ as f64;
    w.resolve_builds(&mut Vec::new());
    assert!(w.build_queue.is_empty());
    assert_eq!(site(&w, home).tier(K::Smelter), 1);
    assert_eq!(site(&w, home).stockpile, paid_stock, "no second charge");
}

#[test]
fn both_component_branches_feed_a_drive_without_mining_manufactured_goods() {
    let (mut w, _, home) = rig();
    let s = site_mut(&mut w, home);
    for kind in FACTORIES { s.set_tier(kind, 1); s.assign(kind, Assignment::crew(1)); }
    for good in [C::Alloys, C::Polymers, C::Silicates, C::Electronics, C::Machinery, C::RareElements, C::Fuel, C::Titanium] {
        s.stockpile.insert(good, 40.0);
    }
    for _ in 0..300 { w.accrue_production(&mut Vec::new()); }
    assert!(stock(&w, home, C::HullSections) > 0.0);
    assert!(stock(&w, home, C::DriveAssemblies) > 0.0);
    for good in GOODS { assert!(crate::production::extraction_structure(good).is_none()); }
    for good in [C::Alloys, C::Polymers, C::Silicates, C::Electronics, C::Machinery, C::RareElements, C::Fuel, C::Titanium] {
        assert!(stock(&w, home, good) < 40.0, "the branches must use {good:?}");
    }
}

#[test]
fn manufactured_goods_trade_travel_in_one_hold_and_survive_a_save() {
    let (mut w, owner, home) = rig();
    w.players.get_mut(&owner).unwrap().warehouse.clear();
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    let freighter = w.alloc_entity_id();
    w.fleets.insert(freighter, Fleet::single(freighter, owner, ShipKind::Convoy, w.hub, FleetOrder::Idle, None));
    let f = w.fleets.get_mut(&freighter).unwrap();
    f.pos = w.hub;
    f.vel = Vec2::ZERO;
    f.order = FleetOrder::Idle;
    f.mission = None;
    f.take_cargo();
    for good in GOODS {
        w.apply_now(&Command::MarketBuy { player_id: owner, commodity: good, units: 25,
            max_unit_price: None }, &mut Vec::new());
        assert_eq!(w.players[&owner].warehouse[&good], 25);
        w.apply_logistics_load(owner, freighter, None, good, 25, &mut Vec::new());
        assert_eq!(w.fleets[&freighter].cargo_amount(good), 25);
    }
    assert_eq!(w.fleets[&freighter].cargo_units(), 100);
    w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    w.fixup_after_load();
    let home_pos = site(&w, home).pos;
    w.fleets.get_mut(&freighter).unwrap().pos = home_pos;
    w.apply_logistics_unload(owner, freighter, Some(home), &mut Vec::new());
    assert!(w.fleets[&freighter].cargo_is_empty());
    for good in GOODS {
        assert_eq!(stock(&w, home, good), 25.0, "{good:?} physically lands at the system");
    }
}

#[test]
fn new_factories_suspend_when_the_colony_cannot_feed_them() {
    for kind in FACTORIES {
        let (mut w, _, home) = rig();
        let s = site_mut(&mut w, home);
        let recipe = CONVERTERS.iter().find(|c| c.structure == kind).unwrap();
        s.set_tier(kind, 1);
        s.assign(kind, Assignment::crew(1));
        s.stockpile.remove(&C::Provisions);
        s.food_state = crate::colony::FoodState::NoProvisions;
        for (good, _) in recipe.inputs { s.stockpile.insert(*good, 40.0); }
        w.accrue_production(&mut Vec::new());
        assert_eq!(stock(&w, home, recipe.output), 0.0);
        for (good, _) in recipe.inputs { assert_eq!(stock(&w, home, *good), 40.0); }
    }
}
