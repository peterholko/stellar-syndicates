//! A planet has one structure work slot. Hulls, modules and Academy courses
//! use their existing independent facilities and never occupy that slot.

use super::*;
use crate::build::{BuildKind, BuildWork};

impl World {
    pub(super) fn update_structure_queues(&mut self, events: &mut Vec<Event>) {
        // Lost-site structure jobs already forfeit their reserved goods. Drop
        // waiting jobs too, so a former owner's queue cannot block a new owner.
        // The change is still disclosed only through the arrived site report.
        self.build_queue.retain(|job| !matches!(job.what, BuildKind::Upgrade { .. } | BuildKind::Founding { .. })
            || self.systems.iter().any(|system| system.id == job.system
                && system.owner == Some(job.owner)
                && system.bodies.iter().any(|body| body.id == job.body_id)));

        // Job ids are receipt order, independent of deadlines or Vec layout.
        // Include system id: body ids are local, not galaxy-wide identifiers.
        let mut heads: BTreeMap<(EntityId, u32), u64> = BTreeMap::new();
        for job in &self.build_queue {
            if matches!(job.what, BuildKind::Upgrade { .. } | BuildKind::Founding { .. }) {
                heads.entry((job.system, job.body_id))
                    .and_modify(|id| *id = (*id).min(job.id)).or_insert(job.id);
            }
        }
        for job in &mut self.build_queue {
            if !matches!(job.what, BuildKind::Upgrade { .. } | BuildKind::Founding { .. }) { continue; }
            let active = heads.get(&(job.system, job.body_id)) == Some(&job.id);
            let work = job.structure_work.get_or_insert_with(|| {
                // Older snapshots allowed parallel timers. Preserve their earned
                // work, then serialize only the remainder. Unknown legacy starts
                // stay unknown in the UI; no current recipe/bonus is substituted.
                let start = job.started_tick.unwrap_or(self.tick);
                let span = job.complete_tick.saturating_sub(start).max(1) as f64;
                BuildWork {
                    required: span,
                    completed: if job.complete_tick <= self.tick { span }
                        else { (self.tick.saturating_sub(start) as f64).min(span) },
                    at_tick: self.tick, rate: 1.0,
                }
            });
            let activating = active && work.rate == 0.0;
            work.set_rate(self.tick, if active { 1.0 } else { 0.0 });
            if activating && job.started_tick.is_none() && job.queued_tick.is_some() {
                job.started_tick = Some(self.tick);
            }
            // Even a fully worked legacy follower must wait for its turn.
            job.complete_tick = if active { work.completion_tick().unwrap() } else { u64::MAX };
            if activating {
                events.push(Event::new(self.time, EventPayload::BuildStarted {
                    id: job.id, owner: job.owner, system: job.system,
                    what: job.what, complete_tick: job.complete_tick,
                }));
            }
        }
        // Work anchors change only on queue transitions. Stable active/waiting
        // jobs do not manufacture a full planetary report on every sim tick.
    }

    /// §construction: cancel a structure order that is still waiting for its
    /// world's work slot. Only the newest order for that structure on that world
    /// may go, so no later order is left paid for a tier it no longer reaches.
    /// The refund is that order's own recipe at its tier, with the same Unstable
    /// Geology premium `apply_build` charged, back into the system stockpile.
    pub(super) fn cancel_build(&mut self, owner: PlayerId, system: EntityId, job_id: u64, events: &mut Vec<Event>) {
        let refuse = |time: f64, reason: &str| Event::new(time, EventPayload::IndustryRejected {
            owner, system, reason: reason.into() });
        let Some(index) = self.build_queue.iter().position(|j| j.id == job_id && j.owner == owner && j.system == system) else {
            events.push(refuse(self.time, "that construction order has already finished or gone"));
            return;
        };
        let job = &self.build_queue[index];
        let BuildKind::Upgrade { upgrade } = job.what else {
            events.push(refuse(self.time, "only a structure waiting for its turn can be cancelled"));
            return;
        };
        if !job.is_queued_structure() {
            events.push(refuse(self.time, "construction has already started, so it can no longer be cancelled"));
            return;
        }
        let body_id = job.body_id;
        let same = |j: &crate::build::BuildJob| j.system == system && j.body_id == body_id
            && matches!(j.what, BuildKind::Upgrade { upgrade: u } if u == upgrade);
        if self.build_queue.iter().any(|j| same(j) && j.id > job_id) {
            events.push(refuse(self.time, "a later order for the same structure depends on it; cancel that one first"));
            return;
        }
        let Some(sys) = self.systems.iter().find(|s| s.id == system && s.owner == Some(owner)) else {
            events.push(refuse(self.time, "the system is no longer yours"));
            return;
        };
        let ahead = self.build_queue.iter().filter(|j| same(j) && j.id < job_id).count() as u32;
        let built = sys.bodies.iter().find(|b| b.id == body_id).map_or(0, |b| b.tier(upgrade));
        let premium = if sys.trait_ == Some(crate::explore::SystemTrait::UnstableGeology) {
            crate::explore::UNSTABLE_COST_MULT
        } else {
            1.0
        };
        let refunded: Vec<(crate::cargo::Commodity, f64)> = crate::build::recipe_for_tier(BuildKind::Upgrade { upgrade }, built + ahead + 1)
            .costs.iter().map(|(c, units)| (*c, units * premium)).collect();
        let job = self.build_queue.remove(index);
        let sys = self.systems.iter_mut().find(|s| s.id == system).unwrap();
        for (c, units) in &refunded {
            *sys.stockpile.entry(*c).or_insert(0.0) += units;
        }
        events.push(Event::new(self.time, EventPayload::BuildCancelled { owner, system, what: job.what, refunded }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{BuildJob, SlotPool, StructureKind as K};
    use crate::cargo::Commodity;
    use crate::production::Assignment;

    fn planet_queue() -> (World, PlayerId, EntityId, u32) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        w.legacy_test_bootstrap = true;
        w.enclaves.clear();
        let owner = PlayerId(5700);
        w.step(&[Command::AddPlayer { id: owner, name: "Planet builders".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.set_population(0.05);
        for body in &mut sys.bodies {
            body.structures.clear();
            body.assignments.clear();
        }
        for commodity in Commodity::ALL { sys.stockpile.insert(commodity, 10_000.0); }
        let body = sys.bodies.iter().find(|b| b.has_deposit_for(K::MiningComplex)
            && b.pool_slots(SlotPool::Industrial) > 0).unwrap().id;
        (w, owner, home, body)
    }

    fn queue(w: &mut World, owner: PlayerId, home: EntityId, body: u32, kind: K) -> u64 {
        let before = w.build_queue.len();
        let events = w.step(&[Command::DevelopSystem { player_id: owner,
            system_id: home, body_id: Some(body), upgrade: kind }]);
        assert_eq!(w.build_queue.len(), before + 1, "accepted structure: {events:?}");
        w.build_queue.last().unwrap().id
    }

    fn job(w: &World, id: u64) -> &BuildJob {
        w.build_queue.iter().find(|j| j.id == id).unwrap()
    }

    fn advance(w: &mut World, tick: u64) {
        while w.tick < tick { w.step(&[]); }
    }

    #[test]
    fn one_structure_per_planet_is_fifo_and_waiting_earns_no_work() {
        let (mut w, owner, home, body) = planet_queue();
        let yard = queue(&mut w, owner, home, body, K::Shipyard);
        let mine = queue(&mut w, owner, home, body, K::MiningComplex);
        let store = queue(&mut w, owner, home, body, K::Warehouse);
        let due = job(&w, yard).complete_tick;
        assert!(!job(&w, yard).is_queued_structure());
        assert!(job(&w, mine).is_queued_structure());
        assert!(job(&w, store).is_queued_structure());
        assert_eq!(job(&w, mine).started_tick, None);
        assert!(job(&w, store).structure_work.as_ref().unwrap().required
            < job(&w, mine).structure_work.as_ref().unwrap().required, "FIFO, not shortest-job first");
        let frozen = job(&w, mine).clone();
        let paid_alloys = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Alloys];
        assert_eq!(paid_alloys, 10_000.0 - 40.0 - 25.0 - 30.0);
        advance(&mut w, due);
        assert_eq!(job(&w, mine), &frozen, "waiting does not create work or reports each tick");
        assert_eq!(job(&w, mine).work().unwrap().completed_at(w.tick), 0.0);
        // Reconnect/save and storage order cannot reorder the receipt queue.
        w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        w.build_queue.reverse();
        w.step(&[]);
        assert!(!w.build_queue.iter().any(|j| j.id == yard));
        assert_eq!(job(&w, mine).started_tick, Some(due));
        assert_eq!(job(&w, mine).work().unwrap().completed, 0.0);
        assert_eq!(job(&w, mine).complete_tick, due + job(&w, mine).work().unwrap().required as u64);
        assert!(job(&w, store).is_queued_structure());
        let mine_due = job(&w, mine).complete_tick;
        advance(&mut w, mine_due);
        assert_eq!(job(&w, store).work().unwrap().completed_at(w.tick), 0.0);
        w.step(&[]);
        assert_eq!(job(&w, store).started_tick, Some(mine_due));
        let end = job(&w, store).complete_tick;
        advance(&mut w, end + 1);
        assert!(w.build_queue.is_empty());
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let planet = sys.bodies.iter().find(|b| b.id == body).unwrap();
        for kind in [K::Shipyard, K::MiningComplex, K::Warehouse] { assert_eq!(planet.tier(kind), 1); }
        assert_eq!(sys.stockpile[&Commodity::Alloys], paid_alloys, "activation never charges again");
    }

    fn cancel(w: &mut World, owner: PlayerId, home: EntityId, job_id: u64) -> Vec<Event> {
        w.step(&[Command::CancelBuild { player_id: owner, system_id: home, job_id }])
    }

    fn refused(events: &[Event], why: &str) -> bool {
        events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { reason, .. } if reason.contains(why)))
    }

    #[test]
    fn a_waiting_structure_cancels_with_its_goods_back_and_a_started_one_keeps_building() {
        let (mut w, owner, home, body) = planet_queue();
        let stock = |w: &World| { let s = w.systems.iter().find(|s| s.id == home).unwrap();
            (s.stockpile[&Commodity::Alloys], s.stockpile[&Commodity::Machinery]) };
        let yard = queue(&mut w, owner, home, body, K::Shipyard);
        let before = stock(&w);
        let mine = queue(&mut w, owner, home, body, K::MiningComplex);
        assert_eq!(stock(&w), (before.0 - 25.0, before.1 - 12.0));
        let events = cancel(&mut w, owner, home, mine);
        assert!(!w.build_queue.iter().any(|j| j.id == mine));
        assert_eq!(stock(&w), before, "the order's goods return to the stockpile");
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::BuildCancelled {
            what: BuildKind::Upgrade { upgrade: K::MiningComplex }, refunded, .. } if refunded.len() == 2)));
        // The active job is no longer waiting: it keeps its slot and its goods.
        let events = cancel(&mut w, owner, home, yard);
        assert!(w.build_queue.iter().any(|j| j.id == yard));
        assert!(refused(&events, "already started"));
        // Another corporation cannot cancel it; nor can a finished order come back.
        let mut events = Vec::new();
        w.cancel_build(PlayerId(1), home, yard, &mut events);
        assert!(w.build_queue.iter().any(|j| j.id == yard) && refused(&events, "finished or gone"));
        let events = cancel(&mut w, owner, home, mine);
        assert!(refused(&events, "finished or gone"));
        assert_eq!(stock(&w), before, "a repeated cancel refunds nothing");
    }

    #[test]
    fn only_the_newest_order_for_a_structure_cancels() {
        let (mut w, owner, home, body) = planet_queue();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut()
            .find(|b| b.id == body).unwrap().set_tier(K::Warehouse, 1);
        queue(&mut w, owner, home, body, K::Shipyard);
        let first = queue(&mut w, owner, home, body, K::Warehouse);
        let second = queue(&mut w, owner, home, body, K::Warehouse);
        let events = cancel(&mut w, owner, home, first);
        assert!(w.build_queue.iter().any(|j| j.id == first), "the tier-II order still builds on it");
        assert!(refused(&events, "later order"));
        cancel(&mut w, owner, home, second);
        cancel(&mut w, owner, home, first);
        assert!(!w.build_queue.iter().any(|j| j.id == first || j.id == second));
    }

    #[test]
    fn a_cancelled_order_returns_the_unstable_geology_premium_it_paid() {
        let (mut w, owner, home, body) = planet_queue();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().trait_ = Some(crate::explore::SystemTrait::UnstableGeology);
        let alloys = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Alloys];
        queue(&mut w, owner, home, body, K::Shipyard);
        let before = alloys(&w);
        let mine = queue(&mut w, owner, home, body, K::MiningComplex);
        assert!((before - alloys(&w) - 25.0 * crate::explore::UNSTABLE_COST_MULT).abs() < 1e-9);
        cancel(&mut w, owner, home, mine);
        assert!((alloys(&w) - before).abs() < 1e-9);
    }

    #[test]
    fn separate_planets_and_systems_build_in_parallel() {
        let (mut w, owner, home, body) = planet_queue();
        let other = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id != body && b.pool_slots(SlotPool::Industrial) > 0).unwrap().id;
        let a = queue(&mut w, owner, home, body, K::Shipyard);
        let b = queue(&mut w, owner, home, other, K::Shipyard);
        // A second controlled system with identical local body ids is a distinct queue.
        let mut colony = w.systems.iter().find(|s| s.id == home).unwrap().clone();
        colony.id = w.alloc_entity_id();
        let colony_id = colony.id;
        w.systems.push(colony);
        let c = queue(&mut w, owner, colony_id, body, K::Shipyard);
        for id in [a, b, c] {
            assert!(!job(&w, id).is_queued_structure());
            assert!(job(&w, id).started_tick.is_some());
            assert!(job(&w, id).complete_tick < u64::MAX);
        }
        let end = [a, b, c].into_iter().map(|id| job(&w, id).complete_tick).max().unwrap();
        advance(&mut w, end + 1);
        assert!(w.build_queue.is_empty());
    }

    #[test]
    fn structure_upgrades_serialize_but_staffed_hulls_are_independent() {
        let (mut w, owner, home, body) = planet_queue();
        let planet = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut()
            .find(|b| b.id == body).unwrap();
        planet.set_tier(K::MiningComplex, 1);
        planet.set_tier(K::Shipyard, 1);
        planet.assignments.insert(K::Shipyard, Assignment::crew(1));
        let first = queue(&mut w, owner, home, body, K::MiningComplex);
        let second = queue(&mut w, owner, home, body, K::MiningComplex);
        w.step(&[Command::BuildShip { player_id: owner, system_id: home,
            ship_kind: ShipKind::TinyFreighter, join: None, loadout: Default::default() }]);
        let hull = w.build_queue.last().unwrap().id;
        assert!(job(&w, hull).ship_work.is_some());
        assert!(job(&w, hull).complete_tick < job(&w, first).complete_tick);
        let hull_due = job(&w, hull).complete_tick;
        advance(&mut w, hull_due);
        assert!(w.step(&[]).iter().any(|e| matches!(e.payload,
            EventPayload::ShipSpawned { kind: ShipKind::TinyFreighter, .. })));
        assert!(!job(&w, first).is_queued_structure());
        assert!(job(&w, second).is_queued_structure());
        let first_due = job(&w, first).complete_tick;
        advance(&mut w, first_due + 1);
        assert_eq!(job(&w, second).started_tick, Some(first_due));
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == body).unwrap().tier(K::MiningComplex), 2);
        let end = job(&w, second).complete_tick;
        advance(&mut w, end + 1);
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter()
            .find(|b| b.id == body).unwrap().tier(K::MiningComplex), 3);
    }

    #[test]
    fn old_parallel_jobs_keep_earned_work_and_site_loss_clears_waiters() {
        let (mut w, owner, home, body) = planet_queue();
        let first = queue(&mut w, owner, home, body, K::Shipyard);
        let second = queue(&mut w, owner, home, body, K::MiningComplex);
        let now = w.tick;
        for j in &mut w.build_queue {
            j.queued_tick = None;
            j.started_tick = Some(now - 2);
            j.complete_tick = now + 98;
            j.structure_work = None;
        }
        w.update_structure_queues(&mut Vec::new());
        assert_eq!(job(&w, first).work().unwrap().completed, 2.0);
        assert_eq!(job(&w, second).work().unwrap().completed, 2.0);
        assert_eq!(job(&w, first).complete_tick, now + 98);
        assert!(job(&w, second).is_queued_structure());
        advance(&mut w, now + 98 + 1);
        assert_eq!(job(&w, second).work().unwrap().completed, 2.0);
        assert_eq!(job(&w, second).complete_tick, now + 98 + 98);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().owner = None;
        w.step(&[]);
        assert!(w.build_queue.is_empty(), "a lost planet cannot strand a waiting queue forever");
    }

    #[test]
    fn a_structure_stays_queued_in_the_picture_until_its_start_report_arrives() {
        let (mut w, owner, home, body) = planet_queue();
        let first = queue(&mut w, owner, home, body, K::Shipyard);
        let second = queue(&mut w, owner, home, body, K::MiningComplex);
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let cc = pos + Vec2::new(100_000.0, 0.0);
        let delay = crate::transit::delay(pos, cc, w.config.c);
        let due = job(&w, first).complete_tick;
        advance(&mut w, due);
        let events = w.step(&[]);
        let activation = events.iter().find_map(|e| matches!(e.payload,
            EventPayload::BuildStarted { id, .. } if id == second).then_some(e.time)).unwrap();
        // World records the site's resulting state at the end of the tick;
        // that report's emission (not the earlier event timestamp) rides home.
        let report_emission = w.time;
        assert!((report_emission - activation - DT).abs() < 1e-9);
        assert!(!job(&w, second).is_queued_structure(), "the true build has already started");
        let before = w.information.site(home, cc, w.config.c, report_emission + delay - 1e-6).unwrap();
        let waiting = before.builds.iter().find(|j| j.id == second).unwrap();
        assert!(waiting.is_queued_structure(), "actual activation is not early information");
        assert_eq!(waiting.started_tick, None);
        assert_eq!(waiting.work().unwrap().completed, 0.0);
        let after = w.information.site(home, cc, w.config.c, report_emission + delay + 1e-6).unwrap();
        let building = after.builds.iter().find(|j| j.id == second).unwrap();
        assert!(!building.is_queued_structure());
        assert_eq!(building.started_tick, Some(due));
    }
}
