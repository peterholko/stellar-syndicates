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
        /// Per-kind ships the ATTACKER lost over the engagement (§Part 2
        /// Lanchester — a composition-vs-composition report). serde default keeps
        /// old snapshots/events loading; empty for a no-loss brush.
        #[serde(default)]
        attacker_losses: std::collections::BTreeMap<ShipKind, u32>,
        /// Per-kind ships the DEFENDER (target side) lost over the engagement.
        #[serde(default)]
        target_losses: std::collections::BTreeMap<ShipKind, u32>,
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

    /// Construction began at an owned system: a recipe was deducted and a build job
    /// enqueued (§step1 growth sink). Owner-only news (the spend is private; the
    /// finished ship reveals as a normal light-gated ghost).
    BuildStarted {
        id: u64,
        owner: PlayerId,
        system: EntityId,
        what: crate::build::BuildKind,
        complete_tick: u64,
    },
    /// A system development completed (an upgrade tier applied). Owner-only.
    SystemUpgraded {
        system: EntityId,
        owner: PlayerId,
        /// Which development completed (Extractor/Depot/…).
        upgrade: crate::build::SystemUpgrade,
        /// The new tier of that development.
        tier: u32,
    },
    /// A build request was SOFT-REJECTED (no debit, no job — async-fair): the
    /// system can't host it right now. Owner-only news; `reason` says why.
    BuildRejected {
        owner: PlayerId,
        system: EntityId,
        what: crate::build::BuildKind,
        reason: BuildRejectReason,
    },
    /// A COLONY SHIP arrived at a system that was ALREADY claimed (§ships
    /// part 3 — you lost the race, or it flipped en route). SOFT: the ship
    /// holds position, fully intact and redirectable; nothing is destroyed.
    /// Owner-only news, light-delayed from the hold position.
    ColonyHeld { owner: PlayerId, system: EntityId, pos: crate::math::Vec2 },
    /// A SCOUT captured an intel snapshot of a rival system's fortifications
    /// (§scout part 2). OWNER-ONLY: the knowledge exists on the scout at `pos`
    /// at the capture moment — the owner learns it when that light reaches
    /// their command center (the timeline delays it accordingly); the scouted
    /// rival learns NOTHING. Emitted on fresh approaches / value changes only,
    /// never per-tick.
    IntelGathered {
        owner: PlayerId,
        system: EntityId,
        defense_tier: u32,
        shipyard_tier: u32,
        /// The scout's position at capture — the report's light source.
        pos: crate::math::Vec2,
    },
    /// A Habitat's supply state flipped (§buildings step 3a). OWNER-ONLY news:
    /// `fed = false` means this tick's Provisions upkeep couldn't be covered, so
    /// the output boost is SUSPENDED (nothing destroyed, no tier lost — it
    /// recovers the tick food is available again); `fed = true` is the recovery.
    /// Emitted only on TRANSITIONS, never per-tick (no spam).
    HabitatSupplyChanged { owner: PlayerId, system: EntityId, fed: bool },
    /// A Defense Platform engaged a hostile raider attacking one of the owner's
    /// convoys inside its protection radius (§buildings step 2c). OWNER-ONLY
    /// detail (tiers lost, result) — the ATTACKER learns only the standard
    /// battle outcome via the accompanying `RaidResolved` (a platform reveals
    /// itself exclusively through engagement results). `pos` is the contact
    /// point, for light-delaying the owner's news like any battle.
    PlatformEngaged {
        owner: PlayerId,
        system: EntityId,
        pos: crate::math::Vec2,
        /// The attacking raider was destroyed by the platform.
        raider_destroyed: bool,
        /// The raider was driven off (broke off home; platform intact that duel).
        driven_off: bool,
        /// Platform tiers lost in the engagement (damage; slots free up).
        tiers_lost: u32,
    },
    /// A dispatch was LIMITED because no owned system could cover its fuel cost
    /// (§step1 part 2). The ship/order/goods are never lost — the op simply held.
    /// Owner-only; `kind` labels what was held ("move"/"raid"/"shipment").
    FuelShortfall { owner: PlayerId, needed: f64, kind: crate::fuel::ShortfallKind },
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
    /// A STANDING ORDER fired (§15): the rule auto-dispatched a convoy carrying
    /// `units` of `commodity` from `source`. The "policy ran while you were away"
    /// notification — feeds the check-in timeline.
    AutoDispatched { player: PlayerId, commodity: Commodity, units: u32, source: EntityId, rule_id: u32 },
    /// An automated supply convoy reached `system` but the corp no longer owns it
    /// (lost / taken mid-transit). What happened to the cargo is governed by the
    /// corp's [`crate::doctrine::DestinationInvalidPolicy`] and reported as
    /// `action`. The "your frontier supply went sideways" notification — an
    /// attention item for the check-in timeline (§16, Layer 2).
    SupplyDiverted { player: PlayerId, commodity: Commodity, units: u32, system: EntityId, action: DivertAction },
    /// A delivery arrived at `system` but its DEPOT was (partly) FULL (§buildings
    /// step 2): `units` of the cargo could not be stored, so the SAME convoy
    /// carries the excess onward to the hub to sell (sub-light, raidable — goods
    /// are never silently destroyed). Any storable part was delivered first (its
    /// own `Delivered` event).
    StorageOverflow { player: PlayerId, commodity: Commodity, units: u32, system: EntityId },
}

/// What became of an automated supply convoy whose destination was no longer
/// owned on arrival (mirrors [`crate::doctrine::DestinationInvalidPolicy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivertAction {
    /// The cargo was lost.
    Lost,
    /// The convoy re-routed home (and will deposit there, raidable in transit).
    ReturnedHome,
    /// The convoy re-routed to the hub to sell (raidable in transit).
    SoldAtHub,
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
            | TradeEvent::LimitFilled { player, .. }
            | TradeEvent::AutoDispatched { player, .. }
            | TradeEvent::SupplyDiverted { player, .. }
            | TradeEvent::StorageOverflow { player, .. } => *player,
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

/// Why a build was soft-rejected (§buildings step 1). Owner-only detail for the
/// timeline notice; the request costs nothing (no debit, no job — async-fair).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum BuildRejectReason {
    /// Every development slot at the system is used (built + in-progress).
    NoSlot,
    /// The system's Shipyard tier is below what this ship kind needs
    /// (§buildings step 3: Convoy ≥ 1, Raider ≥ 2).
    NeedsShipyard { required: u32 },
}

impl Event {
    pub fn new(time: f64, payload: EventPayload) -> Self {
        Event { time, payload }
    }
}
