//! Storage is physical infrastructure; founding and save migration grant it once.
use super::*;
use crate::build::{SlotPool, StructureKind as K};
use crate::cargo::Commodity;

#[test]
fn founding_warehouses_replace_the_base_without_overfilling_body_slots() {
    for seed in [1, 42, 123, 456, 812, 9001] {
        for index in 0..4 {
            let sys = crate::galaxy::generate_home_system(seed, index, EntityId(index as u64 + 1),
                Vec2::ZERO, "Home".into());
            assert_eq!(sys.tier_sum(K::Warehouse), 1);
            assert_eq!(sys.tier_sum(K::OrbitalWarehouse), 0);
            assert_eq!(sys.storage_cap(), 700.0, "no invisible second allowance");
            for b in &sys.bodies {
                assert!(b.pool_slots_built(SlotPool::Infrastructure) <= b.pool_slots(SlotPool::Infrastructure));
                if b.tier(K::Warehouse) > 0 {
                    assert_ne!(b.kind, crate::body::BodyKind::GasGiant);
                    assert!(!b.assignments.contains_key(&K::Warehouse), "storage needs no workers");
                }
            }
        }
    }
}

#[test]
fn warehouses_stack_across_bodies_and_never_delete_overflow() {
    let mut sys = crate::galaxy::generate_home_system(123, 0, EntityId(1), Vec2::ZERO, "Home".into());
    sys.set_tier(K::Warehouse, 2);
    assert_eq!(sys.storage_cap(), 1100.0);
    let other = sys.bodies.iter_mut().find(|b| b.tier(K::Warehouse) == 0).unwrap();
    other.set_tier(K::Warehouse, 1);
    other.set_tier(K::OrbitalWarehouse, 1);
    assert_eq!(sys.storage_cap(), 1100.0 + 700.0 + 2000.0);
    sys.stockpile.clear();
    sys.stockpile.insert(Commodity::Alloys, 3000.0);
    for b in &mut sys.bodies { b.set_tier(K::Warehouse, 0); b.set_tier(K::OrbitalWarehouse, 0); }
    assert_eq!(sys.storage_cap(), 0.0);
    assert_eq!(sys.storage_headroom(), 0.0);
    assert_eq!(sys.stockpile[&Commodity::Alloys], 3000.0);
}

#[test]
fn colony_landing_includes_a_ground_warehouse() {
    let mut w = World::new(SimConfig::for_players(123, 4));
    let owner = PlayerId(7001);
    w.step(&[Command::AddPlayer { id: owner, name: "Storage".into() }]);
    w.enclaves.clear();
    let sys = w.systems.iter().find(|s| s.owner.is_none()
        && !w.home_slots.iter().any(|h| h.system == Some(s.id))).unwrap();
    let (id, pos) = (sys.id, sys.pos);
    assert_eq!(sys.storage_cap(), 0.0);
    let fleet = w.alloc_entity_id();
    w.fleets.insert(fleet, Fleet::single(fleet, owner, ShipKind::Colony, pos, FleetOrder::Idle, None));
    w.resolve_colony_arrivals(&mut Vec::new());
    let sys = w.systems.iter().find(|s| s.id == id).unwrap();
    assert_eq!(sys.owner, Some(owner));
    assert_eq!(sys.tier(K::Warehouse), 1);
    assert_eq!(sys.storage_cap(), 700.0);
}

#[test]
fn legacy_storage_migrates_once_without_advancing_remote_reports() {
    let mut w = World::new(SimConfig::for_players(123, 4));
    let owner = PlayerId(7002);
    w.step(&[Command::AddPlayer { id: owner, name: "Legacy".into() }]);
    let cc = w.players[&owner].command_center;
    let sys = w.systems.iter_mut().find(|s| s.owner.is_none()
        && s.pos.distance(cc) > 10_000.0).unwrap();
    let id = sys.id;
    sys.owner = Some(owner);
    sys.set_tier(K::Warehouse, 0);
    sys.set_tier(K::OrbitalWarehouse, 2); // retain old storage buildings
    sys.stockpile.clear();
    sys.stockpile.insert(Commodity::Alloys, 100.0);
    let delay = crate::transit::delay(sys.pos, cc, w.config.c);
    w.time = 1000.0;
    w.information = Default::default();
    w.record_information();
    w.time = 1010.0;
    w.systems.iter_mut().find(|s| s.id == id).unwrap().stockpile.insert(Commodity::Alloys, 5000.0);
    w.record_information();
    let mut json = serde_json::to_value(&w).unwrap();
    json.as_object_mut().unwrap().remove("ground_storage_migrated");
    let mut restored: World = serde_json::from_value(json).unwrap();
    restored.fixup_after_load();
    let sys = restored.systems.iter().find(|s| s.id == id).unwrap();
    assert_eq!(sys.tier(K::Warehouse), 1);
    assert_eq!(sys.tier(K::OrbitalWarehouse), 2);
    assert_eq!(sys.stockpile[&Commodity::Alloys], 5000.0, "migration never truncates goods");
    let old = restored.information.site(id, cc, restored.config.c, 1005.0 + delay).unwrap();
    assert_eq!(old.at, 1000.0);
    assert_eq!(old.system.stockpile[&Commodity::Alloys], 100.0, "do not import present truth");
    assert_eq!(old.system.tier(K::Warehouse), 1);
    let new = restored.information.site(id, cc, restored.config.c, 1010.0 + delay).unwrap();
    assert_eq!(new.system.stockpile[&Commodity::Alloys], 5000.0);
    restored.systems.iter_mut().find(|s| s.id == id).unwrap().set_tier(K::Warehouse, 0);
    let mut again: World = serde_json::from_str(&serde_json::to_string(&restored).unwrap()).unwrap();
    again.fixup_after_load();
    assert_eq!(again.systems.iter().find(|s| s.id == id).unwrap().tier(K::Warehouse), 0,
        "loading again cannot rebuild a lost warehouse");
}
