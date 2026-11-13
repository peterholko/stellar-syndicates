//! §gun-platform (Peter, Oct 30 2026: "if the player hasn't built enough
//! defenses, game over and they can restart"): founding a corporation, and
//! founding it again where it stood once its colony has fallen to the
//! uncle's finale. A restart keeps the account's seat (its home slot), name,
//! syndicate and medals; everything the corporation held is gone, and the
//! home, its den and their belts are as a new founder finds them.
use super::*;

impl World {
    /// A new corporation at `home_system` (its slot already taken): the
    /// founding kit, the landed colony ship, its captain and its charter.
    pub(super) fn admit_corporation(&mut self, id: PlayerId, name: &str, home: Vec2, home_system: EntityId,
        invited_by: Option<PlayerId>, events: &mut Vec<Event>)
    {
        let legacy_test_bootstrap = self.uses_legacy_test_bootstrap();
        // Requisitions reach a corporation from its second day; one founded
        // once they run starts with two of them in hand (§season).
        let late_grant = if legacy_test_bootstrap { 0.0 } else { crate::season::late_charter_grant(self.season.grip) };
        // Seed the home system with the founding kit's fuel so fleets
        // move from turn one (§fuel-overhaul): Uranium for slow freight
        // until the home's mill runs, a little Deuterium for a fast trip.
        // Like all inflow, the seed respects the storage cap (§buildings
        // step 2); the base cap comfortably exceeds the seed, so a fresh
        // home always receives it in full.
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == home_system) {
            if !legacy_test_bootstrap {
                // Unused home slots can come from older snapshots. Apply
                // today's package on FIRST JOIN only, behind the existing
                // idempotence guard; never shrink an existing corporation.
                // §charter-crates: the holds carry the Landfall crate; the
                // Authority releases the rest a chapter at a time.
                sys.stockpile = crate::founding::CharterCrate::Landfall.stock();
                sys.set_population(crate::colony::HOME_FOUNDING_POP);
                if sys.trait_ == Some(crate::explore::SystemTrait::UnstableGeology) {
                    sys.trait_ = None;
                }
            }
            sys.seed_command_centers();
            if !legacy_test_bootstrap {
                // §charter-v4: only the landed colony ship stands.
                sys.bare_landing();
            }
            {
                let seed = crate::fuel::URANIUM_HOME_SEED.min(sys.storage_headroom());
                *sys.stockpile.entry(crate::fuel::FISSION_FUEL).or_insert(0.0) += seed;
            }
            #[cfg(test)]
            if legacy_test_bootstrap {
                // Existing mechanics tests need a ready sandbox; the
                // dedicated founding tests below exercise the real lean
                // package with this switch off.
                sys.set_tier(crate::build::StructureKind::Shipyard, 1);
                sys.set_tier(crate::build::StructureKind::MiningComplex, 1);
                for (commodity, units) in [
                    (crate::cargo::Commodity::Provisions, 60.0),
                    (crate::cargo::Commodity::Machinery, 40.0),
                    (crate::cargo::Commodity::Alloys, 100.0),
                    (crate::cargo::Commodity::Polymers, 30.0),
                    (crate::cargo::Commodity::Electronics, 150.0),
                    (crate::cargo::Commodity::Deuterium, 154.0),
                    // §reactors-drives: every warp hull takes engines.
                    (crate::cargo::Commodity::FissionReactor, 20.0),
                    (crate::cargo::Commodity::ZephyrDrive, 20.0),
                    // §hull-ladder: two Tiny Freighters' kits — frame, bay,
                    // plating, precision parts — inside the same 600 units
                    // (the sandbox home's storage is nearly full; 26 units of
                    // Deuterium made room). No guns: a Raider stays
                    // unaffordable here (`build_rejected_when_stockpile_short`).
                    (crate::cargo::Commodity::NavComputer, 2.0),
                    (crate::cargo::Commodity::HullSections, 6.0),
                    (crate::cargo::Commodity::CargoBay, 2.0),
                    (crate::cargo::Commodity::Composites, 12.0),
                    (crate::cargo::Commodity::PrecisionComponents, 4.0),
                ] {
                    sys.stockpile.insert(commodity, units);
                }
                if let Some(body) = sys.bodies.iter_mut().find(|b| {
                    b.tier(crate::build::StructureKind::MiningComplex) > 0
                }) {
                    body.assignments.insert(
                        crate::build::StructureKind::MiningComplex,
                        crate::production::Assignment::crew(1),
                    );
                }
                // §workforce cap: a worker for every built post, so the
                // sandbox never reads as a Managed staffing shortage.
                let posts = sys.workforce_posts();
                if crate::colony::workforce_units(sys.population()) < posts {
                    sys.set_population(posts as f64 * crate::colony::POP_PER_WORKFORCE);
                }
            }
        }
        // The Market Warehouse begins empty. The opening programme makes
        // the first sale a physical achievement rather than free inventory.
        let warehouse = if legacy_test_bootstrap {
            [
                crate::cargo::Commodity::MetallicOre,
                crate::cargo::Commodity::Volatiles,
                crate::cargo::Commodity::Alloys,
                crate::cargo::Commodity::Deuterium,
                crate::cargo::Commodity::Provisions,
            ]
            .into_iter()
            .map(|c| (c, 120u32))
            .collect()
        } else {
            BTreeMap::new()
        };
        let surveyed: std::collections::BTreeSet<_> = self
            .systems
            .iter()
            .filter(|s| s.pos.distance(home) <= crate::explore::SURVEY_INITIAL_RADIUS)
            .map(|s| s.id)
            .collect();
        let initial_surveyed = surveyed.len();
        let mut survey_candidates = self
            .home_slots
            .iter()
            .find(|slot| slot.system == Some(home_system))
            .map(|slot| slot.founding_opportunities.clone())
            .unwrap_or_default();
        survey_candidates.retain(|candidate| {
            !surveyed.contains(candidate)
                && self
                    .systems
                    .iter()
                    .find(|system| system.id == *candidate)
                    .is_some_and(|system| system.owner.is_none())
        });
        // Over-capacity and migrated galaxies may not carry the seeded
        // pair. Fall back deterministically to the nearest surveyable,
        // unclaimed systems so onboarding can never dead-end.
        if survey_candidates.len() < 2 {
            let mut nearest: Vec<(f64, EntityId)> = self
                .systems
                .iter()
                .filter(|system| {
                    system.id != home_system
                        && system.owner.is_none()
                        && !surveyed.contains(&system.id)
                        && !survey_candidates.contains(&system.id)
                })
                .map(|system| (system.pos.distance(home), system.id))
                .collect();
            nearest.sort_by(|a, b| {
                a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1))
            });
            survey_candidates.extend(
                nearest
                    .into_iter()
                    .map(|(_, id)| id)
                    .take(2 - survey_candidates.len()),
            );
        }
        let initial_owned_systems = self
            .systems
            .iter()
            .filter(|system| system.owner == Some(id))
            .count();
        self.players.insert(
            id,
            Corporation {
                id,
                name: name.to_string(),
                last_active: self.time,
                armory: Vec::new(),
                joined_tick: self.tick,
                home,
                command_center: home,
                home_system: Some(home_system),
                credits: if legacy_test_bootstrap { 10_000.0 } else { 2_000.0 + late_grant },
                wires: Vec::new(),
                warehouse,
                // §TCA Phase 2: a fresh corp is issued a clean charter.
                tca_standing: crate::tca::TCA_STANDING_START,
                valuation: if legacy_test_bootstrap { 10_000.0 } else { 2_000.0 },
                standing_orders: Vec::new(),
                depot_rules: Vec::new(),
                next_standing_id: 0,
                doctrine: FleetDoctrine::default(),
                intel: BTreeMap::new(),
                scouted_stores: BTreeMap::new(),
                charted: BTreeMap::new(),
                syndicate: None,
                syndicate_prev: None,
                syndicate_since: 0.0,
                research: Default::default(),
                stats: crate::rankings::RankingStats::default(),
                medals: Default::default(),
                board: Default::default(),
                signals: Default::default(),
                // §explore: the starting valley is KNOWN (pre-surveyed);
                // the frontier isn't — that's what scouts are for.
                surveyed,
                mining: Default::default(),
                leave: Default::default(),
                standing_day: 0,
                standing_earned: 0.0,
                founding: if legacy_test_bootstrap {
                    crate::founding::FoundingProgram::default()
                } else {
                    crate::founding::FoundingProgram::new(
                        self.time,
                        initial_surveyed,
                        initial_owned_systems,
                        survey_candidates,
                    )
                },
                ledger: Default::default(),
                captain: None,
                captains: BTreeMap::new(),
                next_captain_id: 1,
                midgame_stage: crate::operation::MidgameStage::HomeDevelopment,
                raid_losses: Default::default(),
                hideouts: Default::default(),
                licences: Default::default(),
                claims: Default::default(),
            },
        );
        events.push(Event::new(
            self.time,
            EventPayload::PlayerJoined {
                id,
                name: name.to_string(),
            },
        ));
        if late_grant > 0.0 {
            events.push(Event::new(self.time, EventPayload::CharterStipend {
                owner: id,
                title: "Late charter grant".into(),
                credits: late_grant,
                standing: 0.0,
                dossier: None,
            }));
        }
        let interceptor = self.spawn_starting_fleets(id, home, events);
        self.spawn_dev_start_ships(id, home, events);
        if let Some(corp) = self.players.get_mut(&id) {
            corp.founding.interceptor = interceptor;
            corp.founding.convoy = None;
            let mut captain = crate::captain::Captain::founding(id, interceptor.unwrap_or(EntityId(0)));
            if interceptor.is_none() {
                // §charter-v4: no starting hull; the founding captain
                // waits at home for the first one.
                captain.assigned_fleet = None;
                captain.stationed_system = Some(home_system);
            }
            captain.last_pos = home;
            corp.captains.insert(captain.id, captain);
        }
        if !legacy_test_bootstrap {
            self.start_local_opening(id, home, events);
            let sandbox = self.dev_pvp_sandbox;
            let now = self.time;
            if let Some(corp) = self.players.get_mut(&id) {
                corp.founding.crates_collected.insert(crate::founding::CharterCrate::Landfall);
                if sandbox {
                    // Dev only: past the charter (its crates open on the next
                    // tick) and unshielded.
                    corp.founding.set_stage(crate::founding::FoundingStage::Complete, now);
                    corp.founding.protection_broken = true;
                }
            }
            // …with what the charter would have built, staffed.
            if sandbox && let Some(sys) = self.systems.iter_mut().find(|s| s.id == home_system) {
                use crate::build::StructureKind as K;
                if let Some(b) = sys.bodies.iter_mut().find(|b| b.tier(K::CommandCenter) > 0) {
                    for k in [K::Bioharvester, K::Habitat, K::Agroplex, K::MiningComplex, K::PowerPlant, K::Shipyard,
                        K::Warehouse, K::Vault, K::OrbitalDefense] {
                        b.set_tier(k, 1);
                    }
                    for k in [K::Bioharvester, K::Agroplex, K::MiningComplex, K::Shipyard] {
                        b.assignments.insert(k, crate::production::Assignment::crew(1));
                    }
                }
                sys.sync_mounts();
            }
        }
        if let Some(inviter) = invited_by {
            self.join_inviters_syndicate(id, inviter);
        }
        // Seed an accurate initial valuation (before the first close).
        self.recompute_valuations();
    }

    /// §gun-platform: found a fallen corporation again where it stood (its
    /// home slot), on the account's dealt home world (`profile`, as on first
    /// join). Everything it held is gone; its name, syndicate and medals stay.
    pub(super) fn refound_corporation(&mut self, id: PlayerId, profile: crate::settlement::FoundingProfile,
        events: &mut Vec<Event>)
    {
        let Some(corp) = self.players.get(&id).filter(|c| c.founding.fallen(self.time)) else {
            return;
        };
        let Some(home_system) = corp.home_system else {
            return;
        };
        let Some(home) = self.home_slots.iter().find(|h| h.system == Some(home_system)).map(|h| h.pos) else {
            return;
        };
        let (name, syndicate, syndicate_prev, syndicate_since, medals) =
            (corp.name.clone(), corp.syndicate, corp.syndicate_prev, corp.syndicate_since, corp.medals.clone());
        self.purge_corporation(id, home_system, events);
        self.players.remove(&id);
        self.admit_corporation(id, &name, home, home_system, None, events);
        self.establish_founding_camp(id, profile);
        if let Some(corp) = self.players.get_mut(&id) {
            corp.syndicate = syndicate;
            corp.syndicate_prev = syndicate_prev;
            corp.syndicate_since = syndicate_since;
            corp.medals = medals;
        }
    }

    /// Everything `id` held, gone: its fights ended where they stood, its
    /// fleets and the pirates and traders its charter called up, its
    /// colonies wiped and let go, its orders, freight, contracts, relations
    /// and finds; its home, its den and their belts as a new founder finds
    /// them.
    fn purge_corporation(&mut self, id: PlayerId, home_system: EntityId, events: &mut Vec<Event>) {
        let fights: Vec<EntityId> = self.engagements.iter()
            .filter(|(_, e)| e.a_owner == id || e.d_owner == id)
            .map(|(eid, _)| *eid)
            .collect();
        for eid in fights {
            self.end_battle(eid, false, events);
        }
        let founding = self.players[&id].founding.clone();
        let den = self.founding_den_system(id);
        let mut gone: std::collections::BTreeSet<EntityId> = self.fleets.iter()
            .filter(|(_, f)| f.owner == id)
            .map(|(fid, _)| *fid)
            .collect();
        gone.extend(founding.privateer);
        if let Some(v) = &founding.vendetta {
            gone.extend(v.fleet);
            gone.extend(v.strikes.iter().filter_map(|s| s.fleet));
        }
        if let Some(enclave) = den.and_then(|d| self.enclaves.remove(&d)) {
            gone.extend(enclave.pack);
        }
        gone.extend(self.merchants.iter().filter(|(_, v)| v.customer == id).map(|(ship, _)| *ship));
        self.merchants.retain(|_, v| v.customer != id);
        gone.extend(self.expeditions.values().filter(|x| x.target_corp == id).map(|x| x.fleet));
        self.expeditions.retain(|_, x| x.target_corp != id);
        self.next_expedition_at.remove(&id);
        for fid in &gone {
            self.fleets.remove(fid);
        }
        self.field_repair_credit.retain(|fid, _| !gone.contains(fid));
        self.scout_contacts.retain(|fid, _| !gone.contains(fid));
        for seen in self.scout_contacts.values_mut() {
            seen.retain(|fid, _| !gone.contains(fid));
        }
        self.interdictions.retain(|_, f| f.owner != id);
        self.emplacements.retain(|e| e.owner != id);
        self.migrant_runs.retain(|_, r| r.owner != id);
        self.fuel_rescues.retain(|_, r| r.owner != id);
        self.freight_queue.retain(|_, x| x.owner != id);
        for run in self.freight_runs.values_mut() {
            run.shipments.retain(|_, x| x.owner != id);
            let left = &run.shipments;
            run.manifest.retain(|x| left.contains_key(x));
        }
        self.pirate_home_raids.retain(|fid, r| r.owner != id && !gone.contains(fid));
        self.pirate_home_raid_after.remove(&id);
        self.assaults.retain(|_, a| a.attacker != id && a.defender != id);
        self.build_queue.retain(|j| j.owner != id);
        self.refit_queue.retain(|j| j.owner != id);
        self.book.retain(|o| o.player != id);
        self.pending_survey_reports.retain(|r| r.recipient != id && r.origin != id);
        self.pending_pocket_reports.retain(|r| r.recipient != id);
        self.exploration.pending.retain(|r| r.recipient != id);
        self.exploration.journal.remove(&id);
        for citation in &mut self.pending_citations {
            citation.culprits.remove(&id);
        }
        self.pending_citations.retain(|c| !c.culprits.is_empty());
        // Its contracts end (their staged actors leave with them) and it
        // drops out of the others.
        self.operations.retain(|_, o| !matches!(o.scope, crate::operation::OperationScope::Private { player } if player == id));
        for o in self.operations.values_mut() {
            o.participants.remove(&id);
            o.assigned_fleets.remove(&id);
            o.contributions.remove(&id);
            o.known.remove(&id);
        }
        self.pending_operation_reports.retain(|r| r.recipient != id);
        self.diplomacy.remove(&id);
        for relations in self.diplomacy.values_mut() {
            relations.remove(&id);
        }
        self.treaty_proposals.retain(|_, t| t.from != id && t.to != id);
        self.pending_diplomacy_notices.retain(|n| n.recipient != id && n.other != id);
        self.season.wanted_at.remove(&id);
        self.season.squeeze.remove(&id);
        self.season.rebels.remove(&id);
        self.season.stoppers.remove(&id);
        self.home_claims.remove(&id);
        // Its worlds: wiped, and let go but for the home it is founded on
        // again. Its pocket finds are forgotten; the home's and the den's
        // belts hide their pockets whole again.
        for sys in &mut self.systems {
            for b in &mut sys.bodies {
                for pocket in &mut b.pockets {
                    pocket.found_by.remove(&id);
                }
            }
            let mine = sys.owner == Some(id);
            if !(mine || sys.id == home_system || Some(sys.id) == den) {
                continue;
            }
            if mine {
                wipe_system(sys);
                if sys.id != home_system {
                    sys.owner = None;
                    sys.claimed_at = None;
                }
            }
            let pocket = if sys.id == home_system { crate::belts::home_pocket() } else { crate::founding::den_pocket() };
            if sys.id == home_system || Some(sys.id) == den {
                for b in sys.bodies.iter_mut().filter(|b| b.is_belt() && b.belt.is_some()) {
                    b.pockets = vec![crate::body::Pocket { deposit: pocket.clone(), found_by: Default::default() }];
                }
            }
        }
    }
}

/// A world as no one has built on it: no structures, people, stores, work,
/// station or blockade. Its geology stays as it is.
fn wipe_system(sys: &mut crate::galaxy::StarSystem) {
    for b in &mut sys.bodies {
        b.structures.clear();
        b.assignments.clear();
        b.population = 0.0;
        b.founding_camp = None;
    }
    sys.stockpile.clear();
    sys.industry = Default::default();
    sys.defense_pool = 0.0;
    sys.food_state = Default::default();
    sys.blockade = None;
    sys.blockade_prev = None;
    sys.mounts.clear();
    sys.garrison_fed = false;
    sys.garrison_suppression = 0.0;
    sys.veil_up = false;
    sys.veil_never = false;
    sys.specialists.clear();
    sys.landing_holds = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind as K;
    use crate::founding::FoundingStage as S;
    use crate::settlement::FoundingProfile as P;

    /// §gun-platform (Peter: "game over and they can restart"): the restart
    /// is refused until the colony falls, then founds it afresh on its home,
    /// keeping its name and seat and nothing it held.
    #[test]
    fn a_fallen_colony_restarts_on_its_home_with_its_name_and_nothing_else() {
        let owner = PlayerId(92_001);
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        w.step(&[Command::FoundColony { id: owner, name: "Phoenix".into(), profile: P::CoastalPlains }]);
        let home = w.players[&owner].home_system.unwrap();
        let den = w.founding_den_system(owner).expect("a den system");
        let fresh: BTreeMap<_, _> = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        // A charter's worth of holdings: a second world built on, a warship,
        // credits, a colony elsewhere and the den's pocket half mined.
        let ore_world = {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            let b = sys.bodies.iter_mut().find(|b| b.tier(K::CommandCenter) == 0 && !b.is_belt()).unwrap();
            b.set_tier(K::MiningComplex, 1);
            b.set_tier(K::PowerPlant, 1);
            b.id
        };
        let other = w.systems.iter().find(|s| s.owner.is_none() && s.id != den).unwrap().id;
        w.systems.iter_mut().find(|s| s.id == other).unwrap().owner = Some(owner);
        if let Some(p) = w.systems.iter_mut().find(|s| s.id == den).unwrap().bodies.iter_mut().find(|b| b.is_belt())
            .and_then(|b| b.pockets.first_mut())
        {
            p.found_by.insert(owner);
            p.deposit.reserves = Some(250.0);
        }
        let hull = w.alloc_entity_id();
        let at = w.players[&owner].home;
        w.fleets.insert(hull, Fleet::single(hull, owner, ShipKind::Raider, at, FleetOrder::Idle, None));
        w.players.get_mut(&owner).unwrap().credits = 9_999.0;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Vendetta, w.time);
        let restart = Command::RestartCorporation { player_id: owner, profile: P::CoastalPlains };
        w.step(&[restart.clone()]);
        assert_eq!(w.players[&owner].credits, 9_999.0, "refused before the fall");
        // The fall.
        w.swear_vendetta(owner, &mut Vec::new());
        let now = w.time;
        if let Some(v) = w.players.get_mut(&owner).unwrap().founding.vendetta.as_mut() {
            v.won = Some(false);
            v.fallen = true;
            v.report_at = Some(now);
        }
        assert!(w.players[&owner].founding.fallen(w.time));
        w.step(&[restart]);
        let c = &w.players[&owner];
        assert_eq!((c.name.as_str(), c.home_system), ("Phoenix", Some(home)), "its name, on its home");
        assert_eq!(c.founding.stage, S::BuildHabitat, "the charter from its first step");
        assert!(c.founding.vendetta.is_none() && !c.founding.fallen(w.time));
        assert_eq!(c.credits, 2_000.0);
        assert_eq!(w.home_slots.iter().filter(|h| h.owner == Some(owner)).count(), 1, "the same seat");
        assert!(!w.fleets.values().any(|f| f.owner == owner), "no hull of its old life");
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.stockpile, fresh, "the founding kit again");
        let built: Vec<_> = sys.bodies.iter().flat_map(|b| b.structures.iter().filter(|(_, t)| **t > 0).map(|(k, _)| *k)).collect();
        assert_eq!(built, vec![K::CommandCenter], "only the landed colony ship stands");
        assert_eq!(sys.bodies.iter().find(|b| b.id == ore_world).unwrap().tier(K::MiningComplex), 0);
        assert!(w.systems.iter().find(|s| s.id == other).unwrap().owner.is_none(), "its colony let go");
        let pocket = w.systems.iter().find(|s| s.id == den).unwrap().bodies.iter().find(|b| b.is_belt())
            .and_then(|b| b.pockets.first()).cloned().unwrap();
        assert!(pocket.found_by.is_empty() && pocket.deposit.reserves == Some(crate::founding::DEN_POCKET_RESERVES),
            "the den's pocket hidden and whole");
        assert!(!w.enclaves.contains_key(&den), "the den rises again with the next privateer");
    }
}
