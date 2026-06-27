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
use crate::market::Side;
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

    /// A battle resolved in true space at `pos` with ONE outcome (§8), decided by
    /// the seeded RNG. Delivered to attacker and defender as a *delayed report* —
    /// each learns the SAME outcome only when its light reaches their command
    /// center, so they may learn it at different times.
    RaidResolved {
        attacker: PlayerId,
        defender: PlayerId,
        attacker_ship: EntityId,
        target_ship: EntityId,
        attacker_kind: ShipKind,
        target_kind: ShipKind,
        outcome: RaidOutcome,
        pos: crate::math::Vec2,
    },

    /// A player claimed a star system at `pos` at this event's `time` (§4). Like
    /// a home-anchor claim, ownership is revealed to rivals only when this event's
    /// light reaches their command center (`time + |pos − cc|/c`) — the owner
    /// knows instantly, rivals learn by light (the view filter enforces it).
    SystemClaimed {
        system: EntityId,
        owner: PlayerId,
        pos: crate::math::Vec2,
    },

    /// A ship was destroyed at `pos` at this event's `time`. Drives the
    /// per-player **delayed** disappearance: the ship is gone from true space
    /// now, but each player keeps seeing its ghost until the light of this event
    /// reaches their command center (`time + |pos − cc|/c`). NEVER delete it from
    /// all views at once — that would be FTL information (§6).
    ShipDestroyed {
        ship: EntityId,
        owner: PlayerId,
        kind: ShipKind,
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
    /// A limit order was placed and rests on the book.
    LimitPlaced { player: PlayerId, side: Side, commodity: Commodity, units: u32, limit_price: f64 },
    /// A limit order (partially) cleared in the batch at the uniform price.
    LimitFilled { player: PlayerId, side: Side, commodity: Commodity, units: u32, unit_price: f64 },
}

impl TradeEvent {
    /// The corporation this news is for.
    pub fn player(&self) -> PlayerId {
        match self {
            TradeEvent::Bought { player, .. }
            | TradeEvent::Delivered { player, .. }
            | TradeEvent::SellDispatched { player, .. }
            | TradeEvent::Sold { player, .. }
            | TradeEvent::LimitPlaced { player, .. }
            | TradeEvent::LimitFilled { player, .. } => *player,
        }
    }
}

/// The result of a battle (§8). One seeded roll per battle; both sides observe
/// the same result, just light-delayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaidOutcome {
    /// The target was destroyed (the attacker won).
    TargetDestroyed,
    /// The attacker was destroyed (escort/duel went the other way).
    AttackerDestroyed,
    /// Both ships were destroyed.
    BothDestroyed,
    /// Both survived — the attacker was driven off.
    BothSurvive,
    /// (Convoy only) the target reached the hub before contact — no battle.
    Escaped,
}

impl RaidOutcome {
    /// (attacker_destroyed, target_destroyed) for this outcome.
    pub fn kills(self) -> (bool, bool) {
        match self {
            RaidOutcome::TargetDestroyed => (false, true),
            RaidOutcome::AttackerDestroyed => (true, false),
            RaidOutcome::BothDestroyed => (true, true),
            RaidOutcome::BothSurvive | RaidOutcome::Escaped => (false, false),
        }
    }
}

impl Event {
    pub fn new(time: f64, payload: EventPayload) -> Self {
        Event { time, payload }
    }
}
