//! Headless corporations, not a second simulation. Decisions see arrived reports
//! only and enqueue the same Commands as a socket. No truth positions, free goods,
//! instant orders, forced research or special combat advantages enter the policy.

use super::*;
use crate::protocol::GhostView;
use serde::{Deserialize, Serialize};
use sim::{BuildKind, Commodity as C, EntityId, ShipKind as H, StructureKind as K, Vec2};
use std::collections::BTreeSet;

// Accounts use positive PostgreSQL i64 ids. This separate, non-sentinel namespace
// cannot collide with an authenticated account or impersonate an existing player.
const BOT_ID_BASE: u64 = 0xB070_0000_0000_0000;
const THINK_SECONDS: f64 = 4.0;
const NAMES: [&str; 8] = [
    "Atlas Freight", "Cinder Industries", "Pioneer Survey", "Meridian Metals",
    "Orion Exchange", "Helix Foundry", "Far Horizon", "Vesper Resources",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Bot {
    slot: u32,
    next_think: f64,
    /// Wait for round-trip evidence before retrying a target. These are our own
    /// issued intentions, not reads of the authoritative pending-order queue.
    waits: BTreeMap<String, f64>,
    surveys: BTreeMap<EntityId, EntityId>,
    pub(super) commands_sent: u64,
}

impl Bot {
    pub(super) fn valid(&self, owner: PlayerId, now: f64) -> bool {
        owner == PlayerId(BOT_ID_BASE + self.slot as u64)
            && self.next_think.is_finite() && self.next_think >= 0.0
            && now.is_finite() && self.waits.values().all(|t| t.is_finite())
    }
}

struct Observation {
    owner: PlayerId,
    now: f64,
    cc: Vec2,
    hub: Vec2,
    c: f64,
    home: sim::information::SiteReport,
    ghosts: Vec<GhostView>,
    fighting: BTreeSet<EntityId>,
    account: Option<MarketAccountSample>,
    ticker: Option<view::MarketTickerSample>,
    research: sim::research::ResearchState,
    available_research: Vec<String>,
    surveyed: BTreeSet<EntityId>,
    shipments: Vec<sim::tca::Shipment>,
    /// The public chart contains positions, NOT unexplored resources/ownership.
    chart: Vec<(EntityId, Vec2)>,
}

#[derive(Clone, Copy)]
struct Project {
    what: BuildKind,
    body: u32,
}

impl GameLoop {
    /// Explicit configuration applies only to a fresh galaxy. Resuming without
    /// BOT_PLAYERS restores the saved roster; it never re-joins or duplicates it.
    pub(super) fn configure_bots(&mut self, requested: Option<u32>) -> anyhow::Result<()> {
        let Some(count) = requested else { return Ok(()); };
        if !self.bots.is_empty() {
            anyhow::ensure!(self.bots.len() == count as usize,
                "saved galaxy has {} bots; use --reset-galaxy to change BOT_PLAYERS", self.bots.len());
            return Ok(());
        }
        if count == 0 { return Ok(()); }
        anyhow::ensure!(count <= self.world.config.max_players,
            "BOT_PLAYERS must not exceed MAX_PLAYERS in a fresh galaxy");
        anyhow::ensure!(self.world.players.is_empty(),
            "adding bots requires a fresh galaxy; archive it with --reset-galaxy");
        for slot in 0..count {
            let owner = PlayerId(BOT_ID_BASE + slot as u64);
            self.bots.insert(owner, Bot {
                slot, next_think: self.world.time + 1.0 + slot as f64 * 0.43,
                waits: BTreeMap::new(), surveys: BTreeMap::new(), commands_sent: 0,
            });
            self.pending.push(Command::AddPlayer {
                id: owner,
                name: format!("{} [Bot {}]", NAMES[slot as usize % NAMES.len()], slot + 1),
            });
        }
        // The ordinary join grants the ordinary 2,000-population start, starter
        // hulls and goods. Record that tick before the initial durable checkpoint.
        self.tick();
        info!(bots = count, systems = self.world.systems.len(), "bot corporations joined");
        Ok(())
    }

    fn bot_observation(&self, owner: PlayerId) -> Option<Observation> {
        let corp = self.world.players.get(&owner)?;
        let now = self.world.time;
        let c = self.world.config.c;
        let cc = corp.command_center;
        let home_id = corp.home_system?;
        let home = self.world.information.owned_sites(owner, cc, c, now)
            .find(|report| report.system.id == home_id)?.clone();
        let mut fighting = BTreeSet::new();
        for battle in self.world.active_battles() {
            let delay = sim::transit::delay(battle.pos, cc, c);
            if now >= battle.started_at + delay {
                fighting.extend(view::battle_participants_at(
                    self.world.battle_records.get(&battle.id), &battle.participants, now, delay));
            }
        }
        for battle in &self.concluded_battles {
            if battle.shows_in_progress(cc, c, now) {
                fighting.extend(view::battle_participants_at(
                    self.world.battle_records.get(&battle.id), &battle.participants,
                    now, sim::transit::delay(battle.pos, cc, c)));
            }
        }
        let (veil, deep_scan) = self.world.known_node_regions(owner);
        let mut ghosts = self.history.view_for_with_arrays(owner, cc, c, now,
            &self.world.known_sensor_sources(owner), &fighting,
            view::NodeEffects { veil: &veil, deep_scan: &deep_scan });
        ghosts.sort_by_key(|g| g.id);
        let market_time = now - sim::transit::delay(self.world.hub, cc, c);
        Some(Observation {
            owner, now, cc, hub: self.world.hub, c, home, ghosts, fighting,
            account: self.market_accounts.at(owner, market_time).cloned(),
            ticker: self.prices.at(market_time).cloned(),
            // Completed research and arrived survey knowledge are CC ledgers,
            // exactly like the human research panel; metrics use arrived sites.
            research: corp.research.clone(),
            available_research: sim::research::visible_ids().filter(|id|
                sim::research::is_available(id, &corp.research,
                    &|m| self.world.corporation_metric(owner, m), now))
                .map(str::to_string).collect(),
            surveyed: corp.surveyed.clone(),
            shipments: self.world.information.shipments(owner, cc, c, now).into_iter().map(|(s, _)| s).collect(),
            chart: self.world.systems.iter().map(|s| (s.id, s.pos)).collect(),
        })
    }

    pub(super) fn think_bots(&mut self) {
        let due: Vec<_> = self.bots.iter().filter(|(_, bot)| bot.next_think <= self.world.time)
            .map(|(id, _)| *id).collect();
        for owner in due {
            let picture = self.bot_observation(owner);
            let bot = self.bots.get_mut(&owner).unwrap();
            bot.next_think = self.world.time + THINK_SECONDS + (bot.slot % 3) as f64 * 0.37;
            let Some(picture) = picture else { continue; };
            if let Some(command) = bot.decide(&picture) {
                debug!(%owner, ?command, "bot issued command");
                bot.commands_sent += 1;
                self.pending.push(command);
            }
            // An offline bot consumes the same compliance evidence as a human
            // View. No independent timer can confirm an unseen outcome.
            let evidence: Vec<_> = picture.ghosts.iter().filter(|g| g.own)
                .map(|g| (g.id, picture.now - g.age, g.jumped)).collect();
            let events = self.world.confirm_orders_from_served(owner, &evidence);
            self.timeline.ingest(&events, &self.world);
        }
    }
}

impl Observation {
    fn own(&self) -> impl Iterator<Item = &GhostView> {
        self.ghosts.iter().filter(|g| g.own && !g.migrant && !g.rescue_service)
    }

    fn count(&self, kind: H) -> u32 {
        self.own().flat_map(|g| g.composition.iter().flatten())
            .filter(|s| s.kind == kind).map(|s| s.count).sum::<u32>()
            + self.home.builds.iter().filter(|b| b.what == BuildKind::Ship { ship: kind }).count() as u32
    }

    fn round_trip(&self, pos: Vec2) -> f64 {
        2.0 * sim::transit::delay(self.cc, pos, self.c) + THINK_SECONDS * 2.0
    }

    /// Plan from public geometry and reported tanks. Cargo has mass too; the
    /// empty-hull estimate can strand a fully loaded Tiny on its opening run.
    /// Merge well intersections so overlapping public wells aren't charged twice.
    fn fuel_per_mass(&self, from: Vec2, to: Vec2) -> f64 {
        let distance = from.distance(to);
        if distance < 1.0 { return 0.0; }
        let direction = (to - from) * (1.0 / distance);
        let radius = sim::transit::HYPERLIMIT;
        let mut intervals = Vec::new();
        for (_, center) in &self.chart {
            let offset = *center - from;
            let along = offset.dot(direction);
            let across_sq = (offset.length_sq() - along * along).max(0.0);
            if across_sq < radius * radius {
                let half = (radius * radius - across_sq).sqrt();
                let start = (along - half).max(0.0);
                let end = (along + half).min(distance);
                if end > start { intervals.push((start, end)); }
            }
        }
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut inside = 0.0;
        let mut end: f64 = 0.0;
        for &(a, b) in &intervals {
            inside += (b - a.max(end)).max(0.0);
            end = end.max(b);
        }
        let engine_distance = (distance - inside) / sim::transit::WARP_FACTOR + inside
            + 250.0 * (1 + intervals.len()) as f64;
        sim::fuel::fuel_cost(engine_distance, 1.0)
            * sim::research::mod_of(&self.research, sim::research::ModKey::FuelConsumption) * 1.08
    }

    fn freight_limit(&self, fleet: &GhostView, dest: Vec2) -> u32 {
        let per_mass = self.fuel_per_mass(fleet.pos, dest);
        if per_mass <= 0.0 { return capacity(fleet); }
        let mass_budget = fleet.fuel.unwrap_or(0.0) / per_mass;
        (((mass_budget - hull_mass(fleet)).max(0.0) / sim::ship::CARGO_MASS_PER_UNIT).floor() as u32)
            .min(capacity(fleet))
    }

    fn enough_fuel(&self, fleet: &GhostView, dest: Vec2) -> bool {
        let mass = hull_mass(fleet) + cargo_used(fleet) as f64 * sim::ship::CARGO_MASS_PER_UNIT;
        fleet.fuel.is_some_and(|fuel| fuel >= self.fuel_per_mass(fleet.pos, dest) * mass)
    }

    fn stock(&self, good: C) -> f64 {
        self.home.system.free_stock(good)
            + self.own().filter(|g| g.docked.as_deref() == Some(self.home.system.id.to_string().as_str()))
                .map(|g| cargo(g, good) as f64).sum::<f64>()
    }

    fn inbound(&self, good: C) -> u32 {
        self.shipments.iter().filter(|s| s.system == self.home.system.id
            && s.direction == sim::tca::ShipmentDir::Outbound && s.commodity == good)
            .map(|s| s.units).sum()
    }

    fn export_reserve(&self, good: C, wanted: &BTreeMap<C, f64>) -> Option<f64> {
        let refinery_product = self.home.system.tier(K::Smelter) > 0
            && matches!(good, C::Alloys | C::ConductiveMetals | C::Titanium | C::Silicates | C::RareElements);
        if !good.is_ore() && good != C::Provisions && !refinery_product { return None; }
        let reserve = wanted.get(&good).copied().unwrap_or(0.0);
        Some(if good.is_ore() && self.home.system.tier(K::Smelter) > 0 || refinery_product {
            reserve.max(20.0)
        } else { reserve })
    }

    fn project_costs(&self, project: Project) -> BTreeMap<C, f64> {
        let mult = match project.what {
            BuildKind::Upgrade { .. } if self.home.system.trait_ == Some(sim::explore::SystemTrait::UnstableGeology)
                => sim::explore::UNSTABLE_COST_MULT,
            BuildKind::Ship { ship } if ship.is_combatant() =>
                sim::research::mod_of(&self.research, sim::research::ModKey::WarshipCost),
            _ => 1.0,
        };
        sim::build::recipe_for(project.what).costs.iter().map(|(c, n)| (*c, n * mult)).collect()
    }

    fn structure(&self, kind: K, tier: u32) -> Option<Project> {
        let system = &self.home.system;
        if system.tier(kind) >= tier || self.home.builds.iter()
            .any(|b| b.what == BuildKind::Upgrade { upgrade: kind }) { return None; }
        let cap = sim::build::max_buildable_tier(kind, sim::research::unlocked_structure_tier(&self.research, kind));
        if tier > cap { return None; }
        // Prefer the natural deposit/inhabited site, but use another legal body
        // rather than repeatedly ordering construction into a full slot pool.
        let preferred = system.site_for(kind);
        let body = system.bodies.iter().filter(|b| {
            let extraction = matches!(kind, K::MiningComplex | K::Bioharvester | K::VolatileHarvester);
            (!extraction || b.has_deposit_for(kind)) && (b.tier(kind) > 0
                || b.pool_slots_built(kind.slot_pool()) < b.pool_slots(kind.slot_pool()))
        }).max_by_key(|b| (b.tier(kind), Some(b.id) == preferred))?;
        Some(Project { what: BuildKind::Upgrade { upgrade: kind }, body: body.id })
    }

    fn ship(&self, kind: H, desired: u32) -> Option<Project> {
        if self.count(kind) >= desired || (sim::ship::requires_hull_unlock(kind)
            && !sim::research::has_hull(&self.research, kind)) { return None; }
        let (yard, tier) = sim::build::yard_for(kind);
        let body = self.home.system.bodies.iter().max_by_key(|b| b.tier(yard))?;
        if body.tier(yard) < tier || self.home.builds.iter().any(|b| matches!(b.what, BuildKind::Ship { .. })) {
            return None;
        }
        Some(Project { what: BuildKind::Ship { ship: kind }, body: body.id })
    }
}

fn cargo(ghost: &GhostView, good: C) -> u32 {
    ghost.cargo_manifest.iter().filter(|c| c.commodity == good).map(|c| c.units).sum()
}

fn cargo_used(ghost: &GhostView) -> u32 {
    ghost.cargo_manifest.iter().map(|c| c.units).sum()
}

fn capacity(ghost: &GhostView) -> u32 {
    ghost.composition.iter().flatten().map(|s| s.kind.cargo_units() * s.count).sum()
}

fn hull_mass(ghost: &GhostView) -> f64 {
    ghost.composition.iter().flatten().map(|s| s.kind.hull_mass() * s.count as f64).sum()
}

impl Bot {
    fn ready(&self, key: &str, now: f64) -> bool {
        self.waits.get(key).is_none_or(|until| *until <= now)
    }

    fn wait(&mut self, key: String, picture: &Observation, pos: Vec2) {
        self.waits.insert(key, picture.now + picture.round_trip(pos));
    }

    fn project(&self, p: &Observation) -> Option<Project> {
        // Distinct opening priorities, shared legal recipes. A lost freighter
        // is replaced before optional expansion; no loss earns a free hull.
        p.structure(K::MiningComplex, 1)
            .or_else(|| p.structure(K::Shipyard, 1))
            .or_else(|| p.ship(H::TinyFreighter, 1))
            .or_else(|| (self.slot % 4 == 0).then(|| p.ship(H::TinyFreighter, 2)).flatten())
            .or_else(|| p.structure(K::Academy, 1))
            .or_else(|| (self.slot % 4 == 2).then(|| p.ship(H::Scout, 1)).flatten())
            .or_else(|| p.ship(H::TinyFreighter, 2))
            .or_else(|| p.ship(H::Scout, 1))
            .or_else(|| p.structure(K::Smelter, 1))
            .or_else(|| p.structure(K::Shipyard, 2))
            .or_else(|| p.ship(H::Raider, 2))
            .or_else(|| p.ship(H::SmallFreighter, 1))
            .or_else(|| p.structure(K::Warehouse, 2))
            .or_else(|| p.structure(K::MiningComplex, 2))
            .or_else(|| p.structure(K::Habitat, 2))
    }

    fn desired_stock(&self, p: &Observation, project: Option<Project>) -> BTreeMap<C, f64> {
        let mut wanted = project.map(|project| p.project_costs(project)).unwrap_or_default();
        // Keep food and a refuelling runway, then buy only the next project's
        // shortfall. The actual goods must come home in a real mixed hold.
        // Low/high water marks prevent a trickle of consumption monopolising
        // every market decision with one-unit top-ups ahead of build materials.
        wanted.insert(C::Fuel, if p.stock(C::Fuel) < 40.0 { 100.0 } else { 40.0 });
        wanted.insert(C::Provisions, if p.stock(C::Provisions) < 10.0 { 40.0 } else { 20.0 });
        if p.home.system.tier(K::Academy) > 0 {
            if let Some(programme) = p.research.active.as_deref().and_then(sim::research::programme) {
                for (good, rate) in sim::research::basket(programme.field, programme.tier) {
                    *wanted.entry(good).or_default() += (rate * 600.0).ceil();
                }
            }
        }
        wanted
    }

    fn decide(&mut self, p: &Observation) -> Option<Command> {
        self.waits.retain(|_, until| *until > p.now);
        self.surveys.retain(|fleet, _| p.own().any(|g| g.id == *fleet));
        let player_id = p.owner;
        let system_id = p.home.system.id;
        let home = p.home.system.pos;
        // Rescue any reported stranded hull, including its escort; the normal
        // paid Authority service travels physically and may itself be delayed.
        for fleet in p.own().filter(|g| g.stalled && !g.rescue_inbound) {
            let key = format!("fleet:{}", fleet.id);
            if self.ready(&key, p.now) && self.ready("market", p.now)
                && p.account.as_ref().is_some_and(|a| a.credits > 2_500.0) {
                self.wait(key, p, fleet.pos);
                self.wait("market".into(), p, p.hub);
                return Some(Command::RequestFuelRescue { player_id, fleet_id: fleet.id });
            }
        }
        // Restore staffing after construction. Spread the initial two workforce
        // across food/extraction as the normal shared-workforce rules allow.
        // Idle shipyards release their worker instead of permanently diluting mines.
        if self.ready("staff", p.now) {
            for body in &p.home.system.bodies {
                for (&kind, &tier) in &body.structures {
                    let wanted = match kind {
                        K::MiningComplex | K::Bioharvester | K::Agroplex | K::Academy | K::Smelter => 1,
                        K::Shipyard => u32::from(p.home.builds.iter().any(|job|
                            job.body_id == body.id && matches!(job.what, BuildKind::Ship { .. }))),
                        _ => continue,
                    };
                    let assigned = body.assignments.get(&kind).map_or(0, |a| a.workers);
                    if tier > 0 && assigned != wanted {
                        self.wait("staff".into(), p, home);
                        return Some(Command::SetAssignment { player_id, system_id, structure: kind,
                            workers: wanted, body_id: Some(body.id), specialists: BTreeMap::new(), refining_ore: None });
                    }
                }
            }
        }

        // Guard orders are persistent. Confirm the escort assignment in the
        // delayed picture BEFORE sending the first laden freighter into danger.
        let freighters: Vec<_> = p.own().filter(|g| g.kind.is_player_freighter()).collect();
        let guards: Vec<_> = p.own().filter(|g| g.kind == H::Raider).collect();
        for (i, guard) in guards.iter().enumerate() {
            let key = format!("fleet:{}", guard.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&guard.id) || guard.stalled { continue; }
            if let Some(target) = freighters.get(i) {
                if guard.guard_target != Some(target.id) {
                    self.wait(key, p, guard.pos);
                    return Some(Command::GuardFleet { player_id, interceptor_id: guard.id, target_id: target.id });
                }
            } else if guard.defend_system.is_none() {
                self.wait(key, p, guard.pos);
                return Some(Command::DefendSystem { player_id, fleet_id: guard.id, system_id, pursuit_radius: 20_000.0 });
            }
        }

        let project = self.project(p);
        if self.ready("build", p.now) && p.home.builds.is_empty() {
            if let Some(project) = project {
                if p.project_costs(project).iter().all(|(c, need)| p.stock(*c) + 1e-9 >= *need) {
                    self.wait("build".into(), p, home);
                    return Some(match project.what {
                        BuildKind::Upgrade { upgrade } => Command::DevelopSystem {
                            player_id, system_id, upgrade, body_id: Some(project.body) },
                        BuildKind::Ship { ship } => Command::BuildShip {
                            player_id, system_id, ship_kind: ship, join: None, loadout: Default::default() },
                        _ => unreachable!(),
                    });
                }
            }
        }
        if self.ready("research", p.now) && p.home.system.tier(K::Academy) > 0
            && p.research.active.is_none() && p.research.queue.is_empty() {
            let first = match self.slot % 4 {
                0 => "prop_freight_frames", 1 => "mat_enrichment",
                2 => "prop_drive_tuning", _ => "mat_ore_recovery",
            };
            let priorities = [first, "mat_enrichment", "prop_freight_frames", "mat_ore_recovery",
                "prop_bunkerage", "prop_efficient_burns", "comp_sensor_gain", "weap_fire_control"];
            if let Some(id) = priorities.iter().find(|id| p.available_research.iter().any(|s| s == **id)) {
                self.wait("research".into(), p, home);
                return Some(Command::SetResearchQueue { player_id, queue: vec![(*id).into()] });
            }
        }

        let wanted = self.desired_stock(p, project);
        // Reserve known cargo already on its way home before placing another
        // buy. One market decision at a time prevents overspending a stale wallet.
        if self.ready("market", p.now) {
            if let (Some(account), Some(ticker)) = (&p.account, &p.ticker) {
                let budget = (account.credits - 300.0).max(0.0);
                // A Tiny cannot carry all colony production. Book a normal,
                // paid common carrier for surplus rather than letting a full
                // stockpile deadlock returning imports. Keep a load for our hull.
                for (&commodity, &stock) in &p.home.system.stockpile {
                    let Some(reserve) = p.export_reserve(commodity, &wanted) else { continue; };
                    let units = (stock - 100.0 - reserve)
                        .floor().max(0.0).min(300.0) as u32;
                    let price = ticker.prices.get(&commodity).copied().unwrap_or(0.0);
                    if units >= 100 && budget >= sim::tca::freight_fee(units, price, home.distance(p.hub)) * 1.2 {
                        self.wait("market".into(), p, p.hub);
                        return Some(Command::BookFreightIn { player_id, system: system_id,
                            commodity, units, sell_on_arrival: true });
                    }
                }
                let project_goods = project.map(|goal| p.project_costs(goal)).unwrap_or_default();
                let mut purchases: Vec<_> = wanted.iter().collect();
                purchases.sort_by_key(|(good, _)| (!project_goods.contains_key(good), **good));
                for (&good, &target) in purchases {
                    let in_transit: u32 = freighters.iter().filter(|g|
                        g.docked.as_deref() != Some(system_id.to_string().as_str())).map(|g| cargo(g, good)).sum();
                    let stored = account.warehouse.get(&good).copied().unwrap_or(0);
                    let needed = (target - p.stock(good) - in_transit as f64 - p.inbound(good) as f64)
                        .ceil().max(0.0) as u32;
                    let missing = needed.saturating_sub(stored);
                    let Some(&price) = ticker.prices.get(&good) else { continue; };
                    // Charter essential imports when our own hull is away. This
                    // pays the normal freight fee, queues a real raidable carrier,
                    // and counts only shipments already reported to the CC.
                    let booked = needed.min(stored).min(100);
                    if booked > 0 && !freighters.iter().any(|g| g.docked.as_deref() == Some("hub"))
                        && budget >= sim::tca::freight_fee(booked, price, home.distance(p.hub)) * 1.2 {
                        self.wait("market".into(), p, p.hub);
                        return Some(Command::BookFreightOut { player_id, system: system_id, commodity: good, units: booked });
                    }
                    let ceiling = price * 1.20;
                    let units = missing.min(50).min(ticker.available_buy.get(&good).copied().unwrap_or(0))
                        .min((budget / (ceiling * 1.1)).floor() as u32);
                    if units > 0 {
                        self.wait("market".into(), p, p.hub);
                        return Some(Command::MarketBuy { player_id, commodity: good, units, max_unit_price: Some(ceiling) });
                    }
                }
            }
        }

        for fleet in &freighters {
            let key = format!("fleet:{}", fleet.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&fleet.id) { continue; }
            if fleet.stalled { continue; }
            let used = cargo_used(fleet);
            let dest = if fleet.docked.as_deref() == Some("hub") { home } else { p.hub };
            let load_limit = p.freight_limit(fleet, dest);
            let room = load_limit.saturating_sub(used);
            if fleet.docked.as_deref() == Some(system_id.to_string().as_str()) {
                // Failed automatic unload (full stockpile): retain useful goods
                // for construction from the docked hold; never sell the imports.
                if fleet.cargo_manifest.iter().any(|c| !c.commodity.is_ore() && c.commodity != C::Provisions) {
                    self.wait(key, p, fleet.pos);
                    return Some(Command::SystemUnload { player_id, fleet_id: fleet.id, system: system_id });
                }
                // Own haulers keep the opening raw-goods circuit; refined
                // surplus uses the paid carrier above. A returned build-material
                // manifest must never be mistaken for cargo to auto-sell.
                let export = p.home.system.stockpile.keys().filter(|good| good.is_ore() || **good == C::Provisions).filter_map(|good|
                    p.export_reserve(*good, &wanted).map(|reserve|
                        (*good, (p.home.system.free_stock(*good) - reserve).floor().max(0.0) as u32)))
                    .filter(|(_, n)| *n > 0).max_by(|(a, an), (b, bn)| {
                        let value = |good, n: u32| p.ticker.as_ref().and_then(|m| m.prices.get(&good))
                            .copied().unwrap_or(0.0) * n.min(room) as f64;
                        value(*a, *an).total_cmp(&value(*b, *bn)).then(an.cmp(bn))
                    });
                if room > 0 {
                    if let Some((commodity, available)) = export {
                        self.wait(key, p, fleet.pos);
                        return Some(Command::SystemLoad { player_id, fleet_id: fleet.id, system: system_id,
                            commodity, units: room.min(available) });
                    }
                }
                // Do not burn a whole trip for one unit just because a nearly
                // empty tank can carry it. Wait for the booked fuel shipment.
                if used >= (load_limit / 2).max(10) {
                    if freighters.first().is_some_and(|g| g.id == fleet.id) && !guards.is_empty()
                        && !guards.iter().any(|g| g.guard_target == Some(fleet.id)) { continue; }
                    if !p.enough_fuel(fleet, p.hub) { continue; }
                    self.wait(key, p, fleet.pos);
                    return Some(Command::HaulToMarketHub { player_id, fleet_id: fleet.id, sell_on_arrival: true });
                }
            } else if fleet.docked.as_deref() == Some("hub") {
                if let Some(account) = &p.account {
                    if room > 0 {
                        // The build recipe wins over optional reserve top-ups;
                        // arriving Authority supplies are already accounted for.
                        let mut goods: Vec<_> = wanted.iter().collect();
                        let project_goods = project.map(|goal| p.project_costs(goal)).unwrap_or_default();
                        goods.sort_by_key(|(good, _)| (!project_goods.contains_key(good), **good));
                        for (&commodity, &target) in goods {
                            let carried: u32 = freighters.iter().map(|g| cargo(g, commodity)).sum();
                            let needed = (target - p.home.system.free_stock(commodity) - carried as f64 - p.inbound(commodity) as f64)
                                .ceil().max(0.0) as u32;
                            let units = room.min(needed).min(account.warehouse.get(&commodity).copied().unwrap_or(0));
                            if units > 0 {
                                self.wait(key, p, p.hub);
                                return Some(Command::HubLoad { player_id, fleet_id: fleet.id, commodity, units });
                            }
                        }
                    }
                    if p.enough_fuel(fleet, home) {
                        self.wait(key, p, p.hub);
                        // HaulToSystem deliberately requires a nonempty hold.
                        // A ballast return is an ordinary move, not a rejected haul.
                        if used == 0 {
                            return Some(Command::MoveShip { player_id, ship_id: fleet.id, dest: home });
                        }
                        return Some(Command::HaulToSystem { player_id, fleet_id: fleet.id, system: system_id });
                    }
                }
            }
        }

        for scout in p.own().filter(|g| g.kind == H::Scout) {
            let key = format!("fleet:{}", scout.id);
            if !self.ready(&key, p.now) || p.fighting.contains(&scout.id) { continue; }
            let threatened = p.ghosts.iter().any(|g| !g.own && !g.owner.is_tca()
                && g.kind.is_combatant() && (g.pos - scout.pos).length() < 25_000.0);
            if threatened || scout.damage.unwrap_or(0.0) > 0.35 {
                if (scout.pos - home).length() > 1_000.0 {
                    self.wait(key, p, scout.pos);
                    self.surveys.remove(&scout.id);
                    return Some(Command::MoveShip { player_id, ship_id: scout.id, dest: home });
                }
                continue;
            }
            if let Some(target) = self.surveys.get(&scout.id) {
                if !p.surveyed.contains(target) && (scout.vel.length_sq() > 1.0 || scout.survey_progress.is_some()) { continue; }
                self.surveys.remove(&scout.id);
            }
            let target = p.chart.iter().filter(|(id, pos)| !p.surveyed.contains(id)
                && !self.surveys.values().any(|target| target == id)
                && p.enough_fuel(scout, *pos)
                && scout.fuel.unwrap_or(0.0) >= hull_mass(scout)
                    * (p.fuel_per_mass(scout.pos, *pos) + p.fuel_per_mass(*pos, home)))
                .min_by(|(_, a), (_, b)| ((*a - scout.pos).length()).total_cmp(&(*b - scout.pos).length()));
            if let Some(&(system_id, _)) = target {
                self.wait(key, p, scout.pos);
                self.surveys.insert(scout.id, system_id);
                return Some(Command::SurveySystem { player_id, fleet_id: scout.id, system_id });
            } else if scout.docked.is_none() && p.enough_fuel(scout, home) {
                self.wait(key, p, scout.pos);
                return Some(Command::MoveShip { player_id, ship_id: scout.id, dest: home });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
