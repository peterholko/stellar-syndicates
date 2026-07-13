//! Commands: the only way the outside world mutates the simulation.
//!
//! The game loop collects commands (from player intents and from the server's
//! own session events) and feeds them to [`crate::world::World::step`] each
//! tick. Keeping every mutation as an explicit, serialisable command is what
//! makes the core deterministic and event-sourceable (§14).

use serde::{Deserialize, Serialize};

use crate::doctrine::{EngagementPosture, FleetDoctrine};
use crate::ids::{EntityId, PlayerId, SyndicateId};
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
        /// §modules Part B4: the LOADOUT to fit the ship with at build — must be
        /// ≤ the hull's module slots and covered by the system's module ledger
        /// (both debited alongside the recipe). serde default = unfitted, so old
        /// clients omitting it build the stock beam brawler exactly as before.
        #[serde(default)]
        loadout: crate::module::Loadout,
    },

    /// §modules Part B3: MANUFACTURE one module into the system's ledger. Needs
    /// an Armaments Complex ≥ 1; costs goods; completes after the build queue.
    BuildModule {
        player_id: PlayerId,
        system_id: EntityId,
        module: crate::module::ModuleKind,
    },

    /// §modules Part B4: REFIT `n` ships of `ship`/`from` in one of the player's
    /// fleets to a new `to` loadout. The fleet must be Idle and DOCKED at a
    /// Shipyard ≥ 1 the player OWNS or is ALLIED with (fits are installed at a
    /// yard). `to` must be ≤ the hull's module slots; the module DELTA (`to`
    /// minus `from`, per ship × n) must be covered by that system's ledger — it
    /// is debited, and the modules REMOVED (`from` minus `to`) are returned to
    /// the ledger. The hulls leave the fleet into the refit queue (safely OUT of
    /// combat while in the yard) and rejoin fitted after `REFIT_TICKS_PER_SHIP`
    /// × n. Soft-reject on any violation (no partial debit). INSTANT local
    /// administration to KICK OFF (like a build); the completion is on the clock.
    RefitShips {
        player_id: PlayerId,
        fleet_id: EntityId,
        ship: crate::ship::ShipKind,
        from: crate::module::Loadout,
        to: crate::module::Loadout,
        n: u32,
    },

    /// §modules Part B3: ship MODULES between systems by convoy — a crate hauler
    /// (owned → owned or allied). Mirrors `TransferSpecialists`: the manifest
    /// clamps to the source ledger and the convoy's module berths; the ledger is
    /// debited at LOADING (the crates are aboard, not at home); the manifest is
    /// fogged (broadcast hides it, sensor coverage reveals it) exactly like cargo.
    /// Delivered into the destination ledger on arrival; lost with the convoy.
    TransferModules {
        player_id: PlayerId,
        from: EntityId,
        to: EntityId,
        manifest: std::collections::BTreeMap<crate::module::ModuleKind, u32>,
    },

    /// WITHDRAW an engaged fleet from its battle (§battles-take-time). A coarse,
    /// LIGHT-DELAYED mid-battle verb: it schedules a break-off-and-flee-home order
    /// (physical disengagement at formation speed — the speed table decides who
    /// escapes) that removes the fleet from any engagement on arrival. Wired to
    /// the order-lifecycle indicator like any order. Soft-reject if not owned.
    Withdraw {
        player_id: PlayerId,
        fleet_id: EntityId,
    },

    /// Set a fleet's TRANSIT throttle (§Part 4): Full or Stealth. Instant local
    /// administration on the player's own fleet — governs its move speed and, via
    /// its velocity, its detection signature. Soft-reject if not the player's.
    SetFleetTransit {
        player_id: PlayerId,
        fleet_id: EntityId,
        mode: crate::ship::TransitMode,
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
        upgrade: crate::build::StructureKind,
        /// §bodies: the BODY to build on. `None` (old clients) auto-sites via
        /// the shared siting rules — same layouts the anchors always used.
        #[serde(default)]
        body_id: Option<u32>,
    },

    /// §economy Part 3: post `workers` workforce crews to a structure at one of
    /// the player's OWNED systems (0 withdraws the line). INSTANT local
    /// administration, like a standing order: validated against ownership and
    /// the structure being BUILT; `workers` clamps to the structure's tier (a
    /// tier-N plant fields at most N crews). Over-posting the colony's
    /// workforce is legal — every line dilutes by the same staffing share
    /// (legible, deadlock-free), so no rejection edge exists there.
    SetAssignment {
        player_id: PlayerId,
        system_id: EntityId,
        structure: crate::build::StructureKind,
        workers: u32,
        /// §bodies: the BODY whose line this staffs. `None` (old clients)
        /// targets the body holding the structure (highest tier first).
        #[serde(default)]
        body_id: Option<u32>,
        /// §economy Part 4: SPECIALISTS posted to the line, from the system's
        /// resident pool (clamped so totals across lines fit the pool).
        /// `default` empty keeps pre-specialist clients/commands parsing.
        #[serde(default)]
        specialists: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32>,
    },

    /// §economy Part 4: sign a Sol SPECIALIST CONTRACT — `SPECIALIST_HIRE_COST`
    /// credits debited instantly (price-certain), then a personnel convoy
    /// spawns at the hub carrying the specialist to `dest_system`
    /// (delivery-risky: sub-light, raidable, manifest sensor-gated). Soft-
    /// reject unless the player owns `dest_system` and can pay.
    HireSpecialist {
        player_id: PlayerId,
        specialist: crate::specialist::SpecialistKind,
        dest_system: EntityId,
    },

    /// §economy Part 4: enqueue an Academy TRAINING COURSE at one of the
    /// player's owned systems (needs Academy tier ≥ 1; costs
    /// `ACADEMY_TRAIN_RECIPE` from the local stockpile; completes into the
    /// resident pool). Instant local administration, like any build.
    TrainSpecialist {
        player_id: PlayerId,
        system_id: EntityId,
        specialist: crate::specialist::SpecialistKind,
    },

    /// §economy Part 4: load resident specialists onto a dedicated personnel
    /// convoy from `from` to `to` (owned or allied). Manifest clamps to the
    /// resident pool and the convoy's passenger berths. The convoy is a normal
    /// sub-light, raidable hull — losing it loses the people aboard.
    TransferSpecialists {
        player_id: PlayerId,
        from: EntityId,
        to: EntityId,
        manifest: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32>,
    },

    /// BLOCKADE a rival system (§contestable-territory Part 1): order one of the
    /// player's fleets to take station on a rival-owned system and strangle its
    /// logistics. The fleet must CONTAIN ≥1 raider (strike capability — corvettes
    /// and scouts contribute strength but can't blockade alone). Fuel-charged and
    /// LIGHT-DELAYED like any move order (the echo lifecycle applies): the fleet
    /// only begins the run once the order's outbound light reaches it. Soft-reject
    /// unless the player owns the fleet, it has a raider, and the target is a
    /// rival's system (a fresh sighting — the target may have changed since).
    BlockadeSystem {
        player_id: PlayerId,
        fleet_id: EntityId,
        system_id: EntityId,
    },

    /// SURVEY a system's exact geology (§explore Part 2 — the scout's second
    /// job). The fleet must CONTAIN ≥1 Scout (the sensing capability; escorts
    /// ride along). Valid on ANY system — unclaimed frontier, an ally's, or a
    /// RIVAL's (pre-siege prospecting is intended). Fuel-charged and
    /// LIGHT-DELAYED via the echo lifecycle like any order; on-site the fleet
    /// dwells `SURVEY_SECS`, LOUD, then the knowledge travels home at c.
    SurveySystem {
        player_id: PlayerId,
        fleet_id: EntityId,
        system_id: EntityId,
    },

    /// ATTACK a rival fleet (§offensive-orders Part 1) — the targeted DESTROY verb.
    /// Orderable on ANY rival fleet (not just convoys). The attacking fleet must
    /// CONTAIN ≥1 raider (strike capability — consistent with `BlockadeSystem`;
    /// corvette/scout-only fleets soft-reject). Fuel-charged and LIGHT-DELAYED via
    /// the echo lifecycle, like a raid. Reuses the intercept-commit pursuit, but on
    /// contact opens a FULL-DURATION battle (not the raid brevity cap): a destroyed
    /// fleet's cargo is lost with it. RAID (`CommitRaid`) steals; ATTACK destroys.
    AttackFleet {
        player_id: PlayerId,
        fleet_id: EntityId,
        target_id: EntityId,
    },

    /// Set a fleet's ENGAGEMENT POSTURE (§offensive-orders Part 2): the standing
    /// per-fleet aggression (Passive / Defensive / WeaponsFree). INSTANT local
    /// administration on the player's own fleet — a standing policy, like the
    /// sibling `SetFleetTransit` throttle and the corp `SetFleetDoctrine` (not a
    /// real-time command). The ACTION it authorizes is taken on the fleet's OWN
    /// local detection (forward autonomy); the owner learns of any engagement
    /// light-delayed. Soft-reject if not the player's fleet.
    SetFleetPosture {
        player_id: PlayerId,
        fleet_id: EntityId,
        posture: EngagementPosture,
    },

    // ---- SYNDICATES (§syndicates Part 1) -------------------------------------
    // Alliance administration. All INSTANT owner-only admin (like the policy
    // commands): they change ground-truth membership immediately, revealing
    // nothing to rivals except light-delayed via the View. Soft-reject on any
    // violation (already affiliated, not the founder, no invite, cap exceeded).
    /// FOUND a syndicate with the caller as founder + sole member. Ignored if the
    /// caller is already in one.
    CreateSyndicate { player_id: PlayerId, name: String },

    /// INVITE a corp into the caller's syndicate (founder-only). Records a pending
    /// invite the invitee accepts separately. Ignored unless the caller is the
    /// founder and the invitee is unaffiliated.
    InviteToSyndicate { player_id: PlayerId, invitee: PlayerId },

    /// ACCEPT a pending invitation to the named syndicate. Ignored unless the
    /// caller is unaffiliated, actually holds the invite, and the roster has room
    /// under the SIZE CAP.
    AcceptSyndicateInvite { player_id: PlayerId, syndicate_id: SyndicateId },

    /// LEAVE the caller's syndicate. If the founder leaves, the seat passes to the
    /// next member; an emptied syndicate dissolves. Ignored if unaffiliated.
    LeaveSyndicate { player_id: PlayerId },

    /// DISSOLVE the caller's syndicate (founder-only): every member becomes
    /// unaffiliated. Ignored unless the caller is the founder.
    DissolveSyndicate { player_id: PlayerId },
}
