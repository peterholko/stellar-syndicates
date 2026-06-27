//! Ships: the mobile entities of the galaxy.
//!
//! Two types embody the §7 convoy-vs-raider dial — not a special rule, just
//! different acceleration / top-speed. Convoys are slow and heavy; raiders are
//! fast and light and can run a convoy down. (The lane mass-reduction effect of
//! §7/§10 lands in a later milestone.)
//!
//! Every ship moves under flip-and-burn and acts on a standing **order**; the
//! world advances each ship once per tick. There is no real-time piloting — the
//! async-native, lightspeed-bound design demands standing orders, not micro.

use serde::{Deserialize, Serialize};

use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::movement::flip_and_burn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShipKind {
    /// Slow, heavy hauler — the largest ship in the game (§7). Carries trade.
    Convoy,
    /// Fast, light interceptor. Cuts chords across open space to run convoys
    /// down.
    Raider,
}

impl ShipKind {
    /// Acceleration magnitude (sim units / s²).
    pub fn accel(self) -> f64 {
        match self {
            ShipKind::Convoy => 9.0,
            ShipKind::Raider => 30.0,
        }
    }

    /// Cruise speed cap (sim units / s). Both stay well below `c` (= 300) so
    /// relativity is respected — nothing outruns its own light.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 36.0,
            ShipKind::Raider => 90.0,
        }
    }
}

/// Seconds a patrolling ship waits at each waypoint before moving on.
const PATROL_DWELL: f64 = 2.5;

/// A ship's standing order — what it does without further input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ShipOrder {
    /// At rest, no goal.
    Idle,
    /// Flip-and-burn to a fixed point, then go [`ShipOrder::Idle`].
    MoveTo { dest: Vec2 },
    /// Cycle forever through a list of waypoints, dwelling briefly at each.
    /// (M2 demo behaviour so the shared world is visibly alive; real
    /// player-issued orders arrive in M4/M5.)
    Patrol {
        waypoints: Vec<Vec2>,
        index: usize,
        /// Sim time until which the ship holds at the current waypoint.
        dwell_until: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: ShipKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub order: ShipOrder,
}

impl Ship {
    pub fn new(id: EntityId, owner: PlayerId, kind: ShipKind, pos: Vec2, order: ShipOrder) -> Self {
        Ship {
            id,
            owner,
            kind,
            pos,
            vel: Vec2::ZERO,
            order,
        }
    }

    /// Advance this ship one timestep at simulation time `time`.
    pub fn advance(&mut self, time: f64, dt: f64) {
        let accel = self.kind.accel();
        let max_speed = self.kind.max_speed();

        match &mut self.order {
            ShipOrder::Idle => {
                // Holds station. (Drag-free space; already at rest.)
                self.vel = Vec2::ZERO;
            }
            ShipOrder::MoveTo { dest } => {
                let step = flip_and_burn(self.pos, self.vel, *dest, accel, max_speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    self.order = ShipOrder::Idle;
                }
            }
            ShipOrder::Patrol {
                waypoints,
                index,
                dwell_until,
            } => {
                if waypoints.is_empty() {
                    self.vel = Vec2::ZERO;
                    return;
                }
                if time < *dwell_until {
                    self.vel = Vec2::ZERO;
                    return;
                }
                let dest = waypoints[*index % waypoints.len()];
                let step = flip_and_burn(self.pos, self.vel, dest, accel, max_speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    *dwell_until = time + PATROL_DWELL;
                    *index = (*index + 1) % waypoints.len();
                }
            }
        }
    }
}
