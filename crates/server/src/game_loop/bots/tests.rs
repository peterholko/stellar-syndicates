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
    assert_eq!(g.world.systems.len(), 104);
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
        assert_eq!(corp.research.completed.len(), 0);
        let own: Vec<_> = g.world.fleets.values().filter(|f| f.owner == owner).collect();
        assert_eq!(own.len(), 2);
        assert_eq!(own.iter().map(|f| f.count(H::TinyFreighter)).sum::<u32>(), 1);
        assert_eq!(own.iter().map(|f| f.count(H::Raider)).sum::<u32>(), 1);
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
        assert!(p.own().any(|f| f.guard_target.is_some()));
    }
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
    assert!(matches!(bot.decide(&p), Some(Command::MoveShip { dest, .. }) if dest == p.home.system.pos));
}

fn planning_picture() -> (Bot, Observation) {
    let g = game(2, 1);
    let owner = PlayerId(BOT_ID_BASE);
    let mut p = g.bot_observation(owner).unwrap();
    p.account = Some(MarketAccountSample { standing: 100.0, credits: 20_000.0,
        valuation: 0.0, warehouse: BTreeMap::new(), orders: Vec::new() });
    p.ticker = Some(view::MarketTickerSample {
        prices: C::ALL.iter().map(|c| (*c, sim::market::base_price(*c))).collect(),
        available_buy: C::ALL.iter().map(|c| (*c, 1_000)).collect(),
        available_sell: C::ALL.iter().map(|c| (*c, 1_000)).collect(),
    });
    (g.bots[&owner].clone(), p)
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
    for field in ["commitments", "returns", "avoided", "route_visits", "trade_cycles", "expansion", "status"] {
        legacy.as_object_mut().unwrap().remove(field);
    }
    let old: Bot = serde_json::from_value(legacy).unwrap();
    assert!(old.commitments.is_empty());
    assert!(old.valid(p.owner, p.now));
}

#[test]
fn critical_fuel_precedes_project_materials_and_rescue_preserves_runway() {
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::Fuel, 0.0);
    let wanted = BTreeMap::from([(C::Fuel, 80.0), (C::Machinery, 100.0)]);
    assert!(matches!(bot.market(&p, &wanted), Some(Command::MarketBuy { commodity: C::Fuel, .. })));
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
fn a_tiny_ore_circuit_is_not_mistaken_for_profit_and_return_mass_counts() {
    let (_, p) = planning_picture();
    let f = p.own().find(|g| g.kind.is_player_freighter()).unwrap();
    let guard = p.own().find(|g| g.kind == H::Raider).unwrap();
    let exports = BTreeMap::from([(C::MetallicOre, 50)]);
    let margin = p.route_margin(f, &exports, &BTreeMap::new(), Some(guard));
    assert!(margin < 0.0, "basic ore is not worth a fuel-loss circuit: {margin}");
    assert!(p.leg(f, p.hub, p.home.system.pos, 50) > p.leg(f, p.hub, p.home.system.pos, 0));
    eprintln!("50-unit Tiny ore circuit: conservative margin {margin:.0} Cr at base prices (fuel + escort included)");
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
fn profitable_owned_routes_have_mixed_manifests_and_a_real_escort() {
    use sim::industry::FreightPort;
    let (mut bot, mut p) = planning_picture();
    p.home.system.stockpile.insert(C::TitaniumOre, 50.0);
    p.home.system.stockpile.insert(C::MetallicOre, 50.0);
    p.home.system.stockpile.insert(C::Fuel, 0.0);
    p.account.as_mut().unwrap().warehouse = BTreeMap::from([(C::Fuel, 10), (C::Machinery, 5)]);
    p.home.system.stockpile.insert(C::Machinery, 0.0);
    let id = p.own().find(|g| g.kind.is_player_freighter()).unwrap().id;
    for g in &mut p.ghosts {
        g.fuel = g.fuel_capacity;
        if g.kind == H::Raider { g.guard_target = Some(id); }
    }
    let wanted = BTreeMap::from([(C::Fuel, 10.0), (C::Machinery, 10.0)]);
    let cmd = bot.routes(&p, &wanted).expect("viable owned circuit");
    let Command::SetFreightRoute { route: Some(route), .. } = cmd else { panic!("expected route") };
    assert!(route.repeat && route.escort.is_some() && route.fuel_reserve > 0.0);
    let hub = route.stops.iter().find(|s| s.port == FreightPort::Hub).unwrap();
    assert_eq!(hub.load.get(&C::Fuel), Some(&10));
    assert!(hub.load.contains_key(&C::Machinery));
    assert!(hub.sell && hub.unload.contains_key(&C::TitaniumOre));
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
    assert!(bot.scouts(&p).is_none(), "return intention must not be replaced by a fresh excursion on an old view");
}

#[test]
fn expansion_requires_stability_and_uses_only_reported_prospects() {
    let (mut bot, mut p) = planning_picture();
    assert!(!bot.expansion_ready(&p), "fresh corporations do not rush colonies");
    p.exported = true;
    p.research.completed.extend(["prop_bunkerage".into(), "prop_drive_tuning".into()]);
    p.home.system.set_tier(K::Academy, 1);
    p.home.system.set_tier(K::MiningComplex, 1);
    p.home.system.stockpile.insert(C::Fuel, 500.0);
    p.home.system.stockpile.insert(C::Provisions, 50.0);
    let mut scout = p.own().next().unwrap().clone();
    scout.id = EntityId(9999); scout.kind = H::Scout;
    scout.composition = Some(vec![crate::protocol::CompCount { kind: H::Scout, count: 1 }]);
    p.ghosts.push(scout);
    let target = EntityId(9998);
    p.prospects = vec![Prospect { id: target, pos: p.cc + Vec2::new(10_000.0, 0.0), score: 5.0 }];
    assert!(bot.expansion_ready(&p));
    bot.colonies(&p);
    assert_eq!(bot.expansion, Some(target));
    p.prospects.clear();
    bot.colonies(&p);
    assert!(bot.expansion.is_none(), "a returned claim removes the stale opportunity");
}

#[test]
#[ignore = "multi-seed normal-economy balance run; release recommended"]
fn multi_seed_bot_balancing_report() {
    let minutes: u32 = std::env::var("BOT_BALANCE_MINUTES").ok().and_then(|s| s.parse().ok()).unwrap_or(180);
    let seeds = std::env::var("BOT_BALANCE_SEEDS").unwrap_or_else(|_| "12648430,42,2026".into());
    for seed in seeds.split(',').map(|s| s.parse::<u64>().unwrap()) {
        let (status, _) = watch::channel(ServerStatus::default());
        let (estimates, _) = mpsc::unbounded_channel();
        let mut g = GameLoop::new(World::new(sim::SimConfig::for_players(seed, 8)), 1.0, status, estimates);
        g.configure_bots(Some(8)).unwrap();
        for minute in 1..=minutes {
            advance(&mut g, 60.0);
            if minute % 30 != 0 && minute != minutes { continue; }
            for (&owner, bot) in &g.bots {
                let p = g.bot_observation(owner).unwrap();
                eprintln!("seed={seed} minute={minute} bot={} cash={:.0} fuel={:.1} hulls={} scouts={} surveys={} colonies={} routes={} stranded={} mine={} yard={} academy={} research={} commands={} status={}",
                    bot.slot + 1, p.account.as_ref().map_or(0.0, |a| a.credits), p.stock(C::Fuel),
                    p.own().count(), p.count(H::Scout), p.surveyed.len().saturating_sub(1), p.sites.len().saturating_sub(1),
                    p.own().filter(|g| matches!(g.industry, Some(sim::industry::FleetIndustry::Route { .. }))).count(),
                    p.own().filter(|g| g.stalled).count(), p.home.system.tier(K::MiningComplex), p.home.system.tier(K::Shipyard),
                    p.home.system.tier(K::Academy), p.research.completed.len(), bot.commands_sent, bot.status);
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
