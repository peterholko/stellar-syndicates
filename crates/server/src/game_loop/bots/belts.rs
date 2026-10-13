//! §belts phase 6: once the charter's barge lesson is done, bots keep their
//! Mining Barges working, prospect their belts, and run a Mining Belt Station
//! with a hauler at their best remote belt. Arrived reports only, as a player.
use super::*;
use sim::industry::{AnchorPhase, FleetIndustry, FreightPort, FreightRoute, RoutePhase, RouteStop};

/// A hostile warship this close to a belt keeps barges and stations away:
/// the 1,500 su they work in, plus a buffer. A barge is cheap; a picket
/// further out mustn't idle it for hours.
pub(super) const BELT_THREAT_SU: f64 = 6_000.0;
/// A barge hauls its own ore home only from a belt this close.
const REMOTE_BELT_SU: f64 = 30_000.0;
/// A station may sit a little further out: a freighter does the hauling.
const STATION_BELT_SU: f64 = 40_000.0;
/// Prospect a belt this often before trusting its pocket list.
const PROSPECT_PASSES: u32 = 2;
/// Barges a station's belt is worth.
const STATION_BARGES: u32 = 3;
/// A barge's working pace, for comparing belts: full-speed su a second.
const BARGE_SU_PER_S: f64 = 150.0;

/// A charted belt as this corporation's arrived reports know it.
#[derive(Clone, Debug)]
pub(super) struct BeltSite {
    pub system: EntityId,
    pub body: u32,
    pub pos: Vec2,
    pub owner: Option<PlayerId>,
    /// (ore, pocket, richness, reserves): the belt's ores and the pockets we found.
    pub ores: Vec<(C, bool, f64, Option<f64>)>,
    /// Prospecting passes reported back.
    pub prospected: u32,
}

impl BeltSite {
    /// The ore a barge earns most from here: (ore, pocket, credits a barge-second).
    pub(super) fn best(&self) -> Option<(C, bool, f64)> {
        self.ores
            .iter()
            .filter(|(_, _, richness, left)| *richness > 0.0 && left.is_none_or(|n| n > 1.0))
            .map(|&(ore, pocket, richness, _)| {
                let cut = sim::industry::BARGE_ORE_PER_S.min(sim::industry::SHIP_YIELD_MULT * richness);
                (ore, pocket, cut * sim::production::ore_bulk_ratio(ore) * sim::market::base_price(ore))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))
    }
}

impl Observation {
    /// Charted belts we may work: unowned or our own, not threatened.
    fn workable_belts(&self) -> impl Iterator<Item = &BeltSite> {
        self.belts.iter().filter(|b| {
            (b.owner.is_none() || b.owner == Some(self.owner)) && !self.threatened_within(b.pos, BELT_THREAT_SU) && b.best().is_some()
        })
    }

    /// The belt a station is worth most at: unowned, within reach, by value.
    pub(super) fn station_belt(&self) -> Option<&BeltSite> {
        let home = self.home.system.pos;
        self.workable_belts()
            .filter(|b| b.owner.is_none() && b.pos.distance(home) <= STATION_BELT_SU)
            .max_by(|a, b| a.best().unwrap().2.total_cmp(&b.best().unwrap().2).then(b.system.cmp(&a.system)))
    }

    /// Our station anchored at a belt (or on its way to one), if any.
    fn our_station(&self) -> Option<&GhostView> {
        self.own().find(|g| g.kind == H::MiningBeltStation && g.anchor.is_some())
    }

    /// Is the founding charter still running? Belt work waits for it to end,
    /// barge lesson included.
    pub(super) fn charter_running(&self) -> bool {
        self.founding_stage != sim::FoundingStage::Complete
    }

    /// Barges this corporation wants: one for the home belt, more at a station.
    pub(super) fn barge_need(&self) -> u32 {
        if self.charter_running() || self.workable_belts().next().is_none() {
            return 0;
        }
        let anchored = self.our_station().is_some_and(|s| s.anchor.is_some_and(|a| a.phase == AnchorPhase::Anchored));
        if anchored { STATION_BARGES } else { 1 }
    }

    /// A station is worth building: barges at work and an unowned belt to anchor at.
    pub(super) fn wants_station(&self) -> bool {
        !self.charter_running() && self.count(H::MiningBarge) > 0 && self.station_belt().is_some()
    }
}

impl Bot {
    /// §belts phase 6: one belt command per think, after the charter lesson.
    pub(super) fn belt_work(&mut self, p: &Observation) -> Option<Command> {
        if p.charter_running() || p.home.system.id != p.capital {
            return None;
        }
        self.belt_haulers.retain(|fleet, station| {
            p.own().any(|g| g.id == *fleet) && p.own().any(|g| g.id == *station && g.anchor.is_some())
        });
        if let Some(command) = self.anchor_station(p) {
            return Some(command);
        }
        if let Some(command) = self.belt_hauler(p) {
            return Some(command);
        }
        let barges: Vec<GhostView> = p.own().filter(|g| g.kind == H::MiningBarge).cloned().collect();
        for barge in &barges {
            if let Some(command) = self.barge_step(p, barge) {
                return Some(command);
            }
        }
        None
    }

    /// One barge: unload at home, come home when full, prospect, then mine
    /// the best belt it can reach.
    fn barge_step(&mut self, p: &Observation, barge: &GhostView) -> Option<Command> {
        let key = format!("fleet:{}", barge.id);
        if !self.ready(&key, p.now) || p.fighting.contains(&barge.id) || barge.stalled {
            return None;
        }
        let home = &p.home.system;
        let at_home = barge.docked.as_deref() == Some(home.id.to_string().as_str());
        let aboard = cargo_used(barge);
        // Our own systems' belts first: every home belt hides a pocket, and
        // its two passes almost always find it, while a remote belt with
        // richer ore usually hides nothing.
        let own_unprospected = p.belts.iter().filter(|b| b.owner == Some(p.owner)
            && b.prospected < PROSPECT_PASSES && !b.ores.iter().any(|(_, pocket, _, _)| *pocket)
            && self.ready(&format!("prospect:{}:{}", b.system, b.body), p.now)
            && !p.threatened_within(b.pos, BELT_THREAT_SU))
            .min_by(|a, b| a.pos.distance(barge.pos).total_cmp(&b.pos.distance(barge.pos)).then(a.system.cmp(&b.system)))
            .map(|b| (b.system, b.body, b.pos));
        // A barge that has worked a home deposit or pocket dry looks for new
        // work like an idle one; away, it first comes home.
        let depleted_home = matches!(&barge.industry, Some(FleetIndustry::Mine { system, status, .. })
            if *system == home.id && status == "depleted");
        let command = match &barge.industry {
            Some(FleetIndustry::Prospect { .. }) => return None,
            Some(FleetIndustry::Mine { system, status, .. }) if !depleted_home => {
                let away = *system != home.id;
                let done = matches!(status.as_str(), "full" | "station full" | "depleted" | "claimed");
                if !away || !done {
                    return None; // working, travelling, waiting out danger, or at home
                }
                self.status = "belts: bringing a barge home".into();
                Command::MoveShip { player_id: p.owner, ship_id: barge.id, dest: home.pos }
            }
            _ if barge.vel.length_sq() > 1.0 => return None, // under way
            _ if at_home && aboard > 0 => {
                self.status = "belts: unloading a barge's ore at home".into();
                Command::SystemUnload { player_id: p.owner, fleet_id: barge.id, system: home.id }
            }
            _ if !at_home && aboard > 0 => {
                self.status = "belts: bringing a barge's ore home".into();
                Command::MoveShip { player_id: p.owner, ship_id: barge.id, dest: home.pos }
            }
            _ if own_unprospected.is_some() => {
                let (system, body, pos) = own_unprospected.unwrap();
                self.wait(format!("prospect:{system}:{body}"), p, pos);
                self.status = "belts: prospecting our own belt for its pocket".into();
                Command::ProspectBelt { player_id: p.owner, fleet_id: barge.id, system_id: system, body_id: body }
            }
            _ => {
                let (belt, ore, pocket) = self.barge_target(p, barge)?;
                // A fresh belt is prospected once before the barge settles in:
                // a found pocket may beat its ores.
                let prospect_key = format!("prospect:{}:{}", belt.system, belt.body);
                let passes = if belt.system == home.id { PROSPECT_PASSES } else { 1 };
                let found = belt.ores.iter().any(|(_, pocket, _, _)| *pocket);
                if belt.prospected < passes && !found && self.ready(&prospect_key, p.now) {
                    self.wait(prospect_key, p, belt.pos);
                    self.status = "belts: prospecting a belt for hidden pockets".into();
                    Command::ProspectBelt { player_id: p.owner, fleet_id: barge.id, system_id: belt.system, body_id: belt.body }
                } else {
                    self.status = "belts: sending a barge to the best belt".into();
                    Command::MineBelt { player_id: p.owner, fleet_id: barge.id, system_id: belt.system,
                        body_id: belt.body, ore, pocket }
                }
            }
        };
        self.wait(key, p, barge.pos);
        Some(command)
    }

    /// The belt this barge earns most at. Our own system's belts and a belt
    /// with our station need no hauling; another belt pays only for the time
    /// the barge spends cutting rather than flying its ore home.
    fn barge_target(&self, p: &Observation, barge: &GhostView) -> Option<(BeltSite, C, bool)> {
        let home = p.home.system.pos;
        let station = p.our_station().and_then(|s| s.anchor).filter(|a| a.phase == AnchorPhase::Anchored);
        let crews = |b: &BeltSite| p.own().filter(|g| matches!(&g.industry,
            Some(FleetIndustry::Mine { system, body, .. }) if *system == b.system && *body == b.body)).count();
        p.workable_belts()
            .filter_map(|b| {
                let (ore, pocket, value) = b.best()?;
                let ours = b.owner == Some(p.owner) || station.is_some_and(|a| a.system == b.system && a.body == b.body);
                let distance = b.pos.distance(home);
                if !ours && distance > REMOTE_BELT_SU {
                    return None;
                }
                let efficiency = if ours {
                    1.0
                } else {
                    let rate = sim::industry::BARGE_ORE_PER_S * sim::production::ore_bulk_ratio(ore);
                    let fill = capacity(barge) as f64 / rate.max(1e-6);
                    fill / (fill + 2.0 * distance / BARGE_SU_PER_S)
                };
                // Barges share a deposit's yield: don't pile onto one belt.
                let share = 1.0 / (1.0 + crews(b) as f64 * 0.5);
                Some((value * efficiency * share, b, ore, pocket))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.system.cmp(&a.1.system)))
            .map(|(_, b, ore, pocket)| (b.clone(), ore, pocket))
    }

    /// A free station at home anchors at the best unowned belt in reach.
    fn anchor_station(&mut self, p: &Observation) -> Option<Command> {
        let station = p.own().find(|g| g.kind == H::MiningBeltStation && g.anchor.is_none()
            && warfare::composition(g).values().sum::<u32>() == 1)?;
        let key = format!("fleet:{}", station.id);
        if !self.ready(&key, p.now) || p.fighting.contains(&station.id) || station.vel.length_sq() > 1.0 {
            return None;
        }
        let belt = p.station_belt()?;
        let command = Command::AnchorStation { player_id: p.owner, fleet_id: station.id,
            system_id: belt.system, body_id: belt.body };
        self.wait(key, p, station.pos);
        self.status = "belts: anchoring a Mining Belt Station at the best belt".into();
        Some(command)
    }

    /// One freighter runs station → home on repeat, carrying the barges' ore.
    fn belt_hauler(&mut self, p: &Observation) -> Option<Command> {
        let station = p.our_station().filter(|s| s.anchor.is_some_and(|a| a.phase == AnchorPhase::Anchored))?;
        let anchor = station.anchor?;
        let home = &p.home.system;
        // The ore the barges cut there (else the station's largest stack).
        let ore = p.own().filter_map(|g| match &g.industry {
            Some(FleetIndustry::Mine { system, body, ore, .. }) if *system == anchor.system && *body == anchor.body => Some(*ore),
            _ => None,
        }).next().or_else(|| station.cargo_manifest.iter().max_by_key(|c| c.units).map(|c| c.commodity))?;
        let running = |g: &GhostView| matches!(&g.industry, Some(FleetIndustry::Route { run })
            if run.phase != RoutePhase::Complete
                && run.route.stops.first().is_some_and(|s| s.port == FreightPort::Station { id: station.id })
                && run.route.stops.first().is_some_and(|s| s.load.contains_key(&ore)));
        if let Some((&hauler, _)) = self.belt_haulers.iter().find(|(_, s)| **s == station.id)
            && p.own().find(|g| g.id == hauler).is_some_and(running)
        {
            return None; // on its run
        }
        // The assigned hauler (else an idle freighter at home) takes the run.
        let assigned = self.belt_haulers.iter().find(|(_, s)| **s == station.id).map(|(f, _)| *f);
        let fleet = p.own().filter(|g| g.kind.is_player_freighter() && !g.stalled
            && g.docked.as_deref() == Some(home.id.to_string().as_str()) && !p.fighting.contains(&g.id)
            && self.ready(&format!("fleet:{}", g.id), p.now)
            && !self.supply_dispatches.contains_key(&g.id))
            .find(|g| assigned == Some(g.id) || assigned.is_none()
                && !matches!(&g.industry, Some(FleetIndustry::Route { run }) if run.phase != RoutePhase::Complete))?
            .clone();
        let mut economy = fleet.clone();
        economy.transit = Some(p.freight_transit());
        let n = capacity(&fleet).min(p.safe_load(&economy, station.pos, home.pos));
        if n < 20 {
            return None;
        }
        let key = format!("fleet:{}", fleet.id);
        self.wait(key, p, fleet.pos);
        self.belt_haulers.insert(fleet.id, station.id);
        self.status = "belts: running a freighter between the station and home".into();
        Some(Command::SetFreightRoute { player_id: p.owner, fleet_id: fleet.id, route: Some(FreightRoute {
            name: "Belt station run".into(), repeat: true, shopping_budget: None,
            transit: p.freight_transit(), fuel_reserve: p.arrival_reserve(&fleet), escort: None,
            stops: vec![
                RouteStop { port: FreightPort::Station { id: station.id }, load: BTreeMap::from([(ore, n)]),
                    unload: BTreeMap::new(), sell: false },
                RouteStop { port: FreightPort::System { id: home.id }, load: BTreeMap::new(),
                    unload: BTreeMap::from([(ore, n)]), sell: false },
            ],
        }) })
    }
}
