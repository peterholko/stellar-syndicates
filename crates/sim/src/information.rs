//! Stationary reports, shared by presentation and command-center decisions.
//!
//! Ownership grants permission to read a report, not permission to read truth.
//! A site's whole administrative state travels together: loss of ownership,
//! inventory, workforce, build queues and population must share one wavefront.
//! This history is part of the saved simulation, so reconnecting cannot turn a
//! fresh snapshot into supposedly old light. Legacy saves start reporting now;
//! absence of an arrived report means unknown, never a fallback to current truth.

use crate::{BuildJob, EntityId, PlayerId, StarSystem, Vec2, World};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

/// Change-only reports for discrete facts. Moving entities may jump, so select
/// the greatest ARRIVED emission, not the newest value or an arrival-age tier.
/// These streams contain state changes, not a tick-by-tick motion history.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Reports<T> {
    last_pos: Vec2,
    #[serde(default)]
    moving_origin: bool,
    samples: VecDeque<(f64, Vec2, Option<T>)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Commodity, SimConfig};

    fn remote() -> (World, PlayerId, EntityId, Vec2, f64) {
        let mut w = World::new(SimConfig::for_players(71, 4));
        let owner = PlayerId(71);
        w.step(&[Command::AddPlayer {
            id: owner,
            name: "Reports".into(),
        }]);
        let cc = w.players[&owner].command_center;
        let sys = w
            .systems
            .iter_mut()
            .find(|s| s.owner.is_none() && s.pos.distance(cc) > 10_000.0)
            .unwrap();
        sys.owner = Some(owner);
        sys.stockpile.clear();
        sys.stockpile.insert(Commodity::Alloys, 10.0);
        let id = sys.id;
        let delay = crate::transit::delay(sys.pos, cc, w.config.c);
        w.time = 1000.0;
        w.information = InformationHistory::default();
        w.record_information();
        (w, owner, id, cc, delay)
    }

    #[test]
    fn inventory_workforce_construction_and_ownership_share_one_wavefront() {
        let (mut w, owner, id, cc, delay) = remote();
        let initial = w.time;
        assert!(
            w.information
                .site(id, cc, w.config.c, initial + delay - 0.001)
                .is_none()
        );
        w.time += delay + 1.0;
        let change = w.time;
        let s = w.systems.iter_mut().find(|s| s.id == id).unwrap();
        let body = &mut s.bodies[0];
        body.set_tier(crate::StructureKind::MiningComplex, 1);
        body.assignments.insert(
            crate::StructureKind::MiningComplex,
            crate::production::Assignment {
                workers: 3,
                specialists: Default::default(),
                suspended: None,
            },
        );
        s.stockpile.insert(Commodity::Alloys, 85.0);
        s.owner = Some(PlayerId(72));
        w.build_queue.push(BuildJob {
            id: 99,
            owner,
            system: id,
            body_id: body.id,
            what: crate::build::BuildKind::Ship {
                ship: crate::ShipKind::Convoy,
            },
            started_tick: Some(w.tick),
            queued_tick: None, structure_work: None,
            complete_tick: w.tick + 1000,
            ship_work: None,
            join: None,
            loadout: Default::default(),
        });
        w.record_information();
        // Persistence must retain the old private picture AND the future report.
        let restored: InformationHistory =
            serde_json::from_str(&serde_json::to_string(&w.information).unwrap()).unwrap();
        let old = restored
            .site(id, cc, w.config.c, change + delay - 0.001)
            .unwrap();
        assert_eq!(old.system.owner, Some(owner));
        assert_eq!(old.system.stockpile[&Commodity::Alloys], 10.0);
        assert!(old.builds.is_empty());
        assert!(old.system.bodies[0].assignments.is_empty());
        let new = restored.site(id, cc, w.config.c, change + delay).unwrap();
        assert_eq!(new.system.owner, Some(PlayerId(72)));
        assert_eq!(new.system.stockpile[&Commodity::Alloys], 85.0);
        assert_eq!(
            new.system.bodies[0].assignments[&crate::StructureKind::MiningComplex].workers,
            3
        );
        assert_eq!(new.builds[0].id, 99);
        assert_eq!(new.builds[0].started_tick, Some(w.tick));
    }

    #[test]
    fn yard_pause_and_resume_share_the_workforce_reports_wavefront() {
        let (mut w, owner, id, cc, delay) = remote();
        let started = w.time;
        let body = w.systems.iter().find(|s| s.id == id).unwrap().bodies[0].id;
        w.build_queue.push(BuildJob {
            id: 100, owner, system: id, body_id: body,
            what: crate::BuildKind::Ship { ship: crate::ShipKind::Convoy },
            started_tick: Some(w.tick), complete_tick: w.tick + 800,
            queued_tick: None, structure_work: None,
            ship_work: Some(crate::build::BuildWork {
                required: 1000.0, completed: 0.0, at_tick: w.tick, rate: 1.25,
            }), join: None, loadout: Default::default(),
        });
        w.record_information();
        w.tick += 150;
        w.time += 150.0 * crate::DT;
        let pause = w.time;
        w.build_queue[0].ship_work.as_mut().unwrap().set_rate(w.tick, 0.0);
        w.build_queue[0].complete_tick = u64::MAX;
        w.record_information();
        let old = w.information.site(id, cc, w.config.c, pause + delay - 0.001).unwrap();
        assert_eq!(old.at, started);
        assert_eq!(old.builds[0].ship_work.as_ref().unwrap().rate, 1.25, "no early pause disclosure");
        let arrived = w.information.site(id, cc, w.config.c, pause + delay).unwrap();
        assert_eq!(arrived.builds[0].complete_tick, u64::MAX);
        let frozen = arrived.builds[0].ship_work.as_ref().unwrap().completed;
        assert_eq!(frozen, 187.5);
        w.tick += 300;
        w.time += 300.0 * crate::DT;
        let resume = w.time;
        let work = w.build_queue[0].ship_work.as_mut().unwrap();
        work.set_rate(w.tick, 1.25);
        w.build_queue[0].complete_tick = work.completion_tick().unwrap();
        w.record_information();
        let history: InformationHistory = serde_json::from_str(&serde_json::to_string(&w.information).unwrap()).unwrap();
        let old = history.site(id, cc, w.config.c, resume + delay - 0.001).unwrap();
        assert_eq!(old.builds[0].ship_work.as_ref().unwrap().rate, 0.0);
        let new = history.site(id, cc, w.config.c, resume + delay).unwrap();
        assert_eq!(new.builds[0].ship_work.as_ref().unwrap().completed, frozen);
        assert_eq!(new.builds[0].ship_work.as_ref().unwrap().rate, 1.25);
    }

    #[test]
    fn latest_intel_does_not_hide_the_previous_arrived_report() {
        let mut reports = Reports::default();
        let pos = Vec2::new(20_000.0, 0.0);
        reports.record(0.0, pos, Some(10u32), 100.0);
        reports.record(5.0, pos, Some(85u32), 100.0);
        let leg = crate::transit::delay(pos, Vec2::ZERO, 400.0);
        assert_eq!(reports.at(Vec2::ZERO, 400.0, leg + 4.999), Some(&10));
        assert_eq!(reports.at(Vec2::ZERO, 400.0, leg + 5.0), Some(&85));
        reports.record(6.0, pos, None, 100.0);
        assert_eq!(reports.at(Vec2::ZERO, 400.0, leg + 5.999), Some(&85));
        assert_eq!(reports.at(Vec2::ZERO, 400.0, leg + 6.0), None);
    }

    #[test]
    fn pruning_keeps_the_predecessor_and_never_backdates_new_light() {
        let mut reports = Reports::default();
        for time in 0..1000 {
            reports.record(time as f64, Vec2::ZERO, Some(time), 20.0);
        }
        assert_eq!(reports.samples.len(), 22);
        assert_eq!(reports.at(Vec2::ZERO, 400.0, 999.0), Some(&999));
        assert_eq!(reports.at(Vec2::ZERO, 400.0, 0.0), None);
    }
}

impl<T> Default for Reports<T> {
    fn default() -> Self {
        Self {
            last_pos: Vec2::ZERO,
            moving_origin: false,
            samples: VecDeque::new(),
        }
    }
}

impl<T: PartialEq> Reports<T> {
    fn record(&mut self, at: f64, pos: Vec2, value: Option<T>, horizon: f64) {
        self.last_pos = pos;
        if self
            .samples
            .back()
            .is_none_or(|(_, _, last)| *last != value)
        {
            self.moving_origin |= self.samples.back().is_some_and(|(_, old, _)| *old != pos);
            self.samples.push_back((at, pos, value));
        }
        while self.samples.len() > 1 && self.samples[1].0 < at - horizon {
            self.samples.pop_front();
        }
    }

    fn at(&self, cc: Vec2, c: f64, now: f64) -> Option<&T> {
        self.state_at(cc, c, now).and_then(|value| value.as_ref())
    }

    fn state_at(&self, cc: Vec2, c: f64, now: f64) -> Option<&Option<T>> {
        // Stationary streams (including frequently refreshed intel) are indexed
        // by emission time. Only discrete, moving-origin changes need the
        // arrival-order search; never scan a site's tick history in darkness.
        if !self.moving_origin {
            let emission = now - crate::transit::delay(self.samples.front()?.1, cc, c);
            let end = self
                .samples
                .partition_point(|(at, _, _)| *at <= emission + 1e-9);
            return end.checked_sub(1).map(|i| &self.samples[i].2);
        }
        self.samples
            .iter()
            .rev()
            .find(|(time, pos, _)| *time + crate::transit::delay(*pos, cc, c) <= now + 1e-9)
            .map(|(_, _, value)| value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteReport {
    pub at: f64,
    pub tick: u64,
    pub system: StarSystem,
    pub builds: Vec<BuildJob>,
    pub garrison: Option<(u32, bool)>,
    pub node_fed: bool,
    pub academies: Vec<crate::world::AcademyContribution>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InformationHistory {
    sites: BTreeMap<EntityId, VecDeque<SiteReport>>,
    programmes: BTreeMap<PlayerId, VecDeque<(f64, Option<String>)>>,
    research: BTreeMap<u64, Vec<(PlayerId, String, f64)>>,
    emplacements: BTreeMap<EntityId, Reports<crate::Emplacement>>,
    captains: BTreeMap<PlayerId, BTreeMap<u32, Reports<crate::Captain>>>,
    shipments: BTreeMap<crate::tca::ShipmentId, Reports<(crate::tca::Shipment, bool)>>,
    standing: BTreeMap<PlayerId, BTreeMap<u32, Reports<crate::standing::StandingOrder>>>,
    syndicates: BTreeMap<crate::SyndicateId, Reports<crate::syndicate::Syndicate>>,
    intel: BTreeMap<PlayerId, BTreeMap<EntityId, Reports<crate::IntelSnapshot>>>,
    facts: BTreeMap<u64, Vec<(PlayerId, LearnedFact)>>,
    fleet_assets: BTreeMap<EntityId, Reports<FleetAssets>>,
    rankings: VecDeque<(f64, Vec<crate::rankings::RankingRow>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LearnedFact {
    Verb(crate::research::Verb, f64),
    Observed(EntityId),
    Scouted(EntityId),
    Stats(crate::rankings::RankingStats),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetAssets {
    pub id: EntityId,
    pub owner: PlayerId,
    pub mission: Option<crate::ship::TradeMission>,
    pub cargo: Vec<crate::cargo::Cargo>,
    #[serde(default)]
    pub operations: Vec<crate::OperationId>,
}

impl InformationHistory {
    pub fn report_fact(&mut self, arrival: f64, owner: PlayerId, fact: LearnedFact) {
        self.facts
            .entry(arrival.max(0.0).to_bits())
            .or_default()
            .push((owner, fact));
    }

    pub fn arrived_facts(&mut self, now: f64) -> Vec<(PlayerId, LearnedFact)> {
        let mut out = Vec::new();
        while self
            .facts
            .first_key_value()
            .is_some_and(|(at, _)| f64::from_bits(*at) <= now + 1e-9)
        {
            out.extend(self.facts.pop_first().unwrap().1);
        }
        out
    }

    pub fn fleet_assets(&self, cc: Vec2, c: f64, now: f64) -> Vec<&FleetAssets> {
        self.fleet_assets
            .values()
            .filter_map(|r| r.at(cc, c, now))
            .collect()
    }

    pub fn fleet_haul_finished(&self, id: EntityId, receiver: Vec2, c: f64, now: f64) -> bool {
        self.fleet_assets
            .get(&id)
            .and_then(|r| r.state_at(receiver, c, now))
            .is_some_and(|state| state.as_ref().is_none_or(|f| f.mission.is_none()))
    }

    pub fn rankings(
        &self,
        cc: Vec2,
        hub: Vec2,
        c: f64,
        now: f64,
    ) -> &[crate::rankings::RankingRow] {
        let emission = now - crate::transit::delay(hub, cc, c);
        let end = self
            .rankings
            .partition_point(|(at, _)| *at <= emission + 1e-9);
        end.checked_sub(1)
            .map_or(&[], |i| self.rankings[i].1.as_slice())
    }
    pub fn intel(
        &self,
        owner: PlayerId,
        cc: Vec2,
        c: f64,
        now: f64,
    ) -> BTreeMap<EntityId, crate::IntelSnapshot> {
        self.intel
            .get(&owner)
            .into_iter()
            .flat_map(|m| m.iter())
            .filter_map(|(id, r)| r.at(cc, c, now).map(|snap| (*id, *snap)))
            .collect()
    }
    pub fn emplacements(&self, cc: Vec2, c: f64, now: f64) -> Vec<crate::Emplacement> {
        self.emplacements
            .values()
            .filter_map(|r| r.at(cc, c, now).cloned())
            .collect()
    }

    pub fn captains(&self, owner: PlayerId, cc: Vec2, c: f64, now: f64) -> Vec<crate::Captain> {
        self.captains
            .get(&owner)
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(|r| r.at(cc, c, now).cloned())
            .collect()
    }

    pub fn shipments(
        &self,
        owner: PlayerId,
        cc: Vec2,
        c: f64,
        now: f64,
    ) -> Vec<(crate::tca::Shipment, bool)> {
        self.shipments
            .values()
            .filter_map(|r| r.at(cc, c, now).copied())
            .filter(|(s, _)| s.owner == owner)
            .collect()
    }

    pub fn standing(
        &self,
        owner: PlayerId,
        cc: Vec2,
        c: f64,
        now: f64,
    ) -> Vec<crate::standing::StandingOrder> {
        self.standing
            .get(&owner)
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(|r| r.at(cc, c, now).copied())
            .collect()
    }

    pub fn syndicates(&self, cc: Vec2, c: f64, now: f64) -> Vec<crate::syndicate::Syndicate> {
        self.syndicates
            .values()
            .filter_map(|r| r.at(cc, c, now).cloned())
            .collect()
    }
    pub fn record_programme(&mut self, owner: PlayerId, now: f64, active: Option<String>) {
        let history = self.programmes.entry(owner).or_default();
        if history.back().is_none_or(|(_, last)| *last != active) {
            history.push_back((now, active));
        }
    }

    pub fn programme_at(&self, owner: PlayerId, at: f64) -> Option<&str> {
        let history = self.programmes.get(&owner)?;
        let end = history.partition_point(|(time, _)| *time <= at + 1e-9);
        end.checked_sub(1).and_then(|i| history[i].1.as_deref())
    }

    pub fn research_report(
        &mut self,
        arrival: f64,
        owner: PlayerId,
        programme: String,
        amount: f64,
    ) {
        self.research
            .entry(arrival.max(0.0).to_bits())
            .or_default()
            .push((owner, programme, amount));
    }

    pub fn arrived_research(&mut self, now: f64) -> Vec<(PlayerId, String, f64)> {
        let mut arrived = Vec::new();
        while self
            .research
            .first_key_value()
            .is_some_and(|(at, _)| f64::from_bits(*at) <= now + 1e-9)
        {
            arrived.extend(self.research.pop_first().unwrap().1);
        }
        arrived
    }
    pub fn systems_for(&self, world: &World, viewer: PlayerId) -> Vec<StarSystem> {
        let Some(corp) = world.players.get(&viewer) else {
            return Vec::new();
        };
        world
            .systems
            .iter()
            .map(|system| {
                if let Some(report) =
                    self.site(system.id, corp.command_center, world.config.c, world.time)
                {
                    return report.system.clone();
                }
                // Static astronomy is public. The first report may still be in
                // flight after a legacy-save upgrade; reveal no live ownership or
                // private economics to fill that gap.
                let mut unknown = system.clone();
                unknown.owner = None;
                unknown.claimed_at = None;
                unknown.blockade = None;
                unknown.blockade_prev = None;
                unknown.stockpile.clear();
                unknown.modules.clear();
                unknown.specialists.clear();
                for body in &mut unknown.bodies {
                    body.population = 0.0;
                    body.inbound_migrants = 0;
                    body.structures.clear();
                    body.assignments.clear();
                }
                unknown
            })
            .collect()
    }

    pub fn record(&mut self, world: &World, academies: Vec<crate::world::AcademyContribution>) {
        let horizon =
            world.config.galaxy_radius * 2.5 / crate::transit::signal_speed(world.config.c) + 1.0;
        if self
            .rankings
            .back()
            .is_none_or(|(_, rows)| *rows != world.rankings)
        {
            self.rankings
                .push_back((world.time, world.rankings.clone()));
        }
        while self.rankings.len() > 1 && self.rankings[1].0 < world.time - horizon {
            self.rankings.pop_front();
        }
        for (&id, fleet) in &world.fleets {
            self.fleet_assets.entry(id).or_default().record(
                world.time,
                fleet.pos,
                Some(FleetAssets {
                    id,
                    owner: fleet.owner,
                    mission: fleet.mission,
                    cargo: fleet.cargo_stacks(),
                    operations: world
                        .operations
                        .values()
                        .filter(|op| op.assigned_fleets.get(&fleet.owner) == Some(&id))
                        .map(|op| op.id)
                        .collect(),
                }),
                horizon,
            );
        }
        for (id, history) in &mut self.fleet_assets {
            if !world.fleets.contains_key(id) {
                history.record(world.time, history.last_pos, None, horizon);
            }
        }
        for e in &world.emplacements {
            self.emplacements.entry(e.id).or_default().record(
                world.time,
                e.pos,
                Some(e.clone()),
                horizon,
            );
        }
        for (id, history) in &mut self.emplacements {
            if !world.emplacements.iter().any(|e| e.id == *id) {
                history.record(world.time, history.last_pos, None, horizon);
            }
        }
        for (&owner, corp) in &world.players {
            for (&system, snapshot) in &corp.intel {
                self.intel
                    .entry(owner)
                    .or_default()
                    .entry(system)
                    .or_default()
                    .record(snapshot.observed_at, snapshot.pos, Some(*snapshot), horizon);
            }
            let histories = self.captains.entry(owner).or_default();
            for (&id, captain) in &corp.captains {
                let history = histories.entry(id).or_default();
                // Loss/recovery has its own already-priced report. Until it
                // arrives, retain the previous officer and assignment intact.
                if captain.missing_report_at.is_some_and(|at| world.time < at) {
                    continue;
                }
                let pos = if captain.missing_report_at.is_some() {
                    corp.command_center
                } else {
                    captain
                        .assigned_fleet
                        .and_then(|id| world.fleets.get(&id).map(|f| f.pos))
                        .or_else(|| {
                            captain.stationed_system.and_then(|id| {
                                world.systems.iter().find(|s| s.id == id).map(|s| s.pos)
                            })
                        })
                        .unwrap_or(history.last_pos)
                };
                history.record(world.time, pos, Some(captain.clone()), horizon);
            }
            for (id, history) in histories {
                if !corp.captains.contains_key(id) {
                    history.record(world.time, history.last_pos, None, horizon);
                }
            }
            let histories = self.standing.entry(owner).or_default();
            for order in &corp.standing_orders {
                let pos = order
                    .source
                    .system_id()
                    .and_then(|id| world.systems.iter().find(|s| s.id == id).map(|s| s.pos))
                    .unwrap_or(corp.command_center);
                // Evaluation cadence is private scheduler bookkeeping, not news.
                let mut report = *order;
                report.next_eval_tick = 0;
                histories.entry(order.id).or_default().record(
                    world.time,
                    pos,
                    Some(report),
                    horizon,
                );
            }
            for (id, history) in histories {
                if !corp.standing_orders.iter().any(|o| o.id == *id) {
                    history.record(world.time, history.last_pos, None, horizon);
                }
            }
        }
        let mut present_shipments = std::collections::BTreeSet::new();
        for (&owner, _) in &world.players {
            for (shipment, aboard) in world.shipments_of(owner) {
                present_shipments.insert(shipment.id);
                let pos = world
                    .freight_runs
                    .values()
                    .find(|run| run.shipments.contains_key(&shipment.id))
                    .and_then(|run| world.fleets.get(&run.fleet).map(|f| f.pos))
                    .unwrap_or_else(|| {
                        if shipment.direction == crate::tca::ShipmentDir::Inbound {
                            world
                                .systems
                                .iter()
                                .find(|s| s.id == shipment.system)
                                .map_or(world.hub, |s| s.pos)
                        } else {
                            world.hub
                        }
                    });
                self.shipments.entry(shipment.id).or_default().record(
                    world.time,
                    pos,
                    Some((shipment, aboard)),
                    horizon,
                );
            }
        }
        for (id, history) in &mut self.shipments {
            if !present_shipments.contains(id) {
                history.record(world.time, history.last_pos, None, horizon);
            }
        }
        for (&id, syndicate) in &world.syndicates {
            let pos = world
                .players
                .get(&syndicate.founder)
                .map_or(world.hub, |c| c.command_center);
            self.syndicates.entry(id).or_default().record(
                world.time,
                pos,
                Some(syndicate.clone()),
                horizon,
            );
        }
        for (id, history) in &mut self.syndicates {
            if !world.syndicates.contains_key(id) {
                history.record(world.time, history.last_pos, None, horizon);
            }
        }
        for system in &world.systems {
            let mut system = system.clone();
            // Only whole inventory units go onto the wire. Quantize before
            // change compression, not the physical stockpile: fractional mine
            // ticks must not allocate a full planetary report 30 times/second.
            for amount in system.stockpile.values_mut() {
                *amount = amount.floor();
            }
            let builds: Vec<_> = world
                .build_queue
                .iter()
                .filter(|job| job.system == system.id)
                .cloned()
                .collect();
            let garrison = world.hosted_garrison(system.id);
            let node_fed = world.nodes.get(&system.id).is_some_and(|n| n.fed);
            let academies: Vec<_> = academies
                .iter()
                .filter(|a| a.system == system.id)
                .cloned()
                .collect();
            let history = self.sites.entry(system.id).or_default();
            if history.back().is_none_or(|last| {
                last.system != system
                    || last.builds != builds
                    || last.garrison != garrison
                    || last.node_fed != node_fed
                    || last.academies != academies
            }) {
                history.push_back(SiteReport {
                    at: world.time,
                    tick: world.tick,
                    system,
                    builds,
                    garrison,
                    node_fed,
                    academies,
                });
            }
            // Retain the predecessor of the horizon, including its old flag.
            // Searching a stationary stream is logarithmic, not a darkness scan.
            while history.len() > 1 && history[1].at < world.time - horizon {
                history.pop_front();
            }
        }
    }

    pub fn site(&self, id: EntityId, cc: Vec2, c: f64, now: f64) -> Option<&SiteReport> {
        let history = self.sites.get(&id)?;
        let pos = history.front()?.system.pos;
        let emission = now - crate::transit::delay(pos, cc, c);
        let end = history.partition_point(|report| report.at <= emission + 1e-9);
        end.checked_sub(1).map(|i| &history[i])
    }

    pub fn owned_sites(
        &self,
        owner: PlayerId,
        cc: Vec2,
        c: f64,
        now: f64,
    ) -> impl Iterator<Item = &SiteReport> {
        self.sites
            .keys()
            .filter_map(move |id| self.site(*id, cc, c, now))
            .filter(move |r| r.system.owner == Some(owner))
    }
}
