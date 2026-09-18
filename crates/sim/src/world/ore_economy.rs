//! Real production/administration checks, not a second economy implementation.
use super::*;
use crate::cargo::Commodity as C;
use crate::build::StructureKind as K;
use crate::production::{Assignment, ORE_REFINING};

fn rig(ore: C) -> (World, PlayerId, EntityId, u32) {
    let mut w = World::new(SimConfig::for_players(316, 4));
    w.legacy_test_bootstrap = true;
    w.enclaves.clear();
    let owner = PlayerId(991);
    w.step(&[Command::AddPlayer { id: owner, name: "Ore works".into() }]);
    let home = w.players[&owner].home_system.unwrap();
    let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
    for b in &mut s.bodies {
        b.assignments.clear(); b.structures.clear(); b.deposits.clear();
    }
    s.set_population(0.05);
    s.seed_warehouse();
    s.stockpile = [(C::Provisions, 50.0), (C::Fuel, 20.0), (ore, 20.0)].into();
    s.set_tier(K::Smelter, 1);
    s.assign(K::Smelter, Assignment { refining_ore: Some(ore), ..Assignment::crew(1) });
    let body = s.bodies.iter().find(|b| b.tier(K::Smelter) > 0).unwrap().id;
    (w, owner, home, body)
}
fn site(w: &World, id: EntityId) -> &StarSystem { w.systems.iter().find(|s| s.id == id).unwrap() }
fn site_mut(w: &mut World, id: EntityId) -> &mut StarSystem { w.systems.iter_mut().find(|s| s.id == id).unwrap() }
fn stock(w: &World, id: EntityId, c: C) -> f64 { site(w,id).stockpile.get(&c).copied().unwrap_or(0.0) }

#[test]
fn ore_recovery_changes_yield_while_enrichment_changes_speed() {
    let (baseline, owner, home, _) = rig(C::CupriteOre);
    let run = |programmes: &[&str]| {
        let mut w: World = serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        for id in programmes { w.players.get_mut(&owner).unwrap().research.completed.insert((*id).into()); }
        w.accrue_production(&mut Vec::new());
        [stock(&w,home,C::ConductiveMetals), stock(&w,home,C::Alloys),
            20.0-stock(&w,home,C::CupriteOre), 20.0-stock(&w,home,C::Fuel)]
    };
    let plain = run(&[]);
    let recovered = run(&["mat_ore_recovery"]);
    let faster = run(&["mat_enrichment"]);
    let both = run(&["mat_enrichment", "mat_ore_recovery"]);
    for i in 0..4 {
        let gain = if i < 2 { 1.15 } else { 1.0 };
        assert!((recovered[i] - plain[i]*gain).abs() < 1e-9, "recovery index {i}");
        assert!((faster[i] - plain[i]*1.15).abs() < 1e-9, "speed index {i}");
        assert!((both[i] - faster[i]*gain).abs() < 1e-9, "combined index {i}");
    }
    let mut w = baseline;
    w.players.get_mut(&owner).unwrap().research.completed.insert("mat_ore_recovery".into());
    assert_eq!(w.production_mods(owner).recovery(K::ChemicalWorks),1.0,
        "ore recovery must not improve every industry");
}

#[test]
fn founding_refiner_can_grow_without_a_second_freighter() {
    use crate::founding::FoundingStage as S;
    let (mut w, owner, home, body) = rig(C::MetallicOre);
    let corp = w.players.get_mut(&owner).unwrap();
    corp.founding.enabled = true;
    corp.founding.stage = S::GrowBusiness;
    corp.research.active = Some("mat_enrichment".into());
    w.advance_founding_programs(&[]);
    assert!(w.players[&owner].founding.research_grant_applied);
    assert_eq!(w.players[&owner].research.progress,
        crate::research::cost_of("mat_enrichment") - crate::founding::FIRST_RESEARCH_REMAINING_S);
    site_mut(&mut w,home).set_tier(K::Academy,1);
    site_mut(&mut w,home).assign(K::Academy,Assignment::crew(1));
    let progress = w.players[&owner].research.progress;
    w.step(&[]);
    assert!(w.players[&owner].research.progress>progress,
        "the refining route receives the same first-programme funding basket");
    // The grant is one-time even if the player changes their intended route.
    w.players.get_mut(&owner).unwrap().research.progress = 1.0;
    w.advance_founding_programs(&[]);
    assert_eq!(w.players[&owner].research.progress,1.0);
    assert_eq!(w.players[&owner].founding.stage,S::GrowBusiness,"a Smelter without Enrichment is not the goal");
    w.players.get_mut(&owner).unwrap().research.completed.insert("mat_enrichment".into());
    site_mut(&mut w,home).bodies.iter_mut().find(|b| b.id==body).unwrap()
        .assignments.get_mut(&K::Smelter).unwrap().workers = 0;
    w.accrue_production(&mut Vec::new()); w.advance_founding_programs(&[]);
    assert_eq!(w.players[&owner].founding.stage,S::GrowBusiness);
    site_mut(&mut w,home).bodies.iter_mut().find(|b| b.id==body).unwrap()
        .assignments.get_mut(&K::Smelter).unwrap().workers = 1;
    site_mut(&mut w,home).stockpile.remove(&C::Fuel);
    w.accrue_production(&mut Vec::new()); w.advance_founding_programs(&[]);
    assert_eq!(w.players[&owner].founding.stage,S::GrowBusiness,"a starved Smelter is not operating");
    site_mut(&mut w,home).stockpile.insert(C::Fuel,20.0);
    w.accrue_production(&mut Vec::new()); w.advance_founding_programs(&[]);
    assert_eq!(w.players[&owner].founding.stage,S::BuildAcademy);
    assert!(stock(&w,home,C::Alloys)>0.0);
    assert_eq!(site(&w,home).tier(K::Shipyard),0,"refining does not require a yard");
    assert!(w.fleets.values().filter(|f| f.owner==owner).map(Fleet::freighter_count).sum::<u32>() < 2);
}

#[test]
fn growth_does_not_turn_the_first_research_grant_into_free_followup_research() {
    let (mut w, owner, home, _) = rig(C::MetallicOre);
    let corp = w.players.get_mut(&owner).unwrap();
    corp.founding.enabled = true;
    corp.founding.stage = crate::founding::FoundingStage::GrowBusiness;
    corp.founding.research_grant_applied = true;
    corp.research.completed.insert("mat_enrichment".into());
    corp.research.active = Some("mat_ore_recovery".into());
    let sys = site_mut(&mut w,home);
    sys.bodies.iter_mut().for_each(|b| b.assignments.clear());
    sys.set_tier(K::Academy,1);
    sys.assign(K::Academy,Assignment::crew(1));
    sys.stockpile = [(C::Provisions,50.0)].into();
    w.step(&[]);
    assert_eq!(w.players[&owner].research.progress,0.0);
    assert_eq!(w.players[&owner].founding.stage,crate::founding::FoundingStage::GrowBusiness);
}

#[test]
fn each_ore_pays_one_input_basket_and_all_of_its_outputs() {
    for recipe in ORE_REFINING {
        let ore = recipe.inputs[0].0;
        let (mut w, _, home, body_id) = rig(ore);
        // Other ore in the hold must NOT turn one Smelter into five lines.
        site_mut(&mut w,home).stockpile.insert(C::MetallicOre,20.0);
        let s = site(&w,home);
        let body = s.bodies.iter().find(|b| b.id == body_id).unwrap();
        let yield_mult = crate::explore::converter_site_mult(body,K::Smelter,s.trait_);
        w.accrue_production(&mut Vec::new());
        let output = stock(&w,home,recipe.output);
        assert!(output > 0.0, "{ore:?} must actually refine");
        for (input, units) in recipe.inputs {
            assert!((20.0 - stock(&w,home,*input) - output / yield_mult * units).abs() < 1e-9);
        }
        for (c, ratio) in recipe.byproducts() {
            assert!((stock(&w,home,*c) - output * ratio).abs() < 1e-9, "{ore:?}: {c:?}");
        }
        if ore != C::MetallicOre { assert_eq!(stock(&w,home,C::MetallicOre),20.0); }
    }
}

#[test]
fn refining_requires_workers_and_fuel_and_restarts_without_new_orders() {
    let (mut w, _, home, body) = rig(C::CupriteOre);
    let b = site_mut(&mut w,home).bodies.iter_mut().find(|b| b.id == body).unwrap();
    b.assignments.get_mut(&K::Smelter).unwrap().workers = 0;
    w.accrue_production(&mut Vec::new());
    assert_eq!(stock(&w,home,C::ConductiveMetals),0.0);
    site_mut(&mut w,home).bodies.iter_mut().find(|b| b.id == body).unwrap()
        .assignments.get_mut(&K::Smelter).unwrap().workers = 1;
    site_mut(&mut w,home).stockpile.remove(&C::Fuel);
    w.accrue_production(&mut Vec::new());
    assert_eq!(stock(&w,home,C::CupriteOre),20.0);
    assert_eq!(stock(&w,home,C::Alloys),0.0);
    site_mut(&mut w,home).stockpile.insert(C::Fuel,20.0);
    w.accrue_production(&mut Vec::new());
    assert!(stock(&w,home,C::ConductiveMetals) > 0.0);
    assert!(stock(&w,home,C::Alloys) > 0.0);
}

#[test]
fn boosted_secondary_yields_cannot_overfill_storage() {
    let (mut w, _, home, body) = rig(C::RareMetalOre);
    let s = site_mut(&mut w,home);
    s.bodies.iter_mut().find(|b| b.id == body).unwrap().profile.special = Some(crate::body::BodySpecial::VolcanicMantle);
    let remaining = s.storage_cap() - s.storage_used();
    *s.stockpile.entry(C::Provisions).or_default() += remaining;
    for _ in 0..40 {
        w.accrue_production(&mut Vec::new());
        assert!(site(&w,home).storage_used() <= site(&w,home).storage_cap() + 1e-8);
    }
}

#[test]
fn recipe_is_a_delivered_standing_assignment_and_survives_unstaffing_and_save() {
    let (mut w, owner, home, body) = rig(C::MetallicOre);
    // A remote colony: issuing an administration order cannot change the line.
    site_mut(&mut w,home).pos = site(&w,home).pos + Vec2::new(20_000.0, 0.0);
    let cmd = Command::SetAssignment { player_id: owner, system_id: home,
        body_id: Some(body), structure: K::Smelter, workers: 0,
        specialists: Default::default(), refining_ore: Some(C::TitaniumOre) };
    w.step(&[cmd.clone()]);
    assert_eq!(site(&w,home).bodies.iter().find(|b| b.id == body).unwrap()
        .assignments[&K::Smelter].refining_ore,Some(C::MetallicOre));
    // Isolate receipt from unrelated movement/immigration during the delay.
    w.apply_now(&cmd, &mut Vec::new());
    let mut loaded: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    loaded.fixup_after_load();
    let a = &site(&loaded,home).bodies.iter().find(|b| b.id == body).unwrap().assignments[&K::Smelter];
    assert_eq!((a.workers,a.refining_ore),(0,Some(C::TitaniumOre)));
    let legacy: Assignment = serde_json::from_str(r#"{"workers":1,"specialists":{},"suspended":null}"#).unwrap();
    assert_eq!(crate::production::assigned_converter(K::Smelter,Some(&legacy)).unwrap().inputs[0].0,C::MetallicOre);
    loaded.apply_now(&Command::SetAssignment { player_id: owner, system_id: home,
        body_id: Some(body), structure: K::Smelter, workers: 1,
        specialists: Default::default(), refining_ore: Some(C::Fuel) }, &mut Vec::new());
    assert_eq!(site(&loaded,home).bodies.iter().find(|b| b.id == body).unwrap()
        .assignments[&K::Smelter].refining_ore,Some(C::TitaniumOre),"reject non-ore recipes");
}

#[test]
fn new_galaxies_contain_all_five_ore_families_without_free_refined_deposits() {
    let mut found = std::collections::BTreeSet::new();
    for seed in [7,41,316,2026] {
        let w = World::new(SimConfig::for_players(seed,4));
        for d in w.systems.iter().flat_map(|s| s.all_deposits()) {
            assert!(C::RAW.contains(&d.resource),"new geology mines raw feedstock");
            if d.resource.is_ore() { found.insert(d.resource); }
        }
    }
    assert_eq!(found,ORE_REFINING.iter().map(|r| r.inputs[0].0).collect());
}

#[test]
fn old_market_prices_and_cargo_survive_catalog_growth() {
    let (mut w, owner, _, _) = rig(C::MetallicOre);
    w.players.get_mut(&owner).unwrap().warehouse.insert(C::MetallicOre,50);
    w.market.execute_sell(C::MetallicOre,50);
    let prior = w.market.price(C::MetallicOre);
    let mut json = serde_json::to_value(&w).unwrap();
    for table in ["prices","base","external_supply","external_demand"] {
        for good in [C::CupriteOre,C::TitaniumOre,C::CrystallineOre,C::RareMetalOre,C::ConductiveMetals,C::Titanium] {
            json["market"][table].as_object_mut().unwrap().remove(good.slug());
        }
    }
    let mut loaded: World = serde_json::from_value(json).unwrap();
    loaded.fixup_after_load();
    assert_eq!(loaded.market.price(C::MetallicOre),prior);
    assert_eq!(loaded.players[&owner].warehouse[&C::MetallicOre],50);
    for r in ORE_REFINING {
        assert!(loaded.market.available_to_buy(r.inputs[0].0) >= 50);
        assert!(loaded.market.available_to_sell(r.inputs[0].0) >= 50);
    }
}

/// §ore-ladder migration: a snapshot saved under the old ore ladder loads with
/// its references rebased, live prices carried along at the same ratio, the
/// rare pools clamped to the thin book, and its resting orders in the rebased
/// goods voided with a full escrow refund — while orders in untouched goods
/// keep resting.
#[test]
fn loading_a_snapshot_from_the_old_ore_ladder_rebases_prices_and_refunds_stale_orders() {
    use crate::market::Side;
    let (mut w, owner, _home, _) = rig(C::RareMetalOre);
    {
        let corp = w.players.get_mut(&owner).unwrap();
        corp.warehouse.insert(C::RareMetalOre, 10);
        corp.credits = 5_000.0;
    }
    w.receive_admin_for_test(&[
        Command::PlaceLimitOrder { player_id: owner, side: Side::Sell, commodity: C::RareMetalOre, units: 10, limit_price: 19.0 },
        Command::PlaceLimitOrder { player_id: owner, side: Side::Buy, commodity: C::RareElements, units: 4, limit_price: 20.0 },
        Command::PlaceLimitOrder { player_id: owner, side: Side::Buy, commodity: C::Alloys, units: 2, limit_price: 25.0 },
    ]);
    assert_eq!(w.book.len(), 3, "all three orders rest");
    assert_eq!(w.players[&owner].warehouse.get(&C::RareMetalOre).copied().unwrap_or(0), 0, "sell escrow left the warehouse");
    let credits_after_escrow = w.players[&owner].credits;

    // Age the snapshot to the references those orders were priced against.
    let mut json = serde_json::to_value(&w).unwrap();
    json["market"]["base"]["rare_metal_ore"] = 19.0.into();
    json["market"]["prices"]["rare_metal_ore"] = 20.9.into();
    json["market"]["base"]["rare_elements"] = 22.0.into();
    json["market"]["prices"]["rare_elements"] = 22.0.into();
    json["market"]["external_supply"]["rare_metal_ore"] = 1600.0.into();
    let mut loaded: World = serde_json::from_value(json).unwrap();
    loaded.fixup_after_load();

    let rare = crate::market::base_price(C::RareMetalOre);
    assert!((loaded.market.price(C::RareMetalOre) - 20.9 * rare / 19.0).abs() < 1e-9, "live price rides the rebase");
    assert_eq!(loaded.market.available_to_buy(C::RareMetalOre), crate::market::THIN_LIQUIDITY.external_cap as u32);
    assert_eq!(loaded.book.len(), 1, "only the Alloys order still rests");
    assert_eq!(loaded.book[0].commodity, C::Alloys);
    assert_eq!(loaded.players[&owner].warehouse.get(&C::RareMetalOre).copied().unwrap_or(0), 10, "sell escrow returned");
    assert!((loaded.players[&owner].credits - (credits_after_escrow + 4.0 * 20.0)).abs() < 1e-9, "buy escrow returned");

    // A second load finds nothing to rebase and touches nothing.
    let price = loaded.market.price(C::RareMetalOre);
    loaded.fixup_after_load();
    assert_eq!(loaded.book.len(), 1);
    assert_eq!(loaded.market.price(C::RareMetalOre), price);
}
