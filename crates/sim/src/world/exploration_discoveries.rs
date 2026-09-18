//! Authored opportunities use the existing fleet/work/report clocks. Navigation
//! discovers surface facts; deeper work is an explicit, delayed fleet order.
//! Only report delivery grants coordinates or manufacturing licenses at CC.
use super::*;
use crate::cargo::Commodity as C;
use crate::module::{Loadout, ModuleKind as M};
use crate::nebula::NebulaKind as N;
use crate::sites::*;
use std::collections::BTreeSet;

impl World {
    /// Append-only migrations with independent RNG streams. Neither reload nor
    /// a new player rewrites an existing discovery, replenishes loot or respawns
    /// a defeated guardian. Sites remain physically discoverable without a quest.
    pub(super) fn seed_discovery_chains(&mut self) {
        let homes: Vec<_> = self.home_slots.iter().filter_map(|h| h.system.map(|id| (id, h.pos))).collect();
        for (home, pos) in homes {
            if self.exploration.chains_seeded_homes.contains(&home) { continue; }
            let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "discovery-chain-v1", home.0);
            let mut points = None;
            for _ in 0..128 {
                let angle = rng.range(0.0, TAU);
                let desired = [24_000.0, 58_000.0, 110_000.0].map(|r| pos + Vec2::from_polar(angle, r));
                let candidate: Option<Vec<_>> = desired.into_iter().map(|p|
                    self.clear_exploration_position(p, 5_000.0, &mut rng)).collect();
                if let Some(p) = candidate
                    && self.home_slots.iter().all(|h| p[2].distance(h.pos) > 50_000.0)
                    && self.fleets.values().all(|f| p[2].distance(f.pos) > 20_000.0)
                    && p.iter().all(|p| p.length() < self.config.galaxy_radius * 1.1)
                { points = Some(p); break; }
            }
            let Some(p) = points else { continue; };
            let first = self.create_exploration_site(p[0], SiteKind::Derelict, 0);
            let second = self.create_exploration_site(p[1], SiteKind::Station, 0);
            let third = self.create_exploration_site(p[2], SiteKind::Precursor, 0);
            for (id, name, requirement, blueprint, followup) in [
                (first, "Surveyor's wreck", StudyRequirement::Scout, M::SurveyDrive,
                    Some(DiscoveryLead { site: second, pos: p[1], clue: "The flight recorder names a silent observatory.".into() })),
                (second, "Lost-route observatory", StudyRequirement::ResearchTeam, M::NebulaSpectrometer,
                    Some(DiscoveryLead { site: third, pos: p[2], clue: "The archive charts a precursor vault. Its last record warns of armed scavengers.".into() })),
                (third, "Guarded precursor vault", StudyRequirement::ResearchTeam, M::PrismaticLance, None),
            ] {
                let site = self.exploration.sites.get_mut(&id).unwrap();
                site.details.name = format!("{name} {}", id.0);
                site.details.opportunity = Some(SiteOpportunity {
                    task: ExpeditionTask::Study, requirement, seconds: DEEP_STUDY_SECONDS,
                    costs: BTreeMap::new(), cargo: BTreeMap::new(), blueprint: Some(blueprint), has_lead: followup.is_some(),
                });
                site.followup = followup;
            }
            let guardian = self.spawn_fixed_pirates(vec![
                (ShipKind::Raider, Loadout::new(vec![M::ReflectivePlating]), 1),
                (ShipKind::Corvette, Loadout::new(vec![M::MassDriver]), 1),
            ], p[2], FleetOrder::Idle);
            self.fleets.get_mut(&guardian).unwrap().fuel = self.fleets[&guardian].fuel_capacity();
            let site = self.exploration.sites.get_mut(&third).unwrap();
            site.guardians.push(guardian);
            site.details.guarded = true;
            self.exploration.chains_seeded_homes.insert(home);
        }
    }

    pub(super) fn seed_nebula_activities(&mut self) {
        for region in self.nebulas.clone() {
            if self.exploration.nebula_activities_seeded.contains(&region.id) { continue; }
            let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "nebula-expeditions-v1", region.id as u64);
            let desired = region.center + Vec2::from_polar(region.rotation, region.radius_x * 0.35);
            let Some(pos) = self.clear_exploration_position(desired, region.radius_x * 0.3, &mut rng)
                .filter(|p| region.contains(*p)) else { continue; };
            let (name, kind, task, requirement, costs, cargo, blueprint) = match region.kind {
                N::MolecularCloud => ("Condensate pocket", SiteKind::Asteroids, ExpeditionTask::Extract,
                    StudyRequirement::FuelledFreighter, vec![(C::Fuel, 6)], vec![(C::Volatiles, 300)], None),
                N::IonNebula => ("Ion resonance lattice", SiteKind::Anomaly, ExpeditionTask::Study,
                    StudyRequirement::ResearchTeam, vec![], vec![], Some(M::NebulaSpectrometer)),
                N::DustCloud => ("Buried flight recorder", SiteKind::Derelict, ExpeditionTask::Study,
                    StudyRequirement::ResearchTeam, vec![], vec![], Some(M::SurveyDrive)),
                N::SupernovaRemnant => ("Shielded remnant recovery", SiteKind::Asteroids, ExpeditionTask::Extract,
                    StudyRequirement::ShieldedFreighter, vec![(C::Polymers, 8), (C::Fuel, 6)],
                    vec![(C::RareElements, 120), (C::PrecisionComponents, 12)], None),
                N::PrecursorCloud => ("Fold-space echo", SiteKind::Anomaly, ExpeditionTask::Study,
                    StudyRequirement::ResearchTeam, vec![], vec![], Some(M::SurveyDrive)),
            };
            let id = self.create_exploration_site(pos, kind, rng.next_u64());
            let site = self.exploration.sites.get_mut(&id).unwrap();
            site.details.name = format!("{name} {}", id.0);
            site.details.environment = Some(region.kind);
            // Samples are modest; the large protected cache has an explicit
            // extraction requirement and can never be vacuumed by auto-Recover.
            site.details.cargo = match region.kind {
                N::MolecularCloud => [(C::Volatiles, 20)].into(),
                N::SupernovaRemnant => [(C::RareElements, 4)].into(),
                N::DustCloud => [(C::Alloys, 8)].into(),
                _ => [(C::Electronics, 6)].into(),
            };
            site.details.modules.clear();
            site.details.opportunity = Some(SiteOpportunity {
                task, requirement, seconds: task.seconds(), costs: costs.into_iter().collect(),
                cargo: cargo.into_iter().collect(), blueprint, has_lead: false,
            });
            self.exploration.nebula_activities_seeded.insert(region.id);
        }
    }

    /// Guardians use local light and a fixed leash. They do not launch home
    /// raids, scale to the visitor, teleport back or respawn. A Scout's lookout
    /// is wider than this leash, leaving a real opportunity to retreat.
    pub(super) fn tick_discovery_guardians(&mut self) {
        let posts: Vec<_> = self.exploration.sites.values().filter(|s| !s.guardians.is_empty())
            .map(|s| (s.id, s.pos, s.guardians.clone())).collect();
        let engaged: BTreeSet<_> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(&e.defenders).copied()).collect();
        for (site_id, station, guards) in posts {
            let guarded = guards.iter().any(|id| self.fleets.get(id).is_some_and(|f| !f.composition.is_empty()));
            let site = self.exploration.sites.get_mut(&site_id).unwrap();
            if site.details.guarded != guarded { site.details.guarded = guarded; site.revision += 1; }
            for guard in guards {
                let Some(fleet) = self.fleets.get(&guard) else { continue; };
                if engaged.contains(&guard) { continue; }
                let target = (fleet.pos.distance(station) < 8_000.0).then(|| self.fleets.values()
                    .filter(|other| !other.owner.is_sentinel() && !engaged.contains(&other.id))
                    .filter_map(|other| {
                        let age = crate::transit::delay(other.pos, fleet.pos, self.config.c);
                        let seen = other.pos - other.vel * age;
                        (seen.distance(station) < 6_000.0 && crate::detection::detected(
                            other.signature() * self.nebula_signature_factor(seen),
                            &[(fleet.pos, EXPEDITION_LOOKOUT_RANGE * self.nebula_sensor_factor(fleet.pos))], seen))
                            .then_some((other.id, seen.distance(fleet.pos)))
                    }).min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))).map(|v| v.0)).flatten();
                let order = if let Some(target) = target { FleetOrder::Intercept { target } }
                    else if fleet.pos.distance(station) > 1.0 { FleetOrder::MoveTo { dest: station } }
                    else { FleetOrder::Idle };
                let fleet = self.fleets.get_mut(&guard).unwrap();
                let same = match (&fleet.order, &order) {
                    (FleetOrder::Idle, FleetOrder::Idle) => true,
                    (FleetOrder::MoveTo { dest: a }, FleetOrder::MoveTo { dest: b }) => a == b,
                    (FleetOrder::Intercept { target: a }, FleetOrder::Intercept { target: b }) => a == b,
                    _ => false,
                };
                if !same { fleet.order = order; fleet.pursuit_plan = None; }
            }
        }
    }

    pub(super) fn reject_deep_expedition(&mut self, id: EntityId, player: PlayerId, site: EntityId, events: &mut Vec<Event>) {
        let Some(fleet) = self.fleets.get_mut(&id) else { return; };
        fleet.order = FleetOrder::Idle;
        fleet.vel = Vec2::ZERO;
        let pos = fleet.pos;
        events.push(Event::new(self.time, EventPayload::OrderRejected {
            owner: player, fleet: id, target: Some(site),
            reason: crate::event::OrderRejectReason::DeliveryConditionsChanged,
        }).at_origin(pos));
        self.queue_site_report(site, player, pos);
    }

    pub(super) fn finish_deep_expedition(&mut self, id: EntityId, player: PlayerId, site_id: EntityId,
        task: ExpeditionTask, events: &mut Vec<Event>) {
        let Some(site) = self.exploration.sites.get(&site_id) else { return; };
        let Some(fleet) = self.fleets.get(&id) else { return; };
        let Some(opportunity) = site.details.opportunity.clone() else {
            self.reject_deep_expedition(id, player, site_id, events); return;
        };
        let ready = site.surveyed.contains(&player) && !site.details.guarded && task == opportunity.task
            && !(task == ExpeditionTask::Study && site.studied.contains(&player))
            && match opportunity.requirement {
                StudyRequirement::Scout => fleet.contains(ShipKind::Scout),
                StudyRequirement::ResearchTeam => fleet.contains(ShipKind::Scout)
                    && (fleet.has_recon() || fleet.has_spectrometer() || self.active_captain_for_fleet(player, id)
                        .is_some_and(|c| c.attributes.fieldcraft >= 2)),
                StudyRequirement::FuelledFreighter | StudyRequirement::ShieldedFreighter => fleet.has_freighter(),
            }
            && opportunity.costs.iter().all(|(c, n)| fleet.cargo_stacks().iter()
                .filter(|s| s.commodity == *c).map(|s| s.units).sum::<u32>() >= *n)
            && (task != ExpeditionTask::Extract || (opportunity.cargo.values().any(|n| *n > 0)
                && fleet.cargo_units().saturating_sub(opportunity.costs.values().sum()) < fleet.cargo_capacity()));
        if !ready { self.reject_deep_expedition(id, player, site_id, events); return; }
        let fleet = self.fleets.get_mut(&id).unwrap();
        for (&kind, &units) in &opportunity.costs { fleet.remove_cargo(kind, units); }
        let site = self.exploration.sites.get_mut(&site_id).unwrap();
        if task == ExpeditionTask::Extract {
            // Conservation and charging happen atomically at completion. An
            // interruption consumes no supplies and grants no partial discovery.
            for (&kind, amount) in &mut site.details.opportunity.as_mut().unwrap().cargo {
                let taken = (*amount).min(fleet.cargo_capacity().saturating_sub(fleet.cargo_units()));
                fleet.add_cargo(kind, taken);
                *amount -= taken;
            }
        }
        let first = site.studied.insert(player);
        if task == ExpeditionTask::Extract { site.revision += 1; }
        else { site.emitted.remove(&player); } // private study does not refresh a rival's report
        fleet.order = FleetOrder::Idle;
        fleet.vel = Vec2::ZERO;
        let pos = fleet.pos;
        if first { self.grant_captain_xp(player, id, crate::captain::CAPTAIN_XP_SURVEY); }
        self.queue_site_report(site_id, player, pos);
    }

    pub(super) fn annotate_exploration(&mut self, player: PlayerId, mut entry: JournalEntry) {
        // Notes are authored AT command, so saving one is immediate. The ID
        // must be public/received; this cannot probe or reveal hidden locations.
        if !self.players.contains_key(&player) { return; }
        let known = match entry.kind {
            JournalKind::Site => self.exploration.sites.get(&entry.id).is_some_and(|s| s.known.contains_key(&player)),
            JournalKind::System => self.systems.iter().any(|s| s.id == entry.id),
        };
        if !known { return; }
        entry.note = entry.note.chars().filter(|c| !c.is_control() || *c == '\n')
            .take(JOURNAL_NOTE_CHARS).collect::<String>().trim().to_string();
        let journal = self.exploration.journal.entry(player).or_default();
        let changed = if !entry.pinned && entry.note.is_empty() {
            journal.remove(&entry.id).is_some()
        } else if journal.get(&entry.id) != Some(&entry)
            && (journal.len() < JOURNAL_LIMIT || journal.contains_key(&entry.id)) {
            journal.insert(entry.id, entry);
            true
        } else { false };
        if changed { *self.exploration.journal_versions.entry(player).or_default() += 1; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(822, 4));
        let player = PlayerId(98111);
        w.step(&[Command::AddPlayer { id: player, name: "Expedition tests".into() }]);
        w.fleets.clear(); w.enclaves.clear(); w.nebulas.clear();
        w.exploration.pending.clear(); w.exploration.next_sensing_at = f64::MAX;
        for site in w.exploration.sites.values_mut() { site.known.clear(); site.emitted.clear(); }
        let site = w.exploration.sites.values().find(|s| s.followup.is_some()
            && s.details.opportunity.as_ref().is_some_and(|o| o.blueprint == Some(M::SurveyDrive))).unwrap().id;
        let fleet = ship(&mut w, player, ShipKind::Scout, site);
        (w, player, site, fleet)
    }

    fn ship(w: &mut World, player: PlayerId, kind: ShipKind, site: EntityId) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, player, kind, w.exploration.sites[&site].pos, FleetOrder::Idle, None));
        id
    }

    fn arrive(w: &mut World) {
        w.time = w.exploration.pending.iter().map(|r| r.arrival).fold(w.time, f64::max);
        w.deliver_site_reports();
    }

    fn identify(w: &mut World, player: PlayerId, site: EntityId, fleet: EntityId) {
        w.finish_expedition(fleet, player, site, ExpeditionTask::Investigate, &mut vec![]);
        arrive(w);
    }

    #[test]
    fn chains_and_nebula_activities_are_authored_finite_and_append_only() {
        for seed in [1, 822, 5005] {
            let mut w = World::new(SimConfig::for_players(seed, 5));
            assert_eq!(w.exploration.chains_seeded_homes.len(), w.home_slots.len());
            assert_eq!(w.exploration.nebula_activities_seeded.len(), 5);
            for site in w.exploration.sites.values() {
                if let Some(lead) = &site.followup { assert_ne!(site.id, lead.site); assert!(w.exploration.sites.contains_key(&lead.site)); }
                if let Some(environment) = site.details.environment {
                    assert!(w.nebulas.iter().any(|n| n.kind == environment && n.contains(site.pos)));
                }
                if !site.guardians.is_empty() {
                    assert!(w.home_slots.iter().all(|h| h.pos.distance(site.pos) > 50_000.0));
                    for guard in &site.guardians {
                        let f = &w.fleets[guard];
                        assert!(f.ships.iter().all(|s| s.hp == s.max_hp()));
                        assert!(!f.operation_privateer && !f.founding_privateer);
                    }
                }
            }
            let before = serde_json::to_value(&w.exploration).unwrap();
            let old_fleets = w.fleets.len();
            w.seed_discovery_chains(); w.seed_nebula_activities();
            assert_eq!(before, serde_json::to_value(&w.exploration).unwrap());
            assert_eq!(old_fleets, w.fleets.len());
            let mut saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            saved.seed_discovery_chains(); saved.seed_nebula_activities();
            assert_eq!(before, serde_json::to_value(&saved.exploration).unwrap());
        }
    }

    #[test]
    fn study_coordinates_and_blueprints_share_one_delayed_private_report() {
        let (mut w, p, first, fleet) = scene();
        let next = w.exploration.sites[&first].followup.as_ref().unwrap().site;
        identify(&mut w, p, first, fleet);
        let original = w.exploration.sites[&first].known[&p].clone();
        assert!(original.details.as_ref().unwrap().opportunity.as_ref().unwrap().has_lead);
        assert!(original.details.as_ref().unwrap().lead.is_none());
        assert!(!w.players[&p].research.blueprints.contains(&M::SurveyDrive));
        assert!(!w.exploration.sites[&next].known.contains_key(&p));
        assert!(w.exploration_arrival_order(fleet, original.pos).is_none(), "travel must not volunteer a deeper investigation");
        w.time += DEEP_STUDY_SECONDS;
        w.finish_deep_expedition(fleet, p, first, ExpeditionTask::Study, &mut vec![]);
        let emission = w.time;
        let due = w.exploration.pending[0].arrival;
        assert!(due > emission);
        assert_eq!(w.exploration.sites[&first].known[&p], original);
        w.time = due - 1e-6; w.deliver_site_reports();
        assert!(w.players[&p].research.blueprints.is_empty());
        assert!(!w.exploration.sites[&next].known.contains_key(&p));
        let mut w: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        arrive(&mut w);
        let report = &w.exploration.sites[&first].known[&p];
        assert!(report.details.as_ref().unwrap().studied);
        assert_eq!(report.details.as_ref().unwrap().lead.as_ref().unwrap().site, next);
        assert_eq!(w.exploration.sites[&next].known[&p].reported_at, emission, "a clue's age is not reset at reception");
        assert!(w.exploration.sites[&next].known[&p].details.is_none());
        assert!(crate::research::has_module(&w.players[&p].research, M::SurveyDrive));
        assert!(w.exploration.reports_for(PlayerId(98112)).is_empty());
        let grants = w.players[&p].research.clone();
        w.finish_deep_expedition(fleet, p, first, ExpeditionTask::Study, &mut vec![]); arrive(&mut w);
        assert_eq!(grants, w.players[&p].research, "repeat study cannot duplicate a reward");
    }

    #[test]
    fn deep_orders_are_delayed_cancelable_and_require_preparation_on_site() {
        let (mut w, p, root, fleet) = scene();
        identify(&mut w, p, root, fleet);
        w.apply(&Command::ExploreSite { player_id: p, fleet_id: fleet, site_id: root, task: ExpeditionTask::Study }, &mut vec![]);
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::Idle));
        let at = w.pending_orders.iter().find(|o| o.ship_id == fleet).unwrap().apply_time;
        w.time = at; w.deliver_due_orders(&mut vec![]);
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::Expedition { task: ExpeditionTask::Study, .. }));
        w.tick_exploration_sites(&mut vec![]);
        assert!(w.players[&p].research.blueprints.is_empty());
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Idle;
        w.time += DEEP_STUDY_SECONDS * 2.0; w.tick_exploration_sites(&mut vec![]);
        assert!(!w.exploration.sites[&root].studied.contains(&p));
        let station = w.exploration.sites[&root].followup.as_ref().unwrap().site;
        w.fleets.get_mut(&fleet).unwrap().pos = w.exploration.sites[&station].pos;
        identify(&mut w, p, station, fleet);
        w.finish_deep_expedition(fleet, p, station, ExpeditionTask::Study, &mut vec![]);
        assert!(!w.exploration.sites[&station].studied.contains(&p));
        w.fleets.get_mut(&fleet).unwrap().set_fitted(ShipKind::Scout, &Loadout::new(vec![M::ReconSuite]), 1);
        w.finish_deep_expedition(fleet, p, station, ExpeditionTask::Study, &mut vec![]);
        assert!(w.exploration.sites[&station].studied.contains(&p));
        assert!(!w.players[&p].research.blueprints.contains(&M::NebulaSpectrometer));
        arrive(&mut w);
        assert!(w.players[&p].research.blueprints.contains(&M::NebulaSpectrometer));
    }

    #[test]
    fn delivered_study_requires_uninterrupted_dwell_then_returns_its_report() {
        let (mut w, p, site, scout) = scene();
        identify(&mut w, p, site, scout);
        w.apply(&Command::ExploreSite { player_id: p, fleet_id: scout, site_id: site, task: ExpeditionTask::Study }, &mut vec![]);
        w.time = w.pending_orders.iter().find(|o| o.ship_id == scout).unwrap().apply_time;
        w.deliver_due_orders(&mut vec![]);
        w.tick_exploration_sites(&mut vec![]);
        let first_start = w.time;
        w.time += DEEP_STUDY_SECONDS - 0.01; w.tick_exploration_sites(&mut vec![]);
        assert!(!w.exploration.sites[&site].studied.contains(&p));
        w.fleets.get_mut(&scout).unwrap().vel = Vec2::new(1.0, 0.0);
        w.tick_exploration_sites(&mut vec![]);
        w.fleets.get_mut(&scout).unwrap().vel = Vec2::ZERO;
        w.tick_exploration_sites(&mut vec![]);
        w.time = first_start + DEEP_STUDY_SECONDS;
        w.tick_exploration_sites(&mut vec![]);
        assert!(!w.exploration.sites[&site].studied.contains(&p), "motion resets work, not just its progress graphic");
        w.time += DEEP_STUDY_SECONDS;
        w.tick_exploration_sites(&mut vec![]);
        assert!(matches!(w.fleets[&scout].order, FleetOrder::Idle));
        assert!(w.exploration.sites[&site].studied.contains(&p));
        assert!(w.players[&p].research.blueprints.is_empty(), "completed work is still remote");
        assert!(!w.exploration.sites[&site].known[&p].details.as_ref().unwrap().studied);
        arrive(&mut w);
        assert!(w.players[&p].research.blueprints.contains(&M::SurveyDrive));
    }

    #[test]
    fn a_guarded_site_starts_a_real_battle_with_a_visiting_warship() {
        let (mut w, p, _, scout) = scene();
        w.fleets.remove(&scout);
        let site = w.exploration.sites.values().find(|s| !s.guardians.is_empty()).unwrap().id;
        let guard = ship(&mut w, PlayerId::PIRATE, ShipKind::Raider, site);
        w.exploration.sites.get_mut(&site).unwrap().guardians = vec![guard];
        let escort = ship(&mut w, p, ShipKind::Corvette, site);
        w.fleets.get_mut(&escort).unwrap().pos.x += 50.0;
        for _ in 0..30 {
            w.step(&[]);
            if w.engagements.values().any(|e| e.attackers.contains(&guard) && e.defenders.contains(&escort)
                || e.defenders.contains(&guard) && e.attackers.contains(&escort)) { return; }
        }
        panic!("guardians must physically fight, not merely set a panel warning");
    }

    #[test]
    fn a_scout_abandons_deep_work_before_a_local_threat_can_award_a_blueprint() {
        let (mut w, p, site, fleet) = scene();
        identify(&mut w, p, site, fleet);
        let station = w.fleets[&fleet].pos;
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Expedition { site, station, task: ExpeditionTask::Study, dwell_since: Some(w.time - DEEP_STUDY_SECONDS) };
        let pirate = ship(&mut w, PlayerId::PIRATE, ShipKind::Raider, site);
        w.fleets.get_mut(&pirate).unwrap().pos = station + Vec2::new(4_000.0, 0.0);
        w.tick_exploration_sites(&mut vec![]);
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { .. }));
        assert!(!w.exploration.sites[&site].studied.contains(&p));
        assert!(w.players[&p].research.blueprints.is_empty());
    }

    #[test]
    fn nebula_extraction_conserves_a_shared_finite_cache_and_charges_supplies_once() {
        let (mut w, p, _, scout) = scene();
        let site = w.exploration.sites.values().find(|s| s.details.environment == Some(N::MolecularCloud)).unwrap().id;
        w.fleets.get_mut(&scout).unwrap().pos = w.exploration.sites[&site].pos;
        identify(&mut w, p, site, scout);
        let surface = w.exploration.sites[&site].details.cargo.clone();
        let a = ship(&mut w, p, ShipKind::Convoy, site);
        let b = ship(&mut w, p, ShipKind::Convoy, site);
        for id in [a, b] {
            w.fleets.get_mut(&id).unwrap().add_cargo(C::Fuel, 12);
            w.finish_deep_expedition(id, p, site, ExpeditionTask::Extract, &mut vec![]);
        }
        let recovered: u32 = [a, b].into_iter().map(|id| w.fleets[&id].cargo_stacks().iter().filter(|c| c.commodity == C::Volatiles).map(|c| c.units).sum::<u32>()).sum();
        assert_eq!(recovered, 300);
        assert_eq!(w.exploration.sites[&site].details.cargo, surface, "surface and deep pools stay distinct");
        assert_eq!(w.exploration.sites[&site].known[&p].details.as_ref().unwrap().opportunity.as_ref().unwrap().cargo[&C::Volatiles], 300, "no instant depletion report");
        let before = w.fleets[&b].cargo_stacks();
        w.finish_deep_expedition(b, p, site, ExpeditionTask::Extract, &mut vec![]);
        assert_eq!(w.fleets[&b].cargo_stacks(), before, "exhausted cache consumes no more supplies");
        for id in [a, b] { assert!(w.fleets[&id].cargo_units() <= w.fleets[&id].cargo_capacity()); }
        arrive(&mut w);
        assert_eq!(w.exploration.sites[&site].known[&p].details.as_ref().unwrap().opportunity.as_ref().unwrap().cargo[&C::Volatiles], 0);
    }

    #[test]
    fn remnant_extraction_needs_shielding_supplies_and_partial_loads_leave_the_rest() {
        let (mut w, p, _, scout) = scene();
        let site = w.exploration.sites.values().find(|s| s.details.environment == Some(N::SupernovaRemnant)).unwrap().id;
        identify(&mut w, p, site, scout);
        let f = ship(&mut w, p, ShipKind::Convoy, site);
        w.fleets.get_mut(&f).unwrap().add_cargo(C::Fuel, 6);
        w.finish_deep_expedition(f, p, site, ExpeditionTask::Extract, &mut vec![]);
        assert_eq!(w.fleets[&f].cargo_units(), 6);
        w.fleets.get_mut(&f).unwrap().add_cargo(C::Polymers, 8);
        w.fleets.get_mut(&f).unwrap().add_cargo(C::Alloys, ShipKind::Convoy.cargo_units() - 15);
        w.finish_deep_expedition(f, p, site, ExpeditionTask::Extract, &mut vec![]);
        assert_eq!(w.fleets[&f].cargo_units(), ShipKind::Convoy.cargo_units());
        assert_eq!(w.exploration.sites[&site].details.opportunity.as_ref().unwrap().cargo.values().sum::<u32>(), 132 - 15);
    }

    #[test]
    fn guardians_block_work_and_clearance_is_news_gated_not_a_timer_respawn() {
        let (mut w, p, _, scout) = scene();
        let site = w.exploration.sites.values().find(|s| !s.guardians.is_empty()).unwrap().id;
        let guard = ship(&mut w, PlayerId::PIRATE, ShipKind::Raider, site);
        w.exploration.sites.get_mut(&site).unwrap().guardians = vec![guard];
        identify(&mut w, p, site, scout);
        let freight = ship(&mut w, p, ShipKind::Convoy, site);
        w.finish_expedition(freight, p, site, ExpeditionTask::Recover, &mut vec![]);
        assert_eq!(w.fleets[&freight].cargo_units(), 0);
        w.fleets.remove(&guard); w.tick_discovery_guardians();
        assert!(!w.exploration.sites[&site].details.guarded);
        assert!(w.exploration.sites[&site].known[&p].details.as_ref().unwrap().guarded);
        w.queue_site_report(site, p, w.exploration.sites[&site].pos); arrive(&mut w);
        assert!(!w.exploration.sites[&site].known[&p].details.as_ref().unwrap().guarded);
        let count = w.fleets.len(); w.seed_discovery_chains();
        assert_eq!(w.fleets.len(), count, "cleared guards never remint");
        w.finish_expedition(freight, p, site, ExpeditionTask::Recover, &mut vec![]);
        assert!(w.fleets[&freight].cargo_units() > 0);
    }

    #[test]
    fn discovery_fittings_are_real_tradeoffs_and_mixed_fleets_keep_their_slowest_ship() {
        let (mut w, p, site, _) = scene();
        let id = ship(&mut w, p, ShipKind::Scout, site);
        let f = w.fleets.get_mut(&id).unwrap();
        let speed = f.max_speed(); let capacity = f.fuel_capacity();
        f.set_fitted(ShipKind::Scout, &Loadout::new(vec![M::SurveyDrive]), 1);
        assert_eq!(f.max_speed(), speed * 1.25);
        assert!((f.fuel_capacity() - capacity * 0.6).abs() < 1e-9);
        f.add(ShipKind::Scout, 1);
        assert_eq!(f.max_speed(), speed, "one fitted hull cannot accelerate an unfitted companion");
        f.add(ShipKind::Convoy, 1);
        assert_eq!(f.max_speed(), ShipKind::Convoy.max_speed());
        let spectrometer = Loadout::new(vec![M::NebulaSpectrometer]);
        assert!(!spectrometer.has_recon()); assert_eq!(spectrometer.speed_mult(), 1.0);
        assert!(!Loadout::new(vec![M::SurveyDrive, M::NebulaSpectrometer]).validate(ShipKind::Scout));
        let lance = Loadout::new(vec![M::PrismaticLance]);
        assert_eq!(lance.offense(), (crate::module::DamageType::Beam, 1.45));
        assert_eq!(lance.fitting_cost(), 3);
        assert!(!Loadout::new(vec![M::PrismaticLance, M::ReflectivePlating]).validate(ShipKind::Raider));
    }

    #[test]
    fn journal_is_private_bounded_and_survives_restart_without_revealing_hidden_sites() {
        let (mut w, p, site, scout) = scene();
        let entry = JournalEntry { id: site, kind: JournalKind::Site, pinned: true, note: "<b>Find the archive</b>".into() };
        w.annotate_exploration(p, entry.clone());
        assert!(w.exploration.journal_for(p).is_empty());
        assert_eq!(w.exploration.journal_version_for(p), 0);
        identify(&mut w, p, site, scout);
        w.annotate_exploration(p, entry.clone());
        assert_eq!(w.exploration.journal_for(p), vec![entry.clone()]);
        assert_eq!(w.exploration.journal_version_for(p), 1);
        w.annotate_exploration(p, entry.clone());
        assert_eq!(w.exploration.journal_version_for(p), 1, "no-op edits do not trigger retransmission");
        assert!(w.exploration.journal_for(PlayerId(777)).is_empty());
        let mut w: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        assert_eq!(w.exploration.journal_for(p), vec![entry.clone()]);
        w.annotate_exploration(p, JournalEntry { note: "星".repeat(999), ..entry.clone() });
        assert_eq!(w.exploration.journal_for(p)[0].note.chars().count(), JOURNAL_NOTE_CHARS);
        w.annotate_exploration(p, JournalEntry { note: "".into(), pinned: false, ..entry });
        assert!(w.exploration.journal_for(p).is_empty());
        assert_eq!(w.exploration.journal_version_for(p), 3);
    }

    #[test]
    fn a_discovery_license_manufactures_real_crates_not_authority_shop_stock() {
        use crate::build::{BuildKind, StructureKind as K};
        for module in [M::SurveyDrive, M::NebulaSpectrometer, M::PrismaticLance] {
            let (mut w, p, _, _) = scene();
            let home = w.players[&p].home_system.unwrap();
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_population(0.05);
            for b in &mut sys.bodies { b.assignments.clear(); }
            for kind in [K::Shipyard, K::ArmamentsComplex, K::OrdnanceFoundry] {
                sys.bodies[0].structures.insert(kind, 1);
                sys.bodies[0].assignments.insert(kind, crate::production::Assignment::crew(1));
            }
            for (c, _) in crate::build::module_recipe(module).costs { sys.stockpile.insert(*c, 1000.0); }
            let before = sys.stockpile.clone();
            let what = BuildKind::Module { module };
            w.apply_build(p, home, None, what, None, Loadout::default(), &mut vec![]);
            assert!(w.build_queue.is_empty(), "knowing the public recipe isn't a license");
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, before);
            w.players.get_mut(&p).unwrap().research.blueprints.insert(module);
            w.apply_build(p, home, None, what, None, Loadout::default(), &mut vec![]);
            assert_eq!(w.build_queue.len(), 1);
            let sys = w.systems.iter().find(|s| s.id == home).unwrap();
            for (c, cost) in crate::build::module_recipe(module).costs { assert_eq!(sys.stockpile[c], before[c] - cost); }
            w.tick += 10_000; w.resolve_builds(&mut vec![]);
            assert!(w.build_queue.is_empty());
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules[&module], 1);
            let credits = w.players[&p].credits;
            let fleets = w.fleets.len();
            w.apply_now(&Command::BuyModule { player_id: p, module, n: 1, dest_system: home }, &mut vec![]);
            assert_eq!(w.players[&p].credits, credits);
            assert_eq!(w.fleets.len(), fleets, "a license does not unlock an infinite Authority supply");
        }
    }

    #[test]
    fn a_spectrometer_extends_site_contacts_without_exposing_combatants_or_shortening_light() {
        let (mut w, p, site, fleet) = scene();
        let pos = w.players[&p].home + Vec2::new(350_000.0, 0.0);
        w.exploration.sites.get_mut(&site).unwrap().pos = pos;
        w.fleets.get_mut(&fleet).unwrap().pos = pos - Vec2::new(75_000.0, 0.0);
        w.exploration.next_sensing_at = w.time; w.tick_exploration_sites(&mut vec![]);
        assert!(!w.exploration.pending.iter().any(|r| r.snapshot.id == site));
        let f = w.fleets.get_mut(&fleet).unwrap();
        f.set_fitted(ShipKind::Scout, &Loadout::new(vec![M::NebulaSpectrometer]), 1);
        assert!(!f.projects_sensor());
        w.exploration.next_sensing_at = w.time; w.tick_exploration_sites(&mut vec![]);
        let report = w.exploration.pending.iter().find(|r| r.snapshot.id == site).unwrap();
        let observer = w.fleets[&fleet].pos;
        let expected = w.time + crate::transit::delay(pos, observer, w.config.c)
            + crate::transit::delay(observer, w.players[&p].command_center, w.config.c);
        assert!((report.arrival - expected).abs() < 1e-9);
        assert!(report.snapshot.details.is_none());
        assert!(!w.exploration.sites[&site].known.contains_key(&p));
    }
}
