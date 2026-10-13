//! Interdiction fields in the world: a fleet raises one when the order reaches
//! it, its destroyer holds it up, and it collapses when the destroyer leaves
//! or is lost, or when it runs out. Movement treats a field as a gravity well
//! (see `integrate_movement`) and jumps can't start or end inside one (see
//! `jump_failure_for`).
use super::*;
use crate::event::OrderRejectReason;
use crate::interdiction::{FieldEnd, InterdictionField, FIELD_COOLDOWN_S, FIELD_FUEL, FIELD_RADIUS, FIELD_SECONDS};
use crate::module::ModuleKind;

/// A live destroyer in the fleet carries the module.
pub(super) fn interdictor(fleet: &Fleet) -> bool {
    !fleet.disposable && fleet.ships.iter().any(|ship| ship.kind == ShipKind::Destroyer
        && ship.hp > 0.0 && ship.loadout.modules().contains(&ModuleKind::InterdictionField))
}

impl World {
    /// An interdiction field covers `pos`.
    pub fn in_interdiction_field(&self, pos: Vec2) -> bool {
        self.interdictions.values().any(|f| f.contains(pos))
    }

    /// When this fleet's generator can raise another field (`None`: now).
    pub fn interdiction_ready_at(&self, fleet: EntityId) -> Option<f64> {
        self.interdiction_ready.get(&fleet).copied().filter(|t| *t > self.time)
    }

    /// The order reached the fleet: raise a field where it stands, or say why not.
    pub(super) fn deploy_interdiction(&mut self, owner: PlayerId, id: EntityId, events: &mut Vec<Event>) {
        let reason = match self.fleets.get(&id).filter(|f| f.owner == owner) {
            None => Some(OrderRejectReason::DeliveryConditionsChanged),
            Some(f) if !interdictor(f) => Some(OrderRejectReason::NoInterdictor),
            Some(f) if self.in_gravity_well(f.pos) || self.in_interdiction_field(f.pos) =>
                Some(OrderRejectReason::InterdictorInGravityWell),
            Some(_) if self.interdiction_ready_at(id).is_some() => Some(OrderRejectReason::InterdictorRecharging),
            Some(f) if f.fuel < FIELD_FUEL => Some(OrderRejectReason::InterdictorLowFuel),
            Some(_) => None,
        };
        if let Some(reason) = reason {
            events.push(Event::new(self.time, EventPayload::OrderRejected { owner, fleet: id, target: None, reason }));
            return;
        }
        if !self.fleet_supplied_for_orders(id, owner, events) {
            return;
        }
        let engaged = self.engagements.values().any(|e| e.attackers.contains(&id) || e.defenders.contains(&id));
        let field = self.alloc_entity_id();
        let fleet = self.fleets.get_mut(&id).unwrap();
        fleet.burn_fusion(FIELD_FUEL);
        // The generator holds station: the field collapses if it leaves.
        if !engaged {
            fleet.order = FleetOrder::Idle;
            tenders::hold(fleet);
        }
        let pos = fleet.pos;
        let expires_at = self.time + FIELD_SECONDS;
        self.interdictions.insert(field, InterdictionField {
            id: field, owner, fleet: id, pos, radius: FIELD_RADIUS, started_at: self.time, expires_at,
        });
        self.interdiction_ready.insert(id, self.time + FIELD_COOLDOWN_S);
        events.push(Event::new(self.time, EventPayload::InterdictionRaised {
            owner, fleet: id, field, pos, radius: FIELD_RADIUS, expires_at,
        }));
    }

    /// Collapse every field whose destroyer left or was lost, or whose time ran out.
    pub(super) fn step_interdictions(&mut self, events: &mut Vec<Event>) {
        let ended: Vec<(EntityId, FieldEnd)> = self.interdictions.values().filter_map(|f| {
            let end = match self.fleets.get(&f.fleet) {
                Some(fleet) if fleet.owner != f.owner || !interdictor(fleet) => FieldEnd::Lost,
                None => FieldEnd::Lost,
                Some(fleet) if !f.contains(fleet.pos) => FieldEnd::Left,
                Some(_) if self.time >= f.expires_at => FieldEnd::Expired,
                Some(_) => return None,
            };
            Some((f.id, end))
        }).collect();
        for (id, end) in ended {
            let f = self.interdictions.remove(&id).expect("listed above");
            events.push(Event::new(self.time, EventPayload::InterdictionCollapsed {
                owner: f.owner, fleet: f.fleet, field: f.id, pos: f.pos, end,
            }));
        }
        let now = self.time;
        self.interdiction_ready.retain(|_, ready| *ready > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Loadout, ModuleKind as M};

    /// A destroyer with the field generator, idle in open space far from any
    /// system, and its owner.
    fn scene() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(125, 4));
        let owner = PlayerId(907);
        w.step(&[Command::AddPlayer { id: owner, name: "Interdiction test".into() }]);
        w.fleets.clear();
        w.enclaves.clear();
        w.nebulas.clear();
        let pos = open_space(&w, w.players[&owner].command_center);
        let id = w.alloc_entity_id();
        let mut dictor = Fleet::single(id, owner, ShipKind::Destroyer, pos, FleetOrder::Idle, None);
        dictor.set_fitted(ShipKind::Destroyer, &Loadout::new(vec![M::InterdictionField]), 1);
        dictor.fuel = dictor.fuel_capacity();
        w.fleets.insert(id, dictor);
        (w, owner, id)
    }

    /// A point well clear of every gravity well, near `from`.
    fn open_space(w: &World, from: Vec2) -> Vec2 {
        (1..40).map(|k| from + Vec2::new(5_000.0 * k as f64, 3_000.0))
            .find(|p| !w.in_gravity_well(*p) && w.systems.iter().all(|s| s.pos.distance(*p) > 6_000.0))
            .expect("open space nearby")
    }

    fn deliver(w: &mut World, owner: PlayerId, id: EntityId) -> Vec<Event> {
        let mut events = vec![];
        w.apply(&Command::DeployInterdiction { player_id: owner, fleet_id: id }, &mut events);
        let arrival = w.pending_orders.iter().map(|o| o.apply_time).fold(w.time, f64::max);
        w.time = arrival;
        w.deliver_due_orders(&mut events);
        events
    }

    fn rejected(events: &[Event]) -> Option<OrderRejectReason> {
        events.iter().find_map(|e| match e.payload {
            EventPayload::OrderRejected { reason, .. } => Some(reason),
            _ => None,
        })
    }

    #[test]
    fn a_field_rises_where_the_order_finds_the_fleet() {
        let (mut w, owner, id) = scene();
        let fuel = w.fleets[&id].fuel;
        let events = deliver(&mut w, owner, id);
        assert!(rejected(&events).is_none(), "{events:?}");
        let field = w.interdictions.values().next().expect("a field");
        assert_eq!((field.owner, field.fleet, field.pos, field.radius), (owner, id, w.fleets[&id].pos, FIELD_RADIUS));
        assert!(w.in_interdiction_field(field.pos + Vec2::new(FIELD_RADIUS - 1.0, 0.0)));
        assert_eq!(w.fleets[&id].fuel, fuel - FIELD_FUEL);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::InterdictionRaised { .. })));
        assert!(w.interdiction_ready_at(id).is_some(), "recharging");
        // Its world state survives a save.
        let saved = serde_json::to_string(&w).unwrap();
        let restored: World = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.interdictions, w.interdictions);
        assert_eq!(restored.interdiction_ready_at(id), w.interdiction_ready_at(id));
    }

    #[test]
    fn a_fleet_in_warp_drops_out_crossing_a_field() {
        let (mut w, owner, id) = scene();
        deliver(&mut w, owner, id);
        let centre = w.interdictions.values().next().unwrap().pos;
        // A cruiser in warp, headed straight through the field.
        let runner = w.alloc_entity_id();
        let start = centre - Vec2::new(4_000.0, 0.0);
        let mut cruiser = Fleet::single(runner, PlayerId(908), ShipKind::Cruiser, start,
            FleetOrder::MoveTo { dest: centre + Vec2::new(4_000.0, 0.0) }, None);
        cruiser.fuel = cruiser.fuel_capacity();
        w.fleets.insert(runner, cruiser);
        let (mut warp_outside, mut thrusters_inside) = (false, true);
        for _ in 0..(60.0 / DT) as u32 {
            w.time += DT;
            w.tick += 1;
            w.integrate_movement(&mut vec![]);
            let f = &w.fleets[&runner];
            let inside = f.pos.distance(centre) < FIELD_RADIUS - 50.0;
            if !inside && f.regime == crate::transit::Regime::Warp { warp_outside = true; }
            if inside && f.regime == crate::transit::Regime::Warp { thrusters_inside = false; }
        }
        assert!(warp_outside, "it warps in open space");
        assert!(thrusters_inside, "and crosses the field on thrusters");
    }

    #[test]
    fn no_jump_starts_or_ends_inside_a_field() {
        let (mut w, owner, id) = scene();
        deliver(&mut w, owner, id);
        let centre = w.interdictions.values().next().unwrap().pos;
        let jumper = w.alloc_entity_id();
        let scout = Fleet::single(jumper, PlayerId(908), ShipKind::Raider, centre + Vec2::new(500.0, 0.0), FleetOrder::Idle, None);
        assert!(scout.can_jump());
        w.fleets.insert(jumper, scout.clone());
        assert_eq!(w.jump_failure_for(&w.fleets[&jumper], centre + Vec2::new(20_000.0, 0.0)),
            Some(crate::event::JumpFailReason::OriginInGravityWell));
        let mut outside = scout;
        outside.pos = centre + Vec2::new(20_000.0, 0.0);
        assert_eq!(w.jump_failure_for(&outside, centre + Vec2::new(300.0, 0.0)),
            Some(crate::event::JumpFailReason::TargetInGravityWell));
    }

    #[test]
    fn a_field_collapses_when_its_destroyer_leaves_is_lost_or_runs_out() {
        for case in 0..3 {
            let (mut w, owner, id) = scene();
            deliver(&mut w, owner, id);
            let mut events = vec![];
            match case {
                0 => { let f = w.fleets.get_mut(&id).unwrap(); f.pos = f.pos + Vec2::new(FIELD_RADIUS + 10.0, 0.0); }
                1 => { w.fleets.remove(&id); }
                _ => { w.time += FIELD_SECONDS; }
            }
            w.step_interdictions(&mut events);
            assert!(w.interdictions.is_empty(), "case {case}");
            let end = events.iter().find_map(|e| match e.payload {
                EventPayload::InterdictionCollapsed { end, .. } => Some(end),
                _ => None,
            });
            assert_eq!(end, Some([FieldEnd::Left, FieldEnd::Lost, FieldEnd::Expired][case]));
        }
    }

    #[test]
    fn the_order_is_refused_without_a_generator_out_of_wells_recharged_and_fuelled() {
        // No generator.
        let (mut w, owner, id) = scene();
        w.fleets.get_mut(&id).unwrap().set_fitted(ShipKind::Destroyer, &Loadout::default(), 1);
        assert_eq!(rejected(&deliver(&mut w, owner, id)), Some(OrderRejectReason::NoInterdictor));
        // Inside a system's gravity well.
        let (mut w, owner, id) = scene();
        let system = w.systems[0].pos;
        w.fleets.get_mut(&id).unwrap().pos = system + Vec2::new(100.0, 0.0);
        assert_eq!(rejected(&deliver(&mut w, owner, id)), Some(OrderRejectReason::InterdictorInGravityWell));
        // Recharging after the first field.
        let (mut w, owner, id) = scene();
        deliver(&mut w, owner, id);
        w.interdictions.clear();
        assert_eq!(rejected(&deliver(&mut w, owner, id)), Some(OrderRejectReason::InterdictorRecharging));
        w.time += FIELD_COOLDOWN_S;
        w.step_interdictions(&mut vec![]);
        assert!(rejected(&deliver(&mut w, owner, id)).is_none(), "ready again");
        // Dry tanks.
        let (mut w, owner, id) = scene();
        w.fleets.get_mut(&id).unwrap().fuel = FIELD_FUEL - 1.0;
        assert_eq!(rejected(&deliver(&mut w, owner, id)), Some(OrderRejectReason::InterdictorLowFuel));
        assert!(w.interdictions.is_empty());
    }
}
