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

use crate::cargo::Cargo;
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

/// Mass added per unit of cargo carried. A fully-loaded convoy is meaningfully
/// heavier than an empty one, so it accelerates noticeably worse (a = F/m) — your
/// richest shipments are also the most sluggish. Tunable.
pub const CARGO_MASS_PER_UNIT: f64 = 28.0;

impl ShipKind {
    /// Engine THRUST (force, F). Acceleration is NOT set directly — it's derived
    /// as `a = F / m` (see [`Ship::accel`]). Convoys have somewhat more thrust
    /// (bigger engines) but vastly more mass, so they still accelerate far worse.
    ///
    /// Tuning note: values are deliberately LOW so the build-up to speed, the
    /// flip-and-burn, and the convoy-vs-raider nimbleness gap are all *watchable*
    /// at the current galaxy scale — a chase plays out over tens of seconds, not
    /// an instant. With these consts an empty raider accelerates at
    /// `2200/200 = 11` su/s² and an empty convoy at `6750/4500 = 1.5` su/s² (a
    /// loaded one ~0.86), so the raider visibly darts while the convoy lumbers.
    pub fn thrust(self) -> f64 {
        match self {
            ShipKind::Convoy => 6750.0,
            ShipKind::Raider => 2200.0,
        }
    }

    /// Hull (empty) MASS, m₀. Trade convoys are ORDERS OF MAGNITUDE more massive
    /// than raiders (here ~22×), which is what makes them ponderous — the
    /// acceleration asymmetry emerges from this, not from hand-set accel consts.
    pub fn hull_mass(self) -> f64 {
        match self {
            ShipKind::Convoy => 4500.0,
            ShipKind::Raider => 200.0,
        }
    }

    /// Cruise speed cap (sim units / s). Both stay well below `c` (= 300) so
    /// relativity is respected — nothing outruns its own light. Acceleration
    /// (above) ramps velocity up to this cap.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 48.0,
            ShipKind::Raider => 120.0,
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
    /// Autonomously pursue a target ship to intercept (§8). Resolved by the
    /// world in true space (contact → convoy lost; target reaches safety →
    /// raid fails). Pursuit steering lives in [`crate::movement::pursue_step`]
    /// (proportional steer-and-correct) and is driven by the world (it needs the
    /// target's state).
    Intercept { target: EntityId },
}

/// What a trade convoy does when it reaches its destination (§9). A buy spawns a
/// delivery convoy (hub → home) that deposits cargo on arrival; a sell spawns a
/// convoy (home → hub) that sells the cargo at the price-on-arrival; a standing
/// logistics order (§15) can also spawn a convoy that deposits cargo into another
/// system's stockpile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeMission {
    DeliverHome,
    SellAtHub,
    /// Deposit the cargo into the destination system's stockpile on arrival (a
    /// system→system supply convoy; §15). Cargo is lost if the destination is no
    /// longer owned by the convoy's owner when it arrives (no gifting rivals).
    DeliverToSystem { system: EntityId },
}

/// A patrolling raider's AUTONOMOUS defensive sortie (§5.1, Pillar 1): it has
/// broken off its patrol on its own to intercept a hostile raider threatening a
/// friendly convoy, and will resume the saved `patrol` route once the threat is
/// gone. Its presence also marks this Intercept as defensive (not a player's
/// manual raid), so the world's standing-doctrine logic owns its lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefenseEngagement {
    pub target: EntityId,
    pub patrol: Vec<Vec2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: ShipKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub order: ShipOrder,
    /// Cargo carried (convoys only; raiders carry none). Broadcast withholds
    /// this — it is revealed by sensor range, not by the Convention.
    pub cargo: Option<Cargo>,
    /// If set, this is a trade convoy that resolves on arrival (§9).
    pub mission: Option<TradeMission>,
    /// If set, this raider is on an AUTONOMOUS defensive intercept (it broke off
    /// patrol to engage a threat) — server-driven standing doctrine, runs whether
    /// or not the owner is connected.
    #[serde(default)]
    pub defense: Option<DefenseEngagement>,
}

impl Ship {
    pub fn new(
        id: EntityId,
        owner: PlayerId,
        kind: ShipKind,
        pos: Vec2,
        order: ShipOrder,
        cargo: Option<Cargo>,
    ) -> Self {
        Ship {
            id,
            owner,
            kind,
            pos,
            vel: Vec2::ZERO,
            order,
            cargo,
            mission: None,
            defense: None,
        }
    }

    /// Total mass = hull + cargo (§7). A loaded ship is heavier, so slower to
    /// accelerate. Recomputed from current cargo, so dropping/gaining cargo
    /// changes how the ship handles.
    pub fn mass(&self) -> f64 {
        let cargo = self.cargo.map(|c| c.units as f64 * CARGO_MASS_PER_UNIT).unwrap_or(0.0);
        self.kind.hull_mass() + cargo
    }

    /// Acceleration DERIVED from thrust and mass: `a = F / m` (§7). Higher mass
    /// (bigger hull, fuller hold) → weaker acceleration for the same thrust. This
    /// is the single source of the raider-vs-convoy nimbleness asymmetry and of
    /// the loaded-convoy penalty — not a hand-set per-kind acceleration.
    pub fn accel(&self) -> f64 {
        self.kind.thrust() / self.mass()
    }

    /// Advance this ship one timestep at simulation time `time`.
    pub fn advance(&mut self, time: f64, dt: f64) {
        let accel = self.accel();
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
            // Interception is driven by the world (it needs the target's state),
            // so there is nothing to do in the self-contained per-ship advance.
            ShipOrder::Intercept { .. } => {}
        }
    }
}
