//! §engagement-rules (Peter, Oct 28 2026: "I sent my interceptor to hunt the
//! pirate and why didn't they engage into a fight once they got close
//! together, I shouldn't have to send an attack order, both should have
//! engagement rules"). A corporation's warships and a loose pirate fight when
//! they meet. Within [`CLOSE_ENGAGE_SU`] of each other:
//! - a loose pirate warship attacks the nearest corporation warship, unless
//!   one is already coming for it;
//! - an idle or travelling corporation warship attacks the nearest loose
//!   pirate warship its corporation senses (the same `detection::detected` the
//!   View uses), unless the corporation's doctrine is Avoid.
//!
//! A loose pirate sits idle outside any script: not a home raid's pack, not
//! the vendetta's, and a founding privateer only once it lurks after its raid.
//! Scripted runs, raids and withdrawals keep their own rules, as do fleets
//! already fighting or under any other order (guard, defend, patrol,
//! blockade), so a broken-off fight is never reopened here. The ordinary
//! contact resolution (`resolve_raids`) then opens one battle for the pair.
//! Fights between corporations still need an order or a doctrine.
use super::*;
use crate::doctrine::EngagementPolicy;

/// How close a corporation's warship and a pirate's come before they fight
/// unordered. Tunable.
pub const CLOSE_ENGAGE_SU: f64 = 2_000.0;

/// A warship that may be drawn into a close-contact fight.
struct Armed {
    id: EntityId,
    owner: PlayerId,
    pos: Vec2,
    signature: f64,
    idle: bool,
    travelling: bool,
}

impl World {
    pub(super) fn close_contact_engagements(&mut self) {
        let engaged: std::collections::BTreeSet<EntityId> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let held = self.node_holdings();
        // Founding privateers still on their raid (inbound, looting or homeward).
        let raiding: std::collections::BTreeSet<EntityId> = self.players.values()
            .filter(|c| c.founding.raid.as_ref().is_some_and(|r| r.phase != crate::founding::RaidPhase::Lurking))
            .filter_map(|c| c.founding.privateer)
            .collect();
        let scripted = |id: &EntityId, f: &crate::ship::Fleet| f.owner.is_pirate()
            && (f.vendetta_pirate || self.pirate_home_raids.contains_key(id) || raiding.contains(id));
        // Pirates a corporation fleet is already ordered against.
        let hunted: std::collections::BTreeSet<EntityId> = self.fleets.values()
            .filter(|f| !f.owner.is_pirate())
            .filter_map(|f| match f.order { FleetOrder::Intercept { target } | FleetOrder::Attack { target } => Some(target), _ => None })
            .collect();
        let armed: Vec<Armed> = self.fleets.iter()
            .filter(|(id, f)| f.is_combatant() && !engaged.contains(id) && !f.owner.is_tca()
                && !scripted(id, f) && !self.in_sovereign_zone(f.pos))
            .map(|(id, f)| Armed {
                id: *id,
                owner: f.owner,
                pos: f.pos,
                signature: f.detection_signature() * self.veil_factor_held(&held, f.owner, f.pos)
                    * self.nebula_signature_factor(f.pos),
                idle: matches!(f.order, FleetOrder::Idle),
                travelling: matches!(f.order, FleetOrder::MoveTo { .. }),
            })
            .collect();
        if !armed.iter().any(|a| a.owner.is_pirate()) || !armed.iter().any(|a| !a.owner.is_pirate()) {
            return;
        }
        // The nearest qualifying warship within reach; ties by id (deterministic).
        let nearest = |from: &Armed, pick: &dyn Fn(&Armed) -> bool| armed.iter()
            .filter(|a| pick(a) && a.pos.distance(from.pos) <= CLOSE_ENGAGE_SU)
            .min_by(|a, b| a.pos.distance(from.pos).total_cmp(&b.pos.distance(from.pos)).then(a.id.cmp(&b.id)))
            .map(|a| a.id);
        let mut orders: Vec<(EntityId, EntityId)> = Vec::new();
        // A loose pirate turns on the nearest corporation warship.
        for pirate in armed.iter().filter(|a| a.owner.is_pirate() && a.idle && !hunted.contains(&a.id)) {
            if let Some(target) = nearest(pirate, &|a| !a.owner.is_pirate()) {
                orders.push((pirate.id, target));
            }
        }
        // A corporation's warship: on a pirate it senses, its doctrine permitting.
        let mut coverage: BTreeMap<PlayerId, Vec<(Vec2, f64)>> = BTreeMap::new();
        for fleet in armed.iter().filter(|a| !a.owner.is_pirate() && (a.idle || a.travelling)) {
            if self.players.get(&fleet.owner).is_none_or(|c| c.doctrine.engagement == EngagementPolicy::Avoid) {
                continue;
            }
            let sources = coverage.entry(fleet.owner).or_insert_with(|| self.close_contact_coverage(fleet.owner));
            let sources: &[(Vec2, f64)] = sources;
            if let Some(target) = nearest(fleet, &|a| a.owner.is_pirate() && a.idle
                && crate::detection::detected(a.signature, sources, a.pos)) {
                orders.push((fleet.id, target));
            }
        }
        for (id, target) in orders {
            if let Some(fleet) = self.fleets.get_mut(&id) {
                fleet.order = FleetOrder::Attack { target };
            }
        }
    }

    /// A corporation's detection coverage now: its command center, its own
    /// sensor-carrying fleets and its Sensor Arrays, as the View counts them.
    fn close_contact_coverage(&self, owner: PlayerId) -> Vec<(Vec2, f64)> {
        let range = self.config.sensor_range * self.research_mod(owner, crate::research::ModKey::SensorRange);
        let cc = self.players[&owner].command_center;
        let mut sources = vec![(cc, range * self.nebula_sensor_factor(cc))];
        sources.extend(self.fleets.values().filter(|f| f.owner == owner && f.projects_sensor())
            .map(|f| (f.pos, range * f.sensor_mult() * self.nebula_sensor_factor(f.pos))));
        sources.extend(self.array_sensor_sources(owner).into_iter()
            .map(|(pos, radius)| (pos, radius * self.nebula_sensor_factor(pos))));
        sources
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::Fleet;

    /// A corporation's Interceptor and a pirate Raider `gap` su apart, in
    /// open space well clear of the Market Hub, both idle.
    fn meeting(gap: f64) -> (World, PlayerId, EntityId, EntityId) {
        let mut w = World::new(SimConfig::for_players(4242, 4));
        let owner = PlayerId(91_001);
        w.step(&[Command::AddPlayer { id: owner, name: "Hunters".into() }]);
        let at = w.players[&owner].command_center + Vec2::new(9_000.0, 0.0);
        let ours = w.alloc_entity_id();
        w.fleets.insert(ours, Fleet::single(ours, owner, ShipKind::Raider, at, FleetOrder::Idle, None));
        let theirs = w.alloc_entity_id();
        w.fleets.insert(theirs, Fleet::single(theirs, PlayerId::PIRATE, ShipKind::Raider,
            at + Vec2::new(gap, 0.0), FleetOrder::Idle, None));
        (w, owner, ours, theirs)
    }

    fn fighting(w: &World, a: EntityId, b: EntityId) -> bool {
        w.engagements.values().any(|e| {
            let all: Vec<_> = e.attackers.iter().chain(e.defenders.iter()).collect();
            all.contains(&&a) && all.contains(&&b)
        })
    }

    fn run(w: &mut World, secs: u32) {
        for _ in 0..secs * crate::config::TICK_HZ {
            w.step(&[]);
        }
    }

    #[test]
    fn a_hunter_and_a_lurking_pirate_fight_when_they_meet() {
        let (mut w, _, ours, theirs) = meeting(1_200.0);
        w.step(&[]);
        assert!(matches!(w.fleets[&ours].order, FleetOrder::Attack { target } if target == theirs), "ours commits");
        assert!(matches!(w.fleets[&theirs].order, FleetOrder::Attack { target } if target == ours), "and so does the pirate");
        run(&mut w, 30);
        assert!(fighting(&w, ours, theirs) || !w.fleets.contains_key(&ours) || !w.fleets.contains_key(&theirs),
            "one battle opens between them");
    }

    #[test]
    fn farther_apart_nobody_moves() {
        let (mut w, _, ours, theirs) = meeting(CLOSE_ENGAGE_SU + 500.0);
        run(&mut w, 2);
        assert!(matches!(w.fleets[&ours].order, FleetOrder::Idle));
        assert!(matches!(w.fleets[&theirs].order, FleetOrder::Idle));
    }

    #[test]
    fn an_avoid_doctrine_holds_fire_but_the_pirate_still_comes() {
        let (mut w, owner, ours, theirs) = meeting(1_200.0);
        w.players.get_mut(&owner).unwrap().doctrine.engagement = EngagementPolicy::Avoid;
        w.step(&[]);
        assert!(matches!(w.fleets[&ours].order, FleetOrder::Idle), "Avoid never starts a fight");
        assert!(matches!(w.fleets[&theirs].order, FleetOrder::Attack { target } if target == ours));
    }

    #[test]
    fn a_scripted_or_hunted_pirate_is_left_to_its_own_rules() {
        let (mut w, _, ours, theirs) = meeting(1_200.0);
        w.fleets.get_mut(&theirs).unwrap().vendetta_pirate = true;
        w.step(&[]);
        assert!(matches!(w.fleets[&ours].order, FleetOrder::Idle), "the vendetta's pack keeps its script");
        assert!(matches!(w.fleets[&theirs].order, FleetOrder::Idle));
        w.fleets.get_mut(&theirs).unwrap().vendetta_pirate = false;
        w.fleets.get_mut(&ours).unwrap().order = FleetOrder::Intercept { target: theirs };
        w.step(&[]);
        assert!(!matches!(w.fleets[&theirs].order, FleetOrder::Attack { .. }), "already hunted: no second battle of its own");
    }

    #[test]
    fn a_travelling_pirate_keeps_its_run_and_a_guard_its_ward() {
        let (mut w, owner, ours, theirs) = meeting(1_200.0);
        let away = w.fleets[&theirs].pos + Vec2::new(40_000.0, 0.0);
        w.fleets.get_mut(&theirs).unwrap().order = FleetOrder::MoveTo { dest: away };
        let ward = w.alloc_entity_id();
        let pos = w.fleets[&ours].pos;
        w.fleets.insert(ward, Fleet::single(ward, owner, ShipKind::TinyFreighter, pos, FleetOrder::Idle, None));
        w.fleets.get_mut(&ours).unwrap().order = FleetOrder::Guard { target: ward };
        w.step(&[]);
        assert!(matches!(w.fleets[&theirs].order, FleetOrder::MoveTo { .. }), "a scripted pirate run isn't diverted");
        // A guard answers by its own rule (it intercepts a hostile near its
        // ward); this one never hands it the close-contact Attack.
        assert!(!matches!(w.fleets[&ours].order, FleetOrder::Attack { .. }), "a guard keeps its own rules: {:?}", w.fleets[&ours].order);
    }
}
