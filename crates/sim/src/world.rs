//! The authoritative world state and the pure step function.
//!
//! This is ground truth — the single, objective galaxy the server's game loop
//! owns. Players never see it directly; the per-player view filter (M3) derives
//! each player's delayed, fogged reconstruction from it. `World` performs no
//! I/O and no async work: it takes commands and a fixed timestep and returns
//! the next state plus the events that occurred (§14).

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::config::{SimConfig, DT};
use crate::event::{Event, EventPayload};
use crate::galaxy::{generate_home_slots, generate_systems, HomeSlot, StarSystem};
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::ship::{Ship, ShipKind, ShipOrder};

/// A player's corporation — their persistent presence in the galaxy. Grows in
/// later milestones (credits, holdings, fleets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corporation {
    pub id: PlayerId,
    pub name: String,
    /// Tick at which this corporation first entered the galaxy.
    pub joined_tick: u64,
    /// The corporation's home anchor — its bright coherence peak (§6).
    pub home: Vec2,
    /// Origin of this player's light-cone: all fog-of-war and command latency
    /// are computed from here (§6). Equals `home` until the command center is
    /// relocated (a later milestone); kept separate so M3 can use it directly.
    pub command_center: Vec2,
}

/// Ground-truth galaxy state. Deterministic given `config.seed` and the
/// command sequence applied via [`World::step`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub config: SimConfig,
    /// Number of completed ticks.
    pub tick: u64,
    /// Simulation time in seconds (`tick * DT`).
    pub time: f64,
    /// The wormhole hub at the galaxy centre — the shared market commons (§4).
    pub hub: Vec2,
    /// Procedurally-placed star systems (static geography).
    pub systems: Vec<StarSystem>,
    /// Pre-generated home-anchor slots; assigned to players on join.
    pub home_slots: Vec<HomeSlot>,
    /// All corporations, keyed by id. `BTreeMap` keeps iteration deterministic.
    pub players: BTreeMap<PlayerId, Corporation>,
    /// All ships, keyed by id. `BTreeMap` keeps integration order deterministic.
    pub ships: BTreeMap<EntityId, Ship>,
    /// Monotonic allocator for entity ids.
    next_entity_id: u64,
    /// World RNG stream (continues past generation) for deterministic events.
    rng: crate::rng::Rng,
}

impl World {
    /// Create a galaxy for the given configuration: hub at the centre, seeded
    /// star systems, and a ring of empty home anchors.
    pub fn new(config: SimConfig) -> Self {
        let mut rng = crate::rng::Rng::new(config.seed);
        let mut next_entity_id = 1u64;

        let systems = {
            let mut alloc = || {
                let id = EntityId(next_entity_id);
                next_entity_id += 1;
                id
            };
            generate_systems(
                &mut rng,
                config.galaxy_radius,
                config.system_count,
                &mut alloc,
            )
        };
        let home_slots = generate_home_slots(
            &mut rng,
            config.galaxy_radius,
            config.home_ring_frac,
            config.max_players,
        );

        World {
            config,
            tick: 0,
            time: 0.0,
            hub: Vec2::ZERO,
            systems,
            home_slots,
            players: BTreeMap::new(),
            ships: BTreeMap::new(),
            next_entity_id,
            rng,
        }
    }

    /// Allocate a fresh, deterministic entity id.
    fn alloc_entity_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    /// Advance the world by exactly one fixed timestep, applying the given
    /// commands at this tick boundary. Returns the events produced this tick.
    ///
    /// Pure and deterministic: same starting `World` + same `commands` always
    /// yields the same next state and events.
    pub fn step(&mut self, commands: &[Command]) -> Vec<Event> {
        let mut events = Vec::new();

        // 1. Apply external commands at the current instant.
        for cmd in commands {
            self.apply(cmd, &mut events);
        }

        // 2. Integrate continuous movement. Ships advance under flip-and-burn
        //    on their standing orders.
        let time = self.time;
        for ship in self.ships.values_mut() {
            ship.advance(time, DT);
        }

        // 3. Advance the clock.
        self.tick += 1;
        self.time += DT;

        events
    }

    fn apply(&mut self, cmd: &Command, events: &mut Vec<Event>) {
        match cmd {
            Command::AddPlayer { id, name } => {
                // Idempotent: a reconnecting player keeps their corporation.
                if self.players.contains_key(id) {
                    return;
                }
                let home = self.assign_home(*id);
                self.players.insert(
                    *id,
                    Corporation {
                        id: *id,
                        name: name.clone(),
                        joined_tick: self.tick,
                        home,
                        command_center: home,
                    },
                );
                events.push(Event::new(
                    self.time,
                    EventPayload::PlayerJoined {
                        id: *id,
                        name: name.clone(),
                    },
                ));
                self.spawn_starting_fleet(*id, home, events);
            }
        }
    }

    /// Assign an unused home anchor to a player (or append one if the galaxy is
    /// over capacity), returning its position.
    fn assign_home(&mut self, id: PlayerId) -> Vec2 {
        if let Some(slot) = self.home_slots.iter_mut().find(|s| s.owner.is_none()) {
            slot.owner = Some(id);
            return slot.pos;
        }
        // Over capacity: place an extra anchor at a deterministic ring spot.
        let n = self.home_slots.len();
        let angle = TAU * (n as f64) * 0.61803398875; // golden-angle scatter
        let pos = Vec2::from_polar(angle, self.config.galaxy_radius * self.config.home_ring_frac);
        self.home_slots.push(HomeSlot {
            pos,
            owner: Some(id),
        });
        pos
    }

    /// Spawn the M2 demo fleet (one convoy, one raider) at a home anchor, set to
    /// patrol so the shared world is visibly alive. (Player-issued orders arrive
    /// in M4/M5.)
    fn spawn_starting_fleet(&mut self, owner: PlayerId, home: Vec2, events: &mut Vec<Event>) {
        let hub = self.hub;
        let nearest = self.nearest_system(home).unwrap_or(hub);

        // Convoy plies the home↔hub trade lane.
        let convoy_id = self.alloc_entity_id();
        self.ships.insert(
            convoy_id,
            Ship::new(
                convoy_id,
                owner,
                ShipKind::Convoy,
                home,
                ShipOrder::Patrol {
                    waypoints: vec![home, hub],
                    index: 1,
                    dwell_until: 0.0,
                },
            ),
        );
        events.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id: convoy_id,
                owner,
                kind: ShipKind::Convoy,
            },
        ));

        // Raider roams home↔nearest-system↔hub.
        let raider_id = self.alloc_entity_id();
        self.ships.insert(
            raider_id,
            Ship::new(
                raider_id,
                owner,
                ShipKind::Raider,
                home,
                ShipOrder::Patrol {
                    waypoints: vec![home, nearest, hub],
                    index: 1,
                    dwell_until: 0.0,
                },
            ),
        );
        events.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id: raider_id,
                owner,
                kind: ShipKind::Raider,
            },
        ));
    }

    /// Position of the star system nearest to `p` (None if no systems).
    fn nearest_system(&self, p: Vec2) -> Option<Vec2> {
        self.systems
            .iter()
            .min_by(|a, b| {
                a.pos
                    .distance_sq(p)
                    .partial_cmp(&b.pos.distance_sq(p))
                    .unwrap()
            })
            .map(|s| s.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PlayerId;

    fn test_world() -> World {
        World::new(SimConfig::for_players(123, 4))
    }

    #[test]
    fn galaxy_is_generated() {
        let w = test_world();
        assert_eq!(w.hub, Vec2::ZERO);
        assert_eq!(w.systems.len(), w.config.system_count as usize);
        assert_eq!(w.home_slots.len(), w.config.max_players as usize);
        // Systems lie within the galaxy radius.
        for s in &w.systems {
            assert!(s.pos.length() <= w.config.galaxy_radius + 1.0);
        }
    }

    #[test]
    fn clock_advances_one_dt_per_step() {
        let mut w = test_world();
        assert_eq!(w.tick, 0);
        w.step(&[]);
        assert_eq!(w.tick, 1);
        assert!((w.time - DT).abs() < 1e-12);
    }

    #[test]
    fn add_player_assigns_home_and_fleet() {
        let mut w = test_world();
        let id = PlayerId(7);
        let ev = w.step(&[Command::AddPlayer {
            id,
            name: "Acme".into(),
        }]);
        // PlayerJoined + two ShipSpawned.
        assert_eq!(ev.len(), 3);
        assert_eq!(w.players.len(), 1);
        assert_eq!(w.ships.len(), 2);
        let corp = &w.players[&id];
        assert_eq!(corp.home, corp.command_center);
        // One anchor is now owned.
        assert_eq!(w.home_slots.iter().filter(|s| s.owner == Some(id)).count(), 1);
    }

    #[test]
    fn add_player_is_idempotent() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let ev2 = w.step(&[Command::AddPlayer {
            id,
            name: "Acme (reconnect)".into(),
        }]);
        assert_eq!(ev2.len(), 0);
        assert_eq!(w.players.len(), 1);
        assert_eq!(w.ships.len(), 2); // no duplicate fleet
    }

    #[test]
    fn ships_actually_move() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let start: Vec<Vec2> = w.ships.values().map(|s| s.pos).collect();
        // Advance a few seconds.
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let moved = w
            .ships
            .values()
            .zip(&start)
            .any(|(s, &p0)| s.pos.distance(p0) > 10.0);
        assert!(moved, "ships should have moved from their start positions");
    }

    #[test]
    fn determinism_same_commands_same_state() {
        let cmds = vec![
            Command::AddPlayer { id: PlayerId(1), name: "A".into() },
            Command::AddPlayer { id: PlayerId(2), name: "B".into() },
        ];
        let mut a = test_world();
        let mut b = test_world();
        for _ in 0..300 {
            a.step(&cmds);
            b.step(&cmds);
        }
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
