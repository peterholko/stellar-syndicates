//! Earned construction work. Only rate changes create a new work segment, so
//! long staffed or paused builds do not defeat site-report change compression.

use super::*;
use crate::build::BuildWork;
#[cfg(test)]
use crate::build::BuildKind;

impl World {
    pub(super) fn update_ship_build_work(&mut self) {
        for job in &mut self.build_queue {
            let Some(yard) = job.work_yard() else { continue; };
            // The actual construction body and gating yard matter. Staffing a
            // Shipyard elsewhere cannot operate this Drydock or Capital Slipway.
            // §power: a yard works at its planet's grid.
            let rate = self.systems.iter().find(|s| s.id == job.system).map_or(0.0, |s|
                crate::production::shipyard_work_rate(s.staffing_factor(job.body_id, yard),
                    s.skill_factor(job.body_id, yard)) * s.body_power(job.body_id));
            let work = job.ship_work.get_or_insert_with(|| {
                // Old saves only knew a deadline. Preserve the earned fraction
                // and (if staffed) its remaining ETA, without guessing old site,
                // research or specialist bonuses. Missing start remains unknown
                // to the UI; its internal ledger covers only remaining work.
                let start = job.started_tick.unwrap_or(self.tick);
                let span = job.complete_tick.saturating_sub(start).max(1) as f64;
                let elapsed = self.tick.saturating_sub(start) as f64;
                let reference_rate = if rate > 0.0 { rate } else { 1.0 };
                BuildWork {
                    required: span * reference_rate,
                    completed: if job.complete_tick <= self.tick { span } else { elapsed.min(span) } * reference_rate,
                    at_tick: self.tick,
                    rate,
                }
            });
            // Changes take effect at this tick, after commands arrive. The old
            // rate earns the interval before the change, never any interval after.
            work.set_rate(self.tick, rate);
            job.complete_tick = work.completion_tick().unwrap_or(u64::MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind as K;
    use crate::production::Assignment;

    fn yard() -> (World, PlayerId, EntityId, u32) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        w.legacy_test_bootstrap = true;
        w.enclaves.clear();
        let owner = PlayerId(5600);
        w.step(&[Command::AddPlayer { id: owner, name: "Yard work".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        s.set_population(0.05);
        for body in &mut s.bodies {
            body.assignments.clear();
            body.structures.remove(&K::Shipyard);
        }
        let body = &mut s.bodies[0];
        body.structures.insert(K::Shipyard, 1);
        let body_id = body.id;
        for (c, _) in crate::build::TINY_FREIGHTER_RECIPE.costs { s.stockpile.insert(*c, 1000.0); }
        (w, owner, home, body_id)
    }

    fn assign(w: &mut World, owner: PlayerId, home: EntityId, body: u32, workers: u32) {
        w.step(&[Command::SetAssignment { player_id: owner, system_id: home,
            refining_ore: None,
            body_id: Some(body), structure: K::Shipyard, workers, specialists: BTreeMap::new() }]);
    }

    fn build(w: &mut World, owner: PlayerId, home: EntityId) {
        w.step(&[Command::BuildShip { player_id: owner, system_id: home,
            ship_kind: ShipKind::TinyFreighter, join: None, loadout: Default::default() }]);
        assert_eq!(w.build_queue.len(), 1);
    }

    #[test]
    fn an_unstaffed_shipyard_waits_instead_of_building() {
        let (mut w, owner, home, body) = yard();
        build(&mut w, owner, home);
        let initial = w.build_queue[0].clone();
        assert_eq!(initial.complete_tick, u64::MAX);
        for _ in 0..(crate::build::TINY_FREIGHTER_RECIPE.build_ticks * 2) { w.step(&[]); }
        assert_eq!(w.build_queue[0], initial, "darkness/waiting does not manufacture work or reports");
        assign(&mut w, owner, home, body, 1);
        let due = w.build_queue[0].complete_tick;
        let mut spawned = 0;
        while w.tick <= due + 1 {
            spawned += w.step(&[]).iter().filter(|e| matches!(e.payload,
                EventPayload::ShipSpawned { owner: o, kind: ShipKind::TinyFreighter, .. } if o == owner)).count();
        }
        assert_eq!(spawned, 1);
        assert!(w.build_queue.is_empty());
    }

    #[test]
    fn removing_workers_pauses_earned_work_and_resuming_does_not_restart_or_double_charge() {
        let (mut w, owner, home, body) = yard();
        assign(&mut w, owner, home, body, 1);
        build(&mut w, owner, home);
        let start = w.build_queue[0].started_tick;
        let old_due = w.build_queue[0].complete_tick;
        let stable = w.build_queue[0].clone();
        for _ in 0..90 { w.step(&[]); }
        assert_eq!(w.build_queue[0], stable, "constant-rate work is one compressed segment");
        let earned = w.build_queue[0].ship_work.as_ref().unwrap().completed_at(w.tick);
        assign(&mut w, owner, home, body, 0);
        let frozen = w.build_queue[0].clone();
        assert_eq!(frozen.ship_work.as_ref().unwrap().completed, earned);
        assert!(earned > 0.0 && earned < frozen.ship_work.as_ref().unwrap().required);
        assert_eq!(frozen.complete_tick, u64::MAX);
        while w.tick < old_due + 100 { w.step(&[]); }
        assert_eq!(w.build_queue[0], frozen, "an expired old estimate cannot finish a paused hull");
        w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assign(&mut w, owner, home, body, 1);
        let resumed = &w.build_queue[0];
        assert_eq!(resumed.started_tick, start);
        assert_eq!(resumed.ship_work.as_ref().unwrap().completed, earned);
        let due = resumed.complete_tick;
        assert!(due - w.tick < crate::build::TINY_FREIGHTER_RECIPE.build_ticks, "resume only remaining work");
        while w.tick <= due + 1 { w.step(&[]); }
        assert!(w.build_queue.is_empty());
        for (good, units) in crate::build::TINY_FREIGHTER_RECIPE.costs {
            assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile[good],
                1000.0 - units, "pausing/resuming never charges {good:?} twice");
        }
    }

    #[test]
    fn only_the_jobs_body_and_gating_yard_supply_work() {
        let (mut w, owner, home, body) = yard();
        assign(&mut w, owner, home, body, 1);
        build(&mut w, owner, home);
        for (ship, gate) in [(ShipKind::Destroyer, K::NavalDrydock), (ShipKind::Titan, K::CapitalSlipway)] {
            // Isolate yard-rate selection from the unrelated hull-unlock gates.
            w.build_queue[0].what = BuildKind::Ship { ship };
            w.build_queue[0].construction_yard = Some(gate);
            w.update_ship_build_work();
            assert_eq!(w.build_queue[0].ship_work.as_ref().unwrap().rate, 0.0);
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            let other = s.bodies.iter_mut().find(|b| b.id != body).unwrap();
            other.structures.insert(gate, 1);
            other.assignments.insert(gate, Assignment::crew(1));
            w.update_ship_build_work();
            assert_eq!(w.build_queue[0].ship_work.as_ref().unwrap().rate, 0.0, "wrong planet");
            let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == body).unwrap();
            b.structures.insert(gate, 1);
            b.assignments.insert(gate, Assignment::crew(1));
            w.update_ship_build_work();
            assert_eq!(w.build_queue[0].ship_work.as_ref().unwrap().rate, 1.25);
        }
    }

    #[test]
    fn legacy_freighters_finish_at_the_paid_yard_without_occupying_orbital_slips() {
        for ship in [ShipKind::LargeFreighter, ShipKind::HeavyFreighter, ShipKind::BulkFreighter] {
            let (mut w, owner, home, body) = yard();
            assign(&mut w, owner, home, body, 1);
            build(&mut w, owner, home);
            let mut saved = serde_json::to_value(&w.build_queue[0]).unwrap();
            saved.as_object_mut().unwrap().remove("construction_yard");
            saved["what"] = serde_json::to_value(BuildKind::Ship { ship }).unwrap();
            w.build_queue[0] = serde_json::from_value(saved).unwrap();
            let legacy = w.build_queue[0].clone();
            assert_eq!(legacy.work_yard(), Some(K::Shipyard));
            assert_eq!(serde_json::from_str::<crate::build::BuildJob>(&serde_json::to_string(&legacy).unwrap()).unwrap(), legacy);
            w.update_ship_build_work();
            assert_eq!(w.build_queue[0].ship_work.as_ref().unwrap().rate, 1.25);
            // The old job still occupies its ground slip, not the new orbital one.
            let mut events = Vec::new();
            w.apply_build(owner, home, None, BuildKind::Ship { ship: ShipKind::TinyFreighter }, None, Default::default(), &mut events);
            assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
                reason: crate::event::BuildRejectReason::NoSlip { .. }, .. })));
            w.players.get_mut(&owner).unwrap().research.completed.insert("prop_line_express_charters".into());
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            s.bodies.iter_mut().find(|b| b.id == body).unwrap().structures.insert(K::NavalDrydock, 1);
            for (good, _) in crate::build::recipe_for(BuildKind::Ship { ship: ShipKind::LargeFreighter }).costs {
                s.stockpile.insert(*good, 10_000.0);
            }
            w.apply_build(owner, home, None, BuildKind::Ship { ship: ShipKind::LargeFreighter }, None, Default::default(), &mut events);
            assert_eq!(w.build_queue.len(), 2);
            assert_eq!(w.build_queue[1].construction_yard, Some(K::NavalDrydock));
            assert_eq!(w.build_queue[1].complete_tick, u64::MAX, "ground workforce cannot build the new orbital job");
            // Resolve the paid legacy work once, without inventing an orbital yard assignment.
            w.tick = legacy.complete_tick;
            w.resolve_builds(&mut events);
            assert_eq!(events.iter().filter(|e| matches!(e.payload, EventPayload::ShipSpawned { kind, .. } if kind == ship)).count(), 1);
            assert_eq!(w.build_queue.len(), 1);
            assert_eq!(w.build_queue[0].construction_yard, Some(K::NavalDrydock));
        }
    }

    #[test]
    fn staffing_speed_changes_preserve_fraction_and_legacy_jobs_preserve_known_work() {
        let mut work = BuildWork { required: 360.0, completed: 0.0, at_tick: 10, rate: 1.125 };
        work.set_rate(110, 1.25);
        assert_eq!(work.completed, 112.5);
        assert_eq!(work.completion_tick(), Some(308));
        work.set_rate(210, 0.0);
        assert_eq!(work.completed_at(500), 237.5);
        assert_eq!(work.completion_tick(), None);

        let (mut w, owner, home, _) = yard();
        build(&mut w, owner, home);
        w.build_queue[0].ship_work = None;
        w.build_queue[0].started_tick = Some(w.tick - 1);
        w.build_queue[0].complete_tick = w.tick + 99;
        w.update_ship_build_work();
        let migrated = w.build_queue[0].ship_work.as_ref().unwrap();
        assert_eq!((migrated.completed, migrated.required, migrated.rate), (1.0, 100.0, 0.0));
        assert_eq!(w.build_queue[0].complete_tick, u64::MAX);
    }
}
