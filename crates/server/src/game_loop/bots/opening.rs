//! The same local tutorial as human founders, using only arrived observations.
use super::*;

impl Observation {
    /// §charter-v4: a fleet at the privateer den during the den step. It
    /// fights there to the end: one Interceptor wins, and nothing repairs
    /// hulls before an Ordnance Foundry.
    pub(super) fn at_den(&self, fleet: &GhostView) -> bool {
        self.den.is_some_and(|(_, pos)| fleet.pos.distance(pos) <= 5_000.0)
    }

    /// The retired v2 local recovery before the Hub voyage.
    pub(super) fn opening_local_work(&self) -> bool {
        self.local_opening && self.founding_stage == sim::FoundingStage::LocalRecovery
    }

    /// §charter-v4: the landing world, where the Command Center stands.
    pub(super) fn landing_body(&self) -> Option<u32> {
        self.home.system.bodies.iter().find(|b| b.tier(K::CommandCenter) > 0).map(|b| b.id)
    }

    /// Provisions a second the home's staffed farms make, as reported.
    pub(super) fn food_grown(&self) -> f64 {
        let system = &self.home.system;
        system.bodies.iter().flat_map(|body| body.structures.iter().map(move |(&kind, &tier)| (body, kind, tier)))
            .filter_map(|(body, kind, tier)| {
                let a = body.assignments.get(&kind).filter(|a| a.workers > 0)?;
                let recipe = sim::production::assigned_converter(kind, Some(a)).filter(|r| r.output == C::Provisions)?;
                Some(recipe.rate * sim::production::tier_throughput(tier) * a.workers.min(tier) as f64
                    / tier.max(1) as f64 * system.staffing_share())
            })
            .sum()
    }

    /// §charter-v5: what the charter's builds still ahead take from the
    /// stockpile, from the trader's last call on: after it nothing nearer than
    /// the Market Hub replaces kit goods spent on optional works.
    pub(super) fn charter_reserve(&self) -> BTreeMap<C, f64> {
        let mut goods = BTreeMap::new();
        let step = self.founding_stage.step();
        if !self.local_opening || self.founding_stage == sim::FoundingStage::Complete
            || step < sim::FoundingStage::EliminateDen.step()
        {
            return goods;
        }
        let system = &self.home.system;
        let expanded = |kind: K| system.bodies.iter().any(|b| b.tier(K::CommandCenter) == 0 && b.tier(kind) > 0);
        let ahead = [
            (system.tier(K::Academy) == 0, BuildKind::Upgrade { upgrade: K::Academy }),
            (step <= sim::FoundingStage::ExpandMining.step() && !expanded(K::PowerPlant),
                BuildKind::Upgrade { upgrade: K::PowerPlant }),
            (step <= sim::FoundingStage::ExpandMining.step() && !expanded(K::MiningComplex),
                BuildKind::Upgrade { upgrade: K::MiningComplex }),
            (step <= sim::FoundingStage::SurveyCandidates.step() && self.count(H::SurveyProbe) == 0,
                BuildKind::Ship { ship: H::SurveyProbe }),
            (self.count(H::MiningBarge) == 0, BuildKind::Ship { ship: H::MiningBarge }),
        ];
        for (_, what) in ahead.into_iter().filter(|(due, _)| *due) {
            for &(c, n) in sim::build::recipe_for(what).costs {
                *goods.entry(c).or_insert(0.0) += n;
            }
        }
        goods
    }

    /// §charter-v5: the founding trader is docked at home with `c` for sale.
    pub(super) fn trader_stocks(&self, c: C) -> bool {
        self.local_opening && self.merchant.as_ref().is_some_and(|v| v.phase == sim::merchant::MerchantPhase::Docked
            && v.system == self.home.system.id && v.stock.get(&c).copied().unwrap_or(0) > 0)
    }
}

/// §charter-v5: the goods the charter's builds after the den take: the
/// Academy, the expansion's Solar Array and Mining Complex, the Survey Probe
/// and the Mining Barge. The guide's "Stock up" prefill counts the same
/// builds (client `merchant.ts`, `AHEAD`).
pub(super) fn charter_goods_ahead() -> BTreeMap<C, f64> {
    let mut goods = BTreeMap::new();
    for what in [BuildKind::Upgrade { upgrade: K::Academy }, BuildKind::Upgrade { upgrade: K::PowerPlant },
        BuildKind::Upgrade { upgrade: K::MiningComplex }, BuildKind::Ship { ship: H::SurveyProbe },
        BuildKind::Ship { ship: H::MiningBarge }]
    {
        for &(c, n) in sim::build::recipe_for(what).costs {
            *goods.entry(c).or_insert(0.0) += n;
        }
    }
    goods
}

impl Bot {
    /// §charter-v4: the landing world's first Solar Array: the Shipyard needs
    /// more than the Command Center's reactor. §charter-v5: built with the
    /// semiconductors the trader sells back after the raid.
    pub(super) fn landing_plant(&self, p: &Observation) -> Option<Project> {
        if p.home.system.tier(K::PowerPlant) > 0 || p.home.system.tier(K::MiningComplex) == 0 {
            return None;
        }
        Some(Project { what: BuildKind::Upgrade { upgrade: K::PowerPlant }, body: p.landing_body()? })
    }

    /// §charter-v5: between the mine and the ore sale the raid takes the
    /// kit's semiconductors and Armaments, so nothing is built from them.
    pub(super) fn raid_pending(&self, p: &Observation) -> bool {
        p.local_opening && p.home.system.tier(K::MiningComplex) > 0
            && p.founding_stage.step() < sim::FoundingStage::PowerShipyard.step()
    }

    /// §charter-v5: "Sell ore to the trader": all the free ore it will take.
    pub(super) fn charter_ore_sale(&mut self, p: &Observation) -> Option<Command> {
        if p.founding_stage != sim::FoundingStage::SellOre || !self.ready("merchant", p.now) {
            return None;
        }
        let visit = p.merchant.as_ref().filter(|v| v.phase == sim::merchant::MerchantPhase::Docked
            && v.system == p.home.system.id)?;
        let ore = (p.home.system.free_stock(C::MetallicOre).floor().max(0.0) as u32).min(visit.ore_room());
        if ore == 0 {
            return None;
        }
        self.wait("merchant".into(), p, p.home.system.pos);
        self.status = "founding charter: selling ore to the trader".into();
        Some(Command::MerchantTrade { player_id: p.owner, buys: BTreeMap::new(),
            sells: BTreeMap::from([(C::MetallicOre, ore)]) })
    }

    /// §charter-v4: Shipyard II, then an Interceptor for the privateer.
    pub(super) fn charter_escort(&self, p: &Observation) -> Option<Project> {
        if !matches!(p.founding_stage, sim::FoundingStage::BuildInterceptor | sim::FoundingStage::DefeatPrivateer)
            || p.count(H::Raider) > 0
        {
            return None;
        }
        p.structure(K::Shipyard, 2).or_else(|| p.ship(H::Raider, 1))
    }

    /// §charter-v4: "Expand mining operations": a Solar Array, then a Mining
    /// Complex, on another world of the home system (a planet before a belt).
    pub(super) fn charter_expansion(&self, p: &Observation) -> Option<Project> {
        if p.founding_stage != sim::FoundingStage::ExpandMining {
            return None;
        }
        let pool = sim::build::SlotPool::Resource;
        let body = p.home.system.bodies.iter()
            .filter(|b| b.tier(K::CommandCenter) == 0 && b.has_deposit_for(K::MiningComplex)
                && (b.tier(K::MiningComplex) > 0 || b.pool_slots_built(pool) < b.pool_slots(pool)))
            .max_by_key(|b| (b.tier(K::MiningComplex), !b.is_belt(), std::cmp::Reverse(b.id)))?;
        let kind = if body.tier(K::PowerPlant) == 0 { K::PowerPlant }
            else if body.tier(K::MiningComplex) == 0 { K::MiningComplex }
            else { return None };
        Some(Project { what: BuildKind::Upgrade { upgrade: kind }, body: body.id })
    }
}

impl Bot {
    /// §charter-v5: "Destroy the privateer": an Interceptor attacks it where
    /// it lurks, tracked since its raid. One already fighting is left alone.
    pub(super) fn charter_privateer(&mut self, p: &Observation) -> Option<Command> {
        if p.founding_stage != sim::FoundingStage::DefeatPrivateer {
            return None;
        }
        let target = p.privateer.and_then(|id| p.ghosts.iter().find(|g| g.id == id))?;
        let raiders = || p.own().filter(|f| warfare::composition(f).get(&H::Raider).copied().unwrap_or(0) > 0);
        if raiders().any(|f| p.fighting.contains(&f.id)) {
            return None;
        }
        let trip = |f: &GhostView| {
            let leg = |a: Vec2, b: Vec2| p.fuel_per_mass(a, b) * hull_mass(f) * p.cruise_mult(f);
            leg(f.pos, target.pos) + leg(target.pos, p.home.system.pos)
        };
        let fleet = raiders()
            .filter(|f| f.supplied && !f.stalled
                && !self.returns.contains_key(&f.id) && self.ready(&format!("fleet:{}", f.id), p.now)
                && f.fuel.is_some() && store(f) >= trip(f))
            .min_by(|a, b| a.pos.distance(target.pos).total_cmp(&b.pos.distance(target.pos)).then(a.id.cmp(&b.id)));
        let Some(fleet) = fleet else {
            // Short of fuel at home: the trader's Deuterium fills it at the dock.
            let short = raiders().filter(|f| !self.returns.contains_key(&f.id))
                .map(|f| trip(f) - store(f) - p.stock(C::Deuterium)).fold(0.0, f64::max);
            return self.trader_fuel(p, short);
        };
        self.wait(format!("fleet:{}", fleet.id), p, fleet.pos);
        self.status = "founding charter: the Interceptor after the privateer".into();
        Some(Command::AttackFleet { player_id: p.owner, fleet_id: fleet.id, target_id: target.id })
    }

    /// §charter-v5: buy `short` Deuterium (and a margin) from the docked
    /// trader, for an Interceptor that can't make its charter trip.
    fn trader_fuel(&mut self, p: &Observation, short: f64) -> Option<Command> {
        if short <= 0.0 || !p.trader_stocks(C::Deuterium) || !self.ready("merchant", p.now) {
            return None;
        }
        let visit = p.merchant.as_ref()?;
        let units = ((short + 2.0).ceil() as u32).min(visit.stock.get(&C::Deuterium).copied().unwrap_or(0));
        let cost = visit.quote(&BTreeMap::from([(C::Deuterium, units)]), &BTreeMap::new()).ok()?.cost;
        if units == 0 || cost > self.spendable(p, true) {
            return None;
        }
        self.spend(p, cost);
        self.wait("merchant".into(), p, p.home.system.pos);
        self.status = "founding charter: Deuterium from the trader for the Interceptor".into();
        Some(Command::MerchantTrade { player_id: p.owner, buys: BTreeMap::from([(C::Deuterium, units)]),
            sells: BTreeMap::new() })
    }

    /// §charter-v5: the trader's last call. Once the privateer's kill is
    /// reported the trader leaves within minutes (`merchant::LAST_CALL_S`), and
    /// after it only the Market Hub sells what the kit no longer covers (the
    /// raid took its semiconductors): the bounty buys what the builds after
    /// the den lack, semiconductors first, as far as the cash goes.
    pub(super) fn charter_stock_up(&mut self, p: &Observation) -> Option<Command> {
        if !matches!(p.founding_stage, sim::FoundingStage::EliminateDen | sim::FoundingStage::PrepareExport)
            || !self.ready("merchant", p.now)
        {
            return None;
        }
        let visit = p.merchant.as_ref().filter(|v| v.phase == sim::merchant::MerchantPhase::Docked
            && v.system == p.home.system.id && p.local_opening)?;
        let mut ahead = charter_goods_ahead();
        // The Interceptor's tank too: development waits while a docked hull
        // is short of fuel, and the Hub's Deuterium is far.
        let fuel = p.fuel_target_of(C::Deuterium) * 2.0 - p.stock(C::Deuterium) - p.inbound(C::Deuterium) as f64;
        let mut buys = BTreeMap::new();
        let mut budget = self.spendable(p, true);
        for c in [C::Electronics, C::Deuterium].into_iter()
            .chain(ahead.keys().copied().filter(|c| *c != C::Electronics).collect::<Vec<_>>())
        {
            let short = if c == C::Deuterium { fuel.ceil().max(0.0) as u32 } else {
                (ahead.remove(&c).unwrap_or(0.0) - p.home.system.free_stock(c)).ceil().max(0.0) as u32
            };
            let mut units = short.min(visit.stock.get(&c).copied().unwrap_or(0));
            let one = visit.quote(&BTreeMap::from([(c, 1)]), &BTreeMap::new()).map_or(f64::INFINITY, |q| q.cost);
            units = units.min((budget / one).floor() as u32);
            if units > 0 {
                budget -= one * units as f64;
                buys.insert(c, units);
            }
        }
        let cost = visit.quote(&buys, &BTreeMap::new()).ok()?.cost;
        self.spend(p, cost);
        self.wait("merchant".into(), p, p.home.system.pos);
        self.status = "founding charter: stocking up from the trader before it leaves".into();
        Some(Command::MerchantTrade { player_id: p.owner, buys, sells: BTreeMap::new() })
    }

    /// §charter-v4: "Eliminate the privateer den": an Interceptor flies to
    /// the den the privateer's log named and stops there, where its garrison
    /// engages. Damaged or not: nothing repairs before an Ordnance Foundry, and
    /// the garrison never regrows. One at the den or on its way is left alone.
    pub(super) fn charter_den(&mut self, p: &Observation) -> Option<Command> {
        let (_, pos) = p.den?;
        let raiders = || p.own().filter(|f| warfare::composition(f).get(&H::Raider).copied().unwrap_or(0) > 0);
        let committed = raiders().any(|f| f.pos.distance(pos) <= sim::pirate::PIRATE_ASSAULT_RADIUS
            || p.fighting.contains(&f.id)
            || f.path.as_ref().and_then(|path| path.last()).is_some_and(|v| v.pos.distance(pos) < 1.0));
        if committed {
            return None;
        }
        let trip = |f: &GhostView| {
            let leg = |a: Vec2, b: Vec2| p.fuel_per_mass(a, b) * hull_mass(f) * p.cruise_mult(f);
            leg(f.pos, pos) + leg(pos, p.home.system.pos)
        };
        let fleet = raiders()
            .filter(|f| f.supplied && !f.stalled
                && !self.returns.contains_key(&f.id) && self.ready(&format!("fleet:{}", f.id), p.now)
                && f.fuel.is_some() && store(f) >= trip(f))
            .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)).then(a.id.cmp(&b.id)));
        let Some(fleet) = fleet else {
            let short = raiders().filter(|f| !self.returns.contains_key(&f.id))
                .map(|f| trip(f) - store(f) - p.stock(C::Deuterium)).fold(0.0, f64::max);
            return self.trader_fuel(p, short);
        };
        self.wait(format!("fleet:{}", fleet.id), p, fleet.pos);
        self.status = "founding charter: the Interceptor to the privateer den".into();
        Some(Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: pos })
    }

    /// §belts: the charter's last lesson, as a founder plays it: send the
    /// Mining Barge to the charted belt the programme points at.
    pub(super) fn belt_lesson(&mut self, p: &Observation) -> Option<Command> {
        let (system_id, body_id, ore, pos) = p.belt_lesson?;
        let barge = p.own().find(|g| g.kind == H::MiningBarge)?;
        if !self.ready(&format!("fleet:{}", barge.id), p.now) || p.fighting.contains(&barge.id) {
            return None;
        }
        let command = match p.founding_stage {
            sim::FoundingStage::MineBelt => {
                let working = matches!(&barge.industry,
                    Some(sim::industry::FleetIndustry::Mine { system, .. }) if *system == system_id);
                if working || p.threatened_within(pos, super::belts::BELT_THREAT_SU) {
                    return None;
                }
                self.status = "founding charter: a Mining Barge to the charted belt".into();
                Command::MineBelt { player_id: p.owner, fleet_id: barge.id, system_id, body_id, ore, pocket: false }
            }
            // §charter v3: the last step brings a load home and unloads it.
            sim::FoundingStage::HaulOre => {
                let home = &p.home.system;
                let aboard: u32 = barge.cargo_manifest.iter().map(|c| c.units).sum();
                let full = matches!(&barge.industry,
                    Some(sim::industry::FleetIndustry::Mine { status, .. }) if status == "full");
                let at_home = barge.docked.as_deref() == Some(home.id.to_string().as_str());
                if at_home && aboard > 0 {
                    self.status = "founding charter: unloading the barge's ore at home".into();
                    Command::SystemUnload { player_id: p.owner, fleet_id: barge.id, system: home.id }
                } else if !at_home && barge.industry.is_some() && (aboard >= 30 || full) {
                    self.status = "founding charter: bringing the barge's ore home".into();
                    Command::MoveShip { player_id: p.owner, ship_id: barge.id, dest: home.pos }
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        self.wait(format!("fleet:{}", barge.id), p, barge.pos);
        Some(command)
    }

    pub(super) fn local_opening(&mut self, p: &Observation) -> Option<Command> {
        if !p.opening_local_work() {
            return None;
        }
        let site = p.recovery_site.as_ref()?;
        let scout = p.own().find(|g| g.kind == H::Scout)?;
        let carrier = p.own().find(|g| g.kind.is_player_freighter())?;
        let home = &p.home.system;
        let at_home = |g: &GhostView| g.docked.as_deref() == Some(home.id.to_string().as_str());
        let ready = |bot: &Self, g: &GhostView| {
            bot.ready(&format!("fleet:{}", g.id), p.now) && !p.fighting.contains(&g.id)
        };
        if ready(self, scout) && !at_home(scout) && p.threatened(scout.pos) {
            return self.return_for_fuel(p, scout);
        }
        // A Scout never investigates a known threat. It can return and retry
        // after the escort's combat report rather than overriding safe arrival.
        let command = if site.details.is_none() {
            if !ready(self, scout) {
                return None;
            }
            if p.threatened(site.pos) || p.threatened(scout.pos) {
                return self.return_for_fuel(p, scout);
            }
            if scout.expedition.as_ref().is_some_and(|e| e.site == site.id) {
                return None;
            }
            Command::ExploreSite {
                player_id: p.owner,
                fleet_id: scout.id,
                site_id: site.id,
                task: sim::sites::ExpeditionTask::Investigate,
            }
        } else if !p.opening_recovered {
            if !ready(self, carrier) {
                return None;
            }
            if carrier
                .expedition
                .as_ref()
                .is_some_and(|e| e.site == site.id)
            {
                return None;
            }
            if carrier.guard_target.is_some() {
                return None;
            }
            if !p.own().any(|g| g.guard_target == Some(carrier.id)) {
                return None;
            }
            if cargo_used(carrier) > 0 {
                if at_home(carrier) {
                    Command::SystemUnload {
                        player_id: p.owner,
                        fleet_id: carrier.id,
                        system: home.id,
                    }
                } else {
                    Command::HaulToSystem {
                        player_id: p.owner,
                        fleet_id: carrier.id,
                        system: home.id,
                    }
                }
            } else {
                Command::ExploreSite {
                    player_id: p.owner,
                    fleet_id: carrier.id,
                    site_id: site.id,
                    task: sim::sites::ExpeditionTask::Recover,
                }
            }
        } else if !at_home(carrier) && ready(self, carrier) {
            if carrier.vel.length_sq() > 1.0
                && carrier
                    .path
                    .as_ref()
                    .and_then(|p| p.last())
                    .is_some_and(|v| v.pos.distance(home.pos) < 1.0)
            {
                return None;
            }
            Command::HaulToSystem {
                player_id: p.owner,
                fleet_id: carrier.id,
                system: home.id,
            }
        } else if at_home(carrier) && cargo_used(carrier) > 0 && ready(self, carrier) {
            Command::SystemUnload {
                player_id: p.owner,
                fleet_id: carrier.id,
                system: home.id,
            }
        } else if !at_home(scout) && ready(self, scout) {
            if scout.vel.length_sq() > 1.0
                && scout
                    .path
                    .as_ref()
                    .and_then(|p| p.last())
                    .is_some_and(|v| v.pos.distance(home.pos) < 1.0)
            {
                return None;
            }
            Command::MoveShip {
                player_id: p.owner,
                ship_id: scout.id,
                dest: home.pos,
            }
        } else if home.tier(K::ServiceBerth) == 0 && home.tier(K::Shipyard) == 0 {
            if !self.ready("build", p.now) || !p.home.builds.is_empty() {
                return None;
            }
            let body = home.bodies.iter().find(|b| b.tier(K::Warehouse) > 0)?;
            if !sim::build::SERVICE_BERTH_RECIPE
                .costs
                .iter()
                .all(|(c, n)| p.stock(*c) >= *n)
            {
                return None;
            }
            self.wait("build".into(), p, home.pos);
            return Some(Command::DevelopSystem {
                player_id: p.owner,
                system_id: home.id,
                body_id: Some(body.id),
                upgrade: K::ServiceBerth,
            });
        } else if at_home(scout)
            && ready(self, scout)
            && home
                .modules
                .get(&sim::ModuleKind::ReconSuite)
                .copied()
                .unwrap_or(0)
                > 0
        {
            let from = scout
                .loadouts
                .as_ref()
                .and_then(|v| v.iter().find(|s| s.kind == H::Scout))
                .map(|s| sim::Loadout::new(s.modules.clone()))
                .unwrap_or_default();
            if from.has_recon() {
                return None;
            }
            Command::RefitShips {
                player_id: p.owner,
                fleet_id: scout.id,
                ship: H::Scout,
                from,
                to: sim::Loadout::new(vec![sim::ModuleKind::ReconSuite]),
                n: 1,
            }
        } else {
            return None;
        };
        let fleet = match &command {
            Command::ExploreSite { fleet_id, .. }
            | Command::HaulToSystem { fleet_id, .. }
            | Command::SystemUnload { fleet_id, .. }
            | Command::RefitShips { fleet_id, .. } => *fleet_id,
            Command::MoveShip { ship_id, .. } => *ship_id,
            _ => return None,
        };
        let pos = p.own().find(|f| f.id == fleet)?.pos;
        self.wait(format!("fleet:{fleet}"), p, pos);
        self.status = "local recovery and Scout fitting before the long Hub voyage".into();
        Some(command)
    }
}
