//! Local administration is not the corporation's communications origin.
use super::*;
use crate::build::{BuildKind, SlotPool, StructureKind as K};
use crate::cargo::Commodity as C;

fn scene() -> (World, PlayerId, EntityId, u32) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    let owner = PlayerId(7801);
    w.step(&[Command::AddPlayer { id: owner, name: "Local command".into() }]);
    w.enclaves.clear();
    let home = w.players[&owner].home_system.unwrap();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    let body = sys.bodies.iter().find(|b| b.tier(K::CommandCenter) == 1).unwrap().id;
    for c in C::ALL { sys.stockpile.insert(c, 10_000.0); }
    (w, owner, home, body)
}

fn queue(w: &mut World, owner: PlayerId, home: EntityId, body: u32, kind: K) -> Vec<Event> {
    let mut events = Vec::new();
    w.apply_build(owner, home, Some(body), BuildKind::Upgrade { upgrade: kind },
        None, Default::default(), &mut events);
    events
}

fn finish_head(w: &mut World) {
    w.update_structure_queues(&mut Vec::new());
    w.tick = w.build_queue.iter().map(|j| j.complete_tick).min().unwrap();
    w.time = w.tick as f64 * crate::DT;
    w.resolve_builds(&mut Vec::new());
}

#[test]
fn landing_command_is_free_unstaffed_and_uses_no_development_slot() {
    let (w, owner, home, body) = scene();
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    let b = sys.bodies.iter().find(|b| b.id == body).unwrap();
    assert_eq!(sys.tier_sum(K::CommandCenter), 1);
    assert!(!b.assignments.contains_key(&K::CommandCenter));
    let mut without = b.clone();
    without.set_tier(K::CommandCenter, 0);
    assert_eq!(b.pool_slots_built(SlotPool::Infrastructure), without.pool_slots_built(SlotPool::Infrastructure));
    assert_eq!(b.infrastructure_slots(), without.infrastructure_slots());
    assert_eq!(b.command_construction_mult(), 1.0);
    assert_eq!(w.players[&owner].command_center, sys.pos);
    // §power: the landing command comes with its planet's first Power Plant,
    // slot-free too.
    assert_eq!(b.tier(K::PowerPlant), 1);
    assert_eq!(sys.tier_sum(K::PowerPlant), 1, "only the command's own planet");
}

#[test]
fn command_upgrades_debit_their_own_recipe_gate_three_and_cap_at_three() {
    let (mut w, owner, home, body) = scene();
    let cc = w.players[&owner].command_center;
    let c = w.config.c;
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert_eq!(w.build_queue.len(), 1);
    let stock = &w.systems.iter().find(|s| s.id == home).unwrap().stockpile;
    assert_eq!(stock[&C::Alloys], 9960.0);
    assert_eq!(stock[&C::Machinery], 9988.0);
    assert_eq!(stock[&C::Electronics], 9992.0);
    let before = stock.clone();
    let rejected = queue(&mut w, owner, home, body, K::CommandCenter);
    assert_eq!(w.build_queue.len(), 1, "pending level II counts toward the research gate");
    assert!(rejected.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
        reason: crate::event::BuildRejectReason::NeedsResearch, .. })));
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, before);
    finish_head(&mut w);
    w.players.get_mut(&owner).unwrap().research.completed.insert("mat_prefab_construction".into());
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert_eq!(w.build_queue.len(), 1);
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    assert_eq!(sys.stockpile[&C::Alloys], 9840.0);
    assert_eq!(sys.stockpile[&C::Machinery], 9948.0);
    assert_eq!(sys.stockpile[&C::Electronics], 9962.0);
    assert_eq!(sys.stockpile[&C::Polymers], 9980.0);
    let expected = (900.0 * sys.bodies.iter().find(|b| b.id == body).unwrap().construction_time_mult()
        * 0.9 * 0.75 / crate::DT).ceil();
    assert_eq!(w.build_queue[0].work().unwrap().required, expected);
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert_eq!(w.build_queue.len(), 1, "pending headquarters prevents a fourth level");
    finish_head(&mut w);
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().tier(K::CommandCenter), 3);
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert!(w.build_queue.is_empty());
    assert_eq!(w.players[&owner].command_center, cc);
    assert_eq!(w.config.c, c, "no changes to information speed");
}

#[test]
fn administration_is_local_and_does_not_reprice_work_already_queued() {
    let (mut w, owner, home, body) = scene();
    let sys = w.systems.iter().find(|s| s.id == home).unwrap();
    let original = sys.bodies.iter().find(|b| b.id == body).unwrap().clone();
    let mut two = original.clone();
    two.set_tier(K::CommandCenter, 2);
    assert_eq!(two.infrastructure_slots(), original.infrastructure_slots() + 1);
    assert_eq!(two.command_construction_mult(), 0.9);
    two.set_tier(K::CommandCenter, 3);
    assert_eq!(two.infrastructure_slots(), original.infrastructure_slots() + 2);
    assert_eq!(two.command_construction_mult(), 0.8);
    assert_eq!(two.ship_build_time_mult(), original.ship_build_time_mult());
    assert_eq!(two.construction_time_mult(), original.construction_time_mult(), "public environmental factor stays natural");
    let other = sys.bodies.iter().find(|b| b.id != body).unwrap().clone();
    // The legacy home package may put its warehouse on a sibling world.
    w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut()
        .find(|b| b.id == body).unwrap().set_tier(K::Warehouse, 1);
    queue(&mut w, owner, home, body, K::CommandCenter);
    queue(&mut w, owner, home, body, K::Warehouse);
    let waiting = w.build_queue[1].work().unwrap().required;
    finish_head(&mut w);
    assert_eq!(w.build_queue[0].work().unwrap().required, waiting, "queued work locks bonuses at receipt");
    finish_head(&mut w);
    queue(&mut w, owner, home, body, K::Warehouse);
    let recipe = crate::build::recipe_for(BuildKind::Upgrade { upgrade: K::Warehouse });
    assert_eq!(w.build_queue[0].work().unwrap().required,
        (recipe.build_ticks as f64 * original.construction_time_mult() * 0.9).round());
    let unchanged = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| b.id == other.id).unwrap();
    assert_eq!(unchanged.command_construction_mult(), other.command_construction_mult());
    assert_eq!(unchanged.infrastructure_slots(), other.infrastructure_slots());
}

#[test]
fn command_requires_residents_and_solid_ground_but_not_a_free_pool_slot() {
    let (mut w, owner, home, body) = scene();
    let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.population = 0.0009;
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert!(w.build_queue.is_empty());
    let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.population = 0.002;
    let kind = b.kind;
    b.kind = crate::body::BodyKind::GasGiant;
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert!(w.build_queue.is_empty());
    let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.kind = kind;
    b.set_tier(K::CommandCenter, 0);
    for k in K::ALL.into_iter().filter(|k| k.uses_slot() && k.slot_pool() == SlotPool::Infrastructure) { b.set_tier(k, 1); }
    assert!(b.pool_slots_built(SlotPool::Infrastructure) >= b.infrastructure_slots());
    queue(&mut w, owner, home, body, K::CommandCenter);
    assert_eq!(w.build_queue.len(), 1, "dedicated footprint can rebuild on a full planet");
    assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&C::Alloys], 9985.0);
}

#[test]
fn colony_landings_deploy_a_landing_command() {
    let (mut w, owner, _, _) = scene();
    let sys = w.systems.iter().find(|s| s.owner.is_none()
        && !w.home_slots.iter().any(|h| h.system == Some(s.id))).unwrap();
    let (id, pos) = (sys.id, sys.pos);
    let fleet = w.alloc_entity_id();
    w.fleets.insert(fleet, Fleet::single(fleet, owner, ShipKind::Colony, pos, FleetOrder::Idle, None));
    w.resolve_colony_arrivals(&mut Vec::new());
    let colony = w.systems.iter().find(|s| s.id == id).unwrap();
    assert_eq!(colony.owner, Some(owner));
    assert_eq!(colony.tier_sum(K::CommandCenter), 1);
    let command = colony.bodies.iter().find(|b| b.tier(K::CommandCenter) == 1).unwrap();
    assert_eq!(command.tier(K::PowerPlant), 1, "a landing brings its plant");
}

#[test]
fn old_saves_migrate_once_without_importing_new_truth_into_delayed_reports() {
    let (mut w, owner, _, _) = scene();
    let cc = w.players[&owner].command_center;
    let sys = w.systems.iter_mut().find(|s| s.owner.is_none() && s.pos.distance(cc) > 10_000.0).unwrap();
    sys.owner = Some(owner);
    sys.seed_population(0.002);
    let id = sys.id;
    let delay = crate::transit::delay(sys.pos, cc, w.config.c);
    w.information = Default::default();
    w.time = 1000.0;
    w.record_information();
    w.time = 1010.0;
    let sys = w.systems.iter_mut().find(|s| s.id == id).unwrap();
    sys.seed_command_centers();
    sys.set_tier(K::CommandCenter, 3);
    w.players.get_mut(&owner).unwrap().research.completed.insert("mat_prefab_construction".into());
    w.record_information();
    let mut json = serde_json::to_value(&w).unwrap();
    json.as_object_mut().unwrap().remove("command_centers_migrated");
    let mut restored: World = serde_json::from_value(json).unwrap();
    restored.fixup_after_load();
    assert_eq!(restored.systems.iter().find(|s| s.id == id).unwrap().tier(K::CommandCenter), 3);
    let old = restored.information.site(id, cc, restored.config.c, 1005.0 + delay).unwrap();
    assert_eq!(old.at, 1000.0);
    assert_eq!(old.system.tier(K::CommandCenter), 1, "historical report only gets the base migration grant");
    assert_eq!(old.production_mods.structure_build_time, 1.0);
    let arrived = restored.information.site(id, cc, restored.config.c, 1010.0 + delay).unwrap();
    assert_eq!(arrived.system.tier(K::CommandCenter), 3);
    assert_eq!(arrived.production_mods.structure_build_time, 0.75);
    restored.systems.iter_mut().find(|s| s.id == id).unwrap().set_tier(K::CommandCenter, 0);
    let mut again: World = serde_json::from_slice(&serde_json::to_vec(&restored).unwrap()).unwrap();
    again.fixup_after_load();
    assert_eq!(again.systems.iter().find(|s| s.id == id).unwrap().tier(K::CommandCenter), 0,
        "reload cannot rebuild a lost command center");
    assert_eq!(again.players[&owner].command_center, cc);
}
