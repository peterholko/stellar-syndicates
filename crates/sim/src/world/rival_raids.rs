//! §player-styles (Peter, Oct 31 2026): raiding other corporations. A rival
//! fleet bound for one of your systems is reported once your sensors see it,
//! and a raid can be set to repeat, setting out again after each trip home.
use super::*;
use crate::event::RepeatStop;
use crate::ship::{DockSite, RaidRepeat};

/// How long a repeating raid waits at home for its repairs and a full tank
/// before it sets out anyway (on at least half a tank). Tunable.
pub const REPEAT_REPAIR_WAIT_S: f64 = 30.0 * 60.0;

impl World {
    /// A fleet on a Raid or Blockade order against another corporation's
    /// system is reported to that corporation once a trip, when its Command
    /// Center or Sensor Arrays first see it (the shared detection rule, as the
    /// charter's raid). The warning travels home on the light from the fleet.
    pub(super) fn warn_of_rival_raids(&mut self, events: &mut Vec<Event>) {
        let mut sightings = Vec::new();
        let mut cleared = Vec::new();
        for (&id, f) in &self.fleets {
            let FleetOrder::Blockade { system, .. } = f.order else {
                if f.raid_sighted.is_some() {
                    cleared.push(id);
                }
                continue;
            };
            if f.raid_sighted == Some(system) || f.owner.is_pirate() || f.owner.is_tca() {
                continue;
            }
            let Some(sys) = self.systems.iter().find(|s| s.id == system) else { continue };
            let Some(owner) = sys.owner.filter(|o| *o != f.owner && self.players.contains_key(o)) else { continue };
            let mut sources = vec![(self.players[&owner].command_center, self.config.sensor_range)];
            sources.extend(self.array_sensor_sources(owner));
            if crate::detection::detected(f.detection_signature(), &sources, f.pos) {
                let speed = f.transit_speed() * crate::transit::WARP_FACTOR;
                let eta = (speed > 0.0).then(|| self.time + f.pos.distance(sys.pos) / speed);
                sightings.push((id, owner, f.owner, system, f.total_count(), f.pos, eta));
            }
        }
        for id in cleared {
            if let Some(f) = self.fleets.get_mut(&id) {
                f.raid_sighted = None;
            }
        }
        for (fleet, owner, by, system, ships, pos, eta) in sightings {
            if let Some(f) = self.fleets.get_mut(&fleet) {
                f.raid_sighted = Some(system);
            }
            events.push(Event::new(self.time, EventPayload::RaidSighted { owner, by, system, fleet, ships, pos, eta }));
        }
    }

    /// A repeating raid home from a trip sets out again once its holds are
    /// unloaded, its tanks full and its hulls repaired (or after
    /// [`REPEAT_REPAIR_WAIT_S`]), unless the trip says to stop.
    pub(super) fn restart_repeat_raids(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<EntityId> = self.fleets.iter()
            .filter(|(_, f)| f.raid_repeat.is_some_and(|r| r.homeward))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(f) = self.fleets.get(&id) else { continue };
            let Some(r) = f.raid_repeat else { continue };
            let owner = f.owner;
            let Some(home) = self.players.get(&owner).and_then(|c| c.home_system) else { continue };
            let docked = matches!(f.order, FleetOrder::Idle) && f.mission.is_none()
                && self.dock_of(id) == Some(DockSite::System(home));
            if !docked {
                continue;
            }
            let stop = if r.haul == 0 {
                Some(RepeatStop::Empty)
            } else if !f.cargo_is_empty() {
                Some(RepeatStop::StoresFull)
            } else if f.total_count() < r.ships {
                Some(RepeatStop::LostShips)
            } else if r.capped {
                Some(RepeatStop::Capped)
            } else {
                let victim = self.systems.iter().find(|s| s.id == r.system).and_then(|s| s.owner).filter(|o| *o != owner);
                match victim {
                    Some(v) if self.blockade_allowed(owner, v, true).is_ok() && !self.pvp_shielded(v)
                        && !self.on_leave(owner) && self.raid_loss_left(v) >= 1.0 => None,
                    Some(v) if self.raid_loss_left(v) < 1.0 => Some(RepeatStop::Capped),
                    _ => Some(RepeatStop::Refused),
                }
            };
            if let Some(reason) = stop {
                if let Some(f) = self.fleets.get_mut(&id) {
                    f.raid_repeat = None;
                }
                events.push(Event::new(self.time, EventPayload::RaidRepeatStopped { owner, fleet: id, system: r.system, reason }));
                continue;
            }
            // Refuelled and repaired first, as far as home can manage.
            let sys = self.systems.iter().find(|s| s.id == home);
            let (tank, room, fuel) = if f.burns_uranium() {
                (f.uranium, f.uranium_capacity(), crate::fuel::FISSION_FUEL)
            } else {
                (f.fuel, f.fuel_capacity(), crate::fuel::MOVEMENT_FUEL)
            };
            let fuelled = tank + 1e-6 >= room * 0.98;
            let since = if r.docked_at > 0.0 { r.docked_at } else { self.time };
            let waited = self.time - since >= REPEAT_REPAIR_WAIT_S;
            // Home's reserves (power, refining) aren't for raids: after the
            // wait a half tank goes, less stops it.
            if !fuelled && (sys.is_none_or(|s| s.stockpile.get(&fuel).copied().unwrap_or(0.0) < 1.0)
                || waited && tank < room * 0.5)
            {
                if let Some(f) = self.fleets.get_mut(&id) {
                    f.raid_repeat = None;
                }
                events.push(Event::new(self.time, EventPayload::RaidRepeatStopped {
                    owner, fleet: id, system: r.system, reason: RepeatStop::NoFuel }));
                continue;
            }
            let yard = sys.is_some_and(|s| crate::build::REPAIR_YARDS.iter().any(|k| s.tier(*k) > 0));
            let damaged = f.ships.iter().any(|s| s.hp + 1e-6 < s.max_hp());
            let repaired = !damaged || !yard || waited;
            if !(fuelled || waited) || !repaired {
                if r.docked_at <= 0.0 && let Some(f) = self.fleets.get_mut(&id) {
                    f.raid_repeat = Some(RaidRepeat { docked_at: self.time, ..r });
                }
                continue;
            }
            if let Some(f) = self.fleets.get_mut(&id) {
                f.order = FleetOrder::Blockade { system: r.system, station: r.station, raid: true, repeat: true };
                f.raid_sighted = None;
                f.raid_repeat = Some(RaidRepeat { trip: r.trip + 1, ships: f.total_count(), homeward: false, haul: 0,
                    capped: false, docked_at: 0.0, ..r });
            }
        }
    }
}
