use super::*;

/// §research-wiring Mass Streams: speed on a corporation's stream, how wide
/// its corridor runs, and how near an end a destination must lie.
pub const MASS_STREAM_SPEED: f64 = 1.4;
pub const MASS_STREAM_WIDTH_SU: f64 = 3_000.0;
pub const MASS_STREAM_END_SU: f64 = 2_000.0;

/// Does a fleet at `pos` bound for `dest` ride the stream between `a` and
/// `b`: inside its corridor, heading for one of its ends?
pub(crate) fn rides_stream(pos: Vec2, dest: Vec2, a: Vec2, b: Vec2) -> bool {
    let ab = b - a;
    let t = ((pos - a).dot(ab) / ab.dot(ab).max(1e-9)).clamp(0.0, 1.0);
    pos.distance(a + ab * t) <= MASS_STREAM_WIDTH_SU
        && (dest.distance(a) <= MASS_STREAM_END_SU || dest.distance(b) <= MASS_STREAM_END_SU)
}

impl World {
    pub(super) fn fuel_research_for(&self, owner: PlayerId) -> crate::ship::FuelResearch {
        self.players.get(&owner).map_or_else(crate::ship::FuelResearch::default, |corp| crate::ship::FuelResearch {
            cargo: self.syndicate_mult(owner, crate::syndicate::SyndicateBonus::Freight),
            ..crate::ship::FuelResearch::from_state(&corp.research)
        })
    }

    /// One aggregate per corporation, then a cheap copy per fleet, before fuel
    /// payment and before emitting a snapshot. No research lookup in the hot
    /// per-hull loop. These are derived, non-serialized values, not retroactive
    /// fuel: Bunkerage opens empty space and only real dock/tender supplies fill
    /// it. Old saves rebuild from their completed programmes without a migration
    /// grant. History still records capacity at emission, so remote UI waits for
    /// that report rather than reading the current research as fleet telemetry.
    pub(super) fn refresh_fleet_fuel_research(&mut self) {
        let effects: BTreeMap<_, _> = self.players.keys().map(|&owner| (owner, self.fuel_research_for(owner))).collect();
        // A stream carries a fleet inside its corridor bound for one of its
        // ends; an interdiction field stalls it like any warp.
        let streams = self.mass_streams();
        let riding: std::collections::BTreeSet<EntityId> = self.fleets.iter().filter(|(_, f)| {
            matches!(f.order, FleetOrder::MoveTo { dest } if streams.get(&f.owner)
                .is_some_and(|&(a, b)| rides_stream(f.pos, dest, a, b)))
                && !self.in_interdiction_field(f.pos)
        }).map(|(&id, _)| id).collect();
        for (id, fleet) in self.fleets.iter_mut() {
            fleet.fuel_research = effects.get(&fleet.owner).copied().unwrap_or_default();
            if riding.contains(id) {
                fleet.fuel_research.stream = MASS_STREAM_SPEED;
            }
        }
        self.refresh_storage_research();
    }

    /// §research-wiring Mass Streams: each corporation's live stream, as its
    /// two stars, while neither end is held under blockade.
    pub fn mass_streams(&self) -> BTreeMap<PlayerId, (Vec2, Vec2)> {
        self.players.keys().filter_map(|&owner| {
            let Some(crate::research::DesignationTarget::SystemPair(a, b)) =
                self.designation(owner, crate::research::Cap::MassStreams) else { return None };
            let end = |id: EntityId| self.systems.iter().find(|s| s.id == id)
                .filter(|s| !s.blockade.as_ref().is_some_and(|b| b.held_since.is_some()))
                .map(|s| s.pos);
            Some((owner, (end(a)?, end(b)?)))
        }).collect()
    }

    /// §research-wiring: each system follows its owner's research — Bulk
    /// Storage's warehouse multiple (1.0 unowned) and the Crown Project world.
    fn refresh_storage_research(&mut self) {
        let mults: BTreeMap<PlayerId, f64> = self.players.keys()
            .map(|&o| (o, self.research_mod(o, crate::research::ModKey::WarehouseCap))).collect();
        let crowns: BTreeMap<PlayerId, crate::research::DesignationTarget> = self.players.keys()
            .filter_map(|&o| self.designation(o, crate::research::Cap::CrownProject).map(|t| (o, t))).collect();
        let acclimated: std::collections::BTreeSet<PlayerId> = self.players.keys().copied()
            .filter(|&o| self.research_flag(o, crate::research::Cap::Xenoacclimation)).collect();
        let campuses: std::collections::BTreeSet<PlayerId> = self.players.keys().copied()
            .filter(|&o| self.research_flag(o, crate::research::Cap::TwinCampuses)).collect();
        let orbital: std::collections::BTreeSet<PlayerId> = self.players.keys().copied()
            .filter(|&o| self.research_flag(o, crate::research::Cap::OrbitalHabitats)).collect();
        for sys in &mut self.systems {
            sys.storage_mult = sys.owner.and_then(|o| mults.get(&o).copied()).unwrap_or(1.0);
            sys.twin_campuses = sys.owner.is_some_and(|o| campuses.contains(&o));
            let crown = sys.owner.and_then(|o| crowns.get(&o).copied());
            let acclimates = sys.owner.is_some_and(|o| acclimated.contains(&o));
            let in_orbit = sys.owner.is_some_and(|o| orbital.contains(&o));
            for b in &mut sys.bodies {
                b.crown_project = crown == Some(crate::research::DesignationTarget::Body { system: sys.id, body: b.id });
                b.acclimated = acclimates && b.acclimatable();
                b.orbital_habitat = in_orbit && !b.is_belt();
            }
        }
    }
}

#[cfg(test)]
mod tests {
use super::*;
use crate::cargo::Commodity as C;

fn scene(kind: ShipKind) -> (World, PlayerId, EntityId) {
    let mut w = World::new(SimConfig::for_players(2026, 4));
    let owner = PlayerId(71);
    w.step(&[Command::AddPlayer { id: owner, name: "Fuel research".into() }]);
    w.fleets.clear();
    w.enclaves.clear();
    w.players.get_mut(&owner).unwrap().captains.clear();
    let id = w.alloc_entity_id();
    w.fleets.insert(id, Fleet::single(id, owner, kind, Vec2::new(900_000.0, 900_000.0), FleetOrder::Idle, None));
    (w, owner, id)
}

fn complete(w: &mut World, owner: PlayerId, programme: &str) {
    let r = &mut w.players.get_mut(&owner).unwrap().research;
    r.active = Some(programme.into());
    r.progress = crate::research::cost_of(programme);
    r.add_verb(crate::research::Verb::LyFlown, 200.0);
    let events = w.step(&[]);
    assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::ResearchCompleted { programme: id, .. } if id == programme)));
}

#[test]
fn bunkerage_adds_real_tank_space_without_creating_fuel() {
    for kind in [ShipKind::Scout, ShipKind::TinyFreighter, ShipKind::Colony] {
        let (mut w, owner, id) = scene(kind);
        let (fuel, capacity) = (w.fleets[&id].fuel, w.fleets[&id].fuel_capacity());
        complete(&mut w, owner, "prop_bunkerage");
        assert!((w.fleets[&id].fuel_capacity() - capacity * 1.25).abs() < 1e-9, "{kind:?}");
        assert_eq!(w.fleets[&id].fuel, fuel, "research adds space, not free fuel");
        let home = w.players[&owner].home_system.unwrap();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.stockpile.insert(C::Deuterium, 100.0);
        w.fleets.get_mut(&id).unwrap().pos = sys.pos;
        w.refuel_docked(&mut vec![]);
        assert!((w.fleets[&id].fuel - capacity * 1.25).abs() < 1e-9);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!((sys.stockpile[&C::Deuterium] - (100.0 - capacity * 0.25)).abs() < 1e-9);
    }
}

#[test]
fn efficient_burns_reduces_physical_burn_and_extends_reported_range() {
    for (kind, transit) in [(ShipKind::Scout, crate::ship::TransitMode::Full),
        (ShipKind::TinyFreighter, crate::ship::TransitMode::Slow)]
    {
        let (mut w, owner, id) = scene(kind);
        complete(&mut w, owner, "prop_bunkerage");
        let mut plain = w.clone();
        complete(&mut w, owner, "prop_efficient_burns");
        let burn = |state: &mut World| {
            let f = state.fleets.get_mut(&id).unwrap();
            f.transit = transit;
            f.order = FleetOrder::MoveTo { dest: f.pos + Vec2::new(100_000.0, 0.0) };
            f.drive_state = crate::ship::DriveState::Cruising(crate::transit::Regime::Warp);
            let before = f.propulsion_store();
            let range = f.range_at(crate::transit::WARP_FACTOR);
            for _ in 0..30 { state.integrate_movement(&mut vec![]); }
            (before - state.fleets[&id].propulsion_store(), range, state.fleets[&id].pos)
        };
        let (old, old_range, old_pos) = burn(&mut plain);
        let (new, new_range, new_pos) = burn(&mut w);
        assert!(old > 0.0);
        assert!((new - old * 0.75).abs() < 1e-9, "{kind:?}: old={old}, new={new}");
        assert!((new_range - old_range / 0.75).abs() < 1e-6);
        assert_eq!(new_pos, old_pos, "fuel research does not change velocity");
    }
}

#[test]
fn fuel_research_survives_world_restore_without_filling_the_tank() {
    let (mut w, owner, id) = scene(ShipKind::Colony);
    complete(&mut w, owner, "prop_bunkerage");
    complete(&mut w, owner, "prop_efficient_burns");
    w.fleets.get_mut(&id).unwrap().fuel = 73.0;
    let json = serde_json::to_vec(&w).unwrap();
    let mut restored: World = serde_json::from_slice(&json).unwrap();
    restored.fixup_after_load();
    restored.fixup_after_load();
    let f = &restored.fleets[&id];
    assert_eq!(f.fuel, 73.0);
    assert!((f.fuel_capacity() - 262.5).abs() < 1e-9);
    assert!((f.range_at(crate::transit::WARP_FACTOR) - w.fleets[&id].range_at(crate::transit::WARP_FACTOR)).abs() < 1e-6);
}

#[test]
fn new_hulls_inherit_fuel_research_and_hub_topups_are_still_paid() {
    let (mut w, owner, _) = scene(ShipKind::Colony);
    complete(&mut w, owner, "prop_bunkerage");
    complete(&mut w, owner, "prop_efficient_burns");
    let id = w.alloc_entity_id();
    let pos = Vec2::new(900_000.0, 900_000.0);
    w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Colony, pos, FleetOrder::Idle, None));
    w.step(&[]);
    assert!((w.fleets[&id].fuel - 210.0).abs() < 1e-9);
    assert!((w.fleets[&id].fuel_capacity() - 262.5).abs() < 1e-9);
    w.fleets.get_mut(&id).unwrap().pos = w.hub;
    w.players.get_mut(&owner).unwrap().credits = 100_000.0;
    let before = w.players[&owner].credits;
    let mut events = vec![];
    w.refuel_docked(&mut events);
    assert!(w.players[&owner].credits < before);
    // §fuel-overhaul: whole units, rounded UP so the tank fills; the spill
    // (half a unit here) is paid for, never free fuel.
    assert!((w.fleets[&id].fuel - 262.5).abs() < 1e-9);
    assert!(events.iter().any(|e| matches!(e.payload, EventPayload::HubFuelPurchased {
        fleet, commodity: C::Deuterium, units: 53, .. } if fleet == id)));
}
}
