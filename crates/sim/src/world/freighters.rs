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
            // §shipbuilding-overhaul: Full and Slow on a fission hull (one
            // tank, one fuel); Stealth needs a Fusion Core.
            let core = if mode == TransitMode::Stealth { (crate::fuel::ReactorGrade::Fusion, crate::fuel::Drive::Zephyr) } else { (crate::fuel::ReactorGrade::Fission, crate::fuel::Drive::Zephyr) };
            let mut f = Fleet::single_on(id, owner, kind, core, start, FleetOrder::MoveTo { dest }, None);
            f.add_cargo(C::MetallicOre, kind.cargo_units());
            f.transit = mode;
            f.drive_state = DriveState::Cruising(crate::transit::Regime::Warp);
            f.vel = Vec2::new(f.transit_speed() * crate::transit::WARP_FACTOR, 0.0);
            let (fuel0, uranium0) = (f.energy_aboard(), f.uranium);
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
            results.push((ticks, fuel0 - f.energy_aboard(), uranium0 - f.uranium, range));
            w.fleets.remove(&id);
        }
        let (full, slow, stealth) = (results[0], results[1], results[2]);
        assert!((slow.0 as i32 - 2 * full.0 as i32).abs() <= 2, "{kind:?}: {results:?}");
        assert!(full.2 > 0.0 && slow.2 > 0.0, "{kind:?}: a fission hull burns Uranium at either throttle");
        assert!((slow.1 / full.1 - 0.25).abs() < 0.001, "{kind:?}: Slow burns a quarter of the energy: {results:?}");
        assert!((slow.3 / full.3 - 4.0).abs() < 1e-9, "the same tank goes four times as far");
        assert!((stealth.1 / full.1 - 1.0).abs() < 0.01, "Stealth's trip energy is Full's, on Deuterium");
        assert!(stealth.2.abs() < 1e-12, "a fusion hull burns no Uranium");
    }
}

#[test]
fn escorts_fly_their_charges_throttle() {
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
        // §shipbuilding-overhaul: a fission escort, so both throttles draw
        // from the one Uranium tank.
        let mut guard = Fleet::single_on(guard_id, owner, ShipKind::Raider, (crate::fuel::ReactorGrade::Fission, crate::fuel::Drive::Zephyr), guard_pos,
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
    assert!(results[0].1.abs() < 1e-12 && results[1].1.abs() < 1e-12, "no Deuterium on a fission hull: {results:?}");
    assert!((results[1].2 / results[0].2 - 0.25).abs() < 1e-6,
        "the escort flies its charge's throttle over the same distance: {results:?}");
}

#[test]
fn slow_commands_are_delayed_and_any_hull_throttles_back() {
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
    // §shipbuilding-overhaul: any formation throttles back, a ship of the
    // line included; Stealth needs fusion-class cores on every hull.
    let bb = w.alloc_entity_id();
    let mut line = Fleet::single(bb, owner, ShipKind::Battleship, pos, FleetOrder::Idle, None);
    line.transit = TransitMode::Slow;
    assert!(line.can_slow() && line.flies_slow(), "a ship of the line idles home on Slow");
    assert_eq!(line.cruise_fuel_mult(), crate::fuel::SLOW_FUEL_MULT);
    assert_eq!(line.transit_speed(), line.max_speed() * crate::fuel::SLOW_SPEED_FRACTION);
    let fission = Fleet::single_on(bb, owner, ShipKind::Corvette, (crate::fuel::ReactorGrade::Fission, crate::fuel::Drive::Zephyr), pos, FleetOrder::Idle, None);
    assert!(!fission.can_stealth(), "no Stealth on a Fission Core");
    assert_eq!(line.effective_transit(), TransitMode::Slow);
}

#[test]
fn freight_construction_starts_with_importable_basics_then_adds_industry() {
    // These are construction manifests, not a credit purchase or an automatic
    // hull replacement. Every unit must reach a yard through normal logistics.
    // §shipbuilding-overhaul: a hull is a kit — engines, a Nav Computer, Hull
    // Sections, a Cargo Bay per 50 units of hold. §hull-ladder (Nov 7 2026):
    // engines are half the kit (×3 a freighter rung), the frame and engines
    // come in the hull's size (heavy from the Heavy Freighter), and the skin
    // is Composites plating.
    let manifests: [&[(C, f64)]; 6] = [
        &[(C::FissionReactor, 2.0), (C::ZephyrDrive, 2.0), (C::NavComputer, 1.0), (C::HullSections, 2.0), (C::CargoBay, 1.0),
            (C::Composites, 4.0), (C::Electronics, 1.0), (C::Machinery, 1.0)],
        &[(C::FissionReactor, 4.0), (C::ZephyrDrive, 4.0), (C::NavComputer, 1.0), (C::HullSections, 15.0), (C::CargoBay, 3.0),
            (C::Composites, 30.0), (C::PrecisionComponents, 3.0), (C::Electronics, 8.0), (C::Machinery, 5.0)],
        &[(C::FissionReactor, 13.0), (C::ZephyrDrive, 13.0), (C::NavComputer, 1.0), (C::HullSections, 44.0), (C::CargoBay, 8.0),
            (C::Composites, 88.0), (C::PrecisionComponents, 9.0), (C::Electronics, 25.0), (C::Machinery, 14.0)],
        &[(C::FissionReactor, 40.0), (C::ZephyrDrive, 40.0), (C::NavComputer, 1.0), (C::HullSections, 133.0), (C::CargoBay, 20.0),
            (C::Composites, 264.0), (C::PrecisionComponents, 27.0), (C::Electronics, 74.0), (C::Machinery, 41.0)],
        &[(C::HeavyFissionReactor, 5.0), (C::HeavyZephyrDrive, 5.0), (C::NavComputer, 1.0), (C::HeavyHullSections, 29.0), (C::CargoBay, 50.0),
            (C::Composites, 110.0), (C::PrecisionComponents, 40.0), (C::Electronics, 108.0), (C::Machinery, 59.0)],
        &[(C::HeavyFissionReactor, 15.0), (C::HeavyZephyrDrive, 15.0), (C::NavComputer, 1.0), (C::HeavyHullSections, 90.0), (C::CargoBay, 120.0),
            (C::Composites, 336.0), (C::PrecisionComponents, 109.0), (C::Electronics, 294.0), (C::Machinery, 161.0)],
    ];
    for (kind, expected) in PLAYER_FREIGHTERS.into_iter().zip(manifests) {
        let reactor = kind.engine_size().reactor(crate::fuel::ReactorGrade::Fission);
        assert_eq!(expected.iter().find(|(c, _)| *c == reactor).unwrap().1, kind.engines() as f64, "{kind:?} engines");
        assert_eq!(expected.iter().find(|(c, _)| *c == C::CargoBay).unwrap().1, (kind.cargo_units() / 50) as f64, "{kind:?} bays");
    }
    for (kind, expected) in PLAYER_FREIGHTERS.into_iter().zip(manifests) {
        let recipe = crate::build::recipe_for(BuildKind::Ship { ship: kind });
        assert_eq!(recipe.costs, expected, "{kind:?} construction manifest");
    }
    let starter = crate::build::TINY_FREIGHTER_RECIPE;
    assert!(starter.costs.iter().map(|(_, n)| n).sum::<f64>() <= 50.0,
        "one Tiny can bring home the entire replacement-hull manifest");
}

#[test]
fn only_the_tiny_freighter_stays_on_the_opening_economy() {
    // §hull-ladder: the Tiny's kit is the charter's (plating from its crate,
    // no precision parts); every freighter from the Small up takes Precision
    // Components, and the heavy sizes wait for the Cruiser's line.
    let has = |recipe: &crate::build::Recipe, good| recipe.costs.iter().any(|(c, _)| *c == good);
    assert!(!has(&crate::build::TINY_FREIGHTER_RECIPE, C::PrecisionComponents));
    assert!(has(&crate::build::SMALL_FREIGHTER_RECIPE, C::PrecisionComponents));
    assert!(crate::build::CONVOY_RECIPE.costs.contains(&(C::Electronics, 25.0)));
    assert!(has(&crate::build::HEAVY_FREIGHTER_RECIPE, C::HeavyHullSections)
        && !has(&crate::build::LARGE_FREIGHTER_RECIPE, C::HeavyHullSections));
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
            2.0 * kind.hull_mass() + capacity * crate::cargo::Commodity::MetallicOre.mass_per_unit()) / capacity;
        let upkeep_per_unit_distance = crate::ship::upkeep_per_sec(kind)
            / (capacity * kind.max_speed());
        if let Some((old_cost, old_cost_per_unit, old_fuel, old_upkeep, old_ticks)) = previous {
            assert!(investment > old_cost, "{kind:?}: more capital in one vulnerable hull");
            // §hull-ladder (§2): ×3 a rung for a hold that grows ×2.4–3, so
            // the steel per unit carried rises gently; the bigger hull buys
            // fewer hulls to crew and fuel, not cheaper steel.
            assert!(investment / capacity < 1.3 * old_cost_per_unit, "{kind:?}: capital per unit carried rises gently");
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
        assert!(!kind.is_combatant() && !kind.broadcasts() && !kind.default_engine().1.jumps());
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
        // §hold-room: ten Provisions fill five room units, ore one a unit.
        let cap = kind.cargo_units();
        w.apply_logistics_load(owner, id, Some(home), C::Provisions, 10, &mut vec![]);
        w.apply_logistics_load(owner, id, Some(home), C::MetallicOre, cap - 5, &mut vec![]);
        assert_eq!(w.fleets[&id].cargo_units(), cap + 5, "{kind:?}");
        assert_eq!(w.fleets[&id].hold_used(), cap as u64 * crate::cargo::ROOM_SCALE as u64, "{kind:?}: full");
        assert_eq!(w.fleets[&id].cargo_stacks().len(), 2);
        let before = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        w.apply_logistics_load(owner, id, Some(home), C::MetallicOre, 1, &mut vec![]);
        assert_eq!(w.fleets[&id].cargo_units(), cap + 5);
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
            // §hull-ladder: a heavy hull's parts are unknown too until the
            // Cruiser's line, so either gate may answer first.
            assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
                reason: crate::event::BuildRejectReason::NeedsResearch | crate::event::BuildRejectReason::ResearchRequired, .. })));
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
            // §licences: an Orbital Shipyard freighter needs the Authority's
            // licence too (world::licences tests the gate).
            w.players.get_mut(&owner).unwrap().licences.insert(kind, 0.0);
            let research = &mut w.players.get_mut(&owner).unwrap().research.completed;
            research.insert(programme.into());
            for (good, _) in crate::build::recipe_for(what).costs {
                if let Some(size) = crate::fuel::size_known_after(*good) { research.insert(size.into()); }
            }
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
fn the_retired_freighter_step_hands_on_and_an_export_draws_no_privateer() {
    let (mut w, owner, _home) = fixture();
    w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::BuildFreighter, w.time);
    let home = w.players[&owner].home;
    let id = w.alloc_entity_id();
    w.fleets.insert(id, Fleet::single(id, owner, ShipKind::TinyFreighter, home, FleetOrder::Idle, None));
    w.step(&[]);
    // §charter-v5: the Freighter left the charter; its step hands on to the
    // Interceptor, and a Hub run is ordinary trade, not a privateer lesson.
    assert_eq!(w.players[&owner].founding.stage, crate::founding::FoundingStage::BuildInterceptor);
    let guard = w.alloc_entity_id();
    w.fleets.insert(guard, Fleet::single(guard, owner, ShipKind::Raider, home, FleetOrder::Guard { target: id }, None));
    let f = w.fleets.get_mut(&id).unwrap();
    f.add_cargo(C::MetallicOre, 20);
    f.order = FleetOrder::MoveTo { dest: w.hub };
    w.step(&[]);
    assert!(w.players[&owner].founding.privateer.is_none());
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
