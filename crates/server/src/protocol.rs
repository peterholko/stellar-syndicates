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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sim::{
    Commodity, CountClass, EngagementPolicy, EngagementPosture, EntityId, FleetDoctrine, OrderKind,
    PlayerId, RaidOutcome, RankingRow, ShipKind, Side, StandingOrder, StructureKind, SyndicateId,
    TradeEvent, TransitMode, Vec2,
};

/// The client↔server wire protocol version. BUMPED to 3 by the §SYNDICATES
/// change: `GhostView` + `SystemStateView` gained an `ally` flag (light-delayed
/// membership knowledge → friendly tint), the per-player view gained a
/// `syndicate` roster + pending `syndicate_invites`, and new alliance-admin
/// `ClientMsg`s were added. (v2 = §FLEETS: `count_class` + `composition`.)
/// A client seeing an unexpected version can warn the user to refresh; the
/// server sends it in [`ServerMsg::Welcome`].
/// (v4 = §battle-records: the per-player view gained `battle_records` — the
/// light-gated, fidelity-tiered replay timeline for each observable battle.)
pub const PROTOCOL_VERSION: u32 = 7;

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

    /// Buy at the Charterhouse Exchange (§9, §TCA): instant settlement into the
    /// corp's warehouse. `ship_to` optionally hands the lot straight to Authority
    /// freight for one of the corp's owned systems (the one-checkbox composition);
    /// serde default so older clients still parse.
    MarketBuy {
        commodity: Commodity,
        units: u32,
        #[serde(default)]
        ship_to: Option<EntityId>,
    },

    /// Sell at the Charterhouse Exchange (§9, §TCA): draws from the corp's
    /// warehouse and settles instantly at the standing price.
    MarketSell { commodity: Commodity, units: u32 },

    /// §TCA Part 5: player-convoy logistics — load/unload across the Charterhouse
    /// warehouse or an owned system's stockpile, and the haul order that sends a
    /// loaded hull to the Charterhouse (optionally selling on arrival).
    HubLoad { fleet_id: EntityId, commodity: Commodity, units: u32 },
    HubUnload { fleet_id: EntityId },
    SystemLoad { fleet_id: EntityId, system: EntityId, commodity: Commodity, units: u32 },
    SystemUnload { fleet_id: EntityId, system: EntityId },
    HaulToCharterhouse {
        fleet_id: EntityId,
        #[serde(default)]
        sell_on_arrival: bool,
    },

    /// §TCA Phase 2: buy charter standing back from the Authority (credits burned,
    /// clamped to the ceiling — you pay only for points actually restored).
    PayReinstatement { points: f64 },

    /// §TCA: toggle whether one of the player's BLOCKADING fleets also engages
    /// Authority freight arriving at the strangled system. Instant local policy.
    SetEngageFreight { fleet_id: EntityId, on: bool },

    /// §TCA: book OUTBOUND Authority freight — warehouse → an owned system.
    BookFreightOut { system: EntityId, commodity: Commodity, units: u32 },

    /// §TCA: book INBOUND Authority freight — an owned system → the warehouse,
    /// optionally sold at the Exchange the moment it lands.
    BookFreightIn {
        system: EntityId,
        commodity: Commodity,
        units: u32,
        #[serde(default)]
        sell_on_arrival: bool,
    },

    /// Place a resting limit order; it clears in the periodic batch (§9).
    PlaceLimitOrder { side: Side, commodity: Commodity, units: u32, limit_price: f64 },

    /// Ship a claimed system's accumulated production to the hub to sell (§9) —
    /// spawns raidable convoys from the system.
    ShipProduction { system_id: EntityId },

    /// Supply a system: move goods from the corp's HUB WAREHOUSE into an owned
    /// system's stockpile via a raidable convoy sailing from the hub — the bridge
    /// that lets market-bought inputs feed a system's converters, and the free
    /// (but interceptable) alternative to booking Authority freight.
    StockSystem { system_id: EntityId, commodity: Commodity, units: u32 },

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
    /// `join` (optional) names a fleet docked at that system for the finished ship
    /// to JOIN; omitted / `null` forms a new fleet-of-one (§FLEETS management v1).
    BuildShip {
        system_id: EntityId,
        ship_kind: ShipKind,
        #[serde(default)]
        join: Option<EntityId>,
        /// §modules Part B4: the loadout to fit the ship with at build — must be
        /// ≤ the hull's slots and covered by the system's module ledger (both
        /// debited). serde default = unfitted (old clients build stock hulls).
        #[serde(default)]
        loadout: sim::Loadout,
    },

    /// Develop one of the player's owned systems (§step1 structure sink), e.g. an
    /// Extractor tier that raises its output — costs a recipe, completes over time.
    DevelopSystem {
        system_id: EntityId,
        upgrade: StructureKind,
        /// §bodies: the body to build on; omitted (old clients) auto-sites.
        #[serde(default)]
        body_id: Option<u32>,
    },

    /// §economy Part 3: post workforce crews (and §Part 4: specialists from the
    /// resident pool) to a structure at one of the player's owned systems (all
    /// zero clears the line). Instant local administration; clamps server-side.
    SetAssignment {
        system_id: EntityId,
        structure: StructureKind,
        workers: u32,
        #[serde(default)]
        specialists: BTreeMap<sim::SpecialistKind, u32>,
        /// §bodies: the body whose line this staffs; omitted targets the holder.
        #[serde(default)]
        body_id: Option<u32>,
    },

    /// §economy Part 4: sign a Sol specialist contract — credits now, a
    /// personnel convoy hub → dest (sub-light, raidable, manifest fogged).
    HireSpecialist { specialist: sim::SpecialistKind, dest_system: EntityId },

    /// §economy Part 4: enqueue an Academy training course (needs Academy ≥ 1).
    TrainSpecialist { system_id: EntityId, specialist: sim::SpecialistKind },

    /// §economy Part 4: carry resident specialists between owned/allied systems
    /// on a dedicated personnel convoy.
    TransferSpecialists { from: EntityId, to: EntityId, manifest: BTreeMap<sim::SpecialistKind, u32> },

    /// §modules Part B3: manufacture one module into the system's ledger (needs an
    /// Armaments Complex ≥ 1). Costs goods; rides the build queue.
    BuildModule { system_id: EntityId, module: sim::ModuleKind },

    /// §modules Part B4: refit `n` ships of `ship`/`from` in a docked fleet to a
    /// new `to` loadout at a Shipyard the player owns or is allied with.
    RefitShips { fleet_id: EntityId, ship: ShipKind, from: sim::Loadout, to: sim::Loadout, n: u32 },

    /// §modules Part B3: ship modules between owned/allied systems on a crate convoy.
    TransferModules { from: EntityId, to: EntityId, manifest: BTreeMap<sim::ModuleKind, u32> },

    /// §modules Part B3: buy `n` modules from Sol (price-certain, delivery-risky) —
    /// a crate convoy carries them to the player's `dest_system`.
    BuyModule { module: sim::ModuleKind, n: u32, dest_system: EntityId },

    /// §modules Part B3: sell `n` modules from `from_system` to Sol — a convoy
    /// carries them to the hub and the buy-back clears on arrival.
    SellModule { module: sim::ModuleKind, n: u32, from_system: EntityId },

    /// WITHDRAW an engaged fleet from its battle (§battles-take-time) — a coarse,
    /// light-delayed break-off order.
    Withdraw { fleet_id: EntityId },

    /// Set a fleet's TRANSIT throttle (§Part 4): Full or Stealth. Instant local
    /// administration on the player's own fleet.
    SetFleetTransit { fleet_id: EntityId, mode: TransitMode },

    /// Ask for a PROJECTED engagement estimate (§FLEETS Part 3): if `attacker`
    /// (one of the player's fleets) raided `target`, what would the losses be?
    /// Computed from the player's OWN view data only — exact where they have
    /// sensor coverage, an honest typical-hull estimate where they don't.
    EstimateEngagement { attacker: EntityId, target: EntityId },

    /// Merge one of the player's fleets INTO another (§FLEETS management v1). Both
    /// must be the player's, idle, and docked together at an owned system.
    MergeFleets { into: EntityId, from: EntityId },

    /// Split ships off one of the player's fleets into a new fleet at an owned
    /// system (§FLEETS management v1). `counts` = how many of each kind to detach.
    SplitFleet { fleet_id: EntityId, counts: BTreeMap<ShipKind, u32> },

    /// BLOCKADE a rival system (§contestable-territory Part 1): order one of the
    /// player's fleets (must contain a raider) to take station on a rival's
    /// system and strangle its logistics. Light-delayed like a move order.
    BlockadeSystem { fleet_id: EntityId, system_id: EntityId },

    /// SURVEY a system's exact geology (§explore Part 2): order one of the
    /// player's fleets (must contain a Scout) to fly on-site and dwell. Valid on
    /// ANY system (pre-siege prospecting intended). Light-delayed like a move.
    SurveySystem { fleet_id: EntityId, system_id: EntityId },

    /// ATTACK a rival fleet (§offensive-orders Part 1) — the targeted destroy verb.
    /// Orderable on any rival fleet; the attacker must contain a raider. Light-
    /// delayed like a raid; on contact it's a FULL battle (destroy, cargo lost),
    /// unlike CommitRaid (steal).
    AttackFleet { fleet_id: EntityId, target_id: EntityId },

    /// Set a fleet's ENGAGEMENT POSTURE (§offensive-orders Part 2): Passive /
    /// Defensive / WeaponsFree. Instant local administration on the player's own
    /// fleet (a standing per-fleet policy, like SetFleetTransit).
    SetFleetPosture { fleet_id: EntityId, posture: EngagementPosture },

    // ---- SYNDICATES (§syndicates Part 1) -------------------------------------
    /// FOUND a syndicate with the caller as founder. The server attaches the
    /// issuing player.
    CreateSyndicate { name: String },
    /// INVITE a corp (BY NAME — resolved server-side to its stable id) into the
    /// caller's syndicate. Founder-only; ignored if the name isn't a joined corp.
    InviteToSyndicate { name: String },
    /// ACCEPT a pending invitation to the named syndicate.
    AcceptSyndicateInvite { syndicate_id: SyndicateId },
    /// LEAVE the caller's syndicate.
    LeaveSyndicate,
    /// DISSOLVE the caller's syndicate (founder-only).
    DissolveSyndicate,

    // ---- RESEARCH (§research R6) --------------------------------------------
    /// SET the caller's syndicate research QUEUE (ordered programme ids). The
    /// front promotes to the active programme; the sim validates + soft-rejects
    /// unknown/hidden/completed ids. CC-local, no positional delay.
    SetResearchQueue { queue: Vec<String> },

    // ---- FITTING (§fitting Stage A) ------------------------------------------
    /// SAVE a doctrine fit (named hull + loadout) on the caller's syndicate.
    /// The sim validates slots + fitting budget; same-name replaces. CC-local.
    SaveFit { name: String, ship: ShipKind, #[serde(default)] loadout: sim::Loadout },
    /// DELETE a doctrine fit by name from the caller's syndicate. CC-local.
    DeleteFit { name: String },
    /// §ladder B4: NAME the syndicate's flagship Titan (empty un-christens).
    NameFlagship { name: String },

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
    /// §TCA: goods held at the WORMHOLE HUB — the only stock the Exchange trades
    /// against, and one of exactly two places a corporation's goods can sit (the
    /// other being each owned system's stockpile). Owner-only.
    pub warehouse: Vec<InvSlot>,
    pub orders: Vec<OrderView>,
    /// Total Fuel across all owned systems' stockpiles — the fleet's operating
    /// reserve (§step1 part 2). Owner-only (summed from owned systems only).
    pub fuel_total: f64,
}

/// §TCA Phase 2: the viewer's own CHARTER STANDING with the Authority, and the
/// band it derives. OWNER-ONLY — your legal standing is between you and the
/// Charterhouse; rivals learn of your offenses only from the PUBLIC citations
/// that travel at lightspeed, never by reading your record.
#[derive(Debug, Clone, Serialize)]
pub struct CharterView {
    pub standing: f64,
    pub max_standing: f64,
    /// The derived band ("good_standing" … "proscribed").
    pub status: sim::CharterStatus,
    /// Human title of the band, for the status chip.
    pub title: &'static str,
    // (§perf Part B: the static band LADDER moved to Welcome — it is a constant
    // table and had been re-sent inside every 10 Hz View.)
    /// Freight-fee multiplier currently applied (1.0 in good standing).
    pub tariff_mult: f64,
    /// Exchange penalty fee currently applied, as a fraction of trade value
    /// (0.0 in good standing — good-standing players pay nothing).
    pub market_penalty_frac: f64,
    /// Credits per standing point to buy back through reinstatement.
    pub reinstate_cost_per_point: f64,
}

/// §TCA: one entry of an Authority freighter's MANIFEST, as a viewer may read it.
/// The hull broadcasts under the Convention, but the manifest is TWO-TIER, per
/// entry: the entry's OWNER always sees their own lot (it is their property), and
/// anyone else sees it only from inside sensor range — the same rule that governs
/// a convoy's cargo. So a rival watching a freighter go by learns nothing about
/// who is shipping what until they get close enough to look.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ManifestEntryView {
    pub owner: PlayerId,
    pub commodity: Commodity,
    pub units: u32,
    pub direction: sim::ShipmentDir,
    /// True when this entry is the viewer's own lot.
    pub mine: bool,
}

/// §TCA: one of the viewer's freight shipments — queued for a departure or
/// already aboard an Authority freighter. OWNER-ONLY: a player sees only their
/// own lots, never anyone else's.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ShipmentView {
    pub id: u64,
    /// The destination (outbound) or origin (inbound) system.
    pub system: EntityId,
    pub commodity: Commodity,
    pub units: u32,
    pub direction: sim::ShipmentDir,
    pub sell_on_arrival: bool,
    pub fee_paid: f64,
    pub booked_at: f64,
    /// `false` = still queued at the Charterhouse; `true` = aboard a freighter.
    pub aboard: bool,
}

/// §TCA: the Authority's freight TERMS for one of the viewer's owned systems —
/// everything the client needs to price and time a booking BEFORE committing.
/// Deterministic: the departure phase and the freighter's constant cruise are
/// pure functions of the config, so these are exact, not estimates.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FreightTermsView {
    pub system: EntityId,
    /// Charterhouse → system distance (sim units).
    pub distance: f64,
    /// Whether the system has a Depot (bigger cap, discounted fee).
    pub depot: bool,
    /// Max units this corp may load to this destination per departure.
    pub cap: u32,
    /// Flight time one way (seconds) — add to a departure for the outbound ETA.
    pub secs_out: f64,
    /// Flight time out AND back (seconds) — an inbound lot's total after departure.
    pub secs_round: f64,
}

/// §TCA: the Charterhouse freight desk — the timetable, the fee formula's inputs,
/// the viewer's own shipment queue. Owner-only.
#[derive(Debug, Clone, Serialize)]
pub struct FreightView {
    /// Sim-time of the next scheduled departure (exact).
    pub next_departure: f64,
    /// Seconds between departures.
    pub period: f64,
    /// Fee = units × (price × `fee_frac` + distance × `fee_per_unit_dist`),
    /// then × `depot_fee_mult` if the destination has a Depot. Exposed as inputs
    /// (not a per-commodity table) so the client prices any lot live off the
    /// ticker; `freight_view_terms_price_a_lot_exactly` guards the contract.
    pub fee_frac: f64,
    pub fee_per_unit_dist: f64,
    pub depot_fee_mult: f64,
    /// Terms for each system the viewer currently owns (the valid destinations).
    pub terms: Vec<FreightTermsView>,
    /// The viewer's own lots, queued and aboard.
    pub shipments: Vec<ShipmentView>,
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
/// attacker and defender may receive it at different times. §FLEETS Part 2: now
/// a composition-vs-composition report — `attacker_kind`/`target_kind` are the
/// flagships, and `*_losses` carry the per-kind ships each side lost.
#[derive(Debug, Clone, Serialize)]
pub struct RaidReport {
    /// §battle-aftermath: stable id shared with the RETAINED copy of this
    /// report (`View.battle_reports`) — the news toast and the map marker /
    /// results panel can point at the same battle.
    pub report_id: u64,
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
    /// Per-kind ships the attacker lost over the engagement.
    pub attacker_losses: Vec<CompCount>,
    /// Per-kind ships the defender (target side) lost over the engagement.
    pub target_losses: Vec<CompCount>,
}

/// §battle-aftermath: a RETAINED concluded-battle report, as one participant
/// learned it — powers the aftermath map marker and the battle-results panel.
/// Owner-only by construction (built per player from their retained journal).
#[derive(Debug, Clone, Serialize)]
pub struct BattleReportView {
    /// Stable id (shared with the transient `Report` news toast for the same
    /// battle, and usable by the timeline to open the same results).
    pub id: u64,
    pub pos: Vec2,
    /// Sim-time the battle CONCLUDED (what happened, when).
    pub at_time: f64,
    /// Sim-time this player's conclusion light ARRIVED (when you learned).
    pub learned_at: f64,
    /// The recipient's side in it.
    pub you: Role,
    pub attacker_kind: ShipKind,
    pub target_kind: ShipKind,
    pub outcome: RaidOutcome,
    /// Composition-vs-composition per-kind losses, as this side learned them.
    pub attacker_losses: Vec<CompCount>,
    pub target_losses: Vec<CompCount>,
}

/// §contestable-territory Part 2: a RETAINED capture report, as one participant
/// learned it — powers the capture aftermath marker + results panel. Owner-only
/// by construction (per participant). `captor` = you took the system; else you
/// lost it. `plunder` is the seized stockpile the captor gained / the old owner
/// lost (the defender's report itemizes it).
#[derive(Debug, Clone, Serialize)]
pub struct CaptureReportView {
    pub id: u64,
    pub pos: Vec2,
    /// Sim-time the system FLIPPED.
    pub at_time: f64,
    /// Sim-time THIS player's light arrived (when you learned).
    pub learned_at: f64,
    pub captor: bool,
    pub plunder: Vec<StockSlot>,
}

/// A PROJECTED engagement estimate (§FLEETS Part 3), computed by running the
/// SAME shared Lanchester attrition forward on the observer's own view data. It
/// is honest about staleness: `composition_age` is how old the target sighting
/// is, `defenses_age` how old the scout snapshot of its fortifications is, and
/// `target_known = false` means the target was OUT of sensor coverage so the
/// projection assumed a typical warfleet of the estimated bucket size (never the
/// true count). Deterministic; never touches authoritative state.
#[derive(Debug, Clone, Serialize)]
pub struct EngagementEstimate {
    pub attacker: EntityId,
    pub target: EntityId,
    /// Projected per-kind losses on each side.
    pub own_losses: Vec<CompCount>,
    pub target_losses: Vec<CompCount>,
    /// Projected survivors on each side.
    pub own_survivors: Vec<CompCount>,
    pub target_survivors: Vec<CompCount>,
    /// True if the target's exact composition was known (in sensor coverage);
    /// false if the projection used the bucket-midpoint typical-hull assumption.
    pub target_known: bool,
    /// The target's estimated-size bucket (always available).
    pub target_count_class: CountClass,
    /// Age of the target sighting the estimate is built on (seconds).
    pub composition_age: f64,
    /// Age of the scouted-defenses snapshot folded in, if any (seconds).
    pub defenses_age: Option<f64>,
    /// Scouted platform tiers folded into the target, if a snapshot covered it.
    pub platform_tiers: Option<u32>,
    /// §tactical T4: the Monte Carlo readout — attacker-favorable fraction over
    /// `runs` rollouts of the REAL engine on derived seeds ("68% favorable").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<u32>,
    /// Per-kind 25th–75th percentile loss bands ("expected losses 4–7
    /// Corvettes"). Predictive Plots research widens the DISPLAY of these —
    /// it never invents math.
    #[serde(default)]
    pub own_loss_bands: Vec<LossRange>,
    #[serde(default)]
    pub target_loss_bands: Vec<LossRange>,
}

/// §tactical T4: one per-kind loss band (25th–75th percentile of the rollouts).
#[derive(Debug, Clone, Serialize)]
pub struct LossRange {
    pub kind: ShipKind,
    pub lo: u32,
    pub hi: u32,
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

/// One resource deposit on a system, as the client sees it. §explore: NO LONGER
/// public — the exact geology is CORP KNOWLEDGE (surveyed-or-owner), shipped
/// per-player in [`SystemStateView::deposits`]; the public spectral read is the
/// richness `band` on [`SystemInfo`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DepositView {
    pub resource: Commodity,
    /// Units produced per second at full extraction.
    pub richness: f64,
    /// Remaining reserves; `null` = renewable.
    pub reserves: Option<f64>,
}

/// A star system as static PUBLIC geography: position, name, and the richness
/// BAND (§explore R1 — the free spectral read; Poor/Fair/Rich by galaxy-wide
/// terciles, same for everyone, never changes). Sent once at join. The exact
/// geology is per-corp knowledge in [`SystemStateView::deposits`]; dynamic state
/// (owner, stockpile) is light-gated there too.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
    /// §explore: the public richness band slug — "poor" | "fair" | "rich".
    pub band: &'static str,
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
    /// The sensor-bubble multiplier a SCOUT projects over the standard ship
    /// bubble (§scout) — for the client's coverage rendering.
    pub scout_sensor_mult: f64,
    /// Sensor-array bubble tunables (§buildings step 2b): a tier-N array projects
    /// `base + per_tier · (N−1)` — lets the client draw its own arrays' coverage.
    pub sensor_array_base: f64,
    pub sensor_array_per_tier: f64,
    /// Defense Platform protection radius (§buildings step 2c) — lets the client
    /// draw a subtle ring on the OWNER's own defended systems.
    pub defense_platform_radius: f64,
    /// §economy Part 2 colony tunables — for the owner-only colony readout:
    /// Provisions/s eaten per million population, population capacity per
    /// Habitat tier (millions), and growth (millions/s while Well Supplied).
    pub provisions_per_million_per_s: f64,
    pub pop_cap_per_habitat_tier: f64,
    pub pop_growth_per_s: f64,
    /// §economy Part 6: Sol's standing specialist contract price (credits) —
    /// for the hire panel.
    pub specialist_hire_cost: f64,
    /// §economy Part 3: the Fuel Refinery's converter rate — units of Fuel/s
    /// at tier-throughput 1.0 with full staffing (the basket is 1 Volatile per
    /// Fuel). For the owner-only refining hint until the Part-6 colony panel.
    pub fuel_refinery_rate: f64,
    /// §contestable-territory Part 2: how long (sim seconds) an unbroken,
    /// defense-suppressed siege must run before a colony ship can capture — the
    /// client renders siege progress against it.
    pub siege_secs: f64,
    /// §pirates: the reserved neutral PIRATE faction id (a `PlayerId`), so the
    /// client can label pirate contacts/raids/reports without a name lookup.
    pub pirate_id: PlayerId,
    /// §node: sim-time (seconds) at which every EXOTIC system AWAKENS into a
    /// capturable node — the client telegraphs the countdown from t=0.
    #[serde(default)]
    pub node_awakening_time: f64,
    /// §node: a node's region radius (sim units) — the client draws the holder's
    /// region ring and the "in-region" cue with it.
    #[serde(default)]
    pub node_region_radius: f64,
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

/// The BLOCKADE at a system as a participant learns it (§contestable-territory).
/// Only ever populated for the besieger and the owner (fog-safe — see the
/// `SystemStateView.blockade` doc). `by` names the blockading corp; `since` is
/// when the unbroken blockade began; `by_me` marks the viewer as the besieger.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BlockadeStateView {
    pub by: PlayerId,
    pub since: f64,
    pub by_me: bool,
    /// §Part 2 SIEGE: sim-time the (defense-suppressed) siege clock started, or
    /// null if the siege can't progress yet (defenses up, a garrison present, or
    /// a home system). The client shows progress = (now − siege_since) /
    /// `GalaxyInfo.siege_secs`; capture becomes possible at full progress.
    pub siege_since: Option<f64>,
}

/// An owner-only in-progress build at a system (§step1). `key` is what's building;
/// `complete_time` is the sim-time of completion (the client shows ETA = it − now).
#[derive(Debug, Clone, Serialize)]
pub struct BuildStateView {
    pub key: String,
    pub complete_time: f64,
    /// §bodies: the body this job builds on / displays at.
    pub body_id: u32,
}

/// A stored SCOUT-INTEL snapshot of a RIVAL system's fortifications (§scout
/// part 2), as delivered to the scout's owner — and to nobody else. It is a
/// SNAPSHOT: `observed_at` is when the scout saw it (the client ages it); it is
/// never live and never auto-updates — the rival may have built since.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct IntelView {
    pub defense_tier: u32,
    pub shipyard_tier: u32,
    /// Sim-time of the observation — T₁, the "as of T" the readout ages from
    /// (the ORIGINAL observation, even when relayed by an ally; §syndicates Part 2).
    pub observed_at: f64,
    /// §syndicates Part 2 relay PROVENANCE — present only for ALLY-sourced intel
    /// (`None` for your own direct scout). Who observed it, and the two chain
    /// legs: `relayed_at` = T₂ (the observation's light reached the ally's command
    /// center — the earliest they could relay), `received_at` = T₃ (the relayed
    /// report's light reached YOUR command center). The picture is honestly staler
    /// than the ally's by the inter-command-center distance, and NEVER upgrades to
    /// live truth — aging is always from T₁.
    #[serde(default)]
    pub relayed_by: Option<PlayerId>,
    #[serde(default)]
    pub relayed_at: Option<f64>,
    #[serde(default)]
    pub received_at: Option<f64>,
    /// §pirates: the scouted PIRATE ENCLAVE tier at this system (0 = not an
    /// enclave). When > 0 the site is a pirate base; `defense_tier` above is its
    /// platform-equivalent base defense (what an assault must grind down).
    #[serde(default)]
    pub enclave_tier: u32,
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
    /// Owner-only: the SOONEST in-progress build at this system (§step1), if
    /// any. Like `stockpile`, never present for a rival — build state never
    /// leaks. Kept alongside `builds` for the single-job consumers.
    pub build: Option<BuildStateView>,
    /// Owner-only: ALL in-progress builds at this system, ordered by completion
    /// (§build-progress — the sim has always allowed concurrent jobs; the view
    /// used to collapse them to the soonest). Same fog rule as `build`: a rival
    /// always sees an empty list.
    pub builds: Vec<BuildStateView>,
    /// Number of Extractor upgrades built here (visible to all once the system is
    /// known — it's part of the system's observable development, not private intel).
    pub extractor_tier: u32,
    /// Number of Depot upgrades built here (§buildings step 2) — owner-only.
    pub depot_tier: u32,
    /// Number of Shipyard upgrades built here (§buildings step 3) — owner-only.
    /// Gates ship construction: Convoy needs ≥ 1, Raider ≥ 2.
    pub shipyard_tier: u32,
    /// Number of Sensor Array upgrades built here (§buildings step 2b) —
    /// owner-only. Projects a standing sensor bubble for the owner.
    pub sensor_tier: u32,
    /// Number of Defense Platform tiers standing here (§buildings step 2c) —
    /// owner-only. A rival learns a platform exists only through engagement
    /// outcomes (delayed battle reports), never through the View.
    pub defense_tier: u32,
    /// Number of Habitat tiers here (§buildings step 3a) — owner-only.
    pub habitat_tier: u32,
    /// §economy Part 2: whether the colony is WELL SUPPLIED — owner-only (rivals
    /// always see false; a rival must never learn whether your colonies are
    /// hungry). Kept under the legacy wire name so the client's amber
    /// supply-trouble tint keeps working; `food_state` below has the full rung.
    pub habitat_fed: bool,
    /// §economy Part 2: the colony's food-ladder rung (slug: `well_supplied` /
    /// `rationing` / `critical` / `no_provisions`) — owner-only; rivals always
    /// see `well_supplied` (the vacuous rung — same fog rule as `habitat_fed`).
    pub food_state: String,
    /// §economy Part 2: colony POPULATION in millions — owner-only; rivals
    /// always see 0 (workforce/economy strength is private intel).
    pub population: f64,
    /// §economy Part 4: the RESIDENT SPECIALIST pool — owner-only; rivals
    /// always see an empty map (your talent is private intel).
    pub specialists: BTreeMap<sim::SpecialistKind, u32>,
    /// §modules Part B3: the system's MODULE LEDGER (kind → crates on hand) —
    /// owner-only; rivals always see an empty map (your armory is private intel).
    /// serde default keeps old clients parsing.
    #[serde(default)]
    pub modules: BTreeMap<sim::ModuleKind, u32>,
    /// §bodies: the system's PLANETS AND MOONS — roster public, deposits
    /// survey-gated, per-body owner data owner-only (see [`BodyView`]).
    pub bodies: Vec<BodyView>,
    /// §economy Part 6: every built STRUCTURE (slug → tier) — owner-only;
    /// rivals always see an empty map (the legacy per-kind tier fields above
    /// stay for the map's visual anchors).
    pub structures: BTreeMap<String, u32>,
    /// §economy Part 6: the colony's WORKFORCE — owner-only; None for rivals.
    pub workforce: Option<WorkforceView>,
    /// §economy Part 6: every production line with its RESOLVED factor chain
    /// (the shown-math law: output = base · throughput · staffing · skill ·
    /// food) — owner-only; rivals always see an empty list.
    pub assignments: Vec<AssignmentView>,
    /// §economy: per built-converter idle status for the system-view banner —
    /// owner-only (rivals always see an empty list). serde default keeps old
    /// clients parsing.
    #[serde(default)]
    pub converters: Vec<ConverterStatusView>,
    /// Number of Fuel Refinery tiers here (§buildings step 3b) — owner-only.
    pub refinery_tier: u32,
    /// BLOCKADE state (§contestable-territory Part 1), if this system is under
    /// blockade. Populated for the two PARTICIPANTS only, each light-honestly:
    /// the BESIEGER (`by`) sees it via their on-station fleet (no delay); the
    /// OWNER sees it once the onset light reaches their command center. Third
    /// parties get `None` here — they observe the fight via `battles`, and the
    /// eventual capture via the light-delayed ownership change.
    pub blockade: Option<BlockadeStateView>,
    /// Development slots USED at this system (built tiers + in-progress upgrade
    /// jobs) — owner-only, like `stockpile`; rivals always see 0 (§buildings step 1).
    pub slots_used: u32,
    /// The system's development slot budget — owner-only; rivals always see 0.
    /// (The budget derives from public geology, but exposing it only to the owner
    /// keeps the whole slots readout on one fog rule.)
    pub slots_total: u32,
    /// TOTAL storage capacity (units, summed across commodities; §buildings step 2)
    /// — owner-only; rivals always see 0.
    pub storage_cap: u32,
    /// Units currently stored against that cap — owner-only; rivals always see 0.
    /// (May exceed `storage_cap` for a grandfathered pre-cap stockpile.)
    pub storage_used: u32,
    /// The VIEWER'S own scout-intel snapshot of this (rival) system, if any —
    /// present only once the capture's light has reached the viewer's command
    /// center (§scout part 2). Never present on the viewer's own systems, and a
    /// scouted rival never sees anything here about being scouted.
    pub intel: Option<IntelView>,
    /// True if this system's (light-gated known) owner is a SYNDICATE ally as the
    /// viewer knows it (§syndicates Part 1). Drives the friendly ally tint; does
    /// NOT grant any owner-only data (stockpile/tiers stay private in Part 1).
    #[serde(default)]
    pub ally: bool,
    /// §syndicates Part 3: OWNER-ONLY — an ally GARRISON hosted at THIS system (the
    /// coalition shield you're feeding). Total ally garrison ships stationed here,
    /// and whether their Provisions upkeep is currently covered. `0` = none; rivals
    /// always see 0 (a private detail of your own system).
    #[serde(default)]
    pub ally_garrison_ships: u32,
    #[serde(default)]
    pub ally_garrison_fed: bool,
    /// §node: this system's EXOTIC NODE, if any. Present (light-gated with the
    /// system) whenever the system carries a node — the bonus slug + awakened flag
    /// are PUBLIC (an awakened node is a galaxy-wide landmark, and the exotic star
    /// is already visible to everyone). `None` for ordinary systems.
    #[serde(default)]
    pub node: Option<NodeStateView>,
    /// §explore R2: the EXACT deposit table — present iff the viewer has SURVEYED
    /// this system or OWNS it (survey knowledge is permanent; holding a system is
    /// knowing it). `None` = unsurveyed: the viewer gets only the public band.
    /// Never leaks a rival's survey state (each corp is gated on its OWN set).
    #[serde(default)]
    pub deposits: Option<Vec<DepositView>>,
    /// §explore R3: the system's HIDDEN TRAIT slug — CURRENT-OWNER-ONLY (rivals
    /// and past owners get `None`, always; traits are never telegraphed). A
    /// Bonus Vein carries its commodity as `bonus_vein:<commodity>`.
    #[serde(default, rename = "trait")]
    pub trait_: Option<String>,
}

/// §node: the per-system view of an EXOTIC NODE. The bonus + awakened state are
/// public (everyone sees the landmark); `fed` and `region_radius` are OWNER-ONLY
/// (your own logistics + the region ring only you command), so a rival sees them as
/// `false`/`0.0`.
#[derive(Debug, Clone, Serialize)]
pub struct NodeStateView {
    /// Stable bonus slug — `relay_anchor` | `veil` | `deep_scan` (for the label).
    pub bonus: String,
    /// Human bonus title (e.g. "Relay Anchor").
    pub title: String,
    /// Has the node awakened (past the awakening time)? Before it, the system is a
    /// telegraphed dormant landmark; after, a live capturable prize.
    pub awakened: bool,
    /// OWNER-ONLY: is the node's upkeep currently met? An unfed node's bonus is
    /// SUSPENDED. Rivals always see false.
    pub fed: bool,
    /// OWNER-ONLY: the node's region radius (sim units) for the holder's map ring;
    /// rivals see 0.0.
    pub region_radius: f64,
}

/// The viewer's SYNDICATE (§syndicates Part 1) — their own roster, delivered in
/// the per-player view so the client can render membership + manage it. Only
/// ever the viewer's OWN syndicate (never a rival's private roster).
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateView {
    pub id: SyndicateId,
    pub name: String,
    pub founder: PlayerId,
    /// Whether the VIEWER is the founder (may invite / dissolve).
    pub is_founder: bool,
    /// The roster (id + display name), member-id ordered for determinism.
    pub members: Vec<SyndicateMember>,
    /// Outstanding invites the founder has sent (names), for the roster panel.
    pub invited: Vec<String>,
    /// §fitting: the syndicate's saved DOCTRINE FITS (named hull + loadout;
    /// any member curates). Owner-only like the rest of the view.
    #[serde(default)]
    pub fits: Vec<FitView>,
    /// §ladder B4: the christened name of the syndicate's Titan (None if
    /// unnamed / not fielded). Owner-only here; rivals meet it only in
    /// participant battle records.
    #[serde(default)]
    pub flagship_name: Option<String>,
}

/// §fitting: one saved doctrine fit on the wire (modules sorted, never empty —
/// an all-stock "fit" isn't storable; stock is the absence of a fit).
#[derive(Debug, Clone, Serialize)]
pub struct FitView {
    pub name: String,
    pub kind: ShipKind,
    pub modules: Vec<sim::ModuleKind>,
}

/// One member of a [`SyndicateView`] roster.
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateMember {
    pub id: PlayerId,
    pub name: String,
}

/// A pending invitation the VIEWER may accept (§syndicates Part 1).
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateInviteView {
    pub id: SyndicateId,
    pub name: String,
}

/// §research R6: the viewer's SYNDICATE research picture — owner-only (design
/// law 3: nothing leaks to rivals). The whole 108-programme tree with per-node
/// state + gate progress, the active programme with its live rate and ETA, the
/// queue, and the per-Academy contribution table (shown math, law 2).
#[derive(Debug, Clone, Serialize)]
pub struct ResearchView {
    /// The programme the clock is accruing into, if any.
    pub active: Option<ActiveResearchView>,
    /// The queue-ahead ids (front is the active once promoted).
    pub queue: Vec<String>,
    /// Total live throughput rate (Σ supplied Academy rates); 0 if stalled/starved.
    pub rate: f64,
    /// True while an available active programme has no staffed Academy.
    pub stalled: bool,
    /// The per-Academy contribution rows (shown factor chains).
    pub academies: Vec<AcademyRow>,
    /// §perf Part B: the DYNAMIC per-node slice only (state + gate) — the static
    /// catalog (names/blurbs/topology/cost) went once in Welcome.
    pub programmes: Vec<ProgrammeDynView>,
}

/// §research R6: the active programme banner data.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveResearchView {
    pub id: String,
    pub name: String,
    /// Throughput-seconds accrued and the total cost.
    pub progress: f64,
    pub cost: f64,
    /// Seconds remaining at the current rate; None when the rate is 0.
    pub eta_secs: Option<f64>,
}

/// §research R6: one Academy's contribution row (the shown factor chain).
#[derive(Debug, Clone, Serialize)]
pub struct AcademyRow {
    pub system: String,
    pub body_id: u32,
    pub tier: u32,
    pub throughput: f64,
    pub staffing: f64,
    pub skill: f64,
    pub food: f64,
    pub rate: f64,
    /// False → the lab can't cover its drip this tick (amber in the UI).
    pub supplied: bool,
}

/// §perf Part B: one programme's STATIC catalog entry — names, blurbs, board
/// topology, cost. The same constant table for everyone (the game's rulebook,
/// not anyone's progress), sent ONCE in Welcome instead of per-member at 10 Hz.
#[derive(Debug, Clone, Serialize)]
pub struct ProgrammeInfo {
    pub id: String,
    /// Field slug ("propulsion" …) — the board this node lives on.
    pub field: String,
    /// School slug ("line_haul" …); None for the shared Tier I/II rungs.
    pub school: Option<String>,
    pub tier: u8,
    pub name: String,
    pub blurb: String,
    /// Cost in throughput-seconds (the ETA denominator context).
    pub cost: f64,
}

/// §research R6 / §perf Part B: one programme node's DYNAMIC slice — the
/// viewer's state + gate progress. The client joins it onto the static
/// [`ProgrammeInfo`] catalog by id.
#[derive(Debug, Clone, Serialize)]
pub struct ProgrammeDynView {
    pub id: String,
    /// One of: "completed" | "active" | "queued" | "available" | "locked".
    pub state: String,
    /// For a LOCKED node whose tier carries a verb/metric gate: the progress bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateProgressView>,
}

/// §research R6: a gate progress bar for a sealed tier.
#[derive(Debug, Clone, Serialize)]
pub struct GateProgressView {
    pub label: String,
    pub current: f64,
    pub threshold: f64,
}

/// §bodies: one PLANET OR MOON on the wire. The roster (id/name/kind/parent/
/// habitable) is public geography — a star's worlds are visible from afar;
/// DEPOSITS ride the survey knowledge ladder (None when unsurveyed);
/// structures/population are OWNER-ONLY (empty/zero for rivals — the fog law
/// one level down, nothing new leaks).
#[derive(Debug, Clone, Serialize)]
pub struct BodyView {
    pub id: u32,
    pub name: String,
    /// Kind slug: rocky | terrestrial | ocean | ice | gas_giant.
    pub kind: String,
    pub parent: Option<u32>,
    pub habitable: bool,
    /// This body's deposits — survey-gated like the system list.
    pub deposits: Option<Vec<DepositView>>,
    /// OWNER-ONLY: structures on this body (slug → tier); empty for rivals.
    pub structures: BTreeMap<String, u32>,
    /// OWNER-ONLY: population on this body (millions); 0 for rivals.
    pub population: f64,
}

/// §economy Part 6: a colony's workforce numbers — owner-only.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorkforceView {
    /// Workforce units the population fields (`floor(pop / 0.8M)`).
    pub units: u32,
    /// Crews posted across all assignments (may exceed `units` — every line
    /// then dilutes by the same share).
    pub posted: u32,
}

/// §economy Part 6: one production line, with the RESOLVED factor chain the
/// client shows verbatim (no hidden math). Owner-only.
#[derive(Debug, Clone, Serialize)]
pub struct AssignmentView {
    /// §bodies: the body whose line this is.
    pub body_id: u32,
    /// Structure slug (matches build keys / icons).
    pub structure: String,
    pub title: String,
    pub tier: u32,
    pub workers: u32,
    /// Specialists posted to this line (kind → n).
    pub specialists: BTreeMap<sim::SpecialistKind, u32>,
    /// Why the line is stopped (`no_food` / `no_inputs` / `storage_full`), if it is.
    pub suspended: Option<String>,
    /// The factor chain, resolved this tick.
    pub throughput: f64,
    pub staffing: f64,
    pub skill: f64,
    pub food: f64,
    /// Net output lines at those factors (commodity, units/s) — extraction
    /// lists each deposit's commodity; a converter lists its output.
    pub outputs: Vec<(Commodity, f64)>,
}

/// §economy: a built CONVERTER's live status, for the system-view idle banner:
/// `running`, or WHY it produces nothing — `needs_crew` (built but no crew
/// posted), or a staffed line's latched outage (`no_inputs` / `no_food` /
/// `storage_full`). Owner-only; mirrors the tick's converter gate so the banner
/// never contradicts what the sim actually did.
#[derive(Debug, Clone, Serialize)]
pub struct ConverterStatusView {
    pub body_id: u32,
    pub structure: String,
    pub title: String,
    pub tier: u32,
    /// running | needs_crew | no_inputs | no_food | storage_full
    pub status: String,
}

/// A convoy's cargo manifest, as revealed to a player whose sensors are within
/// range (Tier 2). Absent from the ghost when out of sensor coverage.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CargoView {
    pub commodity: Commodity,
    pub units: u32,
}

/// One of the player's in-flight order LIFECYCLES (§order-lifecycle), OWNER-ONLY.
/// The client derives the phase from `sim_time`: IN TRANSIT until `delivered_at`,
/// AWAITING ECHO until `echo_at`, then confirmed (the entry drops). Both stamps
/// are exact (computed at issue), so the client ticks precise countdowns with no
/// per-second server traffic.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PendingOrderView {
    pub fleet_id: EntityId,
    pub delivered_at: f64,
    pub echo_at: f64,
    pub kind: OrderKind,
}

/// An ongoing BATTLE as any observer perceives it (§battles-take-time), STRICTLY
/// light-gated: it appears only once the light of its start has reached the
/// viewer's command center. Weapons fire is loud — all participants (even dark
/// fleets) are revealed at the site by that same old light. `age` is how stale
/// the sighting is ("battle raging — as of N ago").
#[derive(Debug, Clone, Serialize)]
pub struct BattleView {
    /// The engagement's stable id — ONE battle entity, ONE map icon. Merging
    /// reinforcements join the same entity, so the id (and the icon) is stable.
    pub id: EntityId,
    pub pos: Vec2,
    /// Light delay of the battle sighting (seconds) — `distance(pos, cc) / c`.
    pub age: f64,
    /// Sim-time the battle began (for the panel's observed-elapsed readout).
    pub started_at: f64,
    /// True if the viewer is one of the two sides (they read their own running
    /// losses by their own light via the delayed reports).
    pub own: bool,
    /// The battle's participant fleet ids — exactly the set revealed to any
    /// observer of the battle by the existing weapons-fire site-reveal (their
    /// ghosts are already sent). The client uses these to SUPPRESS each
    /// participant's own map marker (the icon carries the state) and to build
    /// the battle panel. No new information beyond the ghosts already revealed.
    pub participants: Vec<EntityId>,
}

// --- §battle-records Part A2: the light-gated, fidelity-tiered replay ---------
//
// Every observable battle carries a per-viewer replay: the ARRIVED-round prefix
// (round `i` unlocks when `round.tick·DT + |pos−cc|/c ≤ now`, the same light
// gate as the battle-news event) at the viewer's fidelity. A PARTICIPANT sees
// everything (their own posture only, never the opponent's); a THIRD PARTY who
// currently covers the site sees a BUCKETED spine (CountClass only, no dealt, no
// notes beyond joins/mutual-disengage); anyone else gets no record at all (just
// the existing news + wreck marker). Nothing beyond the light frontier ships.

/// A viewer's fidelity on a battle record.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleFidelity {
    /// A participant corp: exact counts, kills, damage dealt, and every beat.
    Participant,
    /// A third party covering the site: the bucketed spine only.
    Bucket,
}

/// One `(kind, strength)` entry of a recorded side. `exact` is present ONLY at
/// participant fidelity; `class` (the [`CountClass`] bucket) is ALWAYS present —
/// so a third party learns "4–7 Corvettes", never the true count (leak-safe).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RecordCount {
    pub kind: ShipKind,
    pub exact: Option<u32>,
    pub class: CountClass,
}

/// One side of a recorded battle as a viewer sees it.
#[derive(Debug, Clone, Serialize)]
pub struct SideRecordView {
    pub corp: PlayerId,
    /// The corp's engagement doctrine — OWNER-ONLY: `Some` iff this is the
    /// viewer's own side. A rival never learns your posture from the replay.
    pub posture: Option<EngagementPolicy>,
    pub platform_tiers: u32,
    pub initial: Vec<RecordCount>,
    /// §modules B5: the side's opening FITTED stacks (kind + modules + count) —
    /// PARTICIPANT ONLY (empty at bucket/none fidelity, a fog-safe leak guard).
    /// The client labels the side and types its salvos by dominant weapon family.
    #[serde(default)]
    pub loadouts: Vec<LoadoutStack>,
    /// §ladder B4: the christened name of this side's TITAN — PARTICIPANT ONLY
    /// and only when the side fielded one. How a rival ever meets the name.
    #[serde(default)]
    pub flagship_name: Option<String>,
}

/// A recorded beat, viewer-filtered. `kind` is the snake_case note tag; `side`
/// and `comp` accompany the beats that carry them (`joined`).
#[derive(Debug, Clone, Serialize)]
pub struct RoundNoteView {
    pub kind: String,
    pub side: Option<u8>,
    pub comp: Option<Vec<RecordCount>>,
}

/// One recorded round as a viewer sees it. Indexing is by side (0 = attackers,
/// 1 = defenders): `counts[s]` survivors of `s`, `kills[s]` losses of `s`.
#[derive(Debug, Clone, Serialize)]
pub struct RoundRecordView {
    pub tick: u64,
    pub counts: [Vec<RecordCount>; 2],
    pub kills: [Vec<RecordCount>; 2],
    /// Damage each side dealt — PARTICIPANT ONLY (`None` at bucket fidelity).
    pub dealt: Option<[f64; 2]>,
    pub notes: Vec<RoundNoteView>,
    /// §tactical T3: the round's TRUTH KEYFRAME (real positions, torpedo
    /// salvos, exact deaths) — PARTICIPANT ONLY, absent on old records (the
    /// client falls back to choreographed rendering).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<sim::combat::Keyframe>,
}

/// A battle's replay as one viewer perceives it — light-gated + fidelity-tiered.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordView {
    pub id: EntityId,
    pub pos: Vec2,
    pub system: Option<EntityId>,
    /// Sim-time the battle began (`started_tick × DT`) — the "as of N ago" line.
    pub started_at: f64,
    pub raid: bool,
    pub fidelity: BattleFidelity,
    /// Which side (0/1) is the viewer's own, if any — drives the owner-only
    /// posture display and the "you" framing.
    pub own_side: Option<u8>,
    pub sides: [SideRecordView; 2],
    /// The ARRIVED round prefix at the viewer's fidelity (light-gated per round).
    pub rounds: Vec<RoundRecordView>,
    /// Newest arrived round tick — the client's light frontier (rounds beyond it
    /// draw as the hatched "beyond your light cone" zone).
    pub light_frontier_tick: u64,
    /// The outcome, once the FINAL round's light has arrived — `None` while the
    /// battle is still (as far as this viewer's light shows) running.
    pub outcome: Option<RaidOutcome>,
}

/// §perf Part A: a record's per-viewer HEADER — everything except the rounds.
/// Sent once per record per connection (and again only if a christened flagship
/// name changes); the rounds then stream incrementally as their light arrives.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordHeader {
    pub pos: Vec2,
    pub system: Option<EntityId>,
    pub started_at: f64,
    pub raid: bool,
    pub fidelity: BattleFidelity,
    pub own_side: Option<u8>,
    pub sides: [SideRecordView; 2],
}

/// §perf Part A: one record's INCREMENT for one connection — only what that
/// connection hasn't received yet. `header` is present when the record is new to
/// the connection (or its header changed); `new_rounds` appends to what the
/// client already holds; `outcome` is sent exactly once, when its light arrives.
/// Everything inside is filtered per-viewer exactly as [`BattleRecordView`] was —
/// the cursor changes WHEN data ships, never WHAT a viewer may see.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordUpdate {
    pub id: EntityId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<BattleRecordHeader>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub new_rounds: Vec<RoundRecordView>,
    /// The viewer's current light frontier for this record (always fresh).
    pub light_frontier_tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<RaidOutcome>,
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

/// One (kind, count) entry of a fleet's exact composition — revealed only to
/// the owner, or to a rival whose sensors cover the fleet (Tier 2). Ordered by
/// kind for a stable wire form.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CompCount {
    pub kind: ShipKind,
    pub count: u32,
}

/// §modules Part B: one FITTED stack of a fleet — `n` ships of `kind` all carrying
/// `modules` (a canonical, sorted loadout; never empty — unfitted ships aren't
/// stacks). Revealed under the same rule as [`CompCount`].
#[derive(Debug, Clone, Serialize)]
pub struct LoadoutStack {
    pub kind: ShipKind,
    pub modules: Vec<sim::ModuleKind>,
    pub n: u32,
}

/// A FLEET as a player perceives it: a delayed "ghost" — the position the light
/// now arriving at their command center shows, plus how stale that is and how
/// much the object could have moved since (§6). This is the ONLY fleet
/// information a player receives; never the true present state, never another
/// player's view. Deliberately omits the fleet's standing order (internal truth).
///
/// The two-tier INTEL LADDER (GDD §13.1): `count_class` (an estimated-size
/// bucket) is present on every visible fleet — a far observer of a broadcasting
/// hammer knows roughly HOW BIG it is; `composition` (exact kinds + counts) is
/// revealed ONLY within sensor coverage (or for your own fleets), exactly like
/// cargo. You know a fleet is inbound and roughly its size long before you learn
/// what is IN it.
#[derive(Debug, Clone, Serialize)]
pub struct GhostView {
    pub id: EntityId,
    pub owner: PlayerId,
    /// The FLAGSHIP kind — what the fleet is drawn and named for (precedence
    /// colony > convoy > corvette > raider > scout). A fleet-of-one is that ship.
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
    /// §economy Part 4: specialist PASSENGERS aboard — part of the manifest,
    /// included under exactly the cargo rule below (empty = none visible).
    pub passengers: BTreeMap<sim::SpecialistKind, u32>,
    /// The convoy's cargo — present ONLY when this convoy is within the viewing
    /// player's sensor coverage (Tier 2). `None` out of range, or for raiders.
    pub cargo: Option<CargoView>,
    /// The estimated-size BUCKET (`1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+`). Always
    /// present on any visible fleet — the honest, un-invertible size estimate a
    /// fog observer gets even for a fleet far outside sensor coverage.
    pub count_class: CountClass,
    /// The EXACT composition (kinds + counts). Present ONLY for the viewer's own
    /// fleets, or a rival fleet inside sensor coverage (Tier 2). `None` otherwise
    /// — you have the size bucket but not the makeup. Never leaks the true count.
    pub composition: Option<Vec<CompCount>>,
    /// §modules Part B: the FITTED stacks (kind + modules + count). Present under
    /// exactly the `composition` rule — seeing the makeup reveals the fits. Only
    /// non-default stacks; the unfitted remainder = composition − Σ these.
    #[serde(default)]
    pub loadouts: Option<Vec<LoadoutStack>>,
    /// §modules Part B3: module CRATES aboard a transport convoy — fogged like
    /// `passengers` (empty = none visible). Part of the sensor-gated manifest.
    #[serde(default)]
    pub modules: BTreeMap<sim::ModuleKind, u32>,
    /// The dark fleet's DETECTION SIGNATURE (§Part 4) at the retarded moment — how
    /// LOUD it is (1.0 = a lone raider at full speed). Present only for DARK
    /// fleets; drives the client's flare/plume treatment. `None` for broadcasters.
    pub signature: Option<f64>,
    /// The fleet's ENGAGEMENT POSTURE (§offensive-orders Part 2) — OWNER-ONLY, so
    /// `Some(..)` for your own fleets and `None` for every rival (a standing
    /// per-fleet policy is private, like the corp doctrine; never leaks).
    pub posture: Option<EngagementPosture>,
    /// §explore Part 2: SURVEY DWELL progress (0..1) — OWNER-ONLY (your own
    /// fleet's live order state, like `posture`; a rival never sees it — they
    /// see only the louder signature under the normal detection rules). `None`
    /// when not dwelling.
    #[serde(default)]
    pub survey_progress: Option<f64>,
    /// True if this fleet's owner is a SYNDICATE ally as the viewer KNOWS it
    /// (§syndicates Part 1) — light-delayed membership (`World::known_ally`), so a
    /// fresh join/leave isn't seen early. Drives the friendly ally tint/pip.
    #[serde(default)]
    pub ally: bool,
    /// §syndicates Part 3: OWNER-ONLY. When this is one of YOUR fleets stationed as
    /// an ally GARRISON, the host system it defends; `None` otherwise (and always
    /// for rivals — a private status). `garrison_fed` = the host is covering its
    /// Provisions upkeep (else its defense contribution is suspended).
    #[serde(default)]
    pub garrison_host: Option<EntityId>,
    #[serde(default)]
    pub garrison_fed: bool,
    /// §pirates: this fleet belongs to the neutral PIRATE faction (a raider pack) —
    /// drives the distinct hostile-neutral tint. Hostile to everyone.
    #[serde(default)]
    pub pirate: bool,
    /// §TCA: this fleet is a Terran Charter Authority FREIGHTER — the scheduled
    /// common carrier. Drives its own neutral tint, distinct from a corp convoy.
    #[serde(default)]
    pub tca: bool,
    /// §TCA: the freighter's MANIFEST as this viewer may read it — their own lots
    /// always, everyone else's only from inside sensor range. Empty for any other
    /// fleet, and empty for a freighter a distant rival is merely watching.
    #[serde(default)]
    pub manifest: Vec<ManifestEntryView>,
    /// Whether this ghost is close enough for the viewer to read its manifest /
    /// cargo (Tier 2). Set by the view filter; the game loop uses it to decide
    /// whether a rival's freight entries may be shown.
    #[serde(default)]
    pub revealed: bool,
    /// §TCA: OWNER-ONLY — whether this (blockading) fleet is set to engage
    /// Authority freight. `None` on anyone else's ghost, like `posture`.
    #[serde(default)]
    pub engage_freight: Option<bool>,
}

/// Messages pushed by the server to a single player's connection.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    /// Sent once, immediately after a successful join.
    Welcome {
        player_id: PlayerId,
        name: String,
        /// The wire protocol version ([`PROTOCOL_VERSION`]) — lets the client
        /// detect a stale build against a newer server.
        protocol_version: u32,
        /// Sim tick rate (Hz) — lets the client display time correctly.
        tick_hz: u32,
        tick: u64,
        sim_time: f64,
        galaxy: GalaxyInfo,
        /// §perf Part B: the charter band ladder — a static constant table
        /// (title, standing at/below which it applies), sent once here instead
        /// of inside every 10 Hz View.
        charter_ladder: Vec<(&'static str, f64)>,
        /// §perf Part B: the static research programme catalog (names, blurbs,
        /// board topology, costs) — the game's public rulebook, identical for
        /// everyone. The View carries only the per-node dynamic slice.
        research_catalog: Vec<ProgrammeInfo>,
    },

    /// The public star chart CHANGED after this client's Welcome: a join past
    /// the pre-generated home-slot pool MINTS a brand-new home system mid-run,
    /// and every connected client's galaxy snapshot predates it — without this
    /// refresh the new star is invisible and unselectable (not least to its own
    /// owner). Replaces the client's `galaxy.systems` wholesale; public
    /// geography only, so there is nothing to fog.
    GalaxyUpdate { systems: Vec<SystemInfo> },

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
        /// §TCA Phase 2: the player's OWN charter standing and band. Owner-only —
        /// rivals learn of offenses only through public citations, never by
        /// reading a corporation's record.
        charter: CharterView,
        /// §TCA: the Charterhouse freight desk — timetable, terms per owned
        /// destination, and the player's OWN shipment queue. Owner-only, fresh
        /// (it is the player's own administration, like the wallet).
        freight: FreightView,
        /// The player's own fleet doctrine (§16) — fresh private policy (like the
        /// wallet), so the client can display and edit it.
        doctrine: FleetDoctrine,
        /// The player's own in-flight ORDER LIFECYCLES (§order-lifecycle) — the
        /// two exact timestamps per pending order that let the client tick down IN
        /// TRANSIT → AWAITING ECHO. OWNER-ONLY private command data (like the
        /// wallet); a rival's view carries none of it.
        pending_orders: Vec<PendingOrderView>,
        /// Ongoing BATTLES visible to this player (§battles-take-time) — strictly
        /// light-gated; a third-party observer sees them only by their own light.
        battles: Vec<BattleView>,
        /// §syndicates Part 1: the viewer's OWN syndicate roster (fresh private
        /// state, like the wallet), or `None` if unaffiliated. Never a rival's.
        /// Boxed so this (already the largest) View variant stays lean; serde is
        /// transparent through the `Box`, so the wire JSON is unchanged.
        #[serde(default)]
        syndicate: Option<Box<SyndicateView>>,
        /// §syndicates Part 1: pending invitations the viewer may accept.
        #[serde(default)]
        syndicate_invites: Vec<SyndicateInviteView>,
        /// §research R6: the viewer's OWN syndicate research picture (owner-only,
        /// like the roster), or `None` if unaffiliated. Boxed to keep this variant
        /// lean; serde is transparent through the `Box`.
        #[serde(default)]
        research: Option<Box<ResearchView>>,
    },

    /// §perf Part B: the SLOW-MOVING per-player sections that used to ride every
    /// 10 Hz View — standing orders, retained battle/capture reports, and the
    /// published rankings. Sent per connection ONLY when a section's content
    /// changed (signature-gated, the timeline_sent pattern), on the RELIABLE
    /// discrete lane (the View's watch channel may drop frames for a slow
    /// client, which would lose a once-per-change section forever). A present
    /// field REPLACES the client's copy; an absent field means "unchanged". A
    /// fresh connection's first broadcast carries all four (empty signatures).
    Sections {
        #[serde(skip_serializing_if = "Option::is_none")]
        standing_orders: Option<Vec<StandingOrder>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        battle_reports: Option<Vec<BattleReportView>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        capture_reports: Option<Vec<CaptureReportView>>,
        /// §rankings: the PUBLISHED leaderboard — the same snapshot for every
        /// player (public by design), taken on the ledger close; it only changes
        /// on a close, which is exactly when it re-sends.
        #[serde(skip_serializing_if = "Option::is_none")]
        rankings: Option<Vec<RankingRow>>,
    },

    /// §perf Part A: incremental battle-record delivery (was: every record's full
    /// arrived prefix re-shipped inside every View). Rides the RELIABLE discrete
    /// lane (the bounded mpsc, like Timeline/Report) — the View's watch channel is
    /// last-write-wins and may drop frames for a slow client, which would lose a
    /// delta forever. The per-connection cursor advances only when the send
    /// succeeds, so a full queue simply retries next broadcast. `removed` names
    /// records this connection should drop: pruned server-side, or (bucket
    /// fidelity) the viewer's sensor coverage of the site lapsed — exactly the
    /// records that vanished from the old full-set View. A record that becomes
    /// visible again is re-sent in full (fresh cursor), as before.
    BattleRecords {
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        updates: Vec<BattleRecordUpdate>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        removed: Vec<EntityId>,
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

    /// A projected engagement estimate the player asked for (§FLEETS Part 3).
    EngagementEstimate(EngagementEstimate),

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
    // Guard the reserved NEUTRAL sentinel ids (PIRATE, TCA): a real name that
    // happened to hash onto one would be mistaken for the faction that owns pirate
    // packs / Authority freighters. A 1-in-2^64 event, but the sentinels' whole
    // safety rests on "no corporation ever has this id", so we make it certain.
    while hash == PlayerId::PIRATE.0 || hash == PlayerId::TCA.0 {
        hash = hash.wrapping_add(1);
    }
    PlayerId(hash)
}
