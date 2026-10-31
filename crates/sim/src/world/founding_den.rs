//! §charter-v4: the privateer den. The Rogue Privateer flies out of a pirate
//! base in the home's den system ([`crate::galaxy::HomeSlot::den_system`]), a
//! local survey stop holding only a rich asteroid belt. The den rises with the
//! privateer, the privateer's log names it, and a lone Interceptor can destroy
//! its weak garrison. It stays cleared, and its belt becomes the charter's.
use super::*;
use crate::pirate::Enclave;

impl World {
    /// This corporation's den system, from its home slot.
    pub(super) fn founding_den_system(&self, owner: PlayerId) -> Option<EntityId> {
        let home = self.players.get(&owner)?.home_system?;
        self.home_slots.iter().find(|h| h.system == Some(home))?.den_system
    }

    /// Raise the den with the privateer, once per charter: an idle garrison of
    /// weak raiders holding the den system, with no defense platforms. Homes
    /// without a den system (overflow homes) skip the den step.
    pub(super) fn ensure_founding_den(&mut self, owner: PlayerId) {
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        if corp.founding.den.is_some() || !corp.founding.enabled || !corp.founding.local_opening {
            return;
        }
        let Some(den) = self.founding_den_system(owner) else {
            return;
        };
        let Some(pos) = self.systems.iter().find(|s| s.id == den && s.owner.is_none()).map(|s| s.pos) else {
            return;
        };
        if !self.enclaves.contains_key(&den) {
            let id = self.alloc_entity_id();
            let mut garrison = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, pos, FleetOrder::Idle, None);
            garrison.reset_to(ShipKind::Raider, crate::founding::DEN_GARRISON_RAIDERS);
            garrison.fill_stores();
            // The privateer's crew: its weak guns and slow hulls.
            garrison.founding_privateer = true;
            for ship in &mut garrison.ships {
                ship.hp = ship.max_hp() * crate::founding::DEN_GARRISON_HULL_FRAC;
            }
            self.fleets.insert(id, garrison);
            if let Some(sys) = self.systems.iter_mut().find(|s| s.id == den) {
                sys.set_tier(crate::build::StructureKind::DefensePlatform, 0);
                sys.defense_pool = 0.0;
            }
            self.enclaves.insert(den, Enclave {
                system: den,
                tier: 1,
                plunder: crate::founding::DEN_PLUNDER.into_iter().collect(),
                // Never; finite, so the world still saves as JSON.
                next_launch_at: f64::MAX,
                next_grow_at: f64::MAX,
                dormant_until: 0.0,
                pack: Some(id),
                cleared: false,
                founder_den: true,
                for_player: None,
            });
        }
        self.players.get_mut(&owner).unwrap().founding.den = Some(den);
    }

    /// The privateer's log names its den. Observed where and when it died, so
    /// command learns of the den with the news of the kill.
    pub(super) fn reveal_founding_den(&mut self, owner: PlayerId, at: f64, pos: Vec2) {
        let Some(den) = self.players.get(&owner).and_then(|c| c.founding.den) else {
            return;
        };
        if self.enclaves.get(&den).is_none_or(|e| e.cleared) {
            return;
        }
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.intel.insert(den, IntelSnapshot { defense_tier: 0, shipyard_tier: 0, enclave_tier: 1,
                garrison_tier: 0, station_level: 0, observed_at: at, pos });
        }
    }

    /// The privateer's kill promotes the captain aboard the fleet that made it.
    pub(super) fn promote_privateer_hunter(&mut self, owner: PlayerId, fleet: EntityId) {
        let target = crate::captain::xp_for_level(crate::founding::PRIVATEER_PROMOTION_LEVEL);
        if let Some(captain) = self.players.get_mut(&owner).and_then(|corp| {
            corp.captains.values_mut().find(|c| c.assigned_fleet == Some(fleet) && !c.missing())
        }) {
            captain.grant_xp(target.saturating_sub(captain.xp));
        }
    }

    /// When the report that the den fell reaches command: its light from the
    /// den, whoever cleared it. A den found cleared without its event (a
    /// reload between the two) counts from now.
    pub(super) fn note_founding_den_cleared(&mut self, owner: PlayerId, events: &[Event]) {
        let Some(f) = self.players.get(&owner).map(|c| &c.founding) else {
            return;
        };
        let (Some(den), None) = (f.den, f.den_report_at) else {
            return;
        };
        let cc = self.players[&owner].command_center;
        let report = events.iter().find_map(|e| match &e.payload {
            EventPayload::PirateEnclaveCleared { system, pos, .. } if *system == den => {
                Some(e.time + crate::transit::delay(*pos, cc, self.config.c))
            }
            _ => None,
        }).or_else(|| (f.stage == crate::founding::FoundingStage::EliminateDen
            && self.enclaves.get(&den).is_none_or(|e| e.cleared)).then_some(self.time));
        if let Some(at) = report {
            self.players.get_mut(&owner).unwrap().founding.den_report_at = Some(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding::FoundingStage as S;

    fn founder() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        let owner = PlayerId(77);
        w.step(&[Command::AddPlayer { id: owner, name: "Hunter".into() }]);
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.enabled = true;
        f.local_opening = true;
        (w, owner)
    }

    #[test]
    fn every_home_has_a_den_system_holding_only_a_rich_belt_within_survey_reach() {
        let w = World::new(SimConfig::for_players(12648430, 4));
        for slot in &w.home_slots {
            let den = slot.den_system.expect("a den system per home");
            let sys = w.systems.iter().find(|s| s.id == den).unwrap();
            assert_eq!(sys.bodies.len(), 1, "only the belt");
            assert!(sys.bodies[0].is_belt() && !sys.bodies[0].deposits.is_empty(), "a belt with ore");
            // §den-pocket (Peter, Oct 30 2026): and hides what the den guarded.
            assert_eq!(sys.bodies[0].pockets.iter().map(|p| &p.deposit).collect::<Vec<_>>(),
                [&crate::founding::den_pocket()], "the den's Titanium pocket");
            assert!(sys.pos.distance(slot.pos) <= 22_001.0, "on the doorstep, a few minutes out");
            assert!(w.exploration_systems.contains(&den));
        }
    }

    /// §den (Oct 29 2026): the den's fights stand in its asteroid field, with
    /// no world in the arena; a home's keep theirs.
    #[test]
    fn a_dens_battle_has_no_world_to_fight_beside() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        w.step(&[]);
        let den = w.players[&owner].founding.den.unwrap();
        let garrison = w.fleets[&w.enclaves[&den].pack.unwrap()].pos;
        let star = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        assert_eq!(w.battle_world_side(garrison, owner, PlayerId::PIRATE), None, "on its belt");
        assert_eq!(w.battle_world_side(star, owner, PlayerId::PIRATE), None, "at its star");
        let home = w.players[&owner].command_center;
        assert_eq!(w.battle_world_side(home, PlayerId::PIRATE, owner), Some(1), "a home fights beside its world");
    }

    #[test]
    fn the_den_rises_with_the_privateer_and_holds_without_raiding() {
        let (mut w, owner) = founder();
        let den = w.founding_den_system(owner).expect("a den system");
        assert!(!w.enclaves.contains_key(&den), "no den before the pirates come");
        w.ensure_founding_den(owner);
        w.ensure_founding_den(owner);
        assert_eq!(w.players[&owner].founding.den, Some(den));
        let e = &w.enclaves[&den];
        assert!(e.founder_den && e.permanent() && !e.cleared);
        let garrison = &w.fleets[&e.pack.unwrap()];
        assert!(garrison.founding_privateer);
        assert_eq!(garrison.ships.len(), crate::founding::DEN_GARRISON_RAIDERS as usize);
        assert_eq!(w.systems.iter().find(|s| s.id == den).unwrap().tier(crate::build::StructureKind::DefensePlatform), 0);
        // §real-coordinates: it takes its place on the den's belt, then holds.
        w.step(&[]);
        let garrison = &w.fleets[&w.enclaves[&den].pack.unwrap()];
        let sys = w.systems.iter().find(|s| s.id == den).unwrap();
        let ring = sys.bodies[0].belt.unwrap().radius * crate::body::SYSTEM_UNIT_SU;
        assert!((garrison.pos.distance(sys.pos) - ring).abs() < 1.0, "on the belt, not at the star");
        let pos = garrison.pos;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let garrison = &w.fleets[&w.enclaves[&den].pack.unwrap()];
        assert!(garrison.pos.distance(pos) < 1.0, "the garrison holds its den");
        assert_eq!(w.enclaves[&den].tier, 1, "a den never grows");
    }

    #[test]
    fn a_lone_interceptor_clears_the_den_for_good_and_its_report_completes_the_step() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::EliminateDen, w.time);
        let pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Raider, pos, FleetOrder::Idle, None));
        let mut cleared_at = None;
        for _ in 0..(240 * crate::config::TICK_HZ) {
            let ev = w.step(&[]);
            if cleared_at.is_none() && ev.iter().any(|e| matches!(&e.payload,
                EventPayload::PirateEnclaveCleared { system, owner: o, .. } if *system == den && *o == owner)) {
                cleared_at = Some(w.time);
            }
            if w.players[&owner].founding.stage != S::EliminateDen {
                break;
            }
        }
        assert!(cleared_at.is_some(), "one Interceptor destroys the den's garrison");
        assert!(w.fleets.contains_key(&id), "and survives it");
        assert!(w.enclaves[&den].cleared, "the den stays cleared");
        assert_eq!(w.players[&owner].founding.stage, S::PrepareExport, "the report completes the step");
        w.time += crate::pirate::PIRATE_DORMANCY * 3.0;
        w.step(&[]);
        assert!(w.enclaves[&den].cleared && w.enclaves[&den].pack.is_none(), "no respawn");
    }

    #[test]
    fn the_privateers_log_names_the_den_when_its_kill_report_arrives() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let cc = w.players[&owner].command_center;
        let kill = cc + Vec2::new(20_000.0, 0.0);
        w.reveal_founding_den(owner, w.time, kill);
        let snap = w.players[&owner].intel[&den];
        assert_eq!(snap.enclave_tier, 1);
        assert_eq!(snap.pos, kill, "observed where the privateer died: it reaches command with the kill report");
    }

    #[test]
    fn a_home_without_a_den_system_skips_the_den() {
        let (mut w, owner) = founder();
        for slot in &mut w.home_slots {
            slot.den_system = None;
        }
        w.ensure_founding_den(owner);
        assert_eq!(w.players[&owner].founding.den, None);
    }
}
