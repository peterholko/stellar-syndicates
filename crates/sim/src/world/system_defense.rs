//! Standing system defense is onboard automation, not command-center truth.
//! Only the delayed command installs the post. Subsequent reactions use local
//! light and ordinary flight/combat, online or offline; the server view samples
//! the assignment with the very same emission frame as the fleet's position.

use super::*;

impl World {
    pub(super) fn autonomous_system_defense(&mut self) {
        let engaged: std::collections::BTreeSet<_> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(&e.defenders).copied()).collect();
        let mut changes = Vec::new();
        for (&id, fleet) in &self.fleets {
            let Some(post) = fleet.system_defense() else { continue; };
            if engaged.contains(&id) { continue; } // tactical retreat owns a running battle
            // Verify the destination locally, not from an unseen remote capture.
            if fleet.pos.distance(post.station) <= crate::ship::DOCK_RADIUS
                && self.systems.iter().find(|s| s.id == post.system).is_none_or(|s| s.owner != Some(fleet.owner))
            {
                changes.push((id, FleetOrder::Idle, None));
                continue;
            }
            let bubble = self.config.sensor_range * fleet.sensor_mult()
                * self.research_mod(fleet.owner, crate::research::ModKey::SensorRange)
                * self.nebula_sensor_factor(fleet.pos);
            let mut sources = vec![(fleet.pos, bubble)];
            // Heavier formations have no mobile sensor unless an Interceptor
            // accompanies them. They may use the defended station's actual CC
            // or array, not a newly invented full-range sensor on every hull.
            if let Some(corp) = self.players.get(&fleet.owner)
                && corp.command_center.distance(post.station) <= crate::ship::DOCK_RADIUS
            { sources.push((corp.command_center, self.config.sensor_range * self.nebula_sensor_factor(corp.command_center))); }
            sources.extend(self.array_sensor_sources(fleet.owner).into_iter()
                .filter(|(pos, _)| pos.distance(post.station) <= crate::ship::DOCK_RADIUS)
                .map(|(pos, range)| (pos, range * self.nebula_sensor_factor(pos))));
            let visible = |other: &Fleet| {
                sources.iter().filter_map(|&(source, range)| {
                    // Target→sensor AND sensor→defender, never instantaneous
                    // corporate sensor sharing. Prefer the freshest local fix.
                    let age = crate::transit::delay(source, other.pos, self.config.c)
                        + crate::transit::delay(source, fleet.pos, self.config.c);
                    let seen = other.pos - other.vel * age;
                    let signature = other.signature() * self.veil_factor(other.owner, seen)
                        * self.nebula_signature_factor(seen);
                    let detected = if other.broadcasts() { source.distance(seen) <= range }
                        else { crate::detection::detected(signature, &[(source, range)], seen) };
                    (detected && seen.distance(post.station) <= post.radius).then_some((age, seen))
                }).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, seen)| seen)
            };
            let hostile = |other: &Fleet| {
                other.owner != fleet.owner && other.is_combatant() && !other.owner.is_tca()
                    && !self.are_allied(fleet.owner, other.owner)
                    && !self.diplomacy_protects(fleet.owner, other.owner)
                    && (other.owner.is_sentinel() || (!self.founder_protected(fleet.owner) && !self.founder_protected(other.owner)))
                    && !self.in_sovereign_zone(other.pos)
            };
            let (mut friendly, mut enemy) = (fleet.combat_weight(), 0.0);
            for (&other_id, other) in &self.fleets {
                if other_id == id || !other.is_combatant() || visible(other).is_none() { continue; }
                if other.owner == fleet.owner || self.are_allied(fleet.owner, other.owner) {
                    friendly += other.combat_weight();
                } else if hostile(other) { enemy += other.combat_weight(); }
            }
            let retreat = self.players.get(&fleet.owner).and_then(|c| c.doctrine.retreat.min_ratio())
                .is_some_and(|min| friendly / (friendly + enemy).max(1e-9) < min);
            // Begin the physical drop/turn before the leash, allowing for the
            // remaining locked-course coast. The limit is ALWAYS about the
            // system: quarry movement cannot ratchet a guard across the galaxy.
            let coast = fleet.pos + fleet.vel * (crate::transit::drop_seconds(crate::transit::Regime::Warp) + DT * 2.0);
            let outside = fleet.pos.distance(post.station) >= post.radius
                || coast.distance(post.station) >= post.radius;
            if let Some(sortie) = &fleet.defense {
                let keep = !outside && !retreat && self.fleets.get(&sortie.target)
                    .is_some_and(|other| hostile(other) && visible(other).is_some());
                if !keep { changes.push((id, post.order(), None)); }
                continue;
            }
            // Complete the return before taking another sortie. Otherwise a
            // quarry just inside the leash repeatedly pulls a returning guard
            // back out, recreating a drop/spool sawtooth at the perimeter.
            if outside || retreat || !fleet.is_combatant()
                || fleet.pos.distance(post.station) > 1e-6 { continue; }
            // Remain berthed for normal refuelling/resupply instead of installing
            // a dry Intercept at the dock, which would make the berth ineligible.
            if !fleet.supplied || fleet.fuel < crate::fuel::fuel_tick(fleet.mass(), fleet.transit_speed(), DT) { continue; }
            let target = self.fleets.iter().filter_map(|(&target, other)| {
                if target == id || !hostile(other) { return None; }
                visible(other).map(|seen| (target, seen.distance(post.station)))
            }).min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            if let Some((target, _)) = target {
                // Explicit Defend is permission to protect this area, not to
                // raid passing freighters. Keep existing doctrine retreat odds.
                changes.push((id, FleetOrder::Intercept { target }, Some(DefenseEngagement {
                    target, patrol: Vec::new(), guard: None, system: Some(post),
                })));
            }
        }
        for (id, order, defense) in changes {
            if let Some(fleet) = self.fleets.get_mut(&id) {
                fleet.order = order;
                fleet.defense = defense;
                fleet.pursuit_plan = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::{SystemDefense, SYSTEM_DEFENSE_RADIUS};

    fn scene() -> (World, PlayerId, SystemDefense) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let owner = PlayerId(97001);
        w.step(&[Command::AddPlayer { id: owner, name: "System defense".into() }]);
        let system = w.players[&owner].home_system.unwrap();
        let station = w.players[&owner].home;
        w.fleets.clear();
        w.enclaves.clear();
        w.players.get_mut(&owner).unwrap().founding.enabled = false;
        let post = SystemDefense { system, station, radius: SYSTEM_DEFENSE_RADIUS };
        (w, owner, post)
    }

    fn ship(w: &mut World, owner: PlayerId, pos: Vec2, kind: ShipKind, n: u32, order: FleetOrder) -> EntityId {
        let id = w.alloc_entity_id();
        let mut fleet = Fleet::single(id, owner, kind, pos, order, None);
        fleet.reset_to(kind, n);
        fleet.fuel = fleet.fuel_capacity();
        w.fleets.insert(id, fleet);
        id
    }

    #[test]
    fn system_defense_is_a_delayed_order_and_manual_reassignment_cancels_it_at_delivery() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station + Vec2::new(10_000.0, 0.0), ShipKind::Corvette, 1, FleetOrder::Idle);
        let command = Command::DefendSystem { player_id: owner, fleet_id: id, system_id: post.system, pursuit_radius: post.radius };
        let mut events = Vec::new();
        w.apply(&command, &mut events);
        let arrival = w.pending_orders.last().unwrap().apply_time;
        assert!(arrival > w.time + 1.0);
        assert_eq!(w.pending_orders.last().unwrap().kind, crate::event::OrderKind::Defend);
        assert!(matches!(events.last().unwrap().payload, EventPayload::OrderScheduled { fleet, .. } if fleet == id));
        while w.time + DT < arrival {
            w.step(&[]);
            assert!(w.fleets[&id].system_defense().is_none(), "click is not compliance");
        }
        for _ in 0..3 { w.step(&[]); }
        assert_eq!(w.fleets[&id].system_defense(), Some(post));
        w.apply(&Command::HoldFleet { player_id: owner, ship_id: id }, &mut events);
        assert_eq!(w.fleets[&id].system_defense(), Some(post), "release also waits for light");
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        assert!(w.fleets[&id].system_defense().is_none());
        assert!(matches!(w.fleets[&id].order, FleetOrder::Idle));
    }

    #[test]
    fn system_defenders_ignore_civilians_and_distant_contacts_and_obey_the_fixed_leash() {
        for kind in [ShipKind::Raider, ShipKind::Corvette, ShipKind::Destroyer, ShipKind::Cruiser] {
            let (mut w, owner, post) = scene();
            let id = ship(&mut w, owner, post.station, kind, 1, post.order());
            ship(&mut w, PlayerId::TCA, post.station, ShipKind::Freighter, 1, FleetOrder::Idle);
            ship(&mut w, PlayerId::PIRATE, post.station, ShipKind::Convoy, 1, FleetOrder::Idle);
            let enemy = ship(&mut w, PlayerId::PIRATE, post.station + Vec2::new(post.radius + 100.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
            w.autonomous_system_defense();
            assert!(matches!(w.fleets[&id].order, FleetOrder::DefendSystem { .. }));
            w.fleets.get_mut(&enemy).unwrap().pos = post.station + Vec2::new(4_000.0, 0.0);
            w.autonomous_system_defense();
            assert!(matches!(w.fleets[&id].order, FleetOrder::Intercept { target } if target == enemy), "all combat hull classes can defend");
            let departure = post.station + Vec2::new(2_000.0, 0.0);
            w.fleets.get_mut(&id).unwrap().pos = departure;
            w.fleets.get_mut(&enemy).unwrap().pos = post.station + Vec2::new(post.radius + 1.0, 0.0);
            w.autonomous_system_defense();
            assert!(matches!(w.fleets[&id].order, FleetOrder::DefendSystem { .. }));
            assert_eq!(w.fleets[&id].pos, departure, "break-off orders flight, not a teleport");
            w.fleets.get_mut(&enemy).unwrap().pos = post.station + Vec2::new(4_000.0, 0.0);
            w.autonomous_system_defense();
            assert!(matches!(w.fleets[&id].order, FleetOrder::DefendSystem { .. }), "finish returning before another sortie");
        }
    }

    #[test]
    fn system_defenders_return_from_battle_physically_and_keep_their_post_across_a_save() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station, ShipKind::Destroyer, 4, post.order());
        let enemy = ship(&mut w, PlayerId::PIRATE, post.station + Vec2::new(4_000.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let mut battle_pos = None;
        let mut ended = false;
        for _ in 0..(240 * crate::config::TICK_HZ) {
            w.step(&[]);
            if let Some(e) = w.engagements.values().find(|e| e.attackers.contains(&id) || e.defenders.contains(&id)) {
                battle_pos = Some(e.pos);
            }
            if battle_pos.is_some() && w.engagements.is_empty() { ended = true; break; }
        }
        assert!(ended && !w.fleets.contains_key(&enemy), "normal tactical combat must resolve");
        let pos = battle_pos.unwrap();
        assert!(pos.distance(post.station) > 500.0);
        assert!(w.fleets[&id].pos.distance(pos) < 1e-6, "battle ends at the battle marker");
        assert_eq!(w.fleets[&id].system_defense(), Some(post));
        let serialized = serde_json::to_vec(&w).unwrap();
        let mut w: World = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(w.fleets[&id].system_defense(), Some(post));
        for _ in 0..(240 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.dock_of(id) == Some(DockSite::System(post.system)) { break; }
        }
        assert_eq!(w.dock_of(id), Some(DockSite::System(post.system)));
        assert_eq!(w.fleets[&id].system_defense(), Some(post));
        let fuel = w.fleets[&id].fuel;
        for _ in 0..30 { w.integrate_movement(&mut Vec::new()); }
        assert_eq!(w.fleets[&id].fuel, fuel, "holding a post does not burn cruise fuel");
    }

    #[test]
    fn system_defense_rejects_unarmed_foreign_and_unbounded_assignments() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station, ShipKind::Convoy, 1, FleetOrder::Idle);
        w.apply(&Command::DefendSystem { player_id: owner, fleet_id: id, system_id: post.system, pursuit_radius: post.radius }, &mut Vec::new());
        assert!(w.pending_orders.is_empty());
        w.fleets.get_mut(&id).unwrap().reset_to(ShipKind::Raider, 1);
        for radius in [f64::NAN, f64::INFINITY, -1.0, 1.0, 50_000.0] {
            w.apply(&Command::DefendSystem { player_id: owner, fleet_id: id, system_id: post.system, pursuit_radius: radius }, &mut Vec::new());
        }
        w.apply(&Command::DefendSystem { player_id: PlayerId(9), fleet_id: id, system_id: post.system, pursuit_radius: post.radius }, &mut Vec::new());
        assert!(w.pending_orders.is_empty());
    }

    #[test]
    fn a_fleeing_target_cannot_drag_a_warping_defender_past_its_system_leash() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station + Vec2::new(8_500.0, 0.0), ShipKind::Raider, 1, post.order());
        let enemy = ship(&mut w, PlayerId::PIRATE, post.station + Vec2::new(9_700.0, 0.0), ShipKind::Raider, 1,
            FleetOrder::MoveTo { dest: post.station + Vec2::new(60_000.0, 0.0) });
        for fleet_id in [id, enemy] {
            let f = w.fleets.get_mut(&fleet_id).unwrap();
            f.vel = Vec2::new(f.transit_speed() * crate::transit::WARP_FACTOR, 0.0);
            f.drive_state = crate::ship::DriveState::Cruising(crate::transit::Regime::Warp);
        }
        let f = w.fleets.get_mut(&id).unwrap();
        f.order = FleetOrder::Intercept { target: enemy };
        f.defense = Some(DefenseEngagement { target: enemy, guard: None, patrol: vec![], system: Some(post) });
        let mut maximum = 0.0f64;
        let mut broke_off = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.autonomous_system_defense();
            broke_off |= matches!(w.fleets[&id].order, FleetOrder::DefendSystem { .. });
            w.integrate_movement(&mut Vec::new());
            w.time += DT;
            w.tick += 1;
            maximum = maximum.max(w.fleets[&id].pos.distance(post.station));
        }
        assert!(broke_off);
        assert!(maximum <= post.radius + 1e-6, "maximum excursion {maximum} exceeds {}", post.radius);
        assert!(w.fleets[&id].pos.distance(post.station) < 1e-6);
        assert_eq!(w.fleets[&id].system_defense(), Some(post));
    }

    #[test]
    fn changing_a_system_defender_to_a_move_clears_the_saved_sortie_only_on_delivery() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station + Vec2::new(4_000.0, 0.0), ShipKind::Raider, 1, post.order());
        let enemy = ship(&mut w, PlayerId::PIRATE, post.station + Vec2::new(9_000.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let f = w.fleets.get_mut(&id).unwrap();
        f.order = FleetOrder::Intercept { target: enemy };
        f.defense = Some(DefenseEngagement { target: enemy, guard: None, patrol: vec![], system: Some(post) });
        let dest = post.station + Vec2::new(-4_000.0, 0.0);
        w.apply(&Command::MoveShip { player_id: owner, ship_id: id, dest }, &mut Vec::new());
        assert!(w.fleets[&id].defense.is_some());
        w.time = w.pending_orders.last().unwrap().apply_time;
        w.deliver_due_orders(&mut Vec::new());
        assert!(matches!(w.fleets[&id].order, FleetOrder::MoveTo { dest: d } if d == dest));
        assert!(w.fleets[&id].defense.is_none());
        w.autonomous_system_defense();
        assert!(w.fleets[&id].system_defense().is_none());
    }

    #[test]
    fn dry_system_defenders_keep_their_berth_and_resume_after_refuelling() {
        let (mut w, owner, post) = scene();
        let id = ship(&mut w, owner, post.station, ShipKind::Raider, 1, post.order());
        let enemy = ship(&mut w, PlayerId::PIRATE, post.station + Vec2::new(4_000.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.get_mut(&id).unwrap().fuel = 0.0;
        w.autonomous_system_defense();
        assert_eq!(w.dock_of(id), Some(DockSite::System(post.system)));
        w.fleets.get_mut(&id).unwrap().refuel(10.0);
        w.autonomous_system_defense();
        assert!(matches!(w.fleets[&id].order, FleetOrder::Intercept { target } if target == enemy));
    }
}
