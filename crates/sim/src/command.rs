//! Commands: the only way the outside world mutates the simulation.
//!
//! The game loop collects commands (from player intents and from the server's
//! own session events) and feeds them to [`crate::world::World::step`] each
//! tick. Keeping every mutation as an explicit, serialisable command is what
//! makes the core deterministic and event-sourceable (§14).

use serde::{Deserialize, Serialize};

use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;

/// A single authoritative mutation request, applied at a tick boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Command {
    /// Register (or re-attach) a player's corporation. Idempotent: issuing it
    /// for an existing `id` does not duplicate or reset the corporation, so a
    /// reconnecting player keeps their state (M6).
    AddPlayer { id: PlayerId, name: String },

    /// A player orders one of *their* ships to a destination. The order is a
    /// novel command to a mobile target, so it travels at light speed (§3): it
    /// does not take effect immediately but only after the outbound light-travel
    /// time from the player's command center to the ship. The sim schedules it;
    /// the player learns the result later still via their delayed view (the
    /// three clocks of §6). Commands for ships the player does not own are
    /// ignored.
    MoveShip {
        player_id: PlayerId,
        ship_id: EntityId,
        dest: Vec2,
    },

    /// Commit one of the player's raiders to intercept a target ship (§8). Like
    /// any novel command to a mobile asset, it travels at light speed: the
    /// raider only begins pursuing once the order's outbound light reaches it.
    /// The player commits on a *stale* sighting of the target; the raider then
    /// pursues the target's TRUE position. Ignored unless the player owns the
    /// raider and the target belongs to someone else.
    CommitRaid {
        player_id: PlayerId,
        raider_id: EntityId,
        target_id: EntityId,
    },

    /// Recall a raider (break off, return home). Also light-delayed — it may
    /// arrive too late to matter ("commanding into the past").
    RecallRaid {
        player_id: PlayerId,
        raider_id: EntityId,
    },
}
