//! Headless corporations, not a second simulation. Decisions see arrived reports
//! only and enqueue the same Commands as a socket. No truth positions, free goods,
//! instant orders, forced research or special combat advantages enter the policy.

use super::*;
use crate::protocol::GhostView;
use serde::{Deserialize, Serialize};
use sim::{BuildKind, Commodity as C, EntityId, ShipKind as H, StructureKind as K, Vec2};
use std::collections::BTreeSet;

mod planning;
mod logistics;
mod expeditions;
mod development;
mod strategy;
mod warfare;
mod networks;
mod opening;
mod belts;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Commitment { amount: f64, receiver_at: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReturnTrip { destination: Vec2, issued_at: f64 }

// Accounts use positive PostgreSQL i64 ids. This separate, non-sentinel namespace
// cannot collide with an authenticated account or impersonate an existing player.
const BOT_ID_BASE: u64 = 0xB070_0000_0000_0000;
/// §fuel-overhaul: the two fuels a bot keeps stocked.
const FUELS: [C; 2] = [C::Uranium, C::Deuterium];
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
    #[serde(default)]
    survey_started: BTreeSet<EntityId>,
    pub(super) commands_sent: u64,
    #[serde(default)]
    commitments: Vec<Commitment>,
    #[serde(default)]
    returns: BTreeMap<EntityId, ReturnTrip>,
    #[serde(default)]
    avoided: BTreeMap<EntityId, f64>,
    #[serde(default)]
    route_visits: BTreeMap<EntityId, u32>,
    #[serde(default)]
    trade_cycles: u64,
    #[serde(default)]
    expansion: Option<EntityId>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    development: Option<DevelopmentGoal>,
    #[serde(default)]
    campaign: Option<warfare::Campaign>,
    #[serde(default)]
    freight_homes: BTreeMap<EntityId, EntityId>,
    #[serde(default)]
    economy_turn: usize,
    #[serde(default)]
    supply_dispatches: BTreeMap<EntityId, networks::SupplyDispatch>,
    #[serde(default)]
    reconnaissance: BTreeMap<EntityId, expeditions::Reconnaissance>,
    /// §belts phase 6: freighters running a station's ore home (freighter → station).
    #[serde(default)]
    belt_haulers: BTreeMap<EntityId, EntityId>,
}

impl Bot {
    pub(super) fn valid(&self, owner: PlayerId, now: f64) -> bool {
        owner == PlayerId(BOT_ID_BASE + self.slot as u64)
            && self.next_think.is_finite() && self.next_think >= 0.0
            && now.is_finite() && self.waits.values().all(|t| t.is_finite())
            && self.commitments.iter().all(|c| c.amount.is_finite() && c.amount >= 0.0 && c.receiver_at.is_finite())
            && self.returns.values().all(|r| r.destination.x.is_finite() && r.destination.y.is_finite() && r.issued_at.is_finite())
            && self.avoided.values().all(|t| t.is_finite())
            && self.development.as_ref().is_none_or(|g| g.since.is_finite() && g.target > 0)
            && self.campaign.as_ref().is_none_or(|c| c.since.is_finite() && c.deadline.is_finite() && c.deadline >= c.since)
            && self.supply_dispatches.values().all(|s| s.until.is_finite())
            && self.reconnaissance.values().all(|r| r.since.is_finite() && r.deadline.is_finite()
                && r.point.x.is_finite() && r.point.y.is_finite())
    }
}

#[derive(Clone)]
struct Observation {
    owner: PlayerId,
    capital: EntityId,
    now: f64,
    cc: Vec2,
    hub: Vec2,
    c: f64,
    siege_secs: f64,
    home: sim::information::SiteReport,
    sites: Vec<sim::information::SiteReport>,
    prospects: Vec<Prospect>,
    ghosts: Vec<GhostView>,
    fighting: BTreeSet<EntityId>,
    account: Option<MarketAccountSample>,
    market_at: f64,
    exported: bool,
    founding_stage: sim::FoundingStage,
    local_opening: bool,
    opening_recovered: bool,
    recovery_site: Option<sim::sites::SiteReport>,
    expansion_unlocked: bool,
    survey_candidates: Vec<EntityId>,
    /// §belts: during the charter's barge lesson, the charted belt it points
    /// at, the ore worth cutting there and the system's position.
    belt_lesson: Option<(EntityId, u32, C, Vec2)>,
    /// §belts phase 6: every charted belt, from arrived reports, with the
    /// pockets this corporation has found there.
    belts: Vec<belts::BeltSite>,
    ticker: Option<view::MarketTickerSample>,
    research: sim::research::ResearchState,
    available_research: Vec<String>,
    surveyed: BTreeSet<EntityId>,
    shipments: Vec<sim::tca::Shipment>,
    /// The public chart contains positions, NOT unexplored resources/ownership.
    chart: Vec<(EntityId, Vec2)>,
    /// Scout intelligence and diplomatic notices already received at command.
    military_sites: Vec<warfare::MilitarySite>,
    relations: BTreeMap<PlayerId, sim::diplomacy::RelationState>,
    captains: Vec<sim::Captain>,
}

#[derive(Clone)]
struct Prospect { id: EntityId, pos: Vec2, score: f64 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Project {
    what: BuildKind,
    body: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DevelopmentGoal { project: Project, target: u32, since: f64 }

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
                waits: BTreeMap::new(), surveys: BTreeMap::new(), survey_started: BTreeSet::new(), commands_sent: 0,
                commitments: Vec::new(), returns: BTreeMap::new(), avoided: BTreeMap::new(),
                route_visits: BTreeMap::new(), trade_cycles: 0, expansion: None, status: String::new(), development: None,
                campaign: None, freight_homes: BTreeMap::new(), economy_turn: 0, supply_dispatches: BTreeMap::new(),
                reconnaissance: BTreeMap::new(), belt_haulers: BTreeMap::new(),
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
        let sites: Vec<_> = self.world.information.owned_sites(owner, cc, c, now).cloned().collect();
        let home = sites.iter().find(|report| report.system.id == home_id)?.clone();
        // Only geology already surveyed by this corporation may inform a colony
        // choice. Ownership is also an ARRIVED report, never current world truth.
        let intel = self.world.information.intel(owner, cc, c, now);
        let prospects = corp.surveyed.iter().filter_map(|id| {
            let report = self.world.information.site(*id, cc, c, now)?;
            let s = &report.system;
            if s.owner.is_some() || !s.bodies.iter().any(|b| b.habitable) { return None; }
            // Unclaimed is not undefended. A reported pirate base needs a
            // military operation, not a civilian hull parked there forever.
            // A later ARRIVED clearance report may make it a candidate again.
            if intel.get(id).is_some_and(|s| s.enclave_tier > 0) { return None; }
            let score = s.bodies.iter().flat_map(|b| &b.deposits).map(|d|
                d.richness * if d.resource == C::Volatiles { 2.0 } else if d.resource.is_ore() { 1.5 } else { 1.0 })
                .sum::<f64>();
            Some(Prospect { id: *id, pos: s.pos, score })
        }).collect();
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
        let military_sites = intel.iter().filter_map(|(&id, intel)| {
            let report = self.world.information.site(id, cc, c, now)?;
            let system = &report.system;
            if system.owner == Some(owner) || system.owner.is_some_and(|o| self.world.known_ally(owner, o, now)) {
                return None;
            }
            (intel.enclave_tier > 0 || system.owner.is_some()).then_some(warfare::MilitarySite {
                id, pos: system.pos, owner: system.owner, intel: *intel,
                blockade: system.blockade,
            })
        }).collect();
        let relations = self.world.diplomacy.values().flat_map(|r| r.values())
            .filter_map(|r| r.other(owner).map(|other| (other, r.known_by(owner)))).collect();
        Some(Observation {
            owner, capital: home_id, now, cc, hub: self.world.hub, c, siege_secs: self.world.siege_duration_secs(), home, sites, prospects, ghosts, fighting,
            account: self.market_accounts.at(owner, market_time).cloned(),
            market_at: market_time,
            exported: corp.founding.sale_report_at.is_some(),
            // These are the same CC tutorial ledger and public candidate ids
            // served in FoundingView, not an early read of survey geology.
            founding_stage: corp.founding.stage,
            local_opening: corp.founding.local_opening,
            opening_recovered: corp.founding.recovery_report_at.is_some_and(|at| at <= now),
            recovery_site: corp.founding.recovery_site.and_then(|id| self.world.exploration.sites.get(&id)).and_then(|s| s.known.get(&owner)).cloned(),
            expansion_unlocked: corp.founding.expansion_unlocked(),
            survey_candidates: corp.founding.survey_candidates.clone(),
            // The programme's own target: a belt this corporation charted.
            belt_lesson: matches!(corp.founding.stage,
                    sim::FoundingStage::BuildBarge | sim::FoundingStage::MineBelt | sim::FoundingStage::HaulOre)
                .then_some(corp.founding.belt_target).flatten()
                .and_then(|(system_id, body_id)| {
                    let system = self.world.systems.iter().find(|s| s.id == system_id)?;
                    let belt = system.bodies.iter().find(|b| b.id == body_id)?;
                    let ore = belt.deposits.iter().max_by(|a, b| {
                        let value = |d: &sim::galaxy::Deposit| d.richness
                            * sim::production::ore_bulk_ratio(d.resource) * sim::market::base_price(d.resource);
                        value(a).total_cmp(&value(b))
                    })?.resource;
                    Some((system_id, body_id, ore, system.pos))
                }),
            belts: corp.surveyed.iter().filter_map(|id| self.world.information.site(*id, cc, c, now))
                .flat_map(|report| {
                    let s = &report.system;
                    s.bodies.iter().filter(|b| b.is_belt()).map(move |b| belts::BeltSite {
                        system: s.id, body: b.id, pos: s.pos, owner: s.owner,
                        ores: b.deposits.iter().map(|d| (d.resource, false, d.richness, d.reserves))
                            .chain(b.pockets.iter().filter(|k| k.found_by.contains(&owner))
                                .map(|k| (k.deposit.resource, true, k.deposit.richness, k.deposit.reserves)))
                            .collect(),
                        prospected: b.prospected.get(&owner).map_or(0, |k| k.reported),
                    })
                }).collect(),
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
            military_sites, relations,
            captains: self.world.information.captains(owner, cc, c, now),
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
            let old_status = bot.status.clone();
            if let Some(command) = bot.decide(&picture) {
                debug!(%owner, t = self.world.time as u64, ?command, "bot issued command");
                bot.commands_sent += 1;
                self.pending.push(command);
            }
            if bot.status != old_status {
                info!(%owner, t = self.world.time as u64, reason = %bot.status, "bot decision");
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
            + self.home.builds.iter().chain(self.sites.iter().filter(|s| s.system.id != self.home.system.id)
                .flat_map(|s| &s.builds)).filter(|b| b.what == BuildKind::Ship { ship: kind }).count() as u32
    }

    fn round_trip(&self, pos: Vec2) -> f64 {
        2.0 * sim::transit::delay(self.cc, pos, self.c) + THINK_SECONDS * 2.0
    }

    /// Plan from public geometry and reported tanks. Cargo has mass too; the
    /// empty-hull estimate can strand a fully loaded Tiny on its opening run.
    /// Merge well intersections so overlapping public wells aren't charged twice.
    fn fuel_per_mass(&self, from: Vec2, to: Vec2) -> f64 {
        sim::fuel::route_estimate(from, to, 1.0,
            sim::research::mod_of(&self.research, sim::research::ModKey::FuelConsumption),
            self.chart.iter().map(|(_, p)| *p).chain(std::iter::once(self.hub)))
    }

    fn freight_limit(&self, fleet: &GhostView, dest: Vec2) -> u32 {
        let per_mass = self.fuel_per_mass(fleet.pos, dest) * self.cruise_mult(fleet);
        if per_mass <= 0.0 { return capacity(fleet); }
        let mass_budget = store(fleet) / per_mass;
        (((mass_budget - hull_mass(fleet)).max(0.0) / sim::ship::CARGO_MASS_PER_UNIT).floor() as u32)
            .min(capacity(fleet))
    }

    fn enough_fuel(&self, fleet: &GhostView, dest: Vec2) -> bool {
        let mass = hull_mass(fleet) + cargo_used(fleet) as f64 * sim::ship::CARGO_MASS_PER_UNIT;
        fleet.fuel.is_some() && store(fleet) >= self.fuel_per_mass(fleet.pos, dest) * mass * self.cruise_mult(fleet)
    }

    /// §fuel-overhaul: Fusion Freight is researched.
    fn fusion_freight(&self) -> bool { self.research.has("prop_line_fusion_freight") }

    /// §fuel-overhaul: freight flies Slow on Uranium until Fusion Freight makes
    /// full-speed Deuterium freight cost the same.
    fn freight_transit(&self) -> sim::ship::TransitMode {
        if self.fusion_freight() { sim::ship::TransitMode::Full } else { sim::ship::TransitMode::Slow }
    }

    /// The throttle's multiplier on energy per distance, as the sim applies it.
    fn cruise_mult(&self, ghost: &GhostView) -> f64 {
        self.cruise_mult_on(ghost, on_uranium(ghost))
    }

    /// The multiplier on one store, weighed hull by hull by mass as the sim
    /// does (`Fleet::cruise_mult_for`): a quarter on Uranium; Fusion Freight's
    /// cut for player freighters and their cargo; Torch Regime on warships.
    fn cruise_mult_on(&self, ghost: &GhostView, uranium: bool) -> f64 {
        let mods = sim::research::mods(&self.research);
        let freight = mods.get(&sim::research::ModKey::FreighterFusionBurn).copied().unwrap_or(1.0);
        let torch = mods.get(&sim::research::ModKey::WarshipBurn).copied().unwrap_or(1.0);
        let regime = |freighter: bool| if uranium { sim::fuel::SLOW_FUEL_MULT }
            else if freighter { freight } else { 1.0 };
        let stacks = || ghost.composition.iter().flatten().filter(|s| s.count > 0);
        let cargo = cargo_used(ghost) as f64 * sim::ship::CARGO_MASS_PER_UNIT;
        let total = hull_mass(ghost) + cargo;
        if total <= 1e-9 {
            return regime(false);
        }
        let hulls: f64 = stacks().map(|s| s.kind.hull_mass() * s.count as f64
            * regime(s.kind.is_player_freighter()) * if s.kind.is_combatant() { torch } else { 1.0 }).sum();
        (hulls + cargo * regime(stacks().any(|s| s.kind.is_player_freighter()))) / total
    }

    /// §fuel-overhaul: an escort in formation burns its charge's Uranium regime
    /// when it can fly it. Returns (its store, its multiplier).
    fn escort_burn(&self, guard: &GhostView, charge_slow: bool) -> (f64, f64) {
        let uranium = charge_slow && slow_capable(guard);
        (if uranium { guard.uranium } else { guard.fuel }.unwrap_or(0.0), self.cruise_mult_on(guard, uranium))
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
        let refinery_product = self.home.system.bodies.iter().any(|b| b.structures.iter().any(|(&k, &t)| t > 0
            && sim::production::assigned_converter(k, b.assignments.get(&k))
                .is_some_and(|r| r.outputs().any(|(c, _)| c == good))));
        if sim::production::extraction_structure(good).is_none() && good != C::Provisions && !refinery_product { return None; }
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
            BuildKind::Ship { ship: H::Colony } =>
                sim::research::mod_of(&self.research, sim::research::ModKey::ColonyCost),
            _ => 1.0,
        };
        let tier = match project.what {
            BuildKind::Upgrade { upgrade } => self.home.system.bodies.iter().find(|b| b.id == project.body)
                .map_or(0, |b| b.tier(upgrade)) + 1,
            _ => 1,
        };
        sim::build::recipe_for_tier(project.what, tier).costs.iter().map(|(c, n)| (*c, n * mult)).collect()
    }

    /// §power: the planet whose demand outruns its plant (and base), with the
    /// MW it is short. A dry plant is a fuel problem, not a building one.
    pub(super) fn power_shortfall(&self) -> Option<(u32, f64)> {
        self.home.system.bodies.iter().filter(|b| !b.grid.no_fuel)
            .map(|b| (b.id, b.grid.demand - sim::power::BASE_MW - sim::power::plant_mw(b.tier(K::PowerPlant))))
            .filter(|(_, short)| *short > 1e-9)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// §power: build or raise that planet's plant, within its research cap.
    pub(super) fn power_project(&self) -> Option<Project> {
        let (body, _) = self.power_shortfall()?;
        if self.home.builds.iter().any(|b| b.what == BuildKind::Upgrade { upgrade: K::PowerPlant }) { return None; }
        let tier = self.home.system.bodies.iter().find(|b| b.id == body)?.tier(K::PowerPlant) + 1;
        let cap = sim::build::max_buildable_tier(K::PowerPlant, sim::research::unlocked_structure_tier(&self.research, K::PowerPlant));
        (tier <= cap).then_some(Project { what: BuildKind::Upgrade { upgrade: K::PowerPlant }, body })
    }

    fn structure(&self, kind: K, tier: u32) -> Option<Project> {
        // §power: plants are per planet; `power_project` places them.
        if kind == K::PowerPlant { return None; }
        let system = &self.home.system;
        if system.tier(kind) >= tier || self.home.builds.iter()
            .any(|b| b.what == BuildKind::Upgrade { upgrade: kind }) { return None; }
        let cap = sim::build::max_buildable_tier(kind, sim::research::unlocked_structure_tier(&self.research, kind));
        if tier > cap { return None; }
        if sim::build::yard_prereq(kind).is_some_and(|(k, n)| system.tier(k) < n) { return None; }
        // Prefer the natural deposit/inhabited site, but use another legal body
        // rather than repeatedly ordering construction into a full slot pool.
        let preferred = system.site_for(kind);
        let body = system.bodies.iter().filter(|b| {
            let extraction = matches!(kind, K::MiningComplex | K::Bioharvester | K::VolatileHarvester);
            (kind != K::CommandCenter || b.population >= sim::build::COMMAND_CENTER_MIN_POPULATION
                && b.kind != sim::body::BodyKind::GasGiant)
                && (!extraction || b.has_deposit_for(kind)) && (!kind.uses_slot() || b.tier(kind) > 0
                || b.pool_slots_built(kind.slot_pool()) < b.pool_slots(kind.slot_pool()))
        }).max_by_key(|b| (b.tier(kind), Some(b.id) == preferred))?;
        Some(Project { what: BuildKind::Upgrade { upgrade: kind }, body: body.id })
    }

    fn ship(&self, kind: H, desired: u32) -> Option<Project> {
        if kind == H::Colony && !self.expansion_unlocked { return None; }
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

/// §fuel-overhaul: a formation without a ship of the line can fly Slow.
fn slow_capable(ghost: &GhostView) -> bool {
    ghost.composition.as_ref().is_some_and(|stacks| stacks.iter().any(|s| s.count > 0)
        && stacks.iter().all(|s| s.count == 0 || !s.kind.is_siege_anchor()))
}

/// Flying Slow on the fission store.
fn on_uranium(ghost: &GhostView) -> bool {
    ghost.transit == Some(sim::ship::TransitMode::Slow) && slow_capable(ghost)
}

/// The fuel its regime burns.
fn regime_fuel(ghost: &GhostView) -> C {
    if on_uranium(ghost) { C::Uranium } else { C::Deuterium }
}

/// The reported level of the store its regime burns.
fn store(ghost: &GhostView) -> f64 {
    if on_uranium(ghost) { ghost.uranium.unwrap_or(0.0) } else { ghost.fuel.unwrap_or(0.0) }
}

impl Bot {
    fn ready(&self, key: &str, now: f64) -> bool {
        self.waits.get(key).is_none_or(|until| *until <= now)
    }

    fn wait(&mut self, key: String, picture: &Observation, pos: Vec2) {
        self.waits.insert(key, picture.now + picture.round_trip(pos));
    }

    fn project(&self, p: &Observation) -> Option<Project> {
        if p.home.system.id != p.capital { return self.strategic_project(p); }
        if let Some(goal) = &self.development {
            // Paid construction is already supplied. Keep the goal until its
            // completion report, but don't buy a second copy of its recipe.
            return (!p.home.builds.iter().any(|b| b.what == goal.project.what))
                .then_some(goal.project);
        }
        self.next_project(p)
    }

    fn next_project(&self, p: &Observation) -> Option<Project> {
        self.strategic_project(p)
    }

    fn desired_stock(&self, p: &Observation, project: Option<Project>) -> BTreeMap<C, f64> {
        let mut wanted = project.map(|goal| p.project_costs(goal)).unwrap_or_default();
        // §fuel: hold the whole target. Wanting half of it sat on the critical
        // line, and its reorder point sat below it: every refill began in a crisis.
        // §fuel-overhaul: one target per fuel.
        for fuel in FUELS { *wanted.entry(fuel).or_default() += p.fuel_target_of(fuel); }
        let food = (p.home.system.population() * sim::colony::PROVISIONS_PER_MILLION_PER_S
            * p.supply_lead() * 2.0).min(p.home.system.storage_cap() * 0.25)
            .max(if p.stock(C::Provisions) < 10.0 { 40.0 } else { 20.0 });
        *wanted.entry(C::Provisions).or_default() += food;
        if let Some(programme) = p.research.active.as_deref().and_then(sim::research::programme)
            .filter(|_| p.home.system.id == p.capital || p.home.system.tier(K::Academy) > 0) {
            // Ten minutes of electronics cannot sustain a two-hour programme
            // behind a longer freight lead. Buy two delivery windows, bounded
            // by the remaining work (normal research still pays every unit).
            let rate = p.home.academies.iter().map(|a| a.rate).sum::<f64>().max(1.0);
            let work = (p.supply_lead() * 2.0 * rate)
                .min((sim::research::tier_cost_secs(programme.tier) - p.research.progress).max(0.0));
            for (good, rate) in sim::research::basket(programme.field, programme.tier) {
                *wanted.entry(good).or_default() += (rate * work).ceil();
            }
        }
        self.industry_buffers(p, project, &mut wanted);
        // An issued transfer earmarks source goods; it is not destination
        // stock and can never satisfy a build's actual-inventory check.
        for c in C::ALL { *wanted.entry(c).or_default() += self.reserved_supply(p, c) as f64; }
        wanted
    }

    /// A stocked project starts unless a cash or fuel crisis holds it. The
    /// mine and casualty recovery are exempt, and so (§fuel) is a fuel line:
    /// the cure for a fuel shortage is never held by one.
    fn may_build(&self, p: &Observation, project: Project) -> bool {
        let essential = matches!(project.what, BuildKind::Upgrade {
                upgrade: K::MiningComplex | K::VolatileHarvester | K::FuelRefinery | K::UraniumMill })
            || self.recovery_project(p) == Some(project);
        (essential || !self.recovering(p) && !p.fuel_critical())
            && p.project_costs(project).iter().all(|(c, n)| p.stock(*c) + 1e-9 >= *n)
    }

    fn decide(&mut self, p: &Observation) -> Option<Command> {
        self.refresh(p);
        self.refresh_development(p);
        self.refresh_network(p);
        if let Some(cmd) = self.rescue(p).or_else(|| self.recover_military(p)).or_else(|| self.staff(p))
            .or_else(|| self.military(p)).or_else(|| self.guards(p)).or_else(|| self.officers(p))
            .or_else(|| self.local_opening(p))
            .or_else(|| self.scouts(p)).or_else(|| self.belt_lesson(p)).or_else(|| self.belt_work(p))
            .or_else(|| self.colonies(p)) { return Some(cmd); }
        let project = self.project(p);
        let wanted = self.desired_stock(p, project);
        if let Some(cmd) = self.network_routes(p) { return Some(cmd); }
        if self.ready("build", p.now) && p.home.builds.is_empty() {
            if let Some(project) = project {
                if self.may_build(p, project) {
                    self.wait("build".into(), p, p.home.system.pos);
                    self.status = "building the next funded bottleneck improvement".into();
                    return Some(match project.what {
                        BuildKind::Upgrade { upgrade } => Command::DevelopSystem {
                            player_id: p.owner, system_id: p.home.system.id, upgrade, body_id: Some(project.body) },
                        BuildKind::Ship { ship } => Command::BuildShip {
                            player_id: p.owner, system_id: p.home.system.id, ship_kind: ship, join: None, loadout: Default::default() },
                        _ => unreachable!(),
                    });
                }
            }
        }
        if self.ready("research", p.now) && p.home.system.tier(K::Academy) > 0
            && p.research.active.is_none() && p.research.queue.is_empty() && !self.recovering(p) {
            if let Some(id) = self.next_research(p, &wanted) {
                self.wait("research".into(), p, p.home.system.pos);
                self.status = "researching the reported operating bottleneck".into();
                return Some(Command::SetResearchQueue { player_id: p.owner, queue: vec![id.into()] });
            }
        }
        // Rotate the first serviced port; a home buying one batch every think
        // must not starve its colonies (and colonies must not monopolize cash).
        let command = self.network_economy(p, &wanted);
        if command.is_none() { self.explain_blocker(p, project); }
        command
    }
}

#[cfg(test)]
mod tests;
