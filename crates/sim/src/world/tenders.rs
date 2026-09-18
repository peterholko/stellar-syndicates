//! Player-owned mobile fuel service. Orders and onboard ledgers use the same
//! command/observation pipeline as ordinary movement and carried cargo.
use super::*;
use crate::ship::{FuelTransfer, FuelTransferPhase as Phase};
use std::collections::BTreeSet;

/// Tunables: hoses connect only at a stationary, close-range rendezvous.
pub(super) const TRANSFER_RANGE_SU: f64 = 200.0;
const TRANSFER_UNITS_PER_S: f64 = 5.0;

pub(super) fn hold(fleet: &mut Fleet) {
    fleet.vel = Vec2::ZERO;
    fleet.drive_state = crate::ship::DriveState::Thrusters;
    fleet.regime = crate::transit::Regime::Thrusters;
    fleet.pursuit_plan = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commodity as C, Loadout, ModuleKind as M};

    fn scene() -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(125, 4));
        let owner = PlayerId(905);
        w.step(&[Command::AddPlayer { id: owner, name: "Tender test".into() }]);
        w.fleets.clear();
        w.enclaves.clear();
        w.nebulas.clear();
        let pos = w.players[&owner].command_center + Vec2::new(60_000.0, 0.0);
        let id = w.alloc_entity_id();
        let target = w.alloc_entity_id();
        let mut tender = Fleet::single(id, owner, ShipKind::Convoy, pos, FleetOrder::Idle, None);
        tender.set_fitted(ShipKind::Convoy, &Loadout::new(vec![M::FuelTransferRig]), 1);
        tender.fuel = 50.0;
        tender.add_cargo(C::Fuel, 40);
        tender.add_cargo(C::MetallicOre, 7);
        let mut receiver = Fleet::single(target, owner, ShipKind::Raider,
            pos + Vec2::new(100.0, 0.0), FleetOrder::Idle, None);
        receiver.fuel = receiver.fuel_capacity() - 10.25;
        w.fleets.insert(id, tender);
        w.fleets.insert(target, receiver);
        (w, owner, id, target)
    }

    fn pump(w: &mut World, seconds: f64) {
        for _ in 0..(seconds / DT).ceil() as u32 {
            w.time += DT;
            w.tick += 1;
            w.integrate_movement(&mut vec![]);
            w.resolve_tender_transfers();
        }
    }

    fn report(w: &World, id: EntityId) -> FuelTransfer {
        let FleetOrder::Refuel { transfer, .. } = w.fleets[&id].order else { panic!("missing service") };
        transfer
    }

    #[test]
    fn tender_command_waits_for_delivery_and_cannot_create_fuel() {
        let (mut w, owner, id, target) = scene();
        let initial = w.fleets[&target].fuel;
        w.apply(&Command::RefuelFleet { player_id: owner, fleet_id: id, target_id: target }, &mut vec![]);
        assert!(matches!(w.fleets[&id].order, FleetOrder::Idle));
        assert_eq!(w.pending_orders.len(), 1);
        let arrival = w.pending_orders[0].apply_time;
        assert!(arrival > w.time + 1.0);
        w.time = arrival - 0.01;
        w.deliver_due_orders(&mut vec![]);
        assert!(matches!(w.fleets[&id].order, FleetOrder::Idle));
        assert_eq!(w.fleets[&target].fuel, initial);
        w.time = arrival;
        w.deliver_due_orders(&mut vec![]);
        assert_eq!(report(&w, id).phase, Phase::Rendezvous);
        pump(&mut w, 1.0);
        assert!(w.fleets[&target].fuel > initial && w.fleets[&target].fuel < w.fleets[&target].fuel_capacity());
        pump(&mut w, 3.0);
        assert_eq!(report(&w, id).phase, Phase::Complete);
        assert_eq!(report(&w, id).requested, 11);
        assert_eq!(report(&w, id).spent, 11);
        assert!((report(&w, id).delivered - 10.25).abs() < 1e-9);
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), 29);
        assert_eq!(w.fleets[&id].cargo_amount(C::MetallicOre), 7);
        assert_eq!(w.fleets[&id].fuel, 50.0, "never siphon propulsion fuel");
        assert_eq!(w.fleets[&target].fuel, w.fleets[&target].fuel_capacity());
        let saved = serde_json::to_string(&w).unwrap();
        let mut restored: World = serde_json::from_str(&saved).unwrap();
        pump(&mut restored, 2.0);
        assert_eq!(report(&restored, id), report(&w, id));
        assert_eq!(restored.fleets[&id].cargo_amount(C::Fuel), 29, "completion is idempotent across save/load");
    }

    #[test]
    fn tender_rejects_invalid_targets_and_missing_equipment_at_receipt() {
        for case in 0..4 {
            let (mut w, owner, id, target) = scene();
            w.apply(&Command::RefuelFleet { player_id: owner, fleet_id: id, target_id: if case == 0 { id } else { target } }, &mut vec![]);
            match case {
                1 => { w.fleets.get_mut(&target).unwrap().owner = PlayerId(906); }
                2 => { w.fleets.get_mut(&id).unwrap().set_fitted(ShipKind::Convoy, &Loadout::default(), 1); }
                3 => { w.fleets.get_mut(&id).unwrap().remove_cargo(C::Fuel, 40); }
                _ => {}
            }
            w.time = w.pending_orders[0].apply_time;
            let mut events = vec![];
            w.deliver_due_orders(&mut events);
            assert!(matches!(w.fleets[&id].order, FleetOrder::Idle));
            assert!(events.iter().any(|e| matches!(e.payload, EventPayload::OrderRejected { .. })));
            assert!(w.pending_echoes.iter().all(|e| e.retire_at.is_some()),
                "refusal stays news-gated, never successful compliance");
        }
    }

    #[test]
    fn tender_exhaustion_releases_the_receivers_existing_order() {
        let (mut w, owner, id, target) = scene();
        w.fleets.get_mut(&id).unwrap().remove_cargo(C::Fuel, 37);
        let dest = w.fleets[&target].pos + Vec2::new(50_000.0, 0.0);
        let receiver = w.fleets.get_mut(&target).unwrap();
        receiver.fuel = 0.0;
        receiver.stalled = true;
        receiver.order = FleetOrder::MoveTo { dest };
        w.start_tender_transfer(owner, id, target, &mut vec![]);
        let pos = w.fleets[&target].pos;
        pump(&mut w, 0.4);
        assert_eq!(w.fleets[&target].pos, pos, "hose holds the dry ship while it fills");
        pump(&mut w, 1.0);
        assert_eq!(report(&w, id).phase, Phase::Empty);
        assert_eq!(report(&w, id).delivered, 3.0);
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), 0);
        assert!(matches!(w.fleets[&target].order, FleetOrder::MoveTo { dest: d } if d == dest));
        pump(&mut w, 1.0);
        assert!(w.fleets[&target].pos.distance(pos) > 0.0);
    }

    #[test]
    fn tender_needs_proximity_and_a_stationary_target_and_can_be_cancelled() {
        let (mut w, owner, id, target) = scene();
        w.fleets.get_mut(&target).unwrap().pos.x += 10_000.0;
        w.start_tender_transfer(owner, id, target, &mut vec![]);
        let pos = w.fleets[&id].pos;
        pump(&mut w, 2.0);
        assert!(w.fleets[&id].pos.distance(pos) > 0.0, "tender really travels to its target");
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), 40);
        let nearby = w.fleets[&id].pos + Vec2::new(100.0, 0.0);
        let receiver = w.fleets.get_mut(&target).unwrap();
        receiver.pos = nearby;
        receiver.vel = Vec2::new(30.0, 0.0);
        w.prepare_tenders();
        assert_eq!(report(&w, id).phase, Phase::Waiting);
        w.resolve_tender_transfers();
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), 40);
        w.fleets.get_mut(&target).unwrap().vel = Vec2::ZERO;
        pump(&mut w, 0.5);
        assert!(report(&w, id).spent > 0);
        // Installing a different received order immediately removes the hose
        // lock; no service sidecar can continue pumping after cancellation.
        w.fleets.get_mut(&id).unwrap().order = FleetOrder::Idle;
        let remaining = w.fleets[&id].cargo_amount(C::Fuel);
        assert!(!w.prepare_tenders().contains(&target));
        pump(&mut w, 1.0);
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), remaining);
    }

    #[test]
    fn tender_rig_has_real_manufacturing_unlock_and_no_extra_slot() {
        let mut research = crate::research::ResearchState::default();
        assert!(!crate::research::has_module(&research, M::FuelTransferRig));
        research.completed.insert("prop_expedition_iv_fleet_tenders".into());
        assert!(crate::research::has_module(&research, M::FuelTransferRig));
        assert_eq!(M::FuelTransferRig.workshop(), crate::StructureKind::Shipyard);
        assert!(!crate::build::module_recipe(M::FuelTransferRig).costs.is_empty());
        for hull in crate::ship::ALL_SHIP_KINDS {
            assert_eq!(Loadout::new(vec![M::FuelTransferRig]).validate(hull), hull.is_player_freighter());
        }
        for extra in [M::ExtendedTanks, M::CargoPods, M::FuelTransferRig] {
            assert!(!Loadout::new(vec![M::FuelTransferRig, extra]).validate(ShipKind::Convoy));
        }
    }

    #[test]
    fn tender_stops_on_local_battle_light_and_never_pumps_in_combat() {
        let (mut w, owner, id, target) = scene();
        let battle_pos = w.fleets[&target].pos;
        assert!(!w.in_sovereign_zone(battle_pos));
        let pirate = w.alloc_entity_id();
        w.fleets.insert(pirate, Fleet::single(pirate, PlayerId::PIRATE, ShipKind::Raider,
            battle_pos + Vec2::new(1.0, 0.0), FleetOrder::Attack { target }, None));
        w.resolve_raids(&mut vec![]);
        assert_eq!(w.engagements.len(), 1);
        w.fleets.get_mut(&id).unwrap().pos = battle_pos - Vec2::new(10_000.0, 0.0);
        w.start_tender_transfer(owner, id, target, &mut vec![]);
        let emitted = w.time;
        w.prepare_tenders();
        assert_eq!(report(&w, id).phase, Phase::Rendezvous, "no remote battle truth shortcut");
        w.time = emitted + crate::transit::delay(battle_pos, w.fleets[&id].pos, w.config.c) + 0.1;
        let pos = w.fleets[&id].pos;
        pump(&mut w, 1.0);
        assert_eq!(report(&w, id).phase, Phase::Unsafe);
        assert_eq!(w.fleets[&id].pos, pos, "never chase into the battle");
        assert_eq!(w.fleets[&id].cargo_amount(C::Fuel), 40);
        w.engagements.clear();
        pump(&mut w, 1.0);
        assert_eq!(report(&w, id).phase, Phase::Unsafe, "requires deliberate reassignment");
    }

    #[test]
    fn two_tenders_and_mid_transfer_save_do_not_double_fill() {
        let (mut w, owner, id, target) = scene();
        let second = w.alloc_entity_id();
        let mut other = w.fleets[&id].clone();
        other.id = second;
        w.fleets.insert(second, other);
        w.start_tender_transfer(owner, id, target, &mut vec![]);
        w.start_tender_transfer(owner, second, target, &mut vec![]);
        pump(&mut w, 0.5);
        let mut restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        pump(&mut w, 3.0);
        pump(&mut restored, 3.0);
        for state in [&w, &restored] {
            assert_eq!(state.fleets[&target].fuel, state.fleets[&target].fuel_capacity());
            assert_eq!(report(state, id).spent + report(state, second).spent, 11);
            assert!((report(state, id).delivered + report(state, second).delivered - 10.25).abs() < 1e-9);
            assert_eq!(state.fleets[&id].cargo_amount(C::Fuel) + state.fleets[&second].cargo_amount(C::Fuel), 69);
        }
    }
}

fn fitted(fleet: &Fleet) -> bool {
    !fleet.disposable && fleet.ships.iter().any(|ship| ship.kind.is_player_freighter()
        && ship.hp > 0.0 && ship.loadout.modules().contains(&crate::module::ModuleKind::FuelTransferRig))
}

fn terminal(phase: Phase) -> bool {
    matches!(phase, Phase::Complete | Phase::Empty | Phase::Unsafe | Phase::Unavailable)
}

impl World {
    pub(super) fn start_tender_transfer(&mut self, owner: PlayerId, id: EntityId,
        target: EntityId, events: &mut Vec<Event>)
    {
        let valid = id != target
            && self.fleets.get(&id).is_some_and(|f| f.owner == owner && fitted(f)
                && f.cargo_amount(crate::Commodity::Fuel) > 0)
            && self.fleets.get(&target).is_some_and(|f| f.owner == owner && !f.disposable);
        if !valid {
            events.push(Event::new(self.time, EventPayload::OrderRejected {
                owner, fleet: id, target: Some(target),
                reason: crate::event::OrderRejectReason::DeliveryConditionsChanged,
            }));
            return;
        }
        if !self.fleet_supplied_for_orders(id, owner, events) { return; }
        let fleet = self.fleets.get_mut(&id).unwrap();
        // Explicit reassignment retires any old auto-haul/escort mission only
        // on command receipt. No delivery destination may consume this cargo.
        fleet.mission = None;
        fleet.defense = None;
        fleet.pursuit_plan = None;
        fleet.order = FleetOrder::Refuel {
            transfer: FuelTransfer { target, requested: 0, spent: 0, delivered: 0.0,
                phase: Phase::Rendezvous },
            next_pump: self.time,
        };
    }

    /// Prepare before movement. Safety is an onboard lookout, not a CC truth
    /// lookup: a distant battle cannot stop the tender until its local light
    /// arrives within its sensor range. A combat stop is latched until a new
    /// order; it never automatically chases the charge back into the fight.
    pub(super) fn prepare_tenders(&mut self) -> BTreeSet<EntityId> {
        let engaged: BTreeSet<_> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(&e.defenders).copied()).collect();
        let mut updates = Vec::new();
        let mut receivers = BTreeSet::new();
        for (&id, fleet) in &self.fleets {
            let FleetOrder::Refuel { transfer, .. } = fleet.order else { continue; };
            if terminal(transfer.phase) { continue; }
            let range = 20_000.0 * self.research_mod(fleet.owner, crate::research::ModKey::SensorRange)
                * self.nebula_sensor_factor(fleet.pos);
            let unsafe_here = engaged.contains(&id) || self.engagements.values().any(|e| {
                fleet.pos.distance(e.pos) <= range
                    && e.started_at + crate::transit::delay(e.pos, fleet.pos, self.config.c) <= self.time
            });
            let phase = if !fitted(fleet) { Phase::Unavailable }
                else if unsafe_here { Phase::Unsafe }
                else if fleet.cargo_amount(crate::Commodity::Fuel) == 0 { Phase::Empty }
                else if let Some(target) = self.fleets.get(&transfer.target).filter(|f| f.owner == fleet.owner) {
                    if fleet.pos.distance(target.pos) > TRANSFER_RANGE_SU { Phase::Rendezvous }
                    else if engaged.contains(&transfer.target) { Phase::Unsafe }
                    else if target.vel.length() > 0.5 || matches!(target.order, FleetOrder::Jump { .. }) {
                        Phase::Waiting
                    } else if target.fuel_capacity() - target.fuel <= 1e-9 { Phase::Complete }
                    else { receivers.insert(transfer.target); Phase::Transferring }
                } else { Phase::Unavailable };
            updates.push((id, phase));
        }
        for (id, phase) in updates {
            if let FleetOrder::Refuel { transfer, next_pump } = &mut self.fleets.get_mut(&id).unwrap().order {
                if phase != transfer.phase { *next_pump = self.time + 1.0 / TRANSFER_UNITS_PER_S; }
                transfer.phase = phase;
            }
        }
        receivers
    }

    pub(super) fn resolve_tender_transfers(&mut self) {
        let engaged: BTreeSet<_> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(&e.defenders).copied()).collect();
        let ids: Vec<_> = self.fleets.iter().filter_map(|(&id, f)|
            matches!(f.order, FleetOrder::Refuel { transfer, .. }
                if transfer.phase == Phase::Transferring).then_some(id)).collect();
        // Stable id order makes two tenders servicing the same receiver conserve
        // cargo: the second sees the first's actual fill and never overfills it.
        for id in ids {
            let FleetOrder::Refuel { mut transfer, mut next_pump } = self.fleets[&id].order else { continue; };
            let source = &self.fleets[&id];
            let Some(target) = self.fleets.get(&transfer.target) else { continue; };
            if !fitted(source) || source.owner != target.owner || id == target.id
                || engaged.contains(&id) || engaged.contains(&target.id)
                || source.pos.distance(target.pos) > TRANSFER_RANGE_SU
                || source.vel.length() > 0.5 || target.vel.length() > 0.5
                || matches!(target.order, FleetOrder::Jump { .. }) { continue; }
            let room = (target.fuel_capacity() - target.fuel).max(0.0);
            // Cargo is whole units; a partial final can tops off exactly and its
            // unused fraction is line-purge loss (<1 Fuel). Never round UP tank
            // gain or debit a propulsion tank. Both spent and delivered remain
            // in the report, so this small loss is accounted for, not free fuel.
            transfer.requested = transfer.requested.max(transfer.spent + room.ceil() as u32);
            let due = if self.time + 1e-9 >= next_pump {
                ((self.time - next_pump).max(0.0) * TRANSFER_UNITS_PER_S).floor() as u32 + 1
            } else { 0 };
            let units = due.min(room.ceil() as u32).min(source.cargo_amount(crate::Commodity::Fuel));
            if units > 0 {
                let source = self.fleets.get_mut(&id).unwrap();
                let spent = source.remove_cargo(crate::Commodity::Fuel, units);
                let target = self.fleets.get_mut(&transfer.target).unwrap();
                transfer.delivered += target.refuel(spent as f64);
                target.stalled = false;
                transfer.spent += spent;
                next_pump = self.time + 1.0 / TRANSFER_UNITS_PER_S;
            }
            let target = &self.fleets[&transfer.target];
            if target.fuel_capacity() - target.fuel <= 1e-9 {
                transfer.phase = Phase::Complete;
            } else if self.fleets[&id].cargo_amount(crate::Commodity::Fuel) == 0 {
                transfer.phase = Phase::Empty;
            }
            if terminal(transfer.phase) && transfer.delivered > 0.0 {
                self.fleets.get_mut(&transfer.target).unwrap().stalled = false;
            }
            self.fleets.get_mut(&id).unwrap().order = FleetOrder::Refuel { transfer, next_pump };
        }
    }
}
