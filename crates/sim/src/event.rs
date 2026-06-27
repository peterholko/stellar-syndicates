//! Events: everything the simulation emits when it advances.
//!
//! Each call to [`crate::world::World::step`] returns the events produced that
//! tick. Events are the unit the per-player view filter delays and fogs (M3),
//! and the unit the persistence layer appends to its event log (§14). For M1
//! the only events are session-level (a corporation appearing).
//!
//! Every event carries the simulation time at which it occurred so the view
//! filter can later decide when each player's light has reached it.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};
use crate::ship::ShipKind;

/// A discrete thing that happened in the world at `time` (seconds).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Simulation time (seconds) at which this event occurred.
    pub time: f64,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum EventPayload {
    /// A new corporation entered the galaxy for the first time.
    PlayerJoined { id: PlayerId, name: String },
    /// A ship was created (e.g. the demo convoy/raider spawned at a home anchor).
    ShipSpawned {
        id: EntityId,
        owner: PlayerId,
        kind: ShipKind,
    },
    /// A player's move order finally reached a ship (its outbound light arrived)
    /// and took effect.
    OrderApplied { ship_id: EntityId },

    /// Something happened in the economy (§9).
    Trade(TradeEvent),

    /// A raid resolved in true space (§8). Delivered to attacker and defender as
    /// a *delayed report* — each learns it only when the light of the event at
    /// `pos` reaches their command center, so they may learn it at different
    /// times. Carries `pos`/`time` (the event timestamp is `Event.time`).
    RaidResolved {
        attacker: PlayerId,
        defender: PlayerId,
        raider: EntityId,
        convoy: EntityId,
        outcome: RaidOutcome,
        pos: crate::math::Vec2,
    },
}

/// Economy events. `player` always names the corporation involved; values are
/// for the delayed news / log.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum TradeEvent {
    /// A market buy settled instantly at the hub; a delivery convoy is inbound.
    Bought { player: PlayerId, commodity: Commodity, units: u32, unit_price: f64 },
    /// A buy's delivery convoy reached home; goods deposited.
    Delivered { player: PlayerId, commodity: Commodity, units: u32 },
    /// A sell convoy was dispatched toward the hub (goods committed to the dark).
    SellDispatched { player: PlayerId, commodity: Commodity, units: u32 },
    /// A sell convoy reached the hub and cleared at the price-on-arrival.
    Sold { player: PlayerId, commodity: Commodity, units: u32, unit_price: f64 },
}

impl TradeEvent {
    /// The corporation this news is for.
    pub fn player(&self) -> PlayerId {
        match self {
            TradeEvent::Bought { player, .. }
            | TradeEvent::Delivered { player, .. }
            | TradeEvent::SellDispatched { player, .. }
            | TradeEvent::Sold { player, .. } => *player,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RaidOutcome {
    /// The raider reached the convoy — convoy lost.
    Intercepted,
    /// The convoy reached safety (the hub) before contact — raid failed.
    Escaped,
}

impl Event {
    pub fn new(time: f64, payload: EventPayload) -> Self {
        Event { time, payload }
    }
}
