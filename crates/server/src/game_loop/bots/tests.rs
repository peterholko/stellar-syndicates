use super::*;

fn game(players: u32, bots: u32) -> GameLoop {
    let (status, _) = watch::channel(ServerStatus::default());
    let (estimates, _) = mpsc::unbounded_channel();
    let mut game = GameLoop::new(World::new(sim::SimConfig::for_players(0xC0FFEE, players)), 1.0, status, estimates);
    game.configure_bots(Some(bots)).unwrap();
    game
}

fn advance(game: &mut GameLoop, seconds: f64) {
    let end = game.world.time + seconds;
    while game.world.time < end { game.tick(); }
}

#[test]
fn eight_bots_join_an_eight_player_galaxy_with_normal_starts() {
    let g = game(8, 8);
    assert_eq!(g.world.config.max_players, 8);
    // 60 full + 8 homes + 52 sparse, plus each home's six-system neighbourhood.
    assert_eq!(g.world.systems.len(), 104 + 8 * sim::galaxy::HOME_NEIGHBOURS);
    assert_eq!(g.world.players.len(), 8);
    assert_eq!(g.bots.len(), 8);
    assert_eq!(g.sessions.connection_count(), 0);
    let homes: BTreeSet<_> = g.world.players.values().map(|p| p.home_system.unwrap()).collect();
    assert_eq!(homes.len(), 8);
    for (&owner, corp) in &g.world.players {
        assert!(owner.0 > i64::MAX as u64 && !owner.is_sentinel());
        let home = g.world.systems.iter().find(|s| Some(s.id) == corp.home_system).unwrap();
        assert!((home.population() - 0.002).abs() < 1e-6);
        assert_eq!(home.tier(K::MiningComplex), 0);
        assert_eq!(home.tier(K::Shipyard), 0);
        assert_eq!(home.tier(K::CommandCenter), 1);
        assert_eq!(corp.research.completed.len(), 0);
        let own: Vec<_> = g.world.fleets.values().filter(|f| f.owner == owner).collect();
        assert_eq!(own.len(), 2, "§charter v3: a Tiny Freighter and an Interceptor");
        assert_eq!(own.iter().map(|f| f.count(H::TinyFreighter)).sum::<u32>(), 1);
        assert_eq!(own.iter().map(|f| f.count(H::Raider)).sum::<u32>(), 1);
        assert_eq!(own.iter().map(|f| f.count(H::Scout)).sum::<u32>(), 0);
        assert_eq!(own.iter().map(|f| f.cargo_capacity()).sum::<u32>(), 50);
    }
}

#[test]
fn every_bot_starts_developing_without_a_socket() {
    let mut g = game(8, 8);
    advance(&mut g, 90.0);
    for (&owner, bot) in &g.bots {
        let p = g.bot_observation(owner).unwrap();
        // Mine, workforce, persistent guard: safety holds need not generate a
        // fourth order just to prove that the offline controller is alive.
        assert!(bot.commands_sent >= 3, "{owner} must make decisions without a session");
        assert_eq!(p.home.system.tier(K::MiningComplex), 1);
        assert!(p.home.system.bodies.iter().any(|b| b.assignments.get(&K::MiningComplex)
            .is_some_and(|a| a.workers > 0)));
        // Nearby recovery traffic can also expose a neighbour's pirate. A
        // persistent escort legitimately intercepts that threat, not only its
        // own tutorial privateer, before its next guard report arrives.
        let intercepting=g.world.fleets.values().any(|f| f.owner==owner && f.is_combatant()
            && matches!(f.order, sim::ship::FleetOrder::Intercept{target}
                if g.world.fleets.get(&target).is_some_and(|enemy| enemy.owner!=owner)));
        assert!(intercepting || p.own().any(|f| f.guard_target.is_some() || f.kind == H::Raider && p.fighting.contains(&f.id)),
            "{owner}: a guard either remains assigned or reacts to a threat; status={}, fleets={:?}",
            bot.status, g.world.fleets.values().filter(|f| f.owner==owner).map(|f|(f.id,&f.order,f.fuel)).collect::<Vec<_>>());
    }
}

#[test]
fn bots_price_command_upgrades_and_respect_headquarters_research() {
    let g = game(2, 1);
    let mut p = g.bot_observation(PlayerId(BOT_ID_BASE)).unwrap();
    let project = p.structure(K::CommandCenter, 2).unwrap();
    let costs = p.project_costs(project);
    assert_eq!(costs[&C::Alloys], 40.0);
    assert_eq!(costs[&C::Machinery], 12.0);
    assert_eq!(costs[&C::Electronics], 8.0);
    p.home.system.set_tier(K::CommandCenter, 2);
    assert!(p.structure(K::CommandCenter, 3).is_none());
    p.research.completed.insert("mat_prefab_construction".into());
    let costs = p.project_costs(p.structure(K::CommandCenter, 3).unwrap());
    assert_eq!(costs[&C::Alloys], 120.0);
    assert_eq!(costs[&C::Polymers], 20.0);
    p.home.system.set_tier(K::CommandCenter, 3);
    assert!(p.structure(K::CommandCenter, 4).is_none());
}

#[test]
fn bot_checkpoint_restores_intentions_without_rejoining() {
    let mut g = game(2, 2);
    advance(&mut g, 12.0);
    let expected = serde_json::to_value(&g.bots).unwrap();
    let checkpoint = g.durable_checkpoint();
    let saved = serde_json::from_slice(&serde_json::to_vec(&checkpoint).unwrap()).unwrap();
    let (status, _) = watch::channel(ServerStatus::default());
    let (estimates, _) = mpsc::unbounded_channel();
    let mut restored = GameLoop::restore(saved, 1.0, status, estimates);
    restored.configure_bots(None).unwrap();
    restored.configure_bots(Some(2)).unwrap();
    assert_eq!(serde_json::to_value(&restored.bots).unwrap(), expected);
    assert_eq!(restored.world.players.len(), 2);
    assert!(!restored.pending.iter().any(|c| matches!(c, Command::AddPlayer { .. })));
    assert!(restored.configure_bots(Some(8)).is_err());

    let mut old = serde_json::to_value(game(2, 0).durable_checkpoint()).unwrap();
    old.as_object_mut().unwrap().remove("bots");
    let saved: GalaxyCheckpoint = serde_json::from_value(old).unwrap();
    let (status, _) = watch::channel(ServerStatus::default());
    let (estimates, _) = mpsc::unbounded_channel();
    assert!(GameLoop::restore(saved, 1.0, status, estimates).bots.is_empty());
}

#[test]
fn bot_observations_do_not_read_unarrived_market_or_fleet_truth() {
    let mut g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    // Hold the policy still while constructing an arrived baseline.
    g.bots.get_mut(&owner).unwrap().next_think = 1e9;
    advance(&mut g, 100.0);
    let before = g.bot_observation(owner).unwrap();
    let credits = before.account.as_ref().unwrap().credits;
    g.world.players.get_mut(&owner).unwrap().credits += 1_000_000.0;
    g.world.players.get_mut(&owner).unwrap().warehouse.insert(C::Machinery, 999);
    g.market_accounts.record(&g.world);
    let unseen = g.bot_observation(owner).unwrap();
    assert_eq!(unseen.account.as_ref().unwrap().credits, credits);
    assert_ne!(unseen.account.as_ref().unwrap().warehouse.get(&C::Machinery), Some(&999));
    // Even though the controller is hosted by the server, a hidden relocation
    // cannot change its delivered sighting until that sample's light arrives.
    let fleet = before.own().find(|g| g.kind == H::TinyFreighter).unwrap().id;
    let old_pos = before.own().find(|g| g.id == fleet).unwrap().pos;
    g.world.fleets.get_mut(&fleet).unwrap().pos = old_pos + Vec2::new(80_000.0, 0.0);
    g.world.time += DT;
    g.world.tick += 1;
    g.history.record(&g.world);
    let unseen = g.bot_observation(owner).unwrap();
    assert_eq!(unseen.own().find(|g| g.id == fleet).unwrap().pos, old_pos);
}

#[test]
fn bot_market_commands_still_travel_and_do_not_repeat_inside_the_round_trip() {
    let mut g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    g.bots.get_mut(&owner).unwrap().next_think = 1e9;
    advance(&mut g, 100.0);
    let picture = g.bot_observation(owner).unwrap();
    let initial = g.world.players[&owner].warehouse.get(&C::Electronics).copied().unwrap_or(0);
    let delay = sim::transit::delay(picture.cc, picture.hub, picture.c);
    g.pending.push(Command::MarketBuy { player_id: owner, commodity: C::Electronics, units: 1, max_unit_price: None });
    g.tick();
    assert_eq!(g.world.players[&owner].warehouse.get(&C::Electronics).copied().unwrap_or(0), initial);
    advance(&mut g, delay + 1.0);
    assert_eq!(g.world.players[&owner].warehouse.get(&C::Electronics).copied().unwrap_or(0), initial + 1);
    let bot = g.bots.get_mut(&owner).unwrap();
    bot.wait("market".into(), &picture, picture.hub);
    assert!(!bot.ready("market", picture.now + delay));
    assert!(bot.ready("market", picture.now + delay * 2.0 + THINK_SECONDS * 2.0));
}

#[test]
fn remote_colony_inventory_is_a_report_not_owner_truth() {
    let mut g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    g.bots.get_mut(&owner).unwrap().next_think = 1e9;
    g.world.players.get_mut(&owner).unwrap().command_center = g.world.hub;
    advance(&mut g, 100.0);
    let before = g.bot_observation(owner).unwrap();
    let site = before.home.system.id;
    let old = before.home.system.free_stock(C::Machinery);
    g.world.systems.iter_mut().find(|s| s.id == site).unwrap().stockpile.insert(C::Machinery, 500.0);
    g.tick();
    assert_eq!(g.bot_observation(owner).unwrap().home.system.free_stock(C::Machinery), old);
    advance(&mut g, sim::transit::delay(before.home.system.pos, before.cc, before.c) + 1.0);
    assert_eq!(g.bot_observation(owner).unwrap().home.system.free_stock(C::Machinery), 500.0);
}

#[test]
fn cargo_mass_limits_dispatch_and_low_fuel_cannot_send_a_one_unit_export() {
    let g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    let mut p = g.bot_observation(owner).unwrap();
    let mut freighter = p.own().find(|f| f.kind == H::TinyFreighter).unwrap().clone();
    // §fuel-overhaul: price the full-speed (Deuterium) regime.
    freighter.transit = Some(sim::ship::TransitMode::Full);
    let dest = freighter.pos + Vec2::new(100_000.0, 0.0);
    freighter.fuel = Some(52.5);
    let limit = p.freight_limit(&freighter, dest);
    assert!(limit > 0 && limit < 50, "full hold needs more fuel than this tank");
    freighter.cargo_manifest = vec![crate::protocol::CargoView { commodity: C::MetallicOre, units: 50 }];
    assert!(!p.enough_fuel(&freighter, dest));
    freighter.cargo_manifest[0].units = limit;
    assert!(p.enough_fuel(&freighter, dest));

    p.home.system.set_tier(K::MiningComplex, 1);
    p.home.system.stockpile.clear();
    let id = freighter.id;
    freighter.fuel = Some(p.fuel_per_mass(freighter.pos, p.hub)
        * (hull_mass(&freighter) + sim::ship::CARGO_MASS_PER_UNIT * 1.1));
    freighter.cargo_manifest[0].units = 1;
    p.ghosts = vec![freighter];
    let mut bot = g.bots[&owner].clone();
    bot.waits.insert("staff".into(), p.now + 1e6);
    assert!(!matches!(bot.decide(&p), Some(Command::HaulToMarketHub { fleet_id, .. }) if fleet_id == id));
}

#[test]
fn scouts_choose_public_unvisited_stars_and_retreat_from_visible_pirates() {
    let g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    let mut p = g.bot_observation(owner).unwrap();
    p.local_opening = false; // isolate the general survey policy, not the local tutorial
    let mut scout = p.own().next().unwrap().clone();
    scout.kind = H::Scout;
    scout.guard_target = None;
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    scout.fuel = Some(20.0);
    scout.pos = p.home.system.pos + Vec2::new(10_000.0, 0.0);
    scout.docked = None;
    let target = EntityId(987_654);
    p.chart.push((target, scout.pos + Vec2::new(1_000.0, 0.0)));
    p.surveyed = p.chart.iter().filter(|(id, _)| *id != target).map(|(id, _)| *id).collect();
    p.ghosts = vec![scout.clone()];
    let mut bot = g.bots[&owner].clone();
    bot.waits.insert("staff".into(), p.now + 1e6);
    bot.waits.insert("build".into(), p.now + 1e6);
    assert!(matches!(bot.decide(&p), Some(Command::SurveySystem { system_id, .. }) if system_id == target));
    let mut pirate = scout;
    pirate.own = false;
    pirate.id = EntityId(987_655);
    pirate.owner = PlayerId::PIRATE;
    pirate.kind = H::Raider;
    p.ghosts.push(pirate);
    bot.waits.retain(|key, _| !key.starts_with("fleet:"));
    let retreat = bot.decide(&p);
    let dest = match retreat {
        Some(Command::MoveShip { dest, .. } | Command::JumpShip { dest, .. }) => dest,
        _ => panic!("a threatened Scout must retreat, not continue investigating"),
    };
    assert!(dest.distance(p.home.system.pos) < p.ghosts[0].pos.distance(p.home.system.pos));
    assert!(!bot.surveys.contains_key(&p.ghosts[0].id));
    assert!(bot.avoided.contains_key(&target));
}

fn planning_picture() -> (Bot, Observation) {
    let g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    let mut p = g.bot_observation(owner).unwrap();
    p.local_opening = false; // these policy fixtures model the post-opening economy
    p.account = Some(MarketAccountSample { standing: 100.0, credits: 20_000.0,
        valuation: 0.0, warehouse: BTreeMap::new(), orders: Vec::new() });
    p.ticker = Some(view::MarketTickerSample {
        prices: C::ALL.iter().map(|c| (*c, sim::market::base_price(*c))).collect(),
        available_buy: C::ALL.iter().map(|c| (*c, 1_000)).collect(),
        available_sell: C::ALL.iter().map(|c| (*c, 1_000)).collect(),
    });
    (g.bots[&owner].clone(), p)
}

// Policy fixtures below do not advance the world. They supply an arrived
// picture and inspect one decision; the long-running balance harness remains
// ignored and must only be run when the user explicitly requests a simulation.
fn strategic_hull(p: &Observation, id: u64, kind: H, count: u32) -> GhostView {
    let mut f = p.own().next().unwrap().clone();
    f.id = EntityId(id); f.kind = kind; f.own = true; f.owner = p.owner;
    f.composition = Some(vec![crate::protocol::CompCount { kind, count }]);
    f.pos = p.home.system.pos; f.vel = Vec2::ZERO; f.docked = Some(p.capital.to_string());
    f.fuel = Some(10_000.0); f.fuel_capacity = Some(10_000.0); f.damage = Some(0.0);
    f.uranium = Some(10_000.0); f.uranium_capacity = Some(10_000.0);
    f.captain = None; f.guard_target = None; f.defend_system = None; f.industry = None;
    f.cargo_manifest.clear(); f.stalled = false; f.supplied = true; f.age = 0.0;
    f.mission_profile = Some(sim::doctrine::MissionProfile {
        priority: sim::doctrine::TargetPriority::Balanced,
        screening: sim::doctrine::ScreeningRole::ProtectTransports,
        withdrawal: sim::doctrine::DamageWithdrawal::Hull50,
        formation: sim::doctrine::Formation::Automatic,
    });
    f
}

fn strategic_ready() -> (Bot, Observation) {
    let (bot, mut p) = planning_picture();
    p.account.as_mut().unwrap().credits = 1_000_000.0;
    p.home.system.stockpile.insert(C::Deuterium, 100_000.0);
    p.home.system.stockpile.insert(C::Provisions, 200.0);
    p.home.system.set_tier(K::MiningComplex, 1);
    p.home.system.set_tier(K::Academy, 1);
    p.home.system.set_tier(K::Shipyard, 2);
    p.home.system.set_tier(K::UraniumMill, 1);
    p.exported = true; p.expansion_unlocked = true;
    p.research.completed.extend(["prop_bunkerage".into(), "prop_drive_tuning".into()]);
    p.sites = vec![p.home.clone()];
    (bot, p)
}

#[test]
fn midgame_research_continues_past_the_old_shortlist_and_through_high_tiers() {
    let (bot, mut p) = planning_picture();
    for tier in [1, 3, 5, 8] {
        let r = sim::research::CATALOG.iter().find(|r| r.tier == tier && !r.hidden).unwrap();
        p.available_research = vec![r.id.into()];
        assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some(r.id));
        p.research.completed.insert(r.id.into());
        assert_eq!(bot.next_research(&p, &BTreeMap::new()), None);
    }
    p.available_research.clear();
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), None, "never invent a gated unlock");
}

#[test]
fn midgame_eight_corporations_have_distinct_investment_preferences() {
    let (mut bot, p) = planning_picture();
    let mut profiles = BTreeSet::new();
    for slot in 0..8 {
        bot.slot = slot;
        let w = bot.priorities();
        profiles.insert([w.trade, w.industry, w.science, w.expansion, w.military].map(f64::to_bits));
    }
    assert_eq!(profiles.len(), 8);
    let science = sim::research::CATALOG.iter().find(|r| r.field == sim::research::Field::Computation && r.tier == 1).unwrap();
    bot.slot = 2; let scientist = bot.research_value(&p, science);
    bot.slot = 5;
    assert!(scientist > bot.research_value(&p, science));
}

#[test]
fn midgame_a_stable_second_colony_does_not_stop_further_expansion() {
    let (mut bot, mut p) = strategic_ready();
    let mut colony = p.home.clone(); colony.system.id = EntityId(9001);
    colony.system.pos = colony.system.pos + Vec2::new(3_000.0, 0.0);
    colony.system.food_state = sim::colony::FoodState::WellSupplied;
    p.sites.push(colony);
    p.ghosts = vec![strategic_hull(&p, 9100, H::TinyFreighter, 1),
        strategic_hull(&p, 9101, H::SmallFreighter, 1), strategic_hull(&p, 9102, H::Scout, 1)];
    p.prospects = vec![Prospect { id: EntityId(9002), pos: p.cc + Vec2::new(6_000.0, 0.0), score: 8.0 }];
    assert!(bot.expansion_ready(&p));
    bot.colonies(&p);
    assert_eq!(bot.expansion, Some(EntityId(9002)));
    p.sites[1].system.food_state = sim::colony::FoodState::NoProvisions;
    assert!(!bot.expansion_ready(&p), "feed the existing colony before stretching the network again");
}

#[test]
fn midgame_lost_hulls_create_replacement_demand() {
    let (bot, mut p) = strategic_ready();
    p.ghosts.retain(|f| !f.kind.is_combatant());
    assert!(bot.fleet_needs(&p).iter().any(|(h, n, _)| *h == H::Raider && *n > p.count(*h)));
    p.ghosts.clear();
    assert!(bot.fleet_needs(&p).iter().any(|(h, n, _)| *h == H::TinyFreighter && *n > 0));
}

#[test]
fn midgame_a_merged_navy_does_not_hide_missing_individual_escorts() {
    let (bot, mut p) = strategic_ready();
    p.ghosts = vec![strategic_hull(&p, 9190, H::Raider, 20), strategic_hull(&p, 9191, H::TinyFreighter, 1)];
    assert!(bot.fleet_needs(&p).iter().any(|(h, n, _)| *h == H::Raider && *n > p.count(*h)));
}

#[test]
fn midgame_construction_can_choose_a_tier_three_goal() {
    let (bot, mut p) = strategic_ready();
    p.research.completed = sim::research::visible_ids().map(str::to_string).collect();
    for b in &mut p.home.system.bodies {
        for k in K::ALL { b.structures.insert(k, 6); }
        b.structures.insert(K::MiningComplex, 2);
    }
    p.home.system.stockpile.clear();
    p.home.system.stockpile.insert(C::Deuterium, 100_000.0);
    p.ghosts = sim::ship::ALL_SHIP_KINDS.into_iter().filter(|h| h.is_buildable())
        .enumerate().map(|(i, h)| strategic_hull(&p, 9200 + i as u64, h, 100)).collect();
    let escorts = p.own().filter(|f| f.kind.is_player_freighter()).count() + 2;
    for i in 0..escorts { p.ghosts.push(strategic_hull(&p, 9250 + i as u64, H::Raider, 1)); }
    p.sites = vec![p.home.clone()];
    let project = bot.strategic_project(&p).expect("the planner still has a useful upgrade after tier two");
    assert_eq!(project.what, BuildKind::Upgrade { upgrade: K::MiningComplex });
}

#[test]
fn midgame_hulls_progress_to_bulk_freight_and_capital_warships_only_when_unlocked() {
    let (bot, mut p) = planning_picture();
    assert_eq!(bot.preferred_freighter(&p), H::TinyFreighter);
    assert_ne!(bot.preferred_warship(&p), H::Titan);
    p.research.completed = sim::research::visible_ids().map(str::to_string).collect();
    assert_eq!(bot.preferred_freighter(&p), H::BulkFreighter);
    assert_eq!(bot.preferred_warship(&p), H::Titan);
}

#[test]
fn midgame_factories_receive_workforce_for_all_their_tiers() {
    let (mut bot, mut p) = planning_picture();
    for b in &mut p.home.system.bodies { b.population = 0.0; b.structures.clear(); b.assignments.clear(); }
    let body = &mut p.home.system.bodies[0]; body.population = 0.006;
    body.migration_policy = sim::MigrationPolicy::Managed;
    body.structures = BTreeMap::from([(K::MiningComplex, 3), (K::Academy, 3)]);
    p.sites = vec![p.home.clone()];
    for _ in 0..8 {
        bot.waits.clear();
        match bot.staff(&p) {
            Some(Command::SetAssignment { structure, workers, body_id: Some(id), .. }) => {
                p.sites[0].system.bodies.iter_mut().find(|b| b.id == id).unwrap()
                    .assignments.entry(structure).or_default().workers = workers;
            }
            Some(Command::SetMigrationPolicy { body_id, policy, .. }) => {
                p.sites[0].system.bodies.iter_mut().find(|b| b.id == body_id).unwrap().migration_policy = policy;
            }
            None => break,
            _ => panic!("unexpected staffing command"),
        }
    }
    let assignments = &p.sites[0].system.bodies[0].assignments;
    assert_eq!(assignments[&K::MiningComplex].workers, 3);
    assert_eq!(assignments[&K::Academy].workers, 3);
}

#[test]
fn midgame_industry_reserves_feedstock_not_just_the_next_build_recipe() {
    let (bot, mut p) = planning_picture();
    let body = &mut p.home.system.bodies[0];
    body.structures.insert(K::ElectronicsFabricator, 3);
    body.assignments.insert(K::ElectronicsFabricator, sim::production::Assignment::crew(3));
    let wanted = bot.desired_stock(&p, None);
    for &(c, _) in sim::production::converter_for(K::ElectronicsFabricator).unwrap().inputs {
        assert!(wanted.get(&c).is_some_and(|n| *n > 0.0));
    }
}

#[test]
fn midgame_owned_freighters_supply_other_colonies_with_bounded_mixed_manifests() {
    let (mut bot, mut p) = strategic_ready();
    p.home.system.stockpile.insert(C::Provisions, 1_000.0);
    let mut colony = p.home.clone(); colony.system.id = EntityId(9300);
    colony.system.pos = colony.system.pos + Vec2::new(2_000.0, 0.0); colony.system.stockpile.clear();
    p.sites = vec![p.home.clone(), colony.clone()];
    p.ghosts = vec![strategic_hull(&p, 9301, H::TinyFreighter, 1)];
    bot.refresh_network(&p);
    let Some(Command::SetFreightRoute { route: Some(route), .. }) = bot.network_routes(&p) else {
        panic!("a source surplus should supply the reported colony deficit");
    };
    assert_eq!(route.stops[1].port, sim::industry::FreightPort::System { id: colony.system.id });
    assert!(!route.stops[1].sell);
    assert!(route.stops[0].load.values().sum::<u32>() <= 50);
    assert_eq!(route.stops[0].load, route.stops[1].unload);
    assert!(bot.network_routes(&p).is_none(), "do not repeat an order against the old picture");
}

#[test]
fn midgame_idle_spare_freighters_are_rebased_to_new_ports() {
    let (mut bot, mut p) = strategic_ready();
    p.ghosts = vec![strategic_hull(&p, 9310, H::TinyFreighter, 1), strategic_hull(&p, 9311, H::SmallFreighter, 1)];
    bot.refresh_network(&p);
    let mut colony = p.home.clone(); colony.system.id = EntityId(9312); colony.system.pos = colony.system.pos + Vec2::new(2_000.0, 0.0);
    p.sites.push(colony.clone());
    bot.refresh_network(&p);
    assert!(bot.freight_homes.values().any(|id| *id == colony.system.id));
    assert!(bot.freight_homes.values().any(|id| *id == p.capital));
}

#[test]
fn midgame_campaigns_require_strength_and_keep_a_home_guard() {
    let (mut bot, mut p) = strategic_ready();
    let home = strategic_hull(&p, 9400, H::Raider, 1);
    let mut navy = strategic_hull(&p, 9401, H::Raider, 8); navy.pos = navy.pos + Vec2::new(100.0, 0.0);
    let mut pirate = strategic_hull(&p, 9402, H::Raider, 1);
    pirate.own = false; pirate.owner = PlayerId::PIRATE; pirate.pirate = true;
    pirate.pos = pirate.pos + Vec2::new(5_000.0, 0.0); pirate.docked = None;
    p.ghosts = vec![home.clone(), navy.clone(), pirate.clone()];
    assert!(matches!(bot.military(&p), Some(Command::AttackFleet { fleet_id, target_id, .. })
        if fleet_id == navy.id && target_id == pirate.id));
    assert!(!bot.campaign_fleet(home.id));
    assert!(bot.military(&p).is_none(), "campaign orders aren't issued every think");
    let mut saved: Bot = serde_json::from_value(serde_json::to_value(&bot).unwrap()).unwrap();
    assert!(saved.military(&p).is_none(), "restoring preserves the pending campaign");
    saved.campaign = None; saved.waits.clear(); p.ghosts = vec![home, pirate];
    assert!(saved.military(&p).is_none(), "the only defender stays home");
}

#[test]
fn midgame_unknown_or_overwhelming_contacts_are_not_attacked() {
    let (mut bot, mut p) = strategic_ready();
    let home = strategic_hull(&p, 9410, H::Raider, 1);
    let mut navy = strategic_hull(&p, 9411, H::Raider, 2); navy.pos = navy.pos + Vec2::new(100.0, 0.0);
    let mut pirate = strategic_hull(&p, 9412, H::Titan, 10);
    pirate.own = false; pirate.owner = PlayerId::PIRATE; pirate.pirate = true;
    pirate.pos = pirate.pos + Vec2::new(5_000.0, 0.0);
    p.ghosts = vec![home, navy, pirate];
    assert!(bot.military(&p).is_none());
    p.ghosts[2].composition = None;
    assert!(bot.military(&p).is_none(), "fog is not a zero-strength fleet");
}

#[test]
fn midgame_territorial_orders_require_reported_war_and_respect_treaties() {
    let (mut bot, mut p) = strategic_ready(); bot.slot = 5;
    let home = strategic_hull(&p, 9420, H::Raider, 1);
    let mut navy = strategic_hull(&p, 9421, H::Raider, 20); navy.pos = navy.pos + Vec2::new(100.0, 0.0);
    p.ghosts = vec![home, navy];
    let owner = PlayerId(123);
    let pos = p.cc + Vec2::new(5_000.0, 0.0);
    p.military_sites.push(warfare::MilitarySite { id: EntityId(9422), pos, owner: Some(owner),
        blockade: None, intel: sim::IntelSnapshot { station_level: 0, defense_tier: 0, shipyard_tier: 0,
            enclave_tier: 0, garrison_tier: 1, observed_at: p.now, pos } });
    p.relations.insert(owner, sim::diplomacy::RelationState::NonAggression);
    assert!(bot.military(&p).is_none());
    p.relations.insert(owner, sim::diplomacy::RelationState::War);
    assert!(matches!(bot.military(&p), Some(Command::BlockadeSystem { system_id, .. }) if system_id == EntityId(9422)));
}

#[test]
fn midgame_heavily_damaged_fleets_withdraw_instead_of_taking_new_targets() {
    let (mut bot, mut p) = strategic_ready();
    let mut f = strategic_hull(&p, 9430, H::Raider, 2); f.damage = Some(0.6);
    p.fighting.insert(f.id); p.ghosts = vec![f.clone()];
    assert!(matches!(bot.recover_military(&p), Some(Command::Withdraw { fleet_id, .. }) if fleet_id == f.id));
    assert!(bot.recover_military(&p).is_none(), "withdrawal itself has command delay");
}

#[test]
fn midgame_an_unpaid_ambition_yields_to_replacing_the_last_carrier() {
    let (mut bot, mut p) = strategic_ready();
    let goal = Project { what: BuildKind::Upgrade { upgrade: K::Academy },
        body: p.home.system.site_for(K::Academy).unwrap() };
    bot.development = Some(DevelopmentGoal { project: goal, target: 2, since: p.now });
    p.ghosts.retain(|f| !f.kind.is_player_freighter());
    bot.wait("build".into(), &p, p.cc);
    bot.refresh_development(&p);
    assert_eq!(bot.project(&p), Some(goal), "don't replace an intention whose order is still travelling");
    bot.waits.clear();
    bot.refresh_development(&p);
    assert_eq!(bot.project(&p).unwrap().what, BuildKind::Ship { ship: H::TinyFreighter });
}

#[test]
fn midgame_colonies_specialize_by_reported_resources_and_can_export_them() {
    use strategy::Role;
    let (bot, mut p) = strategic_ready();
    p.home.system.id = EntityId(9500);
    for (resource, role) in [(C::MetallicOre, Role::Mining), (C::Volatiles, Role::Fuel), (C::Biomass, Role::Agriculture)] {
        for b in &mut p.home.system.bodies { b.deposits.clear(); }
        p.home.system.bodies[0].deposits.push(sim::galaxy::Deposit {
            resource, richness: 10.0, reserves: None, accessibility: 1.0,
        });
        assert_eq!(bot.role(&p), role);
        p.home.system.stockpile = BTreeMap::from([(resource, 30.0)]);
        assert_eq!(p.exports(&BTreeMap::from([(resource, 5.0)]), 50).get(&resource), Some(&25));
    }
}

#[test]
fn midgame_a_remote_depot_does_not_budget_for_the_entire_home_navy() {
    let (_, mut p) = strategic_ready();
    let mut colony = p.home.clone(); colony.system.id = EntityId(9510);
    colony.system.pos = colony.system.pos + Vec2::new(100_000.0, 0.0);
    for b in &mut colony.system.bodies { b.assignments.clear(); }
    p.sites.push(colony.clone());
    p.ghosts = vec![strategic_hull(&p, 9511, H::Titan, 10)];
    assert!(p.uses_local_port(&p.ghosts[0]));
    let remote = p.at_site(&colony);
    assert!(!remote.uses_local_port(&p.ghosts[0]));
    // §fuel-overhaul: a Titan burns Deuterium; the remote depot keeps none for it.
    assert_eq!(remote.fuel_target_of(C::Deuterium), 0.0);
    assert!(p.fuel_target_of(C::Deuterium) > remote.fuel_target_of(C::Deuterium));
}

#[test]
fn midgame_supply_promises_become_reported_pickups_then_real_cargo_without_duplication() {
    use sim::industry::{FleetIndustry, FreightRun, RoutePhase};
    let (mut bot, mut p) = strategic_ready();
    p.home.system.stockpile.insert(C::Provisions, 1_000.0);
    let mut colony = p.home.clone(); colony.system.id = EntityId(9520);
    colony.system.pos = colony.system.pos + Vec2::new(2_000.0, 0.0); colony.system.stockpile.clear();
    p.sites = vec![p.home.clone(), colony.clone()];
    p.ghosts = vec![strategic_hull(&p, 9521, H::TinyFreighter, 1)];
    let Some(Command::SetFreightRoute { route: Some(route), .. }) = bot.network_routes(&p) else { panic!("supply route"); };
    let (&good, &units) = route.stops[0].load.iter().next().unwrap();
    let remote = p.at_site(&colony);
    assert_eq!(bot.expected_supply(&remote, good), units);
    assert_eq!(bot.reserved_supply(&p, good), units);
    assert_eq!(remote.stock(good), 0.0, "a promise is never arrived stock");
    assert!(bot.market(&remote, &BTreeMap::from([(good, units as f64)])).is_none());
    p.ghosts[0].industry = Some(FleetIndustry::Route { run: FreightRun {
        shopping_spent: 0.0,
        remaining_load: route.stops[0].load.clone(), remaining_unload: BTreeMap::new(),
        route, stop: 0, phase: RoutePhase::Loading, visits: 0, hold: None,
    }});
    bot.refresh_network(&p);
    assert!(bot.supply_dispatches.is_empty());
    assert_eq!(bot.expected_supply(&p.at_site(&colony), good), units, "replace the pending promise, don't add it twice");
    let f = &mut p.ghosts[0];
    f.cargo_manifest = vec![crate::protocol::CargoView { commodity: good, units }];
    if let Some(FleetIndustry::Route { run }) = &mut f.industry { run.phase = RoutePhase::Fuel; run.remaining_load.clear(); }
    let remote = p.at_site(&colony);
    assert_eq!(bot.expected_supply(&remote, good), 0);
    assert_eq!(remote.carried_home(good), units, "a loaded carrier waiting to refuel still owns real cargo");
    assert!(bot.market(&remote, &BTreeMap::from([(good, units as f64)])).is_none());
}

#[test]
fn midgame_scouts_refresh_stale_intelligence_but_abort_a_threatened_approach() {
    let (mut bot, mut p) = strategic_ready(); p.now = 4_000.0;
    let scout = strategic_hull(&p, 9530, H::Scout, 1);
    p.ghosts = vec![scout.clone()];
    let pos = p.cc + Vec2::new(50_000.0, 0.0);
    p.military_sites.push(warfare::MilitarySite { id: EntityId(9531), pos, owner: None, blockade: None,
        intel: sim::IntelSnapshot { station_level: 0, defense_tier: 1, shipyard_tier: 0, enclave_tier: 1, garrison_tier: 0, observed_at: 0.0, pos } });
    assert!(matches!(bot.scouts(&p), Some(Command::MoveShip { .. } | Command::JumpShip { .. })));
    let mission = bot.reconnaissance[&scout.id].clone();
    assert!(mission.point.distance(pos) < sim::ship::SCOUT_INTEL_RANGE);
    assert!(mission.point.distance(pos) > sim::transit::HYPERLIMIT);
    let mut pirate = strategic_hull(&p, 9532, H::Raider, 1);
    pirate.own = false; pirate.owner = PlayerId::PIRATE; pirate.pirate = true; pirate.pos = mission.point;
    p.ghosts.push(pirate); bot.waits.clear();
    bot.scouts(&p);
    assert!(!bot.reconnaissance.contains_key(&scout.id));
    assert!(bot.avoided.contains_key(&mission.system));
}

#[test]
fn midgame_a_weak_picket_is_not_attacked_beside_an_overwhelming_formation() {
    let (mut bot, mut p) = strategic_ready();
    let home = strategic_hull(&p, 9540, H::Raider, 1);
    let mut navy = strategic_hull(&p, 9541, H::Raider, 8); navy.pos = navy.pos + Vec2::new(100.0, 0.0);
    let mut pirate = strategic_hull(&p, 9542, H::Raider, 1);
    pirate.own = false; pirate.owner = PlayerId::PIRATE; pirate.pirate = true;
    pirate.pos = p.cc + Vec2::new(5_000.0, 0.0);
    let mut base = pirate.clone(); base.id = EntityId(9543); base.kind = H::Titan;
    base.composition = Some(vec![crate::protocol::CompCount { kind: H::Titan, count: 20 }]);
    p.ghosts = vec![home, navy, pirate, base];
    assert!(bot.military(&p).is_none());
}

#[test]
fn midgame_troops_wait_for_reported_orbital_control_and_campaigns_end_at_peace() {
    let (mut bot, mut p) = strategic_ready();
    let leader = strategic_hull(&p, 9550, H::Raider, 20);
    let troop = strategic_hull(&p, 9551, H::Transport, 20);
    p.ghosts = vec![leader.clone(), troop.clone()];
    let id = EntityId(9552); let owner = PlayerId(123);
    let pos = p.cc + Vec2::new(5_000.0, 0.0);
    p.military_sites.push(warfare::MilitarySite { id, pos, owner: Some(owner), blockade: None,
        intel: sim::IntelSnapshot { station_level: 0, defense_tier: 0, shipyard_tier: 0, enclave_tier: 0, garrison_tier: 1, observed_at: p.now, pos } });
    p.relations.insert(owner, sim::diplomacy::RelationState::War);
    bot.campaign = Some(warfare::Campaign { target: warfare::Target::System(id), leader: leader.id, since: p.now, deadline: p.now + 50_000.0 });
    assert!(bot.military(&p).is_none(), "do not send marines because a blockade order was merely issued");
    p.military_sites[0].blockade = Some(sim::galaxy::Blockade { by: p.owner, since: p.now, held_since: Some(p.now), siege_since: Some(p.now), anchored: false, catch: 0.0, catch_stabilized: 0.0 });
    assert!(matches!(bot.military(&p), Some(Command::MoveShip { ship_id, dest, .. }) if ship_id == troop.id && dest == pos));
    bot.waits.clear(); p.relations.insert(owner, sim::diplomacy::RelationState::Ceasefire);
    assert!(!matches!(bot.military(&p), Some(Command::BlockadeSystem { .. })));
    assert!(bot.campaign.is_none());
}

#[test]
fn midgame_specialists_are_budgeted_trained_and_posted_through_normal_commands() {
    use sim::specialist::SpecialistKind as S;
    let (mut bot, mut p) = strategic_ready();
    for b in &mut p.home.system.bodies { b.assignments.clear(); b.structures.clear(); b.population = 0.0; }
    let body = &mut p.home.system.bodies[0]; body.population = 0.003;
    body.structures = BTreeMap::from([(K::MiningComplex, 2), (K::Academy, 1)]);
    body.migration_policy = sim::MigrationPolicy::Managed;
    body.assignments.insert(K::MiningComplex, sim::production::Assignment::crew(2));
    body.assignments.insert(K::Academy, sim::production::Assignment::crew(1));
    let body_id = body.id;
    p.sites = vec![p.home.clone()];
    assert_eq!(bot.specialist_need(&p), Some(S::Geologist));
    assert!(bot.desired_stock(&p, None)[&C::Electronics] >= 10.0);
    p.sites[0].system.specialists.insert(S::Geologist, 1);
    assert!(matches!(bot.staff(&p), Some(Command::SetAssignment { structure: K::MiningComplex, workers: 1,
        body_id: Some(id), specialists, .. }) if id == body_id && specialists.get(&S::Geologist) == Some(&1)));
}

#[test]
fn spending_reservations_survive_saves_and_wait_for_account_light() {
    let (mut bot, mut p) = planning_picture();
    bot.spend(&p, 1_000.0);
    let arrival = bot.commitments[0].receiver_at;
    let mut restored: Bot = serde_json::from_value(serde_json::to_value(&bot).unwrap()).unwrap();
    p.now = arrival + 1_000.0;
    p.market_at = arrival - 1.0;
    restored.refresh(&p);
    assert_eq!(restored.cash(&p), 19_000.0, "wall time alone cannot free money still absent from the served account");
    p.account.as_mut().unwrap().credits -= 1_000.0;
    p.market_at = arrival;
    restored.refresh(&p);
    assert!(restored.commitments.is_empty());
    assert_eq!(restored.cash(&p), 19_000.0);
    let mut legacy = serde_json::to_value(&bot).unwrap();
    for field in ["commitments", "returns", "avoided", "route_visits", "trade_cycles", "expansion", "status", "survey_started", "development",
        "campaign", "freight_homes", "economy_turn", "supply_dispatches", "reconnaissance"] {
        legacy.as_object_mut().unwrap().remove(field);
    }
    let old: Bot = serde_json::from_value(legacy).unwrap();
    assert!(old.commitments.is_empty());
    assert!(old.valid(p.owner, p.now));
}

/// The opening stall (Oct 2026): every market action holds the market for a
/// light round trip, and the wanted list ran in commodity order, so an
/// Agroplex's Biomass buffer took every slot while the Shipyard waited on
/// Machinery. Project shortfalls go first; food lines import nothing.
#[test]
fn project_shortfalls_buy_before_buffer_top_ups_and_food_lines_import_nothing() {
    let (mut bot, mut p) = planning_picture();
    let body = p.home.system.bodies[0].id;
    p.home.system.stockpile.insert(C::Machinery, 0.0);
    p.home.system.stockpile.insert(C::Deuterium, 150.0);
    p.home.system.stockpile.insert(C::Provisions, 150.0);
    bot.development = Some(DevelopmentGoal { project: Project { what: BuildKind::Upgrade { upgrade: K::Shipyard }, body },
        target: p.home.system.tier(K::Shipyard) + 1, since: p.now });
    let wanted = BTreeMap::from([(C::Biomass, 245.0), (C::Machinery, 20.0)]);
    let cmd = bot.market(&p, &wanted);
    assert!(matches!(cmd, Some(Command::MarketBuy { commodity: C::Machinery, .. })), "{cmd:?}");

    // A staffed Agroplex asks the buffers for nothing: food has its own target.
    for b in &mut p.home.system.bodies {
        b.structures.insert(K::Agroplex, 1);
        b.assignments.insert(K::Agroplex, sim::production::Assignment { workers: 1, ..Default::default() });
    }
    let mut buffers = BTreeMap::new();
    bot.industry_buffers(&p, None, &mut buffers);
    assert!(buffers.get(&C::Biomass).is_none_or(|n| *n == 0.0), "{buffers:?}");
}

/// The charter's survey stop goes to a one-way Survey Probe, which a
/// picket nearby doesn't hold back; one probe per stop until its report.
#[test]
fn the_charter_survey_stop_gets_a_probe_even_past_a_picket() {
    let (mut bot, mut p) = strategic_ready();
    p.founding_stage = sim::FoundingStage::SurveyCandidates;
    let (stop, pos) = *p.chart.iter().find(|(id, _)| *id != p.home.system.id && !p.surveyed.contains(id)).unwrap();
    p.survey_candidates = vec![stop];
    let mut picket = strategic_hull(&p, 9950, H::Corvette, 1);
    picket.own = false; picket.owner = PlayerId(77); picket.pos = pos + Vec2::new(8_000.0, 0.0);
    p.ghosts.push(picket);
    assert!(p.threatened(pos), "a Scout would hold back");
    assert_eq!(bot.charter_probe_due(&p), Some(stop));
    let probe = strategic_hull(&p, 9951, H::SurveyProbe, 1);
    p.ghosts.push(probe.clone());
    let cmd = bot.scouts(&p);
    assert!(matches!(cmd, Some(Command::SurveySystem { fleet_id, system_id, .. }) if fleet_id == probe.id && system_id == stop), "{cmd:?}");
    assert_eq!(bot.charter_probe_due(&p), None, "no second probe while the first one's report is due");
}

/// The first shipment counts only if an escorted Freighter carries Ferrite
/// Ore: the ore goes first, ahead of dearer Provisions. Until it leaves,
/// Authority doesn't take the ore, unless the colony's storage is full.
#[test]
fn the_first_shipment_carries_ferrite_ore_and_authority_leaves_it_alone() {
    let (mut bot, mut p) = planning_picture();
    p.founding_stage = sim::FoundingStage::PrepareExport;
    p.exported = false;
    p.home.system.stockpile.insert(C::MetallicOre, 120.0);
    p.home.system.stockpile.insert(C::Provisions, 400.0);
    p.home.system.stockpile.insert(C::Deuterium, 100.0);
    let id = p.own().find(|g| g.kind.is_player_freighter()).unwrap().id;
    for g in &mut p.ghosts {
        g.fuel = g.fuel_capacity;
        if g.kind == H::Raider { g.guard_target = Some(id); }
    }
    let cmd = bot.routes(&p, &BTreeMap::new());
    let Some(Command::SetFreightRoute { route: Some(route), .. }) = cmd else { panic!("{cmd:?}") };
    let home = &route.stops[0];
    assert!(home.load.get(&C::MetallicOre).is_some_and(|n| *n > 0), "{:?}", home.load);

    bot.waits.clear();
    assert!(bot.export_surplus(&p, &BTreeMap::new(), false).is_none_or(|c|
        !matches!(c, Command::BookFreightIn { commodity: C::MetallicOre, .. })), "ore waits for the Freighter");
    bot.waits.clear();
    let cmd = bot.export_surplus(&p, &BTreeMap::new(), true);
    assert!(matches!(cmd, Some(Command::BookFreightIn { commodity: C::Provisions, .. })),
        "the bigger pile clears first: {cmd:?}");
    p.home.system.stockpile.insert(C::Provisions, 0.0);
    bot.waits.clear();
    let cmd = bot.export_surplus(&p, &BTreeMap::new(), true);
    assert!(matches!(cmd, Some(Command::BookFreightIn { commodity: C::MetallicOre, units, .. }) if units > 0 && units <= 70),
        "full storage still ships the ore, keeping one load: {cmd:?}");
}

#[test]
fn critical_fuel_precedes_project_materials_and_rescue_preserves_runway() {
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::Deuterium, 0.0);
    let wanted = BTreeMap::from([(C::Deuterium, 80.0), (C::Machinery, 100.0)]);
    assert!(matches!(bot.market(&p, &wanted), Some(Command::MarketBuy { commodity: C::Deuterium, .. })));
    let ship = p.ghosts.iter_mut().find(|g| g.own).unwrap();
    ship.stalled = true; ship.docked = None; ship.fuel = Some(0.0);
    bot.waits.clear(); bot.commitments.clear();
    p.account.as_mut().unwrap().credits = 13.0;
    assert!(bot.rescue(&p).is_none());
    assert!(bot.recovering(&p));
    assert_eq!(bot.spendable(&p, false), 0.0);
    p.account.as_mut().unwrap().credits = 20_000.0;
    assert!(matches!(bot.rescue(&p), Some(Command::RequestFuelRescue { .. })));
    assert!(!bot.commitments.is_empty());
}

#[test]
fn a_staffed_deuterium_consumer_reserves_its_delivery_lead_time() {
    // §fuel-overhaul: refining burns no fuel; the Antimatter Plant eats Deuterium.
    let (_, mut p) = planning_picture();
    let before = p.fuel_target_of(C::Deuterium);
    p.home.system.bodies[0].structures.insert(K::AntimatterPlant, 1);
    p.home.system.bodies[0].assignments.insert(K::AntimatterPlant, sim::production::Assignment {
        workers: 1, ..Default::default()
    });
    let industry = p.fuel_target_of(C::Deuterium);
    assert!(industry > before, "the fuel reserve covers industry as well as tanks: {before} -> {industry}");
    assert!(industry <= p.home.system.storage_cap() * 0.3);
    p.home.system.bodies[0].assignments.get_mut(&K::AntimatterPlant).unwrap().workers = 0;
    assert_eq!(p.fuel_target_of(C::Deuterium), before, "an idle line is not a fuel consumer");
}

#[test]
fn a_tiny_ore_circuit_profit_depends_on_cruise_and_return_mass_counts() {
    let (_, p) = planning_picture();
    let f = p.own().find(|g| g.kind.is_player_freighter()).unwrap();
    let guard = p.own().find(|g| g.kind == H::Raider).unwrap();
    let exports = BTreeMap::from([(C::MetallicOre, 50)]);
    let mut full = f.clone();
    full.transit = Some(sim::ship::TransitMode::Full);
    let f = &full;
    let margin = p.route_margin(f, &exports, &BTreeMap::new(), Some(guard));
    assert!(margin < 0.0, "Full-speed basic ore is not worth a fuel-loss circuit: {margin}");
    assert!(p.leg(f, p.hub, p.home.system.pos, 50) > p.leg(f, p.hub, p.home.system.pos, 0));
    let mut slow = f.clone();
    slow.transit = Some(sim::ship::TransitMode::Slow);
    let slow_margin = p.route_margin(&slow, &exports, &BTreeMap::new(), Some(guard));
    assert!(slow_margin > 0.0, "Slow on Uranium makes the starter guarded ore route viable: {slow_margin}");
    eprintln!("50-unit Tiny ore circuit: Full margin {margin:.0} Cr; Slow {slow_margin:.0} Cr at base prices (conservative fuel + escort included)");
}

#[test]
fn thirteen_credits_can_recover_through_a_small_paid_export() {
    let (mut bot, mut p) = planning_picture();
    p.account.as_mut().unwrap().credits = 13.0;
    p.home.system.stockpile.insert(C::MetallicOre, 100.0);
    let cmd = bot.market(&p, &BTreeMap::new()).expect("size the shipment to real cash, not the normal minimum lot");
    let Command::BookFreightIn { commodity, units, sell_on_arrival, .. } = cmd else { panic!("expected revenue recovery") };
    let fee = sim::tca::freight_fee(units, p.price(commodity), p.home.system.pos.distance(p.hub));
    assert!(sell_on_arrival && units > 0 && units < 50 && fee <= 13.0);
    assert!(units as f64 * p.price(commodity) > fee);
    assert!(bot.commitments[0].amount <= 13.0);
}

#[test]
fn recovery_can_liquidate_optional_warehouse_goods_but_not_operating_fuel() {
    let (mut bot, mut p) = planning_picture();
    p.account.as_mut().unwrap().credits = 13.0;
    p.account.as_mut().unwrap().warehouse = BTreeMap::from([(C::Deuterium, 40), (C::Machinery, 10)]);
    assert!(matches!(bot.market(&p, &BTreeMap::from([(C::Deuterium, 40.0), (C::Machinery, 20.0)])),
        Some(Command::MarketSell { commodity: C::Machinery, units: 10, .. })));
}

#[test]
fn profitable_owned_routes_have_mixed_manifests_and_a_real_escort() {
    use sim::industry::FreightPort;
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::TitaniumOre, 50.0);
    p.home.system.stockpile.insert(C::MetallicOre, 50.0);
    p.home.system.stockpile.insert(C::Deuterium, 0.0);
    p.account.as_mut().unwrap().warehouse = BTreeMap::from([(C::Deuterium, 10), (C::Machinery, 5)]);
    p.home.system.stockpile.insert(C::Machinery, 0.0);
    let id = p.own().find(|g| g.kind.is_player_freighter()).unwrap().id;
    for g in &mut p.ghosts {
        g.fuel = g.fuel_capacity;
        if g.kind == H::Raider { g.guard_target = Some(id); }
    }
    let wanted = BTreeMap::from([(C::Deuterium, 10.0), (C::Machinery, 10.0)]);
    let cmd = bot.routes(&p, &wanted).expect("viable owned circuit");
    let Command::SetFreightRoute { route: Some(route), .. } = cmd else { panic!("expected route") };
    assert_eq!(route.transit, sim::ship::TransitMode::Slow, "freight flies Slow until Fusion Freight");
    assert!(!route.repeat && route.escort.is_some() && route.fuel_reserve > 0.0);
    assert_eq!(route.stops.len(), 2, "the starter must not require advanced multi-stop research");
    assert!(route.stops.last().unwrap().load.is_empty(), "finish selling before planning the return load");
    let hub = route.stops.iter().find(|s| s.port == FreightPort::Hub).unwrap();
    assert!(hub.sell && hub.unload.contains_key(&C::TitaniumOre));
    let f = p.ghosts.iter_mut().find(|f| f.id == id).unwrap();
    f.docked = Some("hub".into()); f.pos = p.hub;
    bot.waits.clear();
    let Some(Command::SetFreightRoute { route: Some(back), .. }) = bot.routes(&p, &wanted) else { panic!("return leg") };
    assert_eq!(back.stops[0].load.get(&C::Deuterium), Some(&10));
    assert!(back.stops[0].load.contains_key(&C::Machinery));
    assert!(back.stops[1].load.is_empty());
    for leg in [&route, &back] {
        for stop in &leg.stops {
            assert!(stop.load.values().sum::<u32>() <= 50);
            assert!(stop.unload.values().sum::<u32>() <= 50, "unload limits are shared across goods too");
        }
    }
    // Send both actual commands through the simulation validator; a plausible
    // policy object is no proof that a beginner is allowed to execute it.
    let mut g = game(2, 1);
    g.bots.get_mut(&p.owner).unwrap().next_think = 1e9;
    for leg in [route, back] {
        g.pending.push(Command::SetFreightRoute { player_id: p.owner, fleet_id: id, route: Some(leg.clone()) });
        advance(&mut g, 10.0);
        assert!(matches!(&g.world.fleets[&id].industry, Some(sim::industry::FleetIndustry::Route { run }) if run.route == leg));
    }
}

#[test]
fn a_dry_route_cannot_block_its_own_emergency_fuel_import() {
    use sim::industry::{FleetIndustry, FreightPort, FreightRoute, FreightRun, RoutePhase, RouteStop};
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::Deuterium, 0.0);
    p.account.as_mut().unwrap().warehouse.insert(C::Deuterium, 50);
    let f = p.ghosts.iter_mut().find(|g| g.kind.is_player_freighter()).unwrap();
    f.fuel = Some(0.0);
    f.industry = Some(FleetIndustry::Route { run: FreightRun {
        shopping_spent: 0.0,
        route: FreightRoute { shopping_budget: None, name: "Fuel circuit".into(), transit: sim::ship::TransitMode::Full, stops: vec![RouteStop {
            port: FreightPort::Hub, load: BTreeMap::from([(C::Deuterium, 50)]), unload: BTreeMap::new(), sell: false,
        }], fuel_reserve: 1.0, escort: None, repeat: true },
        stop: 0, phase: RoutePhase::Fuel, remaining_load: BTreeMap::new(), remaining_unload: BTreeMap::new(),
        visits: 1, hold: Some("Waiting for fuel and arrival reserve".into()),
    } });
    assert!(matches!(bot.market(&p, &BTreeMap::from([(C::Deuterium, 50.0)])),
        Some(Command::BookFreightOut { commodity: C::Deuterium, units: 50, .. })));
}

#[test]
fn an_old_route_completion_is_not_counted_again_while_its_replacement_travels() {
    use sim::industry::{FleetIndustry, FreightRoute, FreightRun, RoutePhase};
    let (mut bot, mut p) = planning_picture();
    let f = p.ghosts.iter_mut().find(|g| g.kind.is_player_freighter()).unwrap();
    let id = f.id;
    f.docked = Some(p.home.system.id.to_string());
    f.industry = Some(FleetIndustry::Route { run: FreightRun {
        shopping_spent: 0.0,
        route: FreightRoute { shopping_budget: None, name: "Home / Market".into(), transit: sim::ship::TransitMode::Slow,
            stops: vec![], fuel_reserve: 0.0, escort: None, repeat: false },
        stop: 1, phase: RoutePhase::Complete, remaining_load: BTreeMap::new(),
        remaining_unload: BTreeMap::new(), visits: 1, hold: None,
    } });
    bot.refresh(&p);
    assert_eq!(bot.trade_cycles, 1);
    bot.route_visits.insert(id, 0); // replacing the finite route
    bot.wait(format!("fleet:{id}"), &p, p.home.system.pos);
    p.now += 1.0;
    bot.refresh(&p);
    assert_eq!(bot.trade_cycles, 1, "the old completion is not another physical round trip");
    p.now += 1_000.0;
    let f = p.ghosts.iter_mut().find(|g| g.id == id).unwrap();
    let Some(FleetIndustry::Route { run }) = &mut f.industry else { unreachable!() };
    run.phase = RoutePhase::Travelling;
    bot.refresh(&p);
    assert_eq!(bot.trade_cycles, 1);
    let f = p.ghosts.iter_mut().find(|g| g.id == id).unwrap();
    let Some(FleetIndustry::Route { run }) = &mut f.industry else { unreachable!() };
    run.phase = RoutePhase::Complete;
    bot.refresh(&p);
    assert_eq!(bot.trade_cycles, 2);
}

#[test]
fn scouts_refuel_instead_of_retrying_unreachable_surveys() {
    let (mut bot, mut p) = planning_picture();
    let mut f = p.own().next().unwrap().clone();
    f.kind = H::Scout; f.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    f.fuel_capacity = Some(2.8); f.fuel = Some(0.25);
    f.pos = p.home.system.pos + Vec2::new(1_500.0, 0.0); f.docked = None;
    p.chart = vec![(EntityId(98765), p.home.system.pos + Vec2::new(200_000.0, 0.0))];
    p.ghosts = vec![f.clone()];
    assert!(matches!(bot.scouts(&p), Some(Command::MoveShip { dest, .. }) if dest == p.home.system.pos));
    assert!(bot.returns.contains_key(&f.id));
    bot.waits.clear();
    assert!(!matches!(bot.scouts(&p), Some(Command::SurveySystem { .. })), "return intention must not be replaced by a fresh excursion on an old view");
}

#[test]
fn scouts_stage_jumps_outside_public_wells_and_wait_for_arrival_light() {
    let (mut bot, mut p) = planning_picture();
    let mut f = p.own().next().unwrap().clone();
    f.kind = H::Scout; f.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    f.fuel_capacity = Some(2.8); f.fuel = Some(2.8); f.vel = Vec2::ZERO;
    let far = p.home.system.pos + Vec2::new(180_000.0, 0.0);
    let target = EntityId(9999);
    p.chart = vec![(p.home.system.id, p.home.system.pos), (target, far)];
    p.ghosts = vec![f.clone()];
    assert!(p.leg(&f, f.pos, far, 0) * 2.0 > 2.8);
    let Some(Command::MoveShip { dest: staging, .. }) = bot.scouts(&p) else { panic!("leave the well before jumping") };
    assert!(staging.distance(p.home.system.pos) > sim::transit::HYPERLIMIT);
    assert!(bot.scouts(&p).is_none(), "do not act on an unchanged report during the command round trip");
    p.now += 60.0;
    p.ghosts[0].pos = staging; p.ghosts[0].docked = None;
    let Some(Command::JumpShip { dest, .. }) = bot.scouts(&p) else { panic!("use normal jump capability to reach the frontier") };
    assert!(dest.distance(staging) <= sim::transit::JUMP_RANGE);
    assert!(p.chart.iter().all(|(_, point)| point.distance(dest) > sim::transit::HYPERLIMIT));
    assert_eq!(bot.surveys.get(&f.id), Some(&target));
}

#[test]
fn expansion_requires_stability_and_uses_only_reported_prospects() {
    let (mut bot, mut p) = planning_picture();
    assert!(!bot.expansion_ready(&p), "fresh corporations do not rush colonies");
    p.exported = true;
    p.research.completed.extend(["prop_bunkerage".into(), "prop_drive_tuning".into()]);
    p.home.system.set_tier(K::Academy, 1);
    p.home.system.set_tier(K::MiningComplex, 1);
    p.home.system.stockpile.insert(C::Deuterium, 500.0);
    p.home.system.stockpile.insert(C::Provisions, 50.0);
    let mut scout = p.own().next().unwrap().clone();
    scout.id = EntityId(9999); scout.kind = H::Scout;
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    p.ghosts.push(scout);
    let target = EntityId(9998);
    p.prospects = vec![Prospect { id: target, pos: p.cc + Vec2::new(10_000.0, 0.0), score: 5.0 }];
    p.home.system.set_tier(K::Shipyard, 1);
    assert!(!bot.expansion_ready(&p), "research and arbitrary surveys cannot bypass the founder's gate");
    assert!(p.ship(H::Colony, 1).is_none());
    p.expansion_unlocked = true;
    assert!(bot.expansion_ready(&p));
    assert!(p.ship(H::Colony, 1).is_some());
    bot.colonies(&p);
    assert_eq!(bot.expansion, Some(target));
    p.prospects.clear();
    bot.colonies(&p);
    assert!(bot.expansion.is_none(), "a returned claim removes the stale opportunity");
}

#[test]
fn scouts_detour_when_the_direct_jump_landings_are_threatened() {
    let (mut bot, mut p) = planning_picture();
    let mut scout = p.own().next().unwrap().clone();
    scout.kind = H::Scout;
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    scout.pos = Vec2::ZERO; scout.vel = Vec2::ZERO; scout.docked = None;
    scout.fuel = Some(2.8); scout.fuel_capacity = Some(2.8);
    p.hub = Vec2::new(90_000.0, 90_000.0);
    let target = Vec2::new(0.0, -180_000.0);
    p.chart = vec![(p.home.system.id, Vec2::new(0.0, 2_000.0)), (EntityId(9800), target)];
    p.ghosts = vec![scout.clone()];
    for (id, x) in [(9801, -18_000.0), (9802, 18_000.0)] {
        let mut pirate = scout.clone();
        pirate.id = EntityId(id); pirate.owner = PlayerId::PIRATE; pirate.own = false;
        pirate.kind = H::Raider; pirate.pos = Vec2::new(x, -30_000.0);
        p.ghosts.push(pirate);
    }
    let Some(Command::JumpShip { dest, .. }) = bot.scout_step(&p, &scout, target) else {
        panic!("a blocked narrow fan is not proof that every safe approach is closed");
    };
    assert!(!p.threatened(dest));
    assert!(dest.distance(scout.pos) <= sim::transit::JUMP_RANGE);
    assert!(dest.distance(target) < scout.pos.distance(target));
    assert!(p.chart.iter().all(|(_, well)| dest.distance(*well) > sim::transit::HYPERLIMIT));
}

#[test]
fn scouts_prioritize_the_founders_candidate_reports_without_ignoring_threats() {
    let (mut bot, mut p) = planning_picture();
    let mut scout = p.own().next().unwrap().clone();
    scout.kind = H::Scout;
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    scout.fuel = Some(20.0); scout.fuel_capacity = Some(20.0);
    scout.pos = p.home.system.pos + Vec2::new(10_000.0, 0.0);
    scout.vel = Vec2::ZERO; scout.docked = None; scout.guard_target = None;
    let nearby = EntityId(9801);
    let candidate = EntityId(9802);
    p.chart = vec![(p.home.system.id, p.home.system.pos),
        (nearby, scout.pos + Vec2::new(1_000.0, 0.0)),
        (candidate, scout.pos + Vec2::new(3_000.0, 0.0))];
    p.survey_candidates = vec![candidate]; p.expansion_unlocked = false;
    p.ghosts = vec![scout.clone()];
    let clean_bot = bot.clone();
    assert!(matches!(bot.scouts(&p), Some(Command::SurveySystem { system_id, .. }) if system_id == candidate),
        "the closer unrelated star must not indefinitely postpone the required survey");
    let mut pirate = scout;
    pirate.id = EntityId(9803); pirate.own = false; pirate.owner = PlayerId::PIRATE;
    pirate.kind = H::Raider; pirate.pos = p.chart[2].1;
    p.ghosts.push(pirate);
    bot = clean_bot;
    assert!(!matches!(bot.scouts(&p), Some(Command::SurveySystem { .. })),
        "a founding lead is not permission to investigate under attack");
}

#[test]
fn bots_send_their_mining_barge_to_the_charters_belt() {
    let (mut bot, mut p) = planning_picture();
    let mut barge = p.own().next().unwrap().clone();
    barge.kind = H::MiningBarge;
    barge.composition = Some(vec![crate::protocol::CompCount { kind: H::MiningBarge, count: 1 }]);
    barge.industry = None; barge.guard_target = None; barge.docked = None;
    let (system, body, ore) = (EntityId(9811), 3, sim::Commodity::MetallicOre);
    p.belt_lesson = Some((system, body, ore, p.home.system.pos + Vec2::new(20_000.0, 0.0)));
    p.ghosts = vec![barge.clone()];
    p.founding_stage = sim::FoundingStage::BuildBarge;
    assert!(bot.belt_lesson(&p).is_none(), "the barge is built first");
    p.founding_stage = sim::FoundingStage::MineBelt;
    assert!(matches!(bot.belt_lesson(&p), Some(Command::MineBelt { fleet_id, system_id, body_id, ore: o, .. })
        if fleet_id == barge.id && system_id == system && body_id == body && o == ore));
    let mut working = barge;
    working.industry = Some(sim::industry::FleetIndustry::Mine {
        system, body, ore, pocket: false, to_station: false, mined: 1.0, fraction: 0.0, status: "mining".into() });
    p.ghosts = vec![working.clone()];
    assert!(planning_picture().0.belt_lesson(&p).is_none(), "a barge already at work is left alone");

    // §charter v3: the last step brings a load home and unloads it.
    p.founding_stage = sim::FoundingStage::HaulOre;
    assert!(planning_picture().0.belt_lesson(&p).is_none(), "an empty hold keeps mining");
    let mut loaded = working;
    loaded.cargo_manifest = vec![crate::protocol::CargoView { commodity: ore, units: 40 }];
    p.ghosts = vec![loaded.clone()];
    let home = p.home.system.pos;
    assert!(matches!(planning_picture().0.belt_lesson(&p), Some(Command::MoveShip { ship_id, dest, .. })
        if ship_id == loaded.id && dest == home));
    loaded.industry = None;
    loaded.docked = Some(p.home.system.id.to_string());
    p.ghosts = vec![loaded.clone()];
    assert!(matches!(planning_picture().0.belt_lesson(&p), Some(Command::SystemUnload { fleet_id, system, .. })
        if fleet_id == loaded.id && system == p.home.system.id));
}

/// §belts phase 6: a belt as an arrived report shows it.
fn belt_site(system: EntityId, body: u32, pos: Vec2, owner: Option<PlayerId>, ores: Vec<(C, bool, f64, Option<f64>)>,
    prospected: u32) -> belts::BeltSite {
    belts::BeltSite { system, body, pos, owner, ores, prospected }
}

#[test]
fn after_the_charter_bots_prospect_the_home_belt_then_keep_their_barges_mining() {
    use sim::industry::FleetIndustry;
    let (mut bot, mut p) = strategic_ready();
    p.founding_stage = sim::FoundingStage::Complete;
    let (home, home_pos) = (p.home.system.id, p.home.system.pos);
    let barge = strategic_hull(&p, 9901, H::MiningBarge, 1);
    p.ghosts = vec![barge.clone()];
    p.belts = vec![belt_site(home, 7, home_pos, Some(p.owner), vec![(C::CrystallineOre, false, 0.25, None)], 0)];
    assert!(matches!(bot.belt_work(&p), Some(Command::ProspectBelt { fleet_id, system_id, body_id: 7, .. })
        if fleet_id == barge.id && system_id == home), "the home belt is prospected first");

    // Prospected twice and a Cuprite pocket found: the pocket beats the belt's ore.
    p.belts[0].prospected = 2;
    p.belts[0].ores.push((C::CupriteOre, true, 0.30, Some(3_000.0)));
    assert!(matches!(strategic_ready().0.belt_work(&p), Some(Command::MineBelt { fleet_id, system_id, ore: C::CupriteOre, pocket: true, .. })
        if fleet_id == barge.id && system_id == home));

    // A barge full at a remote belt comes home, and unloads once docked.
    let mut full = barge.clone();
    full.docked = None;
    full.pos = home_pos + Vec2::new(20_000.0, 0.0);
    full.industry = Some(FleetIndustry::Mine { system: EntityId(4242), body: 3, ore: C::MetallicOre, pocket: false,
        to_station: false, mined: 150.0, fraction: 0.0, status: "full".into() });
    full.cargo_manifest = vec![crate::protocol::CargoView { commodity: C::MetallicOre, units: 150 }];
    p.ghosts = vec![full.clone()];
    assert!(matches!(strategic_ready().0.belt_work(&p), Some(Command::MoveShip { ship_id, dest, .. })
        if ship_id == full.id && dest == home_pos));
    full.industry = None;
    full.docked = Some(home.to_string());
    full.pos = home_pos;
    p.ghosts = vec![full.clone()];
    assert!(matches!(strategic_ready().0.belt_work(&p), Some(Command::SystemUnload { fleet_id, system, .. })
        if fleet_id == full.id && system == home));

    // Nothing of this runs during the charter's own barge lesson.
    p.founding_stage = sim::FoundingStage::MineBelt;
    assert!(strategic_ready().0.belt_work(&p).is_none());
}

/// §power: a planet whose demand outruns its base and plant gets a plant
/// (on that planet, before most other work), its burn joins the fuel
/// buffers, and a planet past a tier IV plant sends research to fusion.
#[test]
fn bots_power_their_short_planets_and_fuel_the_plants() {
    let (bot, mut p) = strategic_ready();
    // Past the opening's necessities (it ends with a Scout).
    let scout = strategic_hull(&p, 9911, H::Scout, 1);
    p.ghosts.push(scout);
    let busy = p.home.system.bodies[0].id;
    for b in &mut p.home.system.bodies {
        b.grid = sim::power::Grid { demand: if b.id == busy { 6.4 } else { 1.0 }, supply: 3.0, power: 0.0, ..Default::default() };
        b.grid.power = (3.0 / b.grid.demand).min(1.0);
    }
    assert_eq!(p.power_shortfall().map(|(body, _)| body), Some(busy), "the short planet, not the one on its base");
    let project = p.power_project().expect("a plant for the short planet");
    assert_eq!((project.what, project.body), (BuildKind::Upgrade { upgrade: K::PowerPlant }, busy));
    assert_eq!(bot.strategic_project(&p), Some(project), "power comes before other development");

    // A dry plant is a fuel problem: no new building, and the burn is stocked.
    // §fuel-overhaul: a Solar Array never runs dry; a Fusion Plant (III) does.
    for b in &mut p.home.system.bodies {
        if b.id == busy {
            b.set_tier(K::PowerPlant, 3);
            b.grid.no_fuel = true;
            b.grid.burn = 0.02;
        }
    }
    assert!(p.power_project().is_none());
    let mut wanted = BTreeMap::new();
    bot.industry_buffers(&p, None, &mut wanted);
    assert!(wanted.get(&C::Deuterium).copied().unwrap_or(0.0) > 0.0, "plant fuel joins the buffers");

    // Past a tier IV plant, research goes to fusion.
    for b in &mut p.home.system.bodies {
        if b.id == busy {
            b.set_tier(K::PowerPlant, 4);
            b.grid = sim::power::Grid { demand: 60.0, supply: 51.0, power: 0.85, ..Default::default() };
        }
    }
    p.available_research = vec![sim::power::FUSION_RESEARCH.to_string()];
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some(sim::power::FUSION_RESEARCH));
}

/// The home belt always hides a pocket; a remote belt with richer ore
/// usually hides nothing. Our own belt gets its passes first.
#[test]
fn barges_prospect_the_home_belt_before_richer_remote_ore() {
    let (mut bot, mut p) = strategic_ready();
    p.founding_stage = sim::FoundingStage::Complete;
    let (home, home_pos) = (p.home.system.id, p.home.system.pos);
    let barge = strategic_hull(&p, 9902, H::MiningBarge, 1);
    p.ghosts = vec![barge];
    p.belts = vec![
        belt_site(EntityId(4242), 3, home_pos + Vec2::new(15_000.0, 0.0), None, vec![(C::CupriteOre, false, 0.5, None)], 1),
        belt_site(home, 7, home_pos, Some(p.owner), vec![(C::CrystallineOre, false, 0.25, None)], 1),
    ];
    assert!(matches!(bot.belt_work(&p), Some(Command::ProspectBelt { system_id, body_id: 7, .. }) if system_id == home),
        "one pass isn't enough at home");
    // With its pocket found, the home belt's search is over.
    p.belts[1].ores.push((C::CupriteOre, true, 0.30, Some(3_000.0)));
    assert!(!matches!(strategic_ready().0.belt_work(&p), Some(Command::ProspectBelt { body_id: 7, .. })));

    // Worked dry at home, the barge takes new work rather than idling.
    p.belts[1].ores.last_mut().unwrap().3 = Some(0.0);
    p.ghosts[0].industry = Some(sim::industry::FleetIndustry::Mine { system: home, body: 7, ore: C::CupriteOre, pocket: true,
        to_station: false, mined: 3_000.0, fraction: 0.0, status: "depleted".into() });
    assert!(matches!(strategic_ready().0.belt_work(&p), Some(Command::MineBelt { pocket: false, .. })),
        "a depleted pocket at home leaves the barge free for the belt's ore");
}

#[test]
fn bots_research_build_and_anchor_a_station_then_run_a_freighter_to_it() {
    use sim::industry::{Anchor, AnchorPhase, FleetIndustry, FreightPort};
    let (bot, mut p) = strategic_ready();
    p.founding_stage = sim::FoundingStage::Complete;
    let home = p.home.system.id;
    let (belt, belt_pos) = (EntityId(4242), p.home.system.pos + Vec2::new(20_000.0, 0.0));
    let barge = strategic_hull(&p, 9901, H::MiningBarge, 1);
    p.ghosts = vec![barge.clone()];
    p.belts = vec![belt_site(belt, 3, belt_pos, None, vec![(C::CupriteOre, false, 0.2, None)], 1)];
    assert!(p.wants_station(), "barges at work and an unowned belt in reach");
    p.available_research = vec!["mat_belt_stations".into()];
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some("mat_belt_stations"));
    p.research.completed.insert("mat_belt_stations".into());
    assert!(bot.fleet_needs(&p).iter().any(|(k, n, _)| *k == H::MiningBeltStation && *n == 1));

    // A built station at home anchors at that belt.
    let station = strategic_hull(&p, 9902, H::MiningBeltStation, 1);
    p.ghosts = vec![barge.clone(), station.clone()];
    assert!(matches!(strategic_ready().0.belt_work(&p), Some(Command::AnchorStation { fleet_id, system_id, body_id: 3, .. })
        if fleet_id == station.id && system_id == belt));

    // Anchored, with a barge cutting there: more barges, and a freighter's run.
    let mut anchored = station.clone();
    anchored.anchor = Some(Anchor { system: belt, body: 3, phase: AnchorPhase::Anchored, since: 0.0 });
    anchored.docked = None;
    anchored.pos = belt_pos;
    let mut cutting = barge.clone();
    cutting.docked = None;
    cutting.pos = belt_pos;
    cutting.industry = Some(FleetIndustry::Mine { system: belt, body: 3, ore: C::CupriteOre, pocket: false,
        to_station: true, mined: 40.0, fraction: 0.0, status: "mining".into() });
    let freighter = strategic_hull(&p, 9903, H::Convoy, 1);
    p.ghosts = vec![cutting, anchored.clone(), freighter.clone()];
    assert_eq!(p.barge_need(), 3);
    let mut hauling = strategic_ready().0;
    let command = hauling.belt_work(&p);
    assert!(matches!(&command, Some(Command::SetFreightRoute { fleet_id, route: Some(r), .. })
        if *fleet_id == freighter.id && r.repeat && r.stops[0].port == FreightPort::Station { id: anchored.id }
            && r.stops[0].load.contains_key(&C::CupriteOre) && r.stops[1].port == FreightPort::System { id: home }),
        "{command:?}");
    assert_eq!(hauling.belt_haulers.get(&freighter.id), Some(&anchored.id));
    assert!(hauling.routes(&p, &BTreeMap::new()).is_none_or(|c| !matches!(c,
        Command::SetFreightRoute { fleet_id, .. } if fleet_id == freighter.id)), "the Hub trade leaves the hauler alone");
}

#[test]
fn colony_goals_end_on_settlement_reports_or_a_closed_founding_gate() {
    let (mut bot, mut p) = planning_picture();
    let goal = DevelopmentGoal { project: Project { what: BuildKind::Ship { ship: H::Colony },
        body: p.home.system.bodies[0].id }, target: 1, since: p.now };
    bot.development = Some(goal.clone());
    bot.expansion = Some(EntityId(9802));
    p.expansion_unlocked = false;
    bot.refresh_development(&p);
    assert_ne!(bot.project(&p), Some(goal.project));
    assert!(bot.expansion.is_none());
    p.expansion_unlocked = true;
    let mut colony = p.home.clone(); colony.system.id = EntityId(9802);
    p.sites.push(colony);
    bot.development = Some(goal.clone()); bot.expansion = Some(EntityId(9802));
    bot.refresh_development(&p);
    assert_ne!(bot.project(&p), Some(goal.project));
    assert!(bot.expansion.is_none(), "the consumed Colony Ship need not appear before its settlement report");
}

#[test]
fn pirate_held_colony_candidates_change_only_when_the_scout_report_arrives() {
    let mut g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    g.bots.get_mut(&owner).unwrap().next_think = 1e9;
    advance(&mut g, 100.0);
    let cc = g.world.players[&owner].command_center;
    let target = g.world.systems.iter_mut().find(|s| s.owner.is_none()
        && s.pos.distance(cc) > 10_000.0 && s.bodies.iter().any(|b| b.habitable)).unwrap();
    let id = target.id;
    let pos = target.pos;
    g.world.players.get_mut(&owner).unwrap().surveyed.insert(id);
    assert!(g.bot_observation(owner).unwrap().prospects.iter().any(|s| s.id == id));
    let delay = sim::transit::delay(pos, cc, g.world.config.c);
    let at = g.world.time;
    g.world.players.get_mut(&owner).unwrap().intel.insert(id, sim::IntelSnapshot { station_level: 0,
        defense_tier: 4, shipyard_tier: 0, enclave_tier: 2, garrison_tier: 0, observed_at: at, pos,
    });
    g.world.record_information();
    assert!(g.bot_observation(owner).unwrap().prospects.iter().any(|s| s.id == id),
        "a capture-time pirate report must not leak ahead of its light");
    advance(&mut g, delay + 1.0);
    assert!(!g.bot_observation(owner).unwrap().prospects.iter().any(|s| s.id == id),
        "a surveyed pirate base is a conquest target, not a civilian settlement");
    let at = g.world.time;
    let report = g.world.players.get_mut(&owner).unwrap().intel.get_mut(&id).unwrap();
    report.enclave_tier = 0; report.defense_tier = 0; report.observed_at = at;
    g.world.record_information();
    assert!(!g.bot_observation(owner).unwrap().prospects.iter().any(|s| s.id == id));
    advance(&mut g, delay + 1.0);
    assert!(g.bot_observation(owner).unwrap().prospects.iter().any(|s| s.id == id));
}

#[test]
fn redirected_colony_hulls_refuel_instead_of_waiting_in_open_space() {
    let (mut bot, mut p) = planning_picture();
    p.expansion_unlocked = true;
    p.home.system.stockpile.insert(C::Deuterium, 500.0);
    let mut colony = p.own().next().unwrap().clone();
    colony.kind = H::Colony;
    colony.composition = Some(vec![crate::protocol::CompCount { kind: H::Colony, count: 1 }]);
    colony.pos = p.home.system.pos + Vec2::new(2_000.0, 0.0);
    colony.docked = None; colony.vel = Vec2::ZERO;
    colony.fuel = Some(p.leg(&colony, colony.pos, p.home.system.pos, 0) * 1.05);
    colony.fuel_capacity = Some(hull_mass(&colony) * sim::fuel::FUEL_PER_HULL_MASS);
    p.ghosts = vec![colony.clone()];
    let target = EntityId(9804);
    p.prospects = vec![Prospect { id: target, pos: p.home.system.pos + Vec2::new(30_000.0, 0.0), score: 5.0 }];
    bot.expansion = Some(target);
    assert!(p.enough_fuel(&colony, p.home.system.pos));
    assert!(!p.enough_fuel(&colony, p.prospects[0].pos));
    assert!(matches!(bot.colonies(&p), Some(Command::MoveShip { dest, .. }) if dest == p.home.system.pos));
    assert!(bot.returns.contains_key(&colony.id));
    bot.waits.clear();
    assert!(!matches!(bot.colonies(&p), Some(Command::MoveShip { dest, .. } | Command::JumpShip { dest, .. })
        if dest == p.prospects[0].pos), "don't replace the refill intention on an old report");
}

#[test]
fn colony_expeditions_stage_through_paid_hub_fuel_without_inventing_a_jump_drive() {
    let (mut bot, mut p) = planning_picture();
    p.expansion_unlocked = true;
    p.home.system.pos = Vec2::new(-100_000.0, 0.0);
    p.cc = p.home.system.pos; p.hub = Vec2::ZERO;
    p.sites = vec![p.home.clone()];
    let mut colony = p.own().next().unwrap().clone();
    colony.kind = H::Colony;
    colony.composition = Some(vec![crate::protocol::CompCount { kind: H::Colony, count: 1 }]);
    let tank = hull_mass(&colony) * sim::fuel::FUEL_PER_HULL_MASS;
    colony.fuel = Some(tank); colony.fuel_capacity = Some(tank);
    colony.pos = p.home.system.pos; colony.vel = Vec2::ZERO;
    colony.docked = Some(p.home.system.id.to_string());
    let target = EntityId(9805);
    let far = Vec2::new(100_000.0, 0.0);
    p.chart = vec![(p.home.system.id, p.home.system.pos), (target, far)];
    p.prospects = vec![Prospect { id: target, pos: far, score: 5.0 }];
    p.ghosts = vec![colony.clone()]; bot.expansion = Some(target);
    assert!(!H::Colony.has_jump_drive());
    assert!(!p.enough_fuel(&colony, far), "the direct warp trip exceeds the tank");
    assert!(Bot::reachable_prospect(&p, &p.prospects[0]), "two ordinary warp legs fit with a paid refill");
    assert!(matches!(bot.colonies(&p), Some(Command::MoveShip { dest, .. }) if dest == p.hub));
    assert!(bot.colonies(&p).is_none(), "wait for the ordinary command/report round trip");
    p.now += 600.0;
    p.ghosts[0].pos = p.hub; p.ghosts[0].docked = Some("hub".into());
    p.ghosts[0].fuel = Some(1.0);
    assert!(bot.colonies(&p).is_none(), "a berth report without sufficient fuel is not ready to depart");
    p.ghosts[0].fuel = Some(tank);
    assert!(matches!(bot.colonies(&p), Some(Command::MoveShip { dest, .. }) if dest == far));
}

#[test]
fn a_reported_colony_range_blocker_prioritizes_available_fuel_research() {
    let (bot, mut p) = planning_picture();
    p.expansion_unlocked = true;
    p.prospects = vec![Prospect { id: EntityId(9806), pos: p.hub + Vec2::new(500_000.0, 0.0), score: 5.0 }];
    p.available_research = vec!["mat_bulk_storage".into(), "prop_efficient_burns".into()];
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some("prop_efficient_burns"));
    p.available_research.retain(|id| id != "prop_efficient_burns");
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some("mat_bulk_storage"), "range pressure never bypasses prerequisites");
    p.available_research.push("prop_efficient_burns".into());
    p.prospects[0].pos = p.home.system.pos + Vec2::new(10_000.0, 0.0);
    assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some("mat_bulk_storage"));
}

#[test]
fn one_unit_pickup_does_not_block_the_rest_of_a_project_import() {
    use sim::industry::{FleetIndustry, FreightRun, RoutePhase};
    let (mut bot, mut p) = planning_picture();
    p.exported = true;
    p.home.system.stockpile.insert(C::Alloys, 0.0);
    p.home.system.stockpile.insert(C::MetallicOre, 200.0);
    p.account.as_mut().unwrap().warehouse.insert(C::Alloys, 1);
    for f in &mut p.ghosts { f.fuel = f.fuel_capacity; }
    let f = p.ghosts.iter_mut().find(|f| f.kind.is_player_freighter()).unwrap();
    f.docked = Some("hub".into()); f.pos = p.hub;
    let wanted = BTreeMap::from([(C::Alloys, 35.0)]);
    let Some(Command::SetFreightRoute { route: Some(route), fleet_id, .. }) = bot.routes(&p, &wanted) else {
        panic!("funded circuit must be viable");
    };
    let f = p.ghosts.iter_mut().find(|f| f.id == fleet_id).unwrap();
    f.pos = p.hub + Vec2::new(1_000.0, 0.0); f.docked = None;
    f.industry = Some(FleetIndustry::Route { run: FreightRun {
        route, stop: 0, phase: RoutePhase::Travelling, remaining_load: BTreeMap::new(),
        shopping_spent: 0.0,
        remaining_unload: BTreeMap::new(), visits: 1, hold: None,
    }});
    p.account.as_mut().unwrap().warehouse.insert(C::Alloys, 35);
    bot.waits.clear();
    assert!(matches!(bot.market(&p, &wanted), Some(Command::BookFreightOut {
        commodity: C::Alloys, units: 34, ..
    })), "one promised unit must not strand the other 34 in the warehouse");
}

#[test]
fn research_supply_budget_covers_delivery_and_is_bounded_by_remaining_work() {
    let (bot, mut p) = planning_picture();
    p.research.active = Some("mat_enrichment".into());
    p.research.progress = 0.0;
    let wanted = bot.desired_stock(&p, None);
    assert!(wanted[&C::Electronics] >= (p.supply_lead() * 2.0 * 0.01).min(72.0).ceil());
    assert!(wanted[&C::Electronics] > 6.0, "ten minutes of supply recreated the starvation cycle");
    p.research.progress = sim::research::cost_of("mat_enrichment") - 10.0;
    assert_eq!(bot.desired_stock(&p, None)[&C::Electronics], 1.0);
}

#[test]
fn full_storage_exports_surplus_before_buying_more_imports() {
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.clear();
    p.home.system.stockpile.insert(C::MetallicOre, p.home.system.storage_cap());
    assert!(matches!(bot.market(&p, &BTreeMap::from([(C::Electronics, 50.0)])),
        Some(Command::BookFreightIn { commodity: C::MetallicOre, sell_on_arrival: true, .. })));
}

/// §fuel-overhaul: Freight Frames, then Fusion Freight, ahead of the ordinary scoring.
#[test]
fn fuel_logistics_lead_the_research_order() {
    let (bot, mut p) = planning_picture();
    p.available_research = ["mat_enrichment", "prop_line_fusion_freight", "weap_fire_control", "prop_freight_frames"]
        .map(String::from).to_vec();
    for id in ["prop_freight_frames", "prop_line_fusion_freight"] {
        assert_eq!(bot.next_research(&p, &BTreeMap::new()), Some(id));
        p.research.completed.insert(id.into());
        p.available_research.retain(|a| a != id);
    }
    assert!(matches!(bot.next_research(&p, &BTreeMap::new()), Some("mat_enrichment" | "weap_fire_control")),
        "then the ordinary scoring");
}

/// §fuel: a refinery only pays beside home Volatiles, and a fuel shortage
/// never holds the fuel line that would end it.
#[test]
fn a_fuel_line_is_built_through_a_fuel_shortage_but_only_beside_volatiles() {
    let (bot, mut p) = strategic_ready();
    for b in &mut p.home.system.bodies { b.deposits.retain(|d| d.resource != C::Volatiles && d.resource != C::Helium3Ice); }
    p.home.system.bodies[0].deposits.push(sim::galaxy::Deposit {
        resource: C::MetallicOre, richness: 1.0, reserves: None, accessibility: 1.0,
    });
    assert_ne!(bot.role(&p), strategy::Role::Fuel);
    assert!(bot.structure_value(&p, K::FuelRefinery) <= 20.0, "imported Volatiles save no freight");
    p.home.system.bodies[0].deposits.push(sim::galaxy::Deposit {
        resource: C::Volatiles, richness: 0.5, reserves: None, accessibility: 1.0,
    });
    assert_eq!(bot.structure_value(&p, K::VolatileHarvester), 150.0);
    assert!(bot.structure_value(&p, K::FuelRefinery) <= 20.0, "nothing to refine yet");
    p.home.system.bodies[0].set_tier(K::VolatileHarvester, 1);
    assert_eq!(bot.structure_value(&p, K::FuelRefinery), 150.0);

    for (c, n) in [(C::Alloys, 500.0), (C::Machinery, 500.0), (C::Polymers, 500.0), (C::Electronics, 500.0)] {
        p.home.system.stockpile.insert(c, n);
    }
    p.home.system.stockpile.insert(C::Deuterium, 0.0);
    assert!(p.fuel_critical());
    let refinery = p.structure(K::FuelRefinery, 1).expect("Bunkerage unlocks the refinery");
    assert!(bot.may_build(&p, refinery), "the cure for a fuel shortage is never held by it");
    let scout = p.ship(H::Scout, p.count(H::Scout) + 1).expect("a Scout fits the yard");
    assert!(!bot.may_build(&p, scout), "optional work still waits for Fuel");
}

/// §fuel: a homebound freighter buys the Fuel its home lacks at the Hub and
/// carries it home; a dry home sends an idle hull for it.
#[test]
fn freighters_bring_the_missing_fuel_home_from_the_hub() {
    use sim::industry::{FleetIndustry, FreightPort, FreightRoute, FreightRun, RoutePhase, RouteStop};
    let (mut bot, mut p) = planning_picture();
    p.exported = true;
    p.home.system.stockpile = BTreeMap::from([(C::Deuterium, 0.0)]);
    let id = p.own().find(|g| g.kind.is_player_freighter()).unwrap().id;
    for g in &mut p.ghosts {
        g.fuel = g.fuel_capacity;
        if g.kind == H::Raider { g.guard_target = Some(id); }
    }
    let wanted = BTreeMap::from([(C::Deuterium, 200.0)]);

    // The charter's first shipment keeps its Freighter home.
    p.exported = false;
    assert!(bot.routes(&p, &wanted).is_none(), "no empty run before the first shipment");
    p.exported = true;

    // Nothing worth selling at home, yet the dry home sends its hull.
    let Some(Command::SetFreightRoute { route: Some(out), .. }) = bot.routes(&p, &wanted) else { panic!("fuel run") };
    assert_eq!(out.stops[1].port, FreightPort::Hub);
    assert!(out.shopping_budget.is_none(), "the buying happens on the return leg");
    assert!(!bot.ready("fuel-run", p.now), "one run per round trip");

    // At the Hub, with an empty warehouse, the hold is filled by buying.
    bot.waits.clear();
    let f = p.ghosts.iter_mut().find(|f| f.id == id).unwrap();
    f.docked = Some("hub".into()); f.pos = p.hub;
    let Some(Command::SetFreightRoute { route: Some(back), .. }) = bot.routes(&p, &wanted) else { panic!("return leg") };
    let fuel = back.stops[0].load.get(&C::Deuterium).copied().unwrap_or(0);
    assert!(fuel > 0 && fuel <= 50, "a Tiny's hold of Fuel: {fuel}");
    assert_eq!(back.stops[1].unload.get(&C::Deuterium), Some(&fuel));
    assert!(back.shopping_budget.is_some_and(|b| b >= fuel as f64 * p.price(C::Deuterium)));
    assert!(!bot.ready("market", p.now) && !bot.commitments.is_empty(), "the cash is held until the account report");
    let mut g = game(2, 1);
    g.bots.get_mut(&p.owner).unwrap().next_think = 1e9;
    g.pending.push(Command::SetFreightRoute { player_id: p.owner, fleet_id: id, route: Some(back.clone()) });
    advance(&mut g, 10.0);
    assert!(matches!(&g.world.fleets[&id].industry, Some(FleetIndustry::Route { run }) if run.route == back),
        "the shopping leg passes the simulation's route check");

    // A hold already bound for the Hub covers a small lack: no second run.
    let (mut bot, mut p) = planning_picture();
    p.exported = true;
    p.home.system.stockpile = BTreeMap::from([(C::Deuterium, 0.0)]);
    let id = p.own().find(|g| g.kind.is_player_freighter()).unwrap().id;
    for g in &mut p.ghosts {
        g.fuel = g.fuel_capacity;
        if g.kind == H::Raider { g.guard_target = Some(id); }
    }
    let mut bound = strategic_hull(&p, 9701, H::SmallFreighter, 1);
    bound.docked = None; bound.pos = (p.home.system.pos + p.hub) * 0.5;
    let stop = |port, sell| RouteStop { port, load: BTreeMap::new(), unload: BTreeMap::new(), sell };
    bound.industry = Some(FleetIndustry::Route { run: FreightRun {
        route: FreightRoute { shopping_budget: None, name: "Home / Market".into(), transit: sim::ship::TransitMode::Slow,
            stops: vec![stop(FreightPort::System { id: p.home.system.id }, false), stop(FreightPort::Hub, true)],
            fuel_reserve: 1.0, escort: None, repeat: false },
        stop: 1, phase: RoutePhase::Travelling, remaining_load: BTreeMap::new(), remaining_unload: BTreeMap::new(),
        visits: 0, hold: None, shopping_spent: 0.0,
    } });
    p.ghosts.push(bound);
    assert_eq!(p.hub_bound_room(), 150);
    assert!(bot.routes(&p, &BTreeMap::from([(C::Deuterium, 100.0)])).is_none(), "the bound Small Freighter brings it");
    assert!(matches!(bot.routes(&p, &wanted), Some(Command::SetFreightRoute { fleet_id, .. }) if fleet_id == id),
        "a bigger lack still sends the idle hull");
}

/// §fuel: the target is held whole and refilled at three quarters, well
/// clear of the critical half.
#[test]
fn fuel_refills_well_above_the_critical_line() {
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::Uranium, 40.0);
    let target = p.fuel_target_of(C::Deuterium);
    assert!(bot.desired_stock(&p, None)[&C::Deuterium] >= target, "the whole target, not the critical half");
    let wanted = BTreeMap::from([(C::Deuterium, target)]);
    p.home.system.stockpile.insert(C::Deuterium, (target * 0.7).floor());
    assert!(!p.fuel_critical());
    assert!(matches!(bot.market(&p, &wanted), Some(Command::MarketBuy { commodity: C::Deuterium, .. })),
        "not critical, yet refilled");
    bot.waits.clear(); bot.commitments.clear();
    p.home.system.stockpile.insert(C::Deuterium, (target * 0.8).ceil());
    assert!(!matches!(bot.market(&p, &wanted), Some(Command::MarketBuy { commodity: C::Deuterium, .. })));
}

/// §fuel: clearing full storage books the biggest pile, not the first ore lot.
#[test]
fn clearing_full_storage_ships_the_biggest_pile_first() {
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.clear();
    let cap = p.home.system.storage_cap();
    assert!(cap >= 500.0);
    p.home.system.stockpile.insert(C::MetallicOre, 120.0);
    p.home.system.stockpile.insert(C::Provisions, cap - 120.0);
    assert!(matches!(bot.export_surplus(&p, &BTreeMap::new(), true),
        Some(Command::BookFreightIn { commodity: C::Provisions, units, .. }) if units > 120));
}

/// §fuel: Freight Frames bring one bigger hold to a fleet of Tiny Freighters.
#[test]
fn freight_frames_bring_one_small_freighter_to_a_tiny_fleet() {
    let (bot, mut p) = strategic_ready();
    p.ghosts = vec![strategic_hull(&p, 9601, H::TinyFreighter, 1), strategic_hull(&p, 9602, H::TinyFreighter, 1)];
    let small = |bot: &Bot, p: &Observation| bot.fleet_needs(p).iter()
        .find(|(k, _, _)| *k == H::SmallFreighter).map(|(_, n, _)| *n);
    assert_eq!(small(&bot, &p), None, "not researched");
    p.research.completed.insert("prop_freight_frames".into());
    assert_eq!(bot.preferred_freighter(&p), H::SmallFreighter);
    assert_eq!(small(&bot, &p), Some(1));
    p.ghosts.push(strategic_hull(&p, 9603, H::SmallFreighter, 1));
    assert_eq!(small(&bot, &p), Some(1), "one is enough");
}

#[test]
fn development_goal_survives_a_new_bottleneck_until_its_completion_report() {
    let (mut bot, mut p) = planning_picture();
    bot.refresh_development(&p);
    let goal = bot.development.clone().unwrap();
    assert_eq!(goal.project.what, BuildKind::Upgrade { upgrade: K::MiningComplex });
    p.home.system.stockpile.insert(C::Deuterium, 0.0);
    bot.refresh_development(&p);
    assert_eq!(bot.project(&p), Some(goal.project));
    let BuildKind::Upgrade { upgrade } = goal.project.what else { unreachable!() };
    p.home.system.bodies.iter_mut().find(|b| b.id == goal.project.body).unwrap().structures.insert(upgrade, goal.target);
    bot.refresh_development(&p);
    assert_ne!(bot.project(&p), Some(goal.project));
    let saved: Bot = serde_json::from_value(serde_json::to_value(&bot).unwrap()).unwrap();
    assert_eq!(saved.project(&p), bot.project(&p));
}

#[test]
fn distant_threat_allows_a_scout_to_leave_in_the_opposite_direction() {
    let (mut bot, mut p) = planning_picture();
    let mut scout = p.own().next().unwrap().clone();
    scout.kind = H::Scout; scout.fuel = Some(2.8); scout.fuel_capacity = Some(2.8);
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    scout.pos = p.home.system.pos; scout.vel = Vec2::ZERO;
    let mut pirate = scout.clone();
    pirate.id = EntityId(991); pirate.own = false; pirate.owner = PlayerId::PIRATE;
    pirate.kind = H::Raider; pirate.pos = scout.pos + Vec2::new(20_000.0, 0.0);
    let target = scout.pos - Vec2::new(40_000.0, 0.0);
    p.chart = vec![(p.home.system.id, scout.pos), (EntityId(992), target)];
    p.ghosts = vec![scout.clone(), pirate];
    assert!(p.threatened(scout.pos));
    assert!(matches!(bot.scouts(&p), Some(Command::MoveShip { dest, .. }) if dest.x < scout.pos.x));
    bot.waits.clear(); bot.surveys.clear();
    p.ghosts[1].pos = scout.pos + Vec2::new(1_000.0, 0.0);
    assert!(bot.scouts(&p).is_none(), "a close threat still closes the dock; do not suicide into a spool");
}

#[test]
fn workforce_budget_keeps_science_staffed_without_diluting_it_into_idle_factories() {
    let (mut bot, mut p) = planning_picture();
    for b in &mut p.home.system.bodies { b.population = 0.0; b.structures.clear(); b.assignments.clear(); }
    let body = &mut p.home.system.bodies[0];
    body.population = 0.004;
    body.migration_policy = sim::MigrationPolicy::Open;
    for kind in [K::Bioharvester, K::Agroplex, K::MiningComplex, K::Academy, K::Smelter, K::Shipyard] {
        body.structures.insert(kind, 1);
        body.assignments.insert(kind, sim::production::Assignment { workers: 1, ..Default::default() });
    }
    p.sites = vec![p.home.clone()];
    for _ in 0..8 {
        bot.waits.clear();
        let Some(cmd) = bot.staff(&p) else { break; };
        match cmd {
            Command::SetAssignment { structure, workers, body_id: Some(id), .. } => {
                let body = p.sites[0].system.bodies.iter_mut().find(|b| b.id == id).unwrap();
                body.assignments.get_mut(&structure).unwrap().workers = workers;
            }
            Command::SetMigrationPolicy { body_id, policy, .. } => {
                p.sites[0].system.bodies.iter_mut().find(|b| b.id == body_id).unwrap().migration_policy = policy;
            }
            _ => panic!("unexpected staffing command"),
        }
    }
    let s = &p.sites[0].system;
    assert_eq!(s.workforce_posted(), 4);
    assert_eq!(s.staffing_share(), 1.0);
    assert_eq!(s.bodies[0].assignments[&K::Academy].workers, 1);
    assert_eq!(s.bodies[0].assignments[&K::Shipyard].workers, 0);
    assert_eq!(s.bodies[0].assignments[&K::Smelter].workers, 0);
}

#[test]
#[ignore = "multi-seed normal-economy balance run; release recommended"]
fn multi_seed_bot_balancing_report() {
    // BOT_BALANCE_TRACE=<filter> (e.g. "server::game_loop::bots=debug") prints
    // every bot command and decision change, for diagnosing a stuck opening.
    if let Ok(filter) = std::env::var("BOT_BALANCE_TRACE") {
        let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_writer(std::io::stderr).with_ansi(false).try_init();
    }
    let minutes: u32 = std::env::var("BOT_BALANCE_MINUTES").ok().and_then(|s| s.parse().ok()).unwrap_or(24 * 60);
    let report_every: u32 = std::env::var("BOT_BALANCE_REPORT_MINUTES").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(60).max(1);
    let detail = std::env::var("BOT_BALANCE_DETAIL").is_ok_and(|s| s == "1");
    let seeds = std::env::var("BOT_BALANCE_SEEDS").unwrap_or_else(|_| "12648430,42,2026".into());
    // Optional, isolated long-run output. Never reset or overwrite an existing
    // playtest: a checkpoint can be resumed only with an explicit opt-in and a
    // matching seed/elapsed-minute ledger. Physics and bot policy are unchanged.
    let save_dir = std::env::var_os("BOT_BALANCE_SAVE_DIR").map(std::path::PathBuf::from);
    assert!(save_dir.is_none() || seeds.split(',').count() == 1, "one save directory needs exactly one seed");
    let resume = std::env::var("BOT_BALANCE_RESUME").is_ok_and(|s| s == "1");
    for seed in seeds.split(',').map(|s| s.parse::<u64>().unwrap()) {
        let started = std::time::Instant::now();
        let mut restored = None;
        let saver = save_dir.as_ref().map(|dir| {
            let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
            let (storage, saved) = runtime.block_on(crate::persistence::Storage::open(dir.clone(), false)).unwrap();
            assert!(saved.is_none() || resume, "save already exists; BOT_BALANCE_RESUME=1 is required");
            restored = saved;
            (runtime, storage)
        });
        let (status, _) = watch::channel(ServerStatus::default());
        let (estimates, _) = mpsc::unbounded_channel();
        let mut metrics = BalanceMetrics { seed, ..Default::default() };
        let mut g = if let Some(saved) = restored {
            saved.validate().unwrap();
            assert_eq!(saved.world.config.seed, seed, "resume must use the checkpoint's seed");
            metrics = serde_json::from_slice(&std::fs::read(save_dir.as_ref().unwrap().join("balance-metrics.json")).unwrap()).unwrap();
            assert_eq!(metrics.seed, seed);
            assert_eq!(saved.world.tick, 1 + metrics.minute as u64 * 60 * sim::config::TICK_HZ as u64,
                "checkpoint and metric ledger must describe the same instant");
            GameLoop::restore(saved, 1.0, status, estimates)
        } else { GameLoop::new(World::new(sim::SimConfig::for_players(seed, 8)), 1.0, status, estimates) };
        g.configure_bots(Some(8)).unwrap();
        let previous_wall = metrics.elapsed_wall_s;
        assert!(metrics.minute <= minutes, "target duration precedes the saved instant");
        if metrics.minute == 0 {
            if let (Some((runtime, storage)), Some(dir)) = (&saver, &save_dir) {
                runtime.block_on(storage.checkpoint(g.durable_checkpoint())).unwrap();
                metrics.write(dir);
            }
        }
        for minute in metrics.minute + 1..=minutes {
            // Exactly 1,800 ordinary physics ticks per minute. Repeated f64
            // 'advance by 60s' comparisons could add an extra tick each minute.
            for _ in 0..60 * sim::config::TICK_HZ { g.tick(); }
            for &owner in g.bots.keys() {
                let p = g.bot_observation(owner).unwrap();
                if p.research.active.is_some() && !p.home.academies.iter().any(|a| a.rate > 0.0 && a.supplied) {
                    *metrics.starved_minutes.entry(owner).or_default() += 1;
                }
                if p.sites.len() > 1 { metrics.first_colony.entry(owner).or_insert(minute); }
                if g.world.systems.iter().filter(|s| s.owner == Some(owner))
                    .any(|s| s.bodies.iter().any(|b| b.grid.power < 0.999)) {
                    *metrics.brownout_minutes.entry(owner).or_default() += 1;
                }
                if p.fuel_critical() { *metrics.fuel_critical_minutes.entry(owner).or_default() += 1; }
            }
            metrics.minute = minute;
            metrics.elapsed_wall_s = previous_wall + started.elapsed().as_secs_f64();
            if minute % (24 * 60) == 0 || minute == minutes {
                if let (Some((runtime, storage)), Some(dir)) = (&saver, &save_dir) {
                    runtime.block_on(storage.checkpoint(g.durable_checkpoint())).unwrap();
                    metrics.write(dir);
                    eprintln!("checkpoint seed={seed} minute={minute} path={} wall_s={:.1}", dir.display(), metrics.elapsed_wall_s);
                }
            }
            if minute % report_every != 0 && minute != minutes { continue; }
            eprintln!("run_progress={}", serde_json::json!({ "seed": seed, "minute": minute,
                "target_minutes": minutes, "tick": g.world.tick, "elapsed_wall_s": metrics.elapsed_wall_s,
                "simulated_seconds": g.world.time, "fleets": g.world.fleets.len() }));
            for (&owner, bot) in &g.bots {
                let p = g.bot_observation(owner).unwrap();
                eprintln!("seed={seed} minute={minute} bot={} cash={:.0} fuel={:.1} hulls={} scouts={} surveys={} colonies={} routes={} stranded={} mine={} yard={} academy={} research={} commands={} status={}",
                    bot.slot + 1, p.account.as_ref().map_or(0.0, |a| a.credits), p.stock(C::Deuterium),
                    p.own().count(), p.count(H::Scout), p.surveyed.len().saturating_sub(1), p.sites.len().saturating_sub(1),
                    p.own().filter(|g| matches!(g.industry, Some(sim::industry::FleetIndustry::Route { .. }))).count(),
                    p.own().filter(|g| g.stalled).count(), p.home.system.tier(K::MiningComplex), p.home.system.tier(K::Shipyard),
                    p.home.system.tier(K::Academy), p.research.completed.len(), bot.commands_sent, bot.status);
                let mut progress = serde_json::json!({
                    "seed": seed, "minute": minute, "bot": bot.slot + 1,
                    "active": p.research.active, "work": p.research.progress,
                    "completed": p.research.completed, "starved_minutes": metrics.starved_minutes.get(&owner).copied().unwrap_or(0),
                    "first_colony": metrics.first_colony.get(&owner), "cycles": bot.trade_cycles,
                    "founding": p.founding_stage, "expansion_unlocked": p.expansion_unlocked,
                    "candidate_reports_outstanding": p.survey_candidates.iter()
                        .filter(|id| !p.surveyed.contains(id)).collect::<Vec<_>>(),
                    "goal": bot.development, "space": p.home.system.storage_headroom(),
                    "expansion_target": bot.expansion,
                    "expansion_ready": bot.expansion_ready(&p),
                    "fuel_critical": p.fuel_critical(), "provisions": p.stock(C::Provisions),
                    "colony_options": p.prospects.iter().map(|s| serde_json::json!({
                        "id": s.id, "distance": p.home.system.pos.distance(s.pos),
                        "threatened": p.threatened(s.pos), "reachable": Bot::reachable_prospect(&p, s),
                    })).collect::<Vec<_>>(),
                    "population": p.home.system.population(), "workforce_share": p.home.system.staffing_share(),
                    "small": p.count(H::SmallFreighter), "medium": p.count(H::Convoy),
                    "academies": p.home.academies, "prospects": p.prospects.len(),
                    "authority_inflight": p.shipments.len(),
                    "holds": p.own().filter_map(|f| match &f.industry {
                        Some(sim::industry::FleetIndustry::Route { run }) => run.hold.as_ref(), _ => None,
                    }).collect::<Vec<_>>(),
                });
                progress["campaign"] = serde_json::json!(bot.campaign);
                progress["relations"] = serde_json::json!(p.relations);
                progress["captains"] = serde_json::json!(p.captains.iter().map(|c|
                    serde_json::json!({ "id": c.id, "level": c.level, "xp": c.xp,
                        "fleet": c.assigned_fleet })).collect::<Vec<_>>());
                progress["hull_counts"] = serde_json::json!(p.own().flat_map(|f| f.composition.iter().flatten())
                    .fold(BTreeMap::<H, u32>::new(), |mut counts, s| { *counts.entry(s.kind).or_default() += s.count; counts }));
                progress["sites"] = serde_json::json!(p.sites.iter().map(|site| serde_json::json!({
                    "id": site.system.id, "name": site.system.name, "population": site.system.population(),
                    "role": format!("{:?}", bot.role(&p.at_site(site))),
                    "food": site.system.food_state, "stock": site.system.stockpile,
                    "builds": site.builds, "bodies": site.system.bodies.iter().map(|b|
                        serde_json::json!({ "id": b.id, "structures": b.structures, "assignments": b.assignments })).collect::<Vec<_>>()
                })).collect::<Vec<_>>());
                eprintln!("progress={progress}");
                // §belts phase 6: what each kind of mining earned, at base
                // prices, per sim hour so far; and how prospecting went.
                let mining = &g.world.players[&owner].mining;
                let hours = (minute as f64 / 60.0).max(1e-9);
                let value = |book: &BTreeMap<C, f64>| book.iter().map(|(c, n)| n * sim::market::base_price(*c)).sum::<f64>();
                let stations = g.world.fleets.values().filter(|f| f.owner == owner && f.contains(H::MiningBeltStation));
                eprintln!("belts={}", serde_json::json!({
                    "seed": seed, "minute": minute, "bot": bot.slot + 1,
                    "barges": g.world.fleets.values().filter(|f| f.owner == owner).map(|f| f.count(H::MiningBarge)).sum::<u32>(),
                    "stations": stations.clone().map(|f| serde_json::json!({
                        "anchor": f.anchor.map(|a| a.phase), "hold": f.cargo_units() })).collect::<Vec<_>>(),
                    "per_hour": {
                        "barges": value(&mining.barge) / hours,
                        "belt_colonies": value(&mining.belt_colony) / hours,
                        "planet_colonies": value(&mining.planet_colony) / hours,
                        "pockets": value(&mining.pocket) / hours,
                    },
                    "units": { "barges": &mining.barge, "pockets": &mining.pocket },
                    "prospecting": { "passes": mining.prospect_passes, "found": mining.pockets_found,
                        "strikes": mining.strikes,
                        "find_rate": mining.pockets_found as f64 / mining.prospect_passes.max(1) as f64 },
                    "station_hauled": mining.station_hauled,
                }));
                // §power: every planet with a grid, and what its plants burn.
                let planets: Vec<_> = g.world.systems.iter().filter(|s| s.owner == Some(owner))
                    .flat_map(|s| s.bodies.iter().filter(|b| b.grid.demand > 0.0 || b.tier(K::PowerPlant) > 0)
                        .map(move |b| serde_json::json!({ "system": s.id, "body": b.name, "demand": b.grid.demand,
                            "supply": b.grid.supply, "power": b.grid.power, "plant": b.tier(K::PowerPlant),
                            "burn_per_h": b.grid.burn * 3600.0, "fuel": sim::power::plant_fuel(b.tier(K::PowerPlant)) })))
                    .collect();
                eprintln!("power={}", serde_json::json!({
                    "seed": seed, "minute": minute, "bot": bot.slot + 1, "planets": planets,
                    "brownout_minutes": metrics.brownout_minutes.get(&owner).copied().unwrap_or(0),
                }));
                // §fuel: the home's Fuel against its target, what is on the way,
                // and the fuel line (research, home Volatiles, refinery, bigger holds).
                let wanted = bot.desired_stock(&p, bot.project(&p));
                let researched: Vec<_> = ["prop_freight_frames", "prop_line_fusion_freight", "prop_bunkerage"].into_iter()
                    .filter(|id| p.research.has(id)).collect();
                let per_fuel = |fuel: C| serde_json::json!({
                    "stock": p.stock(fuel), "target": p.fuel_target_of(fuel),
                    "inbound": p.inbound(fuel), "carried_home": p.carried_home(fuel),
                    "lacking": bot.fuel_lacking(&p, &wanted, fuel),
                });
                eprintln!("fuel={}", serde_json::json!({
                    "seed": seed, "minute": minute, "bot": bot.slot + 1,
                    "uranium": per_fuel(C::Uranium), "deuterium": per_fuel(C::Deuterium),
                    "critical": p.fuel_critical(),
                    "critical_minutes": metrics.fuel_critical_minutes.get(&owner).copied().unwrap_or(0),
                    "hub_bound_room": p.hub_bound_room(), "mill": p.home.system.tier(K::UraniumMill),
                    "researched": researched,
                    "home_volatiles": p.home.system.bodies.iter().any(|b| b.deposits.iter().any(|d| d.resource == C::Volatiles)),
                    "harvester": p.home.system.tier(K::VolatileHarvester), "refinery": p.home.system.tier(K::FuelRefinery),
                    "freighters": p.own().filter(|f| f.kind.is_player_freighter()).map(|f| f.kind).collect::<Vec<_>>(),
                }));
                if minute == minutes {
                    if detail {
                        let project = bot.project(&p);
                        eprintln!("detail={}", serde_json::json!({
                            "seed": seed, "minute": minute, "bot": bot.slot + 1,
                            "project": project.map(|g| format!("{:?}@{}", g.what, g.body)),
                            "project_costs": project.map(|g| p.project_costs(g)),
                            "desired": bot.desired_stock(&p, project),
                            "stock": p.home.system.stockpile, "builds": p.home.builds,
                            "warehouse": p.account.as_ref().map(|a| &a.warehouse),
                            "research": p.research, "academies": p.home.academies,
                            "population": p.home.system.population(),
                            "staffing_share": p.home.system.staffing_share(),
                            "assignments": p.home.system.bodies.iter().map(|b|
                                (b.id, &b.assignments)).collect::<Vec<_>>(),
                            "shipments": p.shipments,
                            "fleets": p.own().map(|f| serde_json::json!({
                                "id": f.id, "kind": f.kind, "dock": f.docked,
                                "pos": f.pos, "vel": f.vel,
                                "cargo": f.cargo_manifest, "industry": f.industry,
                                "fuel": f.fuel, "guard": f.guard_target,
                            })).collect::<Vec<_>>(),
                        }));
                    }
                    // Why an idle freighter at home isn't sent: the route gates.
                    let wanted = bot.desired_stock(&p, bot.project(&p));
                    for fleet in p.own().filter(|g| g.kind.is_player_freighter()) {
                        let limit = p.safe_load(fleet, p.home.system.pos, p.hub);
                        let exports = p.exports(&wanted, limit);
                        let escort = p.own().find(|g| g.guard_target == Some(fleet.id));
                        let units: u32 = exports.values().sum();
                        eprintln!("freighter={} bot={} dock={:?} limit={limit} exports={exports:?} margin={:.0} exported={} cycles={} \
                            fuel={:?} leg={:.2} reserve={:.2} escort_fuel={:?} escort_leg={:.2} cash={:.0} operating_reserve={:.0}",
                            fleet.id, bot.slot + 1, fleet.docked, p.route_margin(fleet, &exports, &BTreeMap::new(), escort),
                            p.exported, bot.trade_cycles, fleet.fuel, p.leg(fleet, fleet.pos, p.hub, units), p.arrival_reserve(fleet),
                            escort.and_then(|g| g.fuel), escort.map_or(0.0, |g| p.leg(g, fleet.pos, p.hub, 0) + p.arrival_reserve(g)),
                            bot.cash(&p), p.operating_reserve());
                    }
                    // Why Authority bookings to home are refused.
                    let truth = &g.world.players[&owner];
                    eprintln!("freight_gate bot={} blockaded_believed={} blockade={:?} freight_suspended={} standing={:.1} \
                        warehouse_truth={:?} warehouse_seen={:?} credits={:.0}",
                        bot.slot + 1, g.world.believed_blockaded(p.home.system.id, g.world.hub, g.world.time),
                        g.world.systems.iter().find(|s| s.id == p.home.system.id).map(|s| &s.blockade),
                        sim::tca::charter_status(truth.tca_standing).freight_suspended(), truth.tca_standing,
                        truth.warehouse, p.account.as_ref().map(|a| &a.warehouse), truth.credits);
                    for entry in g.timeline.digest(owner).0.iter().rev().take(12) {
                        eprintln!("timeline bot={} t={:.0} {}", bot.slot + 1, entry.at_time, entry.text);
                    }
                    // Why a charter survey candidate isn't being surveyed.
                    for id in p.survey_candidates.iter().filter(|id| !p.surveyed.contains(id)) {
                        let Some(&(_, pos)) = p.chart.iter().find(|(s, _)| s == id) else { continue; };
                        let hostile = p.ghosts.iter().filter(|g| !g.own && !g.ally && !g.owner.is_tca() && g.kind.is_combatant())
                            .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)))
                            .map(|g| format!("{:?} owner={} at {:.0}", g.kind, g.owner, g.pos.distance(pos)));
                        eprintln!("candidate={id} bot={} home_distance={:.0} threatened={} avoided={} surveys={:?} nearest_hostile={hostile:?}",
                            bot.slot + 1, pos.distance(p.home.system.pos), p.threatened(pos), bot.avoided.contains_key(id),
                            bot.surveys.values().collect::<Vec<_>>());
                    }
                    for scout in p.own().filter(|f| f.kind == H::Scout) {
                        eprintln!("scout={} bot={} fuel={:?}/{:?} home_distance={:.0} speed={:.0} dock={:?} progress={:?} goal={:?} returning={:?} avoided={} threatened={}",
                            scout.id, bot.slot + 1, scout.fuel, scout.fuel_capacity, scout.pos.distance(p.home.system.pos),
                            scout.vel.length(), scout.docked, scout.survey_progress, bot.surveys.get(&scout.id),
                            bot.returns.get(&scout.id), bot.avoided.len(), p.threatened(scout.pos));
                    }
                }
            }
        }
        for (&owner, bot) in &g.bots {
            let p = g.bot_observation(owner).unwrap();
            assert!(p.account.as_ref().is_some_and(|a| a.credits.is_finite() && a.credits >= 0.0));
            assert_eq!(p.home.system.tier(K::MiningComplex) >= 1, true);
            assert!(bot.valid(owner, p.now));
            // A missing opportunity may legitimately postpone colonization or
            // research. Report those outcomes; don't grant resources to pass.
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct BalanceMetrics {
    seed: u64,
    minute: u32,
    elapsed_wall_s: f64,
    starved_minutes: BTreeMap<PlayerId, u32>,
    first_colony: BTreeMap<PlayerId, u32>,
    /// §power: minutes any of the corporation's planets spent browning out.
    #[serde(default)]
    brownout_minutes: BTreeMap<PlayerId, u32>,
    /// §fuel: minutes the home spent fuel-critical (development held).
    #[serde(default)]
    fuel_critical_minutes: BTreeMap<PlayerId, u32>,
}

impl BalanceMetrics {
    fn write(&self, dir: &std::path::Path) {
        let pending = dir.join("balance-metrics.pending.json");
        std::fs::write(&pending, serde_json::to_vec_pretty(self).unwrap()).unwrap();
        std::fs::rename(pending, dir.join("balance-metrics.json")).unwrap();
    }
}

#[test]
#[ignore = "full headless opening; run explicitly with --ignored --nocapture"]
fn bots_develop_and_physically_trade_without_bonus_resources() {
    let mut g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    let mut exported = false;
    let mut imported = false;
    let mut guarded = false;
    let mut last_commands = 0;
    // One genuine opening: normal economy, normal 30 Hz physics, NPC pirates
    // enabled. Only the test runner advances wall time quickly.
    for minute in 1..=120 {
        advance(&mut g, 60.0);
        let p = g.bot_observation(owner).unwrap();
        guarded |= p.own().any(|f| f.guard_target.is_some());
        exported |= p.own().any(|f| f.kind.is_player_freighter() && f.docked.as_deref() == Some("hub"));
        imported |= p.home.system.tier(K::Shipyard) > 0; // starter stock cannot afford it after the mine
        if minute % 15 == 0 {
            let sent = g.bots[&owner].commands_sent;
            eprintln!("bot t={minute}m commands={} (+{}) mine={} yard={} academy={} hulls={} research={:?} credits={:.0}",
                sent, sent - last_commands, p.home.system.tier(K::MiningComplex), p.home.system.tier(K::Shipyard),
                p.home.system.tier(K::Academy), p.own().count(), p.research.active,
                p.account.as_ref().map_or(0.0, |a| a.credits));
            last_commands = sent;
        }
        if exported && imported && p.home.system.tier(K::Academy) > 0 && p.count(H::Scout) >= 1
            && (p.research.active.is_some() || !p.research.completed.is_empty()) {
            break;
        }
    }
    let p = g.bot_observation(owner).unwrap();
    assert!(guarded, "the starter Interceptor receives a real persistent guard order");
    assert!(exported, "an actual freighter reaches the hub");
    assert!(imported, "market goods physically reach home and fund construction");
    assert!(p.home.system.tier(K::Academy) > 0, "the colony grows beyond its starting goods");
    // The revised policy buys an exploration capability before duplicating a
    // loss-making Tiny ore route. A normal paid Scout is the new milestone.
    assert!(p.count(H::Scout) >= 1, "trade finances an additional useful hull");
    assert!(p.research.active.is_some() || !p.research.completed.is_empty(), "the staffed Academy researches normally");
    assert!(g.world.players[&owner].founding.sale_report_at.is_some(), "an executed ore sale report reached command");
    eprintln!("bot opening passed at {:.1} sim minutes: {} commands, {} own fleets", p.now / 60.0,
        g.bots[&owner].commands_sent, p.own().count());
}
