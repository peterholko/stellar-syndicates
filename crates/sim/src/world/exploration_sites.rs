//! Explore → receive a report → recover physical cargo / restore a sensor site.
//! The world, command, and reporting clocks are the existing game clocks. No
//! client prediction, instant remote stockpile grant, or public truth catalogue.
use super::*;
use crate::cargo::Commodity as C;
use crate::module::ModuleKind as M;
use crate::sites::*;
use std::collections::BTreeSet;

impl World {
    /// Only a deliberate destination order opts into arrival work. Never sweep
    /// idle hulls for sites: that would hijack retreat, Hold, patrol and guard,
    /// or stop a passing ship. Read RECEIVED contacts; restoration stays explicit.
    pub(super) fn exploration_arrival_order(
        &self,
        fleet_id: EntityId,
        dest: Vec2,
    ) -> Option<FleetOrder> {
        let fleet = self.fleets.get(&fleet_id)?;
        if fleet.owner.is_sentinel() || fleet.disposable {
            return None;
        }
        let report = self
            .exploration
            .sites
            .values()
            .filter_map(|site| site.known.get(&fleet.owner))
            .filter(|report| report.pos.distance(dest) <= SITE_RANGE)
            .min_by(|a, b| {
                a.pos
                    .distance(dest)
                    .total_cmp(&b.pos.distance(dest))
                    .then(a.id.cmp(&b.id))
            })?;
        let task = if report.details.is_none() && fleet.contains(ShipKind::Scout) {
            ExpeditionTask::Investigate
        } else if fleet.has_freighter()
            && report.details.as_ref().is_some_and(|d| {
                (d.cargo.values().any(|&n| n > 0) && fleet.cargo_units() < fleet.cargo_capacity())
                    || (d.modules.values().any(|&n| n > 0)
                        && fleet.modules.values().sum::<u32>()
                            < fleet.freighter_count() * crate::module::MODULE_CONVOY_BERTHS)
            })
        {
            ExpeditionTask::Recover
        } else {
            return None;
        };
        Some(FleetOrder::Expedition {
            site: report.id,
            station: dest,
            task,
            dwell_since: None,
        })
    }

    /// Onboard safety precedes navigation and checks again before paying out
    /// work. Only expeditions are eligible; tactical withdrawal, an installed
    /// retreat, Guard and Jump remain owned by their existing systems.
    pub(super) fn autonomous_exploration_safety(&mut self, events: &mut Vec<Event>) {
        let engaged: BTreeSet<_> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(&e.defenders).copied())
            .collect();
        let mut retreats = Vec::new();
        for (&id, fleet) in &self.fleets {
            let FleetOrder::Expedition { site, .. } = fleet.order else {
                continue;
            };
            if engaged.contains(&id) || fleet.defense.is_some() {
                continue;
            }
            let range = EXPEDITION_LOOKOUT_RANGE
                * fleet.exploration_range_mult()
                * self.research_mod(fleet.owner, crate::research::ModKey::SensorRange)
                * self.nebula_sensor_factor(fleet.pos);
            let mut hostile_weight = 0.0;
            let mut friendly_weight = fleet.combat_weight();
            let mut nearest: Option<(EntityId, Vec2, f64)> = None;
            for (&other_id, other) in &self.fleets {
                if other_id == id || !other.is_combatant() {
                    continue;
                }
                // Match standing system defense's local-light approximation.
                // No CC/global sensor sharing or unseen current-position aim.
                let age = crate::transit::delay(other.pos, fleet.pos, self.config.c);
                let seen = other.pos - other.vel * age;
                let sig = other.signature()
                    * self.veil_factor(other.owner, seen)
                    * self.nebula_signature_factor(seen);
                let detected = if other.broadcasts() {
                    fleet.pos.distance(seen) <= range
                } else {
                    crate::detection::detected(sig, &[(fleet.pos, range)], seen)
                };
                if !detected {
                    continue;
                }
                if other.owner == fleet.owner || self.are_allied(fleet.owner, other.owner) {
                    friendly_weight += other.combat_weight();
                    continue;
                }
                let directly_attacking = matches!(other.order,
                    FleetOrder::Attack { target } | FleetOrder::Intercept { target } if target == id);
                if (!directly_attacking && other.owner.is_tca())
                    || self.diplomacy_protects(fleet.owner, other.owner)
                    || (!other.owner.is_sentinel()
                        && (self.founder_protected(fleet.owner)
                            || self.founder_protected(other.owner)))
                    || self.in_sovereign_zone(seen)
                {
                    continue;
                }
                hostile_weight += other.combat_weight();
                let distance = fleet.pos.distance(seen);
                if nearest
                    .is_none_or(|(old, _, d)| distance < d || (distance == d && other_id < old))
                {
                    nearest = Some((other_id, seen, distance));
                }
            }
            let Some((_, threat, _)) = nearest else {
                continue;
            };
            // Civilian work never wins over self-preservation. Armed mixed
            // expeditions keep the corporation's existing retreat-odds policy.
            let retreat = !fleet.is_combatant()
                || self
                    .players
                    .get(&fleet.owner)
                    .and_then(|c| c.doctrine.retreat.min_ratio())
                    .is_some_and(|min| {
                        friendly_weight / (friendly_weight + hostile_weight).max(1e-9) < min
                    });
            if !retreat {
                continue;
            }
            let away = if fleet.pos.distance(threat) > 1e-6 {
                (fleet.pos - threat).normalized()
            } else {
                Vec2::new(1.0, 0.0)
            };
            // Move away, then hold. Do not blindly route home through a hostile
            // or save a return-to-site that could yo-yo back into danger.
            retreats.push((
                id,
                fleet.owner,
                site,
                fleet.pos,
                fleet.pos + away * EXPEDITION_ESCAPE_DISTANCE,
            ));
        }
        for (id, owner, site, origin, dest) in retreats {
            let fleet = self.fleets.get_mut(&id).unwrap();
            fleet.order = FleetOrder::MoveTo { dest };
            fleet.pursuit_plan = None;
            events.push(
                Event::new(
                    self.time,
                    EventPayload::OrderRejected {
                        owner,
                        fleet: id,
                        target: Some(site),
                        reason: crate::event::OrderRejectReason::ExplorationThreat,
                    },
                )
                .at_origin(origin),
            );
        }
    }

    pub(super) fn seed_exploration_sites(&mut self) {
        if !self.exploration.generated {
            self.exploration.generated = true;
            let mut rng = crate::rng::Rng::keyed(self.config.seed, "exploration-sites-v1");
            // Each public nebula contains actual destinations. The phenomenon
            // and alien installation remain unknown until their reports arrive.
            for region in self.nebulas.clone() {
                for (i, kind) in [SiteKind::Anomaly, SiteKind::Precursor]
                    .into_iter()
                    .enumerate()
                {
                    let pos = region.center + Vec2::new((i as f64 - 0.5) * 8_000.0, 0.0);
                    if let Some(pos) = self.clear_exploration_position(pos, 4_000.0, &mut rng) {
                        self.create_exploration_site(pos, kind, rng.next_u64());
                    }
                }
            }
            // Interstellar locations, independent of star placement and all
            // existing RNG streams. No new colonizable systems are manufactured.
            for i in 0..12 {
                let a = rng.range(0.0, TAU);
                let r = rng.range(0.25, 0.85) * self.config.galaxy_radius;
                let desired = Vec2::new(a.cos(), a.sin()) * r;
                if let Some(pos) = self.clear_exploration_position(desired, 12_000.0, &mut rng) {
                    let kind = [
                        SiteKind::Derelict,
                        SiteKind::Station,
                        SiteKind::Asteroids,
                        SiteKind::Precursor,
                    ][i % 4];
                    self.create_exploration_site(pos, kind, rng.next_u64());
                }
            }
        }
        let homes: Vec<_> = self
            .home_slots
            .iter()
            .filter_map(|s| s.system.map(|id| (id, s.pos)))
            .collect();
        for &(id, home) in &homes {
            if !self.exploration.seeded_homes.insert(id) {
                if !self.exploration.utility_seeded_homes.contains(&id) {
                    let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "exploration-utility-v1", id.0);
                    let desired = home + Vec2::from_polar(rng.range(0.0, TAU), 22_000.0);
                    if let Some(pos) = self.clear_exploration_position(desired, 6_000.0, &mut rng) {
                        self.create_exploration_site(pos, SiteKind::Derelict, 2 + rng.next_u64() % 2);
                        self.exploration.utility_seeded_homes.insert(id);
                    }
                }
                continue;
            }
            let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "exploration-home-v1", id.0);
            let angle = rng.range(0.0, TAU);
            // Wreck, repairable station, ore cluster, icy comet. Early visits
            // require no combat, colonization, syndicate or research unlock.
            for (i, kind) in [
                SiteKind::Derelict,
                SiteKind::Station,
                SiteKind::Asteroids,
                SiteKind::Asteroids,
            ]
            .into_iter()
            .enumerate()
            {
                let a = angle + i as f64 * TAU / 4.0;
                let desired = home + Vec2::new(a.cos(), a.sin()) * rng.range(14_000.0, 24_000.0);
                if let Some(pos) = self.clear_exploration_position(desired, 6_000.0, &mut rng) {
                    let mut variant = if i == 3 { 1 } else { rng.next_u64() & !1 };
                    if i == 0 {
                        variant = 2 + (variant >> 1) % 2;
                        self.exploration.utility_seeded_homes.insert(id);
                    }
                    self.create_exploration_site(pos, kind, variant);
                }
            }
        }
        // Independent append-only stream: old discoveries and the original
        // four-contact opening never get rerolled to introduce a new reward.
        for (id, home) in homes {
            if self.exploration.cargo_seeded_homes.contains(&id) { continue; }
            let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "exploration-cargo-pods-v1", id.0);
            let desired = home + Vec2::from_polar(rng.range(0.0, TAU), 32_000.0);
            if let Some(pos) = self.clear_exploration_position(desired, 6_000.0, &mut rng) {
                self.create_exploration_site(pos, SiteKind::Derelict, 4);
                self.exploration.cargo_seeded_homes.insert(id);
            }
        }
        self.seed_discovery_chains();
        self.seed_nebula_activities();
    }

    /// A discovery near one in four additional stars; reuse the same finite
    /// cargo, investigation, safety and delayed-report pipeline as other sites.
    /// Called only after the original sites exist, so none move or get rerolled.
    pub(super) fn seed_exploration_star_sites(&mut self, stars: &[Vec2]) {
        let mut rng = crate::rng::Rng::keyed(self.config.seed, "exploration-star-sites-v1");
        for (i, &star) in stars.iter().step_by(4).enumerate() {
            let angle = rng.range(0.0, TAU);
            let desired = star + Vec2::from_polar(angle, 6_000.0);
            if let Some(pos) = self.clear_exploration_position(desired, 4_000.0, &mut rng) {
                let kind = [
                    SiteKind::Derelict,
                    SiteKind::Asteroids,
                    SiteKind::Station,
                    SiteKind::Precursor,
                ][i % 4];
                self.create_exploration_site(pos, kind, rng.next_u64());
            }
        }
    }

    pub(super) fn clear_exploration_position(
        &self,
        desired: Vec2,
        scatter: f64,
        rng: &mut crate::rng::Rng,
    ) -> Option<Vec2> {
        (0..128).find_map(|attempt| {
            let pos = if attempt == 0 {
                desired
            } else {
                let a = rng.range(0.0, TAU);
                desired + Vec2::new(a.cos(), a.sin()) * rng.range(0.0, scatter)
            };
            (pos.distance(self.hub) > 5_000.0
                && self.systems.iter().all(|s| pos.distance(s.pos) > 3_000.0)
                && self
                    .exploration
                    .sites
                    .values()
                    .all(|s| pos.distance(s.pos) > 2_500.0))
            .then_some(pos)
        })
    }

    pub(super) fn create_exploration_site(&mut self, pos: Vec2, kind: SiteKind, variant: u64) -> EntityId {
        let id = self.alloc_entity_id();
        let (name, cargo, modules, programme, fraction): (&str, Vec<_>, Vec<_>, &str, f64) =
            match kind {
                SiteKind::Derelict => (
                    "Drifting wreck",
                    vec![(C::Alloys, 24 + (variant % 17) as u32)],
                    vec![(
                        match variant % 5 {
                            0 => M::WhippleArmor,
                            1 => M::ReflectivePlating,
                            2 => M::ExtendedTanks,
                            3 => M::ReconSuite,
                            _ => M::CargoPods,
                        },
                        1,
                    )],
                    match variant % 5 {
                        2 => "prop_expedition_iii_extended_tanks",
                        3 => "comp_shadow_iv_recon_suite",
                        4 => "hull_cargo_pods",
                        _ => "prop_bunkerage",
                    },
                    0.15,
                ),
                SiteKind::Station => (
                    "Silent observatory",
                    vec![(C::Electronics, 10), (C::Machinery, 8)],
                    vec![],
                    "comp_sensor_gain",
                    0.20,
                ),
                SiteKind::Asteroids if variant % 2 == 0 => (
                    "Metallic asteroid cluster",
                    vec![
                        (C::MetallicOre, 120),
                        (C::RareElements, 20 + (variant % 25) as u32),
                    ],
                    vec![],
                    "mat_deep_bores",
                    0.15,
                ),
                SiteKind::Asteroids => (
                    "Ice comet fragments",
                    vec![(C::Volatiles, 100), (C::Fuel, 20)],
                    vec![],
                    "prop_efficient_burns",
                    0.20,
                ),
                SiteKind::Anomaly => (
                    "Nebula resonance",
                    vec![(C::RareElements, 25), (C::Electronics, 15)],
                    vec![],
                    "comp_survey_protocols",
                    0.25,
                ),
                SiteKind::Precursor => (
                    "Precursor installation",
                    vec![(C::PrecisionComponents, 12), (C::Electronics, 30)],
                    vec![(M::PointDefenseScreen, 1)],
                    "hull_line_v_cruiser",
                    0.15,
                ),
            };
        self.exploration.sites.insert(
            id,
            ExplorationSite {
                id,
                pos,
                details: SiteDetails {
                    kind,
                    name: format!("{name} {}", id.0),
                    cargo: cargo.into_iter().collect(),
                    modules: modules.into_iter().collect(),
                    programme: programme.into(),
                    research_fraction: fraction,
                    restored_by: None,
                    environment: None,
                    opportunity: None,
                    studied: false,
                    lead: None,
                    guarded: false,
                },
                revision: 0,
                restored_sensor: None,
                surveyed: Default::default(),
                study_paid: Default::default(),
                known: Default::default(),
                emitted: Default::default(),
                followup: None,
                studied: Default::default(),
                guardians: Vec::new(),
            },
        );
        id
    }

    pub(super) fn queue_site_report(&mut self, id: EntityId, player: PlayerId, observer: Vec2) {
        let Some(cc) = self.players.get(&player).map(|c| c.command_center) else {
            return;
        };
        let Some(site) = self.exploration.sites.get_mut(&id) else {
            return;
        };
        let detailed = site.surveyed.contains(&player);
        let key = (if detailed { site.revision } else { 0 }, detailed);
        if site.emitted.get(&player) == Some(&key) {
            return;
        }
        site.emitted.insert(player, key);
        // Stationary light must reach the observer, then its report must reach
        // CC. On-site investigation uses observer=site, so its first leg is zero.
        // Freeze the snapshot now: later depletion/restoration cannot leak into
        // an in-flight report. Unknown contacts expose no changing hidden facts.
        self.exploration.pending.push(PendingSiteReport {
            recipient: player,
            arrival: self.time
                + crate::transit::delay(site.pos, observer, self.config.c)
                + crate::transit::delay(observer, cc, self.config.c),
            snapshot: SiteReport {
                id,
                pos: site.pos,
                reported_at: self.time,
                details: detailed.then(|| {
                    let mut details = site.details.clone();
                    details.studied = site.studied.contains(&player);
                    details.lead = details.studied.then(|| site.followup.clone()).flatten();
                    details
                }),
            },
        });
    }

    pub(super) fn deliver_site_reports(&mut self) {
        self.exploration
            .pending
            .sort_by(|a, b| a.arrival.total_cmp(&b.arrival));
        let n = self
            .exploration
            .pending
            .partition_point(|r| r.arrival <= self.time + 1e-9);
        let due: Vec<_> = self.exploration.pending.drain(..n).collect();
        for report in due {
            let Some(site) = self.exploration.sites.get_mut(&report.snapshot.id) else {
                continue;
            };
            let replace = site.known.get(&report.recipient).is_none_or(|old| {
                report.snapshot.reported_at >= old.reported_at
                    && (old.details.is_none() || report.snapshot.details.is_some())
            });
            if !replace {
                continue;
            }
            if let Some(details) = &report.snapshot.details
                && site.study_paid.insert(report.recipient)
                && let Some(corp) = self.players.get_mut(&report.recipient)
            {
                // Data is useful even if no project is currently running. The
                // existing dossier ledger persists it and enforces prerequisites.
                corp.research
                    .recover_dossier(&details.programme, details.research_fraction);
            }
            // This is the single evidence boundary for licenses and leads.
            // Merely identifying the opportunity grants neither. The in-flight
            // snapshot, not live site truth, supplies every revealed coordinate.
            let discoveries = report.snapshot.details.as_ref().filter(|d| d.studied)
                .map(|d| (d.opportunity.as_ref().and_then(|o| o.blueprint), d.lead.clone()));
            let emitted_at = report.snapshot.reported_at;
            site.known.insert(report.recipient, report.snapshot);
            if let Some((blueprint, lead)) = discoveries {
                if let Some(module) = blueprint
                    && let Some(corp) = self.players.get_mut(&report.recipient)
                { corp.research.blueprints.insert(module); }
                if let Some(lead) = lead
                    && let Some(next) = self.exploration.sites.get_mut(&lead.site)
                {
                    next.known.entry(report.recipient).or_insert(SiteReport {
                        id: lead.site, pos: lead.pos, reported_at: emitted_at, details: None,
                    });
                }
            }
        }
    }

    pub(super) fn issue_expedition(
        &mut self,
        player: PlayerId,
        fleet: EntityId,
        site: EntityId,
        task: ExpeditionTask,
        events: &mut Vec<Event>,
    ) {
        // Issue-time authorization uses only the commander's arrived picture.
        // Guessing an unseen ID must neither route to it nor expose its contents.
        let Some(report) = self
            .exploration
            .sites
            .get(&site)
            .and_then(|s| s.known.get(&player))
        else {
            return;
        };
        if task != ExpeditionTask::Investigate && report.details.is_none() {
            return;
        }
        let Some(hull) = self.fleets.get(&fleet).filter(|f| f.owner == player) else {
            return;
        };
        if (matches!(task, ExpeditionTask::Investigate | ExpeditionTask::Study) && !hull.contains(ShipKind::Scout))
            || (!matches!(task, ExpeditionTask::Investigate | ExpeditionTask::Study) && !hull.has_freighter())
        {
            return;
        }
        let station = report.pos;
        self.schedule_for_owner(
            player,
            fleet,
            FleetOrder::Expedition {
                site,
                station,
                task,
                dwell_since: None,
            },
            crate::event::OrderKind::Survey,
            events,
        );
    }

    pub(super) fn tick_exploration_sites(&mut self, events: &mut Vec<Event>) {
        self.deliver_site_reports();
        self.tick_discovery_guardians();
        self.autonomous_exploration_safety(events);
        if self.time >= self.exploration.next_sensing_at {
            self.exploration.next_sensing_at = self.time + SENSING_PERIOD;
            self.seed_exploration_sites(); // idempotent, also covers overflow home joins
            let mut lost_outposts = Vec::new();
            for site in self.exploration.sites.values_mut() {
                if site
                    .restored_sensor
                    .is_some_and(|id| !self.emplacements.iter().any(|e| e.id == id))
                {
                    site.restored_sensor = None;
                    if let Some(owner) = site.details.restored_by.take() {
                        site.revision += 1;
                        lost_outposts.push((site.id, owner, site.pos));
                    }
                }
            }
            // Loss of an outpost emits a last report; its former owner learns
            // that it needs restoration only after the same return-light delay.
            for (site, owner, pos) in lost_outposts {
                self.queue_site_report(site, owner, pos);
            }
            let mut observations = Vec::new();
            for (&player, corp) in &self.players {
                let mut sources = vec![(corp.command_center, self.config.sensor_range)];
                sources.extend(
                    self.fleets
                        .values()
                        .filter(|f| f.owner == player)
                        .filter_map(|f| {
                            if f.contains(ShipKind::Scout) {
                                Some((f.pos, SCOUT_CONTACT_RANGE * if f.has_spectrometer() { 3.0 } else { f.exploration_range_mult() }))
                            } else if f.projects_sensor() {
                                Some((f.pos, self.config.sensor_range * f.sensor_mult()))
                            } else {
                                None
                            }
                        }),
                );
                sources.extend(
                    self.emplacements
                        .iter()
                        .filter(|e| e.owner == player)
                        .map(|e| (e.pos, e.kind.sensor_range())),
                );
                for site in self.exploration.sites.values() {
                    let detailed = site.surveyed.contains(&player);
                    let key = (if detailed { site.revision } else { 0 }, detailed);
                    if site.emitted.get(&player) == Some(&key) {
                        continue;
                    }
                    let observer = sources
                        .iter()
                        .filter(|(pos, radius)| {
                            pos.distance(site.pos)
                                <= radius
                                    * self.nebula_sensor_factor(*pos)
                                    * self.nebula_signature_factor(site.pos)
                        })
                        .min_by(|(a, _), (b, _)| {
                            (a.distance(site.pos) + a.distance(corp.command_center)).total_cmp(
                                &(b.distance(site.pos) + b.distance(corp.command_center)),
                            )
                        })
                        .map(|(pos, _)| *pos);
                    if let Some(observer) = observer {
                        observations.push((site.id, player, observer));
                    }
                }
            }
            for (site, player, observer) in observations {
                self.queue_site_report(site, player, observer);
            }
        }
        let jobs: Vec<_> = self
            .fleets
            .values()
            .filter_map(|f| match f.order {
                FleetOrder::Expedition {
                    site,
                    station,
                    task,
                    dwell_since,
                } => Some((f.id, f.owner, site, station, task, dwell_since)),
                _ => None,
            })
            .collect();
        for (id, player, site, station, task, since) in jobs {
            let Some(site_pos) = self.exploration.sites.get(&site).map(|s| s.pos) else {
                continue;
            };
            let engaged = self
                .engagements
                .values()
                .any(|e| e.attackers.contains(&id) || e.defenders.contains(&id));
            let Some(fleet) = self.fleets.get_mut(&id) else {
                continue;
            };
            if engaged
                || fleet.defense.is_some()
                || fleet.pos.distance(station) > SITE_RANGE
                || fleet.pos.distance(site_pos) > SITE_RANGE
                || fleet.vel.length_sq() > 1e-9
            {
                if let FleetOrder::Expedition { dwell_since, .. } = &mut fleet.order {
                    *dwell_since = None;
                }
                continue;
            }
            if let FleetOrder::Expedition { dwell_since, .. } = &mut fleet.order {
                dwell_since.get_or_insert(self.time);
            }
            if since.is_none_or(|t| self.time - t + 1e-9 < task.seconds()) {
                continue;
            }
            self.finish_expedition(id, player, site, task, events);
        }
    }

    pub(super) fn finish_expedition(
        &mut self,
        id: EntityId,
        player: PlayerId,
        site_id: EntityId,
        task: ExpeditionTask,
        events: &mut Vec<Event>,
    ) {
        let Some(site) = self.exploration.sites.get(&site_id) else {
            return;
        };
        let pos = site.pos;
        if matches!(task, ExpeditionTask::Study | ExpeditionTask::Extract) {
            self.finish_deep_expedition(id, player, site_id, task, events);
            return;
        }
        if task != ExpeditionTask::Investigate && site.details.guarded {
            self.reject_deep_expedition(id, player, site_id, events);
            return;
        }
        let mut changed = false;
        match task {
            ExpeditionTask::Investigate => {
                if self.fleets[&id].contains(ShipKind::Scout) {
                    changed = self
                        .exploration
                        .sites
                        .get_mut(&site_id)
                        .unwrap()
                        .surveyed
                        .insert(player);
                    if changed {
                        self.grant_captain_xp(player, id, crate::captain::CAPTAIN_XP_SURVEY);
                    }
                }
            }
            ExpeditionTask::Recover => {
                let fleet = self.fleets.get_mut(&id).unwrap();
                if site.surveyed.contains(&player) && fleet.has_freighter() {
                    let site = self.exploration.sites.get_mut(&site_id).unwrap();
                    // Both manifests are conserved, including partial loads. A
                    // full hold leaves goods on site; cargo never teleports home.
                    for (&kind, amount) in &mut site.details.cargo {
                        let taken = (*amount)
                            .min(fleet.cargo_capacity().saturating_sub(fleet.cargo_units()));
                        fleet.add_cargo(kind, taken);
                        *amount -= taken;
                        changed |= taken > 0;
                    }
                    let capacity =
                        fleet.freighter_count() * crate::module::MODULE_CONVOY_BERTHS;
                    for (&kind, amount) in &mut site.details.modules {
                        let taken =
                            (*amount).min(capacity.saturating_sub(fleet.modules.values().sum()));
                        if taken > 0 {
                            *fleet.modules.entry(kind).or_default() += taken;
                        }
                        *amount -= taken;
                        changed |= taken > 0;
                    }
                    site.details.cargo.retain(|_, n| *n > 0);
                    site.details.modules.retain(|_, n| *n > 0);
                    if changed {
                        site.revision += 1;
                    }
                }
            }
            ExpeditionTask::Restore => {
                let fleet = &self.fleets[&id];
                if site.surveyed.contains(&player)
                    && fleet.has_freighter()
                    && site.details.kind == SiteKind::Station
                    && site.details.restored_by.is_none()
                    && fleet
                        .cargo_stacks()
                        .iter()
                        .any(|c| c.commodity == C::Machinery && c.units >= RESTORE_MACHINERY)
                    && fleet
                        .cargo_stacks()
                        .iter()
                        .any(|c| c.commodity == C::Electronics && c.units >= RESTORE_ELECTRONICS)
                    && crate::emplace::site_check(
                        crate::emplace::EmplacementKind::DeepSpaceSensor,
                        pos,
                        &self.emplacements,
                    )
                    .is_ok()
                {
                    let fleet = self.fleets.get_mut(&id).unwrap();
                    fleet.remove_cargo(C::Machinery, RESTORE_MACHINERY);
                    fleet.remove_cargo(C::Electronics, RESTORE_ELECTRONICS);
                    let emplacement = self.alloc_entity_id();
                    self.emplacements.push(crate::emplace::Emplacement {
                        id: emplacement,
                        owner: player,
                        kind: crate::emplace::EmplacementKind::DeepSpaceSensor,
                        pos,
                    });
                    let site = self.exploration.sites.get_mut(&site_id).unwrap();
                    site.restored_sensor = Some(emplacement);
                    site.details.restored_by = Some(player);
                    site.revision += 1;
                    changed = true;
                }
            }
            ExpeditionTask::Study | ExpeditionTask::Extract => unreachable!(),
        }
        self.fleets.get_mut(&id).unwrap().order = FleetOrder::Idle;
        self.fleets.get_mut(&id).unwrap().vel = Vec2::ZERO;
        if !changed && task != ExpeditionTask::Investigate {
            events.push(Event::new(
                self.time,
                EventPayload::OrderRejected {
                    owner: player,
                    fleet: id,
                    target: Some(site_id),
                    reason: crate::event::OrderRejectReason::DeliveryConditionsChanged,
                },
            ));
        }
        // A failed recovery can still find that a rival cleared the cache. Its
        // fresh report goes home on the same light, never an instant rejection.
        self.queue_site_report(site_id, player, self.fleets[&id].pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utility_caches_migrate_once_without_rerolling_existing_discoveries() {
        let mut w = World::new(SimConfig::for_players(822, 4));
        for home in &w.home_slots {
            assert!(w.exploration.sites.values().any(|s| s.pos.distance(home.pos) < 35_000.0
                && s.details.modules.keys().any(|m| m.is_utility())), "an early utility prize near each home");
        }
        let old = w.exploration.sites.clone();
        w.exploration.utility_seeded_homes.clear(); // a pre-utility checkpoint
        w.seed_exploration_sites();
        assert_eq!(w.exploration.sites.len(), old.len() + w.home_slots.len());
        for (id, site) in old {
            assert_eq!(serde_json::to_value(site).unwrap(), serde_json::to_value(&w.exploration.sites[&id]).unwrap());
        }
        let migrated = serde_json::to_value(&w.exploration).unwrap();
        w.seed_exploration_sites();
        assert_eq!(serde_json::to_value(&w.exploration).unwrap(), migrated);
    }

    #[test]
    fn cargo_pod_caches_append_once_and_preserve_every_existing_discovery() {
        let mut w = World::new(SimConfig::for_players(823, 4));
        assert_eq!(w.exploration.cargo_seeded_homes.len(), w.home_slots.len());
        for home in &w.home_slots {
            assert!(w.exploration.sites.values().any(|s| s.pos.distance(home.pos) < 40_000.0
                && s.details.modules.get(&M::CargoPods) == Some(&1)));
        }
        // A depleted old site stays depleted, even across checkpoint migration.
        let depleted = *w.exploration.sites.keys().next().unwrap();
        w.exploration.sites.get_mut(&depleted).unwrap().details.modules.clear();
        let old = w.exploration.sites.clone();
        let mut saved = serde_json::to_value(&w.exploration).unwrap();
        saved.as_object_mut().unwrap().remove("cargo_seeded_homes");
        w.exploration = serde_json::from_value(saved).unwrap();
        w.seed_exploration_sites();
        assert_eq!(w.exploration.sites.len(), old.len() + w.home_slots.len());
        for (id, site) in old {
            assert_eq!(serde_json::to_value(site).unwrap(), serde_json::to_value(&w.exploration.sites[&id]).unwrap());
        }
        let migrated = serde_json::to_value(&w.exploration).unwrap();
        w.seed_exploration_sites();
        assert_eq!(serde_json::to_value(&w.exploration).unwrap(), migrated);
    }

    #[test]
    fn recovered_utilities_are_physical_finite_crates_not_instant_unlocks() {
        for module in [M::ExtendedTanks, M::ReconSuite, M::CargoPods] {
            let (mut w, owner, site, scout) = scene(SiteKind::Derelict);
            w.exploration.sites.get_mut(&site).unwrap().details.modules = [(module, 1)].into();
            investigate(&mut w, owner, site, scout);
            assert!(!crate::research::has_module(&w.players[&owner].research, module));
            let pos = w.fleets[&scout].pos;
            w.fleets.insert(scout, Fleet::single(scout, owner, ShipKind::Convoy, pos, FleetOrder::Idle, None));
            w.finish_expedition(scout, owner, site, ExpeditionTask::Recover, &mut vec![]);
            assert_eq!(w.fleets[&scout].modules[&module], 1);
            assert!(!w.exploration.sites[&site].details.modules.contains_key(&module));
            assert!(!w.systems.iter().any(|s| s.owner == Some(owner) && s.modules.get(&module).copied().unwrap_or(0) > 0));
            w.finish_expedition(scout, owner, site, ExpeditionTask::Recover, &mut vec![]);
            assert_eq!(w.fleets[&scout].modules[&module], 1, "no repeat loot");
        }
    }

    #[test]
    fn recon_discovers_farther_contacts_but_the_report_still_waits_for_light() {
        let (mut w, owner, site, scout) = scene(SiteKind::Derelict);
        let pos = w.players[&owner].home + Vec2::new(300_000.0, 0.0);
        w.exploration.sites.get_mut(&site).unwrap().pos = pos;
        w.fleets.retain(|id, _| *id == scout);
        w.fleets.get_mut(&scout).unwrap().pos = pos - Vec2::new(45_000.0, 0.0);
        w.exploration.next_sensing_at = w.time;
        w.tick_exploration_sites(&mut vec![]);
        assert!(w.exploration.pending.is_empty());
        w.fleets.get_mut(&scout).unwrap().set_fitted(ShipKind::Scout, &crate::module::Loadout::new(vec![M::ReconSuite]), 1);
        w.exploration.next_sensing_at = w.time;
        w.tick_exploration_sites(&mut vec![]);
        assert_eq!(w.exploration.pending.len(), 1);
        assert!(w.exploration.pending[0].arrival > w.time + 100.0);
        assert!(w.exploration.reports_for(owner).is_empty());
        arrive(&mut w);
        assert_eq!(w.exploration.reports_for(owner).len(), 1);
    }

    #[test]
    fn recon_retreats_from_threats_beyond_the_stock_lookout() {
        let (mut w, owner, site, scout) = scene(SiteKind::Derelict);
        w.nebulas.clear();
        let pos = w.players[&owner].home + Vec2::new(300_000.0, 0.0);
        w.fleets.get_mut(&scout).unwrap().pos = pos;
        w.fleets.get_mut(&scout).unwrap().order = FleetOrder::Expedition {
            site, station: pos, task: ExpeditionTask::Investigate, dwell_since: None,
        };
        let id = w.alloc_entity_id();
        let mut pirate = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, pos + Vec2::new(30_000.0, 0.0), FleetOrder::Idle, None);
        pirate.vel = Vec2::new(0.0, pirate.max_speed());
        w.fleets.insert(id, pirate);
        w.autonomous_exploration_safety(&mut vec![]);
        assert!(matches!(w.fleets[&scout].order, FleetOrder::Expedition { .. }));
        w.fleets.get_mut(&scout).unwrap().set_fitted(ShipKind::Scout, &crate::module::Loadout::new(vec![M::ReconSuite]), 1);
        w.autonomous_exploration_safety(&mut vec![]);
        assert!(matches!(w.fleets[&scout].order, FleetOrder::MoveTo { .. }));
    }

    fn scene(kind: SiteKind) -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(822, 4));
        let player = PlayerId(99_510);
        w.step(&[Command::AddPlayer {
            id: player,
            name: "Explorers".into(),
        }]);
        w.exploration.sites.clear();
        w.exploration.pending.clear();
        let pos = w.players[&player].home + Vec2::new(40_000.0, 0.0);
        w.create_exploration_site(pos, kind, 0);
        let site = *w.exploration.sites.keys().next().unwrap();
        let fleet = w.alloc_entity_id();
        w.fleets.insert(
            fleet,
            Fleet::single(fleet, player, ShipKind::Scout, pos, FleetOrder::Idle, None),
        );
        (w, player, site, fleet)
    }

    fn arrive(w: &mut World) {
        w.time = w
            .exploration
            .pending
            .iter()
            .map(|r| r.arrival)
            .fold(w.time, f64::max);
        w.deliver_site_reports();
    }

    fn investigate(w: &mut World, player: PlayerId, site: EntityId, fleet: EntityId) {
        w.finish_expedition(
            fleet,
            player,
            site,
            ExpeditionTask::Investigate,
            &mut Vec::new(),
        );
        arrive(w);
    }

    #[test]
    fn generation_adds_four_off_system_contacts_per_home_without_rerolling_or_reminting() {
        for seed in [1, 21, 822, 951] {
            let mut w = World::new(SimConfig::for_players(seed, 4));
            assert!(w.exploration.sites.len() >= 36);
            for home in &w.home_slots {
                assert!(
                    w.exploration
                        .sites
                        .values()
                        .filter(|s| s.pos.distance(home.pos) <= 30_000.0)
                        .count()
                        >= 4
                );
            }
            for site in w.exploration.sites.values() {
                assert!(w.systems.iter().all(|s| s.pos.distance(site.pos) > 3_000.0));
                if site.details.kind == SiteKind::Anomaly {
                    assert!(w.nebulas.iter().any(|n| n.contains(site.pos)));
                }
            }
            let a = serde_json::to_string(&w.exploration).unwrap();
            w.seed_exploration_sites();
            assert_eq!(a, serde_json::to_string(&w.exploration).unwrap());
            let other = World::new(SimConfig::for_players(seed, 4));
            assert_eq!(a, serde_json::to_string(&other.exploration).unwrap());
        }
    }

    #[test]
    fn a_contact_and_its_identification_each_wait_for_their_own_light() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.exploration.sites[&site].pos;
        w.queue_site_report(site, p, pos);
        assert!(w.exploration.reports_for(p).is_empty());
        let at = w.exploration.pending[0].arrival;
        w.time = at - 1e-4;
        w.deliver_site_reports();
        assert!(w.exploration.reports_for(p).is_empty());
        w.time = at;
        w.deliver_site_reports();
        assert!(w.exploration.reports_for(p)[0].details.is_none());
        w.finish_expedition(fleet, p, site, ExpeditionTask::Investigate, &mut Vec::new());
        assert!(w.exploration.reports_for(p)[0].details.is_none());
        assert!(w.players[&p].research.recovered_data.is_empty());
        arrive(&mut w);
        let report = &w.exploration.reports_for(p)[0];
        assert_eq!(report.details.as_ref().unwrap().kind, SiteKind::Derelict);
        assert!(!w.players[&p].research.recovered_data.is_empty());
        assert!(w.exploration.reports_for(PlayerId(99_511)).is_empty());
    }

    #[test]
    fn guessing_a_site_or_ordering_another_players_fleet_reveals_nothing() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let mut events = Vec::new();
        w.apply(
            &Command::ExploreSite {
                player_id: p,
                fleet_id: fleet,
                site_id: site,
                task: ExpeditionTask::Investigate,
            },
            &mut events,
        );
        assert!(events.is_empty());
        assert!(w.pending_orders.is_empty());
        let pos = w.exploration.sites[&site].pos;
        w.queue_site_report(site, p, pos);
        arrive(&mut w);
        w.fleets.get_mut(&fleet).unwrap().owner = PlayerId(99_511);
        w.issue_expedition(p, fleet, site, ExpeditionTask::Investigate, &mut events);
        assert!(events.is_empty());
        assert!(w.pending_orders.is_empty());
    }

    #[test]
    fn expedition_travel_and_dwell_are_a_delayed_cancelable_fleet_order() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.exploration.sites[&site].pos;
        w.queue_site_report(site, p, pos);
        arrive(&mut w);
        w.apply(
            &Command::MoveShip {
                player_id: p,
                ship_id: fleet,
                dest: pos,
            },
            &mut Vec::new(),
        );
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::Idle));
        assert_eq!(w.pending_orders.len(), 1);
        // Use the genuine movement/receipt pipeline, not an immediate assignment.
        for _ in 0..((22.0 / DT) as usize) {
            w.step(&[]);
        }
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition { .. }
        ));
        assert!(!w.exploration.sites[&site].surveyed.contains(&p));
        // Leaving the site resets work, so 19s + a departure cannot equal a survey.
        if let FleetOrder::Expedition { dwell_since, .. } =
            &mut w.fleets.get_mut(&fleet).unwrap().order
        {
            *dwell_since = Some(w.time - 19.0);
        }
        w.fleets.get_mut(&fleet).unwrap().pos = pos + Vec2::new(400.0, 0.0);
        w.tick_exploration_sites(&mut Vec::new());
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition {
                dwell_since: None,
                ..
            }
        ));
        w.fleets.get_mut(&fleet).unwrap().pos = pos;
        for _ in 0..((22.0 / DT) as usize) {
            w.step(&[]);
        }
        assert!(w.exploration.sites[&site].surveyed.contains(&p));
        assert!(w.exploration.sites[&site].known[&p].details.is_none());
        for _ in 0..((22.0 / DT) as usize) {
            w.step(&[]);
        }
        assert!(w.exploration.sites[&site].known[&p].details.is_some());
    }

    #[test]
    fn arrival_work_requires_a_known_deliberate_destination_not_a_pass_or_idle_hold() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.exploration.sites[&site].pos;
        assert!(
            w.exploration_arrival_order(fleet, pos).is_none(),
            "unreceived contact"
        );
        w.queue_site_report(site, p, pos);
        arrive(&mut w);
        assert!(matches!(
            w.exploration_arrival_order(fleet, pos),
            Some(FleetOrder::Expedition {
                task: ExpeditionTask::Investigate,
                ..
            })
        ));
        assert!(
            w.exploration_arrival_order(fleet, pos + Vec2::new(2_000.0, 0.0))
                .is_none()
        );
        // Merely occupying/crossing the work radius is never an instruction to work.
        for order in [
            FleetOrder::Idle,
            FleetOrder::MoveTo {
                dest: pos + Vec2::new(2_000.0, 0.0),
            },
        ] {
            let before = serde_json::to_string(&order).unwrap();
            w.fleets.get_mut(&fleet).unwrap().order = order;
            w.time += STUDY_SECONDS + 1.0;
            w.tick_exploration_sites(&mut Vec::new());
            assert_eq!(
                serde_json::to_string(&w.fleets[&fleet].order).unwrap(),
                before
            );
            assert!(!w.exploration.sites[&site].surveyed.contains(&p));
        }
    }

    #[test]
    fn an_expedition_retreats_before_completion_and_never_restarts_itself() {
        for kind in [ShipKind::Scout, ShipKind::Convoy] {
            let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
            let pos = w.exploration.sites[&site].pos;
            let task = if kind == ShipKind::Scout {
                ExpeditionTask::Investigate
            } else {
                ExpeditionTask::Recover
            };
            if kind == ShipKind::Convoy {
                investigate(&mut w, p, site, fleet);
            }
            w.time += task.seconds();
            let order = FleetOrder::Expedition {
                site,
                station: pos,
                task,
                dwell_since: Some(w.time - task.seconds()),
            };
            w.fleets
                .insert(fleet, Fleet::single(fleet, p, kind, pos, order, None));
            let pirate = w.alloc_entity_id();
            let danger = pos + Vec2::new(1_000.0, 0.0);
            assert!(!w.in_sovereign_zone(danger));
            w.fleets.insert(
                pirate,
                Fleet::single(
                    pirate,
                    PlayerId::PIRATE,
                    ShipKind::Raider,
                    danger,
                    FleetOrder::Intercept { target: fleet },
                    None,
                ),
            );
            let before = w.exploration.sites[&site].details.clone();
            let mut events = Vec::new();
            // The hostile appeared on the very tick that would have paid out.
            w.tick_exploration_sites(&mut events);
            let FleetOrder::MoveTo { dest } = w.fleets[&fleet].order else {
                panic!("retreat must win over completing site work");
            };
            assert!(
                dest.x < pos.x,
                "retreat points away from the observed hostile"
            );
            assert_eq!(
                w.fleets[&fleet].pos, pos,
                "no teleport; normal drives perform the retreat"
            );
            assert_eq!(w.exploration.sites[&site].details, before);
            assert_eq!(w.fleets[&fleet].cargo_units(), 0);
            if kind == ShipKind::Scout {
                assert!(!w.exploration.sites[&site].surveyed.contains(&p));
                assert!(!w.exploration.sites[&site].study_paid.contains(&p));
            }
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e.payload, EventPayload::OrderRejected {
                fleet: id, reason: crate::event::OrderRejectReason::ExplorationThreat, ..
            } if id == fleet))
            );
            // Losing sight of the pirate doesn't install the abandoned job again.
            w.fleets.remove(&pirate);
            w.time += task.seconds() + 1.0;
            w.tick_exploration_sites(&mut events);
            assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { dest: d } if d == dest));
            w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Idle;
            w.tick_exploration_sites(&mut events);
            assert!(matches!(w.fleets[&fleet].order, FleetOrder::Idle));
        }
    }

    #[test]
    fn expedition_lookout_ignores_friendlies_civilians_and_unseen_threats() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.fleets[&fleet].pos;
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Expedition {
            site,
            station: pos,
            task: ExpeditionTask::Investigate,
            dwell_since: None,
        };
        for (owner, kind, distance) in [
            (p, ShipKind::Raider, 1_000.0),
            (PlayerId::TCA, ShipKind::Raider, 1_000.0),
            (PlayerId::PIRATE, ShipKind::Convoy, 1_000.0),
            (
                PlayerId::PIRATE,
                ShipKind::Raider,
                EXPEDITION_LOOKOUT_RANGE * 3.0,
            ),
        ] {
            let id = w.alloc_entity_id();
            w.fleets.insert(
                id,
                Fleet::single(
                    id,
                    owner,
                    kind,
                    pos + Vec2::new(distance, 0.0),
                    FleetOrder::Idle,
                    None,
                ),
            );
        }
        w.autonomous_exploration_safety(&mut Vec::new());
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition { .. }
        ));
        let pirate = w.alloc_entity_id();
        let mut threat = Fleet::single(
            pirate,
            PlayerId::PIRATE,
            ShipKind::Cruiser,
            pos + Vec2::new(EXPEDITION_LOOKOUT_RANGE - 1.0, 0.0),
            FleetOrder::Idle,
            None,
        );
        // Its CURRENT position crossed the radius, but local light still places
        // this broadcasting capital outside. A truth-radius test would flee early.
        threat.vel = Vec2::new(-500.0, 0.0);
        w.fleets.insert(pirate, threat);
        w.autonomous_exploration_safety(&mut Vec::new());
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition { .. }
        ));
        w.fleets.get_mut(&pirate).unwrap().pos = pos + Vec2::new(1_000.0, 0.0);
        w.fleets.get_mut(&pirate).unwrap().vel = Vec2::ZERO;
        w.autonomous_exploration_safety(&mut Vec::new());
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { .. }));
    }

    #[test]
    fn armed_expeditions_keep_the_existing_retreat_odds_policy() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.fleets[&fleet].pos;
        w.fleets.get_mut(&fleet).unwrap().add(ShipKind::Raider, 1);
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Expedition {
            site,
            station: pos,
            task: ExpeditionTask::Investigate,
            dwell_since: None,
        };
        let pirate = w.alloc_entity_id();
        let mut enemy = Fleet::single(
            pirate,
            PlayerId::PIRATE,
            ShipKind::Raider,
            pos + Vec2::new(1_000.0, 0.0),
            FleetOrder::Idle,
            None,
        );
        enemy.add(ShipKind::Raider, 4);
        w.fleets.insert(pirate, enemy);
        w.players.get_mut(&p).unwrap().doctrine.retreat = crate::doctrine::RetreatThreshold::Never;
        w.autonomous_exploration_safety(&mut Vec::new());
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition { .. }
        ));
        w.players.get_mut(&p).unwrap().doctrine.retreat = crate::doctrine::RetreatThreshold::Half;
        w.autonomous_exploration_safety(&mut Vec::new());
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { .. }));
    }

    #[test]
    fn arrival_recovery_respects_capacity_and_never_spends_restoration_materials() {
        let (mut w, p, site, fleet) = scene(SiteKind::Station);
        investigate(&mut w, p, site, fleet);
        let pos = w.fleets[&fleet].pos;
        w.fleets.insert(
            fleet,
            Fleet::single(fleet, p, ShipKind::Convoy, pos, FleetOrder::Idle, None),
        );
        assert!(matches!(
            w.exploration_arrival_order(fleet, pos),
            Some(FleetOrder::Expedition {
                task: ExpeditionTask::Recover,
                ..
            })
        ));
        let capacity = w.fleets[&fleet].cargo_capacity();
        w.fleets
            .get_mut(&fleet)
            .unwrap()
            .add_cargo(C::Provisions, capacity);
        assert!(
            w.exploration_arrival_order(fleet, pos).is_none(),
            "no room to recover"
        );
        w.fleets
            .get_mut(&fleet)
            .unwrap()
            .remove_cargo(C::Provisions, capacity);
        w.fleets
            .get_mut(&fleet)
            .unwrap()
            .add_cargo(C::Machinery, RESTORE_MACHINERY);
        w.fleets
            .get_mut(&fleet)
            .unwrap()
            .add_cargo(C::Electronics, RESTORE_ELECTRONICS);
        let d = w
            .exploration
            .sites
            .get_mut(&site)
            .unwrap()
            .known
            .get_mut(&p)
            .unwrap()
            .details
            .as_mut()
            .unwrap();
        d.cargo.clear();
        d.modules.clear();
        assert!(
            w.exploration_arrival_order(fleet, pos).is_none(),
            "restoration is never automatic"
        );
        assert_eq!(
            w.fleets[&fleet].cargo_amount(C::Machinery),
            RESTORE_MACHINERY
        );
    }

    #[test]
    fn jumping_to_a_contact_starts_arrival_work_but_moving_on_site_does_not_count_as_work() {
        let (mut w, p, site, fleet) = scene(SiteKind::Derelict);
        let pos = w.fleets[&fleet].pos;
        w.queue_site_report(site, p, pos);
        arrive(&mut w);
        w.time += 30.0;
        w.fleets.get_mut(&fleet).unwrap().pos = pos + Vec2::new(2_000.0, 0.0);
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Jump {
            dest: pos,
            spool_started: Some(w.time - 30.0),
        };
        w.resolve_jumps(&mut Vec::new());
        assert_eq!(w.fleets[&fleet].pos, pos);
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition { .. }
        ));
        if let FleetOrder::Expedition { dwell_since, .. } =
            &mut w.fleets.get_mut(&fleet).unwrap().order
        {
            *dwell_since = Some(w.time - STUDY_SECONDS);
        }
        w.fleets.get_mut(&fleet).unwrap().vel = Vec2::new(10.0, 0.0);
        w.tick_exploration_sites(&mut Vec::new());
        assert!(matches!(
            w.fleets[&fleet].order,
            FleetOrder::Expedition {
                dwell_since: None,
                ..
            }
        ));
        assert!(!w.exploration.sites[&site].surveyed.contains(&p));
        // Clicking an approach point must not add a second work radius: a hull
        // stalled short of that point is still outside the actual site's reach.
        w.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Expedition {
            site,
            station: pos + Vec2::new(SITE_RANGE, 0.0),
            task: ExpeditionTask::Investigate,
            dwell_since: Some(w.time - STUDY_SECONDS),
        };
        w.fleets.get_mut(&fleet).unwrap().pos = pos + Vec2::new(SITE_RANGE + 1.0, 0.0);
        w.fleets.get_mut(&fleet).unwrap().vel = Vec2::ZERO;
        w.tick_exploration_sites(&mut Vec::new());
        assert!(!w.exploration.sites[&site].surveyed.contains(&p));
    }

    #[test]
    fn recovery_is_finite_capacity_limited_and_the_empty_site_waits_for_light() {
        let (mut w, p, site, scout) = scene(SiteKind::Derelict);
        investigate(&mut w, p, site, scout);
        let pos = w.exploration.sites[&site].pos;
        let cargo = w.exploration.sites[&site].details.cargo.clone();
        let id = w.alloc_entity_id();
        let mut f = Fleet::single(id, p, ShipKind::Convoy, pos, FleetOrder::Idle, None);
        f.add_cargo(C::Provisions, f.cargo_capacity() - 5);
        w.fleets.insert(id, f);
        w.finish_expedition(id, p, site, ExpeditionTask::Recover, &mut Vec::new());
        assert_eq!(w.fleets[&id].cargo_units(), w.fleets[&id].cargo_capacity());
        assert_eq!(
            w.exploration.sites[&site].details.cargo[&C::Alloys],
            cargo[&C::Alloys] - 5
        );
        assert_eq!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .cargo,
            cargo
        );
        assert_eq!(w.fleets[&id].modules[&M::WhippleArmor], 1);
        w.fleets
            .get_mut(&id)
            .unwrap()
            .remove_cargo(C::Provisions, 250);
        w.finish_expedition(id, p, site, ExpeditionTask::Recover, &mut Vec::new());
        assert!(w.exploration.sites[&site].details.cargo.is_empty());
        assert!(w.exploration.sites[&site].details.modules.is_empty());
        let aboard = w.fleets[&id].cargo_units();
        w.finish_expedition(id, p, site, ExpeditionTask::Recover, &mut Vec::new());
        assert_eq!(w.fleets[&id].cargo_units(), aboard);
        assert_eq!(w.fleets[&id].modules[&M::WhippleArmor], 1);
        arrive(&mut w);
        assert!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .cargo
                .is_empty()
        );
        assert!(
            w.players[&p].warehouse.is_empty(),
            "the shipment still sits aboard the recovering hull"
        );
    }

    #[test]
    fn outpost_restoration_consumes_delivered_materials_once_and_is_news_gated() {
        let (mut w, p, site, scout) = scene(SiteKind::Station);
        investigate(&mut w, p, site, scout);
        let pos = w.exploration.sites[&site].pos;
        let id = w.alloc_entity_id();
        w.fleets.insert(
            id,
            Fleet::single(id, p, ShipKind::Convoy, pos, FleetOrder::Idle, None),
        );
        w.finish_expedition(id, p, site, ExpeditionTask::Restore, &mut Vec::new());
        assert!(w.exploration.sites[&site].details.restored_by.is_none());
        w.fleets
            .get_mut(&id)
            .unwrap()
            .add_cargo(C::Machinery, RESTORE_MACHINERY);
        w.fleets
            .get_mut(&id)
            .unwrap()
            .add_cargo(C::Electronics, RESTORE_ELECTRONICS);
        let before = w.emplacements.len();
        w.finish_expedition(id, p, site, ExpeditionTask::Restore, &mut Vec::new());
        assert_eq!(w.emplacements.len(), before + 1);
        assert_eq!(w.fleets[&id].cargo_units(), 0);
        assert!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .restored_by
                .is_none()
        );
        w.finish_expedition(id, p, site, ExpeditionTask::Restore, &mut Vec::new());
        assert_eq!(w.emplacements.len(), before + 1);
        arrive(&mut w);
        assert_eq!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .restored_by,
            Some(p)
        );
    }

    #[test]
    fn snapshots_preserve_unarrived_reports_finite_loot_and_paid_research() {
        let (mut w, p, site, scout) = scene(SiteKind::Precursor);
        w.finish_expedition(scout, p, site, ExpeditionTask::Investigate, &mut Vec::new());
        let save = serde_json::to_string(&w).unwrap();
        let mut loaded: World = serde_json::from_str(&save).unwrap();
        loaded.fixup_after_load();
        assert!(loaded.exploration.reports_for(p).is_empty());
        arrive(&mut loaded);
        let dossier = loaded.players[&p].research.recovered_data.clone();
        assert!(dossier.contains_key("hull_line_v_cruiser"));
        let site_count = loaded.exploration.sites.len();
        loaded.finish_expedition(scout, p, site, ExpeditionTask::Investigate, &mut Vec::new());
        loaded.fixup_after_load();
        arrive(&mut loaded);
        assert_eq!(loaded.players[&p].research.recovered_data, dossier);
        assert_eq!(loaded.exploration.sites.len(), site_count);
        assert!(
            !loaded.players[&p].research.has("hull_line_v_cruiser"),
            "data does not grant the technology"
        );
    }

    #[test]
    fn restoring_a_lost_outpost_requires_fresh_materials_and_loss_news_waits_for_light() {
        let (mut w, p, site, scout) = scene(SiteKind::Station);
        investigate(&mut w, p, site, scout);
        let pos = w.exploration.sites[&site].pos;
        let id = w.alloc_entity_id();
        let mut fleet = Fleet::single(id, p, ShipKind::Convoy, pos, FleetOrder::Idle, None);
        fleet.add_cargo(C::Machinery, RESTORE_MACHINERY);
        fleet.add_cargo(C::Electronics, RESTORE_ELECTRONICS);
        w.fleets.insert(id, fleet);
        w.finish_expedition(id, p, site, ExpeditionTask::Restore, &mut Vec::new());
        arrive(&mut w);
        let sensor = w.exploration.sites[&site].restored_sensor.unwrap();
        w.emplacements.retain(|e| e.id != sensor);
        w.exploration.next_sensing_at = w.time;
        w.tick_exploration_sites(&mut Vec::new());
        assert!(w.exploration.sites[&site].details.restored_by.is_none());
        assert_eq!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .restored_by,
            Some(p)
        );
        arrive(&mut w);
        assert!(
            w.exploration.sites[&site].known[&p]
                .details
                .as_ref()
                .unwrap()
                .restored_by
                .is_none()
        );
        w.finish_expedition(id, p, site, ExpeditionTask::Restore, &mut Vec::new());
        assert!(
            w.exploration.sites[&site].restored_sensor.is_none(),
            "spent supplies cannot restore a second time"
        );
    }

    #[test]
    fn an_existing_galaxy_gains_sites_without_moving_systems_or_requiring_a_reset() {
        let original = World::new(SimConfig::for_players(822, 4));
        let positions: Vec<_> = original.systems.iter().map(|s| (s.id, s.pos)).collect();
        let mut saved = serde_json::to_value(&original).unwrap();
        saved.as_object_mut().unwrap().remove("exploration");
        let mut loaded: World = serde_json::from_value(saved).unwrap();
        assert!(loaded.exploration.sites.is_empty());
        loaded.fixup_after_load();
        assert_eq!(
            loaded
                .systems
                .iter()
                .map(|s| (s.id, s.pos))
                .collect::<Vec<_>>(),
            positions
        );
        assert!(loaded.exploration.sites.len() >= 36);
        let site_count = loaded.exploration.sites.len();
        loaded.fixup_after_load();
        assert_eq!(loaded.exploration.sites.len(), site_count);
        assert!(loaded.exploration.sites.keys().all(
            |id| !loaded.fleets.contains_key(id) && loaded.systems.iter().all(|s| s.id != *id)
        ));
    }
}
