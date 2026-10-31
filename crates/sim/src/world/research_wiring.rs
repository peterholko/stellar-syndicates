//! §research-wiring (R2, Oct 22 2026): every visible research programme does
//! something in play. One test per effect family; the reader test in
//! research.rs keeps new keys from going inert again.
use super::*;
use crate::build::{BuildKind, SlotPool, StructureKind as K};
use crate::cargo::Commodity as C;
use crate::research::{Cap, DesignationTarget};
use crate::specialist::SpecialistKind as S;

/// A developed home with goods for anything, free of the charter.
fn planet() -> (World, PlayerId, EntityId, u32) {
    let mut w = World::new(SimConfig::for_players(123, 4));
    w.legacy_test_bootstrap = true;
    w.enclaves.clear();
    let owner = PlayerId(5701);
    w.step(&[Command::AddPlayer { id: owner, name: "Research wiring".into() }]);
    let home = w.players[&owner].home_system.unwrap();
    let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    sys.set_population(0.05);
    for body in &mut sys.bodies {
        body.structures.clear();
        body.assignments.clear();
    }
    for c in C::ALL { sys.stockpile.insert(c, 10_000.0); }
    let body = sys.bodies.iter().find(|b| b.has_deposit_for(K::MiningComplex)
        && b.pool_slots(SlotPool::Industrial) > 0).unwrap().id;
    (w, owner, home, body)
}

fn grant(w: &mut World, owner: PlayerId, programme: &str) {
    assert!(crate::research::programme(programme).is_some(), "{programme}");
    w.players.get_mut(&owner).unwrap().research.completed.insert(programme.into());
    w.refresh_fleet_fuel_research();
}

fn sys(w: &World, id: EntityId) -> &StarSystem {
    w.systems.iter().find(|s| s.id == id).unwrap()
}

fn sys_mut(w: &mut World, id: EntityId) -> &mut StarSystem {
    w.systems.iter_mut().find(|s| s.id == id).unwrap()
}

fn develop(w: &mut World, owner: PlayerId, home: EntityId, body: u32, kind: K) -> u64 {
    let before = w.build_queue.len();
    let events = w.step(&[Command::DevelopSystem { player_id: owner, system_id: home, body_id: Some(body), upgrade: kind }]);
    assert_eq!(w.build_queue.len(), before + 1, "accepted: {events:?}");
    w.build_queue.last().unwrap().id
}

fn waiting(w: &World, id: u64) -> bool {
    w.build_queue.iter().find(|j| j.id == id).unwrap().is_queued_structure()
}

fn designate(w: &mut World, owner: PlayerId, cap: Cap, target: DesignationTarget) -> Vec<Event> {
    w.step(&[Command::SetDesignation { player_id: owner, cap, target }])
}

#[test]
fn bulk_storage_enlarges_warehouses_but_not_the_landing_holds() {
    let (mut w, owner, home, body) = planet();
    let s = sys_mut(&mut w, home);
    s.bodies.iter_mut().find(|b| b.id == body).unwrap().set_tier(K::Warehouse, 2);
    s.landing_holds = 1_000.0;
    let (cap, holds) = (s.storage_cap(), s.landing_holds_cap());
    grant(&mut w, owner, "mat_bulk_storage");
    let s = sys(&w, home);
    assert!((s.storage_cap() - holds - (cap - holds) * 1.5).abs() < 1e-9, "{} vs {cap}", s.storage_cap());
}

#[test]
fn flash_forges_build_two_different_structures_on_a_world_at_once() {
    let (mut w, owner, home, body) = planet();
    let yard = develop(&mut w, owner, home, body, K::Shipyard);
    let mine = develop(&mut w, owner, home, body, K::MiningComplex);
    assert!(!waiting(&w, yard) && waiting(&w, mine), "one slot without the research");
    grant(&mut w, owner, "mat_foundry_v_flash_forges");
    w.step(&[]);
    assert!(!waiting(&w, yard) && !waiting(&w, mine), "two slots");
    let second = develop(&mut w, owner, home, body, K::Shipyard);
    w.step(&[]);
    assert!(waiting(&w, second), "a second order for the same structure waits for the first");
}

#[test]
fn broad_curricula_run_two_courses_and_experts_need_their_research() {
    let (mut w, owner, home, body) = planet();
    let s = sys_mut(&mut w, home);
    let b = s.bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.set_tier(K::Academy, 1);
    b.set_tier(K::Habitat, 2);
    let train = |w: &mut World, kind| w.step(&[Command::TrainSpecialist { player_id: owner, system_id: home, specialist: kind }]);
    train(&mut w, S::Geologist);
    train(&mut w, S::Xenobiologist);
    let courses: Vec<u64> = w.build_queue.iter().filter(|j| matches!(j.what, BuildKind::Train { .. })).map(|j| j.id).collect();
    assert_eq!(courses.len(), 2);
    assert!(!waiting(&w, courses[0]) && waiting(&w, courses[1]), "one course at a time");
    grant(&mut w, owner, "life_talent_iv_broad_curricula");
    w.step(&[]);
    assert!(!waiting(&w, courses[1]), "two with Broad Curricula");
    // An expert course needs Expert Training and a specialist to raise.
    sys_mut(&mut w, home).specialists.insert(S::Geologist, 1);
    let before = w.build_queue.len();
    train(&mut w, S::ExpertGeologist);
    assert_eq!(w.build_queue.len(), before, "not researched");
    grant(&mut w, owner, "life_talent_iii_expert_training");
    train(&mut w, S::ExpertGeologist);
    assert_eq!(w.build_queue.len(), before + 1);
    assert_eq!(sys(&w, home).specialists.get(&S::Geologist), None, "the specialist is in school");
    train(&mut w, S::ExpertGeologist);
    assert_eq!(w.build_queue.len(), before + 1, "nobody left to raise");
}

#[test]
fn founders_institutes_graduate_experts() {
    let (mut w, owner, home, body) = planet();
    let b = sys_mut(&mut w, home).bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.set_tier(K::Academy, 1);
    b.set_tier(K::Habitat, 1);
    grant(&mut w, owner, "life_talent_v_founders_institutes");
    w.step(&[Command::TrainSpecialist { player_id: owner, system_id: home, specialist: S::Geologist }]);
    let due = w.build_queue.iter().find(|j| matches!(j.what, BuildKind::Train { .. })).unwrap().complete_tick;
    while w.tick <= due { w.step(&[]); }
    assert_eq!(sys(&w, home).specialists.get(&S::ExpertGeologist), Some(&1));
}

#[test]
fn experts_skill_a_line_further_and_housing_caps_who_works() {
    use crate::production::{skill_factor, skill_factor_points, skill_points};
    for (m, t) in [(0, 3), (1, 3), (2, 3), (3, 3), (5, 3)] {
        assert_eq!(skill_factor_points(skill_points(0, m, t), t), skill_factor(m, t), "plain specialists read as before");
    }
    assert_eq!(skill_factor_points(skill_points(2, 0, 2), 2), 2.25);
    assert!(skill_factor_points(skill_points(1, 1, 2), 2) > 1.75);
    let (mut w, owner, home, body) = planet();
    let s = sys_mut(&mut w, home);
    let b = s.bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.set_tier(K::MiningComplex, 4);
    b.set_tier(K::Habitat, 1);
    b.assignments.insert(K::MiningComplex, crate::production::Assignment {
        workers: 0, specialists: [(S::Geologist, 4)].into(), ..Default::default() });
    s.specialists.insert(S::Geologist, 4);
    assert_eq!(s.specialist_housing(), 2);
    assert_eq!(s.effective_specialists()[&(body, K::MiningComplex)].0, 2, "two housed, two idle");
    grant(&mut w, owner, "life_talent_v_twin_campuses");
    let s = sys(&w, home);
    assert_eq!(s.specialist_housing(), 3);
    assert_eq!(s.effective_specialists()[&(body, K::MiningComplex)].0, 3);
}

#[test]
fn crown_designations_need_research_hold_a_week_and_move_when_lost() {
    let (mut w, owner, home, body) = planet();
    let target = DesignationTarget::Body { system: home, body };
    let refused = |events: &[Event]| events.iter().any(|e| matches!(e.payload, EventPayload::DesignationRefused { .. }));
    assert!(refused(&designate(&mut w, owner, Cap::CrownProject, target)), "not researched");
    grant(&mut w, owner, "mat_foundry_v_crown_project");
    let (industrial, infrastructure) = {
        let b = sys(&w, home).bodies.iter().find(|b| b.id == body).unwrap();
        (b.industrial_slots(), b.infrastructure_slots())
    };
    assert!(!refused(&designate(&mut w, owner, Cap::CrownProject, target)));
    w.step(&[]);
    let b = sys(&w, home).bodies.iter().find(|b| b.id == body).unwrap();
    assert_eq!((b.industrial_slots(), b.infrastructure_slots()), (industrial + 1, infrastructure + 1));
    let other = sys(&w, home).bodies.iter().find(|b| b.id != body && !b.is_belt()).unwrap().id;
    let elsewhere = DesignationTarget::Body { system: home, body: other };
    assert!(refused(&designate(&mut w, owner, Cap::CrownProject, elsewhere)), "locked for a week");
    w.time += crate::research::DESIGNATION_LOCK_S;
    assert!(!refused(&designate(&mut w, owner, Cap::CrownProject, elsewhere)));
    // Crown Vein: one band richer on its body.
    grant(&mut w, owner, "mat_deepcrust_v_crown_vein");
    assert!(!refused(&designate(&mut w, owner, Cap::CrownVein, target)));
    let mods = w.production_mods(owner);
    assert!(mods.geology(home, body).crown && !mods.geology(home, other).crown);
    let b = sys(&w, home).bodies.iter().find(|b| b.id == body).unwrap();
    let ore = b.deposits.iter().find(|d| d.resource.is_mineable_mineral()).unwrap().resource;
    assert!(b.extraction_mult_with(ore, mods.geology(home, body)) > b.extraction_mult(ore));
}

#[test]
fn mantle_taps_lift_poor_ground_and_end_the_deep_deposits_lag() {
    use crate::body::Geology;
    use crate::explore::GeologyMods;
    let taps = GeologyMods { floor_average: true, ..Default::default() };
    assert_eq!(Geology::UltraPoor.researched(taps), Geology::Average);
    assert_eq!(Geology::Rich.researched(taps), Geology::Rich);
    assert_eq!(Geology::Rich.researched(GeologyMods { floor_average: true, crown: true, ..Default::default() }), Geology::UltraRich);
    let (mut w, owner, _, _) = planet();
    assert!(!w.production_mods(owner).mantle_taps);
    grant(&mut w, owner, "mat_deepcrust_v_mantle_taps");
    assert!(w.production_mods(owner).mantle_taps);
}

#[test]
fn production_research_changes_baskets_yields_and_by_products() {
    let (mut w, owner, _, _) = planet();
    for p in ["mat_autoforges", "life_hydroponics", "life_gene_crops", "mat_foundry_iii_slag_reclamation",
        "mat_deepcrust_iv_trace_refining", "life_civic_rations"] {
        grant(&mut w, owner, p);
    }
    let m = w.production_mods(owner);
    assert!((m.input_mult(K::MachineWorks) - 0.8).abs() < 1e-9);
    assert!((m.input_mult(K::Agroplex) - 0.8).abs() < 1e-9);
    assert!((m.recovery(K::Agroplex) - 1.15 * 1.25).abs() < 1e-9);
    assert_eq!(m.byproduct(K::Smelter), Some((C::Silicates, crate::production::SLAG_SILICATES_PER_OUTPUT)));
    assert!(m.traces(C::MetallicOre) && !m.traces(C::Volatiles));
    let rationing = crate::production::food_factor_with(K::Smelter, crate::colony::FoodState::Rationing, m.rationing);
    assert!((rationing - 0.92).abs() < 1e-9);
}

#[test]
fn an_agroplex_makes_more_from_less_with_crop_research() {
    let run = |research: bool| {
        let (mut w, owner, home, body) = planet();
        if research {
            grant(&mut w, owner, "life_hydroponics");
            grant(&mut w, owner, "life_gene_crops");
        }
        let s = sys_mut(&mut w, home);
        s.stockpile.clear();
        s.stockpile.insert(C::Biomass, 500.0);
        s.stockpile.insert(C::Provisions, 500.0);
        let b = s.bodies.iter_mut().find(|b| b.id == body).unwrap();
        b.set_tier(K::Warehouse, 6);
        b.set_tier(K::Agroplex, 1);
        b.assignments.insert(K::Agroplex, crate::production::Assignment::crew(1));
        for _ in 0..50 { w.step(&[]); }
        let s = sys(&w, home);
        (500.0 - s.stockpile.get(&C::Biomass).copied().unwrap_or(0.0), s.stockpile.get(&C::Provisions).copied().unwrap_or(0.0))
    };
    let (used, food) = run(false);
    let (used_r, food_r) = run(true);
    assert!(used > 0.0 && food_r > food, "{food} → {food_r}");
    assert!(used_r < used, "{used} → {used_r}");
}

#[test]
fn survey_protocols_shorten_surveys_and_prospecting() {
    let (mut w, owner, _, _) = planet();
    let base = w.prospect_secs(owner);
    grant(&mut w, owner, "comp_survey_protocols");
    assert!((w.prospect_secs(owner) - base / 1.25).abs() < 1e-9);
}

#[test]
fn colony_ships_land_with_their_owners_research() {
    let (mut w, owner, _, _) = planet();
    let plain = w.colony_landing_terms(owner);
    assert_eq!(plain.settlers, crate::colony::COLONY_FOUNDING_POP);
    assert!(!plain.habitat && !plain.generation);
    grant(&mut w, owner, "prop_heavy_lifters");
    grant(&mut w, owner, "life_cryo_berths");
    grant(&mut w, owner, "life_growth_v_generation_charters");
    let terms = w.colony_landing_terms(owner);
    assert!((terms.settlers - 0.002).abs() < 1e-12, "1.5 cohorts round up to two: {}", terms.settlers);
    let target = w.systems.iter().find(|s| s.owner.is_none() && !s.bodies.is_empty()).unwrap().id;
    let s = sys_mut(&mut w, target);
    s.seed_population(terms.settlers);
    s.seed_command_centers();
    s.seed_warehouse();
    terms.apply(s);
    let site = s.site_for(K::Habitat).unwrap();
    assert_eq!(s.bodies.iter().find(|b| b.id == site).unwrap().tier(K::Habitat), 1);
    assert_eq!(s.specialists.values().sum::<u32>(), 1);
    assert_eq!(s.food_state, crate::colony::FoodState::WellSupplied);
    assert!(s.stockpile.get(&C::Provisions).copied().unwrap_or(0.0) > 0.0);
}

#[test]
fn prospecting_charters_reveal_a_new_holdings_pockets() {
    let (mut w, owner, _, _) = planet();
    let Some(target) = w.systems.iter().find(|s| s.owner.is_none()
        && s.bodies.iter().any(|b| b.is_belt() && !b.pockets.is_empty())).map(|s| s.id) else { return };
    w.chart_new_holding(owner, target);
    assert!(w.pending_pocket_reports.is_empty(), "not researched");
    grant(&mut w, owner, "mat_deepcrust_iv_prospecting_charters");
    w.chart_new_holding(owner, target);
    assert!(!w.pending_pocket_reports.is_empty());
}

#[test]
fn xenoacclimation_settles_marginal_worlds() {
    let (mut w, owner, home, _) = planet();
    let s = sys_mut(&mut w, home);
    let b = s.bodies.iter_mut().find(|b| !b.habitable && !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant).unwrap();
    let id = b.id;
    b.kind = crate::body::BodyKind::Terrestrial;
    b.profile.environment = crate::body::Environment::Marginal;
    let slots = b.infrastructure_slots();
    grant(&mut w, owner, "life_growth_iv_xenoacclimation");
    let b = sys(&w, home).bodies.iter().find(|b| b.id == id).unwrap();
    assert!(b.acclimated && b.settles());
    assert_eq!(b.infrastructure_slots(), slots + 1);
}

fn fleet_at(w: &mut World, owner: PlayerId, kind: ShipKind, pos: Vec2, order: FleetOrder) -> EntityId {
    let id = w.alloc_entity_id();
    w.fleets.insert(id, Fleet::single(id, owner, kind, pos, order, None));
    id
}

#[test]
fn express_charters_and_long_patrol_tune_freighters_and_warships() {
    let (mut w, owner, home, _) = planet();
    let pos = sys(&w, home).pos + Vec2::new(30_000.0, 0.0);
    let freighter = fleet_at(&mut w, owner, ShipKind::SmallFreighter, pos, FleetOrder::Idle);
    let raider = fleet_at(&mut w, owner, ShipKind::Raider, pos, FleetOrder::Idle);
    w.refresh_fleet_fuel_research();
    let (speed, burn) = (w.fleets[&freighter].max_speed(), w.fleets[&raider].cruise_mult_for(false));
    grant(&mut w, owner, "prop_line_express_charters");
    grant(&mut w, owner, "prop_expedition_iv_long_patrol");
    assert!((w.fleets[&freighter].max_speed() - speed * 1.25).abs() < 1e-9);
    assert!((w.fleets[&raider].cruise_mult_for(false) - burn * 0.7).abs() < 1e-9);
}

#[test]
fn a_mass_stream_speeds_its_owners_fleets_between_its_ends() {
    let (mut w, owner, home, _) = planet();
    let other = w.systems.iter().filter(|s| s.owner.is_none() && s.id != home)
        .min_by(|a, b| a.pos.distance(sys(&w, home).pos).total_cmp(&b.pos.distance(sys(&w, home).pos))).unwrap().id;
    sys_mut(&mut w, other).owner = Some(owner);
    let (a, b) = (sys(&w, home).pos, sys(&w, other).pos);
    let mid = a + (b - a) * 0.5;
    let bound = fleet_at(&mut w, owner, ShipKind::SmallFreighter, mid, FleetOrder::MoveTo { dest: b });
    let astray = fleet_at(&mut w, owner, ShipKind::SmallFreighter, mid, FleetOrder::MoveTo { dest: mid + Vec2::new(0.0, 50_000.0) });
    w.refresh_fleet_fuel_research();
    let speed = w.fleets[&bound].max_speed();
    grant(&mut w, owner, "prop_line_mass_streams");
    let events = designate(&mut w, owner, Cap::MassStreams, DesignationTarget::SystemPair(home, other));
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::DesignationSet { .. })), "{events:?}");
    w.refresh_fleet_fuel_research();
    assert!((w.fleets[&bound].max_speed() - speed * crate::world::fuel_research::MASS_STREAM_SPEED).abs() < 1e-9);
    assert_eq!(w.fleets[&astray].max_speed(), speed, "only toward an end");
    // A held blockade at either end stops the stream.
    sys_mut(&mut w, other).blockade = Some(crate::galaxy::Blockade { by: PlayerId(1), since: 0.0, held_since: Some(0.0),
        siege_since: None, anchored: false, catch: 0.0, catch_stabilized: 0.0 });
    w.refresh_fleet_fuel_research();
    assert_eq!(w.fleets[&bound].max_speed(), speed);
}

#[test]
fn underway_refit_lets_a_tender_mend_hulls_from_carried_alloys() {
    let (mut w, owner, home, _) = planet();
    let pos = sys(&w, home).pos + Vec2::new(40_000.0, 0.0);
    let tender = fleet_at(&mut w, owner, ShipKind::Convoy, pos, FleetOrder::Idle);
    let t = w.fleets.get_mut(&tender).unwrap();
    t.set_fitted(ShipKind::Convoy, &crate::Loadout::new(vec![crate::ModuleKind::FuelTransferRig]), 1);
    t.add_cargo(C::Alloys, 3);
    let hurt = fleet_at(&mut w, owner, ShipKind::Raider, pos + Vec2::new(50.0, 0.0), FleetOrder::Idle);
    w.fleets.get_mut(&hurt).unwrap().ships[0].hp -= 100.0;
    let deficit = |w: &World| w.fleets[&hurt].ships[0].deficit();
    for _ in 0..30 { w.field_repairs(); }
    assert_eq!(deficit(&w), 100.0, "not researched");
    grant(&mut w, owner, "prop_expedition_v_underway_refit");
    for _ in 0..30 { w.field_repairs(); }
    assert!(deficit(&w) < 100.0);
    assert!(w.fleets[&tender].cargo_amount(C::Alloys) < 3, "paid in Alloys");
}

#[test]
fn survey_corps_scouts_survey_uncharted_systems_on_arrival() {
    let (mut w, owner, home, _) = planet();
    let target = w.systems.iter().filter(|s| s.owner.is_none() && s.id != home)
        .min_by(|a, b| a.pos.distance(sys(&w, home).pos).total_cmp(&b.pos.distance(sys(&w, home).pos))).unwrap();
    let (id, pos) = (target.id, target.pos);
    assert!(w.well_system(pos).is_some_and(|s| s.id == id), "the star sits in its own well");
    let scout = fleet_at(&mut w, owner, ShipKind::Scout, pos, FleetOrder::Idle);
    w.survey_corps();
    assert!(matches!(w.fleets[&scout].order, FleetOrder::Idle), "not researched");
    grant(&mut w, owner, "comp_watch_v_survey_corps");
    w.survey_corps();
    assert!(matches!(w.fleets[&scout].order, FleetOrder::Survey { system, .. } if system == id));
}

#[test]
fn beneficiation_floors_poor_ground() {
    use crate::body::Geology;
    let (mut w, owner, home, body) = planet();
    let b = sys_mut(&mut w, home).bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.profile.geology = Geology::UltraPoor;
    let ore = b.deposits.iter().find(|d| d.resource.is_mineable_mineral()).unwrap().resource;
    let poor = b.extraction_mult(ore);
    grant(&mut w, owner, "mat_beneficiation");
    let mods = w.production_mods(owner).geology(home, body);
    let b = sys(&w, home).bodies.iter().find(|b| b.id == body).unwrap();
    assert!(b.extraction_mult_with(ore, mods) >= poor / 0.5 * crate::explore::BENEFICIATION_FLOOR - 1e-9);
}

#[test]
fn gravimetric_survey_charts_the_systems_around_yours() {
    let (mut w, owner, home, _) = planet();
    let near = w.systems.iter().filter(|s| s.id != home && s.owner.is_none()
        && s.pos.distance(sys(&w, home).pos) <= GRAVIMETRIC_RANGE_SU).map(|s| s.id).next();
    let Some(near) = near else { return };
    w.players.get_mut(&owner).unwrap().surveyed.remove(&near);
    w.tick = 30 * crate::config::TICK_HZ as u64 * 100;
    w.gravimetric_survey(&mut Vec::new());
    assert!(!w.pending_survey_reports.iter().any(|r| r.recipient == owner && r.system == near), "not researched");
    grant(&mut w, owner, "comp_gravimetric_survey");
    let mut events = Vec::new();
    w.gravimetric_survey(&mut events);
    assert!(w.pending_survey_reports.iter().any(|r| r.recipient == owner && r.system == near));
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SurveyCompleted { system, .. } if system == near)));
}

#[test]
fn orbital_habitats_house_people_anywhere_and_open_gas_giants() {
    let (mut w, owner, home, _) = planet();
    let s = sys_mut(&mut w, home);
    let giant = s.bodies.iter_mut().find(|b| !b.is_belt() && b.parent.is_none()).unwrap();
    giant.kind = crate::body::BodyKind::GasGiant;
    giant.profile.environment = crate::body::Environment::Uninhabitable;
    let id = giant.id;
    let (slots, mult) = (giant.infrastructure_slots(), giant.habitat_capacity_mult());
    grant(&mut w, owner, "life_growth_iii_orbital_habitats");
    let giant = sys(&w, home).bodies.iter().find(|b| b.id == id).unwrap();
    assert_eq!(giant.infrastructure_slots(), slots + 1, "a gas giant gains a habitat site");
    assert!((giant.habitat_capacity_mult() - mult / 0.5).abs() < 1e-9, "housing as on a Marginal world");
}
