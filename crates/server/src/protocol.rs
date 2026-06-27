//! The WebSocket wire protocol between client and server.
//!
//! Messages are JSON, tagged by a `type` field. The client sends *intents*
//! ([`ClientMsg`]); the server pushes each player their own *filtered view*
//! ([`ServerMsg`]). The client holds no authoritative state — these messages
//! are the entire contract (§14).
//!
//! NOTE (M2): the `View` currently carries TRUE world positions to all players,
//! to verify movement. M3 replaces this with each player's delayed/fogged
//! reconstruction — the wire types here are deliberately explicit (not the raw
//! sim structs) so that step exposes exactly what each player is allowed to see.

use serde::{Deserialize, Serialize};
use sim::{EntityId, HomeSlot, PlayerId, Ship, ShipKind, StarSystem, Vec2};

/// Messages sent by the client to the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// First message a connection must send: identify as a player. The name is
    /// hashed server-side into a stable [`PlayerId`] so reconnecting with the
    /// same name resumes the same corporation.
    Join { name: String },

    /// Application-level keepalive (optional; the client may send periodically).
    Ping,
}

/// Static galaxy geography, sent once at join. Never changes during a session
/// (systems don't move), so it doesn't need to be in the per-tick stream.
#[derive(Debug, Clone, Serialize)]
pub struct GalaxyInfo {
    pub hub: Vec2,
    pub radius: f64,
    /// Speed of light (sim units / s) — lets the client annotate light-delays.
    pub c: f64,
    pub systems: Vec<StarSystem>,
}

/// A ship as shown to a player. Deliberately omits the ship's standing order —
/// that is internal truth the client must not see (and M3 must not leak).
#[derive(Debug, Clone, Serialize)]
pub struct ShipView {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: ShipKind,
    pub pos: Vec2,
    pub vel: Vec2,
}

impl ShipView {
    pub fn from_ship(s: &Ship) -> Self {
        ShipView {
            id: s.id,
            owner: s.owner,
            kind: s.kind,
            pos: s.pos,
            vel: s.vel,
        }
    }
}

/// Messages pushed by the server to a single player's connection.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    /// Sent once, immediately after a successful join.
    Welcome {
        player_id: PlayerId,
        name: String,
        /// Sim tick rate (Hz) — lets the client display time correctly.
        tick_hz: u32,
        tick: u64,
        sim_time: f64,
        galaxy: GalaxyInfo,
    },

    /// The player's per-tick view of the world. In M2 these are TRUE positions
    /// (movement verification); M3 makes them delayed and fogged.
    View {
        tick: u64,
        sim_time: f64,
        players_online: usize,
        /// All home anchors (with owners), so the client can mark homes.
        anchors: Vec<HomeSlot>,
        /// Ships visible to this player.
        ships: Vec<ShipView>,
    },

    /// A protocol-level error (e.g. a malformed first message).
    Error { message: String },
}

/// Stable hash of a player name → [`PlayerId`]. FNV-1a (64-bit): tiny,
/// dependency-free, and reproducible, so the same name always maps to the same
/// corporation. (Not for security — names are not secret in M1.)
pub fn player_id_from_name(name: &str) -> PlayerId {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in name.trim().to_lowercase().bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    PlayerId(hash)
}
