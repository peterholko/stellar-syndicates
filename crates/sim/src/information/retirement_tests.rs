use super::*;
use crate::ship::{Fleet, FleetOrder, ShipKind, TradeMission};
use crate::tca::{Shipment, ShipmentDir, ShipmentId};
use crate::{Command, Commodity, SimConfig};

fn fixture() -> (World, PlayerId, EntityId, ShipmentId) {
    let owner = PlayerId(71);
    let mut world = World::new(SimConfig::for_players(71, 2));
    world.step(&[Command::AddPlayer {
        id: owner,
        name: "Retirement".into(),
    }]);
    world.fleets.clear();
    world.freight_queue.clear();
    world.freight_runs.clear();
    world.hub = Vec2::new(20_000.0, 0.0);
    world.players.get_mut(&owner).unwrap().command_center = Vec2::ZERO;
    world.time = 1000.0;
    world.information = InformationHistory::default();
    let fleet = EntityId(900_000);
    let mut hull = Fleet::single(
        fleet,
        owner,
        ShipKind::TinyFreighter,
        world.hub,
        FleetOrder::Idle,
        None,
    );
    hull.mission = Some(TradeMission::DeliverHome);
    world.fleets.insert(fleet, hull);
    let shipment = ShipmentId(900_000);
    world.freight_queue.insert(
        shipment,
        Shipment {
            id: shipment,
            owner,
            system: world.players[&owner].home_system.unwrap(),
            direction: ShipmentDir::Outbound,
            commodity: Commodity::Deuterium,
            units: 5,
            fee_paid: 10.0,
            booked_at: world.time,
            sell_on_arrival: false,
        },
    );
    world.record_information();
    (world, owner, fleet, shipment)
}

fn horizon(world: &World) -> f64 {
    world.config.galaxy_radius * 2.5 / crate::transit::signal_speed(world.config.c) + 1.0
}

fn retire(world: &mut World, fleet: EntityId, shipment: ShipmentId) -> f64 {
    world.time += 1.0;
    world.fleets.remove(&fleet);
    world.freight_queue.remove(&shipment);
    world.record_information();
    world.time
}

#[test]
fn retired_fleets_and_freight_leave_hot_maps_only_after_their_light() {
    let (mut world, owner, fleet, shipment) = fixture();
    let gone = retire(&mut world, fleet, shipment);
    let c = world.config.c;
    let arrival = gone + crate::transit::delay(world.hub, Vec2::ZERO, c);
    world.time = arrival - 0.001;
    world.record_information();
    assert!(
        world
            .information
            .fleet_assets(Vec2::ZERO, c, world.time)
            .iter()
            .any(|f| f.id == fleet)
    );
    assert!(
        !world
            .information
            .fleet_haul_finished(fleet, Vec2::ZERO, c, world.time)
    );
    assert_eq!(
        world
            .information
            .shipments(owner, Vec2::ZERO, c, world.time)
            .len(),
        1
    );
    world.time = arrival;
    world.record_information();
    assert!(
        world
            .information
            .fleet_assets(Vec2::ZERO, c, world.time)
            .is_empty()
    );
    assert!(
        world
            .information
            .fleet_haul_finished(fleet, Vec2::ZERO, c, world.time)
    );
    assert!(
        world
            .information
            .shipments(owner, Vec2::ZERO, c, world.time)
            .is_empty()
    );
    assert!(
        world.information.fleet_assets.contains_key(&fleet),
        "keep history for farther observers"
    );
    assert!(world.information.shipments.contains_key(&shipment));
    world.time = gone + horizon(&world);
    world.record_information();
    assert!(
        world.information.fleet_assets.contains_key(&fleet),
        "retain the exact horizon boundary"
    );
    assert!(world.information.shipments.contains_key(&shipment));
    world.time = gone + horizon(&world) + 1.0;
    world.record_information();
    assert!(
        !world.information.fleet_assets.contains_key(&fleet),
        "expired fleets must leave the per-tick scan"
    );
    assert!(
        !world.information.shipments.contains_key(&shipment),
        "completed shipments must leave the per-tick scan"
    );
}

#[test]
fn compact_completion_receipts_remain_light_gated_and_survive_save() {
    let (mut world, _, fleet, shipment) = fixture();
    let gone = retire(&mut world, fleet, shipment);
    world.time = gone + horizon(&world) + 1.0;
    world.record_information();
    let info: InformationHistory =
        serde_json::from_slice(&serde_json::to_vec(&world.information).unwrap()).unwrap();
    assert!(
        !info.fleet_assets.contains_key(&fleet),
        "only the compact completion receipt remains"
    );
    assert!(
        info.fleet_haul_finished(fleet, Vec2::ZERO, world.config.c, world.time),
        "standing orders must still be able to release a completed haul after cleanup/load"
    );
    assert!(
        !info.fleet_haul_finished(EntityId(999_999), Vec2::ZERO, world.config.c, world.time),
        "an unknown id is not evidence of completion"
    );
    // Even a point lookup from beyond the ordinary chart must wait for the
    // actual disappearance wavefront, not merely the garbage collector's time.
    let remote = Vec2::new(world.config.galaxy_radius * 10.0, 0.0);
    let arrival = gone + crate::transit::delay(world.hub, remote, world.config.c);
    assert!(!info.fleet_haul_finished(fleet, remote, world.config.c, arrival - 0.001));
    assert!(info.fleet_haul_finished(fleet, remote, world.config.c, arrival));
}

#[test]
fn a_saved_standing_order_releases_its_fleet_after_history_cleanup() {
    let (mut world, owner, fleet, shipment) = fixture();
    let gone = retire(&mut world, fleet, shipment);
    world.time = gone + horizon(&world) + 1.0;
    world.record_information();
    let home = world.players[&owner].home_system.unwrap();
    world
        .players
        .get_mut(&owner)
        .unwrap()
        .standing_orders
        .push(crate::StandingOrder {
            id: 1,
            source: crate::Endpoint::System { id: home },
            dest: crate::Endpoint::Hub,
            commodity: Commodity::Deuterium,
            trigger: crate::Trigger::AboveThreshold { threshold: 100.0 },
            status: crate::OrderStatus::Paused,
            next_eval_tick: 0,
            in_flight: Some(fleet),
            sell_on_arrival: true,
        });
    let mut restored: World = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert!(!restored.information.fleet_assets.contains_key(&fleet));
    // One ordinary reconciliation tick, not an accelerated bot run. Paused
    // prevents a new dispatch from obscuring whether the old latch cleared.
    restored.step(&[]);
    assert_eq!(restored.players[&owner].standing_orders[0].in_flight, None);
}

#[test]
fn unchanged_live_reports_are_not_garbage_collected() {
    let (mut world, owner, fleet, shipment) = fixture();
    world.fleets.get_mut(&fleet).unwrap().mission = None;
    world.record_information();
    world.time += horizon(&world) * 10.0;
    world.record_information();
    assert!(world.information.fleet_assets.contains_key(&fleet));
    assert!(world.information.shipments.contains_key(&shipment));
    assert!(
        world
            .information
            .fleet_haul_finished(fleet, Vec2::ZERO, world.config.c, world.time)
    );
    assert_eq!(
        world
            .information
            .shipments(owner, Vec2::ZERO, world.config.c, world.time)
            .len(),
        1
    );
    // Starting a new haul cannot inherit a previous completion at the receiver.
    world.fleets.get_mut(&fleet).unwrap().mission = Some(TradeMission::DeliverHome);
    world.record_information();
    let arrival = world.time + crate::transit::delay(world.hub, Vec2::ZERO, world.config.c);
    assert!(world.information.fleet_haul_finished(
        fleet,
        Vec2::ZERO,
        world.config.c,
        arrival - 0.001
    ));
    assert!(
        !world
            .information
            .fleet_haul_finished(fleet, Vec2::ZERO, world.config.c, arrival)
    );
}

fn old_absence<T>() -> Reports<T> {
    Reports {
        last_pos: Vec2::ZERO,
        moving_origin: false,
        samples: VecDeque::from([(1.0, Vec2::ZERO, None)]),
    }
}

#[test]
fn legacy_tombstones_are_compacted_for_every_retirable_report_kind() {
    let (mut world, owner, fleet, shipment) = fixture();
    let id = EntityId(800_000);
    world.information.fleet_assets.insert(id, old_absence());
    world
        .information
        .shipments
        .insert(ShipmentId(id.0), old_absence());
    world.information.emplacements.insert(id, old_absence());
    world
        .information
        .syndicates
        .insert(crate::SyndicateId(id.0), old_absence());
    world
        .information
        .captains
        .entry(owner)
        .or_default()
        .insert(800_000, old_absence());
    world
        .information
        .standing
        .entry(owner)
        .or_default()
        .insert(800_000, old_absence());
    let mut legacy = serde_json::to_value(&world.information).unwrap();
    legacy.as_object_mut().unwrap().remove("fleet_completions");
    world.information = serde_json::from_value(legacy).unwrap();
    world.time += horizon(&world) + 1.0;
    world.record_information();
    assert!(!world.information.fleet_assets.contains_key(&id));
    assert!(!world.information.shipments.contains_key(&ShipmentId(id.0)));
    assert!(!world.information.emplacements.contains_key(&id));
    assert!(
        !world
            .information
            .syndicates
            .contains_key(&crate::SyndicateId(id.0))
    );
    assert!(!world.information.captains[&owner].contains_key(&800_000));
    assert!(!world.information.standing[&owner].contains_key(&800_000));
    assert!(
        world
            .information
            .fleet_haul_finished(id, Vec2::ZERO, world.config.c, world.time)
    );
    assert!(world.information.fleet_assets.contains_key(&fleet));
    assert!(world.information.shipments.contains_key(&shipment));
}

#[test]
fn history_churn_does_not_grow_the_per_tick_working_set() {
    let (mut world, _, live_fleet, live_shipment) = fixture();
    // Synthetic report churn, not a bot simulation: thousands of old completed
    // shipments/fleets must disappear from the collections record() traverses.
    for batch in 0..4 {
        for i in 0..1024 {
            let id = EntityId(1_000_000 + batch * 1024 + i);
            world.information.fleet_assets.insert(id, old_absence());
            world
                .information
                .shipments
                .insert(ShipmentId(id.0), old_absence());
        }
        world.time += horizon(&world) + 1.0;
        world.record_information();
        assert_eq!(world.information.fleet_assets.len(), 1);
        assert_eq!(world.information.shipments.len(), 1);
        assert!(world.information.fleet_assets.contains_key(&live_fleet));
        assert!(world.information.shipments.contains_key(&live_shipment));
    }
    for _ in 0..10 {
        world.time += crate::DT;
        world.record_information();
        assert_eq!(world.information.fleet_assets.len(), 1);
        assert_eq!(world.information.shipments.len(), 1);
    }
}
