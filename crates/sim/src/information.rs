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

#[cfg(test)]
mod retirement_tests;

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

    #[test]
    fn production_modifiers_arrive_with_the_factory_report_and_survive_save() {
        let (mut w, owner, id, cc, delay) = remote();
        let changed = w.time + delay + 1.0;
        w.time = changed;
        w.players.get_mut(&owner).unwrap().research.completed.insert("mat_ore_recovery".into());
        w.players.get_mut(&owner).unwrap().research.completed.insert("mat_enrichment".into());
        w.record_information();
        let w: World = serde_json::from_value(serde_json::to_value(&w).unwrap()).unwrap();
        let before = w.information.site(id,cc,w.config.c,changed+delay-0.001).unwrap();
        assert_eq!(before.production_mods,crate::production::ProductionMods::default(),
            "current corporate research cannot upgrade a still-old factory report");
        let after = w.information.site(id,cc,w.config.c,changed+delay).unwrap();
        assert_eq!(after.production_mods.processing,1.15);
        assert_eq!(after.production_mods.ore_recovery,1.15);
        assert_eq!(after.at,changed);
        let mut legacy = serde_json::to_value(after).unwrap();
        legacy.as_object_mut().unwrap().remove("production_mods");
        let restored: SiteReport = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.production_mods,crate::production::ProductionMods::default());
    }

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
                refining_ore: None,
                workers: 3,
                specialists: Default::default(),
                suspended: None,
            },
        );
        s.stockpile.insert(Commodity::Alloys, 85.0);
        s.owner = Some(PlayerId(72));
        w.build_queue.push(BuildJob {
            id: 99,
            construction_yard: None,
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
            construction_yard: None,
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
        self.trim(at, horizon);
    }

    /// [`Self::record`] of a present value, cloning it only when it differs from
    /// the retained report. Same retained stream, no copy for unchanged state.
    fn record_present(&mut self, at: f64, pos: Vec2, value: &T, horizon: f64)
    where
        T: Clone,
    {
        self.last_pos = pos;
        if self
            .samples
            .back()
            .is_none_or(|(_, _, last)| last.as_ref() != Some(value))
        {
            self.moving_origin |= self.samples.back().is_some_and(|(_, old, _)| *old != pos);
            self.samples.push_back((at, pos, Some(value.clone())));
        }
        self.trim(at, horizon);
    }

    /// The bookkeeping [`Self::record`] performs when the value is unchanged.
    fn touch(&mut self, at: f64, pos: Vec2, horizon: f64) {
        self.last_pos = pos;
        self.trim(at, horizon);
    }

    fn trim(&mut self, at: f64, horizon: f64) {
        while self.samples.len() > 1 && self.samples[1].0 < at - horizon {
            self.samples.pop_front();
        }
    }

    /// A final absence must outlive the same full light-history window as its
    /// preceding reports. A live, unchanged value never expires this way.
    /// Trimming samples alone leaves one tombstone in every dead entity's map
    /// slot forever; callers also remove that slot once this receipt is old.
    fn expired_absence(&self, now: f64, horizon: f64) -> Option<(f64, Vec2)> {
        self.samples.back().and_then(|(at, pos, value)| {
            (value.is_none() && *at + horizon < now).then_some((*at, *pos))
        })
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
    #[serde(default)]
    pub production_mods: crate::production::ProductionMods,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InformationHistory {
    sites: BTreeMap<EntityId, VecDeque<SiteReport>>,
    programmes: BTreeMap<PlayerId, VecDeque<(f64, Option<String>)>>,
    research: BTreeMap<u64, Vec<(PlayerId, String, f64)>>,
    emplacements: BTreeMap<EntityId, Reports<crate::Emplacement>>,
    /// Interdiction fields are public: anyone sees one once its light arrives.
    interdictions: BTreeMap<EntityId, Reports<crate::interdiction::InterdictionField>>,
    captains: BTreeMap<PlayerId, BTreeMap<u32, Reports<crate::Captain>>>,
    shipments: BTreeMap<crate::tca::ShipmentId, Reports<(crate::tca::Shipment, bool)>>,
    standing: BTreeMap<PlayerId, BTreeMap<u32, Reports<crate::standing::StandingOrder>>>,
    syndicates: BTreeMap<crate::SyndicateId, Reports<crate::syndicate::Syndicate>>,
    intel: BTreeMap<PlayerId, BTreeMap<EntityId, Reports<crate::IntelSnapshot>>>,
    facts: BTreeMap<u64, Vec<(PlayerId, LearnedFact)>>,
    fleet_assets: BTreeMap<EntityId, Reports<FleetAssets>>,
    rankings: VecDeque<(f64, Vec<crate::rankings::RankingRow>)>,
    /// Compact completion receipts, queried only by fleet id; never traversed
    /// by record() or fleet_assets(). Keep the original emission/position so an
    /// old standing-order latch still resolves after cleanup/load, without
    /// equating an unknown id with a completed haul or revealing a loss early.
    /// The enclosing serde(default) also accepts pre-cleanup saved histories.
    fleet_completions: BTreeMap<EntityId, (f64, Vec2)>,
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
    /// Add the landing module using each historical report's own population.
    /// Do not import present tiers, ownership, inventory or timestamps.
    pub(crate) fn migrate_command_centers(&mut self) {
        for report in self.sites.values_mut().flat_map(|reports| reports.iter_mut()) {
            if report.system.owner.is_some() { report.system.seed_command_centers(); }
        }
    }

    /// Re-express only the old report's own base storage as a building. Do not
    /// copy current inventory, ownership, queues or timestamps into delayed light.
    pub(crate) fn migrate_ground_storage(&mut self) {
        for report in self.sites.values_mut().flat_map(|reports| reports.iter_mut()) {
            if report.system.owner.is_some() {
                report.system.seed_warehouse();
            }
        }
    }

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
        if let Some(history) = self.fleet_assets.get(&id) {
            history.state_at(receiver, c, now)
                .is_some_and(|state| state.as_ref().is_none_or(|f| f.mission.is_none()))
        } else {
            self.fleet_completions.get(&id).is_some_and(|(at, pos)| {
                *at + crate::transit::delay(*pos, receiver, c) <= now + 1e-9
            })
        }
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

    /// The interdiction fields whose light has reached `cc`, as they were then.
    pub fn interdiction_fields(&self, cc: Vec2, c: f64, now: f64) -> Vec<crate::interdiction::InterdictionField> {
        self.interdictions
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
                    body.founding_camp = None;
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
        // One pass over the operations table, in its own order, instead of one
        // filtered scan of it per fleet per tick.
        let mut operations_by_assignment: BTreeMap<(PlayerId, EntityId), Vec<crate::OperationId>> =
            BTreeMap::new();
        for operation in world.operations.values() {
            for (&owner, &fleet) in &operation.assigned_fleets {
                operations_by_assignment
                    .entry((owner, fleet))
                    .or_default()
                    .push(operation.id);
            }
        }
        for (&id, fleet) in &world.fleets {
            let history = match self.fleet_assets.entry(id) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    self.fleet_completions.remove(&id);
                    entry.insert(Reports::default())
                }
            };
            let operations = operations_by_assignment
                .get(&(fleet.owner, id))
                .map_or(&[][..], Vec::as_slice);
            // Compare against the retained report before building a new one.
            let unchanged = history.samples.back().is_some_and(|(_, _, last)| {
                last.as_ref().is_some_and(|assets| {
                    assets.id == id
                        && assets.owner == fleet.owner
                        && assets.mission == fleet.mission
                        && assets.operations == operations
                        && fleet.cargo_stacks_matches(&assets.cargo)
                })
            });
            if unchanged {
                history.touch(world.time, fleet.pos, horizon);
            } else {
                history.record(
                    world.time,
                    fleet.pos,
                    Some(FleetAssets {
                        id,
                        owner: fleet.owner,
                        mission: fleet.mission,
                        cargo: fleet.cargo_stacks(),
                        operations: operations.to_vec(),
                    }),
                    horizon,
                );
            }
        }
        // The hot maps contain only live entities and disappearance reports
        // still inside the light window, not every fleet/shipment ever made.
        // Legacy tombstones are retired on the first post-load record pass.
        self.fleet_assets.retain(|id, history| {
            if world.fleets.contains_key(id) { return true; }
            history.record(world.time, history.last_pos, None, horizon);
            if let Some(receipt) = history.expired_absence(world.time, horizon) {
                self.fleet_completions.insert(*id, receipt);
                false
            } else {
                true
            }
        });
        for e in &world.emplacements {
            self.emplacements
                .entry(e.id)
                .or_default()
                .record_present(world.time, e.pos, e, horizon);
        }
        self.emplacements.retain(|id, history| {
            if world.emplacements.iter().any(|e| e.id == *id) { return true; }
            history.record(world.time, history.last_pos, None, horizon);
            history.expired_absence(world.time, horizon).is_none()
        });
        for f in world.interdictions.values() {
            self.interdictions
                .entry(f.id)
                .or_default()
                .record_present(world.time, f.pos, f, horizon);
        }
        self.interdictions.retain(|id, history| {
            if world.interdictions.contains_key(id) { return true; }
            history.record(world.time, history.last_pos, None, horizon);
            history.expired_absence(world.time, horizon).is_none()
        });
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
                history.record_present(world.time, pos, captain, horizon);
            }
            histories.retain(|id, history| {
                if corp.captains.contains_key(id) { return true; }
                history.record(world.time, history.last_pos, None, horizon);
                history.expired_absence(world.time, horizon).is_none()
            });
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
            histories.retain(|id, history| {
                if corp.standing_orders.iter().any(|o| o.id == *id) { return true; }
                history.record(world.time, history.last_pos, None, horizon);
                history.expired_absence(world.time, horizon).is_none()
            });
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
        self.shipments.retain(|id, history| {
            if present_shipments.contains(id) { return true; }
            history.record(world.time, history.last_pos, None, horizon);
            history.expired_absence(world.time, horizon).is_none()
        });
        for (&id, syndicate) in &world.syndicates {
            let pos = world
                .players
                .get(&syndicate.founder)
                .map_or(world.hub, |c| c.command_center);
            self.syndicates
                .entry(id)
                .or_default()
                .record_present(world.time, pos, syndicate, horizon);
        }
        self.syndicates.retain(|id, history| {
            if world.syndicates.contains_key(id) { return true; }
            history.record(world.time, history.last_pos, None, horizon);
            history.expired_absence(world.time, horizon).is_none()
        });
        // Group the per-site inputs once per tick, in their source order, and
        // resolve each owner's research modifiers once, instead of a filtered
        // copy and three modifier folds per system per tick.
        let mut builds_by_system: BTreeMap<EntityId, Vec<&BuildJob>> = BTreeMap::new();
        for job in &world.build_queue {
            builds_by_system.entry(job.system).or_default().push(job);
        }
        let mut academies_by_system: BTreeMap<EntityId, Vec<&crate::world::AcademyContribution>> =
            BTreeMap::new();
        for contribution in &academies {
            academies_by_system
                .entry(contribution.system)
                .or_default()
                .push(contribution);
        }
        let mut production_mods_by_owner: BTreeMap<PlayerId, crate::production::ProductionMods> =
            BTreeMap::new();
        for system in &world.systems {
            let production_mods = system
                .owner
                .map(|owner| {
                    *production_mods_by_owner
                        .entry(owner)
                        .or_insert_with(|| world.production_mods(owner))
                })
                .unwrap_or_default();
            let builds = builds_by_system
                .get(&system.id)
                .map_or(&[][..], Vec::as_slice);
            let site_academies = academies_by_system
                .get(&system.id)
                .map_or(&[][..], Vec::as_slice);
            let garrison = world.hosted_garrison(system.id);
            let node_fed = world.nodes.get(&system.id).is_some_and(|n| n.fed);
            let history = self.sites.entry(system.id).or_default();
            // Compare against the retained report before copying anything: an
            // unchanged site costs a read, not a full planetary clone per tick.
            let unchanged = history.back().is_some_and(|last| {
                last.garrison == garrison
                    && last.node_fed == node_fed
                    && last.production_mods == production_mods
                    && last.builds.iter().eq(builds.iter().copied())
                    && last.academies.iter().eq(site_academies.iter().copied())
                    && site_system_matches(&last.system, system)
            });
            if !unchanged {
                let mut system = system.clone();
                // Only whole inventory units go onto the wire. Quantize before
                // change compression, not the physical stockpile: fractional mine
                // ticks must not allocate a full planetary report 30 times/second.
                for amount in system.stockpile.values_mut() {
                    *amount = amount.floor();
                }
                history.push_back(SiteReport {
                    at: world.time,
                    tick: world.tick,
                    system,
                    builds: builds.iter().map(|job| (*job).clone()).collect(),
                    garrison,
                    node_fed,
                    academies: site_academies.iter().map(|a| (*a).clone()).collect(),
                    production_mods,
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

/// Does the retained site report's system still describe `live`? The report
/// stores the stockpile floored to whole units, so that map is compared
/// floored; every other field is compared exactly as `StarSystem`'s derived
/// equality would. The field list is deliberately exhaustive: adding a field
/// to `StarSystem` must be reflected here, or this stops compiling.
fn site_system_matches(stored: &StarSystem, live: &StarSystem) -> bool {
    let StarSystem {
        id,
        pos,
        name,
        bodies,
        legacy_deposits,
        claim_cost,
        owner,
        claimed_at,
        stockpile,
        industry,
        modules,
        legacy_extractor_tier,
        legacy_depot_tier,
        legacy_shipyard_tier,
        legacy_sensor_tier,
        legacy_defense_tier,
        defense_pool,
        legacy_habitat_tier,
        food_state,
        legacy_refinery_tier,
        blockade,
        blockade_prev,
        mounts,
        trait_,
        garrison_fed,
        garrison_suppression,
        cache_claimed,
        legacy_structures,
        legacy_population,
        legacy_assignments,
        specialists,
    } = stored;
    *id == live.id
        && *pos == live.pos
        && *name == live.name
        && *bodies == live.bodies
        && *legacy_deposits == live.legacy_deposits
        && *claim_cost == live.claim_cost
        && *owner == live.owner
        && *claimed_at == live.claimed_at
        && floored_stockpile_matches(stockpile, &live.stockpile)
        && *industry == live.industry
        && *modules == live.modules
        && *legacy_extractor_tier == live.legacy_extractor_tier
        && *legacy_depot_tier == live.legacy_depot_tier
        && *legacy_shipyard_tier == live.legacy_shipyard_tier
        && *legacy_sensor_tier == live.legacy_sensor_tier
        && *legacy_defense_tier == live.legacy_defense_tier
        && *defense_pool == live.defense_pool
        && *legacy_habitat_tier == live.legacy_habitat_tier
        && *food_state == live.food_state
        && *legacy_refinery_tier == live.legacy_refinery_tier
        && *blockade == live.blockade
        && *blockade_prev == live.blockade_prev
        && mounts_match(mounts, &live.mounts)
        && *trait_ == live.trait_
        && *garrison_fed == live.garrison_fed
        && *garrison_suppression == live.garrison_suppression
        && *cache_claimed == live.cache_claimed
        && *legacy_structures == live.legacy_structures
        && *legacy_population == live.legacy_population
        && *legacy_assignments == live.legacy_assignments
        && *specialists == live.specialists
}

/// Station hardpoints match when their fits do and their health is in the
/// same 5% band: repair creeps every tick, and a report a few percent behind
/// is still a true picture (the stockpile is floored the same way).
fn mounts_match(stored: &[crate::station::Mount], live: &[crate::station::Mount]) -> bool {
    let band = |h: f64| (h * 20.0).floor() as i64;
    stored.len() == live.len()
        && stored.iter().zip(live).all(|(a, b)| a.fit == b.fit && band(a.health) == band(b.health))
}

/// The stored (floored) stockpile against the live one, floored the same way
/// the report would store it. Mirrors `BTreeMap`'s own equality walk.
fn floored_stockpile_matches(
    stored: &BTreeMap<crate::cargo::Commodity, f64>,
    live: &BTreeMap<crate::cargo::Commodity, f64>,
) -> bool {
    stored.len() == live.len()
        && stored
            .iter()
            .zip(live)
            .all(|((stored_key, stored_units), (live_key, live_units))| {
                stored_key == live_key && *stored_units == live_units.floor()
            })
}
