//! Local batteries support either side of a nearby battle. Selection is fixed
//! before recording opens; old in-progress/saved battles keep their old roster.

use super::*;

impl World {
    pub(super) fn battle_escort_assignments(&self, attackers: &[EntityId], defenders: &[EntityId]) -> BTreeMap<EntityId, EntityId> {
        attackers.iter().chain(defenders).filter_map(|id| {
            let fleet = self.fleets.get(id)?;
            if !fleet.ships.iter().any(|ship| ship.loadout.has_datalink()) { return None; }
            let charge = match fleet.order {
                FleetOrder::Guard { target } => Some(target),
                _ => fleet.defense.as_ref().and_then(|sortie| sortie.guard),
            }?;
            // Store only the assignment, never an out-of-battle position. The
            // tactical layer also requires a living same-side hull in range.
            self.fleets.get(&charge).filter(|other| other.owner == fleet.owner || self.contract_guard_authorized(fleet.owner, charge)).map(|_| (*id, charge))
        }).collect()
    }

    pub(super) fn platform_state(&self, site: Option<EntityId>) -> (u32, f64) {
        site.and_then(|id| self.systems.iter().find(|s| s.id == id))
            .map(|s| (s.tier_sum(crate::build::StructureKind::DefensePlatform), s.defense_pool))
            .unwrap_or((0, 0.0))
    }

    pub(super) fn prepare_battle_support(&mut self) {
        let ids: Vec<_> = self.engagements.keys().copied().collect();
        for id in ids {
            let e = &self.engagements[&id];
            if e.tactical.is_some() || self.battle_records.contains_key(&id) { continue; }
            let (pos, owners) = (e.pos, [e.a_owner, e.d_owner]);
            for (side, owner) in owners.into_iter().enumerate() {
                let e = &self.engagements[&id];
                let assigned = if side == 0 { e.attacker_platform_system } else { e.platform_system };
                if assigned.is_some() { continue; }
                let Some(site) = self.covering_platform(owner, pos) else { continue; };
                let (tiers, _) = self.platform_state(Some(site));
                let e = self.engagements.get_mut(&id).unwrap();
                e.raid = false;
                if side == 0 {
                    e.attacker_platform_system = Some(site);
                    e.attacker_platform_start_tiers = tiers;
                    e.a_start_strength = crate::combat::Forces::from_fleet(&e.a_start, &BTreeMap::new())
                        .with_platform(tiers, 0.0).strength();
                } else {
                    e.platform_system = Some(site);
                    e.platform_start_tiers = tiers;
                    e.d_start_strength = crate::combat::Forces::from_fleet(&e.d_start, &BTreeMap::new())
                        .with_platform(tiers, 0.0).strength();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind::DefensePlatform;

    fn scene() -> (World, PlayerId, PlayerId, EntityId, Vec2) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let (p, rival) = (PlayerId(95101), PlayerId(95102));
        w.step(&[Command::AddPlayer { id: p, name: "Battery".into() },
            Command::AddPlayer { id: rival, name: "Rival".into() }]);
        w.enclaves.clear();
        w.fleets.clear();
        let id = w.players[&p].home_system.unwrap();
        w.systems.retain(|s| s.id == id);
        w.systems[0].set_tier(DefensePlatform, 2);
        w.systems[0].defense_pool = 0.0;
        let pos = w.systems[0].pos;
        (w, p, rival, id, pos)
    }

    fn fleet(w: &mut World, p: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, p, kind, pos, FleetOrder::Idle, None));
        id
    }

    #[test]
    fn battle_recorder_receives_the_guard_charge_during_its_defensive_sortie() {
        let (mut w, owner, rival, _, pos) = scene();
        let charge = fleet(&mut w, owner, ShipKind::Convoy, pos);
        let escort = fleet(&mut w, owner, ShipKind::Corvette, pos);
        let enemy = fleet(&mut w, rival, ShipKind::Raider, pos);
        let guard = w.fleets.get_mut(&escort).unwrap();
        guard.set_fitted(ShipKind::Corvette, &crate::module::Loadout::from_key("point_defense_screen+escort_datalink"), 1);
        guard.order = FleetOrder::Intercept { target: enemy };
        guard.defense = Some(DefenseEngagement { target: enemy, guard: Some(charge), patrol: vec![], system: None });
        w.fleets.get_mut(&enemy).unwrap().order = FleetOrder::Attack { target: charge };
        let guards = BTreeMap::from([(escort, charge)]);
        assert_eq!(w.battle_escort_assignments(&[enemy], &[charge, escort]), guards);
        w.resolve_raids(&mut vec![]);
        let (&id, battle) = w.engagements.iter().find(|(_, e)| e.defenders.contains(&escort)).unwrap();
        let mut state = battle.tactical.clone().unwrap();
        assert!(!state.set_escorts(guards), "actual tactical state already received the named charge");
        assert_eq!(w.battle_records[&id].tactical_replay().unwrap().reconstruct().unwrap(), state);
        let guard = w.fleets.get_mut(&escort).unwrap();
        guard.order = FleetOrder::Idle;
        guard.defense = None;
        assert!(w.battle_escort_assignments(&[enemy], &[charge, escort]).is_empty());
    }

    #[test]
    fn platforms_support_armed_fleets_on_either_side_and_replay_exactly() {
        for friendly_attacks in [false, true] {
            let (mut w, p, rival, system, pos) = scene();
            let own = fleet(&mut w, p, ShipKind::Raider, pos);
            let enemy = fleet(&mut w, rival, ShipKind::Raider, pos);
            let (a, d) = if friendly_attacks { (own, enemy) } else { (enemy, own) };
            w.fleets.get_mut(&a).unwrap().order = FleetOrder::Attack { target: d };
            w.resolve_raids(&mut Vec::new());
            let (&id, e) = w.engagements.iter().next().unwrap();
            let side = if friendly_attacks { 0 } else { 1 };
            assert_eq!([e.attacker_platform_system, e.platform_system][side], Some(system));
            assert_eq!(w.battle_records[&id].sides[side].platform_tiers, 2);
            let tac = e.tactical.as_ref().unwrap();
            assert_eq!(tac.platform_tiers_for(side as u8), 2);
            assert_eq!(tac.platform_tiers_for(1 - side as u8), 0);
            let replayed = w.battle_records[&id].tactical_replay().unwrap().reconstruct().unwrap();
            assert_eq!(serde_json::to_value(&replayed).unwrap(), serde_json::to_value(tac).unwrap());
            let saved: World = serde_json::from_slice(&serde_json::to_vec(&w).unwrap()).unwrap();
            assert_eq!(saved.platform_state(Some(system)), w.platform_state(Some(system)));
            assert_eq!(saved.engagements[&id].attacker_platform_system, e.attacker_platform_system);
        }
    }

    #[test]
    fn platforms_stack_with_escorts_but_not_with_a_second_battle_or_outside_coverage() {
        let (mut w, p, rival, system, pos) = scene();
        let cargo = fleet(&mut w, p, ShipKind::Convoy, pos);
        let escort = fleet(&mut w, p, ShipKind::Corvette, pos);
        let enemy = fleet(&mut w, rival, ShipKind::Raider, pos);
        w.fleets.get_mut(&enemy).unwrap().order = FleetOrder::Attack { target: cargo };
        w.resolve_raids(&mut Vec::new());
        let first = w.engagements.values().next().unwrap();
        assert!(first.defenders.contains(&escort));
        assert_eq!(first.platform_system, Some(system));
        assert_eq!(w.covering_platform(p, pos), None, "battery already physically committed");

        // Well outside the join radius: a distinct simultaneous engagement.
        let elsewhere = pos + Vec2::new(800.0, 0.0);
        let own = fleet(&mut w, p, ShipKind::Raider, elsewhere);
        let foe = fleet(&mut w, rival, ShipKind::Raider, elsewhere);
        w.fleets.get_mut(&own).unwrap().order = FleetOrder::Attack { target: foe };
        w.resolve_raids(&mut Vec::new());
        assert_eq!(w.engagements.len(), 2);
        assert_eq!(w.engagements.values().filter(|e| e.platform_system == Some(system)
            || e.attacker_platform_system == Some(system)).count(), 1);
        w.engagements.clear();
        assert_eq!(w.covering_platform(p, pos), Some(system));
        assert_eq!(w.covering_platform(p, pos + Vec2::new(crate::build::DEFENSE_PLATFORM_RADIUS + 0.1, 0.0)), None);
    }

    #[test]
    fn platform_damage_is_written_to_its_own_side_not_the_enemy_system() {
        let (mut w, p, rival, system, pos) = scene();
        let own = fleet(&mut w, p, ShipKind::Raider, pos);
        let foe = fleet(&mut w, rival, ShipKind::Raider, pos);
        w.fleets.get_mut(&own).unwrap().order = FleetOrder::Attack { target: foe };
        w.resolve_raids(&mut Vec::new());
        let id = *w.engagements.keys().next().unwrap();
        let tac = w.engagements.get_mut(&id).unwrap().tactical.as_mut().unwrap();
        for c in tac.combatants.iter_mut().filter(|c| c.platform) { c.hp -= 7.0; }
        // No tactical step at tick 1: inspect the write-back itself in isolation.
        w.tick = 1;
        w.resolve_raids(&mut Vec::new());
        assert!((w.platform_state(Some(system)).1 - 14.0).abs() < 1e-8);
        assert_eq!(w.platform_state(Some(system)).0, 2);
    }
}
