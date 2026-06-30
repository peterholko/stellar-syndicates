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
use sim::{
    Commodity, EntityId, FleetDoctrine, PlayerId, RaidOutcome, ShipKind, Side, StandingOrder,
    SystemUpgrade, TradeEvent, Vec2,
};

/// Messages sent by the client to the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// First message a connection must send: identify as a player. The name is
    /// hashed server-side into a stable [`PlayerId`] so reconnecting with the
    /// same name resumes the same corporation.
    Join { name: String },

    /// Order one of the player's own ships to a destination. Travels at light
    /// speed to the ship (§6); the server attaches the issuing player.
    MoveShip { ship_id: EntityId, dest: Vec2 },

    /// Commit one of the player's raiders to intercept a target ship (§8).
    CommitRaid { raider_id: EntityId, target_id: EntityId },

    /// Recall a raider (break off, return home). May arrive too late (§8).
    RecallRaid { raider_id: EntityId },

    /// Buy at market on the hub Exchange (§9): instant settlement, then a
    /// delivery convoy carries the goods home.
    MarketBuy { commodity: Commodity, units: u32 },

    /// Sell at market (§9): a convoy carries the goods to the hub and clears at
    /// the price-on-arrival.
    MarketSell { commodity: Commodity, units: u32 },

    /// Place a resting limit order; it clears in the periodic batch (§9).
    PlaceLimitOrder { side: Side, commodity: Commodity, units: u32, limit_price: f64 },

    /// Claim an unclaimed star system for its credit cost (§4). The server
    /// attaches the issuing player; the sim resolves it in true space.
    ClaimSystem { system_id: EntityId },

    /// Ship a claimed system's accumulated production to the hub to sell (§9) —
    /// spawns raidable convoys from the system.
    ShipProduction { system_id: EntityId },

    /// Create or replace a standing logistics order (§15). `order.id == 0` creates;
    /// a matching id edits. Instant local administration; the server attaches the
    /// issuing player.
    SetStandingOrder { order: StandingOrder },

    /// Remove a standing order by id.
    ClearStandingOrder { order_id: u32 },

    /// Set the corporation's fleet doctrine (§16) — the constrained combat &
    /// logistics policy. Instant local administration; the server attaches the
    /// issuing player.
    SetFleetDoctrine { doctrine: FleetDoctrine },

    /// Build a ship at one of the player's owned systems (§step1 growth sink) — costs
    /// a commodity recipe from that system's stockpile and completes over time.
    BuildShip { system_id: EntityId, ship_kind: ShipKind },

    /// Develop one of the player's owned systems (§step1 structure sink), e.g. an
    /// Extractor tier that raises its output — costs a recipe, completes over time.
    DevelopSystem { system_id: EntityId, upgrade: SystemUpgrade },

    /// Application-level keepalive (optional; the client may send periodically).
    Ping,
}

/// One of the player's own resting limit orders.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct OrderView {
    pub id: u64,
    pub side: Side,
    pub commodity: Commodity,
    pub units: u32,
    pub limit_price: f64,
}

/// A standing price the player reads off the (lagged) hub ticker.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PriceView {
    pub commodity: Commodity,
    pub price: f64,
}

/// The hub Exchange as the player sees it — prices **light-delayed** from the
/// hub (§9). `staleness` is how old the ticker is (the hub→command-center light
/// delay); execution still happens at the true current price, so the displayed
/// price is only a guide.
#[derive(Debug, Clone, Serialize)]
pub struct MarketView {
    pub prices: Vec<PriceView>,
    pub staleness: f64,
}

/// One commodity holding in the player's wallet.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct InvSlot {
    pub commodity: Commodity,
    pub units: u32,
}

/// The player's own treasury + holdings + resting limit orders (own state,
/// shown fresh).
#[derive(Debug, Clone, Serialize)]
pub struct WalletView {
    pub credits: f64,
    /// Equity / net worth, from the slow valuation close (§9).
    pub valuation: f64,
    pub inventory: Vec<InvSlot>,
    pub orders: Vec<OrderView>,
    /// Total Fuel across all owned systems' stockpiles — the fleet's operating
    /// reserve (§step1 part 2). Owner-only (summed from owned systems only).
    pub fuel_total: f64,
}

/// Which side of a raid the recipient is on.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Attacker,
    Defender,
}

/// A delayed report of a raid outcome (§8), tailored to the recipient. Delivered
/// only once the light of the event has reached the player's command center, so
/// attacker and defender may receive it at different times.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RaidReport {
    pub outcome: RaidOutcome,
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub attacker_ship: EntityId,
    pub target_ship: EntityId,
    pub attacker_kind: ShipKind,
    pub target_kind: ShipKind,
    pub pos: Vec2,
    /// Sim time at which the battle resolved.
    pub at_time: f64,
    /// How long ago (light delay, seconds) — you are learning this stale news.
    pub age: f64,
    /// The recipient's side.
    pub you: Role,
}

/// Severity of a check-in timeline entry — drives the client's colour/icon.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineSeverity {
    Good,
    Bad,
    Warn,
    Info,
}

/// One entry in the player's check-in timeline (§16, Layer 3): a discrete thing
/// that became OBSERVABLE to them at `at_time` (their own clock — own economy is
/// instant, distant battles/rival claims arrive light-delayed). The server
/// composes the human-readable `text`; the client lists entries newest-first.
#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    /// Sim-time the news became observable to this player.
    pub at_time: f64,
    pub severity: TimelineSeverity,
    pub text: String,
}

/// One resource deposit on a system, as the client sees it (static geology,
/// public knowledge — prospecting/fog of deposits is deferred). Lets the client
/// render the frontier-richer gradient and the system's would-be production.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DepositView {
    pub resource: Commodity,
    /// Units produced per second at full extraction.
    pub richness: f64,
    /// Remaining reserves; `null` = renewable.
    pub reserves: Option<f64>,
}

/// A star system as static geography + geology: position, name, deposits, and
/// the credit cost to claim it. Sent once at join. Dynamic state (who owns it,
/// how much it has stockpiled) is light-gated and lives in [`SystemStateView`].
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
    pub deposits: Vec<DepositView>,
    pub claim_cost: f64,
}

/// Static galaxy geography, sent once at join. Never changes during a session
/// (systems don't move), so it doesn't need to be in the per-tick stream.
#[derive(Debug, Clone, Serialize)]
pub struct GalaxyInfo {
    pub hub: Vec2,
    pub radius: f64,
    /// Speed of light (sim units / s) — lets the client annotate light-delays.
    pub c: f64,
    /// Sensor detection radius each of the player's assets projects — lets the
    /// client draw its sensor coverage around its own ships + command center.
    pub sensor_range: f64,
    /// Raider cruise speed (sim units / s) — lets the client compute a CRUDE,
    /// drifting intercept estimate for a committed raid (rendered as a soft zone).
    pub raider_speed: f64,
    pub systems: Vec<SystemInfo>,
    /// What a player can BUILD at an owned system + each recipe's cost/time (§step1).
    /// Static (const recipes), sent once so the client renders costs without re-tx.
    pub build_options: Vec<BuildOptionView>,
}

/// A buildable thing and its recipe (§step1 growth sink), for the System-view UI.
/// `key` is a stable identifier the client maps back to a build command.
#[derive(Debug, Clone, Serialize)]
pub struct BuildOptionView {
    pub key: String,
    pub label: String,
    pub costs: Vec<StockSlot>,
    pub build_secs: f64,
}

/// One commodity in a system's stockpile (whole units), shown only to the owner.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct StockSlot {
    pub commodity: Commodity,
    pub units: u32,
}

/// An owner-only in-progress build at a system (§step1). `key` is what's building;
/// `complete_time` is the sim-time of completion (the client shows ETA = it − now).
#[derive(Debug, Clone, Serialize)]
pub struct BuildStateView {
    pub key: String,
    pub complete_time: f64,
}

/// The DYNAMIC, per-tick, light-gated state of a star system (companion to the
/// static [`SystemInfo`]). `owner` is revealed to rivals only once the claim's
/// light has reached the viewer's command center — the owner sees their own claim
/// instantly (§6). `stockpile` (accumulated production) is private: present only
/// for the owner. No information about a rival's holdings ever leaks.
#[derive(Debug, Clone, Serialize)]
pub struct SystemStateView {
    pub id: EntityId,
    pub owner: Option<PlayerId>,
    pub stockpile: Option<Vec<StockSlot>>,
    /// Owner-only: the in-progress build at this system (§step1), if any. Like
    /// `stockpile`, never present for a rival — build state never leaks.
    pub build: Option<BuildStateView>,
    /// Number of Extractor upgrades built here (visible to all once the system is
    /// known — it's part of the system's observable development, not private intel).
    pub extractor_tier: u32,
}

/// A convoy's cargo manifest, as revealed to a player whose sensors are within
/// range (Tier 2). Absent from the ghost when out of sensor coverage.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CargoView {
    pub commodity: Commodity,
    pub units: u32,
}

/// A home anchor as a player perceives it. `pos` is static geography; `owner`
/// is light-gated by the view filter — it is `None` to a player until the light
/// of the claim event has reached their command center (a rival's presence must
/// not be learned faster than light).
#[derive(Debug, Clone, Serialize)]
pub struct AnchorView {
    pub pos: Vec2,
    pub owner: Option<PlayerId>,
}

/// A ship as a player perceives it: a delayed "ghost" — the position the light
/// now arriving at their command center shows, plus how stale that is and how
/// much the object could have moved since (§6). This is the ONLY ship
/// information a player receives; never the true present state, never another
/// player's view. Deliberately omits the ship's standing order (internal truth).
#[derive(Debug, Clone, Serialize)]
pub struct GhostView {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: ShipKind,
    /// Where the object was when the arriving light left it (retarded position).
    pub pos: Vec2,
    /// Velocity at that retarded moment (for heading / dead-reckoning).
    pub vel: Vec2,
    /// Light delay in seconds — how stale this sighting is ("seen Xs ago").
    pub age: f64,
    /// Radius (sim units) the object could have moved since the light left:
    /// `age · max_speed`. Applies to EVERY object alike, including your own ships
    /// (§6) — there is no FTL tether to your fleet, so certainty tracks PROXIMITY
    /// to the command center, not ownership: a ship near home is fresh and
    /// near-certain (age≈0), a distant own ship is fogged like an enemy at the
    /// same range. Drives the on-map uncertainty cone.
    pub uncertainty: f64,
    /// True if this is one of the viewing player's own ships.
    pub own: bool,
    /// The convoy's broadcast route (waypoints), light-delayed like its
    /// position. `None` for raiders (they don't broadcast).
    pub route: Option<Vec<Vec2>>,
    /// The convoy's cargo — present ONLY when this convoy is within the viewing
    /// player's sensor coverage (Tier 2). `None` out of range, or for raiders.
    pub cargo: Option<CargoView>,
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

    /// The player's per-tick delayed/fogged view of the world, computed from
    /// THEIR command center (§6). Every player receives a *different* one; none
    /// receives true positions, another player's view, or any presence
    /// information faster than light. (Deliberately carries no global
    /// player-count — that would leak join/leave instantly; the client derives
    /// "corps in view" from the fair, light-delayed ghosts it can see.)
    View {
        tick: u64,
        sim_time: f64,
        /// This player's command center — the origin of their light-cone.
        command_center: Vec2,
        /// Home anchors; each owner is light-gated (see [`AnchorView`]).
        anchors: Vec<AnchorView>,
        /// Star systems' dynamic state: ownership light-gated to rivals, stockpile
        /// shown only to the owner (see [`SystemStateView`]).
        systems: Vec<SystemStateView>,
        /// Ships as delayed ghosts from this player's vantage.
        ghosts: Vec<GhostView>,
        /// The hub ticker, light-delayed (§9).
        market: MarketView,
        /// The player's own credits + holdings (fresh).
        wallet: WalletView,
        /// The player's own standing logistics orders (§15) — fresh (own private
        /// policy, not light-gated, like the wallet). Lets the client list/edit them
        /// and show what's running automatically.
        standing_orders: Vec<StandingOrder>,
        /// The player's own fleet doctrine (§16) — fresh private policy (like the
        /// wallet), so the client can display and edit it.
        doctrine: FleetDoctrine,
    },

    /// A delayed raid report (§8) — arrives on the recipient's own clock.
    Report { report: RaidReport },

    /// The player's check-in timeline (§16, Layer 3) — the retained digest of what
    /// became OBSERVABLE to them, buffered across disconnects. Sent on connect (the
    /// "welcome back" digest) and again whenever it grows. `away_since` is the
    /// sim-time they were last online, so the client can split "while you were
    /// away" from earlier entries. Awareness only — never new information, never
    /// faster than light.
    Timeline { entries: Vec<TimelineEntry>, away_since: f64 },

    /// Economy news for this player (§9): a buy settled, a delivery arrived, a
    /// sell was dispatched or cleared.
    Trade { trade: TradeEvent },

    /// Feedback for an order the player just issued — the OUTBOUND command in
    /// flight (§6, "commanding into the past"). Sent immediately to the issuing
    /// player, carrying authoritative sim-times:
    ///   * `depart_time` — the order leaves the command center;
    ///   * `arrive_time` — it reaches the ship (as the player observes it): the
    ///     violet comet travels command-center → ghost over this window.
    ///
    /// This is the one thing the MAP can't show — your command crossing space,
    /// not yet arrived. The ship's *reaction* needs no signal: the player simply
    /// sees the ghost change course on the map when its light arrives (the map IS
    /// the inbound channel). Both times derive from the player's OBSERVED light
    /// delay to the ship (its ghost staleness), so nothing reveals true distance.
    CommandSignal {
        ship_id: EntityId,
        depart_time: f64,
        arrive_time: f64,
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
