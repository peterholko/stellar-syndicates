//! The local opening uses real hulls, finite cargo and ordinary report light.
//! Never run its bootstrap during save repair or a reconnect.
use super::*;
use crate::{StructureKind as K, founding::FoundingStage as S};

impl World {
    pub(super) fn available_service_berth(
        &self,
        owner: PlayerId,
        system: EntityId,
    ) -> Option<crate::build::ServiceAssignment> {
        let sys = self
            .systems
            .iter()
            .find(|s| s.id == system && s.owner == Some(owner))?;
        let body = sys.bodies.iter().find(|b| {
            b.tier(K::Warehouse) > 0
                && b.tier(K::ServiceBerth) > 0
                && !self.refit_queue.iter().any(|j| {
                    j.system == system && j.service.as_ref().is_some_and(|a| a.body == b.id)
                })
        })?;
        let borrowed = if sys.staffing_factor(body.id, K::ServiceBerth) > 0.0 {
            None
        } else {
            let miner = sys.bodies.iter().find_map(|b| {
                b.assignments
                    .get(&K::MiningComplex)
                    .filter(|a| a.workers > 0)
                    .map(|a| (b.id, a.clone()))
            })?;
            Some(miner)
        };
        Some(crate::build::ServiceAssignment {
            body: body.id,
            borrowed,
        })
    }

    pub(super) fn restore_service_crew(&mut self, job: &crate::build::RefitJob) {
        let Some(berth) = &job.service else {
            return;
        };
        let Some((body, original)) = &berth.borrowed else {
            return;
        };
        let Some(sys) = self
            .systems
            .iter_mut()
            .find(|s| s.id == job.system && s.owner == Some(job.owner))
        else {
            return;
        };
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == berth.body)
            && b.assignments.get(&K::ServiceBerth) == Some(&crate::production::Assignment::crew(1))
        {
            b.assignments.remove(&K::ServiceBerth);
        } else {
            return;
        } // A later manual posting supersedes the loan.
        let mut loan = original.clone();
        loan.workers -= 1;
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == *body)
            && b.assignments
                .get(&K::MiningComplex)
                .is_some_and(|a| a.workers == loan.workers && a.specialists == loan.specialists)
        {
            b.assignments.insert(K::MiningComplex, original.clone());
        }
    }

    /// Whether an owned combatant is escorting `fleet`: on a Guard order, or on
    /// a defensive sortie that will resume guarding it.
    pub(super) fn founding_escorted(&self, owner: PlayerId, fleet: EntityId) -> bool {
        self.fleets.values().any(|g| {
            g.owner == owner
                && g.is_combatant()
                && (matches!(g.order, FleetOrder::Guard { target } if target == fleet)
                    || g.defense.as_ref().is_some_and(|d| d.guard == Some(fleet)))
        })
    }

    /// §charter-v5 (Peter, Oct 16 2026): "Export your ore", after the den. The
    /// first Ferrite Ore to leave for the Market Hub, in an owned Freighter
    /// past the home's gravity well or booked on Authority freight, stamped
    /// with its light home. Noted whenever it happens in the charter, so an
    /// early export completes the step as it is reached.
    pub(super) fn note_founding_export(&mut self, owner: PlayerId, events: &[Event]) {
        let corp = &self.players[&owner];
        if corp.founding.export_dispatched_at.is_some() || !corp.founding.local_opening {
            return;
        }
        let (cc, home, hub) = (corp.command_center, corp.home, self.hub);
        let booked = events.iter().find_map(|e| match &e.payload {
            EventPayload::Trade(TradeEvent::FreightBooked { player, commodity: crate::Commodity::MetallicOre,
                direction: ShipmentDir::Inbound, .. }) if *player == owner => Some(e.time),
            _ => None,
        });
        let sailed = self.fleets.values()
            .filter(|f| f.owner == owner && f.has_freighter() && f.cargo_amount(crate::Commodity::MetallicOre) > 0
                && matches!(f.order, FleetOrder::MoveTo { dest } if dest.distance(hub) < crate::ship::DOCK_RADIUS)
                && f.pos.distance(home) > crate::transit::HYPERLIMIT)
            .map(|f| self.time + crate::transit::delay(f.pos, cc, self.config.c))
            .reduce(f64::min);
        if let Some(at) = booked.or(sailed) {
            self.players.get_mut(&owner).unwrap().founding.export_dispatched_at = Some(at);
        }
    }

    /// §charter-v4: a founder lands with only its Command Center and builds
    /// everything else, ships included. The survey step charts the local stop
    /// that keeps a belt with ore (`seed_exploration_stars`), so the barge
    /// lesson always has a nearby target.
    pub(super) fn start_local_opening(
        &mut self,
        owner: PlayerId,
        home: Vec2,
        _events: &mut Vec<Event>,
    ) {
        // §charter-v4: the den system's rich belt, else the local stop.
        let lesson_stop = self.founding_den_system(owner).or_else(|| self.founding_lesson_stop(home));
        let corp = self.players.get_mut(&owner).unwrap();
        corp.founding.local_opening = true;
        corp.founding.survey_candidates = match lesson_stop {
            Some(stop) => vec![stop],
            // A home without local stops keeps one authored prospect, the
            // mineral one when it has two.
            None => corp.founding.survey_candidates.last().copied().into_iter().collect(),
        };
    }

    /// §charter-v5: the charter's first ten steps — Landfall (the home's
    /// first buildings), Raid (the privateer's raid, the trader, the Shipyard)
    /// and the Interceptor that destroys the privateer. Returns false once the
    /// programme is past them.
    pub(super) fn advance_local_opening(
        &mut self,
        owner: PlayerId,
        events: &[Event],
        out: &mut Vec<Event>,
    ) -> bool {
        let founding = self.players[&owner].founding.clone();
        if founding.stage.step() > S::DefeatPrivateer.step() {
            return false;
        }
        let home_id = self.players[&owner].home_system;
        let cc = self.players[&owner].command_center;
        if founding.privateer_report_at.is_none()
            && let Some(pirate) = founding.privateer
        {
            if let Some((at, pos, hunters)) = events.iter().find_map(|e| match &e.payload {
                EventPayload::RaidResolved {
                    attacker,
                    defender,
                    attacker_ship,
                    target_ship,
                    outcome,
                    pos,
                    aftermath,
                    ..
                } => {
                    let (a, d) = outcome.kills();
                    // Our fleets in the fight: the one named, and every
                    // survivor on our side.
                    let side = usize::from(*attacker != owner);
                    let mut hunters = vec![if side == 0 { *attacker_ship } else { *target_ship }];
                    hunters.extend(aftermath.iter().flat_map(|sides| sides[side].survivors.iter().map(|s| s.fleet_id)));
                    ((*attacker == owner || *defender == owner)
                        && ((a && *attacker_ship == pirate) || (d && *target_ship == pirate)))
                        .then_some((e.time, *pos, hunters))
                }
                _ => None,
            }) {
                self.players
                    .get_mut(&owner)
                    .unwrap()
                    .founding
                    .privateer_report_at = Some(at + crate::transit::delay(pos, cc, self.config.c));
                for fleet in hunters {
                    self.promote_privateer_hunter(owner, fleet);
                }
                self.reveal_founding_den(owner, at, pos);
                // §vendetta: his last words name his uncle.
                if !owner.is_bot() {
                    out.push(Event::new(at, EventPayload::Vendetta {
                        owner, beat: crate::founding::VendettaBeat::Kill, pos, stolen: BTreeMap::new(), wreck: None }));
                }
            }
        }
        let arrived = self.players[&owner]
            .founding
            .privateer_report_at
            .is_some_and(|at| at <= self.time);
        if arrived && !founding.reward_granted {
            let title = self.pirate_family(owner)
                .map_or_else(|| "Rogue Privateer destroyed · bounty".to_string(), |p| format!("{} destroyed · bounty", p.nephew_name()));
            let corp = self.players.get_mut(&owner).unwrap();
            corp.credits += crate::founding::PRIVATEER_CREDIT_BOUNTY;
            corp.founding.reward_granted = true;
            // §founding R6: the bounty is a reward moment, not a silent credit.
            out.push(Event::new(
                self.time,
                EventPayload::CharterStipend {
                    owner,
                    title,
                    credits: crate::founding::PRIVATEER_CREDIT_BOUNTY,
                    standing: 0.0,
                    dossier: None,
                },
            ));
        }
        self.advance_founding_raid(owner, out);
        // The home's own facts: it is the command's system, so its truth is
        // its report. The mine keeps reading its site report, as before.
        let mine = home_id
            .and_then(|id| self.information.site(id, cc, self.config.c, self.time))
            .filter(|r| r.system.owner == Some(owner))
            .is_some_and(|r| {
                let s = &r.system;
                s.bodies.iter().any(|b| {
                    b.tier(K::MiningComplex) > 0 && s.staffing_factor(b.id, K::MiningComplex) > 0.0
                })
            });
        let home_has = |kind: K, staffed: bool| {
            home_id.and_then(|id| self.systems.iter().find(|s| s.id == id)).is_some_and(|s| {
                s.bodies.iter().any(|b| {
                    b.tier(kind) >= 1
                        && (!staffed || b.assignments.get(&kind).is_some_and(|a| {
                            a.workers > 0 || a.specialists.values().any(|posted| *posted > 0)
                        }))
                })
            })
        };
        // The trader has left without a sale: nothing to wait for.
        let trader_gone = founding.merchant_called && !self.merchants.values().any(|v| v.customer == owner);
        let f = &self.players[&owner].founding;
        let next = match f.stage {
            S::BuildBioharvester => home_has(K::Bioharvester, true).then_some(S::BuildHabitat),
            S::BuildHabitat => home_has(K::Habitat, false).then_some(S::BuildAgroplex),
            S::BuildAgroplex => home_has(K::Agroplex, true).then_some(S::BuildMine),
            S::BuildMine => mine.then_some(S::PirateRaid),
            // The report of what the raid took reaching command.
            S::PirateRaid => f.raid_report_at.is_some_and(|at| at <= self.time).then_some(S::BuildGunPlatform),
            // §gun-platform: a station over the home, at any level.
            S::BuildGunPlatform => home_id.and_then(|id| self.systems.iter().find(|s| s.id == id))
                .is_some_and(|s| s.station_level() >= 1).then_some(S::SellOre),
            S::SellOre => (f.ore_sold > 0 || trader_gone).then_some(S::PowerShipyard),
            S::PowerShipyard => home_has(K::PowerPlant, false).then_some(S::BuildShipyard),
            S::BuildShipyard => home_has(K::Shipyard, true).then_some(S::BuildInterceptor),
            // Peter (Oct 30 2026): its captain isn't optional. Done once a
            // captain commands it, or when none is free to.
            S::BuildInterceptor => {
                let free = self.players[&owner].captains.values().any(|c| c.available_in_reserve() && !c.missing());
                self.fleets.values().any(|g| g.owner == owner && g.contains(ShipKind::Raider)
                    && (!free || self.captain_assigned_to(owner, g.id).is_some()))
                    .then_some(S::DefeatPrivateer)
            }
            // The privateer's log names its den; homes without one skip it.
            S::DefeatPrivateer => f.reward_granted
                .then_some(if f.den.is_some() { S::EliminateDen } else { S::PrepareExport }),
            // Retired opening steps hand on to their successors.
            S::BuySemiconductors => Some(S::PowerShipyard),
            S::BuildFreighter | S::BuildConvoy => Some(S::BuildInterceptor),
            S::CompleteExport | S::GrowBusiness | S::BuildSecondFreighter => Some(S::BuildShipyard),
            _ => None,
        };
        if let Some(next) = next {
            self.players
                .get_mut(&owner)
                .unwrap()
                .founding
                .set_stage(next, self.time);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Commodity as C;
    use crate::module::ModuleKind as M;
    fn scene() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        let owner = PlayerId(88101);
        w.step(&[Command::AddPlayer {
            id: owner,
            name: "Local opening".into(),
        }]);
        w.enclaves.clear();
        let home = w.players[&owner].home_system.unwrap();
        (w, owner, home)
    }

    /// Dev only (`DEV_START_SHIPS`): a founder joining with the switch on gets
    /// the hull at home, fed and fuelled, and can fly it at once.
    #[test]
    fn the_dev_switch_gives_a_new_founder_a_flyable_interceptor() {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        w.dev_start_ships = vec![ShipKind::Raider];
        let owner = PlayerId(88102);
        w.step(&[Command::AddPlayer { id: owner, name: "Dev start".into() }]);
        let home = w.players[&owner].home;
        let (&id, fleet) = w.fleets.iter().find(|(_, f)| f.owner == owner && f.contains(ShipKind::Raider))
            .expect("an Interceptor at home");
        assert!(fleet.at_world.is_some_and(|a| Some(a.system) == w.players[&owner].home_system), "berthed at a home world");
        assert!(fleet.pos.distance(home) < crate::transit::HYPERLIMIT);
        assert!(w.players[&owner].captains.values().all(|c| c.assigned_fleet.is_none()), "no captain: the charter assigns one");
        let dest = home + Vec2::new(4_000.0, 0.0);
        w.step(&[Command::MoveShip { player_id: owner, ship_id: id, dest }]);
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        assert!(w.fleets[&id].supplied, "fed from home");
        assert!(w.fleets[&id].pos.distance(home) > 1_000.0, "and under way: {:?}", w.fleets[&id].pos);
        // Off by default.
        let (plain, other, _) = scene();
        assert!(!plain.fleets.values().any(|f| f.owner == other), "no hulls without the switch");
    }

    /// The service-berth mechanics outlived the starting Scout; build one.
    fn add_scout(w: &mut World, owner: PlayerId) -> EntityId {
        let id = w.alloc_entity_id();
        let home = w.players[&owner].home;
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Scout, home, FleetOrder::Idle, None));
        id
    }

    fn add_ship(w: &mut World, owner: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None));
        id
    }

    /// The landing world: the body holding the Command Center.
    fn landing(w: &World, home: EntityId) -> u32 {
        w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.tier(K::CommandCenter) > 0).unwrap().id
    }

    /// §charter-v4: no Warehouse lands; the berth tests build one at home.
    fn warehouse_at_landing(w: &mut World, home: EntityId) {
        let id = landing(w, home);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.bodies.iter_mut().find(|b| b.id == id).unwrap().structures.insert(K::Warehouse, 1);
    }

    /// Build `kind` on a home body, staffed by one crew.
    fn place(w: &mut World, home: EntityId, body: u32, kind: K) {
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let b = s.bodies.iter_mut().find(|b| b.id == body).unwrap();
        b.structures.insert(kind, 1);
        b.assignments.insert(kind, crate::production::Assignment::crew(1));
    }

    fn stage(w: &World, owner: PlayerId) -> S {
        w.players[&owner].founding.stage
    }

    fn stock(w: &World, home: EntityId, c: C) -> f64 {
        w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }

    /// §first-session: while the charter runs, level I of each opening
    /// building on the landing world rises in ten seconds. A second level, or
    /// a finished charter, keeps the world's normal time.
    #[test]
    fn the_opening_buildings_rise_in_ten_seconds() {
        let required = |w: &World, kind: K| w.build_queue.iter().rev()
            .find(|job| job.what == crate::build::BuildKind::Upgrade { upgrade: kind })
            .and_then(|job| job.structure_work.as_ref()).map(|work| work.required).unwrap();
        let order = |w: &mut World, owner: PlayerId, home: EntityId, land: u32, kind: K| {
            w.receive_admin_for_test(&[Command::DevelopSystem { player_id: owner, system_id: home, upgrade: kind, body_id: Some(land) }]);
        };
        let stocked = |w: &mut World, _owner: PlayerId, home: EntityId| {
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            for c in [C::Alloys, C::Machinery, C::Polymers, C::Electronics, C::Provisions] { s.stockpile.insert(c, 5_000.0); }
        };
        let ten = (crate::founding::OPENING_BUILD_SECS / crate::config::DT).round();
        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        stocked(&mut w, owner, home);
        // §charter-gates: step 7 has opened all five. (Step 4's mine takes the
        // privateer's flight instead: `founding_raid`.)
        w.players.get_mut(&owner).unwrap().founding.stage = S::PowerShipyard;
        for kind in crate::founding::OPENING_BUILDS {
            order(&mut w, owner, home, land, kind);
            assert_eq!(required(&w, kind), ten, "{kind:?} I takes ten seconds");
        }
        order(&mut w, owner, home, land, K::Habitat);
        assert!(required(&w, K::Habitat) > ten, "level II keeps its own time");

        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        stocked(&mut w, owner, home);
        w.players.get_mut(&owner).unwrap().founding.stage = S::Complete;
        order(&mut w, owner, home, land, K::Bioharvester);
        assert!(required(&w, K::Bioharvester) > ten, "after the charter, the normal time");
    }

    #[test]
    fn a_founder_lands_with_only_its_command_center_and_the_kit() {
        let (mut w, owner, home) = scene();
        let f = w.players[&owner].founding.clone();
        assert!(f.local_opening);
        assert_eq!(f.stage, S::BuildBioharvester);
        assert!(!w.fleets.values().any(|g| g.owner == owner), "§charter-v4: no starting ships");
        let captain = w.players[&owner].captains.values().next().unwrap();
        assert_eq!((captain.assigned_fleet, captain.stationed_system), (None, Some(home)),
            "the founding captain waits at home for the first hull");
        let land = landing(&w, home);
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        for b in &s.bodies {
            let kinds: Vec<_> = b.structures.keys().copied().collect();
            assert_eq!(kinds, if b.id == land { vec![K::CommandCenter] } else { vec![] },
                "only the landed colony ship stands: {}", b.name);
        }
        assert_eq!(s.storage_cap(), crate::build::LANDING_HOLDS, "its holds are the storage");
        assert!(s.bodies.iter().find(|b| b.id == land).unwrap().habitable);
        // §charter-crates: the holds carry the Landfall crate, and the
        // Authority releases the rest a chapter at a time.
        for (c, n) in crate::founding::CharterCrate::Landfall.stock() {
            // The settlers have eaten a tick's Provisions.
            assert!((stock(&w, home, c) - n).abs() < 0.1, "{c:?}");
        }
        assert_eq!(stock(&w, home, crate::fuel::FISSION_FUEL), crate::fuel::URANIUM_HOME_SEED, "only the home's seed");
        assert!(crate::founding::charter_supplies()[&crate::fuel::FISSION_FUEL] + crate::fuel::URANIUM_HOME_SEED >= 52.5 + 1.4,
            "the first Freighter and the probe fill up");
        // No free power anywhere: only the Command Center's reactor.
        w.step(&[]);
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        for b in s.bodies.iter().filter(|b| !b.is_belt()) {
            assert_eq!(b.grid.supply, if b.id == land { crate::power::COMMAND_CENTER_MW } else { 0.0 }, "{}", b.name);
        }
        // The survey step still charts the local stop that keeps a belt with ore.
        assert_eq!(f.survey_candidates.len(), 1);
        let stop = w.systems.iter().find(|s| s.id == f.survey_candidates[0]).unwrap();
        assert!(stop.bodies.iter().any(|b| b.is_belt() && !b.deposits.is_empty()));
        let mut loaded: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        loaded.fixup_after_load();
        loaded.step(&[Command::AddPlayer { id: owner, name: "Reconnect".into() }]);
        assert!(!loaded.fleets.values().any(|g| g.owner == owner), "reconnecting grants nothing");
    }

    #[test]
    fn the_landing_steps_follow_the_homes_own_buildings() {
        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        place(&mut w, home, land, K::Bioharvester);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildHabitat);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.bodies.iter_mut().find(|b| b.id == land).unwrap().structures.insert(K::Habitat, 1);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildAgroplex);
        place(&mut w, home, land, K::Agroplex);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildMine);
        // The landing world holds its own basic ore beside its Biomass.
        place(&mut w, home, land, K::MiningComplex);
        for _ in 0..50 {
            if stage(&w, owner) != S::BuildMine {
                break;
            }
            w.step(&[]);
        }
        assert_eq!(stage(&w, owner), S::PirateRaid, "the mine report hands on to the raid");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == land).unwrap().grid.power, 1.0, "the reactor runs the mine");
    }

    #[test]
    fn after_the_raid_an_ore_sale_the_solar_array_and_the_shipyard_lead_to_the_interceptor() {
        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PirateRaid, w.time);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PirateRaid, "the raid's report hasn't come in");
        let now = w.time;
        w.players.get_mut(&owner).unwrap().founding.raid_report_at = Some(now);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildGunPlatform, "the report hands on to the home's defense");
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildGunPlatform, "no station yet");
        place(&mut w, home, land, K::OrbitalDefense);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::SellOre, "the Gun Platform hands on to the ore sale");
        w.step(&[]);
        assert_eq!(stage(&w, owner), S::SellOre, "the trader is in orbit, waiting for ore");
        w.players.get_mut(&owner).unwrap().founding.ore_sold = 12;
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PowerShipyard);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.bodies.iter_mut().find(|b| b.id == land).unwrap().structures.insert(K::PowerPlant, 1);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildShipyard);
        let before = w.players[&owner].credits;
        place(&mut w, home, land, K::Shipyard);
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildInterceptor);
        assert!(notices.iter().any(|e| matches!(&e.payload,
            EventPayload::CharterStipend { title, .. } if title == "Shipyard staffed")));
        assert_eq!(w.players[&owner].credits, before + 400.0);
        let at_home = w.players[&owner].home;
        let interceptor = add_ship(&mut w, owner, ShipKind::Raider, at_home);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildInterceptor, "Peter: the captain is not optional");
        let captain = *w.players[&owner].captains.keys().next().unwrap();
        w.apply_now(&Command::AssignCaptain { player_id: owner, captain_id: captain, fleet_id: interceptor }, &mut Vec::new());
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::DefeatPrivateer, "an Interceptor launched under its captain");
    }

    #[test]
    fn with_no_captain_free_the_interceptor_alone_completes_its_step() {
        let (mut w, owner, _) = scene();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildInterceptor, w.time);
        let home = w.players[&owner].home;
        add_ship(&mut w, owner, ShipKind::Raider, home);
        let other = add_ship(&mut w, owner, ShipKind::Raider, home + Vec2::new(5_000.0, 0.0));
        for c in w.players.get_mut(&owner).unwrap().captains.values_mut() { c.assigned_fleet = Some(other); c.stationed_system = None; }
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::DefeatPrivateer, "never a dead end");
    }

    #[test]
    fn a_trader_gone_without_a_sale_leaves_no_dead_end() {
        let (mut w, owner, _) = scene();
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.set_stage(S::SellOre, 0.0);
        f.merchant_called = true;
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PowerShipyard);
    }

    #[test]
    fn the_land_grant_keeps_building_on_the_landing_world_until_mining_expands() {
        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        let ore = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id != land && b.deposits.iter().any(|d| d.resource == C::MetallicOre))
            .expect("the system's ore body keeps its deposit").id;
        let build = |w: &mut World| {
            let mut events = Vec::new();
            w.apply_build(owner, home, Some(ore), crate::build::BuildKind::Upgrade { upgrade: K::MiningComplex },
                None, Default::default(), &mut events);
            events
        };
        let events = build(&mut w);
        assert!(w.build_queue.is_empty());
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { owner: o, reason, .. }
            if *o == owner && reason.starts_with("Charter land grant"))), "{events:?}");
        assert!(!w.players[&owner].founding.land_grant_open());
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::ExpandMining, w.time);
        assert!(w.players[&owner].founding.land_grant_open());
        build(&mut w);
        assert_eq!(w.build_queue.len(), 1, "the expansion opens the rest of the home system");
        // The step itself: a staffed mine there, on its own plant.
        place(&mut w, home, ore, K::MiningComplex);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::ExpandMining, "a mine without power is not an expansion");
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.bodies.iter_mut().find(|b| b.id == ore).unwrap().structures.insert(K::PowerPlant, 1);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::SurveyCandidates);
    }

    #[test]
    fn the_privateers_kill_pays_its_bounty_promotes_the_captain_and_names_its_den() {
        let (mut w, owner, _) = scene();
        let home = w.players[&owner].home;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PirateRaid, w.time);
        w.step(&[]);
        let privateer = w.players[&owner].founding.privateer.expect("the raid set out");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildInterceptor, w.time);
        let guard = add_ship(&mut w, owner, ShipKind::Raider, home);
        let captain = {
            let c = w.players.get_mut(&owner).unwrap().captains.values_mut().next().unwrap();
            c.assigned_fleet = Some(guard);
            c.stationed_system = None;
            c.id
        };
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::DefeatPrivateer, "an Interceptor launched");
        let before = w.players[&owner].credits;
        let at = w.time;
        let pos = w.fleets[&privateer].pos;
        let kill = Event::new(at, EventPayload::RaidResolved {
            battle_id: None, aftermath: None,
            attacker: owner, defender: PlayerId::PIRATE, attacker_ship: guard, target_ship: privateer,
            attacker_kind: ShipKind::Raider, target_kind: ShipKind::Raider,
            outcome: crate::event::RaidOutcome::TargetDestroyed, pos,
            attacker_losses: Default::default(), target_losses: Default::default(),
        });
        w.advance_founding_programs(&[kill]);
        let report = w.players[&owner].founding.privateer_report_at.unwrap();
        assert!(report > at, "the fight's report travels home");
        assert_eq!(w.players[&owner].captains[&captain].level, 2, "the kill promotes the Interceptor's captain");
        // The privateer's log names its den, arriving with the kill report.
        let den = w.players[&owner].founding.den.expect("the den rose with the privateer");
        let snap = w.players[&owner].intel[&den];
        assert_eq!((snap.enclave_tier, snap.observed_at, snap.pos), (1, at, pos));
        w.time = report;
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].credits >= before + crate::founding::PRIVATEER_CREDIT_BOUNTY);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::EliminateDen, "then its den");
        assert_eq!(w.players[&owner].founding.survey_candidates, vec![den], "whose belt is the survey's");
        // Its fall reaches command: the den's light, not the kill's.
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let cleared = Event::new(w.time, EventPayload::PirateEnclaveCleared {
            owner, system: den, pos: den_pos, plunder: Default::default(), personal: false });
        w.advance_founding_programs(&[cleared]);
        assert_eq!(stage(&w, owner), S::EliminateDen);
        w.time = w.players[&owner].founding.den_report_at.unwrap();
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PrepareExport, "then the first ore to the Market Hub");
        assert!(notices.iter().any(|e| matches!(&e.payload,
            EventPayload::CharterStipend { title, .. } if title == "Privateer den eliminated")));
    }

    /// §charter-v5: "Export your ore", after the den: the first Ferrite Ore to
    /// leave for the Market Hub, by Freighter past the gravity well or booked
    /// on Authority freight; an early export completes it as it is reached.
    #[test]
    fn the_first_ore_export_after_the_den_hands_on_to_research() {
        let (mut w, owner, _) = scene();
        let at_home = w.players[&owner].home;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PrepareExport, "nothing has left yet");
        let freight = add_ship(&mut w, owner, ShipKind::TinyFreighter, at_home);
        let f = w.fleets.get_mut(&freight).unwrap();
        f.add_cargo(C::MetallicOre, 20);
        f.order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.export_dispatched_at.is_none(), "still inside the gravity well");
        let out = (w.hub - at_home).normalized();
        w.fleets.get_mut(&freight).unwrap().pos = at_home + out * 1_500.0;
        w.advance_founding_programs(&[]);
        let at = w.players[&owner].founding.export_dispatched_at.expect("the export sailed");
        assert!(at > w.time, "on its light home");
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PrepareExport, "not before its report");
        w.time = at;
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::FirstResearch);

        // Authority freight counts too, booked before the step.
        let (mut w, owner, home) = scene();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildInterceptor, w.time);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::MetallicOre, 100.0);
        let mut events = Vec::new();
        w.apply_now(&Command::BookFreightIn { player_id: owner, system: home, commodity: C::MetallicOre,
            units: 50, sell_on_arrival: true }, &mut events);
        w.advance_founding_programs(&events);
        assert!(w.players[&owner].founding.export_dispatched_at.is_some(), "booked on Authority freight: {events:?}");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::FirstResearch, "an early export completes the step as it is reached");
    }

    #[test]
    fn charter_stipends_pay_once_when_a_milestone_is_first_reached() {
        let (mut w, owner, _home) = scene();
        let before = w.players[&owner].credits;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PirateRaid, w.time);
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 300.0);
        assert!(notices.iter().any(|e| matches!(&e.payload,
            EventPayload::CharterStipend { owner: o, credits, title, .. } if *o == owner && *credits == 300.0 && title == "Mine online")));
        w.time += 1.0;
        assert!(w.advance_founding_programs(&[]).iter().all(|e| !matches!(e.payload, EventPayload::CharterStipend { .. })));
        assert_eq!(w.players[&owner].credits, before + 300.0, "paid once");
        // A milestone stamped in the past (an old save) never pays retroactively.
        w.players.get_mut(&owner).unwrap().founding.milestones.insert(S::DefeatPrivateer, 0.0);
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 300.0);
        // Before the Interceptor, the stipends leave the founder short of
        // buying back what the raid took (the Solar Array's, the Shipyard's
        // and the Interceptor's 35 semiconductors and its 15 Armaments): the
        // ore sale makes up the rest, inside the trader's cap.
        let early: f64 = [S::BuildMine, S::PirateRaid, S::BuildInterceptor]
            .into_iter().filter_map(crate::founding::charter_stipend).map(|(_, c)| c).sum();
        let price = |c: C| crate::market::base_price(c) * (1.0 + crate::merchant::FOUNDING_MARKUP);
        let short = 35.0 * price(C::Electronics) + crate::founding::KIT_ARMAMENTS * price(C::Armaments)
            - (before + early);
        let ore = short / (crate::market::base_price(C::MetallicOre) * crate::merchant::ORE_PRICE_SHARE);
        assert!(short > 0.0, "the ore sale is needed");
        assert!(ore <= crate::merchant::ORE_CAP as f64 * 0.8, "and fits the trader's cap: {ore:.0} ore");
    }

    /// §vendetta (Oct 29 2026): the kit and the trader's stock supply every
    /// charter build, the second Interceptor the uncle's fight needs too, and
    /// the privateer's bounty pays for the last call's stock-up.
    #[test]
    fn the_kit_and_the_trader_supply_every_charter_build_and_the_bounty_pays_the_last_call() {
        use crate::build::{recipe_for, BuildKind as B};
        let need = |builds: &[B]| {
            let mut goods: BTreeMap<C, f64> = BTreeMap::new();
            for &what in builds {
                for &(c, n) in recipe_for(what).costs {
                    *goods.entry(c).or_default() += n;
                }
            }
            *goods.entry(C::Electronics).or_default() += crate::merchant::RESEARCH_RESERVE;
            goods
        };
        let up = |k: K| B::Upgrade { upgrade: k };
        let ship = |s: ShipKind| B::Ship { ship: s };
        // After the den: the Academy, the Smelter, the expansion's Solar Array
        // and mine, the probe, the barge and the second Interceptor (the
        // client's `AHEAD`).
        let ahead = [up(K::Academy), up(K::Smelter), up(K::PowerPlant), up(K::MiningComplex), ship(ShipKind::SurveyProbe),
            ship(ShipKind::MiningBarge), ship(ShipKind::Raider), ship(ShipKind::Raider)];
        let before = [up(K::Bioharvester), up(K::Habitat), up(K::Agroplex), up(K::MiningComplex), up(K::OrbitalDefense),
            up(K::PowerPlant), up(K::Shipyard), ship(ShipKind::Raider), ship(ShipKind::TinyFreighter)];
        let kit = crate::founding::charter_supplies();
        let trader: BTreeMap<C, u32> = crate::merchant::FOUNDING_STOCK.into_iter().collect();
        for (c, n) in need(&[&before[..], &ahead[..]].concat()) {
            // The raid takes the kit's semiconductors and Armaments; the trader sells them back.
            let kept = if crate::founding::RAID_TAKES.contains(&c) { 0.0 } else { kit.get(&c).copied().unwrap_or(0.0) };
            let sold = trader.get(&c).copied().unwrap_or(0) as f64;
            assert!(kept + sold >= n, "{c:?}: the charter needs {n}, the kit keeps {kept} and the trader sells {sold}");
        }
        assert_eq!(kit[&C::WarpCore], 5.0, "the Tiny Freighter, three Interceptors and the barge");
        // The last call buys what the raid took for the builds ahead, and the
        // Interceptor's tank (30 Deuterium) besides.
        let price = |c: C| crate::market::base_price(c) * (1.0 + crate::merchant::FOUNDING_MARKUP);
        let ahead = need(&ahead);
        let stock_up: f64 = crate::founding::RAID_TAKES.iter().map(|c| ahead.get(c).copied().unwrap_or(0.0) * price(*c)).sum::<f64>()
            + 30.0 * price(C::Deuterium);
        let paid = crate::founding::PRIVATEER_CREDIT_BOUNTY + crate::founding::charter_stipend(S::DefeatPrivateer).unwrap().1;
        assert!(paid >= stock_up + 1_000.0, "the bounty buys the stock-up with room to spare: {paid} vs {stock_up:.0}");
    }

    #[test]
    fn opening_service_berth_uses_no_slot_and_restores_borrowed_mining_crew() {
        let (mut w, owner, home) = scene();
        let scout = add_scout(&mut w, owner);
        warehouse_at_landing(&mut w, home);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let b = s
            .bodies
            .iter_mut()
            .find(|b| b.tier(K::Warehouse) > 0)
            .unwrap();
        let slots = b.pool_slots_built(K::Warehouse.slot_pool());
        b.structures.insert(K::ServiceBerth, 1);
        assert_eq!(slots, b.pool_slots_built(K::Warehouse.slot_pool()));
        b.structures.insert(K::MiningComplex, 1);
        b.assignments
            .insert(K::MiningComplex, crate::production::Assignment::crew(1));
        s.modules.insert(M::ReconSuite, 1);
        let food: Vec<_> = s
            .bodies
            .iter()
            .map(|b| {
                (
                    b.id,
                    b.assignments.get(&K::Agroplex).cloned(),
                    b.assignments.get(&K::Bioharvester).cloned(),
                )
            })
            .collect();
        let before = w.fleets[&scout].clone();
        w.apply_refit(
            owner,
            scout,
            ShipKind::Scout,
            Default::default(),
            crate::module::Loadout::new(vec![M::ReconSuite]),
            1,
            &mut vec![],
        );
        assert_eq!(w.refit_queue.len(), 1);
        assert!(
            w.refit_queue[0]
                .service
                .as_ref()
                .unwrap()
                .borrowed
                .is_some()
        );
        for _ in 0..300 {
            w.tick += 1;
            w.resolve_refits(&mut vec![]);
        }
        assert!(w.refit_queue.is_empty());
        assert!(w.fleets[&scout].has_recon_on(ShipKind::Scout));
        assert_eq!(w.fleets[&scout].fuel, before.fuel);
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(s.bodies.iter().any(|b| {
            b.assignments
                .get(&K::MiningComplex)
                .is_some_and(|a| a.workers == 1)
        }));
        assert!(
            s.bodies
                .iter()
                .all(|b| !b.assignments.contains_key(&K::ServiceBerth))
        );
        assert_eq!(
            food,
            s.bodies
                .iter()
                .map(|b| (
                    b.id,
                    b.assignments.get(&K::Agroplex).cloned(),
                    b.assignments.get(&K::Bioharvester).cloned()
                ))
                .collect::<Vec<_>>()
        );
        assert_eq!(crate::build::max_buildable_tier(K::ServiceBerth, 6), 1);
    }

    #[test]
    fn opening_service_berth_requires_warehouse_and_cannot_manufacture_ships() {
        let (mut w, owner, home) = scene();
        // The landing world, inside the land grant, before any Warehouse.
        let body = landing(&w, home);
        w.apply_build(
            owner,
            home,
            Some(body),
            crate::build::BuildKind::Upgrade {
                upgrade: K::ServiceBerth,
            },
            None,
            Default::default(),
            &mut vec![],
        );
        assert!(w.build_queue.is_empty());
        warehouse_at_landing(&mut w, home);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let b = s
            .bodies
            .iter_mut()
            .find(|b| b.tier(K::Warehouse) > 0)
            .unwrap();
        b.structures.insert(K::ServiceBerth, 1);
        b.assignments
            .insert(K::ServiceBerth, crate::production::Assignment::crew(1));
        for c in C::ALL {
            s.stockpile.insert(c, 100.0);
        }
        w.apply_build(
            owner,
            home,
            None,
            crate::build::BuildKind::Ship {
                ship: ShipKind::TinyFreighter,
            },
            None,
            Default::default(),
            &mut vec![],
        );
        assert!(w.build_queue.is_empty());
    }

    #[test]
    fn opening_refining_policy_waits_for_light_and_only_uses_surplus() {
        let (mut w, owner, home) = scene();
        let cc = w.players[&owner].command_center;
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.pos = cc + Vec2::new(20_000.0, 0.0);
        for b in &mut s.bodies {
            b.assignments.clear();
        }
        let b = &mut s.bodies[0];
        // §charter-v4: no free power; the Smelter's world needs a plant.
        b.structures.insert(K::PowerPlant, 1);
        b.structures.insert(K::Smelter, 1);
        b.assignments
            .insert(K::Smelter, crate::production::Assignment::crew(1));
        s.stockpile = [
            (C::MetallicOre, 52.0),
            (C::Deuterium, 22.0),
            (C::Provisions, 100.0),
        ]
        .into();
        w.apply(
            &Command::SetRefiningReserve {
                player_id: owner,
                system_id: home,
                ore: 50,
                fuel: 20,
            },
            &mut vec![],
        );
        assert!(
            w.systems
                .iter()
                .find(|s| s.id == home)
                .unwrap()
                .industry
                .refining_reserve
                .is_empty()
        );
        w.time = w
            .pending_administration
            .iter()
            .map(|p| p.0)
            .fold(w.time, f64::max);
        w.step(&[]);
        for _ in 0..600 {
            w.accrue_production(&mut vec![]);
        }
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(s.stockpile[&C::Alloys] > 0.0);
        assert!(
            s.stockpile[&C::MetallicOre] >= 50.0 - 1e-9 && s.stockpile[&C::Deuterium] >= 20.0 - 1e-9
        );
        assert!(
            s.free_stock(C::MetallicOre) >= 50.0 - 1e-9,
            "a Smelter floor must not block freight exports"
        );
        w.apply_now(
            &Command::SetRefiningReserve {
                player_id: PlayerId(999),
                system_id: home,
                ore: 0,
                fuel: 0,
            },
            &mut vec![],
        );
        assert_eq!(
            w.systems
                .iter()
                .find(|s| s.id == home)
                .unwrap()
                .industry
                .refining_reserve[&C::Deuterium],
            20
        );
    }

    #[test]
    fn opening_service_cancellation_returns_crew_and_crate_without_free_fitting() {
        let (mut w, owner, home) = scene();
        let scout = add_scout(&mut w, owner);
        warehouse_at_landing(&mut w, home);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let body = s
            .bodies
            .iter_mut()
            .find(|b| b.tier(K::Warehouse) > 0)
            .unwrap();
        body.structures.insert(K::ServiceBerth, 1);
        body.structures.insert(K::MiningComplex, 1);
        body.assignments
            .insert(K::MiningComplex, crate::production::Assignment::crew(1));
        s.modules.insert(M::ReconSuite, 1);
        w.apply_refit(
            owner,
            scout,
            ShipKind::Scout,
            Default::default(),
            crate::module::Loadout::new(vec![M::ReconSuite]),
            1,
            &mut vec![],
        );
        assert_eq!(w.refit_queue.len(), 1);
        let mut w: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        w.fleets.get_mut(&scout).unwrap().pos = w.hub;
        w.resolve_refits(&mut vec![]);
        assert!(w.refit_queue.is_empty());
        assert!(!w.fleets[&scout].has_recon_on(ShipKind::Scout));
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(s.modules[&M::ReconSuite], 1);
        assert_eq!(
            s.bodies
                .iter()
                .filter_map(|b| b.assignments.get(&K::MiningComplex))
                .map(|a| a.workers)
                .sum::<u32>(),
            1
        );
        assert!(
            s.bodies
                .iter()
                .all(|b| !b.assignments.contains_key(&K::ServiceBerth))
        );
        assert!(!crate::build::service_berth_hull(ShipKind::Cruiser));
        assert!(!crate::build::service_berth_hull(ShipKind::Convoy));
    }

}
