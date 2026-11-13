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
    /// first Alloys to leave for the Market Hub (§charter-v6; Ferrite Ore
    /// until Nov 7 2026), in an owned Freighter past the home's gravity well
    /// or booked on Authority freight, stamped with its light home. Noted
    /// whenever it happens in the charter, so an early export completes the
    /// step as it is reached.
    pub(super) fn note_founding_export(&mut self, owner: PlayerId, events: &[Event]) {
        let corp = &self.players[&owner];
        if corp.founding.export_dispatched_at.is_some() || !corp.founding.local_opening {
            return;
        }
        let (cc, home, hub, departed) = (corp.command_center, corp.home, self.hub, corp.founding.export_departed_at);
        let booked = events.iter().find_map(|e| match &e.payload {
            EventPayload::Trade(TradeEvent::FreightBooked { player, commodity: crate::Commodity::Alloys,
                direction: ShipmentDir::Inbound, .. }) if *player == owner => Some(e.time),
            _ => None,
        });
        // A loaded freighter under way for the Hub: past the well it has
        // sailed (on its light home); inside it, it has departed.
        let bound = |f: &crate::ship::Fleet| f.owner == owner && f.has_freighter()
            && f.cargo_amount(crate::Commodity::Alloys) > 0
            && matches!(f.order, FleetOrder::MoveTo { dest } if dest.distance(hub) < crate::ship::DOCK_RADIUS);
        let sailed = self.fleets.values()
            .filter(|f| bound(f) && f.pos.distance(home) > crate::transit::HYPERLIMIT)
            .map(|f| self.time + crate::transit::delay(f.pos, cc, self.config.c))
            .reduce(f64::min);
        // §export-follow (Peter, Nov 2 2026): the departure starts the follow's
        // clock; EXPORT_FOLLOW_MAX_S later the step completes whatever the
        // ship has done since.
        let departed = match departed {
            None if self.fleets.values().any(|f| bound(f)) => {
                self.players.get_mut(&owner).unwrap().founding.export_departed_at = Some(self.time);
                Some(self.time)
            }
            other => other,
        };
        let capped = departed.map(|at| at + crate::founding::EXPORT_FOLLOW_MAX_S).filter(|at| *at <= self.time);
        if let Some(at) = [booked, sailed, capped].into_iter().flatten().reduce(f64::min) {
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
                // §privateer-contract: the kill completes the Authority's
                // contract on him; its report pays the bounty.
                self.complete_founding_privateer_contract(owner, pos, out);
                // §den-contract: the Authority's contract on the den, on the
                // same light as the kill.
                self.post_founding_den_contract(owner, pos, out);
            }
        }
        // §privateer-contract: the Authority's contract on him, offered as
        // the hunt begins (the Interceptor under its captain).
        if founding.stage == S::DefeatPrivateer && founding.privateer_report_at.is_none() {
            self.post_founding_privateer_contract(owner, true, out);
            // §den-retreat (Peter, Nov 12 2026): he went home to his den, so
            // the hunt is an assault on it: the den is known as the hunt
            // begins, and the Authority's contract on it is posted beside the
            // one on him (once).
            if let Some(den) = founding.den
                && !self.operations.values().any(|o| o.is_founder_den()
                    && matches!(o.scope, crate::operation::OperationScope::Private { player } if player == owner))
                && let Some(pos) = self.systems.iter().find(|s| s.id == den).map(|s| s.pos)
            {
                self.reveal_founding_den(owner, self.time, pos);
                self.post_founding_den_contract(owner, cc, out);
            }
        }
        // §den-retreat: the den falls in the battle that kills him, before his
        // kill's report moves the step on; its fall is noted as it happens.
        if founding.stage == S::DefeatPrivateer {
            self.note_and_tell_founding_den_cleared(owner, events, out);
        }
        let arrived = self.players[&owner]
            .founding
            .privateer_report_at
            .is_some_and(|at| at <= self.time);
        if arrived && !founding.reward_granted {
            // §founding R6: the bounty is a reward moment, not a silent
            // credit. §privateer-contract: the contract's report pays it, with
            // its own line, on this same light.
            self.players.get_mut(&owner).unwrap().founding.reward_granted = true;
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
            // §habitat-first (Nov 8 2026): a home for the settlers (no power:
            // the colony ship's reactor keeps its people alive), then light
            // for the colony, then the food chain on it.
            S::BuildHabitat => home_has(K::Habitat, false).then_some(S::PowerShipyard),
            S::PowerShipyard => home_has(K::PowerPlant, false).then_some(S::BuildBioharvester),
            S::BuildBioharvester => home_has(K::Bioharvester, true).then_some(S::BuildAgroplex),
            // §charter-v6 (Peter, Nov 7 2026): the Command Center's reactor
            // runs the colony, not the mine or the Smelter; the mine, then the
            // Smelter: ore to Alloys begins before the raid breaks the plan.
            S::BuildAgroplex => home_has(K::Agroplex, true).then_some(S::BuildMine),
            S::BuildMine => mine.then_some(S::BuildSmelter),
            S::BuildSmelter => home_has(K::Smelter, true).then_some(S::PirateRaid),
            // The report of what the raid took reaching command.
            S::PirateRaid => f.raid_report_at.is_some_and(|at| at <= self.time).then_some(S::BuildGunPlatform),
            // §gun-platform: a station over the home, at any level.
            S::BuildGunPlatform => home_id.and_then(|id| self.systems.iter().find(|s| s.id == id))
                .is_some_and(|s| s.station_level() >= 1).then_some(S::SellOre),
            // The Alloys sale, then the Shipyard for the ship that takes the
            // engines back.
            S::SellOre => (f.alloys_sold > 0 || trader_gone).then_some(S::BuildShipyard),
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
            S::BuySemiconductors => Some(S::BuildShipyard),
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

    /// §stores-flows: a tally folds into a per-second rate that settles on
    /// the true rate, then fades and leaves once the flow stops.
    #[test]
    fn flows_settle_on_the_rate_then_fade() {
        let (mut w, _, home) = scene();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.flows.clear();
        for _ in 0..(120 * crate::config::TICK_HZ) {
            sys.tally_made(C::Alloys, 0.5 * DT);
            sys.tally_used(C::Alloys, 0.2 * DT);
            sys.fold_flows(DT);
        }
        let f = sys.flows[&C::Alloys];
        assert!((f.made - 0.5).abs() < 0.01 && (f.used - 0.2).abs() < 0.01, "{f:?}");
        for _ in 0..(600 * crate::config::TICK_HZ) {
            sys.fold_flows(DT);
        }
        assert!(!sys.flows.contains_key(&C::Alloys), "gone once it fades");
        // A trickle below the grain still builds up rather than being dropped.
        for _ in 0..(120 * crate::config::TICK_HZ) {
            sys.tally_made(C::Machinery, 0.001 * DT);
            sys.fold_flows(DT);
        }
        assert!((sys.flows[&C::Machinery].made - 0.001).abs() < 1e-4);
    }

    /// §stores-flows (Peter, Nov 5 2026: the Stores panel's in, out and net):
    /// a staffed mine's ore shows as made at its real rate, the settlers'
    /// food as used, and the site report carries both at its grain.
    #[test]
    fn a_home_reports_what_its_mine_makes_and_its_settlers_eat() {
        let (mut w, owner, home) = scene();
        let land = landing(&w, home);
        // §charter-v6: the Solar Array runs the mine; the Command Center's
        // reactor runs the colony.
        place(&mut w, home, land, K::PowerPlant);
        place(&mut w, home, land, K::MiningComplex);
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let before = stock(&w, home, C::MetallicOre);
        for _ in 0..(60 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let grew = (stock(&w, home, C::MetallicOre) - before) / 60.0;
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let ore = sys.flows.get(&C::MetallicOre).copied().unwrap_or_default();
        assert!(grew > 0.0 && (ore.made - grew).abs() < 0.03 * grew.max(0.01), "made {:.4}/s vs grew {grew:.4}/s", ore.made);
        assert!(sys.flows.get(&C::Provisions).is_some_and(|f| f.used > 0.0), "the settlers eat");
        let cc = w.players[&owner].command_center;
        let report = w.information.site(home, cc, w.config.c, w.time).expect("its report");
        let reported = report.system.flows.get(&C::MetallicOre).copied().unwrap_or_default();
        assert!((reported.made - ore.made).abs() <= crate::production::FLOW_QUANTUM, "at the report's grain");
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
        // §charter-gates: step 6 has opened all five. (Step 5's mine takes the
        // privateer's flight instead: `founding_raid`.)
        w.players.get_mut(&owner).unwrap().founding.stage = S::BuildSmelter;
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
        assert_eq!(f.stage, S::BuildHabitat, "§habitat-first: the settlers' home first");
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
        // §habitat-first (Nov 8 2026): the settlers' home first (it draws no
        // power), then the Solar Array, then the food chain on its power.
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.bodies.iter_mut().find(|b| b.id == land).unwrap().structures.insert(K::Habitat, 1);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PowerShipyard);
        place(&mut w, home, land, K::PowerPlant);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildBioharvester);
        place(&mut w, home, land, K::Bioharvester);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildAgroplex);
        place(&mut w, home, land, K::Agroplex);
        w.advance_founding_programs(&[]);
        // §charter-v6 (Peter, Nov 7 2026): the Command Center's reactor runs
        // the colony, not the mine.
        assert_eq!(stage(&w, owner), S::BuildMine);
        // The landing world holds its own basic ore beside its Biomass.
        place(&mut w, home, land, K::MiningComplex);
        for _ in 0..50 {
            if stage(&w, owner) != S::BuildMine {
                break;
            }
            w.step(&[]);
        }
        assert_eq!(stage(&w, owner), S::BuildSmelter, "the mine report hands on to the Smelter");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == land).unwrap().grid.power, 1.0, "the Solar Array runs the mine");
        // §sell-alloys (Nov 11 2026): the sale is Alloys, so the Smelter
        // keeps no ore back for it.
        let reserve = &w.systems.iter().find(|s| s.id == home).unwrap().industry.refining_reserve;
        assert!(reserve.get(&C::MetallicOre).is_none_or(|n| *n == 0), "no ore floor");
        place(&mut w, home, land, K::Smelter);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PirateRaid, "the Smelter hands on to the raid");
    }

    #[test]
    fn after_the_raid_an_alloy_sale_and_the_shipyard_lead_to_the_interceptor() {
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
        assert_eq!(stage(&w, owner), S::SellOre, "the Laser Battery hands on to the sale");
        w.step(&[]);
        assert_eq!(stage(&w, owner), S::SellOre, "the trader is in orbit, waiting for Alloys");
        w.players.get_mut(&owner).unwrap().founding.alloys_sold = 3;
        let notices = w.advance_founding_programs(&[]);
        // §charter-v6 (Nov 7 2026): the Smelter and the Solar Array stood
        // before the raid; the sale hands on to the Shipyard, the ship that
        // gets the engines back. The sale pays nothing.
        assert_eq!(stage(&w, owner), S::BuildShipyard);
        assert!(!notices.iter().any(|e| matches!(&e.payload, EventPayload::CharterStipend { .. })));
        place(&mut w, home, land, K::Shipyard);
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::BuildInterceptor);
        assert!(!notices.iter().any(|e| matches!(&e.payload, EventPayload::CharterStipend { .. })), "the Shipyard pays nothing");
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
        // §charter-v6: on to the Shipyard.
        assert_eq!(stage(&w, owner), S::BuildShipyard);
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
        // §privateer-contract: the hunt begins with the Authority's contract
        // on him, offered at the colony: news at once, by his name.
        w.advance_founding_programs(&[]);
        let contract = w.founding_privateer_contract(owner).expect("the Authority's contract on him");
        let mut delivered = Vec::new();
        w.deliver_operation_reports(&mut delivered);
        {
            use crate::operation::{OperationIssuer, OperationKind, OperationState};
            let o = &w.operations[&contract];
            let lurk = w.players[&owner].founding.raid.as_ref().unwrap().lurk;
            assert_eq!((o.issuer, o.state), (OperationIssuer::Authority, OperationState::Offered), "offered, for the founder to take");
            assert!(matches!(o.kind, OperationKind::PrivateerPatrol { pos } if pos == lurk), "at his last known position");
            assert_eq!(o.known[&owner].state, OperationState::Offered, "news at once");
            let nephew = w.pirate_family(owner).unwrap().nephew_name();
            assert_eq!(o.briefing.as_ref().unwrap().title, format!("Eliminate {nephew}"));
            assert_eq!(o.reward.credits, crate::founding::PRIVATEER_CREDIT_BOUNTY);
            assert_eq!(o.expires_at, f64::MAX, "it never lapses");
        }
        w.advance_founding_programs(&[]);
        assert_eq!(w.operations.values().filter(|o| o.is_founder_privateer()).count(), 1, "one contract");
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
        // The privateer's log names its den, arriving with the kill report,
        // and §den-contract the Authority's contract on it rides along.
        let den = w.players[&owner].founding.den.expect("the den rose with the privateer");
        let snap = w.players[&owner].intel[&den];
        assert_eq!((snap.enclave_tier, snap.observed_at, snap.pos), (1, at, pos));
        assert!(w.operations.values().any(|o| o.is_founder_den()
            && matches!(o.kind, crate::operation::OperationKind::PirateBounty { system, .. } if system == den)));
        // The kill claims the contract, taken on or not; its report pays.
        assert_eq!(w.operations[&contract].state, crate::operation::OperationState::Completed);
        assert_eq!(w.operations[&contract].winner, Some(owner));
        let mut paid = Vec::new();
        w.deliver_operation_reports(&mut paid);
        assert_eq!(w.players[&owner].credits, before, "nothing before its report");
        w.time = report;
        w.deliver_operation_reports(&mut paid);
        paid.extend(w.advance_founding_programs(&[]));
        let nephew = w.pirate_family(owner).unwrap().nephew_name();
        let moments: Vec<_> = paid.iter().filter_map(|e| match &e.payload {
            EventPayload::CharterStipend { owner: o, title, credits, .. } if *o == owner => Some((title.clone(), *credits)),
            _ => None,
        }).collect();
        assert_eq!(moments, vec![(format!("{nephew} destroyed · contract"), crate::founding::PRIVATEER_CREDIT_BOUNTY)],
            "one reward moment: the contract's");
        assert!((w.players[&owner].credits - before - crate::founding::PRIVATEER_CREDIT_BOUNTY).abs() < 1e-6, "paid once");
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
        assert_eq!(stage(&w, owner), S::RepairInterceptor, "then the den's aftermath (§stash)");
        // §den-contract: the den's contract pays for it, not a stipend
        // (`founding_den` tests).
        assert!(!notices.iter().any(|e| matches!(&e.payload, EventPayload::CharterStipend { .. })));
        // No hull hurt, a freighter at hand and nothing in the den: the three
        // steps hand straight on, each stamped.
        for _ in 0..4 {
            w.advance_founding_programs(&[]);
        }
        assert_eq!(stage(&w, owner), S::PrepareExport, "then the first ore to the Market Hub");
        let f = &w.players[&owner].founding;
        assert!([S::RepairInterceptor, S::BuildTinyFreighter, S::HaulLoot].iter().all(|s| f.milestones.contains_key(s)));
    }

    /// §charter-v5: the first export, after the den: the first cargo to
    /// leave for the Market Hub, by Freighter past the gravity well or booked
    /// on Authority freight; an early export completes it as it is reached.
    /// §charter-v6 (Nov 7 2026): the cargo is the Smelter's Alloys.
    #[test]
    fn the_first_alloys_export_after_the_den_hands_on_to_research() {
        let (mut w, owner, _) = scene();
        let at_home = w.players[&owner].home;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PrepareExport, "nothing has left yet");
        let freight = add_ship(&mut w, owner, ShipKind::TinyFreighter, at_home);
        let f = w.fleets.get_mut(&freight).unwrap();
        f.add_cargo(C::Alloys, 20);
        f.order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.export_dispatched_at.is_none(), "still inside the gravity well");
        assert_eq!(w.players[&owner].founding.export_departed_at, Some(w.time), "but it has departed: the follow runs");
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
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::Alloys, 100.0);
        let mut events = Vec::new();
        w.apply_now(&Command::BookFreightIn { player_id: owner, system: home, commodity: C::Alloys,
            units: 50, sell_on_arrival: true }, &mut events);
        w.advance_founding_programs(&events);
        assert!(w.players[&owner].founding.export_dispatched_at.is_some(), "booked on Authority freight: {events:?}");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::FirstResearch, "an early export completes the step as it is reached");
    }

    /// §export-follow (Peter, Nov 2 2026): the first shipment is followed out
    /// of the well for EXPORT_FOLLOW_MAX_S at most. A freighter that leaves the
    /// dock but never clears the hyperlimit still completes the step then.
    #[test]
    fn the_export_follow_ends_at_the_hyperlimit_or_ninety_seconds_after_departure() {
        let (mut w, owner, _) = scene();
        let at_home = w.players[&owner].home;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        let freight = add_ship(&mut w, owner, ShipKind::TinyFreighter, at_home);
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.export_departed_at.is_none(), "an empty freighter at its dock has not departed");
        let f = w.fleets.get_mut(&freight).unwrap();
        f.add_cargo(C::Alloys, 20);
        f.order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        let departed = w.players[&owner].founding.export_departed_at.expect("loaded and under way for the Hub: departed");
        assert_eq!(departed, w.time);
        assert!(w.players[&owner].founding.export_dispatched_at.is_none(), "still inside the well");
        w.time = departed + crate::founding::EXPORT_FOLLOW_MAX_S - 1.0;
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::PrepareExport, "the follow still runs");
        w.time = departed + crate::founding::EXPORT_FOLLOW_MAX_S;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.export_dispatched_at, Some(w.time), "the cap completes it, no light delay");
        w.advance_founding_programs(&[]);
        assert_eq!(stage(&w, owner), S::FirstResearch);
    }

    #[test]
    fn charter_stipends_pay_once_when_a_milestone_is_first_reached() {
        let (mut w, owner, _home) = scene();
        let before = w.players[&owner].credits;
        // §charter-v6: the mine's step hands on to the Smelter's.
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildSmelter, w.time);
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 2_000.0);
        assert!(notices.iter().any(|e| matches!(&e.payload,
            EventPayload::CharterStipend { owner: o, credits, title, .. } if *o == owner && *credits == 2_000.0 && title == "Mine online")));
        w.time += 1.0;
        assert!(w.advance_founding_programs(&[]).iter().all(|e| !matches!(e.payload, EventPayload::CharterStipend { .. })));
        assert_eq!(w.players[&owner].credits, before + 2_000.0, "paid once");
        // A milestone stamped in the past (an old save) never pays retroactively.
        w.players.get_mut(&owner).unwrap().founding.milestones.insert(S::ExpandMining, 0.0);
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 2_000.0);
        // §three-stipends (Peter, Nov 6 2026): the mine, the Smelter and the
        // second mine pay, 2,000 each; nothing else does.
        let paying: Vec<_> = crate::founding::FoundingStage::STEPS.into_iter().chain([S::Complete])
            .filter_map(|s| crate::founding::charter_stipend(s).map(|(title, c)| (s, title, c))).collect();
        // §charter-v6 (Nov 7 2026): the mine's as its step hands on to the
        // Smelter, the Smelter's as its hands on to the raid.
        assert_eq!(paying, vec![(S::BuildSmelter, "Mine online", 2_000.0), (S::PirateRaid, "Smelter running", 2_000.0),
            (S::SurveyCandidates, "Second mine running", 2_000.0)]);
        // Before the Interceptor, the start and the mine's and the Smelter's
        // stipends buy back what the raid took (the Solar Array's, the
        // Shipyard's and the Interceptor's 35 semiconductors and its 15
        // Armaments) at twice the Hub, with room for prices that drift; the
        // colony smelts the Interceptor's Alloys. The Alloys sale is the
        // lesson, inside the trader's cap.
        let early: f64 = crate::founding::FoundingStage::STEPS.into_iter()
            .take_while(|s| *s != S::DefeatPrivateer).filter_map(crate::founding::charter_stipend).map(|(_, c)| c).sum();
        assert_eq!(early, 4_000.0, "the mine's and the Smelter's");
        let price = |c: C| crate::market::base_price(c) * (1.0 + crate::merchant::FOUNDING_MARKUP);
        let bill = 35.0 * price(C::Electronics) + 15.0 * price(C::Armaments);
        assert!(crate::merchant::FOUNDING_STOCK.iter().all(|(c, _)| *c != C::Alloys), "the trader sells no Alloys");
        assert!(before + early >= bill * 1.08, "the parts with prices 8% dearer: {bill:.0} of {:.0} Cr", before + early);
        assert!(before + early <= bill + 2_500.0, "and not much more: {bill:.0} of {:.0} Cr", before + early);
    }

    /// §vendetta (Oct 29 2026): the kit and the trader's stock supply every
    /// charter build, the second Interceptor the uncle's fight needs too, and
    /// the privateer's bounty pays for the trader's goods for the builds after
    /// the den. §den-stores (Nov 5 2026): with the den's stores, and
    /// §den-contract the den's contract paying for the Tiny Freighter's parts
    /// and fuel. §smelt-from-landing (Nov 6 2026): the colony smelts every
    /// Alloy from the Interceptor on; the trader sells none.
    #[test]
    fn the_kit_and_the_trader_supply_every_charter_build_and_the_bounty_and_the_den_contract_pay_for_it() {
        use crate::build::{recipe_for, BuildKind as B};
        let need = |builds: &[B]| {
            let mut goods: BTreeMap<C, f64> = BTreeMap::new();
            for &what in builds {
                for &(c, n) in recipe_for(what).costs {
                    *goods.entry(c).or_default() += n;
                }
            }
            goods
        };
        let up = |k: K| B::Upgrade { upgrade: k };
        let ship = |s: ShipKind| B::Ship { ship: s };
        // The Landfall crate's builds, then the trader's: the first
        // Interceptor and the Tiny Freighter.
        let before = [up(K::Bioharvester), up(K::Habitat), up(K::Agroplex), up(K::MiningComplex), up(K::OrbitalDefense),
            up(K::PowerPlant), up(K::Shipyard), up(K::Smelter), ship(ShipKind::Raider), ship(ShipKind::TinyFreighter)];
        // After the den: the Academy, the Small Freighter, the expansion's
        // Solar Array and mine, the probe, the barge and the uncle's two
        // Interceptors.
        let ahead = [up(K::Academy), ship(ShipKind::SmallFreighter), up(K::PowerPlant), up(K::MiningComplex),
            ship(ShipKind::SurveyProbe), ship(ShipKind::MiningBarge), ship(ShipKind::Raider), ship(ShipKind::Raider)];
        // Once the Smelter runs it makes the Alloys of what comes after it,
        // all but the barge's (the Reach crate's).
        let smelted = need(&[ship(ShipKind::Raider), ship(ShipKind::TinyFreighter), up(K::Academy),
            ship(ShipKind::SmallFreighter), up(K::PowerPlant), up(K::MiningComplex), ship(ShipKind::SurveyProbe),
            ship(ShipKind::Raider), ship(ShipKind::Raider)])[&C::Alloys];
        let kit = crate::founding::charter_supplies();
        let den: BTreeMap<C, u32> = crate::founding::DEN_STORES.into_iter().collect();
        let trader: BTreeMap<C, u32> = crate::merchant::FOUNDING_STOCK.into_iter().collect();
        assert!(!trader.contains_key(&C::Alloys) && !den.contains_key(&C::Alloys), "Alloys are smelted");
        let mut all = need(&[&before[..], &ahead[..]].concat());
        *all.entry(C::Electronics).or_default() += crate::merchant::RESEARCH_RESERVE;
        for (c, n) in all {
            // §shipbuilding-overhaul: the engines and the kit parts are their
            // own story below.
            if c.is_part() { continue; }
            // The raid takes the kit's semiconductors and Armaments; the
            // trader sells them back, and §stash the den's stash returns them.
            // §hull-ladder: the Small Freighter's plating and precision parts
            // are the trader's too (a Hub import is an hour away).
            let n = if c == C::Alloys { n - smelted } else { n };
            let kept = kit.get(&c).copied().unwrap_or(0.0) + den.get(&c).copied().unwrap_or(0) as f64;
            let sold = trader.get(&c).copied().unwrap_or(0) as f64;
            assert!(kept + sold >= n, "{c:?}: the charter needs {n}, the kit and the den keep {kept} and the trader sells {sold}");
        }
        // §reactors-drives + §hull-ladder (Nov 7 2026): the raid takes the
        // kit's six Fission Reactors and seven Zephyr Drives and the den
        // returns them; the trader sells three of each (the Interceptor's
        // featured at half, the Tiny Freighter's pair at its usual price).
        // The Small Freighter's four of each come from the kit, so its step
        // waits on no Hub import (the charter harness, Nov 6 2026); what the
        // uncle's two Interceptors still lack is the first engines a player
        // buys at the Hub.
        let price = |c: C| crate::market::base_price(c) * (1.0 + crate::merchant::FOUNDING_MARKUP);
        let parts = |what: B, goods: &[C]| recipe_for(what).costs.iter()
            .filter(|(c, _)| goods.contains(c)).map(|&(c, n)| n * price(c)).sum::<f64>();
        let needed = need(&[&before[..], &ahead[..]].concat());
        assert_eq!((kit[&C::FissionReactor], kit[&C::ZephyrDrive]), (6.0, 7.0), "the charter's engines");
        assert!(!den.keys().any(|c| c.is_engine_part()), "the den's engines are the raid's take");
        assert_eq!((trader[&C::FissionReactor], trader[&C::ZephyrDrive]), (4, 3), "the trader's four and three");
        assert_eq!((needed[&C::FissionReactor], needed[&C::ZephyrDrive]), (12.0, 12.0), "every charter hull's engines");
        // §hull-ladder: the charter's own hulls, through the barge, take no
        // Hub part at all (a Hub import is a light round trip and a freight
        // run — an hour in the harness); the uncle's two Interceptors
        // (§vendetta) are the running colony's, the first parts a player buys
        // at the Hub, for about two Interceptors' price.
        let charter = need(&[&before[..], &ahead[..ahead.len() - 2]].concat());
        let hub_bill = |goods: &BTreeMap<C, f64>| goods.iter()
            .filter(|(c, _)| c.is_part())
            .map(|(c, n)| (n - kit.get(c).copied().unwrap_or(0.0) - trader.get(c).copied().unwrap_or(0) as f64)
                .max(0.0) * crate::market::base_price(*c))
            .sum::<f64>();
        assert_eq!(hub_bill(&charter), 0.0, "the kit and the trader fit every charter hull");
        let vendetta_bill = hub_bill(&needed);
        assert!(vendetta_bill < crate::founding::PRIVATEER_CREDIT_BOUNTY,
            "the uncle's two Interceptors from the Hub cost less than the bounty did: {vendetta_bill:.0} Cr");
        // The Small Freighter's kit beyond its engines, at the trader's twice.
        let parts_bill = parts(ship(ShipKind::SmallFreighter),
            &[C::HullSections, C::CargoBay, C::NavComputer, C::Composites, C::PrecisionComponents]);
        assert!(parts_bill < 9_000.0, "the trader's bill for the Small Freighter's kit: {parts_bill:.0} Cr");
        let engines_bill = price(C::FissionReactor);
        // The trader sells what the builds after the den need of what the raid
        // took, less what §stash the den's stash brings back, as each comes
        // up (§den-contract: it stays for the charter); the Tiny Freighter's
        // parts and its fuel (a store and a tank) and the Small Freighter's
        // Polymers besides.
        let mut ahead = need(&ahead);
        *ahead.entry(C::Electronics).or_default() += crate::merchant::RESEARCH_RESERVE;
        let stock_up: f64 = crate::founding::RAID_TAKES.iter().filter(|c| !c.is_engine_part())
            .map(|c| (ahead.get(c).copied().unwrap_or(0.0) - kit.get(c).copied().unwrap_or(0.0)
                - den.get(c).copied().unwrap_or(0) as f64).max(0.0) * price(*c))
            .sum::<f64>()
            // §shipbuilding-overhaul: the Tiny Freighter's pair of reactors
            // and drives from the trader at its usual price.
            + 2.0 * (price(C::FissionReactor) + price(C::ZephyrDrive));
        // §reactors-drives: a Fission hull's tank holds Uranium alone.
        let tank = ShipKind::TinyFreighter.hull_mass() * crate::fuel::FUEL_PER_HULL_MASS;
        // The Small Freighter's Machinery is the den's.
        // §shipbuilding-overhaul: the Hub's cores and kit parts for the later
        // builds are bought from the same purse.
        let cost = stock_up + parts(ship(ShipKind::TinyFreighter), &[C::Machinery, C::Polymers])
            + tank * price(C::Uranium) + parts(ship(ShipKind::SmallFreighter), &[C::Polymers])
            + engines_bill + parts_bill;
        let stipend = |s: S| crate::founding::charter_stipend(s).map_or(0.0, |(_, c)| c);
        let paid = crate::founding::PRIVATEER_CREDIT_BOUNTY + crate::founding::DEN_CONTRACT_CREDITS
            + stipend(S::SurveyCandidates);
        assert!(crate::founding::charter_stipend(S::RepairInterceptor).is_none(), "the den pays through its contract");
        assert!(paid >= cost + 500.0, "the bounty, the den's contract and the second mine's stipend buy them with room to spare: {paid} vs {cost:.0} (engines {engines_bill:.0}, parts {parts_bill:.0}, stock-up {stock_up:.0})");
        assert!(paid <= cost + 3_500.0, "and not much more: {paid} vs {cost:.0} (engines {engines_bill:.0}, parts {parts_bill:.0}, stock-up {stock_up:.0})");
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
        s.put_modules(M::ReconSuite, 1);
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
        s.put_modules(M::ReconSuite, 1);
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
        assert_eq!(s.module_stock(M::ReconSuite), 1);
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
