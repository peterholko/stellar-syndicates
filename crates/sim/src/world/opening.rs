//! The local opening uses real hulls, finite cargo and ordinary report light.
//! Never run its bootstrap during save repair or a reconnect.
use super::*;
use crate::module::ModuleKind as M;
use crate::sites::{ExpeditionTask, SiteKind};
use crate::{Commodity as C, StructureKind as K, founding::FoundingStage as S};

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

    pub(super) fn start_local_opening(
        &mut self,
        owner: PlayerId,
        home: Vec2,
        events: &mut Vec<Event>,
    ) {
        let scout = self.alloc_entity_id();
        self.fleets.insert(
            scout,
            Fleet::single(scout, owner, ShipKind::Scout, home, FleetOrder::Idle, None),
        );
        events.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id: scout,
                owner,
                kind: ShipKind::Scout,
            },
        ));
        let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "founding-recovery-v2", scout.0);
        let pos = (0..24).find_map(|i| {
            self.clear_exploration_position(
                home + Vec2::from_polar(i as f64 * TAU / 24.0, 16_000.0),
                3_000.0,
                &mut rng,
            )
        });
        let site = pos.map(|pos| {
            let id = self.create_exploration_site(pos, SiteKind::Derelict, 3);
            let cache = self.exploration.sites.get_mut(&id).unwrap();
            cache.founding_owner = Some(owner);
            cache.details.name = "Founders’ recovery beacon".into();
            cache.details.cargo = [(C::Alloys, 20), (C::Machinery, 4)].into();
            cache.details.modules = [(M::ReconSuite, 1)].into();
            cache.details.research_fraction = 0.0;
            id
        });
        // Extremely crowded legacy worlds keep their original, completable arc.
        let mut nearby: Vec<_> = self
            .systems
            .iter()
            .filter(|s| {
                self.exploration_systems.contains(&s.id) && s.pos.distance(home) <= 34_000.0
            })
            .map(|s| (s.pos.distance(home), s.id))
            .collect();
        nearby.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let corp = self.players.get_mut(&owner).unwrap();
        corp.founding.local_opening = site.is_some();
        corp.founding.recovery_site = site;
        corp.founding.scout = Some(scout);
        if nearby.len() >= 2 {
            corp.founding.survey_candidates =
                nearby.into_iter().take(2).map(|(_, id)| id).collect();
        }
        if let Some(site) = site {
            self.queue_site_report(site, owner, home);
        }
    }

    pub(super) fn advance_local_opening(&mut self, owner: PlayerId, events: &[Event]) -> bool {
        let founding = self.players[&owner].founding.clone();
        if !matches!(
            founding.stage,
            S::BuildMine | S::LocalRecovery | S::PrepareExport
        ) {
            return false;
        }
        let home_id = self.players[&owner].home_system;
        let cc = self.players[&owner].command_center;
        if founding.privateer_report_at.is_none()
            && let Some(pirate) = founding.privateer
        {
            if let Some((at, pos)) = events.iter().find_map(|e| match e.payload {
                EventPayload::RaidResolved {
                    attacker,
                    defender,
                    attacker_ship,
                    target_ship,
                    outcome,
                    pos,
                    ..
                } => {
                    let (a, d) = outcome.kills();
                    ((attacker == owner || defender == owner)
                        && ((a && attacker_ship == pirate) || (d && target_ship == pirate)))
                        .then_some((e.time, pos))
                }
                _ => None,
            }) {
                self.players
                    .get_mut(&owner)
                    .unwrap()
                    .founding
                    .privateer_report_at = Some(at + crate::transit::delay(pos, cc, self.config.c));
            }
        }
        let arrived = self.players[&owner]
            .founding
            .privateer_report_at
            .is_some_and(|at| at <= self.time);
        if arrived && !founding.reward_granted {
            let corp = self.players.get_mut(&owner).unwrap();
            corp.credits += crate::founding::PRIVATEER_CREDIT_BOUNTY;
            corp.founding.reward_granted = true;
        }
        // Only a deliberate trip to this local cache triggers the lesson. An
        // unrelated movement or the long Hub departure must never summon it.
        if founding.privateer.is_none()
            && let Some(site) = founding.recovery_site
            && let Some(cache) = self.exploration.sites.get(&site)
            && cache.known.get(&owner).is_some_and(|r| r.details.is_some())
        {
            let dest = cache.pos;
            let convoy = self
                .fleets
                .values()
                .filter(|f| f.owner == owner && f.has_freighter())
                .filter(|f| match f.order {
                    FleetOrder::Expedition {
                        site: target,
                        task: ExpeditionTask::Recover,
                        ..
                    } => target == site,
                    FleetOrder::MoveTo { dest: target } => {
                        target.distance(dest) <= crate::sites::SITE_RANGE
                    }
                    _ => false,
                })
                .min_by_key(|f| f.id)
                .map(|f| (f.id, f.pos));
            if let Some((id, pos)) = convoy {
                let pirate = self.spawn_founding_privateer(id, pos, dest);
                // Within ordinary sensors, but with real warning/approach time.
                let direction = (dest - pos).normalized();
                let p = self.fleets.get_mut(&pirate).unwrap();
                p.pos = pos + direction * 7_000.0 + Vec2::new(-direction.y, direction.x) * 4_000.0;
                let f = &mut self.players.get_mut(&owner).unwrap().founding;
                f.privateer = Some(pirate);
                f.convoy = Some(id);
            }
        }
        let mine = home_id
            .and_then(|id| self.information.site(id, cc, self.config.c, self.time))
            .filter(|r| r.system.owner == Some(owner))
            .is_some_and(|r| {
                let s = &r.system;
                s.bodies.iter().any(|b| {
                    b.tier(K::MiningComplex) > 0 && s.staffing_factor(b.id, K::MiningComplex) > 0.0
                })
            });
        if founding.stage == S::BuildMine && mine {
            self.players
                .get_mut(&owner)
                .unwrap()
                .founding
                .set_stage(S::LocalRecovery, self.time);
        }
        let fitted = self
            .fleets
            .values()
            .find(|f| f.owner == owner && f.has_recon_on(ShipKind::Scout))
            .map(|f| f.pos);
        let recovered = founding
            .recovery_report_at
            .is_some_and(|at| at <= self.time);
        if mine
            && recovered
            && arrived
            && founding.local_ready_report_at.is_none()
            && let Some(pos) = fitted
        {
            self.players
                .get_mut(&owner)
                .unwrap()
                .founding
                .local_ready_report_at =
                Some(self.time + crate::transit::delay(pos, cc, self.config.c));
        }
        if founding
            .local_ready_report_at
            .is_some_and(|at| at <= self.time)
            && matches!(
                self.players[&owner].founding.stage,
                S::BuildMine | S::LocalRecovery
            )
        {
            self.players
                .get_mut(&owner)
                .unwrap()
                .founding
                .set_stage(S::PrepareExport, self.time);
        }
        // Guide handoff follows departure's light, not a later sale. The actual
        // executed sale remains separately recorded by the transaction ledger.
        // Remember an early guarded dispatch too: optional tasks can be done
        // out of order without requiring another shipment just for the guide.
        if founding.export_dispatched_at.is_none() {
            let departure = self.fleets.values().find(|f| f.owner == owner && f.has_freighter()
                    && f.cargo_amount(C::MetallicOre) > 0
                    && self.fleets.values().any(|g| g.owner == owner && g.is_combatant()
                        && matches!(g.order, FleetOrder::Guard { target } if target == f.id))
                    && matches!(f.order, FleetOrder::MoveTo { dest } if dest.distance(self.hub) < crate::ship::DOCK_RADIUS))
                    .map(|f| f.pos);
            if let Some(pos) = departure {
                self.players
                    .get_mut(&owner)
                    .unwrap()
                    .founding
                    .export_dispatched_at =
                    Some(self.time + crate::transit::delay(pos, cc, self.config.c));
            }
        }
        if self.players[&owner].founding.stage == S::PrepareExport {
            if self.players[&owner]
                .founding
                .export_dispatched_at
                .is_some_and(|at| at <= self.time)
            {
                self.players
                    .get_mut(&owner)
                    .unwrap()
                    .founding
                    .set_stage(S::GrowBusiness, self.time);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn opening_assets_are_finite_and_reconnect_does_not_regrant_them() {
        let (mut w, owner, _) = scene();
        let f = w.players[&owner].founding.clone();
        assert!(f.local_opening);
        assert_eq!(w.fleets.values().filter(|g| g.owner == owner).count(), 3);
        assert!(w.fleets[&f.scout.unwrap()].contains(ShipKind::Scout));
        let site = f.recovery_site.unwrap();
        assert_eq!(
            w.exploration.sites[&site].details.cargo,
            [(C::Alloys, 20), (C::Machinery, 4)].into()
        );
        assert_eq!(
            w.exploration.sites[&site]
                .details
                .modules
                .get(&M::ReconSuite),
            Some(&1)
        );
        assert!(
            w.exploration.sites[&site].known.is_empty(),
            "beacon light is still travelling"
        );
        w.exploration
            .sites
            .get_mut(&site)
            .unwrap()
            .details
            .cargo
            .clear();
        let mut loaded: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        loaded.fixup_after_load();
        loaded.step(&[Command::AddPlayer {
            id: owner,
            name: "Reconnect".into(),
        }]);
        assert_eq!(loaded.players[&owner].founding.recovery_site, Some(site));
        assert!(loaded.exploration.sites[&site].details.cargo.is_empty());
        assert_eq!(
            loaded.fleets.values().filter(|g| g.owner == owner).count(),
            3
        );
        let mut legacy = serde_json::to_value(&f).unwrap();
        legacy.as_object_mut().unwrap().remove("local_opening");
        let legacy: crate::founding::FoundingProgram = serde_json::from_value(legacy).unwrap();
        assert!(!legacy.local_opening, "old saves keep the old arc");
    }

    #[test]
    fn opening_hub_departure_does_not_spawn_a_tutorial_ambush() {
        let (mut w, owner, _) = scene();
        let freight = w.players[&owner].founding.convoy.unwrap();
        w.fleets.get_mut(&freight).unwrap().order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.privateer.is_none());
        let site = w.players[&owner].founding.recovery_site.unwrap();
        let pos = w.exploration.sites[&site].pos;
        w.exploration
            .sites
            .get_mut(&site)
            .unwrap()
            .surveyed
            .insert(owner);
        w.queue_site_report(site, owner, pos);
        w.time += 100.0;
        w.deliver_site_reports();
        w.fleets.get_mut(&freight).unwrap().order = FleetOrder::Expedition {
            site,
            task: ExpeditionTask::Recover,
            station: pos,
            dwell_since: None,
        };
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.privateer.is_some());
        let assigned = w.players[&owner].founding.privateer;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.privateer, assigned);
    }

    #[test]
    fn opening_handoff_waits_for_departure_light_not_sale() {
        let (mut w, owner, _) = scene();
        let freight = w.players[&owner].founding.convoy.unwrap();
        w.players.get_mut(&owner).unwrap().founding.stage = S::PrepareExport;
        w.fleets
            .values_mut()
            .find(|f| f.owner == owner && f.is_combatant())
            .unwrap()
            .order = FleetOrder::Guard { target: freight };
        let f = w.fleets.get_mut(&freight).unwrap();
        f.pos = f.pos + Vec2::new(20_000.0, 0.0);
        f.add_cargo(C::MetallicOre, 50);
        f.order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::PrepareExport);
        let at = w.players[&owner].founding.export_dispatched_at.unwrap();
        w.time = at - 0.01;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::PrepareExport);
        w.time = at;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::GrowBusiness);
        assert!(w.players[&owner].founding.opening_export_reports.is_empty());
    }

    #[test]
    fn opening_service_berth_uses_no_slot_and_restores_borrowed_mining_crew() {
        let (mut w, owner, home) = scene();
        let scout = w.players[&owner].founding.scout.unwrap();
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
        let body = w
            .systems
            .iter()
            .find(|s| s.id == home)
            .unwrap()
            .bodies
            .iter()
            .find(|b| b.tier(K::Warehouse) == 0)
            .unwrap()
            .id;
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
    fn opening_recovery_is_physical_finite_private_and_report_delayed() {
        let (mut w, owner, home) = scene();
        let f = w.players[&owner].founding.clone();
        let site = f.recovery_site.unwrap();
        let carrier = f.convoy.unwrap();
        let pos = w.exploration.sites[&site].pos;
        w.finish_expedition(
            f.scout.unwrap(),
            owner,
            site,
            ExpeditionTask::Investigate,
            &mut vec![],
        );
        let before = w
            .systems
            .iter()
            .find(|s| s.id == home)
            .unwrap()
            .stockpile
            .clone();
        w.fleets.get_mut(&carrier).unwrap().pos = pos;
        w.finish_expedition(carrier, owner, site, ExpeditionTask::Recover, &mut vec![]);
        assert_eq!(w.fleets[&carrier].cargo_amount(C::Alloys), 20);
        assert_eq!(w.fleets[&carrier].cargo_amount(C::Machinery), 4);
        assert_eq!(w.fleets[&carrier].modules.get(&M::ReconSuite), Some(&1));
        assert!(w.exploration.sites[&site].details.cargo.is_empty());
        assert_eq!(
            w.systems.iter().find(|s| s.id == home).unwrap().stockpile,
            before
        );
        assert!(w.players[&owner].founding.recovery_report_at.unwrap() > w.time);
        w.finish_expedition(carrier, owner, site, ExpeditionTask::Recover, &mut vec![]);
        assert_eq!(w.fleets[&carrier].cargo_amount(C::Alloys), 20);
        let stranger = PlayerId(99992);
        w.step(&[Command::AddPlayer {
            id: stranger,
            name: "Other founder".into(),
        }]);
        w.queue_site_report(site, stranger, pos);
        assert!(
            !w.exploration
                .pending
                .iter()
                .any(|p| p.recipient == stranger && p.snapshot.id == site)
        );
        let other = w.players[&stranger].founding.convoy.unwrap();
        w.issue_expedition(stranger, other, site, ExpeditionTask::Recover, &mut vec![]);
        assert!(matches!(w.fleets[&other].order, FleetOrder::Idle));
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
        b.structures.insert(K::Smelter, 1);
        b.assignments
            .insert(K::Smelter, crate::production::Assignment::crew(1));
        s.stockpile = [
            (C::MetallicOre, 52.0),
            (C::Fuel, 22.0),
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
            s.stockpile[&C::MetallicOre] >= 50.0 - 1e-9 && s.stockpile[&C::Fuel] >= 20.0 - 1e-9
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
                .refining_reserve[&C::Fuel],
            20
        );
    }

    #[test]
    fn opening_service_cancellation_returns_crew_and_crate_without_free_fitting() {
        let (mut w, owner, home) = scene();
        let scout = w.players[&owner].founding.scout.unwrap();
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

    #[test]
    fn opening_ready_progress_requires_arrived_recovery_battle_and_fit() {
        let (mut w, owner, home) = scene();
        let scout = w.players[&owner].founding.scout.unwrap();
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let b = &mut s.bodies[0];
        b.structures.insert(K::MiningComplex, 1);
        b.assignments
            .insert(K::MiningComplex, crate::production::Assignment::crew(1));
        w.record_information();
        w.time += 1.0;
        let ready = w.time + 20.0;
        let founding = &mut w.players.get_mut(&owner).unwrap().founding;
        founding.recovery_report_at = Some(ready);
        founding.privateer_report_at = Some(ready);
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::LocalRecovery);
        w.time = ready;
        w.advance_founding_programs(&[]);
        assert_eq!(
            w.players[&owner].founding.stage,
            S::LocalRecovery,
            "recovery is not a fitted ship"
        );
        let pos = w.players[&owner].command_center + Vec2::new(20_000.0, 0.0);
        let fleet = w.fleets.get_mut(&scout).unwrap();
        fleet.pos = pos;
        fleet.ships[0].loadout = crate::module::Loadout::new(vec![M::ReconSuite]);
        fleet.rebuild_cache();
        w.advance_founding_programs(&[]);
        let at = w.players[&owner].founding.local_ready_report_at.unwrap();
        assert!(at > w.time);
        w.time = at - 0.01;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::LocalRecovery);
        w.time = at;
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::PrepareExport);
    }
}
