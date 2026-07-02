//! Commands: the only way the outside world mutates the simulation.
//!
//! The game loop collects commands (from player intents and from the server's
//! own session events) and feeds them to [`crate::world::World::step`] each
//! tick. Keeping every mutation as an explicit, serialisable command is what
//! makes the core deterministic and event-sourceable (§14).

use serde::{Deserialize, Serialize};

use crate::doctrine::FleetDoctrine;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::standing::StandingOrder;

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

    /// Buy at market on the hub Exchange (§9): instant settlement at the true
    /// standing price (credits debited now), then a delivery convoy carries the
    /// goods hub → home (raidable in transit). Price-certain, delivery-risky.
    MarketBuy {
        player_id: PlayerId,
        commodity: crate::cargo::Commodity,
        units: u32,
    },

    /// Sell at market (§9): commit goods to the crossing FIRST — a convoy carries
    /// them home → hub and sells at the price-on-arrival (not a locked launch
    /// price). The seller faces double uncertainty (raid + unknown final price).
    MarketSell {
        player_id: PlayerId,
        commodity: crate::cargo::Commodity,
        units: u32,
    },

    /// Place a resting limit order (§9). It clears in the periodic uniform-price
    /// call auction — within a batch everyone clears at one price, so reacting
    /// fastest confers no edge (the anti-sniping mechanism). Resources are
    /// reserved when placed (credits for a buy, goods for a sell).
    PlaceLimitOrder {
        player_id: PlayerId,
        side: crate::market::Side,
        commodity: crate::cargo::Commodity,
        units: u32,
        limit_price: f64,
    },

    /// Dispatch convoys to carry a claimed system's accumulated production to the
    /// hub to sell (§9). One raidable convoy per stockpiled commodity, flying the
    /// dangerous, fog-blind frontier→hub crossing; each sells on arrival at the
    /// price-on-arrival. Ignored unless the player owns the system and it has
    /// production to ship.
    ShipProduction {
        player_id: PlayerId,
        system_id: EntityId,
    },

    /// Create or replace a standing logistics order (§15) — a constrained
    /// automation rule the corp runs server-side, online or off. INSTANT local
    /// administration (like a limit order): it changes only the player's own
    /// private policy table and reveals nothing to rivals; the CONVOYS it later
    /// spawns are sub-light and raidable. `order.id == 0` creates (a fresh id is
    /// allocated); a matching id replaces (edit), preserving anti-spam state.
    /// Validated against the constrained option set; nonsense is ignored.
    SetStandingOrder {
        player_id: PlayerId,
        order: StandingOrder,
    },

    /// Remove a standing order by id (no-op if absent). Does not recall any convoy
    /// it already dispatched. Instant local administration.
    ClearStandingOrder {
        player_id: PlayerId,
        order_id: u32,
    },

    /// Set the corporation's fleet doctrine (§16) — the constrained, server-run
    /// combat & logistics policy ([`FleetDoctrine`]) that governs how autonomous
    /// pickets engage/retreat/escort and how automated supply re-routes when a
    /// destination is lost. INSTANT local administration (like a standing order):
    /// it changes only the corp's own private policy and reveals nothing to rivals;
    /// the SHIPS it later commands are sub-light, raidable, and light-revealed.
    /// Always valid (a closed menu of enums), so it is never rejected.
    SetFleetDoctrine {
        player_id: PlayerId,
        doctrine: FleetDoctrine,
    },

    /// Build a ship at one of the player's OWNED systems (§step1 growth sink).
    /// Deducts a fixed RECIPE of commodities from that system's stockpile NOW and
    /// enqueues a build job that completes after the recipe's duration, spawning the
    /// ship (Idle) at the system. INSTANT local administration (not light-delayed):
    /// you commit resources at your own system immediately; the COMPLETION reveals to
    /// rivals only as a normal light-gated ghost. Ignored unless the player owns the
    /// system and its stockpile covers the recipe (a soft reject — no partial debit).
    BuildShip {
        player_id: PlayerId,
        system_id: EntityId,
        ship_kind: crate::ship::ShipKind,
        /// (§FLEETS management v1) The fleet to JOIN when the build completes, if
        /// it's still docked at this system — else a new fleet-of-one is formed.
        /// `None` always forms a new fleet (the pre-FLEETS behaviour). serde
        /// default so old clients omitting it still parse.
        #[serde(default)]
        join: Option<EntityId>,
    },

    /// Merge one of the player's fleets INTO another (§FLEETS management v1).
    /// Both must be the player's, Idle, and co-located at one of the player's
    /// OWNED systems. `from`'s composition (and cargo, if `into` carries none) is
    /// absorbed into `into`; `from` is removed. Soft-reject on any violation — an
    /// in-flight fleet can't be merged (no in-flight detachment in v1).
    MergeFleets {
        player_id: PlayerId,
        into: EntityId,
        from: EntityId,
    },

    /// Split ships off one of the player's fleets into a NEW fleet (§FLEETS
    /// management v1). The source must be the player's, Idle, and at one of their
    /// OWNED systems. `counts` names how many of each kind to detach; the new
    /// fleet spawns Idle beside the source. Soft-reject if the counts are empty,
    /// exceed what's aboard, or would empty the source (split SOME, keep SOME).
    SplitFleet {
        player_id: PlayerId,
        fleet_id: EntityId,
        counts: std::collections::BTreeMap<crate::ship::ShipKind, u32>,
    },

    /// Develop one of the player's OWNED systems (§step1 structure sink) — e.g. an
    /// Extractor tier that raises its output. Same deduct-and-enqueue semantics as
    /// `BuildShip`; on completion the upgrade is applied (only if still owned).
    DevelopSystem {
        player_id: PlayerId,
        system_id: EntityId,
        upgrade: crate::build::SystemUpgrade,
    },
}
