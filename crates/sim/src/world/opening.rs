//! The local opening uses real hulls, finite cargo and ordinary report light.
//! Never run its bootstrap during save repair or a reconnect.
use super::*;
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

    /// §founding R3: the Authority insures the starter hull ONCE. A founder
    /// whose Freighter is destroyed before the first export departs, leaving
    /// none (and none under construction), gets a replacement at home. The
    /// scripted Privateer is withdrawn so the lesson can replay on the next
    /// guarded trip.
    pub(super) fn insure_founding_freighter(
        &mut self,
        owner: PlayerId,
        events: &[Event],
        out: &mut Vec<Event>,
    ) {
        let lost = events.iter().any(|e| {
            matches!(e.payload, EventPayload::ShipDestroyed { owner: o, kind, .. }
                if o == owner && kind.is_player_freighter())
        });
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        let f = &corp.founding;
        if !lost
            || !f.enabled
            || !f.local_opening
            || f.freighter_replaced
            || f.export_dispatched_at.is_some()
            || !matches!(f.stage, S::BuildMine | S::LocalRecovery | S::PrepareExport)
            || self.fleets.values().any(|g| g.owner == owner && g.has_freighter())
            || self.build_queue.iter().any(|j| {
                j.owner == owner
                    && matches!(j.what, crate::build::BuildKind::Ship { ship } if ship.is_player_freighter())
            })
        {
            return;
        }
        let Some(system) = corp.home_system else {
            return;
        };
        let home = corp.home;
        let privateer = f.privateer.filter(|_| f.privateer_report_at.is_none());
        let id = self.alloc_entity_id();
        self.fleets.insert(
            id,
            Fleet::single(id, owner, ShipKind::TinyFreighter, home, FleetOrder::Idle, None),
        );
        if let Some(p) = privateer
            && self.fleets.get(&p).is_some_and(|g| g.founding_privateer)
            && !self
                .engagements
                .values()
                .any(|e| e.attackers.contains(&p) || e.defenders.contains(&p))
        {
            self.fleets.remove(&p);
        }
        let founding = &mut self.players.get_mut(&owner).unwrap().founding;
        founding.freighter_replaced = true;
        founding.convoy = Some(id);
        if privateer.is_some() {
            founding.privateer = None;
        }
        out.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id,
                owner,
                kind: ShipKind::TinyFreighter,
            },
        ));
        out.push(Event::new(
            self.time,
            EventPayload::FounderFreighterReplaced {
                owner,
                fleet: id,
                system,
            },
        ));
    }

    /// §charter v3: a founder starts with a Tiny Freighter and an Interceptor;
    /// no Scout and no recovery beacon. The survey step charts the local stop
    /// that keeps a belt with ore (`seed_exploration_stars`), so the barge
    /// lesson always has a nearby target.
    pub(super) fn start_local_opening(
        &mut self,
        owner: PlayerId,
        home: Vec2,
        _events: &mut Vec<Event>,
    ) {
        let lesson_stop = self.founding_lesson_stop(home);
        let corp = self.players.get_mut(&owner).unwrap();
        corp.founding.local_opening = true;
        corp.founding.survey_candidates = match lesson_stop {
            Some(stop) => vec![stop],
            // A home without local stops keeps one authored prospect, the
            // mineral one when it has two.
            None => corp.founding.survey_candidates.last().copied().into_iter().collect(),
        };
    }

    /// The Landfall chapter: bring the mine online, send the first escorted
    /// shipment, and win the fight with the Rogue Privateer it draws. Returns
    /// false once the programme is past Landfall.
    pub(super) fn advance_local_opening(
        &mut self,
        owner: PlayerId,
        events: &[Event],
        out: &mut Vec<Event>,
    ) -> bool {
        let founding = self.players[&owner].founding.clone();
        if !matches!(
            founding.stage,
            S::BuildMine | S::LocalRecovery | S::PrepareExport | S::DefeatPrivateer
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
            // §founding R6: the bounty is a reward moment, not a silent credit.
            out.push(Event::new(
                self.time,
                EventPayload::CharterStipend {
                    owner,
                    title: "Rogue Privateer destroyed · bounty".into(),
                    credits: crate::founding::PRIVATEER_CREDIT_BOUNTY,
                },
            ));
        }
        // §founding R3: only a GUARDED departure summons the lesson, so the
        // scripted Privateer never hunts a lone civilian hull: the first
        // escorted Freighter to leave its gravity well, normally the first
        // shipment. Short docking moves inside the well never do.
        if founding.privateer.is_none() && !founding.reward_granted {
            let convoy = self
                .fleets
                .values()
                .filter(|f| f.owner == owner && f.has_freighter() && self.founding_escorted(owner, f.id))
                .filter_map(|f| match f.order {
                    FleetOrder::MoveTo { dest } if f.pos.distance(dest) > crate::transit::HYPERLIMIT => {
                        Some((f.id, f.pos, dest))
                    }
                    _ => None,
                })
                .min_by_key(|(id, _, _)| *id);
            if let Some((id, pos, dest)) = convoy {
                let pirate = self.spawn_founding_privateer(id, pos, dest);
                // Inside ordinary sensors so the approach is visible, and close
                // enough that the escort meets it before the Freighter arrives.
                let direction = (dest - pos).normalized();
                let ahead = (pos.distance(dest) * 0.6)
                    .clamp(4_000.0, crate::founding::LOCAL_PRIVATEER_AHEAD_SU);
                let p = self.fleets.get_mut(&pirate).unwrap();
                p.pos = pos
                    + direction * ahead
                    + Vec2::new(-direction.y, direction.x) * crate::founding::LOCAL_PRIVATEER_OFFSET_SU;
                let f = &mut self.players.get_mut(&owner).unwrap().founding;
                f.privateer = Some(pirate);
                f.convoy = Some(id);
            }
        }
        self.insure_founding_freighter(owner, events, out);
        let mine = home_id
            .and_then(|id| self.information.site(id, cc, self.config.c, self.time))
            .filter(|r| r.system.owner == Some(owner))
            .is_some_and(|r| {
                let s = &r.system;
                s.bodies.iter().any(|b| {
                    b.tier(K::MiningComplex) > 0 && s.staffing_factor(b.id, K::MiningComplex) > 0.0
                })
            });
        // The retired beacon step hands straight on to the shipment.
        if (founding.stage == S::BuildMine && mine) || founding.stage == S::LocalRecovery {
            self.players
                .get_mut(&owner)
                .unwrap()
                .founding
                .set_stage(S::PrepareExport, self.time);
        }
        // The shipment counts on its departure's light, not a later sale; the
        // executed sale stays in the transaction ledger. An early escorted
        // dispatch is remembered too.
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
        let f = &self.players[&owner].founding;
        let next = match f.stage {
            S::PrepareExport if f.export_dispatched_at.is_some_and(|at| at <= self.time) => {
                Some(S::DefeatPrivateer)
            }
            S::DefeatPrivateer if f.reward_granted => Some(S::BuildShipyard),
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

    /// The service-berth mechanics outlived the starting Scout; build one.
    fn add_scout(w: &mut World, owner: PlayerId) -> EntityId {
        let id = w.alloc_entity_id();
        let home = w.players[&owner].home;
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Scout, home, FleetOrder::Idle, None));
        id
    }

    #[test]
    fn a_founder_starts_with_a_freighter_and_an_escort_and_reconnecting_grants_nothing() {
        let (w, owner, _) = scene();
        let f = w.players[&owner].founding.clone();
        assert!(f.local_opening);
        let own: Vec<_> = w.fleets.values().filter(|g| g.owner == owner).collect();
        assert_eq!(own.len(), 2, "a Tiny Freighter and an Interceptor");
        assert!(own.iter().any(|g| g.contains(ShipKind::TinyFreighter)));
        assert!(own.iter().any(|g| g.contains(ShipKind::Raider)));
        assert!(!own.iter().any(|g| g.contains(ShipKind::Scout)), "§charter v3: no starting Scout");
        assert!(f.recovery_site.is_none() && f.scout.is_none(), "and no recovery beacon");
        // The survey step charts the local stop that keeps a belt with ore.
        assert_eq!(f.survey_candidates.len(), 1);
        let stop = w.systems.iter().find(|s| s.id == f.survey_candidates[0]).unwrap();
        assert!(w.exploration_systems.contains(&stop.id));
        assert!(stop.bodies.iter().any(|b| b.is_belt() && !b.deposits.is_empty()));
        let mut loaded: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
        loaded.fixup_after_load();
        loaded.step(&[Command::AddPlayer {
            id: owner,
            name: "Reconnect".into(),
        }]);
        assert_eq!(loaded.fleets.values().filter(|g| g.owner == owner).count(), 2);
        assert_eq!(loaded.players[&owner].founding.survey_candidates, f.survey_candidates);
    }

    #[test]
    fn only_an_escorted_departure_draws_the_privateer() {
        let (mut w, owner, _) = scene();
        let freight = w.players[&owner].founding.convoy.unwrap();
        w.fleets.get_mut(&freight).unwrap().order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        assert!(
            w.players[&owner].founding.privateer.is_none(),
            "an unescorted Freighter is never hunted by the scripted Privateer"
        );
        let guard = w.players[&owner].founding.interceptor
            .or_else(|| w.fleets.values().find(|f| f.owner == owner && f.is_combatant()).map(|f| f.id))
            .unwrap();
        w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: freight };
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].founding.privateer.is_some());
        let assigned = w.players[&owner].founding.privateer;
        let p = &w.fleets[&assigned.unwrap()];
        let from = w.fleets[&freight].pos;
        assert!((4_000.0..12_000.0).contains(&p.pos.distance(from)), "visible, but not on top of the Freighter");
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.privateer, assigned);
    }

    #[test]
    fn a_lost_starter_freighter_is_replaced_once_and_the_lesson_resets() {
        let (mut w, owner, home) = scene();
        let freight = w.players[&owner].founding.convoy.unwrap();
        let guard = w.fleets.values().find(|f| f.owner == owner && f.is_combatant()).unwrap().id;
        let pos = w.fleets[&freight].pos;
        w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: freight };
        w.fleets.get_mut(&freight).unwrap().order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        let privateer = w.players[&owner].founding.privateer.unwrap();
        let at = w.time;
        let destroyed = |ship| [Event::new(at, EventPayload::ShipDestroyed {
            ship, owner, kind: ShipKind::TinyFreighter, pos,
        })];
        let lost = destroyed(freight);
        w.fleets.remove(&freight);
        assert!(w.advance_founding_programs(&[]).is_empty(), "a hull missing without a loss is not insured");
        let notices = w.advance_founding_programs(&lost);
        let replacement = w.players[&owner].founding.convoy.unwrap();
        assert_ne!(replacement, freight);
        assert!(w.fleets[&replacement].has_freighter() && w.fleets[&replacement].owner == owner);
        assert_eq!(w.fleets[&replacement].pos, w.players[&owner].home);
        assert!(!w.fleets.contains_key(&privateer), "the Authority chases the Privateer off");
        assert!(w.players[&owner].founding.privateer.is_none(), "the lesson can replay");
        assert!(notices.iter().any(|e| matches!(e.payload,
            EventPayload::FounderFreighterReplaced { owner: o, fleet, system } if o == owner && fleet == replacement && system == home)));
        let lost_again = destroyed(replacement);
        w.fleets.remove(&replacement);
        w.advance_founding_programs(&lost_again);
        assert!(!w.fleets.values().any(|f| f.owner == owner && f.has_freighter()), "insurance pays once");
    }

    #[test]
    fn charter_stipends_pay_once_when_a_milestone_is_first_reached() {
        let (mut w, owner, _home) = scene();
        let before = w.players[&owner].credits;
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::PrepareExport, w.time);
        let notices = w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 300.0);
        assert!(notices.iter().any(|e| matches!(&e.payload,
            EventPayload::CharterStipend { owner: o, credits, .. } if *o == owner && *credits == 300.0)));
        w.time += 1.0;
        assert!(w.advance_founding_programs(&[]).iter().all(|e| !matches!(e.payload, EventPayload::CharterStipend { .. })));
        assert_eq!(w.players[&owner].credits, before + 300.0, "paid once");
        // A milestone stamped in the past (an old save) never pays retroactively.
        w.players.get_mut(&owner).unwrap().founding.milestones.insert(S::DefeatPrivateer, 0.0);
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].credits, before + 300.0);
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
        assert_eq!(w.players[&owner].founding.stage, S::DefeatPrivateer, "the shipment counts on its departure");
        assert!(w.players[&owner].founding.opening_export_reports.is_empty());
    }

    #[test]
    fn opening_service_berth_uses_no_slot_and_restores_borrowed_mining_crew() {
        let (mut w, owner, home) = scene();
        let scout = add_scout(&mut w, owner);
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
    fn landfall_advances_on_the_mine_report_the_shipment_and_the_privateer_report() {
        let (mut w, owner, home) = scene();
        assert_eq!(w.players[&owner].founding.stage, S::BuildMine);
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let body = s.bodies.iter().find(|b| b.deposits.iter().any(|d| d.resource == C::MetallicOre)).unwrap().id;
        s.bodies.iter_mut().find(|b| b.id == body).unwrap().structures.insert(K::MiningComplex, 1);
        s.bodies.iter_mut().find(|b| b.id == body).unwrap()
            .assignments.insert(K::MiningComplex, crate::production::Assignment::crew(1));
        // The step reads home's arrived report, refreshed as the world ticks.
        for _ in 0..50 {
            if w.players[&owner].founding.stage != S::BuildMine {
                break;
            }
            w.step(&[]);
        }
        assert_eq!(w.players[&owner].founding.stage, S::PrepareExport, "the mine report hands on to the shipment");

        // The first escorted shipment leaves: it draws the privateer, and its
        // departure light hands on to the fight.
        let freight = w.players[&owner].founding.convoy.unwrap();
        let guard = w.fleets.values().find(|f| f.owner == owner && f.is_combatant()).unwrap().id;
        w.fleets.get_mut(&guard).unwrap().order = FleetOrder::Guard { target: freight };
        let f = w.fleets.get_mut(&freight).unwrap();
        f.add_cargo(C::MetallicOre, 40);
        f.order = FleetOrder::MoveTo { dest: w.hub };
        w.advance_founding_programs(&[]);
        let privateer = w.players[&owner].founding.privateer.expect("the escorted shipment draws the privateer");
        w.time = w.players[&owner].founding.export_dispatched_at.unwrap();
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::DefeatPrivateer);

        // Its destruction pays the bounty once the report lands, and opens the
        // Industry chapter.
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
        assert_eq!(w.players[&owner].founding.stage, S::DefeatPrivateer);
        w.time = report;
        w.advance_founding_programs(&[]);
        assert!(w.players[&owner].credits >= before + crate::founding::PRIVATEER_CREDIT_BOUNTY);
        w.advance_founding_programs(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::BuildShipyard);
    }
}
