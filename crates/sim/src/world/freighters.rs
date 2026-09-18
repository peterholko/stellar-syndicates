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
fn freight_construction_starts_with_importable_basics_then_adds_industry() {
    // These are construction manifests, not a credit purchase or an automatic
    // hull replacement. Every unit must reach a yard through normal logistics.
    let manifests: [&[(C, f64)]; 6] = [
        &[(C::Alloys, 20.0), (C::Machinery, 8.0), (C::Polymers, 8.0)],
        &[(C::Alloys, 45.0), (C::Machinery, 16.0), (C::Polymers, 16.0)],
        &[(C::Alloys, 90.0), (C::Machinery, 28.0), (C::Polymers, 20.0), (C::Electronics, 10.0)],
        &[(C::Alloys, 180.0), (C::Machinery, 45.0), (C::Electronics, 25.0), (C::HullSections, 12.0)],
        &[(C::Alloys, 320.0), (C::Machinery, 70.0), (C::HullSections, 30.0), (C::PrecisionComponents, 15.0)],
        &[(C::Alloys, 520.0), (C::Machinery, 100.0), (C::HullSections, 60.0),
            (C::PrecisionComponents, 25.0), (C::DriveAssemblies, 16.0)],
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
        C::HullSections | C::PrecisionComponents | C::DriveAssemblies)));
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
    for ((kind, programme), tier) in PLAYER_FREIGHTERS.into_iter().zip(PROGRAMMES).zip([1, 1, 2, 3, 4, 5]) {
        let (mut w, owner, home) = fixture();
        assert_eq!(crate::build::yard_for(kind), (K::Shipyard, tier));
        let what = BuildKind::Ship { ship: kind };
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.set_tier(K::Shipyard, tier);
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
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(K::Shipyard, tier - 1);
        let mut events = Vec::new();
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut events);
        assert!(w.build_queue.is_empty(), "{kind:?}: research cannot bypass the yard");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
            reason: crate::event::BuildRejectReason::NeedsYard { yard: K::Shipyard, required }, ..
        } if required == tier)));
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(K::Shipyard, tier);
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut vec![]);
        assert_eq!(w.build_queue.len(), 1, "{kind:?}: unlocked hull queues without a syndicate");
        assert_eq!(w.build_queue[0].what, what);
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
    assert_eq!(w.players[&owner].founding.stage, crate::founding::FoundingStage::ExportProduction);
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
