//! Commands: the only way the outside world mutates the simulation.
//!
//! The game loop collects commands (from player intents and from the server's
//! own session events) and feeds them to [`crate::world::World::step`] each
//! tick. Keeping every mutation as an explicit, serialisable command is what
//! makes the core deterministic and event-sourceable (§14).

use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;

/// A single authoritative mutation request, applied at a tick boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Command {
    /// Register (or re-attach) a player's corporation. Idempotent: issuing it
    /// for an existing `id` does not duplicate or reset the corporation, so a
    /// reconnecting player keeps their state (M6).
    AddPlayer { id: PlayerId, name: String },
}
