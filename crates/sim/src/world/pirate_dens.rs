//! §pirate-dens (Peter, Oct 24 2026): every pirate belongs to a den. A pirate
//! that retreats flies to its den rather than off into space; at its den it
//! refuels and mends; and once pirates burn fuel, one whose stores can no
//! longer carry it home turns back while they still can.
use super::*;

/// Whether pirates pay for their movement like everyone else. Off until the
/// raid range question is settled: a loaded raider (cargo weighs 28 a unit)
/// reaches ~90k su on both stores, and some raids today run 280k and more.
pub(crate) const PIRATES_BURN_FUEL: bool = false;

/// Whether this owner's fleets pay for their movement. Authority freighters
/// own no economy to fuel from and run on their own terms.
pub(crate) fn pays_fuel(owner: PlayerId) -> bool {
    !owner.is_tca() && (!owner.is_pirate() || PIRATES_BURN_FUEL)
}

/// Hull a pirate mends at its den, as a share of each ship's maximum per
/// second: a wrecked pack is whole again in half an hour.
pub(crate) const DEN_REPAIR_PER_S: f64 = 1.0 / 1_800.0;
/// A pirate turns home once it has spent this share of its full range…
pub(crate) const HOMEWARD_LOW_SHARE: f64 = 0.5;
/// …and its range left falls below this many times the trip home…
pub(crate) const HOMEWARD_MARGIN: f64 = 1.25;
/// …plus this slack, for the thruster legs inside the two wells.
pub(crate) const HOMEWARD_SLACK_SU: f64 = 10_000.0;
/// A pack sets out from its den only with this share of its hull.
pub(crate) const DEN_READY_HULL: f64 = 0.9;

/// Warp range on these stores: the fusion tank at Full, then the fission store
/// at Slow, which a fleet switches to when the tank runs dry.
fn warp_range(f: &Fleet, fuel: f64, uranium: f64) -> f64 {
    let per_su = |slow: bool| crate::fuel::FUEL_PER_MASS_DISTANCE * f.mass().max(1e-9)
        / crate::transit::WARP_FACTOR * f.cruise_mult_for(slow) * f.fuel_research.consumption;
    let full = fuel.max(0.0) / per_su(false).max(1e-12);
    let slow = if f.can_slow() { uranium.max(0.0) / per_su(true).max(1e-12) } else { 0.0 };
    full + slow
}

/// Warp range left on the fleet's own stores.
pub(crate) fn warp_range_left(f: &Fleet) -> f64 {
    warp_range(f, f.fuel, f.uranium)
}

impl World {
    fn den_standing(&self, den: EntityId) -> bool {
        self.enclaves.get(&den).is_some_and(|e| e.active(self.time))
    }

    /// Every pirate has a den. It keeps a standing one; otherwise it takes the
    /// den whose pack it is, the base whose patrol or support guard it is, or the
    /// base of its home raid, and failing those the nearest standing den. A
    /// founder's privateer den holds only its own garrison.
    pub(super) fn assign_pirate_dens(&mut self) {
        let homeless: Vec<EntityId> = self.fleets.iter()
            .filter(|(_, f)| f.owner.is_pirate() && !f.pirate_den.is_some_and(|den| self.den_standing(den)))
            .map(|(id, _)| *id)
            .collect();
        for fid in homeless {
            let linked = self.enclaves.iter().find(|(_, e)| e.pack == Some(fid)).map(|(id, _)| *id)
                .or_else(|| self.pirate_campaign.bases.iter()
                    .find(|(_, b)| b.patrol == Some(fid) || b.supports.iter().any(|s| s.guards.contains(&fid)))
                    .map(|(id, _)| *id))
                .or_else(|| self.pirate_home_raids.get(&fid).map(|raid| raid.base))
                .filter(|den| self.den_standing(*den));
            let pos = self.fleets[&fid].pos;
            let den = linked.or_else(|| self.enclaves.iter()
                .filter(|(_, e)| e.active(self.time) && !e.founder_den)
                .filter_map(|(id, _)| self.systems.iter().find(|s| s.id == *id).map(|s| (*id, s.pos.distance(pos))))
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                .map(|(id, _)| id));
            self.fleets.get_mut(&fid).unwrap().pirate_den = den;
        }
    }

    /// Where a pirate's den is, when one stands.
    pub(crate) fn pirate_den_pos(&self, fid: EntityId) -> Option<Vec2> {
        let den = self.fleets.get(&fid)?.pirate_den.filter(|den| self.den_standing(*den))?;
        self.systems.iter().find(|s| s.id == den).map(|s| s.pos)
    }

    /// At its den a pirate out of battle fills both stores, and mends unless
    /// the den is under attack or holds a fixed site, whose garrison must not
    /// undo an earlier expedition's damage.
    pub(super) fn pirate_den_services(&mut self) {
        let engaged: std::collections::BTreeSet<EntityId> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let home: Vec<(EntityId, bool)> = self.fleets.iter()
            .filter(|(id, f)| f.owner.is_pirate() && !engaged.contains(id))
            .filter_map(|(id, f)| {
                let den = f.pirate_den?;
                let pos = self.pirate_den_pos(*id)?;
                if f.pos.distance(pos) > pirate::PIRATE_ASSAULT_RADIUS { return None; }
                let contested = self.fleets.values().any(|other| !other.owner.is_sentinel()
                    && other.is_combatant() && other.pos.distance(pos) <= pirate::PIRATE_ASSAULT_RADIUS);
                let fixed = self.enclaves.get(&den).is_some_and(|e| e.permanent());
                Some((*id, !contested && !fixed))
            })
            .collect();
        for (fid, mend) in home {
            let f = self.fleets.get_mut(&fid).unwrap();
            f.fuel = f.fuel.max(f.fuel_capacity());
            f.uranium = f.uranium.max(f.uranium_capacity());
            f.stalled = false;
            if mend {
                for ship in f.ships.iter_mut() {
                    ship.hp = (ship.hp + ship.max_hp() * DEN_REPAIR_PER_S * DT).min(ship.max_hp());
                }
            }
        }
    }

    /// Before this tick's movement: a pirate under way that has spent half its
    /// range and can no longer carry the rest home with a margin breaks off and
    /// heads for its den. One sent out far from its den on full stores keeps
    /// its post. A home raid ends first, so its own withdrawal runs.
    pub(super) fn pirates_turn_home(&mut self, events: &mut Vec<Event>) {
        let engaged: std::collections::BTreeSet<EntityId> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let turning: Vec<(EntityId, Vec2)> = self.fleets.iter()
            .filter(|(id, f)| f.owner.is_pirate() && !engaged.contains(id) && !matches!(f.order, FleetOrder::Idle))
            .filter_map(|(id, f)| {
                let den = self.pirate_den_pos(*id)?;
                let trip = f.pos.distance(den);
                let homeward = matches!(f.order, FleetOrder::MoveTo { dest } if dest.distance(den) <= 1.0);
                let left = warp_range_left(f);
                let low = left < warp_range(f, f.fuel_capacity(), f.uranium_capacity()) * HOMEWARD_LOW_SHARE;
                (trip > pirate::PIRATE_ASSAULT_RADIUS && !homeward && low
                    && left < trip * HOMEWARD_MARGIN + HOMEWARD_SLACK_SU).then_some((*id, den))
            })
            .collect();
        for (fid, den) in turning {
            if self.pirate_home_raids.contains_key(&fid) {
                self.end_home_raid(fid, events);
            }
            if let Some(f) = self.fleets.get_mut(&fid) {
                f.order = FleetOrder::MoveTo { dest: den };
                f.pursuit_plan = None;
            }
        }
    }

    /// A pack sets out from its den only whole enough to fight.
    pub(super) fn pirate_ready(&self, fid: EntityId) -> bool {
        self.fleets.get(&fid).is_some_and(|f| {
            let (hp, max) = f.ships.iter().fold((0.0, 0.0), |(hp, max), s| (hp + s.hp.max(0.0), max + s.max_hp()));
            max <= 0.0 || hp / max >= DEN_READY_HULL
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One active hideout far out, one pirate somewhere else, nothing linking them.
    fn scene() -> (World, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(321, 4));
        w.fleets.clear();
        w.pirate_home_raids.clear();
        w.pirate_campaign.bases.clear();
        let den = *w.enclaves.iter().find(|(_, e)| !pirate::permanent_site(e.tier) && !e.founder_den).unwrap().0;
        w.enclaves.retain(|id, _| *id == den);
        let e = w.enclaves.get_mut(&den).unwrap();
        e.pack = None;
        e.next_launch_at = 1e12;
        e.next_grow_at = 1e12;
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let id = w.alloc_entity_id();
        let mut f = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, den_pos + Vec2::new(60_000.0, 0.0), FleetOrder::Idle, None);
        f.reset_to(ShipKind::Raider, 1);
        w.fleets.insert(id, f);
        (w, den, id)
    }

    #[test]
    fn every_pirate_gets_a_den_and_a_standing_one_replaces_a_cleared_one() {
        let (mut w, den, id) = scene();
        w.step(&[]);
        assert_eq!(w.fleets[&id].pirate_den, Some(den), "an unlinked pirate takes the nearest standing den");
        w.enclaves.get_mut(&den).unwrap().cleared = true;
        w.step(&[]);
        assert_eq!(w.fleets[&id].pirate_den, None, "no den stands, so none is claimed");
    }

    #[test]
    fn pirates_pay_for_fuel_only_when_switched_on_and_their_den_refills_and_mends_them() {
        assert!(pays_fuel(PlayerId(7)) && !pays_fuel(PlayerId::TCA));
        assert_eq!(pays_fuel(PlayerId::PIRATE), PIRATES_BURN_FUEL);
        let (mut w, den, id) = scene();
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let full = w.fleets[&id].fuel;
        w.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: den_pos + Vec2::new(200_000.0, 0.0) };
        for _ in 0..(60 * crate::config::TICK_HZ) { w.step(&[]); }
        assert_eq!(w.fleets[&id].fuel < full, PIRATES_BURN_FUEL, "a pirate under way pays for its distance only when switched on");
        let f = w.fleets.get_mut(&id).unwrap();
        f.pos = den_pos;
        f.order = FleetOrder::Idle;
        f.fuel = 0.0;
        f.uranium = 0.0;
        for ship in &mut f.ships { ship.hp = ship.max_hp() * 0.85; }
        w.step(&[]);
        let f = &w.fleets[&id];
        assert!((f.fuel - f.fuel_capacity()).abs() < 1e-9 && (f.uranium - f.uranium_capacity()).abs() < 1e-9, "the den fills both stores");
        assert!(!w.pirate_ready(id), "a damaged pack waits at its den");
        for _ in 0..(120 * crate::config::TICK_HZ) { w.step(&[]); }
        assert!(w.pirate_ready(id), "its den mends it"); 
    }

    #[test]
    fn a_pirate_short_of_fuel_turns_for_its_den() {
        let (mut w, den, id) = scene();
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        w.step(&[]);
        let f = w.fleets.get_mut(&id).unwrap();
        f.order = FleetOrder::MoveTo { dest: den_pos + Vec2::new(300_000.0, 0.0) };
        f.uranium = 0.0;
        f.fuel = f.fuel_capacity() * 0.25;
        w.step(&[]);
        assert!(matches!(w.fleets[&id].order, FleetOrder::MoveTo { dest } if dest == den_pos), "it breaks off for home");
    }

    #[test]
    fn a_retreating_pirate_flies_to_its_den_not_into_empty_space() {
        let (mut w, den, id) = scene();
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        w.step(&[]);
        w.send_ship_home(id, PlayerId::PIRATE);
        assert!(matches!(w.fleets[&id].order, FleetOrder::MoveTo { dest } if dest == den_pos));
    }
}
