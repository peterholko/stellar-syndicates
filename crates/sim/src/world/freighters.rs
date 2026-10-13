//! Freight sizes are real hulls, not menu aliases; no Authority hull changes.
use super::*;
use crate::build::{BuildKind, StructureKind as K};
use crate::cargo::Commodity as C;
use crate::module::{Loadout, ModuleKind as M};
use crate::ship::PLAYER_FREIGHTERS;

const PROGRAMMES: [Option<&str>; 6] = [None, Some("prop_freight_frames"),
    Some("prop_heavy_lifters"), Some("prop_line_express_charters"),
    Some("prop_line_bulk_charters"), Some("prop_line_autonomous_freight")];

fn fixture() -> (World, PlayerId, EntityId) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    let owner = PlayerId(6800);
    w.step(&[Command::AddPlayer { id: owner, name: "Freight sizes".into() }]);
    w.enclaves.clear();
    let home = w.players[&owner].home_system.unwrap();
    (w, owner, home)
}

#[test]
fn slow_freighters_take_twice_as_long_on_uranium_and_burn_a_quarter_of_the_energy() {
    use crate::ship::{DriveState, TransitMode};
    let (mut w, owner, _) = fixture();
    // Isolate propulsion from refuelling, commands, wells and captain bonuses.
    // Exercise the real tick debit and movement integrator, not just a quote.
    w.fleets.clear();
    let start = Vec2::new(5_000_000.0, 0.0);
    let dest = start + Vec2::new(1_000.0, 0.0);
    for kind in PLAYER_FREIGHTERS {
        let mut results = Vec::new();
        for mode in [TransitMode::Full, TransitMode::Slow, TransitMode::Stealth] {
            let id = w.alloc_entity_id();
            let mut f = Fleet::single(id, owner, kind, start, FleetOrder::MoveTo { dest }, None);
            f.add_cargo(C::MetallicOre, kind.cargo_units());
            f.transit = mode;
            f.drive_state = DriveState::Cruising(crate::transit::Regime::Warp);
            f.vel = Vec2::new(f.transit_speed() * crate::transit::WARP_FACTOR, 0.0);
            let (fuel0, uranium0) = (f.fuel, f.uranium);
            let range = f.range_at(crate::transit::WARP_FACTOR);
            w.fleets.insert(id, f);
            let mut ticks = 0;
            while w.fleets[&id].pos.distance(dest) > 0.1 && ticks < 1_000 {
                w.time += DT;
                w.integrate_movement(&mut vec![]);
                ticks += 1;
            }
            assert!(ticks < 1_000, "{kind:?} {mode:?} must reach the destination");
            let f = &w.fleets[&id];
            results.push((ticks, fuel0 - f.fuel, uranium0 - f.uranium, range));
            w.fleets.remove(&id);
        }
        let (full, slow, stealth) = (results[0], results[1], results[2]);
        assert!((slow.0 as i32 - 2 * full.0 as i32).abs() <= 2, "{kind:?}: {results:?}");
        assert!(full.2.abs() < 1e-12 && slow.1.abs() < 1e-12, "{kind:?}: each regime burns its own store");
        assert!((slow.2 / full.1 - 0.25).abs() < 0.001, "{kind:?}: {results:?}");
        assert!((slow.3 / full.3 - 4.0).abs() < 1e-9, "the same-size store goes four times as far");
        assert!((stealth.1 / full.1 - 1.0).abs() < 0.01, "Stealth's trip cost is unchanged");
    }
}

#[test]
fn escorts_fly_their_charges_uranium_regime() {
    use crate::ship::{DriveState, TransitMode};
    let (mut w, owner, _) = fixture();
    w.fleets.clear();
    let mut results = Vec::new();
    for mode in [TransitMode::Full, TransitMode::Slow] {
        let start = Vec2::new(5_000_000.0, 0.0);
        let id = w.alloc_entity_id();
        let guard_id = w.alloc_entity_id();
        let mut f = Fleet::single(id, owner, ShipKind::TinyFreighter, start,
            FleetOrder::MoveTo { dest: start + Vec2::new(10_000.0, 0.0) }, None);
        f.transit = mode;
        f.drive_state = DriveState::Cruising(crate::transit::Regime::Warp);
        f.vel = Vec2::new(f.transit_speed() * crate::transit::WARP_FACTOR, 0.0);
        let guard_pos = f.pos - f.vel.normalized() * GUARD_STANDOFF_SU;
        let mut guard = Fleet::single(guard_id, owner, ShipKind::Raider, guard_pos,
            FleetOrder::Guard { target: id }, None);
        guard.vel = f.vel;
        guard.drive_state = f.drive_state;
        let before = (guard.fuel, guard.uranium);
        w.fleets.insert(id, f);
        w.fleets.insert(guard_id, guard);
        let ticks = if mode == TransitMode::Full { 120 } else { 240 };
        for _ in 0..ticks { w.time += DT; w.integrate_movement(&mut vec![]); }
        assert!((w.fleets[&guard_id].vel.length() - w.fleets[&id].vel.length()).abs() < 1e-6);
        let g = &w.fleets[&guard_id];
        results.push((w.fleets[&id].pos.distance(start), before.0 - g.fuel, before.1 - g.uranium));
        w.fleets.clear();
    }
    assert!((results[0].0 - results[1].0).abs() < 0.1, "{results:?}");
    assert!(results[0].2.abs() < 1e-12 && results[1].1.abs() < 1e-12, "{results:?}");
    assert!((results[1].2 / results[0].1 - 0.25).abs() < 1e-6,
        "the escort burns its charge's Uranium regime over the same distance: {results:?}");
}

#[test]
fn slow_commands_are_delayed_and_ships_of_the_line_cannot_fly_slow() {
    use crate::ship::TransitMode;
    let (mut w, owner, _) = fixture();
    w.fleets.clear();
    let id = w.alloc_entity_id();
    let pos = w.players[&owner].command_center + Vec2::new(10_000.0, 0.0);
    w.fleets.insert(id, Fleet::single(id, owner, ShipKind::TinyFreighter, pos, FleetOrder::Idle, None));
    w.step(&[Command::SetFleetTransit { player_id: owner, fleet_id: id, mode: TransitMode::Slow }]);
    assert_eq!(w.fleets[&id].transit, TransitMode::Full, "setting cannot beat its order signal");
    for _ in 0..((crate::transit::delay(pos, w.players[&owner].command_center, w.config.c) + 1.0) / DT) as u32 {
        w.step(&[]);
    }
    assert_eq!(w.fleets[&id].transit, TransitMode::Slow);
    let restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    assert_eq!(restored.fleets[&id].transit, TransitMode::Slow);
    let f = w.fleets.get_mut(&id).unwrap();
    f.composition.insert(ShipKind::Raider, 1);
    assert!(f.can_slow(), "an escort warship flies Slow with its charge");
    f.composition.insert(ShipKind::Battleship, 1);
    assert!(!f.can_slow(), "a ship of the line carries no fission store");
    assert_eq!(f.cruise_fuel_mult(), 1.0);
    assert_eq!(f.transit_speed(), f.max_speed());
}

#[test]
fn freight_construction_starts_with_importable_basics_then_adds_industry() {
    // These are construction manifests, not a credit purchase or an automatic
    // hull replacement. Every unit must reach a yard through normal logistics.
    let manifests: [&[(C, f64)]; 6] = [
        &[(C::Alloys, 20.0), (C::Machinery, 8.0), (C::Polymers, 8.0), (C::WarpCore, 1.0)],
        &[(C::Alloys, 45.0), (C::Machinery, 16.0), (C::Polymers, 16.0), (C::WarpCore, 1.0)],
        &[(C::Alloys, 90.0), (C::Machinery, 28.0), (C::Polymers, 20.0), (C::Electronics, 10.0), (C::WarpCore, 2.0)],
        &[(C::Alloys, 180.0), (C::Machinery, 45.0), (C::Electronics, 25.0), (C::HullSections, 12.0), (C::WarpCore, 3.0)],
        &[(C::Alloys, 320.0), (C::Machinery, 70.0), (C::HullSections, 30.0), (C::PrecisionComponents, 15.0), (C::WarpCore, 5.0)],
        &[(C::Alloys, 520.0), (C::Machinery, 100.0), (C::HullSections, 60.0),
            (C::PrecisionComponents, 25.0), (C::WarpCore, 16.0)],
    ];
    for (kind, expected) in PLAYER_FREIGHTERS.into_iter().zip(manifests) {
        let recipe = crate::build::recipe_for(BuildKind::Ship { ship: kind });
        assert_eq!(recipe.costs, expected, "{kind:?} construction manifest");
    }
    let starter = crate::build::TINY_FREIGHTER_RECIPE;
    assert!(starter.costs.iter().map(|(_, n)| n).sum::<f64>() <= 50.0,
        "one Tiny can bring home the entire replacement-hull manifest");
}

#[test]
fn medium_freight_introduces_electronics_before_heavy_components() {
    let costs = crate::build::CONVOY_RECIPE.costs;
    assert!(costs.contains(&(C::Electronics, 10.0)));
    assert!(!costs.iter().any(|(c, _)| matches!(c,
        C::HullSections | C::PrecisionComponents)));
}

#[test]
fn larger_freighters_trade_more_capital_at_risk_for_lower_full_load_costs() {
    let mut previous = None;
    for kind in PLAYER_FREIGHTERS {
        let recipe = crate::build::recipe_for(BuildKind::Ship { ship: kind });
        // Reference prices compare material baskets; they do NOT promise live
        // market profit. Fuel compares equal-distance loaded-out/empty-back
        // trips with identical fittings/captains and no escort costs.
        let investment: f64 = recipe.costs.iter()
            .map(|(c, n)| crate::market::base_price(*c) * n).sum();
        let capacity = kind.cargo_units() as f64;
        let fuel_per_unit = crate::fuel::fuel_cost(1.0,
            2.0 * kind.hull_mass() + capacity * crate::ship::CARGO_MASS_PER_UNIT) / capacity;
        let upkeep_per_unit_distance = crate::ship::upkeep_per_sec(kind)
            / (capacity * kind.max_speed());
        if let Some((old_cost, old_cost_per_unit, old_fuel, old_upkeep, old_ticks)) = previous {
            assert!(investment > old_cost, "{kind:?}: more capital in one vulnerable hull");
            assert!(investment / capacity < old_cost_per_unit, "{kind:?}: construction economies of scale");
            assert!(fuel_per_unit < old_fuel, "{kind:?}: full-load fuel economies");
            assert!(upkeep_per_unit_distance < old_upkeep, "{kind:?}: even accounting for slower transit");
            assert!(recipe.build_ticks > old_ticks, "{kind:?}: replacement takes longer");
        }
        previous = Some((investment, investment / capacity, fuel_per_unit,
            upkeep_per_unit_distance, recipe.build_ticks));
    }
}

#[test]
fn six_sizes_have_independent_holds_and_keep_civilian_rules() {
    let mut mixed = Fleet::single(EntityId(1), PlayerId(1), ShipKind::Raider,
        Vec2::ZERO, FleetOrder::Idle, None);
    for (kind, capacity) in PLAYER_FREIGHTERS.into_iter().zip([50, 150, 400, 1000, 2500, 6000]) {
        let mut f = Fleet::single(EntityId(2), PlayerId(1), kind, Vec2::ZERO, FleetOrder::Idle, None);
        assert_eq!(f.cargo_capacity(), capacity);
        assert!(f.has_freighter() && kind.is_buildable());
        assert!(!kind.is_combatant() && !kind.broadcasts() && !kind.has_jump_drive());
        assert_eq!(f.sensor_mult(), crate::ship::CONVOY_SENSOR_MULT);
        assert!(Loadout::new(vec![M::CargoPods]).validate(kind));
        assert!(Loadout::new(vec![M::FuelTransferRig]).validate(kind));
        assert!(!Loadout::new(vec![M::MassDriver]).validate(kind));
        f.set_fitted(kind, &Loadout::new(vec![M::CargoPods]), 1);
        assert_eq!(f.cargo_capacity(), 2 * capacity);
        mixed.add(kind, 1);
    }
    assert_eq!(mixed.cargo_capacity(), 10_100);
    mixed.set_fitted(ShipKind::TinyFreighter, &Loadout::new(vec![M::CargoPods]), 1);
    assert_eq!(mixed.cargo_capacity(), 10_150, "pods double only their fitted hull");
    assert_eq!(mixed.cargo_capacity_after_refit(ShipKind::TinyFreighter,
        &Loadout::new(vec![M::CargoPods]), &Loadout::default(), 1), 10_100);
    assert_eq!(mixed.freighter_count(), 6);
    assert!(ShipKind::Freighter.broadcasts());
    assert!(!ShipKind::Freighter.is_player_freighter());
}

#[test]
fn each_freighter_loads_mixed_cargo_up_to_its_own_capacity() {
    for kind in PLAYER_FREIGHTERS {
        let (mut w, owner, home) = fixture();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.stockpile.insert(C::MetallicOre, 10_000.0);
        sys.stockpile.insert(C::Provisions, 10_000.0);
        let pos = sys.pos;
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None));
        let cap = kind.cargo_units();
        w.apply_logistics_load(owner, id, Some(home), C::Provisions, 10, &mut vec![]);
        w.apply_logistics_load(owner, id, Some(home), C::MetallicOre, cap - 10, &mut vec![]);
        assert_eq!(w.fleets[&id].cargo_units(), cap, "{kind:?}");
        assert_eq!(w.fleets[&id].cargo_stacks().len(), 2);
        let before = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        w.apply_logistics_load(owner, id, Some(home), C::MetallicOre, 1, &mut vec![]);
        assert_eq!(w.fleets[&id].cargo_units(), cap);
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, before);
    }
}

#[test]
fn freight_unlocks_and_yard_tiers_are_enforced_at_receipt() {
    let yards = [(K::Shipyard, 1), (K::Shipyard, 1), (K::Shipyard, 2),
        (K::NavalDrydock, 1), (K::NavalDrydock, 2), (K::NavalDrydock, 3)];
    for ((kind, programme), (yard, tier)) in PLAYER_FREIGHTERS.into_iter().zip(PROGRAMMES).zip(yards) {
        let (mut w, owner, home) = fixture();
        assert_eq!(crate::build::yard_for(kind), (yard, tier));
        let what = BuildKind::Ship { ship: kind };
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.set_tier(yard, tier);
        for (good, _) in crate::build::recipe_for(what).costs { sys.stockpile.insert(*good, 10_000.0); }
        let stock = sys.stockpile.clone();
        if let Some(programme) = programme {
            let mut events = Vec::new();
            w.apply_build(owner, home, None, what, None, Loadout::default(), &mut events);
            assert!(w.build_queue.is_empty(), "{kind:?}: no research means no hull");
            assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
                reason: crate::event::BuildRejectReason::NeedsResearch, .. })));
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
            w.players.get_mut(&owner).unwrap().research.completed.insert(programme.into());
        }
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(yard, tier - 1);
        let mut events = Vec::new();
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut events);
        assert!(w.build_queue.is_empty(), "{kind:?}: research cannot bypass the yard");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
            reason: crate::event::BuildRejectReason::NeedsYard { yard: needed, required }, ..
        } if required == tier && needed == yard)));
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(yard, tier);
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
        assert_eq!(w.build_queue.len(), 1, "{kind:?}: unlocked hull queues without a syndicate");
        assert_eq!(w.build_queue[0].what, what);
        assert_eq!(w.build_queue[0].construction_yard, Some(yard));
        let paid = &w.systems.iter().find(|s| s.id == home).unwrap().stockpile;
        for (good, units) in crate::build::recipe_for(what).costs {
            assert_eq!(paid[good], stock[good] - units, "{kind:?}: pays the actual recipe once");
        }
    }
}

#[test]
fn tiny_freighter_advances_the_founding_export() {
    let (mut w, owner, _home) = fixture();
    w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::BuildConvoy, w.time);
    let id = w.players[&owner].founding.convoy.unwrap();
    w.step(&[]);
    assert_eq!(w.players[&owner].founding.convoy, Some(id));
    // §charter v3: a founder with a Freighter goes on to the first shipment,
    // and an escorted departure draws the privateer lesson.
    assert_eq!(w.players[&owner].founding.stage, crate::founding::FoundingStage::PrepareExport);
    let guard = w.fleets.values().find(|f| f.owner == owner && f.is_combatant()).unwrap().id;
    w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: id };
    w.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: w.hub };
    w.step(&[]);
    assert!(w.players[&owner].founding.privateer.is_some());
}

#[test]
fn existing_convoy_saves_keep_their_cargo_and_identifier() {
    let mut f = Fleet::single(EntityId(7), PlayerId(8), ShipKind::Convoy, Vec2::ZERO, FleetOrder::Idle, None);
    f.add_cargo(C::Alloys, 250);
    let saved = serde_json::to_string(&f).unwrap();
    let loaded: Fleet = serde_json::from_str(&saved).unwrap();
    assert_eq!(serde_json::to_string(&ShipKind::Convoy).unwrap(), "\"convoy\"");
    assert_eq!(loaded.cargo_units(), 250);
    assert_eq!(loaded.cargo_capacity(), 400);
    for kind in PLAYER_FREIGHTERS {
        assert_eq!(serde_json::from_str::<ShipKind>(&serde_json::to_string(&kind).unwrap()).unwrap(), kind);
    }
}
