//! The authoritative world state and the pure step function.
//!
//! This is ground truth — the single, objective galaxy the server's game loop
//! owns. Players never see it directly; the per-player view filter (M3) derives
//! each player's delayed, fogged reconstruction from it. `World` performs no
//! I/O and no async work: it takes commands and a fixed timestep and returns
//! the next state plus the events that occurred (§14).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::config::{SimConfig, DT};
use crate::event::{Event, EventPayload};
use crate::ids::PlayerId;

/// A player's corporation — their persistent presence in the galaxy. Grows in
/// later milestones (credits, holdings, command-center position, fleets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corporation {
    pub id: PlayerId,
    pub name: String,
    /// Tick at which this corporation first entered the galaxy.
    pub joined_tick: u64,
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
    /// All corporations, keyed by id. `BTreeMap` keeps iteration order
    /// deterministic.
    pub players: BTreeMap<PlayerId, Corporation>,
    /// Monotonic allocator for entity ids (used from M2 onward).
    next_entity_id: u64,
}

impl World {
    /// Create an empty world for the given configuration. Galaxy generation
    /// (hub, anchors, systems) arrives in M2.
    pub fn new(config: SimConfig) -> Self {
        World {
            config,
            tick: 0,
            time: 0.0,
            players: BTreeMap::new(),
            next_entity_id: 1,
        }
    }

    /// Allocate a fresh, deterministic entity id.
    #[allow(dead_code)]
    pub(crate) fn alloc_entity_id(&mut self) -> crate::ids::EntityId {
        let id = crate::ids::EntityId(self.next_entity_id);
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

        // 2. (M2+) integrate continuous subsystems — movement, accrual — here.

        // 3. Advance the clock.
        self.tick += 1;
        self.time += DT;

        events
    }

    fn apply(&mut self, cmd: &Command, events: &mut Vec<Event>) {
        match cmd {
            Command::AddPlayer { id, name } => {
                // Idempotent: a reconnecting player keeps their corporation.
                if !self.players.contains_key(id) {
                    self.players.insert(
                        *id,
                        Corporation {
                            id: *id,
                            name: name.clone(),
                            joined_tick: self.tick,
                        },
                    );
                    events.push(Event::new(
                        self.time,
                        EventPayload::PlayerJoined {
                            id: *id,
                            name: name.clone(),
                        },
                    ));
                }
            }
        }
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
    fn clock_advances_one_dt_per_step() {
        let mut w = test_world();
        assert_eq!(w.tick, 0);
        w.step(&[]);
        assert_eq!(w.tick, 1);
        assert!((w.time - DT).abs() < 1e-12);
    }

    #[test]
    fn add_player_is_idempotent() {
        let mut w = test_world();
        let id = PlayerId(7);
        let ev = w.step(&[Command::AddPlayer {
            id,
            name: "Acme".into(),
        }]);
        assert_eq!(ev.len(), 1);
        assert_eq!(w.players.len(), 1);

        // Re-adding the same id produces no new event and no duplicate.
        let ev2 = w.step(&[Command::AddPlayer {
            id,
            name: "Acme (reconnect)".into(),
        }]);
        assert_eq!(ev2.len(), 0);
        assert_eq!(w.players.len(), 1);
        // Original join tick preserved.
        assert_eq!(w.players[&id].joined_tick, 0);
    }

    #[test]
    fn determinism_same_commands_same_state() {
        let cmds = vec![
            Command::AddPlayer {
                id: PlayerId(1),
                name: "A".into(),
            },
            Command::AddPlayer {
                id: PlayerId(2),
                name: "B".into(),
            },
        ];
        let mut a = test_world();
        let mut b = test_world();
        for _ in 0..100 {
            a.step(&cmds);
            b.step(&cmds);
        }
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
