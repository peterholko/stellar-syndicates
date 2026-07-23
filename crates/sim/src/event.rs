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

/// The FLAVOR of a light-delayed order, for the owner-only lifecycle indicator
/// (IN TRANSIT → AWAITING ECHO → CONFIRMED). Purely a label for the panel/digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    Move,
    Raid,
    Recall,
    /// A mid-battle WITHDRAW (§battles-take-time) — disengage an engaged fleet.
    Withdraw,
    /// A BLOCKADE order (§contestable-territory) — take station on a rival system.
    Blockade,
    /// An ATTACK order (§offensive-orders) — destroy a rival fleet (full battle).
    Attack,
    /// A SURVEY order (§explore Part 2) — chart a system's exact geology on-site.
    Survey,
}

impl OrderKind {
    /// A short human label for the digest/panel ("confirmed <order>").
    pub fn label(self) -> &'static str {
        match self {
            OrderKind::Move => "move",
            OrderKind::Raid => "raid",
            OrderKind::Recall => "recall",
            OrderKind::Withdraw => "withdraw",
            OrderKind::Blockade => "blockade",
            OrderKind::Attack => "attack",
            OrderKind::Survey => "survey",
        }
    }
}

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

    /// OWNER-ONLY (§order-lifecycle): the player's order has been DELIVERED to the
    /// fleet (its outbound light arrived and the fleet is now executing) — but the
    /// light showing the new behavior hasn't returned. `echo_at` is exactly when
    /// that confirming light reaches the command center. Owner-only, fog-safe
    /// (it's the player's own command data); never delivered to rivals.
    OrderDelivered {
        owner: PlayerId,
        fleet: EntityId,
        kind: OrderKind,
        echo_at: f64,
    },
    /// OWNER-ONLY (§order-lifecycle): the confirming light has arrived — the
    /// player can now SEE the fleet complying with the order. Owner-only.
    OrderConfirmed {
        owner: PlayerId,
        fleet: EntityId,
        kind: OrderKind,
    },

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

    /// §ladder B4: a syndicate's TITAN — its one flagship — was destroyed.
    /// `name` is its christened name (None if unnamed; cleared on this event).
    /// HEADLINE news: broadcast to every corp, light-delayed from the wreck
    /// (the owner learns instantly — it's their ship).
    FlagshipDestroyed {
        owner: PlayerId,
        syndicate: crate::ids::SyndicateId,
        name: Option<String>,
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
        upgrade: crate::build::StructureKind,
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
    /// §economy Part 4: a Sol specialist CONTRACT was signed — credits debited,
    /// a personnel convoy dispatched hub → `dest`. OWNER-ONLY, own clock
    /// (price-certain; the delivery is the risky part).
    SpecialistHired { owner: PlayerId, kind: crate::specialist::SpecialistKind, dest: EntityId },
    /// §economy Part 4: an Academy finished a training course — one specialist
    /// joined the system's resident pool. OWNER-ONLY, own clock.
    SpecialistTrained { owner: PlayerId, system: EntityId, kind: crate::specialist::SpecialistKind },
    /// §modules Part B3: a module finished manufacture — one crate joined the
    /// system's module ledger. OWNER-ONLY, own clock.
    ModuleBuilt { owner: PlayerId, system: EntityId, kind: crate::module::ModuleKind },
    /// §modules Part B4: a REFIT completed — `n` hulls of `ship` rejoined fitted
    /// to `loadout` at `system`. OWNER-ONLY, own clock (a yard job, like a build).
    ShipsRefitted { owner: PlayerId, system: EntityId, ship: crate::ship::ShipKind, loadout: crate::module::Loadout, n: u32 },
    /// §modules Part B3: a module convoy LANDED its crates into a system's ledger.
    /// OWNER-ONLY, own clock (own-economy precedent, like SpecialistsDelivered).
    ModulesDelivered { owner: PlayerId, system: EntityId, manifest: std::collections::BTreeMap<crate::module::ModuleKind, u32> },
    /// §modules Part B3 (Sol hub): a module PURCHASE from Sol was settled — credits
    /// debited now, a delivery convoy dispatched hub → `dest`. OWNER-ONLY, own clock
    /// (price-certain; the delivery is the risky part, like SpecialistHired).
    ModulesPurchased { owner: PlayerId, kind: crate::module::ModuleKind, n: u32, dest: EntityId, unit_price: f64 },
    /// §modules Part B3 (Sol hub): a module SALE cleared at Sol on the convoy's
    /// arrival — credits paid at the buy-back price. OWNER-ONLY, own clock.
    ModulesSold { owner: PlayerId, kind: crate::module::ModuleKind, n: u32, unit_price: f64 },
    /// §modules Part B3: a convoy DIED with modules aboard — the crates are lost
    /// with the ship (the one true loss rule, like SpecialistsLost). OWNER-ONLY,
    /// light-delayed from the wreck like any battle news.
    ModulesLost { owner: PlayerId, manifest: std::collections::BTreeMap<crate::module::ModuleKind, u32>, pos: crate::math::Vec2 },
    /// §research: a syndicate COMPLETED a programme — its effect applies instantly
    /// galaxy-wide (design decision #5). OWNER-ONLY to the syndicate's members.
    ResearchCompleted { syndicate: crate::ids::SyndicateId, programme: String },
    /// §research: a tier's verb GATE was first crossed for a syndicate (a new
    /// row of programmes opened). OWNER-ONLY.
    TierUnlocked { syndicate: crate::ids::SyndicateId, field: crate::research::Field, school: Option<crate::research::School>, tier: u8 },
    /// §research: the syndicate's research CLOCK stalled (no staffed/funded
    /// Academy) or resumed. OWNER-ONLY. Fires once per transition.
    ResearchStalled { syndicate: crate::ids::SyndicateId },
    ResearchResumed { syndicate: crate::ids::SyndicateId },
    /// §economy Part 4: a personnel convoy LANDED its passengers into a
    /// system's resident pool. OWNER-ONLY, own clock (own-economy precedent).
    SpecialistsDelivered { owner: PlayerId, system: EntityId, manifest: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32> },
    /// §economy Part 4: a fleet DIED with specialists aboard — the people are
    /// lost with the ship (the one true loss rule for specialists; residents
    /// on the ground are never destroyed). OWNER-ONLY, light-delayed from the
    /// wreck like any battle news.
    SpecialistsLost { owner: PlayerId, manifest: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32>, pos: crate::math::Vec2 },
    /// §economy Part 3: an assignment was (re)posted — `workers` crews to
    /// `structure` at `system`. OWNER-ONLY, own clock (instant local admin,
    /// like standing orders). The UI's confirmation signal.
    AssignmentSet { owner: PlayerId, system: EntityId, structure: crate::build::StructureKind, workers: u32 },
    /// §economy Part 3: a production line STOPPED producing — latched, so it
    /// fires once per outage, with the binding cause (food > inputs > storage).
    /// OWNER-ONLY, own clock. Nothing is destroyed; fixing the cause resumes it.
    ProductionSuspended { owner: PlayerId, system: EntityId, structure: crate::build::StructureKind, reason: crate::production::SuspendReason },
    /// §economy Part 3: a suspended line PRODUCED again (the recovery notice).
    ProductionResumed { owner: PlayerId, system: EntityId, structure: crate::build::StructureKind },
    /// §economy Part 2: a colony's FOOD STATE moved on the 4-rung ladder
    /// (replaces the old binary HabitatSupplyChanged). OWNER-ONLY news, on the
    /// owner's own clock (own-economy precedent). Down-rungs are warnings
    /// (workforce efficiency drops — nothing destroyed, nobody dies);
    /// up-rungs are recoveries. Emitted only on TRANSITIONS, never per-tick.
    FoodStateChanged { owner: PlayerId, system: EntityId, state: crate::colony::FoodState },
    /// §pirates: a player DESTROYED a pirate enclave's base (ground its defense to
    /// 0). `owner` = the victor (they seize the plunder into their inventory);
    /// light-delayed from the base to their command center. The base goes dormant
    /// and respawns weaker. Owner-only (the victor's news).
    PirateEnclaveCleared {
        owner: PlayerId,
        system: EntityId,
        pos: crate::math::Vec2,
        plunder: std::collections::BTreeMap<crate::cargo::Commodity, u32>,
    },
    /// §node: an EXOTIC system AWAKENED into a capturable node at the configured
    /// awakening time. Announced GALAXY-WIDE, light-delayed from the node's position
    /// to each observer's command center (same gate as a rival claim). `bonus` names
    /// the tactical edge it grants; `pos` is for the light-delay + the map badge.
    NodeAwakened { system: EntityId, pos: crate::math::Vec2, bonus: crate::node::NodeBonus },
    /// §node: a node's HOLDER changed (colony-claimed if it was unowned, or
    /// sieged→captured if held). EXPOSURE — announced GALAXY-WIDE, light-delayed:
    /// every corp learns who now commands the node (there is no hiding a node's
    /// master). `owner` = the new holder; `pos` for the light-delay + badge tint.
    NodeCaptured {
        owner: PlayerId,
        system: EntityId,
        pos: crate::math::Vec2,
        bonus: crate::node::NodeBonus,
    },
    /// §node: an AWAKENED node's upkeep state changed. `fed = false` ⇒ this tick's
    /// upkeep mix couldn't be covered from the node's local stockpile, so its bonus
    /// SUSPENDS (nothing destroyed — recovers when fed); `fed = true` is recovery.
    /// OWNER-ONLY (your own logistics), emitted on TRANSITIONS only.
    NodeSupplyChanged { owner: PlayerId, system: EntityId, fed: bool },
    /// §explore Part 2: a SURVEY dwell completed. Fired AT THE FLEET'S POSITION —
    /// the knowledge travels home at c (the sim inserts into the corp's `surveyed`
    /// set when the report light reaches their command center, then relays to
    /// allies on the intel chain), and the timeline light-delays the owner's
    /// notice from this same `pos`. Owner-only news.
    SurveyCompleted { owner: PlayerId, system: EntityId, pos: crate::math::Vec2 },
    /// §explore Part 3: a system's HIDDEN TRAIT revealed to its (new) owner —
    /// fired at claim AND at capture (the knowledge transfers as spoils). The
    /// blind claimer's gamble resolving IS the reveal. OWNER-ONLY, light-delayed
    /// from the system (knowledge travels home at c, like the survey report).
    TraitRevealed {
        owner: PlayerId,
        system: EntityId,
        pos: crate::math::Vec2,
        trait_: crate::explore::SystemTrait,
    },
    /// §syndicates Part 3: an ALLY GARRISON's supply state changed at a host system.
    /// `owner` = the garrison's SENDER (whose fleet it is — they learn their shield
    /// went hungry/recovered); `host` = the system feeding it; `fed = false` means
    /// the host couldn't cover this tick's Provisions upkeep so the garrison's
    /// defense contribution is SUSPENDED (never destroyed — it recovers when fed).
    /// Emitted only on TRANSITIONS, per (sender, host) pair.
    GarrisonSupplyChanged { owner: PlayerId, host: EntityId, fed: bool },
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

    /// A rival BLOCKADE was ESTABLISHED at one of `owner`'s systems (§contestable-
    /// territory Part 1): a hostile fleet took station and interdiction began.
    /// Light-delayed from the system to the OWNER (they learn a rival arrived
    /// only when that light reaches their command center); the besieger `by`
    /// knows via their own on-station fleet. Nothing is destroyed — outbound
    /// convoys hold at origin, inbound hold at standoff, production still accrues.
    BlockadeEstablished { by: PlayerId, owner: PlayerId, system: EntityId, pos: crate::math::Vec2 },
    /// A blockade at one of `owner`'s systems LIFTED (§contestable-territory) —
    /// the last on-station blockader was destroyed, driven off, or withdrew.
    /// Logistics resume. Light-delayed to the owner from the system.
    BlockadeLifted { owner: PlayerId, system: EntityId, pos: crate::math::Vec2 },

    /// A fleet ORDER was soft-rejected (§TCA) — owner-only and instant: the order
    /// never installed, the fleet kept doing what it was doing, and nothing was
    /// spent. Tells the player WHY so the refusal isn't a mystery.
    OrderRejected {
        owner: PlayerId,
        fleet: EntityId,
        target: EntityId,
        reason: OrderRejectReason,
    },

    /// A besieged system was CAPTURED (§contestable-territory Part 2): a colony
    /// ship arrived while defenses were suppressed and the siege clock had run,
    /// so the system FLIPPED from `old_owner` to `new_owner`. Both learn it
    /// light-delayed from `pos` (the old owner: "you lost X"; the captor: "you
    /// captured X"). `plunder` is the seized stockpile (the defender's report
    /// itemizes what was lost). `tiers_kept` is the halved development the captor
    /// inherits. NEVER emitted for a home system (home protection).
    SystemCaptured {
        old_owner: PlayerId,
        new_owner: PlayerId,
        system: EntityId,
        pos: crate::math::Vec2,
        plunder: std::collections::BTreeMap<Commodity, u32>,
    },
}

/// Economy events. `player` always names the corporation involved; values are
/// for the delayed news / log.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum TradeEvent {
    /// A market buy settled instantly at the hub; a delivery convoy is inbound.
    Bought { player: PlayerId, commodity: Commodity, units: u32, unit_price: f64 },
    /// A delivery convoy arrived and deposited its cargo. `system == None` means it
    /// landed in the corp's HQ trading pool (a market buy); `Some(id)` means it was
    /// stocked into THAT system's stockpile (a Supply-from-HQ run or standing order).
    Delivered { player: PlayerId, commodity: Commodity, units: u32, system: Option<EntityId> },
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
    /// A SUPPLY-FROM-HQ convoy left home carrying `units` of `commodity` out of the
    /// corp's trading inventory toward `system`'s stockpile (sub-light, raidable).
    /// Arrival is reported by the usual `Delivered` (deposited) / `StorageOverflow`.
    StockDispatched { player: PlayerId, commodity: Commodity, units: u32, system: EntityId },
    /// An Exchange order or freight booking was SOFT-REJECTED (§9, §TCA) — owner-
    /// only and instant (your own administration): nothing was spent, the request
    /// simply couldn't be honored. Names WHY, so the fix is obvious. `system` is
    /// the destination/origin a freight booking concerned; `None` for a plain
    /// Exchange order, which concerns only the Charterhouse.
    Rejected {
        player: PlayerId,
        commodity: Commodity,
        units: u32,
        system: Option<EntityId>,
        reason: TradeRejectReason,
    },
    /// §TCA: a freight booking was ACCEPTED — goods escrowed, fee charged (and
    /// destroyed). Owner-only. `depart_at` and `eta` are DETERMINISTIC forecasts
    /// (the departure phase and the freighter's constant cruise are pure functions
    /// of the config), so the client can show them before the player commits.
    FreightBooked {
        player: PlayerId,
        system: EntityId,
        commodity: Commodity,
        units: u32,
        direction: crate::tca::ShipmentDir,
        fee: f64,
        /// Sim-time of the scheduled departure this lot is forecast to ride.
        depart_at: f64,
        /// Sim-time the goods are forecast to reach their destination (the system
        /// for an outbound lot; back at the Charterhouse for an inbound one).
        eta: f64,
    },
    /// §TCA Part 5: a player convoy took goods aboard at the Charterhouse
    /// (`system` = None) or at one of the corp's own systems. Owner-only.
    Loaded { player: PlayerId, commodity: Commodity, units: u32, system: Option<EntityId> },
    /// §TCA Part 5: a player convoy put its hold ashore. Owner-only.
    Unloaded { player: PlayerId, commodity: Commodity, units: u32, system: Option<EntityId> },
    /// §TCA: a freight shipment reached a milestone of its journey. Owner-only.
    FreightMoved {
        player: PlayerId,
        system: EntityId,
        commodity: Commodity,
        units: u32,
        stage: FreightStage,
    },
}

/// Where a §TCA freight shipment got to — the owner-only progress notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreightStage {
    /// Loaded aboard an Authority freighter that has left the Charterhouse.
    Departed,
    /// Unloaded into the destination system's stockpile.
    DeliveredToSystem,
    /// Collected from the origin system and aboard for the run home.
    CollectedForPickup,
    /// Landed in the owner's Charterhouse warehouse.
    ArrivedAtWarehouse,
    /// Could not be delivered (the system is no longer the owner's, or its depot
    /// had no room), so the Authority carried it back to the owner's warehouse.
    /// Friendlier than the convoy cargo-lost rule, and deliberately so.
    ReturnedUndeliverable,
    /// The origin system was lost before pickup, so the queued lot is gone — to
    /// nobody. The captor gets nothing; the fee is not refunded.
    ForfeitedOnCapture,
    /// The lot was destroyed with the freighter carrying it.
    LostWithFreighter,
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
            | TradeEvent::StorageOverflow { player, .. }
            | TradeEvent::StockDispatched { player, .. }
            | TradeEvent::Rejected { player, .. }
            | TradeEvent::FreightBooked { player, .. }
            | TradeEvent::FreightMoved { player, .. }
            | TradeEvent::Loaded { player, .. }
            | TradeEvent::Unloaded { player, .. } => *player,
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
    /// The requested ship kind is not buildable by a corporation (§TCA — the
    /// Authority Freighter is TCA-only). Should never reach a real client (it is
    /// absent from every BUILDABLE menu); a defensive soft-reject for a malformed
    /// or legacy command.
    NotBuildable,
    /// §ladder: the hull's research programme (UnlockHull) isn't completed —
    /// capitals are prizes on the Line ladder, not catalog items.
    NeedsResearch,
    /// §ladder B4: the syndicate already FIELDS (or is building) its one Titan
    /// — the singleton flagship. Rebuild is allowed only after it is lost.
    TitanFielded,
}

/// Why an Exchange order (§9) or a freight booking (§TCA) was SOFT-REJECTED.
/// Owner-only detail for the timeline notice. The async-fair rule: a rejected
/// request costs NOTHING — no debit, no escrow, no shipment, no partial state —
/// and it is never a hard error to the client.
///
/// (No `Eq`: `CannotAffordFee` carries the f64 fee it couldn't pay.)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum TradeRejectReason {
    /// The corp's CHARTERHOUSE WAREHOUSE holds fewer units than the order needs.
    /// Selling — and sell-side limit escrow — draws ONLY from the warehouse now;
    /// goods held at home must first be moved to the Charterhouse (by TCA freight
    /// or a player convoy) before they can be sold.
    InsufficientWarehouseStock { have: u32 },
    /// §TCA freight: the named system is not one the corp currently owns. The
    /// Authority serves a corporation's OWN colonies only — it will not deliver to
    /// (or collect from) a rival's ground.
    NotYourSystem,
    /// §TCA freight: the corp's stockpile AT THE ORIGIN SYSTEM is short of the
    /// units an inbound booking asked to collect.
    InsufficientSystemStock { have: u32 },
    /// §TCA freight: the treasury can't cover the freight fee. The fee is charged
    /// in full at booking or not at all — no partial lots.
    CannotAffordFee { fee: f64 },
    /// §TCA freight: the Charterhouse won't book to or from a BLOCKADED system.
    /// This is the Authority acting on its OWN light-delayed knowledge: it starts
    /// refusing only once the blockade's light reaches the hub, and keeps refusing
    /// until the lift's light does. Freight already in flight carries on — it
    /// launched on information that was true when it left.
    DestinationBlockaded,
    /// §TCA Part 5: the fleet can't do logistics right now — not the player's,
    /// not Idle, or currently engaged in a battle. Load and unload are dockside
    /// work; a fleet under way or under fire isn't doing it.
    FleetUnavailable,
    /// §TCA Part 5: the fleet is too far from the Charterhouse (or the system's
    /// star) to move goods across the boundary.
    OutOfLogisticsRange,
    /// §TCA Part 5: no room. Either the fleet has no cargo hull at all (`capacity`
    /// 0 — raiders, corvettes, scouts and colony ships carry none) or the lot
    /// would overflow what its convoys can lift.
    NoCargoRoom { capacity: u32 },
    /// §TCA Part 5: the fleet is already carrying a DIFFERENT commodity. A
    /// player convoy's hold is single-commodity (unchanged in this phase) —
    /// unload first.
    CargoMismatch,
}

/// Why a fleet ORDER was soft-rejected. Owner-only: the order simply never
/// installs, nothing is spent, and the client is never hard-errored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum OrderRejectReason {
    /// §TCA: the target shelters inside the CHARTERHOUSE SOVEREIGNTY BUBBLE, where
    /// no engagement may open. Fleeing into the bubble is sanctuary, by design.
    InsideSovereignZone,
}

impl Event {
    pub fn new(time: f64, payload: EventPayload) -> Self {
        Event { time, payload }
    }
}
