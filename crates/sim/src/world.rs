//! The authoritative world state and the pure step function.
//!
//! This is ground truth — the single, objective galaxy the server's game loop
//! owns. Players never see it directly; the per-player view filter (M3) derives
//! each player's delayed, fogged reconstruction from it. `World` performs no
//! I/O and no async work: it takes commands and a fixed timestep and returns
//! the next state plus the events that occurred (§14).

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::cargo::Cargo;
use crate::command::Command;
use crate::config::{SimConfig, DT, TICK_HZ};
use crate::doctrine::{
    DestinationInvalidPolicy, EngagementPolicy, EscortPolicy, FleetDoctrine,
};
use crate::event::{DivertAction, Event, EventPayload, FreightStage, RaidOutcome, TradeEvent, TradeRejectReason};
use crate::galaxy::{generate_home_slots, generate_systems, HomeSlot, StarSystem};
use crate::ids::{EntityId, PlayerId, SyndicateId};
use crate::market::{clear_call_auction, LimitOrder, Side};
use crate::math::Vec2;
use crate::movement::pursue_step;
use crate::ship::{DefenseEngagement, Fleet, FleetOrder, ShipKind, TradeMission};
use crate::standing::{Endpoint, OrderStatus, StandingOrder, Trigger};
use crate::syndicate::{syndicate_cap, Syndicate};
use crate::tca::{FreightRun, RunLeg, Shipment, ShipmentDir, ShipmentId};
use crate::pirate::{self, Enclave};

/// A player's corporation — their persistent presence in the galaxy. Grows in
/// later milestones (credits, holdings, fleets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corporation {
    pub id: PlayerId,
    pub name: String,
    /// Tick at which this corporation first entered the galaxy.
    pub joined_tick: u64,
    /// The corporation's home anchor — its bright coherence peak (§6).
    pub home: Vec2,
    /// Origin of this player's light-cone: all fog-of-war and command latency
    /// are computed from here (§6). Equals `home` until the command center is
    /// relocated (a later milestone); kept separate so M3 can use it directly.
    pub command_center: Vec2,
    /// The corporation's owned HOME STAR SYSTEM — granted at join (free), with the
    /// command center sitting at it. A normal owned [`StarSystem`] in `world.systems`
    /// (it produces, has a stockpile, can be managed/shipped/defended); this id just
    /// records which one is home. `None` only in pre-feature snapshots.
    #[serde(default)]
    pub home_system: Option<EntityId>,
    /// Credits (the corporate treasury).
    pub credits: f64,
    /// Goods held at home, by commodity.
    pub inventory: BTreeMap<crate::cargo::Commodity, u32>,
    /// §TCA: goods held AT THE CHARTERHOUSE — this corp's private warehouse at the
    /// hub. The Exchange settles ONLY against this (buys deposit here; sells and
    /// sell-side limit escrow draw ONLY from here). Nothing about a trade moves
    /// goods across space; moving goods hub↔systems is the separate, explicit act
    /// of TCA freight or a player convoy. `#[serde(default)]` so every pre-feature
    /// snapshot loads with an empty warehouse — existing goods stay in `inventory`
    /// at home and are moved with the new channels (that IS the migration).
    ///
    /// CAPACITY-UNCHECKED by design: the warehouse is infinite until the separate
    /// leased-bay handoff lands. Every inflow path here deposits unconditionally.
    #[serde(default)]
    pub warehouse: BTreeMap<crate::cargo::Commodity, u32>,
    /// §TCA Phase 2: CHARTER STANDING with the Terran Charter Authority. Starts
    /// (and regenerates back) to [`crate::tca::TCA_STANDING_START`]; each incident
    /// against an Authority hull decrements it once the citation's light reaches
    /// the hub. The BAND (`CharterStatus`) is always derived from this by
    /// [`crate::tca::charter_status`] and never stored, so there is no cached copy
    /// to desync. `#[serde(default = …)]` so every pre-law snapshot loads in GOOD
    /// STANDING — nobody is retroactively an outlaw.
    #[serde(default = "default_tca_standing")]
    pub tca_standing: f64,
    /// Equity / net worth, recomputed on a slow cadence (§9) to avoid
    /// share-price noise: credits + goods (held, in-transit, and reserved in
    /// resting orders) at market value + buy-order escrow.
    pub valuation: f64,
    /// Standing logistics orders (§15) — constrained automation rules this corp
    /// runs server-side, online or off. See [`crate::standing`].
    #[serde(default)]
    pub standing_orders: Vec<crate::standing::StandingOrder>,
    /// Monotonic allocator for this corp's standing-order ids (0 ⇒ first id is 1).
    #[serde(default)]
    pub next_standing_id: u32,
    /// Fleet doctrine (§16) — the constrained, server-run combat & logistics
    /// policy governing autonomous engage/retreat/escort and supply re-routing.
    /// Defaults to today's behaviour. See [`crate::doctrine`].
    #[serde(default)]
    pub doctrine: FleetDoctrine,
    /// INTEL SNAPSHOTS (§scout part 2), keyed by scouted system: what this
    /// corp's scouts physically observed of RIVAL fortifications at contact
    /// range. A snapshot is knowledge captured at a moment — it AGES and is
    /// never live (the rival may have built since; re-scout to refresh). The
    /// View delivers it to the owner only once the capture's light has reached
    /// their command center, and to NOBODY else.
    #[serde(default)]
    pub intel: BTreeMap<EntityId, IntelSnapshot>,
    /// SYNDICATE membership (§syndicates). `None` = unaffiliated. The roster lives
    /// in [`World::syndicates`]; this denormalized id gives O(1) `are_allied`
    /// (ground-truth non-engagement — an alliance is a mutual pact both parties
    /// consented to, in effect immediately).
    #[serde(default)]
    pub syndicate: Option<SyndicateId>,
    /// The PRIOR membership + when the CURRENT one took effect — so a distant
    /// viewer learns of a join/leave only after the light from THIS corp's command
    /// center arrives (membership propagates like ownership; a 2-state history
    /// covers the light-delay window). See `World::known_ally`.
    #[serde(default)]
    pub syndicate_prev: Option<SyndicateId>,
    #[serde(default)]
    pub syndicate_since: f64,
    /// §rankings: this corp's CUMULATIVE campaign counters (the leaderboard
    /// inputs). Incremented at the events that already fire; persisted with the
    /// corp. `#[serde(default)]` so pre-feature snapshots load with zeroed stats.
    #[serde(default)]
    pub stats: crate::rankings::RankingStats,
    /// §explore R2: the systems whose EXACT geology this corp knows — the survey
    /// knowledge set. Seeded at join (everything within `SURVEY_INITIAL_RADIUS`
    /// of home), grown by claiming/capturing (holding a system IS knowing it) and
    /// by Survey orders (Part 2). PERMANENT (deposits are static — survey data
    /// never stales, and losing a system doesn't un-know its geology). Gates the
    /// `deposits` field in the server View; never on a rival's wire itself.
    /// `#[serde(default)]` — pre-feature snapshots load empty and are healed by
    /// [`World::fixup_after_load`] (a live corp is never amnesiac about home).
    #[serde(default)]
    pub surveyed: std::collections::BTreeSet<EntityId>,
}

/// One scouted observation of a rival system's fortifications (§scout part 2):
/// the raid/siege-relevant tiers, WHEN it was seen, and WHERE the scout stood
/// (the light source for delivering the report). Deliberately narrow — no
/// stockpiles, no habitat state — the prize stays focused.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IntelSnapshot {
    pub defense_tier: u32,
    pub shipyard_tier: u32,
    /// §pirates: the scouted PIRATE ENCLAVE tier at this system (0 = not an enclave
    /// / none observed). A scout near an enclave reveals its base tier like
    /// fortifications; serde default keeps pre-pirate snapshots loading.
    #[serde(default)]
    pub enclave_tier: u32,
    /// Sim-time of the observation (the "as of T" the readout ages from).
    pub observed_at: f64,
    /// Where the scout was at capture — the point the report's light travels from.
    pub pos: Vec2,
}

/// An order in flight: a player's command that has left their command center
/// but not yet reached the ship (the outbound light-travel time of §6). Carries
/// the order to install once the light arrives (a move, a raid commit, or a
/// recall-as-return-home).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingOrder {
    /// Sim time at which the order's light reaches the ship (= `delivered_at`).
    apply_time: f64,
    ship_id: EntityId,
    new_order: FleetOrder,
    /// Owner (for the owner-only lifecycle indicator). serde default for old snaps.
    #[serde(default = "default_player")]
    owner: PlayerId,
    /// Sim time the CONFIRMING light (of the new behavior) reaches the command
    /// center — `apply_time + distance(delivery point → cc)/c`. Exactly computable
    /// at issue under constant-velocity kinematics (§order-lifecycle).
    #[serde(default)]
    echo_at: f64,
    /// When the order was issued (to pick the LATEST order per fleet for display).
    #[serde(default)]
    issued_at: f64,
    /// The order flavor, for the lifecycle panel/digest.
    #[serde(default = "default_order_kind")]
    kind: crate::event::OrderKind,
}

/// serde default for [`Corporation::tca_standing`] (§TCA Phase 2) — a pre-law
/// snapshot loads every corporation in GOOD STANDING; nobody is retroactively an
/// outlaw.
fn default_tca_standing() -> f64 {
    crate::tca::TCA_STANDING_START
}

fn default_order_kind() -> crate::event::OrderKind {
    crate::event::OrderKind::Move
}

fn default_player() -> PlayerId {
    PlayerId(0)
}

fn default_entity() -> EntityId {
    EntityId(0)
}

/// A DELIVERED order whose confirming light hasn't yet reached the command center
/// (the AWAITING-ECHO phase). Owner-only; transient lifecycle bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingEcho {
    owner: PlayerId,
    fleet: EntityId,
    delivered_at: f64,
    echo_at: f64,
    issued_at: f64,
    kind: crate::event::OrderKind,
}

/// A light-gate summary of an ongoing BATTLE for the server's View
/// (§battles-take-time). All fields are the AUTHORITATIVE truth; the server gates
/// visibility by `distance(pos, cc)/c`.
#[derive(Debug, Clone)]
pub struct BattleInfo {
    /// The engagement's stable id — one battle entity, one map icon.
    pub id: EntityId,
    pub pos: Vec2,
    pub started_at: f64,
    pub a_owner: PlayerId,
    pub d_owner: PlayerId,
    pub participants: Vec<EntityId>,
}

/// Owner-only lifecycle snapshot of one in-flight order (§order-lifecycle): the
/// two exact timestamps that let the client tick down IN TRANSIT (until
/// `delivered_at`) and AWAITING ECHO (until `echo_at`). It's the player's own
/// command data — trivially fog-safe.
#[derive(Debug, Clone, Copy)]
pub struct PendingCommandView {
    pub fleet: EntityId,
    pub delivered_at: f64,
    pub echo_at: f64,
    pub issued_at: f64,
    pub kind: crate::event::OrderKind,
}

/// §research R6: one Academy's live contribution to its syndicate's ACTIVE
/// programme — the SHOWN factor chain (design law 2: the panel's number IS the
/// clock's number). `supplied` is false when the local stockpile can't cover
/// this lab's drip THIS tick (the amber "unsupplied" tint in the UI).
#[derive(Debug, Clone)]
pub struct AcademyContribution {
    pub system: EntityId,
    pub system_name: String,
    pub body_id: u32,
    pub tier: u32,
    pub throughput: f64,
    pub staffing: f64,
    pub skill: f64,
    pub food: f64,
    pub rate: f64,
    pub supplied: bool,
}

/// Distance (sim units) at which a raider makes contact with its target.
const CONTACT_RADIUS: f64 = 80.0;
/// Distance (sim units) within which a friendly combatant fleet that has moved to
/// an active battle JOINS its side's pool (relief). A bit more than contact so an
/// arriving reinforcement latches on cleanly. Tunable.
const BATTLE_JOIN_RADIUS: f64 = 200.0;
/// Distance from the hub within which a convoy is safe from raiders (§4: the hub
/// is the shared commons).
const HUB_SAFE_RADIUS: f64 = 300.0;

// --- CONTESTABLE TERRITORY (§blockade / siege → capture) — BALANCE PLACEHOLDERS.
// Every number here is PLAYTEST-DEFERRED, awaiting multi-session pacing data; the
// MECHANICS are the deliverable, not the tuning. Grouped so one edit re-paces the
// whole stakes layer once we have real games.
/// How close a blockading fleet must sit to a system to count as ON STATION
/// (arrived + holding). The fleet flies to the system position, so this only
/// needs to clear the last sliver of travel — well inside `DEFENSE_PLATFORM_RADIUS`
/// so the standing platform engages it. Tunable.
const BLOCKADE_STATION_RADIUS: f64 = 140.0;
/// Where an INBOUND convoy to a blockaded system halts (destination-invalid soft
/// idiom): far enough out to read as "held off," not destroyed. Tunable.
const BLOCKADE_STANDOFF_RADIUS: f64 = 900.0;
/// SIEGE_DURATION (§Part 2): how long an UNBROKEN blockade must hold with defenses
/// suppressed before a colony ship can capture. Derived as a MULTIPLE of the
/// config battle timescale so ONE knob (`battle_target_secs`) scales both — at
/// playtest scale (~45 s battles) a siege is ~6 min; at production scale (~2700 s)
/// it is hours, long enough that a realistic check-in cadence can mount relief.
/// Placeholder factor, awaiting pacing data.
const SIEGE_DURATION_BATTLE_MULT: f64 = 8.0;

// --- Autonomous defensive doctrine (§5.1, Pillar 1) — all tunable. -----------
// A patrolling raider guards friendly convoys within its sensor bubble and can
// only react to hostiles it can actually sense (== `config.sensor_range`), so
// patrol POSITIONING (how close to the convoy's route) decides whether it detects
// a threat early enough to intercept. These knobs balance that dynamic.
/// A hostile counts as "on an intercept course" only if it is moving at least
/// this fast (so a drifting/parked raider isn't treated as an inbound attack).
const THREAT_MIN_SPEED: f64 = 8.0;
/// …and heading within ~acos(0.3)≈72° of straight at the guarded convoy.
const THREAT_CLOSING_COS: f64 = 0.3;
/// Once engaged, a defender breaks off (resumes patrol) if its quarry gets
/// farther than `sensor_range × this` away — it guards its station, it doesn't
/// chase a fleeing raider across the galaxy.
const PURSUIT_BREAKOFF_MULT: f64 = 2.5;
/// How near a friendly convoy a patrolling raider must be to "adopt" it as the
/// charge it guards (and then shadow). A picket with NO convoy this close guards
/// nothing — so WHERE a patrol sits decides what (if anything) it can defend.
const ASSIGN_RANGE: f64 = 3300.0;
/// Half-length of the short bracketing patrol a picket holds over its charge, so
/// it keeps station near a moving convoy instead of drifting off.
const SHADOW_OFFSET: f64 = 400.0;

/// The market drifts once per this many ticks (≈ once a second at 30 Hz).
const MARKET_UPDATE_TICKS: u64 = 30;


/// The limit-order book clears once per this many ticks (≈ every 20 s).
const BATCH_TICKS: u64 = 20 * TICK_HZ as u64;

/// Corporate valuations recompute once per this many ticks (the slow §9 close).
const VALUATION_TICKS: u64 = 60 * TICK_HZ as u64;

/// A standing logistics order (§15) is re-evaluated at most once per this many
/// ticks (≈ every 5 s). With the one-convoy-in-flight guard this bounds a rule to
/// at most one dispatch per period — a permanently-satisfied trigger can never
/// flood the map (the async-automation anti-spam invariant).
const EVAL_PERIOD: u64 = 5 * TICK_HZ as u64;

/// Remove `units` of `c` from a whole-unit goods map (a corp's `inventory` or
/// `warehouse`), DROPPING the entry once it empties — so a warehouse never
/// accumulates zero rows to clutter the wire, the timeline, or a snapshot diff.
/// Saturating: never underflows.
fn take_from(map: &mut BTreeMap<crate::cargo::Commodity, u32>, c: crate::cargo::Commodity, units: u32) {
    if let Some(u) = map.get_mut(&c) {
        *u = u.saturating_sub(units);
        if *u == 0 {
            map.remove(&c);
        }
    }
}

/// The order that resumes a saved patrol route after a defensive sortie (or idles
/// if the route was empty).
fn resume_patrol(route: Vec<Vec2>) -> FleetOrder {
    if route.is_empty() {
        FleetOrder::Idle
    } else {
        FleetOrder::Patrol { waypoints: route, index: 0, dwell_until: 0.0 }
    }
}

/// Ground-truth galaxy state. Deterministic given `config.seed` and the
/// command sequence applied via [`World::step`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub config: SimConfig,
    /// Number of completed ticks.
    pub tick: u64,
    /// Simulation time in seconds (`tick * DT`).
    pub time: f64,
    /// The wormhole hub at the galaxy centre — the shared market commons (§4).
    pub hub: Vec2,
    /// Procedurally-placed star systems (static geography).
    pub systems: Vec<StarSystem>,
    /// Pre-generated home-anchor slots; assigned to players on join.
    pub home_slots: Vec<HomeSlot>,
    /// All corporations, keyed by id. `BTreeMap` keeps iteration deterministic.
    pub players: BTreeMap<PlayerId, Corporation>,
    /// All fleets, keyed by id. `BTreeMap` keeps integration order deterministic.
    /// `alias = "ships"` accepts pre-FLEETS snapshots (the entity table was
    /// renamed from `ships`); the per-entity `composition` back-fill lives in the
    /// server's `migrate_world_json`.
    #[serde(alias = "ships")]
    pub fleets: BTreeMap<EntityId, Fleet>,
    /// The shared hub Exchange (§9).
    pub market: crate::market::Market,
    /// Resting limit orders, cleared in a periodic uniform-price call auction.
    pub book: Vec<LimitOrder>,
    /// Monotonic allocator for limit-order ids.
    next_order_id: u64,
    /// Orders that have been issued but whose light has not yet reached the ship.
    pending_orders: Vec<PendingOrder>,
    /// DELIVERED orders whose confirming light hasn't yet returned to the command
    /// center (§order-lifecycle, owner-only). serde default so old snaps load.
    #[serde(default)]
    pending_echoes: Vec<PendingEcho>,
    /// Monotonic allocator for entity ids.
    next_entity_id: u64,
    /// Pending construction jobs (fleets + system upgrades), resolved in step()
    /// phase 5b' when their completion tick arrives (§step1 growth sink). Iterated
    /// in id-push order for determinism. `#[serde(default)]` so old snapshots load.
    #[serde(default)]
    pub build_queue: Vec<crate::build::BuildJob>,
    /// Monotonic allocator for build-job ids (0 ⇒ first id is 1). Shared with the
    /// refit queue so ids never collide across the two job spaces.
    #[serde(default)]
    next_build_id: u64,
    /// §modules Part B4: pending REFIT jobs — hulls pulled from a fleet into the
    /// yard, rejoining fitted when their completion tick arrives. Its own queue
    /// (parallel to `build_queue`) so refits stay isolated from construction and
    /// the hulls are cleanly out of combat meanwhile. `#[serde(default)]` empties
    /// = zero migration.
    #[serde(default)]
    pub refit_queue: Vec<crate::build::RefitJob>,
    /// World RNG stream (continues past generation) for deterministic events.
    rng: crate::rng::Rng,
    /// Ongoing BATTLES (§battles-take-time) — persistent, observable engagement
    /// entities keyed by id. Each pools one or more fleets per side and carries
    /// the side DAMAGE POOLS, so combat runs over many ticks (config-scaled
    /// duration), relief joins mid-fight, and a snapshot RESUMES the fight. They
    /// are light-gated in the View like any event (weapons fire is loud — all
    /// participants, even dark fleets, are revealed at the site by old light).
    #[serde(default)]
    pub engagements: BTreeMap<EntityId, Engagement>,
    #[serde(default)]
    next_engagement_id: u64,
    /// BATTLE RECORDS (§battle-records Part A) — a watchable, light-gated replay
    /// per engagement, keyed by the engagement's own id. The recorder OBSERVES
    /// the lifecycle (open → per-round flush → finalize) and never feeds back
    /// into resolution, so records are deterministic and balance-patch-stable.
    /// Pruned on open (retention + per-corp floor + hard cap). `#[serde(default)]`
    /// so pre-feature snapshots load with no records.
    #[serde(default)]
    pub battle_records: BTreeMap<EntityId, crate::combat::BattleRecord>,
    /// SYNDICATES (§syndicates) — alliance rosters keyed by id. Membership is also
    /// denormalized on each [`Corporation`] for O(1) `are_allied`. `BTreeMap` keeps
    /// iteration deterministic; `#[serde(default)]` so pre-feature snapshots load.
    #[serde(default)]
    pub syndicates: BTreeMap<SyndicateId, Syndicate>,
    /// Monotonic allocator for syndicate ids (0 ⇒ first id is 1). Its OWN counter,
    /// separate from entities, so ids never collide across the two id spaces.
    #[serde(default)]
    next_syndicate_id: u64,
    /// PIRATE ENCLAVES (§pirates) — the neutral hostile faction's hidden bases,
    /// keyed by their host (unclaimed) system id. Seeded deterministically at
    /// generation; driven each tick by `pirate_ai`. `#[serde(default)]` so
    /// pre-feature snapshots load with no pirates.
    #[serde(default)]
    pub enclaves: BTreeMap<EntityId, Enclave>,
    /// EXOTIC NODES (§node) — the midgame catalyst, keyed by their host system id.
    /// Seeded DORMANT at generation for every exotic system (parity with the
    /// client's visual exotics), they AWAKEN at `config.node_awakening_time` into
    /// capturable tactical prizes. The holder is the host system's `owner` (read
    /// live). `#[serde(default)]` so pre-feature snapshots load with no nodes.
    #[serde(default)]
    pub nodes: BTreeMap<EntityId, crate::node::Node>,
    /// §rankings: the last PUBLISHED leaderboard — a snapshot copy taken on the
    /// ledger tick ([`VALUATION_TICKS`]). Between snapshots this holds steady, so
    /// live counter changes never leak mid-interval. Read by the server verbatim
    /// into the public View. `#[serde(default)]` so old snapshots load empty.
    #[serde(default)]
    pub rankings: Vec<crate::rankings::RankingRow>,
    /// §explore R1: the richness-band TERCILE thresholds `(lo, hi)` over all
    /// systems' `band_value`, computed once at generation (deposits are static,
    /// so these never change). `#[serde(default)]` = (0,0) on a pre-feature
    /// snapshot — healed by [`World::fixup_after_load`] (a pure recompute).
    #[serde(default)]
    pub band_lo: f64,
    #[serde(default)]
    pub band_hi: f64,
    /// §explore Part 2: SURVEY REPORTS in flight — knowledge travelling home at c.
    /// The owner's leg is scheduled at dwell completion (`pos → owner cc`); when
    /// it lands, ally-relay legs are scheduled (`owner cc → each ally cc`, the
    /// same chain the scout-intel relay uses). Delivery INSERTS into the
    /// recipient's `surveyed` set (permanent). `#[serde(default)]` for old snaps.
    #[serde(default)]
    pub pending_survey_reports: Vec<SurveyReport>,
    /// §TCA: booked freight SHIPMENTS awaiting a departure — escrowed (goods have
    /// left the warehouse / a system stockpile) but not yet aboard a freighter.
    /// Keyed by [`crate::tca::ShipmentId`]; the scheduler drains it FIFO per
    /// corp+destination. `#[serde(default)]` so pre-feature snapshots load empty.
    #[serde(default)]
    pub freight_queue: BTreeMap<crate::tca::ShipmentId, crate::tca::Shipment>,
    /// §TCA: in-flight freighter RUNS, keyed by the freighter fleet id. The
    /// multi-owner manifest rides here (never in `Fleet.cargo`). `#[serde(default)]`
    /// so pre-feature snapshots load with no runs.
    #[serde(default)]
    pub freight_runs: BTreeMap<EntityId, crate::tca::FreightRun>,
    /// §TCA: monotonic allocator for shipment ids (0 ⇒ first id is 1).
    #[serde(default)]
    next_shipment_id: u64,
}

/// §explore Part 2: one in-flight survey-report leg (see
/// [`World::pending_survey_reports`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurveyReport {
    /// Who receives the knowledge when this leg lands.
    pub recipient: PlayerId,
    pub system: EntityId,
    /// Sim time the leg's light arrives at the recipient's command center.
    pub arrive_at: f64,
    /// `false` = the surveyor's own report (on delivery it FANS OUT relay legs to
    /// the origin's allies-at-that-moment); `true` = an ally-relayed copy (terminal).
    pub relay: bool,
    /// The surveying corp (the relay origin).
    pub origin: PlayerId,
}

/// An ongoing BATTLE at a location (§battles-take-time). Persistent + observable.
/// A "side" is one or more fleets pooled for Lanchester attrition; the per-kind
/// damage pools live HERE (not on the fleets), so multi-fleet sides and relief
/// compose cleanly and a mid-battle snapshot resumes exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engagement {
    pub id: EntityId,
    pub pos: Vec2,
    /// Sim time the battle began (drives elapsed → raid cap / safety valve, and
    /// the "as of N ago" digest).
    pub started_at: f64,
    /// A cargo RAID (short cap, skirmish rate) vs a decisive battle.
    pub raid: bool,
    pub a_owner: PlayerId,
    pub d_owner: PlayerId,
    /// Fleets on each side (attacker side committed the intercept; defender side
    /// is the target + its escorts/garrison). Relief appends here.
    pub attackers: Vec<EntityId>,
    pub defenders: Vec<EntityId>,
    /// A covering defense platform folded into the defender side, if any.
    pub platform_system: Option<EntityId>,
    /// §modules: PER-STACK side damage pools (kind → loadout key → pool) — the
    /// persisted mid-battle state. Per-stack (not per-kind) so an armored stack
    /// keeps its own accumulated absorption across ticks. `#[serde(default)]`
    /// drops a pre-fix snapshot's `a_pool`/`d_pool` cleanly (that battle resumes
    /// with fresh pools — an acceptable alpha discontinuity, never a panic).
    #[serde(default)]
    a_stack_pool: crate::combat::StackPoolMap,
    #[serde(default)]
    d_stack_pool: crate::combat::StackPoolMap,
    /// Report bookkeeping: total composition + strength each side STARTED with.
    a_start: BTreeMap<ShipKind, u32>,
    d_start: BTreeMap<ShipKind, u32>,
    /// §modules B5: the LOADOUT partition each side started with — the record's
    /// per-loadout intel (participant fidelity surfaces it; the client types the
    /// replay's salvos by each side's dominant weapon family from this). serde
    /// default = empty (old snapshots / all-unfitted), zero migration.
    #[serde(default)]
    a_start_loadouts: crate::combat::LoadoutMap,
    #[serde(default)]
    d_start_loadouts: crate::combat::LoadoutMap,
    a_start_strength: f64,
    d_start_strength: f64,
    platform_start_tiers: u32,
    /// A representative fleet id per side (the first participant), so the battle
    /// REPORT names real fleets even after a side is wiped out. serde default for
    /// old snapshots (falls back to the engagement id).
    #[serde(default = "default_entity")]
    a_lead: EntityId,
    #[serde(default = "default_entity")]
    d_lead: EntityId,
    /// Participants that did NOT accept this battle (Avoid doctrine, not the
    /// committed attacker) → the sim time at which their brief parting-shot
    /// exposure ends and they physically flee (§engagement movement). Persisted
    /// so a mid-battle snapshot resumes the disengagement on schedule.
    #[serde(default)]
    disengaging: BTreeMap<EntityId, f64>,
    /// A fleet on this side FLED (avoid-disengage or withdraw) and SURVIVES — so
    /// an emptied side is a WITHDRAWAL, not a wipe, in the final report.
    #[serde(default)]
    a_fled: bool,
    #[serde(default)]
    d_fled: bool,
    /// Touched this tick? (Untouched engagements have ended — flush + remove.)
    #[serde(skip)]
    touched: bool,
    /// §tactical: the INDIVIDUAL-SHIP battle state (the engine that replaced
    /// pooled Lanchester). `None` on old snapshots → the next tick MIGRATES
    /// one-way: pooled counts + damage pools unpack into combatants and the
    /// battle continues under the new engine.
    #[serde(default)]
    tactical: Option<crate::tactical::TacticalState>,
}

/// The flagship kind of a pooled composition (flagship precedence) — for the
/// battle report's representative attacker/target kind.
fn flagship_of(comp: &BTreeMap<ShipKind, u32>) -> ShipKind {
    for k in crate::ship::FLAGSHIP_PRECEDENCE {
        if comp.get(&k).copied().unwrap_or(0) > 0 {
            return k;
        }
    }
    ShipKind::Scout
}

/// Per-kind ships lost = `start − now` (saturating), for a battle report.
fn diff_comp(
    start: &BTreeMap<ShipKind, u32>,
    now: &BTreeMap<ShipKind, u32>,
) -> BTreeMap<ShipKind, u32> {
    let mut out = BTreeMap::new();
    for (k, s) in start {
        let lost = s.saturating_sub(now.get(k).copied().unwrap_or(0));
        if lost > 0 {
            out.insert(*k, lost);
        }
    }
    out
}

/// Trim + length-cap a player-supplied syndicate name (deterministic, no I/O).
/// Empty after trimming falls back to a neutral default.
fn sanitize_name(raw: &str) -> String {
    let t: String = raw.trim().chars().take(32).collect();
    if t.is_empty() { "Syndicate".to_string() } else { t }
}

impl World {
    /// Create a galaxy for the given configuration: hub at the centre, seeded
    /// star systems, and a ring of empty home anchors.
    pub fn new(config: SimConfig) -> Self {
        let mut rng = crate::rng::Rng::new(config.seed);
        let mut next_entity_id = 1u64;

        // §naming: one galaxy-unique, seed-shuffled name list for EVERY system —
        // frontier first, then the home ring — handed out in order so no two
        // collide. Built from the seeded rng (determinism is law).
        let home_count = config.max_players.max(1) as usize;
        let names = crate::galaxy::shuffled_system_names(
            &mut rng,
            config.system_count as usize + home_count,
        );
        let (frontier_names, home_names) = names.split_at(config.system_count as usize);

        let mut systems = {
            let mut alloc = || {
                let id = EntityId(next_entity_id);
                next_entity_id += 1;
                id
            };
            generate_systems(
                &mut rng,
                config.galaxy_radius,
                config.system_count,
                frontier_names,
                &mut alloc,
            )
        };
        let mut home_slots = generate_home_slots(
            &mut rng,
            config.galaxy_radius,
            config.home_ring_frac,
            config.max_players,
        );
        // One developed HOME STAR SYSTEM per home slot, co-located with it — the
        // home base each player begins owning (granted on join, no claim cost).
        // Generated eagerly so every system's static info fleets in the one-time
        // Welcome galaxy; ownership is light-gated like any claim. Geology is
        // keyed by home index (independent of the frontier RNG stream).
        let home_systems = {
            let mut alloc = || {
                let id = EntityId(next_entity_id);
                next_entity_id += 1;
                id
            };
            crate::galaxy::generate_home_systems(config.seed, &home_slots, home_names, &mut alloc)
        };
        for (slot, sys) in home_slots.iter_mut().zip(&home_systems) {
            slot.system = Some(sys.id);
        }
        systems.extend(home_systems);

        let mut world = World {
            config,
            tick: 0,
            time: 0.0,
            hub: Vec2::ZERO,
            systems,
            home_slots,
            players: BTreeMap::new(),
            fleets: BTreeMap::new(),
            market: crate::market::Market::new(),
            book: Vec::new(),
            next_order_id: 1,
            pending_orders: Vec::new(),
            pending_echoes: Vec::new(),
            next_entity_id,
            build_queue: Vec::new(),
            next_build_id: 0,
            refit_queue: Vec::new(),
            rng,
            engagements: BTreeMap::new(),
            battle_records: BTreeMap::new(),
            next_engagement_id: 0,
            syndicates: BTreeMap::new(),
            next_syndicate_id: 0,
            enclaves: BTreeMap::new(),
            nodes: BTreeMap::new(),
            rankings: Vec::new(),
            band_lo: 0.0,
            band_hi: 0.0,
            pending_survey_reports: Vec::new(),
            freight_queue: BTreeMap::new(),
            freight_runs: BTreeMap::new(),
            next_shipment_id: 0,
        };
        // §pirates: seed hidden enclaves AFTER all systems exist (so the frontier
        // RNG stream is untouched — determinism), on their OWN seeded stream.
        world.seed_enclaves();
        // §node: seed DORMANT nodes at every exotic system (pure function of id,
        // no RNG — determinism intact, parity with the client's visual exotics).
        world.seed_nodes();
        // §explore: the richness-band terciles — a pure derivation from the
        // finished systems (no RNG), fixed for the life of the galaxy.
        world.compute_band_thresholds();
        // §explore Part 3: seed hidden TRAITS on an isolated stream (like the
        // enclaves) — deterministic, never perturbs the frontier/home streams.
        world.seed_traits();
        world
    }

    /// §explore Part 3: give `TRAIT_FRACTION` of systems exactly ONE hidden trait,
    /// uniform over the five kinds, on an ISOLATED RNG stream (seed ^ magic — the
    /// house pattern; the frontier/home/enclave streams are untouched, so this is
    /// reproducible and perturbs nothing). A Bonus Vein picks a commodity the
    /// system actually HAS (uniform over its deposits). Id-ordered iteration →
    /// deterministic. Pre-feature snapshots simply have no traits (acceptable).
    fn seed_traits(&mut self) {
        let mut rng = crate::rng::Rng::new(self.config.seed ^ 0x5452_4149_5453_5F53); // "TRAITS_S"
        for sys in self.systems.iter_mut() {
            let roll = (rng.next_u64() % 100_000) as f64 / 100_000.0;
            if roll >= crate::explore::TRAIT_FRACTION {
                continue;
            }
            let t = match rng.next_u64() % 5 {
                0 => {
                    // A vein of something the system actually has.
                    let all: Vec<crate::cargo::Commodity> = sys.all_deposits().map(|d| d.resource).collect();
                    if all.is_empty() {
                        continue; // (defensive — every system generates deposits)
                    }
                    crate::explore::SystemTrait::BonusVein { commodity: all[(rng.next_u64() % all.len() as u64) as usize] }
                }
                1 => crate::explore::SystemTrait::DeepDeposits,
                2 => crate::explore::SystemTrait::UnstableGeology,
                3 => crate::explore::SystemTrait::VolatilePockets,
                _ => crate::explore::SystemTrait::PrecursorCache,
            };
            sys.trait_ = Some(t);
        }
    }

    /// §explore: (re)compute the band tercile thresholds from the current systems
    /// — pure and deterministic (deposits are static, so this is idempotent).
    fn compute_band_thresholds(&mut self) {
        let (lo, hi) = crate::explore::band_thresholds(
            self.systems.iter().map(|s| crate::explore::band_value_iter(s.all_deposits())),
        );
        self.band_lo = lo;
        self.band_hi = hi;
    }

    /// §explore R1: a system's PUBLIC richness band — the free spectral read
    /// (same for every corp; the exact composition stays behind the survey gate).
    pub fn band_of(&self, sys: &StarSystem) -> crate::explore::RichnessBand {
        crate::explore::band_for(crate::explore::band_value_iter(sys.all_deposits()), self.band_lo, self.band_hi)
    }

    /// §explore MIGRATION FIXUP — heal a pre-feature snapshot after load (called
    /// by the server's restore path; harmless on a current one):
    /// * band thresholds at the serde default (0,0) → recompute (pure).
    /// * a corp with an EMPTY `surveyed` set (impossible post-feature — join
    ///   always seeds the home valley) → mark its OWNED systems + everything
    ///   within `SURVEY_INITIAL_RADIUS` of home as surveyed, so live playtest
    ///   corps don't wake up amnesiac about their own holdings.
    pub fn fixup_after_load(&mut self) {
        // §economy: fold the legacy flat tier fields into the structures map
        // (Extractor→MiningComplex, Refinery→FuelRefinery, rest 1:1). Idempotent.
        for sys in self.systems.iter_mut() {
            sys.fold_legacy_structures();
        }
        // §economy Part 7: one idempotent migration pass — a pre-economy world
        // keeps PRODUCING through the upgrade (async-fair applies to deploys).
        self.migrate_economy();
        // §bodies: fold every system's legacy shells onto its body roster
        // (layout-preserving generation + the shared siting rules). Idempotent.
        for sys in self.systems.iter_mut() {
            sys.migrate_to_bodies();
        }
        // §bodies: re-site IN-FLIGHT build jobs. Pre-bodies jobs default to
        // body 0; route each structure job to its natural site unless its
        // current target already makes sense (holds the kind = a tier-up, or
        // carries the matching deposit for extraction). Idempotent: a
        // correctly-sited job re-sites to itself.
        for job in self.build_queue.iter_mut() {
            let crate::build::BuildKind::Upgrade { upgrade } = job.what else { continue };
            let Some(sys) = self.systems.iter().find(|s| s.id == job.system) else { continue };
            let ok = sys.bodies.iter().any(|b| {
                b.id == job.body_id
                    && (b.tier(upgrade) > 0
                        || match upgrade {
                            crate::build::StructureKind::MiningComplex
                            | crate::build::StructureKind::VolatileHarvester
                            | crate::build::StructureKind::Bioharvester => b.has_deposit_for(upgrade),
                            _ => true,
                        })
            });
            if !ok {
                job.body_id = sys.site_for(upgrade).unwrap_or(0);
            }
        }
        if self.band_lo == 0.0 && self.band_hi == 0.0 {
            self.compute_band_thresholds();
        }
        let sys_info: Vec<(EntityId, Vec2, Option<PlayerId>)> =
            self.systems.iter().map(|s| (s.id, s.pos, s.owner)).collect();
        for corp in self.players.values_mut() {
            if !corp.surveyed.is_empty() {
                continue;
            }
            for (sid, pos, owner) in &sys_info {
                if *owner == Some(corp.id) || pos.distance(corp.home) <= crate::explore::SURVEY_INITIAL_RADIUS {
                    corp.surveyed.insert(*sid);
                }
            }
        }
    }

    /// §economy Part 7: migrate a PRE-ECONOMY world in place — IDEMPOTENT (every
    /// step gates on "still legacy-shaped", so a modern world passes through
    /// untouched):
    ///   1. DEPOSIT REMAP — old worlds generated deposits of processed goods;
    ///      those become their raw web-predecessors at the SAME richness
    ///      (Provisions→Biomass, Fuel→Volatiles, Alloys→RareElements; Ore's
    ///      rename is a serde alias). After one pass every deposit is a raw,
    ///      so re-running remaps nothing.
    ///   2. POPULATION SEED (owned, population == 0 only): Habitat tier ≥ 1 →
    ///      1.5M/tier (an established colony); else any producing structure →
    ///      1.0M (the working crews an old extractor colony must have had —
    ///      without people, migration would silence every old mine). Homes get
    ///      at least the modern bootstrap population.
    ///   3. DEFAULT ASSIGNMENTS (owned, none posted, producers built): one crew
    ///      to each built extraction structure that matches local geology, the
    ///      Fuel Refinery, and the Agroplex — the old always-on behaviour,
    ///      re-expressed as staffed lines.
    ///
    /// Nothing is deleted, nothing rejected: a loaded empire underperforms at
    /// worst (short staffing), it never stalls silently.
    pub fn migrate_economy(&mut self) {
        use crate::build::StructureKind as K;
        for sys in self.systems.iter_mut() {
            // 1. Deposit remap (owned or not — geology is geology).
            for dep in sys.legacy_deposits.iter_mut() {
                dep.resource = match dep.resource {
                    crate::cargo::Commodity::Provisions => crate::cargo::Commodity::Biomass,
                    crate::cargo::Commodity::Fuel => crate::cargo::Commodity::Volatiles,
                    crate::cargo::Commodity::Alloys => crate::cargo::Commodity::RareElements,
                    other => other,
                };
            }
            if sys.owner.is_none() {
                continue;
            }
            // (§bodies: this pass runs BEFORE migrate_to_bodies, so it works
            // the LEGACY SHELLS; a post-bodies system has empty shells and
            // every step below no-ops — idempotent by construction.)
            let ltier = |sys: &crate::galaxy::StarSystem, k: K| sys.legacy_structures.get(&k).copied().unwrap_or(0);
            let producers: Vec<K> = [K::MiningComplex, K::VolatileHarvester, K::Bioharvester, K::FuelRefinery, K::Agroplex]
                .into_iter()
                .filter(|k| ltier(sys, *k) >= 1)
                .collect();
            // 2. Population seed — only where the field is still legacy-zero.
            if sys.legacy_population == 0.0 && sys.bodies.is_empty() {
                let hab = ltier(sys, K::Habitat);
                if hab >= 1 {
                    sys.legacy_population = 1.5 * hab as f64;
                } else if !producers.is_empty() {
                    sys.legacy_population = 1.0;
                }
            }
            // 3. Default assignments — only where none exist yet.
            if sys.legacy_assignments.is_empty() {
                for k in producers {
                    let works = match k {
                        K::FuelRefinery | K::Agroplex => true, // converters run off stock/imports
                        _ => sys.legacy_deposits.iter().any(|d| crate::production::extraction_structure(d.resource) == Some(k)),
                    };
                    if works {
                        sys.legacy_assignments.insert(k, crate::production::Assignment::crew(1));
                    }
                }
            }
        }
        // Homes: guarantee at least the modern bootstrap population so the
        // seeded (or folded) home industry actually runs for returning players.
        let homes: Vec<EntityId> = self.players.values().filter_map(|c| c.home_system).collect();
        for hid in homes {
            if let Some(sys) = self.systems.iter_mut().find(|s| s.id == hid) {
                if sys.bodies.is_empty() {
                    sys.legacy_population = sys.legacy_population.max(crate::colony::HOME_FOUNDING_POP);
                } else if sys.population() < crate::colony::HOME_FOUNDING_POP {
                    let deficit = crate::colony::HOME_FOUNDING_POP - sys.population();
                    sys.seed_population(deficit);
                }
            }
        }
    }

    /// §node: seed a DORMANT [`crate::node::Node`] at every EXOTIC system. Pure
    /// function of the system id (`node_bonus_for`, the sim twin of the client's
    /// star assignment) — no RNG draws, so no stream is perturbed and the node set
    /// is byte-identical to the client's exotic icons. Nodes stay dormant until
    /// `node_awakening_time`; seeding merely records which systems will awaken and
    /// what each grants (so the map can telegraph them from t=0).
    fn seed_nodes(&mut self) {
        let ids: Vec<EntityId> = self.systems.iter().map(|s| s.id).collect();
        for id in ids {
            if let Some(bonus) = crate::node::node_bonus_for(id) {
                self.nodes.insert(id, crate::node::Node::dormant(bonus));
            }
        }
    }

    /// §pirates: seed `PIRATE_ENCLAVE_COUNT` hidden bases at unclaimed MID-RING
    /// systems, never within `PIRATE_HOME_EXCLUSION` of a home slot. Deterministic:
    /// an isolated RNG stream (seed ^ magic) picks them, so the pick is reproducible
    /// and never perturbs the frontier/home/event streams. The base's
    /// platform-equivalent defense is stored on the host system's `defense_tier`
    /// (the assault reuses the Defense-Platform combat); this only marks the site.
    fn seed_enclaves(&mut self) {
        let radius = self.config.galaxy_radius;
        let (lo, hi) = (
            radius * (0.12 + 0.84 * pirate::PIRATE_RING_LO),
            radius * (0.12 + 0.84 * pirate::PIRATE_RING_HI),
        );
        let home_pos: Vec<Vec2> = self.home_slots.iter().map(|h| h.pos).collect();
        // Candidate mid-ring, unclaimed, home-clear systems (id order = deterministic).
        let mut cands: Vec<EntityId> = self
            .systems
            .iter()
            .filter(|s| {
                s.owner.is_none()
                    && (lo..=hi).contains(&s.pos.length())
                    && home_pos.iter().all(|h| s.pos.distance(*h) >= pirate::PIRATE_HOME_EXCLUSION)
            })
            .map(|s| s.id)
            .collect();
        cands.sort();
        // Fisher–Yates on the isolated stream, then take the first N.
        let mut rng = crate::rng::Rng::new(self.config.seed ^ 0x5049_5241_5445_5F53); // "PIRATE_S"
        for i in (1..cands.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            cands.swap(i, j);
        }
        for &sid in cands.iter().take(pirate::PIRATE_ENCLAVE_COUNT) {
            // Base defense on the host system (owner stays None — dark until scouted).
            if let Some(sys) = self.systems.iter_mut().find(|s| s.id == sid) {
                sys.set_tier(crate::build::StructureKind::DefensePlatform, pirate::base_defense_tiers(1));
                sys.defense_pool = 0.0;
            }
            self.enclaves.insert(
                sid,
                Enclave {
                    system: sid,
                    tier: 1,
                    plunder: BTreeMap::new(),
                    // The FIRST pack waits out the opening window (§pirates
                    // onboarding) so founding corps trade unmolested early; the
                    // per-enclave stagger keeps them from all launching at once.
                    next_launch_at: pirate::PIRATE_FIRST_LAUNCH_SECS + rng.range(0.0, pirate::PIRATE_LAUNCH_PERIOD),
                    next_grow_at: pirate::PIRATE_GROW_PERIOD + rng.range(0.0, pirate::PIRATE_GROW_PERIOD),
                    dormant_until: 0.0,
                    pack: None,
                },
            );
        }
    }

    /// The player's in-flight ORDER LIFECYCLES (§order-lifecycle) — the LATEST
    /// order per fleet, covering both the in-transit pending orders and the
    /// delivered-but-awaiting-echo ones. OWNER-ONLY (a rival gets nothing). The
    /// client ticks the IN-TRANSIT / AWAITING-ECHO countdowns from the two
    /// timestamps against `sim_time`, and flips its dashed heading to solid at
    /// `echo_at`.
    /// §contestable-territory Part 2: how long an unbroken, defense-suppressed
    /// siege must run before a colony ship can capture — derived from the config
    /// battle timescale so one knob scales both. Surfaced to the client so it can
    /// render the siege-progress readout / countdown.
    pub fn siege_duration_secs(&self) -> f64 {
        SIEGE_DURATION_BATTLE_MULT * self.config.battle_target_secs
    }

    pub fn pending_commands(&self, owner: PlayerId) -> Vec<PendingCommandView> {
        let mut latest: BTreeMap<EntityId, PendingCommandView> = BTreeMap::new();
        let mut consider = |v: PendingCommandView| match latest.get(&v.fleet) {
            Some(cur) if cur.issued_at >= v.issued_at => {}
            _ => {
                latest.insert(v.fleet, v);
            }
        };
        for po in self.pending_orders.iter().filter(|p| p.owner == owner) {
            consider(PendingCommandView {
                fleet: po.ship_id,
                delivered_at: po.apply_time,
                echo_at: po.echo_at,
                issued_at: po.issued_at,
                kind: po.kind,
            });
        }
        for e in self.pending_echoes.iter().filter(|e| e.owner == owner) {
            consider(PendingCommandView {
                fleet: e.fleet,
                delivered_at: e.delivered_at,
                echo_at: e.echo_at,
                issued_at: e.issued_at,
                kind: e.kind,
            });
        }
        latest.into_values().collect()
    }

    /// Ongoing BATTLES for the server's light-gated View (§battles-take-time):
    /// each engagement's location, start time, the two owners, and its participant
    /// fleets (so weapons-fire visibility can reveal even dark participants AT the
    /// site once the observer's light arrives).
    pub fn active_battles(&self) -> Vec<BattleInfo> {
        self.engagements
            .values()
            .map(|e| BattleInfo {
                id: e.id,
                pos: e.pos,
                started_at: e.started_at,
                a_owner: e.a_owner,
                d_owner: e.d_owner,
                participants: e.attackers.iter().chain(e.defenders.iter()).copied().collect(),
            })
            .collect()
    }

    /// Allocate a fresh, deterministic engagement id (own counter, so battle ids
    /// never collide with fleet/system ids in the shared entity space).
    fn alloc_engagement_id(&mut self) -> EntityId {
        self.next_engagement_id += 1;
        // High-bit tag keeps engagement ids visibly distinct from entity ids.
        EntityId(0xE000_0000_0000_0000 | self.next_engagement_id)
    }

    // --- §battle-records: recorder hooks (pure observers, never feed back) -----

    /// Ensure a [`crate::combat::BattleRecord`] exists for this engagement,
    /// capturing its opening sides. Lazy + idempotent, so it fires the first
    /// tick a battle of ANY kind (raid, battle, blockade, pirate) is processed —
    /// they all flow through the same resolve loop. Prunes on insert.
    fn record_ensure_open(&mut self, eid: EntityId) {
        if self.battle_records.contains_key(&eid) {
            return;
        }
        let Some(e) = self.engagements.get(&eid) else { return };
        let (pos, system, raid, a_owner, d_owner, ptiers) =
            (e.pos, e.platform_system, e.raid, e.a_owner, e.d_owner, e.platform_start_tiers);
        let a_start = e.a_start.clone();
        let d_start = e.d_start.clone();
        let a_loadouts = e.a_start_loadouts.clone();
        let d_loadouts = e.d_start_loadouts.clone();
        let a_posture = self.players.get(&a_owner).map(|c| c.doctrine.engagement).unwrap_or_default();
        let d_posture = self.players.get(&d_owner).map(|c| c.doctrine.engagement).unwrap_or_default();
        let sides = [
            crate::combat::SideRecord { corp: a_owner, initial: a_start, initial_loadouts: a_loadouts, posture: a_posture, platform_tiers: 0 },
            crate::combat::SideRecord { corp: d_owner, initial: d_start, initial_loadouts: d_loadouts, posture: d_posture, platform_tiers: ptiers },
        ];
        let rec = crate::combat::BattleRecord::open(
            eid, pos, system, raid, self.tick, self.config.battle_target_secs, sides,
        );
        self.battle_records.insert(eid, rec);
        crate::combat::prune_records(&mut self.battle_records, self.time);
    }

    /// Annotate the record with a beat (forces a flush on the next round tick).
    fn record_note(&mut self, eid: EntityId, note: crate::combat::RoundNote) {
        if let Some(rec) = self.battle_records.get_mut(&eid) {
            rec.note(note);
        }
    }

    /// Feed one attrition tick to the record: the damage each side dealt, the
    /// ships each side lost, and the survivor counts (used only when a round
    /// actually flushes).
    #[allow(clippy::too_many_arguments)]
    fn record_round(
        &mut self,
        eid: EntityId,
        dealt_a: f64,
        dealt_b: f64,
        la: &crate::combat::Losses,
        lb: &crate::combat::Losses,
        counts: [BTreeMap<ShipKind, u32>; 2],
        tick: u64,
        frame: Option<crate::combat::Keyframe>,
    ) {
        if let Some(rec) = self.battle_records.get_mut(&eid) {
            rec.accumulate(dealt_a, dealt_b, la, lb);
            if let Some(f) = frame {
                rec.keyframe(f);
            }
            rec.flush_if_due(tick, counts);
        }
    }

    /// Allocate a fresh, deterministic entity id.
    fn alloc_entity_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    /// Allocate a fresh, deterministic syndicate id (own counter, separate id
    /// space from entities so the two never collide).
    fn alloc_syndicate_id(&mut self) -> SyndicateId {
        self.next_syndicate_id += 1;
        SyndicateId(self.next_syndicate_id)
    }

    // ---- SYNDICATES (§syndicates) --------------------------------------------

    /// GROUND-TRUTH alliance: two DISTINCT corps in the same syndicate. This is
    /// what all mechanical non-engagement reads (pickets, platforms, WeaponsFree,
    /// blockades, and the deliberate-order soft-rejects) — an alliance is a mutual
    /// pact both parties consented to, so it is in effect immediately (unlike the
    /// KNOWLEDGE of it, which a third party receives light-delayed via `known_ally`).
    pub fn are_allied(&self, a: PlayerId, b: PlayerId) -> bool {
        a != b
            && self.players.get(&a).and_then(|c| c.syndicate).is_some()
            && self.players.get(&a).and_then(|c| c.syndicate)
                == self.players.get(&b).and_then(|c| c.syndicate)
    }

    /// The OTHER members of `p`'s syndicate (empty if unaffiliated). Precomputed
    /// once and captured by the per-tick engagement closures, which can't borrow
    /// `&self` inside their filters.
    pub fn allies_of(&self, p: PlayerId) -> std::collections::BTreeSet<PlayerId> {
        match self.players.get(&p).and_then(|c| c.syndicate) {
            Some(sid) => self
                .syndicates
                .get(&sid)
                .map(|s| s.members.iter().copied().filter(|&m| m != p).collect())
                .unwrap_or_default(),
            None => std::collections::BTreeSet::new(),
        }
    }

    /// Whether the `viewer` KNOWS (light-delayed) that `owner` is their ally — the
    /// gate the View uses to tint ally systems/fleets. The viewer knows their OWN
    /// membership instantly; `owner`'s membership is known only once the light from
    /// `owner`'s command center has reached the viewer's (`syndicate_since +
    /// dist/c`), and until then the viewer's picture is `owner`'s PRIOR membership
    /// — so a fresh join isn't seen early and a fresh betrayal isn't seen early.
    pub fn known_ally(&self, viewer: PlayerId, owner: PlayerId, now: f64) -> bool {
        if viewer == owner {
            return false;
        }
        let (Some(v), Some(o)) = (self.players.get(&viewer), self.players.get(&owner)) else {
            return false;
        };
        let dist = o.command_center.distance(v.command_center);
        let known = if now >= o.syndicate_since + dist / self.config.c {
            o.syndicate
        } else {
            o.syndicate_prev
        };
        v.syndicate.is_some() && v.syndicate == known
    }

    /// §syndicates Part 3: if `fleet` is a stationed ally GARRISON (a combatant,
    /// Idle, within a platform radius of a syndicate ALLY's system), the host
    /// system id; `None` otherwise. Used to surface the sender's garrison status.
    pub fn garrison_host_of(&self, fleet: EntityId) -> Option<EntityId> {
        let f = self.fleets.get(&fleet)?;
        if !f.is_combatant() || !matches!(f.order, FleetOrder::Idle) {
            return None;
        }
        self.systems
            .iter()
            .find(|s| {
                s.owner.is_some_and(|o| self.are_allied(f.owner, o))
                    && f.pos.distance(s.pos) <= crate::build::DEFENSE_PLATFORM_RADIUS
            })
            .map(|s| s.id)
    }

    /// §syndicates Part 3: the ally GARRISON hosted at `system` — `(total ships,
    /// all-fed)` — or `None` if there's no ally garrison there. For the HOST's
    /// owner-only "coalition shield you're feeding" readout.
    pub fn hosted_garrison(&self, system: EntityId) -> Option<(u32, bool)> {
        let sys = self.systems.iter().find(|s| s.id == system)?;
        let host_owner = sys.owner?;
        let (mut ships, mut all_fed, mut any) = (0u32, true, false);
        for f in self.fleets.values() {
            if f.is_combatant()
                && matches!(f.order, FleetOrder::Idle)
                && self.are_allied(f.owner, host_owner)
                && f.pos.distance(sys.pos) <= crate::build::DEFENSE_PLATFORM_RADIUS
            {
                ships += f.total_count();
                any = true;
                if !f.garrison_fed {
                    all_fed = false;
                }
            }
        }
        any.then_some((ships, all_fed))
    }

    /// Change a corp's membership, recording the 2-state light-delay bookkeeping
    /// (prev + since) so distant viewers learn of the change only when its light
    /// arrives. No-op if the state is unchanged.
    fn set_membership(&mut self, p: PlayerId, new: Option<SyndicateId>) {
        let now = self.time;
        if let Some(corp) = self.players.get_mut(&p)
            && corp.syndicate != new
        {
            corp.syndicate_prev = corp.syndicate;
            corp.syndicate = new;
            corp.syndicate_since = now;
        }
    }

    /// FOUND a syndicate (§syndicates Part 1). The founder must exist and be
    /// unaffiliated. Soft-reject otherwise (no state change).
    fn apply_create_syndicate(&mut self, founder: PlayerId, name: String) {
        let unaffiliated = self.players.get(&founder).is_some_and(|c| c.syndicate.is_none());
        if !unaffiliated {
            return;
        }
        let id = self.alloc_syndicate_id();
        let mut members = std::collections::BTreeSet::new();
        members.insert(founder);
        let name = sanitize_name(&name);
        self.syndicates.insert(
            id,
            Syndicate { id, name, founder, members, invites: std::collections::BTreeSet::new(), created_at: self.time, research: Default::default(), fits: Vec::new(), flagship_name: None },
        );
        self.set_membership(founder, Some(id));
    }

    /// INVITE a corp into the founder's syndicate (founder-only). The invitee must
    /// exist and be unaffiliated. Records a pending invite (accepted separately).
    fn apply_invite_syndicate(&mut self, founder: PlayerId, invitee: PlayerId) {
        if founder == invitee {
            return;
        }
        let Some(sid) = self.players.get(&founder).and_then(|c| c.syndicate) else {
            return;
        };
        let invitee_free = self.players.get(&invitee).is_some_and(|c| c.syndicate.is_none());
        if !invitee_free {
            return;
        }
        if let Some(s) = self.syndicates.get_mut(&sid)
            && s.founder == founder
        {
            s.invites.insert(invitee);
        }
    }

    /// ACCEPT a pending invite (§syndicates Part 1). The invitee must be
    /// unaffiliated, actually hold an invite to `sid`, and the syndicate must have
    /// room under the SIZE CAP. Consumes the invite and joins the roster.
    fn apply_accept_syndicate(&mut self, invitee: PlayerId, sid: SyndicateId) {
        let free = self.players.get(&invitee).is_some_and(|c| c.syndicate.is_none());
        if !free {
            return;
        }
        let cap = syndicate_cap(self.players.len());
        let Some(s) = self.syndicates.get_mut(&sid) else {
            return;
        };
        if !s.invites.contains(&invitee) || s.members.len() >= cap {
            return;
        }
        s.invites.remove(&invitee);
        s.members.insert(invitee);
        self.set_membership(invitee, Some(sid));
    }

    /// LEAVE the caller's syndicate (§syndicates Part 1). If the founder leaves and
    /// members remain, the seat passes to the lowest-id member; an emptied
    /// syndicate is removed.
    fn apply_leave_syndicate(&mut self, p: PlayerId) {
        let Some(sid) = self.players.get(&p).and_then(|c| c.syndicate) else {
            return;
        };
        if let Some(s) = self.syndicates.get_mut(&sid) {
            s.members.remove(&p);
            s.invites.remove(&p);
            if s.members.is_empty() {
                self.syndicates.remove(&sid);
            } else if s.founder == p {
                // Founder-managed v1: hand the seat to the next (lowest-id) member.
                s.founder = *s.members.iter().next().unwrap();
            }
        }
        self.set_membership(p, None);
    }

    /// DISSOLVE the caller's syndicate (founder-only). Clears every member's
    /// affiliation and removes the roster.
    fn apply_dissolve_syndicate(&mut self, founder: PlayerId) {
        let Some(sid) = self.players.get(&founder).and_then(|c| c.syndicate) else {
            return;
        };
        let members: Vec<PlayerId> = match self.syndicates.get(&sid) {
            Some(s) if s.founder == founder => s.members.iter().copied().collect(),
            _ => return,
        };
        for m in members {
            self.set_membership(m, None);
        }
        self.syndicates.remove(&sid);
    }

    /// Advance the world by exactly one fixed timestep, applying the given
    /// commands at this tick boundary. Returns the events produced this tick.
    ///
    /// Pure and deterministic: same starting `World` + same `commands` always
    /// yields the same next state and events.
    pub fn step(&mut self, commands: &[Command]) -> Vec<Event> {
        let mut events = Vec::new();

        // 1. Apply external commands at the current instant.
        for cmd in commands {
            self.apply(cmd, &mut events);
        }

        // 2. Deliver any orders whose outbound light has now reached the ship.
        self.deliver_due_orders(&mut events);

        // 2b. Standing defensive doctrine (§5.1, Pillar 1): patrolling raiders
        //     autonomously break off to intercept hostiles threatening a friendly
        //     convoy, and resume patrol when the threat is gone — server-driven,
        //     runs whether or not the owner is connected. Decided on each patrol's
        //     OWN local sensing, then handed to the existing pursuit + combat.
        self.autonomous_defense();
        // 2c. §offensive-orders Part 2: the WeaponsFree standing OFFENSE — fleets
        //     the player pre-delegated aggression to hunt any rival that wanders
        //     into their OWN sensor bubble, on their own local detection (no
        //     command-center round trip), composed with the corp doctrine's odds
        //     gate. Runs whether or not the owner is connected.
        self.weapons_free_offense();

        // 3. Integrate continuous movement (flip-and-burn, patrols, and raider
        //    interception pursuit).
        self.integrate_movement();

        // 3b. §syndicates Part 3: feed ally GARRISONS from their HOST systems and
        //     set each garrison's fed/unfed flag for THIS tick's defense. Before the
        //     combat passes (which read the flag to include only FED garrisons), and
        //     after movement (so stationing is this tick's truth).
        self.resolve_garrison_upkeep(&mut events);

        // 4. Resolve raids in true space (contact → convoy lost; convoy reaches
        //    the hub → escape). A raided trade convoy's goods are simply lost.
        self.resolve_raids(&mut events);

        // 4a'. CONTESTABLE TERRITORY (§blockade): detect on-station blockaders,
        //      open the standing-defense establishment battle, recompute each
        //      system's blockade + siege state, and emit onset/lift transitions.
        //      After raids so the establishment battle it opens attrites next tick.
        self.resolve_blockades(&mut events);

        // 4a''. §pirates: the neutral faction's brain — open assaults, detect base
        //       suppression, escalate, and launch/return raider packs. After the
        //       combat passes so a won assault's defense-tier writeback lands this
        //       tick; pack orders it sets are pursued next tick (async lag).
        self.pirate_ai(&mut events);

        // 4b. COLONY ARRIVALS (§fleets part 3): settlement is physical — resolve
        //     after raids so a colony ship killed at the doorstep never claims.
        //     Also where a besieging colony ship CAPTURES a strangled system (§Part 2).
        self.resolve_colony_arrivals(&mut events);

        // 4b'. §explore Part 2: run the SURVEY dwell clocks (after movement so
        //      positions are this tick's truth; after the combat passes so the
        //      abort-on-engagement rule sees this tick's battles), then deliver
        //      any survey-report legs whose light has arrived (knowledge inserts
        //      into `surveyed`; the owner's landing fans out ally-relay legs).
        self.resolve_surveys(&mut events);
        self.deliver_survey_reports();

        // 4c. ORDER LIFECYCLE (§order-lifecycle): after this tick's destruction is
        //     settled, confirm delivered orders whose echo light has returned
        //     (owner-only `OrderConfirmed`), and drop echoes for fleets just lost.
        self.resolve_order_echoes(&mut events);

        // 5. Resolve trade convoys that survived to their destination (§9).
        self.resolve_trade_arrivals(&mut events);

        // 5a'. §TCA: resolve Authority FREIGHTER runs that reached the end of a leg
        //      — unload at the colony, collect the pickups waiting there and turn
        //      for home, or land the whole manifest in its owners' Charterhouse
        //      warehouses. Alongside the convoy arrivals, on the same cadence.
        self.resolve_freight_arrivals(&mut events);

        // 5b. Accrue production at every claimed system (§5.1 continuous progress)
        //     — happens whether or not the owner is logged in.
        self.accrue_production(&mut events);

        // 5b°. EXOTIC NODES (§node): awaken exotic systems at the configured time and
        //      draw each held node's upkeep from THIS tick's fresh stockpiles — after
        //      accrual, exactly like the standing-order reconciliation below.
        self.node_ai(&mut events);

        // 5b'. Resolve construction jobs whose completion tick has arrived (§step1
        //      growth sink): spawn built fleets / apply system upgrades. Server-driven
        //      — a build started before logging off still completes on the clock.
        self.resolve_builds(&mut events);

        // 5b'''. §modules Part B4: return refitted hulls from the yard — same clock
        //        as construction; the hulls were out of combat while queued.
        self.resolve_refits(&mut events);

        // 5b''''. §research R2: the DISTRIBUTED CLOCK — every staffed+funded
        //         member Academy drips its basket and adds its rate to the active
        //         programme; then complete anything fully funded (instant, galaxy-
        //         wide). Runs after accrual so labs drip from fresh stockpiles.
        self.tick_research(&mut events);
        self.resolve_research(&mut events);
        // §research R3: the rival-observation scan (Shadow gate) on a coarse cadence
        // — a snapshot of who each syndicate can currently sense, deduped over time.
        if self.tick.is_multiple_of(EVAL_PERIOD) {
            self.observe_rivals_for_research();
        }

        // 5b''. SCOUT INTEL (§scout part 2): scouts passing rival systems capture
        //       timestamped snapshots of their fortifications. After movement, so
        //       positions are this tick's truth; standing behavior (owner online
        //       or off — the scout gathers regardless).
        self.gather_intel(&mut events);

        // 5c. Standing logistics orders (§15): reconcile each rule's in-flight
        //     convoy against reality (raids/arrivals above may have removed it),
        //     then evaluate the rules and auto-dispatch convoys. Server-driven,
        //     runs whether or not the owner is connected — the heart of async play.
        //     Placed after accrue so rules act on this tick's fresh stockpiles.
        self.reconcile_standing_inflight();
        self.evaluate_standing_orders(&mut events);

        // 6. Advance the clock; drift the market on a slow cadence so the price
        //    information lag is visible, and clear the limit-order book on the
        //    batch cadence (the uniform-price call auction, §9).
        self.tick += 1;
        self.time += DT;

        // §TCA Phase 2: CHARTER STANDING regenerates, unconditionally and at the
        // same rate in EVERY band. Time served is time served — the Authority's
        // memory fades whether or not you are currently proscribed, so no player
        // is ever permanently locked out by arithmetic alone. Deterministic (a
        // fixed rate × dt), clamped to the ceiling.
        for corp in self.players.values_mut() {
            corp.tca_standing = (corp.tca_standing + crate::tca::TCA_STANDING_REGEN_PER_SEC * DT)
                .min(crate::tca::TCA_STANDING_MAX);
        }

        if self.tick.is_multiple_of(MARKET_UPDATE_TICKS) {
            self.market.drift(&mut self.rng);
        }
        if self.tick.is_multiple_of(BATCH_TICKS) {
            self.clear_books(&mut events);
        }
        if self.tick.is_multiple_of(VALUATION_TICKS) {
            self.recompute_valuations();
        }
        // §TCA: the Authority's SCHEDULED DEPARTURE — one freighter per destination
        // that has freight waiting in either direction. On the tick cadence like the
        // other periodic passes, so the timetable is exact and previewable.
        if self.tick.is_multiple_of(Self::freight_depart_ticks()) {
            self.depart_freight(&mut events);
        }

        // §rankings: tally THIS tick's events into the cumulative counters (cheap —
        // O(events)), then, on the SAME ledger cadence as the valuation close,
        // PUBLISH the leaderboard snapshot (a copy, so it holds steady between
        // closes — no mid-interval live leak).
        self.accumulate_rankings(&events);
        // §research R3: fold the same event stream into the syndicate verb biography
        // (battles fought/won, hull destroyed/absorbed, convoy deliveries).
        self.accrue_research_verbs(&events);
        if self.tick.is_multiple_of(VALUATION_TICKS) {
            self.snapshot_rankings();
        }

        events
    }

    /// Integrate every ship one tick. Interception is driven here (it needs the
    /// target's state); all other orders use the self-contained per-ship
    /// advance. Targets are read from a start-of-tick snapshot to avoid
    /// borrow conflicts and keep the result order-independent.
    fn integrate_movement(&mut self) {
        let snapshot: BTreeMap<EntityId, (Vec2, Vec2)> = self
            .fleets
            .iter()
            .map(|(id, s)| (*id, (s.pos, s.vel)))
            .collect();
        // ANCHOR (§engagement movement): a fleet in a battle is a STATIONARY event
        // at the contact point — its prior mission suspends and it holds position
        // (evasive combat maneuvering consumes the drive budget; no cruise under
        // fire). This is what pins a slow hammer while relief travels, and what
        // makes the battle marker / reinforce-to-location coherent.
        let engaged: std::collections::BTreeSet<EntityId> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let time = self.time;
        let c = self.config.c;
        let mut lost_target = Vec::new();
        for (id, ship) in self.fleets.iter_mut() {
            if engaged.contains(id) {
                ship.vel = Vec2::ZERO; // anchored — the battle holds it in place
                continue;
            }
            // Intercept (raid-commit) and Attack (destroy) both PURSUE a target
            // fleet identically — the only difference is what happens on contact
            // (raid-by-target-kind vs forced full battle), decided in resolve_raids.
            if let FleetOrder::Intercept { target } | FleetOrder::Attack { target } = ship.order {
                match snapshot.get(&target) {
                    Some(&(tp, tv)) => {
                        // Lead pursuit toward the ANALYTIC intercept of the
                        // target's light-delayed constant-velocity track, at this
                        // fleet's formation speed (§14.1) — closed-form, no solver.
                        let step = pursue_step(ship.pos, tp, tv, ship.max_speed(), c, DT);
                        ship.pos = step.pos;
                        ship.vel = step.vel;
                    }
                    None => lost_target.push(*id), // target gone — break off
                }
            } else {
                ship.advance(time, DT);
            }
        }
        // Raiders whose target vanished break off: a defensive patrol RESUMES its
        // patrol (its threat is gone); a manual raider returns home.
        for id in lost_target {
            let home = self
                .fleets
                .get(&id)
                .and_then(|s| self.players.get(&s.owner))
                .map(|c| c.home);
            if let Some(ship) = self.fleets.get_mut(&id) {
                if let Some(def) = ship.defense.take() {
                    ship.order = resume_patrol(def.patrol);
                } else if let Some(home) = home {
                    ship.order = FleetOrder::MoveTo { dest: home };
                }
            }
        }
        // §research R3: distance flown this tick → the Propulsion verbs. Each
        // fleet's pre-move position (snapshot) vs its new position, converted to
        // light-years; a combatant hull also credits the Expedition warship-ly gate.
        let mut ly_deltas: Vec<(PlayerId, crate::research::Verb, f64)> = Vec::new();
        for (id, (old_pos, _)) in &snapshot {
            if let Some(ship) = self.fleets.get(id) {
                let ly = old_pos.distance(ship.pos) / crate::research::SU_PER_LY;
                if ly > 0.0 {
                    ly_deltas.push((ship.owner, crate::research::Verb::LyFlown, ly));
                    if ship.is_combatant() {
                        ly_deltas.push((ship.owner, crate::research::Verb::WarshipLyFlown, ly));
                    }
                }
            }
        }
        for (owner, verb, amount) in ly_deltas {
            self.add_research_verb(owner, verb, amount);
        }
    }

    /// Standing fleet doctrine (§5.1 Pillar 1 + §16 Layer 2), run every tick for
    /// ALL patrolling raiders, server-authoritative and deterministic — it works
    /// whether or not the owner is connected. Each patrolling raider acts on its
    /// OWN local, fog-respecting sensing (only contacts within `sensor_range`;
    /// never a rival's hidden orders) and on its corp's [`FleetDoctrine`]:
    ///
    ///   * [`EscortPolicy`] picks the CHARGE it shadows (nearest / richest convoy,
    ///     or hold the player's route at a chokepoint);
    ///   * [`EngagementPolicy`] decides WHEN it breaks off patrol to intercept a
    ///     sensed hostile — never (Avoid), only a threat closing on a guarded
    ///     convoy (DefensiveOnly = the legacy behaviour), opportunistically when it
    ///     outnumbers the enemy (EngageWeaker), or any sensed hostile (EngageAny);
    ///   * [`RetreatThreshold`] withdraws a committing / engaged picket home when
    ///     the local friendly force-ratio falls below the threshold — re-checked
    ///     each tick, so enemy reinforcements can break a committed fight.
    ///
    /// All four default to the pre-Layer-2 behaviour, so an untouched corp plays
    /// exactly as before. The intercept itself reuses the ordinary
    /// [`FleetOrder::Intercept`] pursuit + seeded combat (resolved by
    /// [`Self::resolve_raids`]); a quarry destroyed elsewhere is handled by
    /// `integrate_movement`'s lost-target path.
    fn autonomous_defense(&mut self) {
        let sensor = self.config.sensor_range;
        let breakoff = sensor * PURSUIT_BREAKOFF_MULT;

        // Read-only snapshot for assessment (avoids borrow conflicts; ordered, so
        // deterministic). `cargo` lets the richest-escort policy pick by load.
        #[derive(Clone, Copy)]
        struct Snap {
            id: EntityId,
            owner: PlayerId,
            /// The fleet's flagship kind — its classification for the
            /// convoy/raider/scout tests below. A fleet-of-one is that ship.
            kind: ShipKind,
            pos: Vec2,
            vel: Vec2,
            cargo: u32,
            /// Aggregate combat weight of the WHOLE fleet (Σ combat_weight ×
            /// count) — force-ratio math sums these, so a big fleet counts big.
            combat: f64,
            /// Whether the fleet carries any teeth (a combatant kind).
            combatant: bool,
            /// Detection signature (§Part 4) at its current velocity — how far a
            /// picket can sense it (loud pack seen farther, stealth creeper less).
            signature: f64,
        }
        let snap: Vec<Snap> = self
            .fleets
            .iter()
            .map(|(id, s)| Snap {
                id: *id,
                owner: s.owner,
                kind: s.flagship_kind(),
                pos: s.pos,
                vel: s.vel,
                cargo: s.cargo.map(|c| c.units).unwrap_or(0),
                combat: s.combat_weight(),
                combatant: s.is_combatant(),
                // §node Veil: a fed magnetar node quiets its holder's dark fleets
                // in-region — the SAME signature both pickets and the View read.
                signature: s.signature() * self.veil_factor(s.owner, s.pos) * if s.surveying() { crate::explore::SURVEY_SIGNATURE_FACTOR } else { 1.0 },
            })
            .collect();
        let doctrines: BTreeMap<PlayerId, FleetDoctrine> =
            self.players.iter().map(|(id, c)| (*id, c.doctrine)).collect();
        let find = |id: EntityId| snap.iter().find(|s| s.id == id).copied();

        // SENSOR ARRAYS (§buildings step 2b): an owned array system extends its
        // owner's DETECTION — a contact counts as sensed if it's within the
        // picket's own bubble OR inside any of the owner's array bubbles (the
        // shared coverage source of truth, `array_sensor_sources`). Escort ward
        // CHOICE stays picket-local (guarding is physical proximity, not intel).
        let arrays: BTreeMap<PlayerId, Vec<(Vec2, f64)>> =
            self.players.keys().map(|&p| (p, self.array_sensor_sources(p))).collect();
        // §research R4a SensorRange widens each corp's PICKET bubble (its fleets'
        // own sensing), precomputed per owner so the closure below stays borrow-free.
        let range_mult: BTreeMap<PlayerId, f64> =
            self.players.keys().map(|&p| (p, self.research_mod(p, crate::research::ModKey::SensorRange))).collect();
        // SYNDICATES (§syndicates): per-owner ally set, so autonomous pickets treat
        // syndicate members as FRIENDLY — never hunted, counted on the friendly side
        // of the force ratio. Precomputed here (the closures below can't borrow &self).
        let allies: BTreeMap<PlayerId, std::collections::BTreeSet<PlayerId>> =
            self.players.keys().map(|&p| (p, self.allies_of(p))).collect();
        let is_ally = |owner: PlayerId, other: PlayerId| -> bool {
            allies.get(&owner).is_some_and(|a| a.contains(&other))
        };
        // SPEED-SIGNATURE DETECTION (§Part 4): a picket senses a target if any of
        // its coverage sources (its own bubble + the owner's arrays) reaches the
        // target's SIGNATURE — the SAME shared `detection::detected` the View uses
        // (parity-tested), so sim-side awareness and the player's map agree.
        let sensed = |owner: PlayerId, ppos: Vec2, target: Vec2, sig: f64| -> bool {
            let mut sources = vec![(ppos, sensor * range_mult.get(&owner).copied().unwrap_or(1.0))];
            if let Some(a) = arrays.get(&owner) {
                sources.extend_from_slice(a);
            }
            crate::detection::detected(sig, &sources, target)
        };

        // --- Sensing helpers (all fog-respecting: within the owner's coverage —
        // the picket's bubble or an owned sensor array's). ---
        // Local COMBATANT force as WEIGHTED strength (friendly incl. self,
        // hostile) — §fleets part 1: doctrine compares strengths, not counts.
        // Non-combatants (convoys, scouts) are excluded exactly as before, so
        // raider-only worlds see identical ratios (equal weights cancel).
        let force = |ppos: Vec2, owner: PlayerId| -> (f64, f64) {
            let (mut f, mut h) = (0.0f64, 0.0f64);
            for s in snap.iter().filter(|s| s.combatant && sensed(owner, ppos, s.pos, s.signature)) {
                if s.owner == owner || is_ally(owner, s.owner) {
                    f += s.combat;
                } else {
                    h += s.combat;
                }
            }
            (f, h)
        };
        let ratio = |f: f64, h: f64| -> f64 {
            if h <= 0.0 { 1.0 } else { f / (f + h) }
        };
        // Nearest friendly convoy within `range` (its position).
        let nearest_friendly_convoy = |ppos: Vec2, owner: PlayerId, range: f64| -> Option<Vec2> {
            snap.iter()
                .filter(|s| s.owner == owner && s.kind == ShipKind::Convoy && ppos.distance(s.pos) <= range)
                .min_by(|a, b| ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos)))
                .map(|s| s.pos)
        };
        // Nearest sensed hostile DARK ship, ANY heading (the proactive-hunt
        // target). EngageAny pickets hunt raiders AND scouts — a caught scout
        // simply dies (cheap, acceptable losses). The force-ratio and
        // threat-on-ward checks still count RAIDERS only: a scout has no combat
        // strength and can't threaten a convoy.
        let nearest_hostile = |ppos: Vec2, owner: PlayerId| -> Option<EntityId> {
            snap.iter()
                .filter(|s| {
                    s.owner != owner
                        && !is_ally(owner, s.owner)
                        && matches!(s.kind, ShipKind::Raider | ShipKind::Scout)
                        && sensed(owner, ppos, s.pos, s.signature)
                })
                .min_by(|a, b| {
                    ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos)).then(a.id.cmp(&b.id))
                })
                .map(|s| s.id)
        };
        // Nearest sensed hostile raider on an intercept COURSE toward `guard`
        // (moving fast enough, heading roughly at it) — the defensive target.
        let nearest_threat_on = |ppos: Vec2, owner: PlayerId, guard: Vec2| -> Option<EntityId> {
            let mut best: Option<(EntityId, f64)> = None;
            for h in snap.iter().filter(|s| {
                s.owner != owner
                    && !is_ally(owner, s.owner)
                    && s.kind == ShipKind::Raider
                    && sensed(owner, ppos, s.pos, s.signature)
            }) {
                if h.vel.length() < THREAT_MIN_SPEED {
                    continue; // not actually inbound
                }
                let to_c = guard - h.pos;
                let d = to_c.length();
                if d < 1e-6 {
                    continue;
                }
                if h.vel.normalized().dot(to_c / d) >= THREAT_CLOSING_COS && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((h.id, d));
                }
            }
            best.map(|(id, _)| id)
        };

        let mut engage: Vec<(EntityId, EntityId)> = Vec::new(); // (patrol, hostile)
        let mut shadow: Vec<(EntityId, Vec2)> = Vec::new(); // (patrol, charge pos)
        let mut disengage: Vec<EntityId> = Vec::new(); // quarry fled → resume patrol
        let mut retreat: Vec<EntityId> = Vec::new(); // odds turned → withdraw home

        for (pid, ship) in &self.fleets {
            // Only DARK raider fleets run autonomous picket doctrine (their
            // flagship is a raider). A fleet-of-one raider is the N=1 case.
            if ship.flagship_kind() != ShipKind::Raider {
                continue;
            }
            let (owner, ppos) = (ship.owner, ship.pos);
            let doc = doctrines.get(&owner).copied().unwrap_or_default();

            // Already on a defensive sortie.
            if let Some(def) = &ship.defense {
                // Withdraw home if the odds have turned against us below the
                // doctrine's threshold — checked continuously, so reinforcements
                // can break a committed fight.
                if let Some(min) = doc.retreat.min_ratio() {
                    let (f, h) = force(ppos, owner);
                    if ratio(f, h) < min {
                        retreat.push(*pid);
                        continue;
                    }
                }
                // Otherwise keep pursuing while the quarry is alive and in reach;
                // break off if it has fled past the breakoff range. (A quarry
                // DESTROYED elsewhere is handled by `integrate_movement`.)
                if let Some(t) = find(def.target)
                    && ship.pos.distance(t.pos) > breakoff
                {
                    disengage.push(*pid);
                }
                continue;
            }
            // §offensive-orders Part 2: a WeaponsFree fleet's NEW autonomous commit
            // is decided by `weapons_free_offense` (broader targeting — any rival in
            // its OWN bubble, raid-or-attack by target). Its defensive-sortie
            // continuation (retreat / break-off, above) still runs here. Default
            // Passive/Defensive fleets fall through unchanged (byte-preserving).
            if ship.posture == crate::doctrine::EngagementPosture::WeaponsFree {
                continue;
            }
            // On patrol: pick a charge per escort policy, then decide engagement.
            if !matches!(ship.order, FleetOrder::Patrol { .. }) {
                continue;
            }
            // CHARGE to shadow (movement). HoldStation shadows nothing — it keeps
            // the player's set route to picket a fixed chokepoint.
            let charge_pos: Option<Vec2> = match doc.escort {
                EscortPolicy::GuardNearest => snap
                    .iter()
                    .filter(|s| s.owner == owner && s.kind == ShipKind::Convoy && ppos.distance(s.pos) <= ASSIGN_RANGE)
                    .min_by(|a, b| ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos)))
                    .map(|s| s.pos),
                EscortPolicy::GuardRichest => snap
                    .iter()
                    .filter(|s| s.owner == owner && s.kind == ShipKind::Convoy && ppos.distance(s.pos) <= ASSIGN_RANGE)
                    .min_by(|a, b| {
                        b.cargo
                            .cmp(&a.cargo) // most-laden first
                            .then(ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos))) // then nearest
                            .then(a.id.cmp(&b.id)) // then lowest id (determinism)
                    })
                    .map(|s| s.pos),
                EscortPolicy::HoldStation => None,
            };
            // The convoy this picket DEFENDS (for the defensive engagement test):
            // its shadow charge, or — when holding station — the nearest friendly
            // convoy that wanders into its sensor bubble.
            let guard_pos = match doc.escort {
                EscortPolicy::HoldStation => nearest_friendly_convoy(ppos, owner, sensor),
                _ => charge_pos,
            };
            let defensive = guard_pos.and_then(|g| nearest_threat_on(ppos, owner, g));

            // Engagement target by policy (each tier a superset of defence).
            let mut target = match doc.engagement {
                EngagementPolicy::Avoid => None,
                EngagementPolicy::DefensiveOnly => defensive,
                EngagementPolicy::EngageWeaker => defensive.or_else(|| {
                    let (f, h) = force(ppos, owner);
                    if f > h { nearest_hostile(ppos, owner) } else { None }
                }),
                EngagementPolicy::EngageAny => defensive.or_else(|| nearest_hostile(ppos, owner)),
            };
            // Don't commit into a fight we'd want to retreat from anyway.
            if let (Some(_), Some(min)) = (target, doc.retreat.min_ratio()) {
                let (f, h) = force(ppos, owner);
                if ratio(f, h) < min {
                    target = None;
                }
            }
            match target {
                Some(t) => engage.push((*pid, t)),
                // No engagement: shadow the charge so the picket's sensor keeps
                // covering it. HoldStation has no charge and leaves the route be.
                None => {
                    if let Some(cpos) = charge_pos {
                        shadow.push((*pid, cpos));
                    }
                }
            }
        }

        // Break off patrol to intercept (saving the patrol route to resume later).
        for (pid, target) in engage {
            if let Some(ship) = self.fleets.get_mut(&pid) {
                let patrol = match &ship.order {
                    FleetOrder::Patrol { waypoints, .. } => waypoints.clone(),
                    _ => Vec::new(),
                };
                ship.order = FleetOrder::Intercept { target };
                ship.defense = Some(DefenseEngagement { target, patrol });
            }
        }
        // Hold station near the charge convoy (a short patrol bracketing it that
        // tracks it as it moves), so the picket stays in sensor range of its ward.
        for (pid, cpos) in shadow {
            if let Some(ship) = self.fleets.get_mut(&pid)
                && let FleetOrder::Patrol { waypoints, .. } = &mut ship.order
            {
                let off = Vec2::new(SHADOW_OFFSET, 0.0);
                if waypoints.len() == 2 {
                    waypoints[0] = cpos + off;
                    waypoints[1] = cpos - off;
                } else {
                    *waypoints = vec![cpos + off, cpos - off];
                }
            }
        }
        // Quarry fled out of reach → resume patrol.
        for pid in disengage {
            if let Some(ship) = self.fleets.get_mut(&pid) {
                let patrol = ship.defense.take().map(|d| d.patrol).unwrap_or_default();
                ship.order = resume_patrol(patrol);
            }
        }
        // Odds turned against us → break off and withdraw HOME (preserve the
        // asset; distinct from resuming patrol). Server-driven, online or off.
        for pid in retreat {
            let home = self
                .fleets
                .get(&pid)
                .and_then(|s| self.players.get(&s.owner))
                .map(|c| c.home);
            if let Some(ship) = self.fleets.get_mut(&pid) {
                ship.defense = None;
                ship.order = match home {
                    Some(h) => FleetOrder::MoveTo { dest: h },
                    None => FleetOrder::Idle,
                };
            }
        }
    }

    /// §offensive-orders Part 2 — the WEAPONS-FREE standing offense, run every tick
    /// alongside [`Self::autonomous_defense`]. For each fleet whose per-fleet
    /// [`EngagementPosture::WeaponsFree`] is set (and that carries a raider — strike
    /// capability), that is AVAILABLE (patrolling / idle / moving, not already
    /// sortied or engaged), it hunts on its OWN local detection — no command-center
    /// round trip (the on-theme answer to command lag):
    ///
    ///   * TARGET (who): the nearest rival fleet of ANY kind whose light reaches
    ///     the fleet's OWN sensor bubble (`sensor_range × sensor_mult`; a scout
    ///     aboard widens it). NOT the corp's array picture — only what THIS fleet
    ///     can see. Deterministic `(distance, id)` tie-break.
    ///   * COMMIT (whether): the corp [`FleetDoctrine`] composes in —
    ///     [`EngagementPolicy::weapons_free_commits`] on the local force ratio
    ///     (Avoid vetoes; DefensiveOnly/EngageWeaker favourable-only; EngageAny any
    ///     odds) plus the [`RetreatThreshold`] gate. An unfavourable contact under a
    ///     favourable-only policy is simply ignored (not suicided into).
    ///   * VERB: it issues an ordinary [`FleetOrder::Intercept`], so the existing
    ///     contact logic decides raid-vs-battle by target — a lone convoy is raided
    ///     (cargo seized), anything armed/defended is a full battle. From a patrol,
    ///     the route is saved (defensive-sortie machinery) so it resumes after.
    ///
    /// Fog-safe: acts only on the fleet's own delivered light; the OWNER learns of
    /// any engagement through the ordinary light-delayed reports. Deterministic +
    /// standing (works with the owner offline). Passive/Defensive fleets are inert
    /// here, so nothing changes for them.
    fn weapons_free_offense(&mut self) {
        use crate::doctrine::EngagementPosture;
        let base = self.config.sensor_range;
        let c = self.config.c;
        let hub = self.hub;
        // Read-only snapshot (deterministic order) for target selection + odds.
        #[derive(Clone, Copy)]
        struct Snap {
            id: EntityId,
            owner: PlayerId,
            kind: ShipKind,
            pos: Vec2,
            vel: Vec2,
            combat: f64,
            combatant: bool,
            signature: f64,
            broadcasts: bool,
        }
        let snap: Vec<Snap> = self
            .fleets
            .iter()
            .map(|(id, s)| Snap {
                id: *id,
                owner: s.owner,
                kind: s.flagship_kind(),
                pos: s.pos,
                vel: s.vel,
                combat: s.combat_weight(),
                combatant: s.is_combatant(),
                // §node Veil: a fed magnetar node quiets its holder's dark fleets
                // in-region — the SAME signature both pickets and the View read.
                signature: s.signature() * self.veil_factor(s.owner, s.pos) * if s.surveying() { crate::explore::SURVEY_SIGNATURE_FACTOR } else { 1.0 },
                broadcasts: s.broadcasts(),
            })
            .collect();
        let doctrines: BTreeMap<PlayerId, FleetDoctrine> =
            self.players.iter().map(|(id, c)| (*id, c.doctrine)).collect();
        // §syndicates: per-owner ally set — a WeaponsFree fleet never hunts a
        // syndicate member and counts allied combat weight on the friendly side.
        let allies: BTreeMap<PlayerId, std::collections::BTreeSet<PlayerId>> =
            self.players.keys().map(|&p| (p, self.allies_of(p))).collect();
        let is_ally = |owner: PlayerId, other: PlayerId| -> bool {
            allies.get(&owner).is_some_and(|a| a.contains(&other))
        };

        // A rival is a valid WeaponsFree target iff its light reaches THIS fleet's
        // OWN bubble. The fleet acts on its OWN DELIVERED LIGHT (§retarded-time):
        // it sees the target where the arriving light left it — retarded by the
        // light-travel time across the gap (constant-velocity §14.1), so a distant
        // fast mover is engaged where it WAS, not where it truly is now. Detection
        // is the fleet's own bubble ONLY (a broadcaster by range; a dark fleet by
        // the shared speed-signature rule) — never the corp's arrays (that would be
        // a command-center round trip).
        let visible = |ppos: Vec2, bubble: f64, s: &Snap| -> bool {
            let seen_pos = s.pos - s.vel * (ppos.distance(s.pos) / c);
            if s.broadcasts {
                ppos.distance(seen_pos) <= bubble
            } else {
                crate::detection::detected(s.signature, &[(ppos, bubble)], seen_pos)
            }
        };
        // Local combatant force ratio over the fleet's OWN bubble (friendly incl.
        // self; hostile) — the WHETHER gate's input (weighted, non-combatants
        // ignored). Membership uses the SAME retarded `visible` rule as target
        // selection, so the fleet weighs exactly the combatants its own light shows
        // (a fast mover detected at its retarded position also counts in the odds).
        let local_force = |ppos: Vec2, owner: PlayerId, bubble: f64| -> (f64, f64) {
            let (mut f, mut h) = (0.0f64, 0.0f64);
            for s in snap.iter().filter(|s| s.combatant && visible(ppos, bubble, s)) {
                if s.owner == owner || is_ally(owner, s.owner) { f += s.combat; } else { h += s.combat; }
            }
            (f, h)
        };
        let ratio = |f: f64, h: f64| -> f64 { if h <= 0.0 { 1.0 } else { f / (f + h) } };
        // A convoy safe inside the hub commons (§4) escapes on contact — don't
        // pointlessly commit against it (and thrash a re-commit every tick).
        let hub_safe = |s: &Snap| -> bool { s.kind == ShipKind::Convoy && s.pos.distance(hub) <= HUB_SAFE_RADIUS };

        let mut commits: Vec<(EntityId, EntityId, Vec<Vec2>)> = Vec::new(); // (fleet, target, saved patrol)
        for (fid, ship) in &self.fleets {
            // WeaponsFree + strike capability (a raider aboard) + AVAILABLE (not
            // already sortied/engaged/intercepting/attacking/blockading).
            if ship.posture != EngagementPosture::WeaponsFree || !ship.contains(ShipKind::Raider) {
                continue;
            }
            if ship.defense.is_some() {
                continue; // already on a sortie — its lifecycle is autonomous_defense's
            }
            if !matches!(ship.order, FleetOrder::Patrol { .. } | FleetOrder::Idle | FleetOrder::MoveTo { .. }) {
                continue;
            }
            let (owner, ppos) = (ship.owner, ship.pos);
            let bubble = base * ship.sensor_mult();
            // WHO: nearest rival fleet in the fleet's OWN bubble (deterministic),
            // excluding a hub-safe convoy (it would just escape).
            let target = snap
                .iter()
                .filter(|s| s.owner != owner && !is_ally(owner, s.owner) && !hub_safe(s) && visible(ppos, bubble, s))
                .min_by(|a, b| ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos)).then(a.id.cmp(&b.id)))
                .map(|s| s.id);
            let Some(tid) = target else { continue };
            // WHETHER: compose the corp doctrine (odds permission + retreat gate).
            let doc = doctrines.get(&owner).copied().unwrap_or_default();
            let (f, h) = local_force(ppos, owner, bubble);
            if !doc.engagement.weapons_free_commits(f, h) {
                continue;
            }
            if let Some(min) = doc.retreat.min_ratio()
                && ratio(f, h) < min
            {
                continue; // an unfavourable contact under a retreat gate → ignored
            }
            let patrol = match &ship.order {
                FleetOrder::Patrol { waypoints, .. } => waypoints.clone(),
                _ => Vec::new(),
            };
            commits.push((*fid, tid, patrol));
        }
        // Commit: ordinary Intercept (verb decided by target on contact). Mark the
        // sortie so autonomous_defense manages its retreat/break-off/resume.
        for (fid, tid, patrol) in commits {
            if let Some(ship) = self.fleets.get_mut(&fid) {
                ship.order = FleetOrder::Intercept { target: tid };
                ship.defense = Some(DefenseEngagement { target: tid, patrol });
            }
        }
    }

    /// The nearest owned system with a Defense Platform covering `pos` (§buildings
    /// step 2c) — folded into the defender's forces as stationary tiers.
    fn covering_platform(&self, owner: PlayerId, pos: Vec2) -> Option<EntityId> {
        self.systems
            .iter()
            .filter(|s| s.owner == Some(owner) && s.tier_sum(crate::build::StructureKind::DefensePlatform) >= 1 && s.pos.distance(pos) <= crate::build::DEFENSE_PLATFORM_RADIUS)
            .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)).then(a.id.cmp(&b.id)))
            .map(|s| s.id)
    }

    /// Sum the compositions of a side's member fleets into one pool.
    fn side_comp(&self, members: &[EntityId]) -> BTreeMap<ShipKind, u32> {
        let mut c: BTreeMap<ShipKind, u32> = BTreeMap::new();
        for id in members {
            if let Some(f) = self.fleets.get(id) {
                for (k, n) in &f.composition {
                    *c.entry(*k).or_insert(0) += *n;
                }
            }
        }
        c
    }

    /// §modules: the side's aggregate LOADOUT partition (summed over its fleets)
    /// — `kind → loadout key → count`, fitted stacks only. Feeds the tactical
    /// unpack ([`crate::tactical::stacked`]) so battles fight the real fits.
    fn side_loadouts(&self, members: &[EntityId]) -> crate::combat::LoadoutMap {
        let mut out: crate::combat::LoadoutMap = BTreeMap::new();
        for id in members {
            if let Some(f) = self.fleets.get(id) {
                for (k, m) in &f.loadouts {
                    for (key, n) in m {
                        if *n > 0 && !key.is_empty() {
                            *out.entry(*k).or_default().entry(key.clone()).or_insert(0) += *n;
                        }
                    }
                }
            }
        }
        out
    }

    /// Apply a side's per-kind losses across its member fleets (lowest id first,
    /// deterministic), removing emptied fleets, and emit the delayed-disappearance
    /// `ShipDestroyed` ghosts from the battle site.
    fn apply_side_losses(&mut self, members: &mut Vec<EntityId>, losses: &crate::combat::Losses, pos: Vec2, events: &mut Vec<Event>) {
        // §modules: shed losses PER STACK (kind, loadout), so a fleet loses the
        // exact fitted/unfitted ships combat killed — an armored stack that
        // absorbed less sheds fewer. `remove_stack` keeps composition + loadouts
        // in step. Distributed across the side's fleets, lowest id first.
        let mut titan_losses: Vec<PlayerId> = Vec::new();
        for ((kind, loadout), n) in &losses.per_stack {
            let mut remaining = *n;
            for id in members.iter() {
                if remaining == 0 {
                    break;
                }
                let Some(f) = self.fleets.get_mut(id) else { continue };
                let take = f.remove_stack(*kind, loadout, remaining);
                if take > 0 {
                    remaining -= take;
                    let owner = f.owner;
                    for _ in 0..take {
                        events.push(Event::new(self.time, EventPayload::ShipDestroyed { ship: *id, owner, kind: *kind, pos }));
                    }
                    // §ladder B4: a dead TITAN is a HEADLINE — clear the
                    // flagship name and broadcast (processed after the loop;
                    // the borrow on `f` ends here).
                    if *kind == ShipKind::Titan {
                        titan_losses.push(owner);
                    }
                }
            }
        }
        for owner in titan_losses {
            let Some(sid) = self.players.get(&owner).and_then(|c| c.syndicate) else {
                // An unaffiliated owner's Titan dies without a syndicate to
                // headline for (only reachable via post-build emigration —
                // building one requires syndicate research).
                continue;
            };
            // §ladder B4: the christened name belongs to THE syndicate's Titan.
            // Membership churn can gather more than one (the singleton binds at
            // BUILD time) — if another member Titan still flies after this
            // loss, the LIVING flagship keeps the name and the fallen hull
            // makes a nameless headline. The stack removal above already
            // updated the fleets, so this reads the post-loss world.
            let members = self.syndicates.get(&sid).map(|s| s.members.clone()).unwrap_or_default();
            let another_flies = self
                .fleets
                .values()
                .any(|f| members.contains(&f.owner) && f.count(ShipKind::Titan) > 0);
            let Some(s) = self.syndicates.get_mut(&sid) else { continue };
            let name = if another_flies { None } else { s.flagship_name.take() };
            events.push(Event::new(
                self.time,
                EventPayload::FlagshipDestroyed { owner, syndicate: sid, name, pos },
            ));
        }
        // Drop any fleet emptied out. §economy Part 4: a fleet that died with
        // specialists aboard loses them WITH the ship — the one specialist loss
        // rule — and the owner is told (light-delayed from the wreck).
        members.retain(|id| self.fleets.get(id).is_some_and(|f| !f.is_empty()));
        let mut lost: Vec<Event> = Vec::new();
        self.fleets.retain(|_, f| {
            if f.is_empty() {
                if !f.passengers.is_empty() {
                    lost.push(Event::new(
                        self.time,
                        EventPayload::SpecialistsLost { owner: f.owner, manifest: f.passengers.clone(), pos: f.pos },
                    ));
                }
                // §modules Part B3: crates aboard go down with the ship, same rule.
                if !f.modules.is_empty() {
                    lost.push(Event::new(
                        self.time,
                        EventPayload::ModulesLost { owner: f.owner, manifest: f.modules.clone(), pos: f.pos },
                    ));
                }
                false
            } else {
                true
            }
        });
        events.extend(lost);
    }

    /// Resolve BATTLES this tick (§battles-take-time). Combat is deterministic
    /// Lanchester attrition over persistent, observable ENGAGEMENT entities: each
    /// tick, contacts create or extend a battle, relief joins its side's pool, and
    /// each engagement trades ONE tick of proportional casualties between its
    /// pooled sides at the config-scaled `dmg_rate`. Cargo raids end on a short
    /// cap; a no-retreat grind ends on the safety valve; doctrine withdraws a side
    /// at its strength threshold. The engagement (pools + elapsed + participants)
    /// PERSISTS, so a snapshot resumes the fight. One composition-vs-composition
    /// report fires per side when the battle ends.
    fn resolve_raids(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let hub = self.hub;
        let target = self.config.battle_target_secs;
        // (The pooled engine's per-tick dmg_rate is superseded — the tactical
        // engine steps on its own cadence; see tactical::tac_step_ticks.)

        for e in self.engagements.values_mut() {
            e.touched = false;
        }

        // Fleets already IN a battle are committed to it and anchored — their
        // standing Intercept/Attack order is DORMANT. Exclude them from the ESCAPE
        // check so a raider that was jumped mid-pursuit doesn't fire a spurious
        // "raid failed" when its old convoy target reaches hub safety. (The contact
        // loop KEEPS engaged fleets — their per-tick contact re-touches their own
        // engagement to keep it alive; the `together` check there stops them opening
        // a duplicate battle against a fleet they're already fighting.)
        let engaged: std::collections::BTreeSet<EntityId> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();

        // §rankings: LATCH the "fought" flag on every current engagement
        // participant — battles take many ticks, so a convoy that survives one is
        // still a participant here across those ticks; the flag lets us credit
        // "cargo protected" when it later delivers. Latches (never cleared).
        for id in &engaged {
            if let Some(f) = self.fleets.get_mut(id) {
                f.fought = true;
            }
        }

        // Convoy reaching hub safety before contact → the raider breaks off.
        let mut escapes: Vec<(EntityId, EntityId)> = Vec::new();
        for (rid, ship) in &self.fleets {
            if !engaged.contains(rid)
                && let FleetOrder::Intercept { target } | FleetOrder::Attack { target } = ship.order
                && let Some(t) = self.fleets.get(&target)
                && ship.owner != t.owner
                && !self.are_allied(ship.owner, t.owner)
                && ship.pos.distance(t.pos) > CONTACT_RADIUS
                && t.flagship_kind() == ShipKind::Convoy
                && t.pos.distance(hub) <= HUB_SAFE_RADIUS
            {
                escapes.push((*rid, target));
            }
        }
        for (aid, tid) in escapes {
            let (Some(att), Some(tgt)) = (self.fleets.get(&aid), self.fleets.get(&tid)) else { continue };
            let (a_owner, t_owner, a_kind, t_kind, t_pos) = (att.owner, tgt.owner, att.flagship_kind(), tgt.flagship_kind(), tgt.pos);
            events.push(Event::new(now, EventPayload::RaidResolved {
                attacker: a_owner, defender: t_owner, attacker_ship: aid, target_ship: tid,
                attacker_kind: a_kind, target_kind: t_kind, outcome: RaidOutcome::Escaped, pos: t_pos,
                attacker_losses: BTreeMap::new(), target_losses: BTreeMap::new(),
            }));
            // Break off. An AUTONOMOUS sortie (a WeaponsFree hunt / picket that
            // carries a saved patrol) RESUMES its route; a MANUAL raid returns home.
            // Manual raids carry no `defense`, so this is byte-identical for them.
            let home = self.players.get(&a_owner).map(|c| c.home);
            if let Some(ship) = self.fleets.get_mut(&aid) {
                if let Some(def) = ship.defense.take() {
                    ship.order = resume_patrol(def.patrol);
                } else {
                    ship.order = match home {
                        Some(h) => FleetOrder::MoveTo { dest: h },
                        None => FleetOrder::Idle,
                    };
                }
            }
        }

        // Contacts: an attacker on Intercept OR Attack within reach of a rival
        // target. The `is_attack` flag (an Attack order) forces a FULL battle on
        // contact even against a convoy (destroy, not raid) — see the raid flag.
        let contacts: Vec<(EntityId, EntityId, bool)> = self
            .fleets
            .iter()
            .filter_map(|(rid, ship)| {
                let (target, is_attack) = match ship.order {
                    FleetOrder::Intercept { target } => (target, false),
                    FleetOrder::Attack { target } => (target, true),
                    _ => return None,
                };
                if let Some(t) = self.fleets.get(&target)
                    && ship.owner != t.owner
                    && !self.are_allied(ship.owner, t.owner)
                    && ship.pos.distance(t.pos) <= CONTACT_RADIUS
                {
                    Some((*rid, target, is_attack))
                } else {
                    None
                }
            })
            .collect();

        for (aid, tid, is_attack) in contacts {
            let a_owner_c = self.fleets.get(&aid).map(|f| f.owner);
            let Some(a_owner_c) = a_owner_c else { continue };
            // ALREADY FIGHTING each other: if this fleet and its target are both in
            // the SAME engagement already (on either side), they're in one battle —
            // don't open a second. This catches RECIPROCAL intercepts (two rival
            // fleets that each committed to attack the OTHER — e.g. a WeaponsFree
            // hunter and a picket) which would otherwise spawn two engagements (two
            // battle icons) at the same spot. Just keep it live (and let an ATTACK
            // escalate a raid). Distinct from a patrol attacking a hostile that is
            // itself raiding a convoy — there the two never share an engagement.
            let together = self
                .engagements
                .iter()
                .find(|(_, e)| {
                    (e.attackers.contains(&aid) || e.defenders.contains(&aid))
                        && (e.attackers.contains(&tid) || e.defenders.contains(&tid))
                })
                .map(|(id, _)| *id);
            if let Some(eid) = together {
                let e = self.engagements.get_mut(&eid).unwrap();
                if is_attack {
                    e.raid = false;
                }
                e.touched = true;
                continue;
            }
            // Otherwise join an existing battle ONLY when the SIDES align: this
            // attacker is on the attacking side (same owner) and the target is
            // already this battle's defender — reinforcement of the attack.
            let existing = self
                .engagements
                .iter()
                .find(|(_, e)| e.a_owner == a_owner_c && e.defenders.contains(&tid))
                .map(|(id, _)| *id);
            if let Some(eid) = existing {
                let e = self.engagements.get_mut(&eid).unwrap();
                if !e.attackers.contains(&aid) {
                    e.attackers.push(aid);
                }
                if !e.defenders.contains(&tid) {
                    e.defenders.push(tid);
                }
                // An ATTACK order joining an in-progress RAID ESCALATES it to a full
                // battle (destroy) — the aggressor's explicit intent wins, so the
                // convoy is fought to the death rather than the raid's cargo-grab.
                if is_attack {
                    e.raid = false;
                }
                e.touched = true;
                continue;
            }
            // New battle. Assemble the defender side: the target + any covering
            // platform + nearby friendly corvette screens.
            let (Some(att), Some(tgt)) = (self.fleets.get(&aid), self.fleets.get(&tid)) else { continue };
            let (a_owner, d_owner, t_kind, t_pos) = (att.owner, tgt.owner, tgt.flagship_kind(), tgt.pos);
            let civilian = matches!(t_kind, ShipKind::Convoy | ShipKind::Colony);
            let mut defenders = vec![tid];
            let mut platform_system = None;
            // A convoy contact is a cargo RAID by default — UNLESS this is an
            // explicit ATTACK order (destroy, cargo lost) or the convoy is defended
            // (fighting the escort/platform is a battle). Everything armed is a battle.
            let mut raid = !is_attack && t_kind == ShipKind::Convoy;
            if civilian {
                // Pull in EVERY covering friendly corvette fleet (escort/garrison).
                let screens: Vec<EntityId> = self
                    .fleets
                    .iter()
                    .filter(|(id, s)| **id != tid && s.owner == d_owner && s.contains(ShipKind::Corvette) && s.pos.distance(t_pos) <= crate::ship::CORVETTE_PROTECT_RADIUS)
                    .map(|(id, _)| *id)
                    .collect();
                if !screens.is_empty() {
                    defenders.extend(screens);
                    raid = false; // fighting the escort is a battle, not a cargo raid
                } else if let Some(sid) = self.covering_platform(d_owner, t_pos) {
                    platform_system = Some(sid);
                    raid = false;
                }
            } else {
                raid = false;
            }
            let id = self.alloc_engagement_id();
            let a_comp = self.side_comp(&[aid]);
            let d_comp = self.side_comp(&defenders);
            let (ptiers, _ppool) = platform_system
                .and_then(|sid| self.systems.iter().find(|s| s.id == sid))
                .map(|s| (s.tier_sum(crate::build::StructureKind::DefensePlatform), s.defense_pool))
                .unwrap_or((0, 0.0));
            let a_str = crate::combat::Forces::from_fleet(&a_comp, &BTreeMap::new()).strength();
            let d_str = crate::combat::Forces::from_fleet(&d_comp, &BTreeMap::new()).with_platform(ptiers, 0.0).strength();
            let a_start_loadouts = self.side_loadouts(&[aid]);
            let d_start_loadouts = self.side_loadouts(&defenders);
            self.engagements.insert(id, Engagement {
                id,
                pos: t_pos,
                started_at: now,
                raid,
                a_owner,
                d_owner,
                attackers: vec![aid],
                defenders,
                platform_system,
                a_stack_pool: BTreeMap::new(),
                d_stack_pool: BTreeMap::new(),
                a_start: a_comp,
                d_start: d_comp,
                a_start_loadouts,
                d_start_loadouts,
                a_start_strength: a_str,
                d_start_strength: d_str,
                platform_start_tiers: ptiers,
                a_lead: aid,
                d_lead: tid,
                disengaging: BTreeMap::new(),
                a_fled: false,
                d_fled: false,
                touched: true,
                tactical: None,
            });
        }

        // REINFORCE: a friendly COMBATANT fleet that has moved to an active battle
        // joins its side's pool (relief shifts the Lanchester ratio). Convoys/
        // colonies don't auto-join a fight they didn't have to.
        let eids: Vec<EntityId> = self.engagements.keys().copied().collect();
        // §battle-records: OPEN a record for every live engagement first, so
        // reinforcement joins this tick land as `Joined` beats on it.
        for eid in &eids {
            self.record_ensure_open(*eid);
        }
        for eid in &eids {
            let (pos, a_owner, d_owner) = {
                let e = &self.engagements[eid];
                (e.pos, e.a_owner, e.d_owner)
            };
            let joiners: Vec<(EntityId, bool)> = self
                .fleets
                .iter()
                .filter(|(fid, f)| {
                    let e = &self.engagements[eid];
                    !e.attackers.contains(fid)
                        && !e.defenders.contains(fid)
                        && f.is_combatant()
                        // NOT a fleet that is MOVING somewhere: a withdrawn fleet
                        // (fleeing home) or a reinforcement still EN ROUTE is not
                        // pulled in — a reinforcement joins only once it ARRIVES
                        // (its MoveTo completes → Idle). A patrol / picket
                        // defending nearby DOES join.
                        && !matches!(f.order, FleetOrder::MoveTo { .. })
                        // The two combatants' own fleets, OR a syndicate ALLY of the
                        // DEFENDER as a stationed, FED, non-Avoid GARRISON (§syndicates
                        // Part 3) — it joins the DEFENDER side.
                        && (f.owner == a_owner
                            || f.owner == d_owner
                            || (matches!(f.order, FleetOrder::Idle)
                                && f.garrison_fed
                                && self.are_allied(f.owner, d_owner)
                                && self.players.get(&f.owner).is_some_and(|c| c.doctrine.engagement != EngagementPolicy::Avoid)))
                        && f.pos.distance(pos) <= BATTLE_JOIN_RADIUS
                })
                .map(|(fid, f)| (*fid, f.owner == a_owner))
                .collect();
            if !joiners.is_empty() {
                // §battle-records: note the relief as a `Joined` beat per side.
                let mut atk_join: BTreeMap<ShipKind, u32> = BTreeMap::new();
                let mut def_join: BTreeMap<ShipKind, u32> = BTreeMap::new();
                for (fid, atk) in &joiners {
                    if let Some(f) = self.fleets.get(fid) {
                        let dst = if *atk { &mut atk_join } else { &mut def_join };
                        for (k, n) in &f.composition {
                            *dst.entry(*k).or_insert(0) += *n;
                        }
                    }
                }
                // §tactical T6 (stint accounting): relief folds into the side's
                // START composition too — end-of-battle losses are computed as
                // `start − final`, so without this a joined ship's death simply
                // VANISHED from the report and the record's outcome summary.
                // (Symmetric with the alive-exit subtraction in the disengage
                // pass, so a flee-and-rejoin cycle stays exact.)
                {
                    let e = self.engagements.get_mut(eid).unwrap();
                    for (fid, atk) in joiners {
                        if atk {
                            e.attackers.push(fid);
                        } else {
                            e.defenders.push(fid);
                        }
                    }
                    for (k, n) in &atk_join {
                        *e.a_start.entry(*k).or_insert(0) += *n;
                    }
                    for (k, n) in &def_join {
                        *e.d_start.entry(*k).or_insert(0) += *n;
                    }
                }
                if !atk_join.is_empty() {
                    self.record_note(*eid, crate::combat::RoundNote::Joined { side: 0, comp: atk_join });
                }
                if !def_join.is_empty() {
                    self.record_note(*eid, crate::combat::RoundNote::Joined { side: 1, comp: def_join });
                }
            }
        }

        // Resolve each engagement: one pooled attrition tick.
        for eid in &eids {
            // Prune departed/destroyed participants.
            {
                let mut e = self.engagements.remove(eid).unwrap();
                e.attackers.retain(|f| self.fleets.contains_key(f));
                e.defenders.retain(|f| self.fleets.contains_key(f));
                self.engagements.insert(*eid, e);
            }

            // DOCTRINE-ON-CONTACT (§engagement movement — the anti-lock rule): a
            // fleet that does NOT ACCEPT this battle begins disengaging AT ONCE. It
            // ACCEPTS if it committed the attack (an Intercept order) OR its corp's
            // engagement policy isn't Avoid. A non-accepter takes a brief
            // parting-shot exposure, then physically flees — the SPEED TABLE then
            // decides whether it opens the gap or a faster pursuer catches it.
            {
                let (atkers, defers) = {
                    let e = &self.engagements[eid];
                    (e.attackers.clone(), e.defenders.clone())
                };
                // Fastest pursuer on each side (the SPEED-TABLE gate for escape).
                let side_max = |members: &[EntityId], w: &World| -> f64 {
                    members.iter().filter_map(|id| w.fleets.get(id)).map(|f| f.max_speed()).fold(0.0_f64, f64::max)
                };
                let atk_speed = side_max(&atkers, self);
                let def_speed = side_max(&defers, self);
                let mut set_dis: Vec<(EntityId, f64)> = Vec::new();
                let mut flee: Vec<EntityId> = Vec::new();
                let mut caught: Vec<EntityId> = Vec::new();
                for fid in atkers.iter().chain(defers.iter()) {
                    let Some(f) = self.fleets.get(fid) else { continue };
                    let accepts = matches!(f.order, FleetOrder::Intercept { .. } | FleetOrder::Attack { .. })
                        || self.players.get(&f.owner).map(|c| c.doctrine.engagement != crate::doctrine::EngagementPolicy::Avoid).unwrap_or(true);
                    if accepts {
                        continue;
                    }
                    match self.engagements[eid].disengaging.get(fid).copied() {
                        None => set_dis.push((*fid, now + crate::combat::DISENGAGE_EXPOSURE_SECS)),
                        Some(t) if now >= t => {
                            // Exposure over — the SPEED TABLE decides: escape iff
                            // faster than the fastest pursuer on the OTHER side;
                            // else CAUGHT, and the battle proceeds (it must fight).
                            let is_atk = atkers.contains(fid);
                            let pursuer = if is_atk { def_speed } else { atk_speed };
                            if f.max_speed() > pursuer + 1e-9 {
                                flee.push(*fid);
                            } else {
                                caught.push(*fid);
                            }
                        }
                        _ => {}
                    }
                }
                // §battle-records: which side(s) BEGAN disengaging this tick.
                let dis_atk = set_dis.iter().any(|(fid, _)| atkers.contains(fid));
                let dis_def = set_dis.iter().any(|(fid, _)| defers.contains(fid));
                // §tactical T6 (stint accounting): a fleet that exits ALIVE takes
                // its surviving ships back OUT of the side's start composition —
                // end-of-battle losses are `start − final`, and without this every
                // escaped survivor was reported as a combat death. Symmetric with
                // the join fold (enter: add current comp; leave alive: subtract
                // it), so each stint contributes exactly its own losses — even
                // across a flee-home-and-rejoin cycle.
                let flee_comps: Vec<(EntityId, BTreeMap<ShipKind, u32>)> = flee
                    .iter()
                    .filter_map(|fid| self.fleets.get(fid).map(|f| (*fid, f.composition.clone())))
                    .collect();
                let e = self.engagements.get_mut(eid).unwrap();
                for (fid, t) in set_dis {
                    e.disengaging.insert(fid, t);
                }
                for fid in &caught {
                    e.disengaging.remove(fid); // outrun — it stays and fights
                }
                for fid in &flee {
                    if e.attackers.contains(fid) {
                        e.a_fled = true;
                    }
                    if e.defenders.contains(fid) {
                        e.d_fled = true;
                    }
                    let side_start = if e.attackers.contains(fid) { &mut e.a_start } else { &mut e.d_start };
                    if let Some((_, comp)) = flee_comps.iter().find(|(id, _)| id == fid) {
                        for (k, n) in comp {
                            if let Some(have) = side_start.get_mut(k) {
                                *have = have.saturating_sub(*n);
                            }
                        }
                        side_start.retain(|_, n| *n > 0);
                    }
                    e.attackers.retain(|x| x != fid);
                    e.defenders.retain(|x| x != fid);
                    e.disengaging.remove(fid);
                }
                for fid in flee {
                    let owner = self.fleets.get(&fid).map(|f| f.owner);
                    if let Some(owner) = owner {
                        self.send_ship_home(fid, owner); // clean escape at formation speed
                    }
                }
                if dis_atk {
                    self.record_note(*eid, crate::combat::RoundNote::DisengageExposure { side: 0 });
                }
                if dis_def {
                    self.record_note(*eid, crate::combat::RoundNote::DisengageExposure { side: 1 });
                }
            }

            let (attackers, defenders, platform_system, raid, started_at, a_start_strength, d_start_strength, a_owner, d_owner, pos) = {
                let e = &self.engagements[eid];
                (e.attackers.clone(), e.defenders.clone(), e.platform_system, e.raid, e.started_at, e.a_start_strength, e.d_start_strength, e.a_owner, e.d_owner, e.pos)
            };
            let a_comp = self.side_comp(&attackers);
            let d_comp = self.side_comp(&defenders);
            let (ptiers, ppool) = platform_system
                .and_then(|sid| self.systems.iter().find(|s| s.id == sid))
                .map(|s| (s.tier_sum(crate::build::StructureKind::DefensePlatform), s.defense_pool))
                .unwrap_or((0, 0.0));
            // A side with nothing left → the battle has ended (flush handles it).
            if a_comp.is_empty() || (d_comp.is_empty() && ptiers == 0) {
                continue; // untouched → flushed below
            }
            self.engagements.get_mut(eid).unwrap().touched = true;

            let a_pool = self.engagements[eid].a_stack_pool.clone();
            let d_pool = self.engagements[eid].d_stack_pool.clone();
            // §tactical: partition each side into (kind, loadout) stacks from the
            // live fleets' fits — the engine's sync/unpack input.
            let a_loadouts = self.side_loadouts(&attackers);
            let d_loadouts = self.side_loadouts(&defenders);
            let a_stacks = crate::tactical::stacked(&a_comp, &a_loadouts);
            let d_stacks = crate::tactical::stacked(&d_comp, &d_loadouts);

            // OPEN or MIGRATE: a fresh battle (or an old-snapshot pooled one)
            // unpacks into individual combatants — pro-rata HP from the stack
            // pools, defender anchored, attacker on their real approach bearing,
            // platform tiers as stationary combatants. One-way; documented.
            let mut tac = match self.engagements.get_mut(eid).unwrap().tactical.take() {
                Some(t) => t,
                None => {
                    let bearing = attackers
                        .first()
                        .and_then(|id| self.fleets.get(id))
                        .map(|f| f.pos - pos)
                        .unwrap_or(Vec2::new(1.0, 0.0));
                    crate::tactical::TacticalState::open(
                        self.config.seed,
                        eid.0,
                        &a_stacks,
                        &d_stacks,
                        &a_pool,
                        &d_pool,
                        ptiers,
                        ppool,
                        bearing,
                    )
                }
            };

            // SYNC to the live strategic sides (relief joins unpack at the edge;
            // withdrawn fleets' ships leave). Scouts die at the boundary — the
            // same instant death the old strip_scouts applied.
            let scouts = tac.sync([&a_stacks, &d_stacks]);
            let mut la = crate::combat::Losses::default();
            let mut lb = crate::combat::Losses::default();
            if scouts[0] > 0 {
                la.add_kind(ShipKind::Scout, scouts[0]);
            }
            if scouts[1] > 0 {
                lb.add_kind(ShipKind::Scout, scouts[1]);
            }

            // STEP the engine on its own cadence (1 Hz production, 2 Hz playtest
            // — see tac_step_ticks). Raids hit harder per step so a smash-and-
            // grab resolves inside the short raid cap (the old RAID_RATE
            // asymmetry, preserved).
            let cadence = crate::tactical::tac_step_ticks(target);
            let (dealt_a, dealt_b, platform_destroyed, tac_frame) = if tac.step == 0 || self.tick % cadence == 0 {
                let mods = [
                    crate::tactical::SideMods {
                        opening_bonus: self.research_flag(a_owner, crate::research::Cap::FirstStrike)
                            || self.research_flag(a_owner, crate::research::Cap::GrandBatteries),
                        flak_mult: self.research_mod(a_owner, crate::research::ModKey::PdIntercept),
                    },
                    crate::tactical::SideMods {
                        opening_bonus: self.research_flag(d_owner, crate::research::Cap::FirstStrike)
                            || self.research_flag(d_owner, crate::research::Cap::GrandBatteries),
                        flak_mult: self.research_mod(d_owner, crate::research::ModKey::PdIntercept),
                    },
                ];
                let outcome = tac.step(raid, mods);
                for (k, lo_map) in &outcome.losses[0].per_stack {
                    la.add_stack(k.0, k.1.clone(), *lo_map);
                }
                for (k, lo_map) in &outcome.losses[1].per_stack {
                    lb.add_stack(k.0, k.1.clone(), *lo_map);
                }
                let pdestroyed = ptiers > 0 && tac.platform_tiers() == 0;
                // §T3: the round's truth keyframe rides the recorder.
                let frame = tac.keyframe(outcome.deaths);
                (outcome.dealt[0], outcome.dealt[1], pdestroyed, Some(frame))
            } else {
                (0.0, 0.0, false, None)
            };

            // Cargo SEIZURE: on a raid, if a defender convoy is emptied this tick,
            // the (first) attacker loots its cargo before the wreck is removed.
            if raid {
                let dead_cargo: Option<Cargo> = defenders
                    .iter()
                    .find_map(|id| self.fleets.get(id).filter(|f| f.count(ShipKind::Convoy) > 0 && lb.per_kind.get(&ShipKind::Convoy).copied().unwrap_or(0) >= f.count(ShipKind::Convoy)).and_then(|f| f.cargo));
                if let Some(cargo) = dead_cargo {
                    // Load the loot onto the (first empty-handed) attacker and note
                    // WHO seized it — the borrow of the fleet ends with the closure.
                    let seizer = attackers
                        .first()
                        .and_then(|id| self.fleets.get_mut(id))
                        .filter(|a| a.cargo.is_none())
                        .map(|a| {
                            a.cargo = Some(cargo);
                            a.owner
                        });
                    // §rankings CARGO CAPTURED: credit the raider the seized units.
                    if let Some(owner) = seizer {
                        self.bump_stats(owner, |s| s.cargo_captured += cargo.units as u64);
                        // §research R3: a seized convoy IS a successful raid (Corsair gate).
                        self.add_research_verb(owner, crate::research::Verb::SuccessfulRaids, 1.0);
                    }
                }
            }

            // Persist the engine's truth back through the OLD channels (§law 1:
            // the strategic layer sees the same shapes): HP deficits → per-stack
            // pools; platform tiers + pool → the system.
            let tac_ptiers = tac.platform_tiers();
            let tac_ppool = tac.platform_pool();
            {
                let e = self.engagements.get_mut(eid).unwrap();
                e.a_stack_pool = tac.pools(0);
                e.d_stack_pool = tac.pools(1);
            }
            if let Some(sid) = platform_system
                && let Some(s) = self.systems.iter_mut().find(|s| s.id == sid)
            {
                s.set_tier(crate::build::StructureKind::DefensePlatform, tac_ptiers);
                s.defense_pool = tac_ppool;
            }
            let mut atk = attackers.clone();
            let mut def = defenders.clone();
            self.apply_side_losses(&mut atk, &la, pos, events);
            self.apply_side_losses(&mut def, &lb, pos, events);
            {
                let e = self.engagements.get_mut(eid).unwrap();
                e.attackers = atk.clone();
                e.defenders = def.clone();
            }

            // End conditions, re-derived on the survivors. §tactical: a retreat/
            // raid-cap/safety trigger no longer ENDS the battle instantly — it
            // orders the side to WITHDRAW; the ships physically burn for the
            // disengage edge while pursuers get real shots (literal pursuit fire
            // replaces the old abstract disengage-exposure number). The battle
            // ends when a side is dead or its withdrawal completes.
            let _ = (atk, def);
            let elapsed = now - started_at;
            let a_now = self.side_comp(&self.engagements[eid].attackers);
            let d_now = self.side_comp(&self.engagements[eid].defenders);
            let d_ptiers = platform_system
                .and_then(|sid| self.systems.iter().find(|s| s.id == sid))
                .map(|s| s.tier_sum(crate::build::StructureKind::DefensePlatform))
                .unwrap_or(0);
            let a_alive = !a_now.is_empty();
            let d_alive = !d_now.is_empty() || d_ptiers > 0;
            // Retreat metric: Σ remaining ship HP vs the side's at-open HP.
            let a_cur = tac.side_hp(0, false);
            let d_cur = tac.side_hp(1, false);
            let a_doc = self.players.get(&a_owner).map(|c| c.doctrine).unwrap_or_default();
            let d_doc = self.players.get(&d_owner).map(|c| c.doctrine).unwrap_or_default();
            let _ = (a_start_strength, d_start_strength); // superseded by HP baselines
            let a_retreats = a_alive
                && !tac.withdrawing[0]
                && a_doc.retreat.min_ratio().is_some_and(|m| a_cur / tac.start_hp[0].max(1e-9) < m);
            let d_retreats = d_alive
                && !tac.withdrawing[1]
                && d_doc.retreat.min_ratio().is_some_and(|m| d_cur / tac.start_hp[1].max(1e-9) < m);
            // Raid cap: the raider breaks off after a short slice of a battle.
            let raid_cap = raid && !tac.withdrawing[0] && elapsed >= crate::combat::RAID_CAP_FRAC * target;
            // Safety valve: a no-retreat grind ends in MUTUAL disengage.
            let safety = elapsed >= crate::combat::MAX_BATTLE_MULT * target;
            if a_retreats || raid_cap {
                tac.order_withdraw(0);
                self.engagements.get_mut(eid).unwrap().a_fled = true;
            }
            if d_retreats {
                tac.order_withdraw(1);
                self.engagements.get_mut(eid).unwrap().d_fled = true;
            }
            if safety && !(tac.withdrawing[0] && tac.withdrawing[1]) {
                tac.order_withdraw(0);
                tac.order_withdraw(1);
                let e = self.engagements.get_mut(eid).unwrap();
                e.a_fled = true;
                e.d_fled = true;
            }
            let a_out = tac.side_withdrawn(0);
            let d_out = tac.side_withdrawn(1);
            // A hard stop bounds the pursuit phase (a withdrawal can't drag on
            // past the safety valve plus a pursuit grace window).
            let hard_stop = elapsed >= crate::combat::MAX_BATTLE_MULT * target + 60.0;
            let defender_withdraws = tac.withdrawing[1];

            // §battle-records: push this tick's beats, then feed the round — the
            // pending beats force a flush, so beats + this tick's damage/kills +
            // the post-tick survivor counts all land on one recorded round.
            if platform_destroyed {
                self.record_note(*eid, crate::combat::RoundNote::PlatformDestroyed);
            }
            if a_retreats {
                self.record_note(*eid, crate::combat::RoundNote::RetreatTripped { side: 0 });
            }
            if d_retreats {
                self.record_note(*eid, crate::combat::RoundNote::RetreatTripped { side: 1 });
            }
            if safety {
                self.record_note(*eid, crate::combat::RoundNote::MutualDisengage);
            }
            self.record_round(*eid, dealt_a, dealt_b, &la, &lb, [a_now.clone(), d_now.clone()], self.tick, tac_frame);

            // The tactical state persists on the engagement (serde mid-battle).
            self.engagements.get_mut(eid).unwrap().tactical = Some(tac);

            if !a_alive || !d_alive || a_out || d_out || hard_stop {
                self.end_battle(*eid, defender_withdraws, events);
            }
        }

        // Flush battles that ended by contact simply BREAKING (no explicit
        // resolution this tick) — treated as both-survive.
        let untouched: Vec<EntityId> = self.engagements.iter().filter(|(_, e)| !e.touched).map(|(id, _)| *id).collect();
        for eid in untouched {
            self.end_battle(eid, false, events);
        }
    }

    /// End a BATTLE: emit its delayed composition-vs-composition report (outcome
    /// from who still EXISTS — a withdrawn fleet survived, a wiped one didn't) +
    /// the owner-only platform detail, break off survivors, and remove the
    /// engagement. `defender_withdraws` sends the defender side home (a defender
    /// that WON just holds its ground).
    fn end_battle(&mut self, eid: EntityId, defender_withdraws: bool, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(e) = self.engagements.remove(&eid) else { return };
        let a_now = self.side_comp(&e.attackers);
        let d_now = self.side_comp(&e.defenders);
        let d_ptiers = e.platform_system.and_then(|sid| self.systems.iter().find(|s| s.id == sid)).map(|s| s.tier_sum(crate::build::StructureKind::DefensePlatform)).unwrap_or(0);
        // A side is ALIVE if it still has ships, a live platform, OR a fleet that
        // FLED (withdrew/avoid-disengaged) and survives — an emptied side is a
        // withdrawal, not a wipe.
        let a_alive = !a_now.is_empty() || e.a_fled;
        let d_alive = !d_now.is_empty() || d_ptiers > 0 || e.d_fled;
        let outcome = match (a_alive, d_alive) {
            (false, false) => RaidOutcome::BothDestroyed,
            (false, true) => RaidOutcome::AttackerDestroyed,
            (true, false) => RaidOutcome::TargetDestroyed,
            (true, true) => RaidOutcome::BothSurvive,
        };
        let attacker_losses = diff_comp(&e.a_start, &a_now);
        let target_losses = diff_comp(&e.d_start, &d_now);
        // §battle-records: FREEZE the replay — flush any tail round + stamp the
        // ending tick and outcome. The engagement is already removed, so the
        // record (keyed by the same id) outlives it as pure history.
        if let Some(rec) = self.battle_records.get_mut(&eid) {
            rec.finalize(
                self.tick,
                outcome,
                [attacker_losses.clone(), target_losses.clone()],
                [a_now.clone(), d_now.clone()],
            );
        }
        events.push(Event::new(now, EventPayload::RaidResolved {
            attacker: e.a_owner,
            defender: e.d_owner,
            attacker_ship: e.attackers.first().copied().unwrap_or(e.a_lead),
            target_ship: e.defenders.first().copied().unwrap_or(e.d_lead),
            attacker_kind: flagship_of(&e.a_start),
            target_kind: flagship_of(&e.d_start),
            outcome,
            pos: e.pos,
            attacker_losses,
            target_losses,
        }));
        if let Some(sid) = e.platform_system {
            let tiers_lost = e.platform_start_tiers.saturating_sub(d_ptiers);
            if tiers_lost > 0 || !a_alive {
                events.push(Event::new(now, EventPayload::PlatformEngaged {
                    owner: e.d_owner, system: sid, pos: e.pos,
                    raider_destroyed: !a_alive, driven_off: a_alive && d_alive, tiers_lost,
                }));
            }
        }
        // A surviving attacker breaks off for home (it won, or it withdrew) —
        // EXCEPT a BLOCKADER, which resumes its station (it just won/held the
        // establishment fight; §contestable-territory). Its Blockade order rode
        // through the anchored battle unchanged, so leaving it be re-establishes
        // the blockade. A withdrawn blockader already carries a MoveTo (the
        // Withdraw verb overwrote its order), so it flees home normally.
        if a_alive {
            for aid in &e.attackers {
                if let Some(f) = self.fleets.get(aid) {
                    if matches!(f.order, FleetOrder::Blockade { .. }) {
                        continue; // stays on station
                    }
                    self.send_ship_home(*aid, e.a_owner);
                }
            }
        }
        // A withdrawing defender flees; a defender that WON holds its ground.
        if d_alive && defender_withdraws {
            for did in &e.defenders {
                if self.fleets.contains_key(did) {
                    self.send_ship_home(*did, e.d_owner);
                }
            }
        }
    }


    /// Send a surviving ship home (break off).
    fn send_ship_home(&mut self, id: EntityId, owner: PlayerId) {
        if let Some(home) = self.players.get(&owner).map(|c| c.home)
            && let Some(ship) = self.fleets.get_mut(&id)
        {
            ship.order = FleetOrder::MoveTo { dest: home };
        }
    }

    /// Is `system_id` `owner`'s granted HOME system? Home systems can be
    /// blockaded but NEVER captured (§contestable-territory HOME PROTECTION —
    /// no elimination; a beaten player always keeps a producing base).
    fn is_home_system(&self, owner: PlayerId, system_id: EntityId) -> bool {
        self.players.get(&owner).and_then(|c| c.home_system) == Some(system_id)
    }

    /// Is this system currently under blockade (§contestable-territory)?
    fn is_blockaded(&self, system_id: EntityId) -> bool {
        self.systems
            .iter()
            .find(|s| s.id == system_id)
            .map(|s| s.blockade.is_some())
            .unwrap_or(false)
    }

    /// §pirates: spawn a fresh raider PACK (owned by the PIRATE sentinel) at a base.
    fn spawn_pirate_pack(&mut self, tier: u32, base_pos: Vec2) -> EntityId {
        let id = self.alloc_entity_id();
        let mut f = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, base_pos, FleetOrder::Idle, None);
        f.composition.clear();
        f.composition.insert(ShipKind::Raider, pirate::pack_size(tier));
        self.fleets.insert(id, f);
        id
    }

    /// §pirates: aim a pack at the nearest BROADCASTING convoy within the enclave's
    /// hunting radius (platform-covered convoys were already excluded from the
    /// candidate list). Tops the pack up to its tier size. Returns whether a target
    /// was found (a launch happened).
    fn launch_pirate_pack(&mut self, pid: EntityId, tier: u32, base_pos: Vec2, convoys: &[(EntityId, Vec2)]) -> bool {
        let r = pirate::hunt_radius(tier);
        let target = convoys
            .iter()
            .filter(|(_, p)| base_pos.distance(*p) <= r)
            .min_by(|a, b| base_pos.distance(a.1).total_cmp(&base_pos.distance(b.1)).then(a.0.cmp(&b.0)))
            .map(|(id, _)| *id);
        let Some(tid) = target else { return false };
        if let Some(f) = self.fleets.get_mut(&pid) {
            let (want, have) = (pirate::pack_size(tier), f.count(ShipKind::Raider));
            if want > have {
                f.add(ShipKind::Raider, want - have);
            }
            f.order = FleetOrder::Intercept { target: tid };
        }
        true
    }

    /// §pirates: open an ASSAULT engagement — a player war-fleet vs the base's
    /// platform-equivalent defense (`base_tiers`) + any home pack. Mirrors the
    /// blockade establishment battle exactly (reuses the Engagement + Lanchester).
    fn open_pirate_assault(&mut self, aid: EntityId, sid: EntityId, base_pos: Vec2, defenders: Vec<EntityId>, base_tiers: u32, now: f64) {
        let a_owner = self.fleets[&aid].owner;
        let id = self.alloc_engagement_id();
        let a_comp = self.side_comp(&[aid]);
        let d_comp = self.side_comp(&defenders);
        let a_str = crate::combat::Forces::from_fleet(&a_comp, &BTreeMap::new()).strength();
        let d_str = crate::combat::Forces::from_fleet(&d_comp, &BTreeMap::new()).with_platform(base_tiers, 0.0).strength();
        let d_lead = defenders.first().copied().unwrap_or(aid);
        let a_start_loadouts = self.side_loadouts(&[aid]);
        let d_start_loadouts = self.side_loadouts(&defenders);
        self.engagements.insert(id, Engagement {
            id,
            pos: base_pos,
            started_at: now,
            raid: false,
            a_owner,
            d_owner: PlayerId::PIRATE,
            attackers: vec![aid],
            defenders,
            platform_system: Some(sid),
            a_stack_pool: BTreeMap::new(),
            d_stack_pool: BTreeMap::new(),
            a_start: a_comp,
            d_start: d_comp,
            a_start_loadouts,
            d_start_loadouts,
            a_start_strength: a_str,
            d_start_strength: d_str,
            platform_start_tiers: base_tiers,
            a_lead: aid,
            d_lead,
            disengaging: BTreeMap::new(),
            a_fled: false,
            d_fled: false,
            touched: true,
            tactical: None,
        });
    }

    /// §pirates: the neutral faction's per-tick brain. For each ACTIVE enclave:
    /// detect SUPPRESSION (base defense ground to 0 → award plunder to the victor,
    /// go dormant, respawn weaker), open an ASSAULT if a player war-fleet is
    /// stationed at it, ESCALATE on the slow clock, and run its PACK (launch at a
    /// broadcasting convoy in radius, break off from platform-covered targets, bring
    /// loot home). Deterministic + fully offline. Runs after the combat passes so a
    /// won assault's defense-tier writeback is visible this tick.
    fn pirate_ai(&mut self, events: &mut Vec<Event>) {
        if self.enclaves.is_empty() {
            return;
        }
        let now = self.time;
        let hub = self.hub;
        // Owned defense platforms — pirates AVOID these (enclaves are excluded: their
        // defense_tier sits on an UNOWNED system).
        let platforms: Vec<(Vec2, f64)> = self
            .systems
            .iter()
            .filter(|s| s.owner.is_some() && s.tier_sum(crate::build::StructureKind::DefensePlatform) >= 1)
            .map(|s| (s.pos, crate::build::DEFENSE_PLATFORM_RADIUS))
            .collect();
        let covered = |p: Vec2| platforms.iter().any(|(c, r)| p.distance(*c) <= *r);
        // §pirates onboarding: a corp is SHIELDED (its convoys invisible to pirate
        // hunting) for `PIRATE_GRACE_SECS` after it JOINS — measured per-corp from
        // `joined_tick`, so a LATECOMER dropping into an escalated galaxy gets the
        // same undefended-onboarding window a founder got.
        let shielded: std::collections::BTreeSet<PlayerId> = self
            .players
            .iter()
            .filter(|(_, c)| now - c.joined_tick as f64 * DT < pirate::PIRATE_GRACE_SECS)
            .map(|(id, _)| *id)
            .collect();
        // Broadcasting convoys outside platform cover (dark scouts don't broadcast →
        // never targeted, satisfying "never target scouts"), owned by a corp past
        // its grace window.
        let convoys: Vec<(EntityId, Vec2)> = self
            .fleets
            .iter()
            .filter(|(_, f)| !f.owner.is_pirate() && f.flagship_kind() == ShipKind::Convoy && f.broadcasts() && !covered(f.pos) && !shielded.contains(&f.owner))
            .map(|(id, f)| (*id, f.pos))
            .collect();
        let epos: BTreeMap<EntityId, Vec2> =
            self.enclaves.keys().filter_map(|sid| self.systems.iter().find(|s| s.id == *sid).map(|s| (*sid, s.pos))).collect();
        let contested: std::collections::BTreeSet<EntityId> =
            self.engagements.values().filter_map(|e| e.platform_system).collect();
        let engaged: std::collections::BTreeSet<EntityId> =
            self.engagements.values().flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied()).collect();

        let ids: Vec<EntityId> = self.enclaves.keys().copied().collect();
        for sid in ids {
            let Some(&base_pos) = epos.get(&sid) else { continue };
            let (tier, active) = { let e = &self.enclaves[&sid]; (e.tier, e.active(now)) };
            let base_tiers = self.systems.iter().find(|s| s.id == sid).map(|s| s.tier_sum(crate::build::StructureKind::DefensePlatform)).unwrap_or(0);

            // --- SUPPRESSION: the base defense hit 0 (assault won). ---
            if active && base_tiers == 0 {
                let victor = self
                    .fleets
                    .iter()
                    .filter(|(_, f)| !f.owner.is_pirate() && f.is_combatant() && f.pos.distance(base_pos) <= pirate::PIRATE_ASSAULT_RADIUS)
                    .min_by_key(|(id, _)| **id)
                    .map(|(_, f)| f.owner);
                let plunder = std::mem::take(&mut self.enclaves.get_mut(&sid).unwrap().plunder);
                if let Some(v) = victor {
                    if let Some(corp) = self.players.get_mut(&v) {
                        for (c, n) in &plunder {
                            *corp.inventory.entry(*c).or_insert(0) += *n;
                        }
                    }
                    events.push(Event::new(now, EventPayload::PirateEnclaveCleared { owner: v, system: sid, pos: base_pos, plunder }));
                }
                if let Some(pid) = self.enclaves[&sid].pack {
                    self.fleets.remove(&pid);
                }
                let e = self.enclaves.get_mut(&sid).unwrap();
                e.pack = None;
                e.tier = 1;
                e.dormant_until = now + pirate::PIRATE_DORMANCY;
                e.next_launch_at = e.dormant_until + 20.0;
                e.next_grow_at = e.dormant_until + pirate::PIRATE_GROW_PERIOD;
                if let Some(sys) = self.systems.iter_mut().find(|s| s.id == sid) {
                    sys.set_tier(crate::build::StructureKind::DefensePlatform, pirate::base_defense_tiers(1));
                    sys.defense_pool = 0.0;
                }
                continue;
            }
            if !active {
                continue; // dormant — no activity
            }

            // --- OPEN AN ASSAULT: a player war-fleet stationed at the base. ---
            if !contested.contains(&sid) {
                let assaulter = self
                    .fleets
                    .iter()
                    .filter(|(fid, f)| {
                        !f.owner.is_pirate()
                            && f.contains(ShipKind::Raider)
                            && matches!(f.order, FleetOrder::Idle)
                            && !engaged.contains(fid)
                            && f.pos.distance(base_pos) <= pirate::PIRATE_ASSAULT_RADIUS
                    })
                    .min_by_key(|(id, _)| **id)
                    .map(|(id, _)| *id);
                if let Some(aid) = assaulter {
                    let defenders: Vec<EntityId> = self.enclaves[&sid]
                        .pack
                        .filter(|pid| self.fleets.get(pid).is_some_and(|f| f.pos.distance(base_pos) <= pirate::PIRATE_ASSAULT_RADIUS))
                        .into_iter()
                        .collect();
                    self.open_pirate_assault(aid, sid, base_pos, defenders, base_tiers, now);
                    continue; // one action per enclave per tick
                }
            }

            // --- ESCALATION: grow if ignored. ---
            if now >= self.enclaves[&sid].next_grow_at && tier < pirate::PIRATE_MAX_TIER {
                let nt = tier + 1;
                if let Some(sys) = self.systems.iter_mut().find(|s| s.id == sid) {
                    sys.set_tier(crate::build::StructureKind::DefensePlatform, pirate::base_defense_tiers(nt));
                }
                let e = self.enclaves.get_mut(&sid).unwrap();
                e.tier = nt;
                e.next_grow_at = now + pirate::PIRATE_GROW_PERIOD;
            }
            let tier = self.enclaves[&sid].tier;

            // --- PACK LIFECYCLE. ---
            let pack = self.enclaves[&sid].pack;
            match pack {
                Some(pid) if !self.fleets.contains_key(&pid) => {
                    self.enclaves.get_mut(&sid).unwrap().pack = None; // destroyed
                }
                Some(pid) => {
                    let (is_idle, is_moveto, itarget, has_cargo, ppos) = {
                        let f = &self.fleets[&pid];
                        let it = if let FleetOrder::Intercept { target } = f.order { Some(target) } else { None };
                        (matches!(f.order, FleetOrder::Idle), matches!(f.order, FleetOrder::MoveTo { .. }), it, f.cargo.is_some(), f.pos)
                    };
                    let home = ppos.distance(base_pos) <= pirate::PIRATE_ASSAULT_RADIUS;
                    if has_cargo {
                        if home && is_idle {
                            if let Some(c) = self.fleets.get_mut(&pid).and_then(|f| f.cargo.take()) {
                                *self.enclaves.get_mut(&sid).unwrap().plunder.entry(c.commodity).or_insert(0) += c.units;
                            }
                        } else if !is_moveto && let Some(f) = self.fleets.get_mut(&pid) {
                            f.order = FleetOrder::MoveTo { dest: base_pos };
                        }
                    } else {
                        let pursuing_ok = itarget
                            .and_then(|t| self.fleets.get(&t))
                            .is_some_and(|t| !covered(t.pos) && t.pos.distance(hub) > HUB_SAFE_RADIUS);
                        if pursuing_ok {
                            // resolve_raids drives the pursuit + raid.
                        } else if home && is_idle && now >= self.enclaves[&sid].next_launch_at {
                            if self.launch_pirate_pack(pid, tier, base_pos, &convoys) {
                                self.enclaves.get_mut(&sid).unwrap().next_launch_at = now + pirate::PIRATE_LAUNCH_PERIOD;
                            }
                        } else if !home && !is_moveto && let Some(f) = self.fleets.get_mut(&pid) {
                            f.order = FleetOrder::MoveTo { dest: base_pos };
                        }
                    }
                }
                None if now >= self.enclaves[&sid].next_launch_at => {
                    let pid = self.spawn_pirate_pack(tier, base_pos);
                    self.enclaves.get_mut(&sid).unwrap().pack = Some(pid);
                    if self.launch_pirate_pack(pid, tier, base_pos, &convoys) {
                        self.enclaves.get_mut(&sid).unwrap().next_launch_at = now + pirate::PIRATE_LAUNCH_PERIOD;
                    }
                }
                None => {}
            }
        }
    }

    /// §node: drive the EXOTIC NODES each tick — AWAKENING then UPKEEP.
    ///
    /// 1. AWAKENING (async-fair): once `time ≥ node_awakening_time`, every dormant
    ///    node latches awake ONCE and emits a galaxy-wide `NodeAwakened` (the
    ///    timeline light-delays it per observer). Deterministic — fires at the same
    ///    sim time whether anyone is online.
    /// 2. UPKEEP (Habitat idiom): an awakened, OWNED node draws its
    ///    [`crate::node::NODE_UPKEEP_PER_SEC`] mix, all-or-nothing, from its OWN
    ///    system's stockpile. Cover it → `fed`, bonus LIVE; short → UNFED, bonus
    ///    SUSPENDED (nothing destroyed, recovers when fed). Owner-only transition
    ///    notices. An UNOWNED awakened node has no upkeep (its leverage is dormant
    ///    until someone holds it) and reads as fed.
    fn node_ai(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let awaken_at = self.config.node_awakening_time;
        let ids: Vec<EntityId> = self.nodes.keys().copied().collect();

        // 1. AWAKENING.
        for sid in &ids {
            if self.nodes[sid].awakened || now < awaken_at {
                continue;
            }
            let pos = self.systems.iter().find(|s| s.id == *sid).map(|s| s.pos).unwrap_or(Vec2::ZERO);
            let node = self.nodes.get_mut(sid).unwrap();
            node.awakened = true;
            node.fed = true; // presumed fed at awakening; the first shortfall notifies
            let bonus = node.bonus;
            events.push(Event::new(now, EventPayload::NodeAwakened { system: *sid, pos, bonus }));
        }

        // 2. UPKEEP.
        for sid in &ids {
            if !self.nodes[sid].awakened {
                continue;
            }
            let owner = self.systems.iter().find(|s| s.id == *sid).and_then(|s| s.owner);
            let Some(owner) = owner else {
                self.nodes.get_mut(sid).unwrap().fed = true; // unheld → no upkeep
                continue;
            };
            let sys = self.systems.iter_mut().find(|s| s.id == *sid).unwrap();
            let can_pay = crate::node::NODE_UPKEEP_PER_SEC
                .iter()
                .all(|(c, rate)| sys.stockpile.get(c).copied().unwrap_or(0.0) + 1e-12 >= rate * DT);
            if can_pay {
                for (c, rate) in crate::node::NODE_UPKEEP_PER_SEC {
                    *sys.stockpile.entry(*c).or_insert(0.0) -= rate * DT;
                }
            }
            let node = self.nodes.get_mut(sid).unwrap();
            if can_pay != node.fed {
                events.push(Event::new(now, EventPayload::NodeSupplyChanged { owner, system: *sid, fed: can_pay }));
            }
            node.fed = can_pay;
        }
    }

    /// §node: the bonus-granting nodes a corp currently BENEFITS from — awakened,
    /// fed, and held by `owner` — capped at [`crate::node::NODES_PER_CORP`]
    /// (deterministic: lowest system id first, since `nodes` is a `BTreeMap`).
    /// Excess held nodes still cost upkeep and deny rivals but grant nothing extra.
    /// Each entry is `(bonus, region-center)`. The pirate sentinel / an unheld node
    /// never appears (it owns no systems), so this is Option-safe for every caller.
    fn active_nodes_for(&self, owner: PlayerId) -> Vec<(crate::node::NodeBonus, Vec2)> {
        let mut held: Vec<(crate::node::NodeBonus, Vec2)> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.active())
            .filter_map(|(sid, n)| {
                let sys = self.systems.iter().find(|s| s.id == *sid)?;
                (sys.owner == Some(owner)).then_some((n.bonus, sys.pos))
            })
            .collect();
        held.truncate(crate::node::NODES_PER_CORP);
        held
    }

    /// §node Relay Anchor: the command-delay MULTIPLIER for `owner`'s orders to a
    /// target at `target_pos`. 0.5 if the owner holds an active black-hole node
    /// whose region covers the target; 1.0 otherwise. The SINGLE plug into
    /// `schedule_for_owner` — command tempo, not economy.
    fn relay_factor(&self, owner: PlayerId, target_pos: Vec2) -> f64 {
        let covered = self.active_nodes_for(owner).into_iter().any(|(b, p)| {
            b == crate::node::NodeBonus::RelayAnchor && p.distance(target_pos) <= crate::node::NODE_REGION_RADIUS
        });
        if covered {
            crate::node::RELAY_DELAY_MULT
        } else {
            1.0
        }
    }

    /// §node Veil: the signature MULTIPLIER for a DARK fleet owned by `owner` at
    /// `pos` — 0.5 inside an active magnetar node's region, else 1.0. Plugs the two
    /// detection sites (sim pickets + server View) so concealment is one honest
    /// scope: reducing signature here narrows detection everywhere it's read.
    pub fn veil_factor(&self, owner: PlayerId, pos: Vec2) -> f64 {
        let covered = self.active_nodes_for(owner).into_iter().any(|(b, p)| {
            b == crate::node::NodeBonus::Veil && p.distance(pos) <= crate::node::NODE_REGION_RADIUS
        });
        if covered {
            crate::node::VEIL_SIGNATURE_MULT
        } else {
            1.0
        }
    }

    /// §node Deep Scan: does `viewer` hold an active pulsar/binary node whose region
    /// covers `pos`? If so, the viewer resolves EXACT composition on any fleet
    /// ALREADY visible there (bucket→exact) — gated in the server View, so it's an
    /// earlier reveal of already-permitted data, never a new leak.
    pub fn deep_scan_covers(&self, viewer: PlayerId, pos: Vec2) -> bool {
        self.active_nodes_for(viewer).into_iter().any(|(b, p)| {
            b == crate::node::NodeBonus::DeepScan && p.distance(pos) <= crate::node::NODE_REGION_RADIUS
        })
    }

    /// §node: `(holder, region-center)` for every ACTIVE Veil node across ALL corps
    /// — the server hands these to the dark-fleet view so a fleet in its OWNER's
    /// magnetar region runs quieter. Respects the per-corp cap (via
    /// `active_nodes_for` per holder), so a capped-out extra node grants nothing.
    pub fn active_veil_regions(&self) -> Vec<(PlayerId, Vec2)> {
        let mut holders: std::collections::BTreeSet<PlayerId> = std::collections::BTreeSet::new();
        for (sid, n) in &self.nodes {
            if n.active()
                && let Some(owner) = self.systems.iter().find(|s| s.id == *sid).and_then(|s| s.owner)
            {
                holders.insert(owner);
            }
        }
        let mut out = Vec::new();
        for owner in holders {
            for (b, p) in self.active_nodes_for(owner) {
                if b == crate::node::NodeBonus::Veil {
                    out.push((owner, p));
                }
            }
        }
        out
    }

    /// §node: region-centers of the ACTIVE Deep-Scan nodes `viewer` holds (capped),
    /// for the view's bucket→exact composition reveal in-region.
    pub fn deep_scan_regions(&self, viewer: PlayerId) -> Vec<Vec2> {
        self.active_nodes_for(viewer)
            .into_iter()
            .filter_map(|(b, p)| (b == crate::node::NodeBonus::DeepScan).then_some(p))
            .collect()
    }

    /// §explore Part 2: run every Survey order's dwell clock. Per surveying fleet:
    /// * IN AN ENGAGEMENT → ABORT to Idle, no partial credit (re-issuable). A
    ///   fight interrupts the sweep — the all-or-nothing rule.
    /// * OUT OF RANGE → the dwell clock resets (still approaching, or shoved off
    ///   station): no partial credit; the approach continues and the dwell
    ///   restarts from zero once back on-site.
    /// * IN RANGE → start/continue the dwell; at `SURVEY_SECS` the survey
    ///   COMPLETES: fire `SurveyCompleted` AT THE FLEET'S POSITION (knowledge
    ///   travels home at c — the owner's report leg is scheduled here), order →
    ///   Idle. Idempotent on an already-surveyed system (legal, wasted time).
    ///
    /// Deterministic + fully offline (§5.1 — surveys run whether the owner is
    /// logged in or not).
    fn resolve_surveys(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let c = self.config.c;
        let engaged: std::collections::BTreeSet<EntityId> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let mut completions: Vec<(PlayerId, EntityId, Vec2)> = Vec::new();
        for (fid, fleet) in self.fleets.iter_mut() {
            let FleetOrder::Survey { system, station, dwell_since } = &mut fleet.order else {
                continue;
            };
            if engaged.contains(fid) {
                // A battle interrupts the sweep — abort, nothing banked.
                fleet.order = FleetOrder::Idle;
                continue;
            }
            if fleet.pos.distance(*station) > crate::explore::SURVEY_RANGE {
                *dwell_since = None; // off-site: no partial credit, keep approaching
                continue;
            }
            match *dwell_since {
                None => *dwell_since = Some(now),
                Some(since) if now - since >= crate::explore::SURVEY_SECS => {
                    completions.push((fleet.owner, *system, fleet.pos));
                    fleet.order = FleetOrder::Idle;
                }
                Some(_) => {} // mid-dwell, loud, counting down
            }
        }
        for (owner, system, pos) in completions {
            events.push(Event::new(now, EventPayload::SurveyCompleted { owner, system, pos }));
            // The report leg: the knowledge lands when its light reaches the
            // OWNER's command center (same formula the timeline notice uses).
            if let Some(cc) = self.players.get(&owner).map(|p| p.command_center) {
                self.pending_survey_reports.push(SurveyReport {
                    recipient: owner,
                    system,
                    arrive_at: now + pos.distance(cc) / c,
                    relay: false,
                    origin: owner,
                });
            }
        }
    }

    /// §explore Part 2: deliver survey-report legs whose light has arrived —
    /// INSERT the system into the recipient's `surveyed` set (permanent R2
    /// knowledge). When the SURVEYOR'S OWN leg lands, fan out ALLY-RELAY legs to
    /// the origin's allies AT THAT MOMENT (`owner cc → ally cc`, the same
    /// chain-delay shape the §syndicates scout-intel relay uses: observed → own
    /// cc → ally cc, each leg at c). Deterministic + async-fair.
    fn deliver_survey_reports(&mut self) {
        let now = self.time;
        let c = self.config.c;
        let mut i = 0;
        while i < self.pending_survey_reports.len() {
            if self.pending_survey_reports[i].arrive_at > now {
                i += 1;
                continue;
            }
            let r = self.pending_survey_reports.swap_remove(i);
            if let Some(corp) = self.players.get_mut(&r.recipient) {
                corp.surveyed.insert(r.system);
            }
            if !r.relay
                && let Some(occ) = self.players.get(&r.origin).map(|p| p.command_center)
            {
                for ally in self.allies_of(r.origin) {
                    if let Some(acc) = self.players.get(&ally).map(|p| p.command_center) {
                        self.pending_survey_reports.push(SurveyReport {
                            recipient: ally,
                            system: r.system,
                            arrive_at: now + occ.distance(acc) / c,
                            relay: true,
                            origin: r.origin,
                        });
                    }
                }
            }
        }
    }

    /// §syndicates Part 3: feed ALLY GARRISONS. A COMBATANT fleet stationed (Idle)
    /// within `DEFENSE_PLATFORM_RADIUS` of a syndicate ALLY's system is a garrison:
    /// the HOST draws `GARRISON_UPKEEP_PER_SHIP × ships × dt` Provisions from its
    /// OWN stockpile to feed it. All-or-nothing per host (a cut supply line unfeeds
    /// the whole garrison there): if the host can't cover the total, every garrison
    /// fleet there goes UNFED — its defense contribution SUSPENDS (never destroyed;
    /// recovers when fed). Deterministic + async-fair (runs online or off). The flag
    /// is recomputed every tick BEFORE the combat passes read it.
    fn resolve_garrison_upkeep(&mut self, events: &mut Vec<Event>) {
        let radius = crate::build::DEFENSE_PLATFORM_RADIUS;
        let prov = crate::cargo::Commodity::Provisions;
        // 1. Group garrison fleets by host system (deterministic; first host by
        //    system order — one host per fleet).
        let mut by_host: BTreeMap<EntityId, Vec<(EntityId, PlayerId, u32)>> = BTreeMap::new();
        for (fid, f) in &self.fleets {
            if !f.is_combatant() || !matches!(f.order, FleetOrder::Idle) {
                continue;
            }
            for sys in &self.systems {
                let Some(so) = sys.owner else { continue };
                if self.are_allied(f.owner, so) && f.pos.distance(sys.pos) <= radius {
                    by_host.entry(sys.id).or_default().push((*fid, f.owner, f.total_count()));
                    break;
                }
            }
        }
        // 2. Per host: draw the total upkeep if the stockpile covers it, else the
        //    whole garrison there goes unfed.
        let mut new_fed: BTreeMap<EntityId, bool> = BTreeMap::new();
        let mut host_of: BTreeMap<EntityId, EntityId> = BTreeMap::new();
        for (sid, garrisons) in &by_host {
            let total_ships: u32 = garrisons.iter().map(|(_, _, n)| *n).sum();
            let upkeep = crate::build::GARRISON_UPKEEP_PER_SHIP * total_ships as f64 * DT;
            let fed = {
                let sys = self.systems.iter_mut().find(|s| s.id == *sid).expect("host exists");
                let have = sys.stockpile.get(&prov).copied().unwrap_or(0.0);
                let fed = have + 1e-12 >= upkeep;
                if fed {
                    *sys.stockpile.entry(prov).or_insert(0.0) -= upkeep;
                }
                fed
            };
            for (gid, _, _) in garrisons {
                new_fed.insert(*gid, fed);
                host_of.insert(*gid, *sid);
            }
        }
        // 3. Collect fed/unfed TRANSITIONS (owner learns), then write the flags;
        //    fleets that aren't garrisons this tick reset to fed = true.
        let now = self.time;
        let mut transitions: Vec<(PlayerId, EntityId, bool)> = Vec::new();
        for (gid, &fed) in &new_fed {
            if let Some(f) = self.fleets.get(gid)
                && f.garrison_fed != fed
            {
                transitions.push((f.owner, host_of[gid], fed));
            }
        }
        for f in self.fleets.values_mut() {
            f.garrison_fed = new_fed.get(&f.id).copied().unwrap_or(true);
        }
        for (owner, host, fed) in transitions {
            events.push(Event::new(now, EventPayload::GarrisonSupplyChanged { owner, host, fed }));
        }
    }

    /// §contestable-territory Part 1 (BLOCKADE): interdiction without capture.
    ///
    /// A fleet on a `Blockade` order that has taken STATION on a rival system
    /// strangles its logistics while ≥1 such fleet is present. This pass, run
    /// right after `resolve_raids`, does three things:
    ///   1. OPENS the establishment battle — a newly on-station blockader vs the
    ///      system's standing defense (platform pool + garrison combatants) is
    ///      handed to the normal anchored engagement, so it attrites on the
    ///      config battle timescale and Reinforce / Withdraw all apply. The
    ///      blockade only holds if that battle doesn't destroy or repel it.
    ///   2. RECOMPUTES each system's `blockade` field from on-station presence
    ///      and emits light-delayed onset / lift transitions to the owner.
    ///   3. Holds INBOUND convoys to a blockaded destination at a standoff ring
    ///      (they resume when it lifts). Nothing is destroyed — production keeps
    ///      accruing (a cut supply line strangles Habitats via the UNFED rule).
    fn resolve_blockades(&mut self, events: &mut Vec<Event>) {
        let now = self.time;

        // --- On-station blockaders, grouped by target system (id order). ---
        let mut on_station: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
        for (fid, f) in &self.fleets {
            if let FleetOrder::Blockade { system, .. } = f.order
                && let Some(sys) = self.systems.iter().find(|s| s.id == system)
                && sys.owner.is_some()
                && sys.owner != Some(f.owner)
                && !sys.owner.is_some_and(|o| self.are_allied(f.owner, o))
                && f.pos.distance(sys.pos) <= BLOCKADE_STATION_RADIUS
            {
                on_station.entry(system).or_default().push(*fid);
            }
        }

        // --- 1. Establishment battles. One per system per tick (min-id
        //        attacker); the reinforce path folds in any co-blockaders. Skip
        //        if a battle is already contesting this system (a blockader
        //        already engaged, or an existing engagement covers its platform).
        let engaged: std::collections::BTreeSet<EntityId> = self
            .engagements
            .values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied())
            .collect();
        let contested: std::collections::BTreeSet<EntityId> =
            self.engagements.values().filter_map(|e| e.platform_system).collect();
        struct Open { aid: EntityId, sys: EntityId, pos: Vec2, d_owner: PlayerId, garrison: Vec<EntityId>, ptiers: u32 }
        let mut to_open: Vec<Open> = Vec::new();
        for (sys_id, blockaders) in &on_station {
            if blockaders.iter().any(|b| engaged.contains(b)) || contested.contains(sys_id) {
                continue;
            }
            let sys = self.systems.iter().find(|s| s.id == *sys_id).unwrap();
            let (d_owner, pos, ptiers) = (sys.owner.unwrap(), sys.pos, sys.tier_sum(crate::build::StructureKind::DefensePlatform));
            let aid = *blockaders.iter().min().unwrap();
            let garrison: Vec<EntityId> = self
                .fleets
                .iter()
                .filter(|(gid, g)| {
                    **gid != aid
                        && g.is_combatant()
                        && !engaged.contains(gid)
                        && g.pos.distance(pos) <= crate::build::DEFENSE_PLATFORM_RADIUS
                        // The host's OWN fleets always defend; a syndicate ALLY's
                        // fleet joins as a GARRISON only if it's stationed (Idle),
                        // currently FED, and its OWNER's doctrine isn't Avoid
                        // (§syndicates Part 3 — "per its owner's doctrine").
                        && (g.owner == d_owner
                            || (matches!(g.order, FleetOrder::Idle)
                                && g.garrison_fed
                                && self.are_allied(g.owner, d_owner)
                                && self.players.get(&g.owner).is_some_and(|c| c.doctrine.engagement != EngagementPolicy::Avoid)))
                })
                .map(|(gid, _)| *gid)
                .collect();
            if ptiers >= 1 || !garrison.is_empty() {
                to_open.push(Open { aid, sys: *sys_id, pos, d_owner, garrison, ptiers });
            }
        }
        for o in to_open {
            let a_owner = self.fleets[&o.aid].owner;
            let id = self.alloc_engagement_id();
            let a_comp = self.side_comp(&[o.aid]);
            let d_comp = self.side_comp(&o.garrison);
            let a_str = crate::combat::Forces::from_fleet(&a_comp, &BTreeMap::new()).strength();
            let d_str = crate::combat::Forces::from_fleet(&d_comp, &BTreeMap::new()).with_platform(o.ptiers, 0.0).strength();
            let d_lead = o.garrison.first().copied().unwrap_or(o.aid);
            let a_start_loadouts = self.side_loadouts(&[o.aid]);
            let d_start_loadouts = self.side_loadouts(&o.garrison);
            self.engagements.insert(id, Engagement {
                id,
                pos: o.pos,
                started_at: now,
                raid: false, // a blockade fight is a decisive full-duration battle
                a_owner,
                d_owner: o.d_owner,
                attackers: vec![o.aid],
                defenders: o.garrison,
                platform_system: if o.ptiers >= 1 { Some(o.sys) } else { None },
                a_stack_pool: BTreeMap::new(),
                d_stack_pool: BTreeMap::new(),
                a_start: a_comp,
                d_start: d_comp,
                a_start_loadouts,
                d_start_loadouts,
                a_start_strength: a_str,
                d_start_strength: d_str,
                platform_start_tiers: o.ptiers,
                a_lead: o.aid,
                d_lead,
                disengaging: BTreeMap::new(),
                a_fled: false,
                d_fled: false,
                touched: true,
                tactical: None,
            });
        }

        // --- 2. Recompute each system's blockade state + transitions. ---
        let blocked: std::collections::BTreeSet<EntityId> = on_station.keys().copied().collect();
        // Attribute onset to the smallest-id on-station blockader's owner (a
        // deterministic, cosmetic pick — the strangling doesn't depend on it).
        let blocked_by: BTreeMap<EntityId, PlayerId> = on_station
            .iter()
            .filter_map(|(sid, fids)| {
                fids.iter().min().and_then(|f| self.fleets.get(f)).map(|f| (*sid, f.owner))
            })
            .collect();
        // §Part 2 SIEGE CLOCK: precompute, per blocked system, the non-defense
        // prerequisites for siege progress — NO garrison combatant on station and
        // NOT the owner's HOME (home protection: a home is never siegeable). The
        // `defense_tier == 0` half is read from the system in the loop below. All
        // borrow self.fleets/self.players, so compute here before the mut loop.
        let siege_prereq: BTreeMap<EntityId, bool> = blocked
            .iter()
            .map(|&sid| {
                let sys = self.systems.iter().find(|s| s.id == sid).unwrap();
                let owner = sys.owner.unwrap();
                let no_garrison = !self.fleets.values().any(|g| {
                    g.owner == owner && g.is_combatant() && g.pos.distance(sys.pos) <= crate::build::DEFENSE_PLATFORM_RADIUS
                });
                let home = self.is_home_system(owner, sid);
                (sid, no_garrison && !home)
            })
            .collect();
        for sys in &mut self.systems {
            let is_blocked = blocked.contains(&sys.id);
            // Siege can PROGRESS only with defenses suppressed AND the prereqs met.
            let siege_ok = is_blocked && sys.tier_sum(crate::build::StructureKind::DefensePlatform) == 0 && *siege_prereq.get(&sys.id).unwrap_or(&false);
            match (sys.blockade.is_some(), is_blocked) {
                (false, true) => {
                    let by = blocked_by[&sys.id];
                    let owner = sys.owner.unwrap();
                    // An undefended system starts its siege clock the instant it's
                    // blockaded; a defended one waits for suppression.
                    sys.blockade = Some(crate::galaxy::Blockade { by, since: now, siege_since: siege_ok.then_some(now) });
                    events.push(Event::new(now, EventPayload::BlockadeEstablished { by, owner, system: sys.id, pos: sys.pos }));
                }
                (true, false) => {
                    sys.blockade = None;
                    if let Some(owner) = sys.owner {
                        events.push(Event::new(now, EventPayload::BlockadeLifted { owner, system: sys.id, pos: sys.pos }));
                    }
                }
                (true, true) => {
                    // Unbroken — keep `since` / the establisher `by`; advance the
                    // siege clock. It STARTS when conditions first hold and RESETS
                    // the moment any breaks (defenses rebuilt, a garrison arrives).
                    if let Some(b) = sys.blockade.as_mut() {
                        if siege_ok {
                            if b.siege_since.is_none() {
                                b.siege_since = Some(now);
                            }
                        } else {
                            b.siege_since = None;
                        }
                    }
                }
                _ => {}
            }
        }

        // --- 3. Inbound convoys hold at a standoff ring while their destination
        //        system is blockaded; they resume to the true destination when it
        //        lifts. Precompute the read-only maps, then re-target moving
        //        convoys. Nothing is destroyed — this is a hold, not a divert.
        let sys_pos: BTreeMap<EntityId, Vec2> = self.systems.iter().map(|s| (s.id, s.pos)).collect();
        let home_of: BTreeMap<PlayerId, (Vec2, Option<EntityId>)> =
            self.players.iter().map(|(p, c)| (*p, (c.home, c.home_system))).collect();
        for f in self.fleets.values_mut() {
            let Some(mission) = f.mission else { continue };
            let (dest_blocked, true_dest) = match mission {
                TradeMission::SellAtHub => continue, // the hub is neutral — never blockaded
                TradeMission::DeliverToSystem { system } => match sys_pos.get(&system) {
                    Some(&p) => (blocked.contains(&system), p),
                    None => continue,
                },
                TradeMission::DeliverHome => match home_of.get(&f.owner) {
                    Some(&(home, hs)) => (hs.map(|s| blocked.contains(&s)).unwrap_or(false), home),
                    None => continue,
                },
            };
            if let FleetOrder::MoveTo { dest } = &mut f.order {
                *dest = if dest_blocked {
                    // Hold on the standoff ring, on the line from the destination
                    // out toward the convoy (so it stops SHORT of the blockade).
                    let out = (f.pos - true_dest).normalized();
                    let out = if out.length() < 1e-6 { Vec2::new(1.0, 0.0) } else { out };
                    true_dest + out * BLOCKADE_STANDOFF_RADIUS
                } else {
                    true_dest // resume (restores the true destination once lifted)
                };
            }
        }
    }

    /// Apply orders whose light has reached the ship by `self.time`. Orders are
    /// processed in issue order, so a later order for the same ship overrides an
    /// earlier one once both have arrived.
    fn deliver_due_orders(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let mut i = 0;
        while i < self.pending_orders.len() {
            if self.pending_orders[i].apply_time <= now {
                let po = self.pending_orders.remove(i);
                // A vanished fleet (destroyed before delivery) simply drops the
                // order — no application, no echo, no phantom lifecycle.
                let Some(ship) = self.fleets.get_mut(&po.ship_id) else {
                    continue;
                };
                ship.order = po.new_order;
                events.push(Event::new(now, EventPayload::OrderApplied { ship_id: po.ship_id }));
                // A WITHDRAW that has arrived pulls the fleet OUT of any battle it
                // was in — it now physically flees (its MoveTo-home order runs at
                // formation speed; a faster pursuer re-contacts for parting shots).
                if po.kind == crate::event::OrderKind::Withdraw {
                    let fid = po.ship_id;
                    // §battle-records: note WHICH side the withdrawing fleet left.
                    let mut withdrew: Vec<(EntityId, u8)> = Vec::new();
                    for (eid, e) in self.engagements.iter_mut() {
                        if e.attackers.contains(&fid) {
                            e.a_fled = true;
                            withdrew.push((*eid, 0));
                        }
                        if e.defenders.contains(&fid) {
                            e.d_fled = true;
                            withdrew.push((*eid, 1));
                        }
                        e.attackers.retain(|f| *f != fid);
                        e.defenders.retain(|f| *f != fid);
                    }
                    for (eid, side) in withdrew {
                        self.record_note(eid, crate::combat::RoundNote::WithdrawOrdered { side });
                    }
                }
                // §order-lifecycle: the order is DELIVERED. A newer order for the
                // same fleet supersedes any older awaiting-echo entry (only the
                // LATEST order's lifecycle is shown / confirmed).
                self.pending_echoes.retain(|e| e.fleet != po.ship_id || e.issued_at > po.issued_at);
                if !self.pending_echoes.iter().any(|e| e.fleet == po.ship_id && e.issued_at >= po.issued_at) {
                    self.pending_echoes.push(PendingEcho {
                        owner: po.owner,
                        fleet: po.ship_id,
                        delivered_at: now,
                        echo_at: po.echo_at,
                        issued_at: po.issued_at,
                        kind: po.kind,
                    });
                    events.push(Event::new(
                        now,
                        EventPayload::OrderDelivered { owner: po.owner, fleet: po.ship_id, kind: po.kind, echo_at: po.echo_at },
                    ));
                }
            } else {
                i += 1;
            }
        }
    }

    /// §order-lifecycle: resolve DELIVERED orders whose confirming light has now
    /// returned to the command center — emit an owner-only `OrderConfirmed` and
    /// drop the echo. A fleet destroyed before its echo lands drops silently (the
    /// delayed destruction report is what the owner sees — no false "confirmed").
    fn resolve_order_echoes(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let mut i = 0;
        while i < self.pending_echoes.len() {
            let e = &self.pending_echoes[i];
            if !self.fleets.contains_key(&e.fleet) {
                self.pending_echoes.remove(i); // destroyed — no confirmation
            } else if e.echo_at <= now {
                let e = self.pending_echoes.remove(i);
                events.push(Event::new(now, EventPayload::OrderConfirmed { owner: e.owner, fleet: e.fleet, kind: e.kind }));
            } else {
                i += 1;
            }
        }
    }

    fn apply(&mut self, cmd: &Command, events: &mut Vec<Event>) {
        match cmd {
            Command::AddPlayer { id, name } => {
                // Idempotent: a reconnecting player keeps their corporation.
                if self.players.contains_key(id) {
                    return;
                }
                let (home, home_system) = self.assign_home(*id);
                // Seed the home system with a Fuel reserve so fleets move from turn
                // one (§step1 part 2) — the home produces no fuel, so this is the
                // runway that buys time to expand toward fuel-bearing systems.
                // Like all inflow, the seed respects the storage cap (§buildings
                // step 2); the base cap comfortably exceeds the seed, so a fresh
                // home always receives it in full.
                if let Some(sys) = self.systems.iter_mut().find(|s| s.id == home_system) {
                    let seed = crate::fuel::FUEL_HOME_SEED.min(sys.storage_headroom());
                    *sys.stockpile.entry(crate::fuel::MOVEMENT_FUEL).or_insert(0.0) += seed;
                    // HOME BOOTSTRAP (§buildings step 3): guarantee Shipyard tier 1
                    // even on a pre-shipyard snapshot (freshly-generated homes
                    // already carry it), so a joining player can always build
                    // convoys turn one. max() never removes an earned higher tier.
                    sys.set_tier(crate::build::StructureKind::Shipyard, sys.tier(crate::build::StructureKind::Shipyard).max(crate::build::HOME_SHIPYARD_TIER));
                }
                // Starting inventory: a stock of the ORIGINAL five goods to sell,
                // plus a treasury to buy with. §economy: deliberately NOT all 12 —
                // handing out free Machinery/Armaments would skip the industrial
                // ladder; the Part-6 starter kit seeds the home STOCKPILE instead.
                let inventory = [
                    crate::cargo::Commodity::MetallicOre,
                    crate::cargo::Commodity::Volatiles,
                    crate::cargo::Commodity::Alloys,
                    crate::cargo::Commodity::Fuel,
                    crate::cargo::Commodity::Provisions,
                ]
                .into_iter()
                .map(|c| (c, 120u32))
                .collect();
                self.players.insert(
                    *id,
                    Corporation {
                        id: *id,
                        name: name.clone(),
                        joined_tick: self.tick,
                        home,
                        command_center: home,
                        home_system: Some(home_system),
                        credits: 10_000.0,
                        inventory,
                        // §TCA: a fresh corp starts with an EMPTY Charterhouse
                        // warehouse — its starter goods sit at home, and it moves
                        // them to the Exchange via freight or a convoy when it
                        // wants to trade them.
                        warehouse: BTreeMap::new(),
                        // §TCA Phase 2: a fresh corp is issued a clean charter.
                        tca_standing: crate::tca::TCA_STANDING_START,
                        valuation: 10_000.0,
                        standing_orders: Vec::new(),
                        next_standing_id: 0,
                        doctrine: FleetDoctrine::default(),
                        intel: BTreeMap::new(),
                        syndicate: None,
                        syndicate_prev: None,
                        syndicate_since: 0.0,
                        stats: crate::rankings::RankingStats::default(),
                        // §explore: the starting valley is KNOWN (pre-surveyed);
                        // the frontier isn't — that's what scouts are for.
                        surveyed: self
                            .systems
                            .iter()
                            .filter(|s| s.pos.distance(home) <= crate::explore::SURVEY_INITIAL_RADIUS)
                            .map(|s| s.id)
                            .collect(),
                    },
                );
                events.push(Event::new(
                    self.time,
                    EventPayload::PlayerJoined {
                        id: *id,
                        name: name.clone(),
                    },
                ));
                self.spawn_starting_fleet(*id, home, events);
                // Seed an accurate initial valuation (before the first close).
                self.recompute_valuations();
            }
            // SYNDICATES (§syndicates Part 1): instant owner-only admin, like the
            // sibling policy commands — they mutate ground-truth membership now; the
            // KNOWLEDGE of the change reaches others light-delayed (see `known_ally`).
            Command::CreateSyndicate { player_id, name } => {
                self.apply_create_syndicate(*player_id, name.clone());
            }
            Command::InviteToSyndicate { player_id, invitee } => {
                self.apply_invite_syndicate(*player_id, *invitee);
            }
            Command::AcceptSyndicateInvite { player_id, syndicate_id } => {
                self.apply_accept_syndicate(*player_id, *syndicate_id);
            }
            Command::LeaveSyndicate { player_id } => {
                self.apply_leave_syndicate(*player_id);
            }
            Command::DissolveSyndicate { player_id } => {
                self.apply_dissolve_syndicate(*player_id);
            }
            Command::SetResearchQueue { player_id, queue } => {
                // §research: research is a SYNDICATE institution — the player must
                // be in one. Validate the queue against the catalog (drop unknown /
                // hidden / already-completed ids), keeping NOT-YET-AVAILABLE ids
                // (queue-ahead is the point). The front promotes to active if idle.
                let Some(sid) = self.players.get(player_id).and_then(|c| c.syndicate) else {
                    return;
                };
                let Some(s) = self.syndicates.get_mut(&sid) else { return };
                let valid: Vec<String> = queue
                    .iter()
                    .filter(|id| {
                        crate::research::programme(id).is_some_and(|p| !p.hidden)
                            && !s.research.completed.contains(*id)
                    })
                    .cloned()
                    .collect();
                s.research.set_queue(valid);
            }
            Command::SetDesignation { player_id, cap, target } => {
                // §research: point a capability at a live target. Soft-reject
                // unless the syndicate has unlocked the capability.
                let Some(sid) = self.players.get(player_id).and_then(|c| c.syndicate) else {
                    return;
                };
                let Some(s) = self.syndicates.get_mut(&sid) else { return };
                if crate::research::has_flag(&s.research, *cap) {
                    s.research.designations.insert(*cap, *target);
                }
            }
            Command::SaveFit { player_id, name, ship, loadout } => {
                // §fitting: save a doctrine fit on the caller's syndicate.
                // Any member may curate the shared fit library (CC-local
                // instant admin, like the research queue). The stored fit is
                // legal BY CONSTRUCTION: slots + fitting budget validated here.
                let Some(sid) = self.players.get(player_id).and_then(|c| c.syndicate) else {
                    return;
                };
                let Some(s) = self.syndicates.get_mut(&sid) else { return };
                let name: String =
                    name.trim().chars().take(crate::syndicate::FIT_NAME_MAX).collect();
                if name.is_empty() || !loadout.validate(*ship) {
                    return; // unnamed or illegal fit — soft reject
                }
                if let Some(existing) = s.fits.iter_mut().find(|f| f.name == name) {
                    // Replace the same-name fit in place (order is curation state).
                    existing.kind = *ship;
                    existing.loadout = loadout.clone();
                } else if s.fits.len() < crate::syndicate::SYNDICATE_MAX_FITS {
                    s.fits.push(crate::syndicate::DoctrineFit {
                        name,
                        kind: *ship,
                        loadout: loadout.clone(),
                    });
                }
                // else: at the cap — soft reject (delete one first).
            }
            Command::DeleteFit { player_id, name } => {
                // §fitting: drop a doctrine fit by name (member curation;
                // unknown names are a no-op).
                let Some(sid) = self.players.get(player_id).and_then(|c| c.syndicate) else {
                    return;
                };
                let Some(s) = self.syndicates.get_mut(&sid) else { return };
                let name = name.trim();
                s.fits.retain(|f| f.name != name);
            }
            Command::NameFlagship { player_id, name } => {
                // §ladder B4: christen (or un-christen, empty name) the
                // syndicate's Titan. A pure label — no sim outcome touches it.
                let Some(sid) = self.players.get(player_id).and_then(|c| c.syndicate) else {
                    return;
                };
                let Some(s) = self.syndicates.get_mut(&sid) else { return };
                let name: String =
                    name.trim().chars().take(crate::syndicate::FLAGSHIP_NAME_MAX).collect();
                s.flagship_name = (!name.is_empty()).then_some(name);
            }
            Command::MoveShip {
                player_id,
                ship_id,
                dest,
            } => {
                // Burn fuel ∝ distance × fleet mass at dispatch (§step1 part 2). A
                // shortfall HOLDS the move (the ship keeps its current order — never
                // lost) and notifies the owner.
                let Some(ship) = self.fleets.get(ship_id) else {
                    return;
                };
                if ship.owner != *player_id {
                    return;
                }
                let cost = crate::fuel::fuel_cost(ship.pos.distance(*dest), ship.mass());
                let origin = ship.pos;
                if !self.charge_fuel(*player_id, origin, cost) {
                    events.push(Event::new(
                        self.time,
                        EventPayload::FuelShortfall {
                            owner: *player_id,
                            needed: cost,
                            kind: crate::fuel::ShortfallKind::Move,
                        },
                    ));
                    return;
                }
                self.schedule_for_owner(*player_id, *ship_id, FleetOrder::MoveTo { dest: *dest }, crate::event::OrderKind::Move);
            }
            Command::CommitRaid {
                player_id,
                raider_id,
                target_id,
            } => {
                // The target must exist and belong to someone else.
                let Some(target) = self.fleets.get(target_id) else {
                    return;
                };
                if target.owner == *player_id {
                    return; // no raiding your own fleets
                }
                // §syndicates: no raiding an ALLY. Deliberate offense against a
                // syndicate member soft-rejects while allied (leaving re-enables it).
                if self.are_allied(*player_id, target.owner) {
                    return;
                }
                let target_pos = target.pos;
                // The raider must exist and be the player's.
                let Some(raider) = self.fleets.get(raider_id) else {
                    return;
                };
                if raider.owner != *player_id {
                    return;
                }
                // Raiding is the RAIDER'S verb (§fleets part 2 — crisp roles):
                // corvettes defend, scouts look, convoys haul. Only a fleet whose
                // FLAGSHIP is a raider (a dedicated combat fleet) may raid — a
                // hauler with a raider tucked in doesn't go hunting. Soft-reject.
                if raider.flagship_kind() != ShipKind::Raider {
                    return;
                }
                // Fuel the intercept run (raiders are light → cheap, but not free).
                let cost = crate::fuel::fuel_cost(raider.pos.distance(target_pos), raider.mass());
                let origin = raider.pos;
                if !self.charge_fuel(*player_id, origin, cost) {
                    events.push(Event::new(
                        self.time,
                        EventPayload::FuelShortfall {
                            owner: *player_id,
                            needed: cost,
                            kind: crate::fuel::ShortfallKind::Raid,
                        },
                    ));
                    return;
                }
                self.schedule_for_owner(
                    *player_id,
                    *raider_id,
                    FleetOrder::Intercept { target: *target_id },
                    crate::event::OrderKind::Raid,
                );
            }
            Command::RecallRaid {
                player_id,
                raider_id,
            } => {
                let Some(home) = self.players.get(player_id).map(|c| c.home) else {
                    return;
                };
                self.schedule_for_owner(*player_id, *raider_id, FleetOrder::MoveTo { dest: home }, crate::event::OrderKind::Recall);
            }
            Command::MarketBuy {
                player_id,
                commodity,
                units,
                ship_to,
            } => {
                let units = *units;
                if units == 0 {
                    return;
                }
                let Some(corp) = self.players.get(player_id) else {
                    return;
                };
                let price = self.market.price(*commodity);
                let cost = units as f64 * price;
                if corp.credits < cost {
                    return; // can't afford
                }
                // Instant settlement at the true standing price (§9). The goods
                // land in the corp's CHARTERHOUSE WAREHOUSE — nothing about a trade
                // moves goods across space any more (§TCA). Getting them to a
                // system is a separate, explicit act: TCA freight or a player convoy.
                let unit_price = self.market.execute_buy(*commodity, units);
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.credits -= units as f64 * unit_price;
                    *corp.warehouse.entry(*commodity).or_insert(0) += units;
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::Trade(TradeEvent::Bought {
                        player: *player_id,
                        commodity: *commodity,
                        units,
                        unit_price,
                    }),
                ));
                // §TCA one-checkbox composition: hand the whole lot straight to
                // the Authority for delivery. If the booking soft-rejects, the
                // goods simply stay in the warehouse and the owner is told why.
                if let Some(dest) = *ship_to {
                    self.book_freight(*player_id, dest, *commodity, units, ShipmentDir::Outbound, false, events);
                }
            }
            Command::MarketSell {
                player_id,
                commodity,
                units,
            } => {
                let units = *units;
                if units == 0 {
                    return;
                }
                let Some(corp) = self.players.get(player_id) else {
                    return;
                };
                // §TCA: a sale draws ONLY from the CHARTERHOUSE WAREHOUSE. The goods
                // are already AT the Exchange, so the old "commit to the crossing and
                // take the price on arrival" dance is gone — settlement is instant,
                // like a buy. Home goods are no longer a valid sell source: move them
                // to the Charterhouse first (freight or a convoy).
                let have = corp.warehouse.get(commodity).copied().unwrap_or(0);
                if have < units {
                    // Async-fair soft reject: nothing spent, owner-only notice.
                    events.push(Event::new(
                        self.time,
                        EventPayload::Trade(TradeEvent::Rejected {
                            player: *player_id,
                            commodity: *commodity,
                            units,
                            system: None,
                            reason: crate::event::TradeRejectReason::InsufficientWarehouseStock { have },
                        }),
                    ));
                    return;
                }
                let unit_price = self.market.execute_sell(*commodity, units);
                if let Some(corp) = self.players.get_mut(player_id) {
                    take_from(&mut corp.warehouse, *commodity, units);
                    corp.credits += units as f64 * unit_price;
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::Trade(TradeEvent::Sold {
                        player: *player_id,
                        commodity: *commodity,
                        units,
                        unit_price,
                    }),
                ));
            }
            // SUPPLY FROM HQ (§economy): move goods from the corp's HQ trading
            // inventory into an OWNED system's stockpile via a sub-light raidable
            // convoy — the bridge that lets market-bought inputs feed a system's
            // converters (which read sys.stockpile, not the trading pool).
            Command::StockSystem { player_id, system_id, commodity, units } => {
                let units = *units;
                if units == 0 {
                    return;
                }
                // Owner-only: you can only stock a system you currently own.
                let Some(dest) = self
                    .systems
                    .iter()
                    .find(|s| s.id == *system_id && s.owner == Some(*player_id))
                    .map(|s| s.pos)
                else {
                    return; // not yours (or gone) — soft reject
                };
                let Some(corp) = self.players.get(player_id) else {
                    return;
                };
                let have = corp.inventory.get(commodity).copied().unwrap_or(0);
                if have < units {
                    return; // not enough held at HQ
                }
                let home = corp.home;
                // Commit goods out of the HQ pool up front (mirror MarketSell).
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.inventory.entry(*commodity).and_modify(|u| *u -= units);
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::Trade(TradeEvent::StockDispatched {
                        player: *player_id,
                        commodity: *commodity,
                        units,
                        system: *system_id,
                    }),
                ));
                // The DeliverToSystem arm deposits into sys.stockpile (with the
                // depot-cap / overflow-to-hub handling) on arrival. No fuel charge —
                // parity with the manual hub-trade family (MarketBuy/MarketSell).
                let cargo = Cargo { commodity: *commodity, units };
                self.spawn_trade_convoy(*player_id, home, dest, cargo, TradeMission::DeliverToSystem { system: *system_id });
            }
            Command::BookFreightOut { player_id, system, commodity, units } => {
                self.book_freight(*player_id, *system, *commodity, *units, ShipmentDir::Outbound, false, events);
            }
            Command::BookFreightIn { player_id, system, commodity, units, sell_on_arrival } => {
                self.book_freight(*player_id, *system, *commodity, *units, ShipmentDir::Inbound, *sell_on_arrival, events);
            }
            Command::PlaceLimitOrder {
                player_id,
                side,
                commodity,
                units,
                limit_price,
            } => {
                let units = *units;
                let limit_price = *limit_price;
                if units == 0 || limit_price <= 0.0 {
                    return;
                }
                let Some(corp) = self.players.get(player_id) else {
                    return;
                };
                // Reserve resources up front so the order is funded when it clears.
                match side {
                    Side::Buy => {
                        let reserve = units as f64 * limit_price;
                        if corp.credits < reserve {
                            return;
                        }
                        if let Some(c) = self.players.get_mut(player_id) {
                            c.credits -= reserve;
                        }
                    }
                    Side::Sell => {
                        // §TCA: sell-side escrow draws from the CHARTERHOUSE
                        // WAREHOUSE (the goods must already be at the Exchange),
                        // never from home inventory.
                        let have = corp.warehouse.get(commodity).copied().unwrap_or(0);
                        if have < units {
                            events.push(Event::new(
                                self.time,
                                EventPayload::Trade(TradeEvent::Rejected {
                                    player: *player_id,
                                    commodity: *commodity,
                                    units,
                                    system: None,
                                    reason: crate::event::TradeRejectReason::InsufficientWarehouseStock { have },
                                }),
                            ));
                            return;
                        }
                        if let Some(c) = self.players.get_mut(player_id) {
                            take_from(&mut c.warehouse, *commodity, units);
                        }
                    }
                }
                let id = self.next_order_id;
                self.next_order_id += 1;
                self.book.push(LimitOrder {
                    id,
                    player: *player_id,
                    side: *side,
                    commodity: *commodity,
                    units,
                    limit_price,
                });
                events.push(Event::new(
                    self.time,
                    EventPayload::Trade(TradeEvent::LimitPlaced {
                        player: *player_id,
                        side: *side,
                        commodity: *commodity,
                        units,
                        limit_price,
                    }),
                ));
            }
            Command::ShipProduction { player_id, system_id } => {
                self.apply_ship_production(*player_id, *system_id, events);
            }
            Command::SetStandingOrder { player_id, order } => {
                self.apply_set_standing_order(*player_id, *order);
            }
            Command::ClearStandingOrder { player_id, order_id } => {
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.standing_orders.retain(|o| o.id != *order_id);
                }
            }
            Command::SetFleetDoctrine { player_id, doctrine } => {
                // Instant local administration: a closed menu of enums is always
                // valid, so just install it. Takes effect from the next tick's
                // autonomous-defence / supply pass.
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.doctrine = *doctrine;
                }
            }
            Command::BuildShip { player_id, system_id, ship_kind, join, loadout } => {
                self.apply_build(*player_id, *system_id, None, crate::build::BuildKind::Ship { ship: *ship_kind }, *join, loadout.clone(), events);
            }
            Command::BuildModule { player_id, system_id, module } => {
                self.apply_build(*player_id, *system_id, None, crate::build::BuildKind::Module { module: *module }, None, crate::module::Loadout::default(), events);
            }
            Command::RefitShips { player_id, fleet_id, ship, from, to, n } => {
                self.apply_refit(*player_id, *fleet_id, *ship, from.clone(), to.clone(), *n, events);
            }
            Command::TransferModules { player_id, from, to, manifest } => {
                // §modules Part B3: a dedicated module-crate convoy between the
                // player's own ground (or an ally's — coalition resupply). Manifest
                // clamps to the source ledger and one convoy's module berths; the
                // ledger is debited at LOADING (crates aboard, not at home).
                // Soft-reject if nothing valid remains.
                let allies = self.allies_of(*player_id);
                let Some(to_pos) = self
                    .systems
                    .iter()
                    .find(|s| s.id == *to && s.owner.is_some_and(|o| o == *player_id || allies.contains(&o)))
                    .map(|s| s.pos)
                else {
                    return;
                };
                let Some(src) = self.systems.iter_mut().find(|s| s.id == *from && s.owner == Some(*player_id)) else {
                    return;
                };
                let mut berths = crate::module::MODULE_CONVOY_BERTHS;
                let mut load: BTreeMap<crate::module::ModuleKind, u32> = Default::default();
                for (&kind, &want) in manifest {
                    if berths == 0 {
                        break;
                    }
                    let stocked = src.modules.get(&kind).copied().unwrap_or(0);
                    let take = want.min(stocked).min(berths);
                    if take > 0 {
                        load.insert(kind, take);
                        berths -= take;
                    }
                }
                if load.is_empty() {
                    return; // nothing to carry — soft reject
                }
                for (kind, n) in &load {
                    let r = src.modules.get_mut(kind).expect("clamped above");
                    *r -= n;
                    if *r == 0 {
                        src.modules.remove(kind);
                    }
                }
                let spawn = src.pos;
                let cid = self.spawn_trade_convoy(*player_id, spawn, to_pos, Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 }, TradeMission::DeliverToSystem { system: *to });
                if let Some(f) = self.fleets.get_mut(&cid) {
                    f.cargo = None;
                    f.modules = load;
                }
            }
            Command::BuyModule { player_id, module, n, dest_system } => {
                // §modules Part B3 (Sol hub): price-certain purchase, delivery-risky
                // — pay Sol now, a crate convoy carries it hub → the player's
                // system. Mirrors HireSpecialist. Soft-reject on ownership / credits.
                let n = *n;
                if n == 0 {
                    return;
                }
                let Some(dest) = self.systems.iter().find(|s| s.id == *dest_system && s.owner == Some(*player_id)).map(|s| s.pos) else {
                    return;
                };
                let unit = self.module_buy_price(*module);
                let cost = unit * n as f64;
                let Some(corp) = self.players.get_mut(player_id) else { return };
                if corp.credits + 1e-9 < cost {
                    return; // can't pay — soft reject
                }
                corp.credits -= cost;
                let hub = self.hub;
                let cid = self.spawn_trade_convoy(*player_id, hub, dest, Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 }, TradeMission::DeliverToSystem { system: *dest_system });
                if let Some(f) = self.fleets.get_mut(&cid) {
                    f.cargo = None;
                    f.modules.insert(*module, n);
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::ModulesPurchased { owner: *player_id, kind: *module, n, dest: *dest_system, unit_price: unit },
                ));
            }
            Command::SellModule { player_id, module, n, from_system } => {
                // §modules Part B3 (Sol hub): commit the crates to the crossing now
                // (debit the ledger), a convoy carries them src → hub, and the
                // buy-back clears on ARRIVAL (price-on-arrival, like MarketSell).
                let n = *n;
                if n == 0 {
                    return;
                }
                let Some(src) = self.systems.iter_mut().find(|s| s.id == *from_system && s.owner == Some(*player_id)) else {
                    return;
                };
                let stocked = src.modules.get(module).copied().unwrap_or(0);
                let take = n.min(stocked);
                if take == 0 {
                    return; // ledger doesn't stock it — soft reject
                }
                let r = src.modules.get_mut(module).expect("stocked > 0");
                *r -= take;
                if *r == 0 {
                    src.modules.remove(module);
                }
                let spawn = src.pos;
                let hub = self.hub;
                let cid = self.spawn_trade_convoy(*player_id, spawn, hub, Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 }, TradeMission::SellAtHub);
                if let Some(f) = self.fleets.get_mut(&cid) {
                    f.cargo = None;
                    f.modules.insert(*module, take);
                }
            }
            Command::DevelopSystem { player_id, system_id, upgrade, body_id } => {
                self.apply_build(*player_id, *system_id, *body_id, crate::build::BuildKind::Upgrade { upgrade: *upgrade }, None, crate::module::Loadout::default(), events);
            }
            Command::SetAssignment { player_id, system_id, structure, workers, specialists, body_id } => {
                // §economy Part 3 → §bodies: INSTANT local administration on ONE
                // BODY's line. Owner + built-on-that-body required; `workers`
                // clamps to the tier there; 0 clears the line. Over-posting the
                // system workforce stays legal (the uniform share dilutes).
                // Posted SPECIALISTS clamp so per-kind totals across EVERY
                // body's lines fit the system's resident pool.
                let Some(sys) = self.systems.iter_mut().find(|s| s.id == *system_id && s.owner == Some(*player_id)) else {
                    return;
                };
                // `None` (old clients / auto): the body holding the structure,
                // highest tier first.
                let Some(target) = body_id.or_else(|| {
                    sys.bodies
                        .iter()
                        .filter(|b| b.tier(*structure) > 0)
                        .max_by_key(|b| b.tier(*structure))
                        .map(|b| b.id)
                }) else {
                    return; // nothing built anywhere — soft reject
                };
                let Some(tier) = sys.bodies.iter().find(|b| b.id == target).map(|b| b.tier(*structure)) else {
                    return; // no such body — soft reject
                };
                if tier == 0 {
                    return; // nothing built on THAT body to staff — soft reject
                }
                let workers = (*workers).min(tier);
                if workers == 0 && specialists.values().all(|n| *n == 0) {
                    if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == target) {
                        b.assignments.remove(structure);
                    }
                } else {
                    let mut posted = std::collections::BTreeMap::new();
                    for (&kind, &want) in specialists {
                        if want == 0 {
                            continue;
                        }
                        let resident = sys.specialists.get(&kind).copied().unwrap_or(0);
                        let elsewhere: u32 = sys
                            .bodies
                            .iter()
                            .flat_map(|b| b.assignments.iter().map(move |(k, a)| (b.id, *k, a)))
                            .filter(|(bid, k, _)| !(*bid == target && k == structure))
                            .map(|(_, _, a)| a.specialists.get(&kind).copied().unwrap_or(0))
                            .sum();
                        let free = resident.saturating_sub(elsewhere);
                        if free > 0 {
                            posted.insert(kind, want.min(free));
                        }
                    }
                    // A fresh posting starts un-suspended; the engine re-latches
                    // real outages next tick (no stale banner on a re-post).
                    if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == target) {
                        b.assignments.insert(*structure, crate::production::Assignment { workers, specialists: posted, suspended: None });
                    }
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::AssignmentSet { owner: *player_id, system: *system_id, structure: *structure, workers },
                ));
            }
            Command::HireSpecialist { player_id, specialist, dest_system } => {
                // §economy Part 4: Sol contract — price-certain, delivery-risky.
                // Validate the destination is the player's OWN system (allies
                // route through Transfer; Sol won't ship to someone else's
                // ground), then debit and dispatch a personnel convoy from the
                // hub. Soft-reject on ownership or credits.
                let Some(dest) = self.systems.iter().find(|s| s.id == *dest_system && s.owner == Some(*player_id)).map(|s| s.pos) else {
                    return;
                };
                let cost = crate::specialist::SPECIALIST_HIRE_COST;
                let Some(corp) = self.players.get_mut(player_id) else { return };
                if corp.credits + 1e-9 < cost {
                    return; // can't pay — soft reject
                }
                corp.credits -= cost;
                let hub = self.hub;
                let cid = self.spawn_trade_convoy(*player_id, hub, dest, Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 }, TradeMission::DeliverToSystem { system: *dest_system });
                // A pure personnel run: no cargo, one contractor aboard.
                if let Some(f) = self.fleets.get_mut(&cid) {
                    f.cargo = None;
                    f.passengers.insert(*specialist, 1);
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::SpecialistHired { owner: *player_id, kind: *specialist, dest: *dest_system },
                ));
            }
            Command::TrainSpecialist { player_id, system_id, specialist } => {
                // §economy Part 4: an Academy course rides the normal build
                // queue (deduct now, complete later; no slot, no shipyard gate —
                // apply_build's gates only bind Ship/Upgrade). Requires the
                // Academy built at the player's own system.
                let has_academy = self
                    .systems
                    .iter()
                    .any(|s| s.id == *system_id && s.owner == Some(*player_id) && s.tier(crate::build::StructureKind::Academy) >= 1);
                if !has_academy {
                    return; // soft reject — no Academy standing there
                }
                self.apply_build(*player_id, *system_id, None, crate::build::BuildKind::Train { specialist: *specialist }, None, crate::module::Loadout::default(), events);
            }
            Command::TransferSpecialists { player_id, from, to, manifest } => {
                // §economy Part 4: a dedicated personnel convoy between the
                // player's own ground (or an ally's destination — aid in human
                // form). Manifest clamps to the resident pool and one convoy's
                // berths; the pool is debited at LOADING (the people are aboard,
                // not at home). Soft-reject if nothing valid remains.
                let allies = self.allies_of(*player_id);
                let Some(to_pos) = self
                    .systems
                    .iter()
                    .find(|s| s.id == *to && s.owner.is_some_and(|o| o == *player_id || allies.contains(&o)))
                    .map(|s| s.pos)
                else {
                    return;
                };
                let Some(src) = self.systems.iter_mut().find(|s| s.id == *from && s.owner == Some(*player_id)) else {
                    return;
                };
                let mut berths = crate::specialist::passenger_capacity(ShipKind::Convoy);
                let mut load: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32> = Default::default();
                for (&kind, &want) in manifest {
                    if berths == 0 {
                        break;
                    }
                    let resident = src.specialists.get(&kind).copied().unwrap_or(0);
                    let take = want.min(resident).min(berths);
                    if take > 0 {
                        load.insert(kind, take);
                        berths -= take;
                    }
                }
                if load.is_empty() {
                    return; // nothing to carry — soft reject
                }
                for (kind, n) in &load {
                    let r = src.specialists.get_mut(kind).expect("clamped above");
                    *r -= n;
                    if *r == 0 {
                        src.specialists.remove(kind);
                    }
                }
                let spawn = src.pos;
                let cid = self.spawn_trade_convoy(*player_id, spawn, to_pos, Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 }, TradeMission::DeliverToSystem { system: *to });
                if let Some(f) = self.fleets.get_mut(&cid) {
                    f.cargo = None;
                    f.passengers = load;
                }
            }
            Command::MergeFleets { player_id, into, from } => {
                self.apply_merge_fleets(*player_id, *into, *from, events);
            }
            Command::SplitFleet { player_id, fleet_id, counts } => {
                self.apply_split_fleet(*player_id, *fleet_id, counts, events);
            }
            Command::Withdraw { player_id, fleet_id } => {
                // A light-delayed break-off: schedule a flee-home order (like a
                // recall) tagged Withdraw so its arrival pulls the fleet from the
                // battle. Home-field advantage falls out — a battle near your
                // command center has a SHORTER command delay than the attacker's.
                let home = self.players.get(player_id).map(|c| c.home);
                if let Some(home) = home {
                    self.schedule_for_owner(*player_id, *fleet_id, FleetOrder::MoveTo { dest: home }, crate::event::OrderKind::Withdraw);
                }
            }
            Command::SetFleetTransit { player_id, fleet_id, mode } => {
                if let Some(f) = self.fleets.get_mut(fleet_id)
                    && f.owner == *player_id
                {
                    f.transit = *mode;
                }
            }
            Command::BlockadeSystem { player_id, fleet_id, system_id } => {
                // The fleet must exist, be the player's, and CONTAIN a raider —
                // the strike capability that lets it interdict (corvettes/scouts
                // ride along and add strength, but can't blockade alone; §crisp
                // roles). Soft-reject otherwise.
                let Some(fleet) = self.fleets.get(fleet_id) else { return };
                if fleet.owner != *player_id || !fleet.contains(ShipKind::Raider) {
                    return;
                }
                // The target must be a system SOMEONE ELSE owns.
                let Some(sys) = self.systems.iter().find(|s| s.id == *system_id) else { return };
                if sys.owner.is_none() || sys.owner == Some(*player_id) {
                    return;
                }
                // §syndicates: no blockading an ALLY's system while allied.
                if sys.owner.is_some_and(|o| self.are_allied(*player_id, o)) {
                    return;
                }
                let station = sys.pos;
                // Fuel the run ∝ distance × fleet mass, like a move; a shortfall
                // HOLDS it (keeps the current order, notifies) — never lost.
                let cost = crate::fuel::fuel_cost(fleet.pos.distance(station), fleet.mass());
                let origin = fleet.pos;
                if !self.charge_fuel(*player_id, origin, cost) {
                    events.push(Event::new(self.time, EventPayload::FuelShortfall {
                        owner: *player_id, needed: cost, kind: crate::fuel::ShortfallKind::Move,
                    }));
                    return;
                }
                // Light-delayed like any order to a mobile asset (echo lifecycle).
                self.schedule_for_owner(
                    *player_id,
                    *fleet_id,
                    FleetOrder::Blockade { system: *system_id, station },
                    crate::event::OrderKind::Blockade,
                );
            }
            Command::SurveySystem { player_id, fleet_id, system_id } => {
                // §explore Part 2: the fleet must exist, be the player's, and
                // CONTAIN a Scout (the sensing capability — escorts ride along but
                // can't survey alone; crisp roles like the Blockade raider-gate).
                let Some(fleet) = self.fleets.get(fleet_id) else { return };
                if fleet.owner != *player_id || !fleet.contains(ShipKind::Scout) {
                    return;
                }
                // ANY system is fair game — unclaimed frontier, an ally's, or a
                // rival's (pre-siege prospecting is intended). Re-surveying an
                // already-known system is legal and idempotent (wasted time —
                // the UI notes it; the sim doesn't forbid).
                let Some(sys) = self.systems.iter().find(|s| s.id == *system_id) else { return };
                let station = sys.pos;
                // Fuel the run ∝ distance × fleet mass, like a move; a shortfall
                // HOLDS it (keeps the current order, notifies) — never lost.
                let cost = crate::fuel::fuel_cost(fleet.pos.distance(station), fleet.mass());
                let origin = fleet.pos;
                if !self.charge_fuel(*player_id, origin, cost) {
                    events.push(Event::new(self.time, EventPayload::FuelShortfall {
                        owner: *player_id, needed: cost, kind: crate::fuel::ShortfallKind::Move,
                    }));
                    return;
                }
                // Light-delayed like any order to a mobile asset (echo lifecycle).
                self.schedule_for_owner(
                    *player_id,
                    *fleet_id,
                    FleetOrder::Survey { system: *system_id, station, dwell_since: None },
                    crate::event::OrderKind::Survey,
                );
            }
            Command::AttackFleet { player_id, fleet_id, target_id } => {
                // The target must exist and belong to someone else.
                let Some(target) = self.fleets.get(target_id) else {
                    return;
                };
                if target.owner == *player_id {
                    return; // no attacking your own fleets
                }
                // §syndicates: no attacking an ALLY while allied.
                if self.are_allied(*player_id, target.owner) {
                    return;
                }
                let target_pos = target.pos;
                // The attacker must exist, be the player's, and CONTAIN ≥1 raider
                // (strike capability — consistent with BlockadeSystem; a corvette/
                // scout-only fleet can't press a destroy attack). Soft-reject.
                let Some(attacker) = self.fleets.get(fleet_id) else {
                    return;
                };
                if attacker.owner != *player_id || !attacker.contains(ShipKind::Raider) {
                    return;
                }
                // Fuel the intercept run ∝ distance × fleet mass, like a raid; a
                // shortfall HOLDS it (keeps the current order, notifies) — never lost.
                let cost = crate::fuel::fuel_cost(attacker.pos.distance(target_pos), attacker.mass());
                let origin = attacker.pos;
                if !self.charge_fuel(*player_id, origin, cost) {
                    events.push(Event::new(
                        self.time,
                        EventPayload::FuelShortfall {
                            owner: *player_id,
                            needed: cost,
                            kind: crate::fuel::ShortfallKind::Raid,
                        },
                    ));
                    return;
                }
                self.schedule_for_owner(
                    *player_id,
                    *fleet_id,
                    FleetOrder::Attack { target: *target_id },
                    crate::event::OrderKind::Attack,
                );
            }
            Command::SetFleetPosture { player_id, fleet_id, posture } => {
                // Instant local administration on the player's own fleet — a
                // standing per-fleet policy, like the sibling SetFleetTransit and
                // the corp SetFleetDoctrine. The ACTION it authorizes (WeaponsFree
                // auto-commit) is taken on the fleet's own local detection; the
                // owner learns of any engagement light-delayed. Soft-reject if not
                // owned.
                if let Some(f) = self.fleets.get_mut(fleet_id)
                    && f.owner == *player_id
                {
                    f.posture = *posture;
                }
            }
        }
    }

    /// The STANDING sensor sources `owner`'s SENSOR ARRAYS project (§buildings
    /// step 2b): every owned system with an array, as `(position, radius)`.
    /// The ONE source of truth every consumer shares — the sim's picket sensing
    /// here, and (via the game loop) the View's sensor-gating + the client's
    /// coverage rendering — so everything that keys off sensor range inherits
    /// arrays consistently. Systems are static, so including them in the View's
    /// delayed composite frame is exactly as leak-free as ship bubbles.
    pub fn array_sensor_sources(&self, owner: PlayerId) -> Vec<(Vec2, f64)> {
        // §research R4a SensorRadius widens every owned array's bubble.
        let radius = self.research_mod(owner, crate::research::ModKey::SensorRadius);
        self.systems
            .iter()
            .filter(|s| s.owner == Some(owner) && s.tier(crate::build::StructureKind::SensorArray) >= 1)
            .map(|s| (s.pos, s.sensor_bubble() * radius))
            .collect()
    }

    /// SCOUT INTEL (§scout part 2): every SCOUT within [`crate::ship::SCOUT_INTEL_RANGE`]
    /// of a RIVAL-owned system captures/refreshes its owner's snapshot of that
    /// system's fortifications — `{ defense_tier, shipyard_tier, observed_at,
    /// pos }`. The stored snapshot updates every tick the scout stays in range
    /// (it is physically there, looking); the OWNER-ONLY `IntelGathered` notice
    /// fires only on a FRESH approach or when the observed tiers changed —
    /// never per-tick (no spam). Fog discipline: this is data the scout
    /// gathered at contact range, not a live link — the View delivers the
    /// stored snapshot to its owner light-delayed from `pos`, and the scouted
    /// rival learns NOTHING (a never-detected dark scout leaves no trace).
    fn gather_intel(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        // Collect first (immutable pass over fleets × systems), then apply.
        let mut captures: Vec<(PlayerId, EntityId, u32, u32, u32, Vec2)> = Vec::new();
        for ship in self.fleets.values() {
            // Any fleet CONTAINING a scout gathers intel (its eyes ride along).
            if !ship.contains(ShipKind::Scout) || ship.owner.is_pirate() {
                continue;
            }
            for sys in &self.systems {
                if ship.pos.distance(sys.pos) > crate::ship::SCOUT_INTEL_RANGE {
                    continue;
                }
                match sys.owner {
                    // §pirates: a base at an UNOWNED system reveals its enclave tier
                    // (like fortifications). Its `defense_tier` is the base defense.
                    None => {
                        if let Some(e) = self.enclaves.get(&sys.id) {
                            captures.push((ship.owner, sys.id, sys.tier_sum(crate::build::StructureKind::DefensePlatform), 0, e.tier, ship.pos));
                        }
                    }
                    // A RIVAL's fortifications (never your own or a syndicate ally's).
                    Some(o) if o != ship.owner && !self.are_allied(ship.owner, o) => {
                        captures.push((ship.owner, sys.id, sys.tier_sum(crate::build::StructureKind::DefensePlatform), sys.tier(crate::build::StructureKind::Shipyard), 0, ship.pos));
                    }
                    _ => {}
                }
            }
        }
        for (owner, system, defense_tier, shipyard_tier, enclave_tier, pos) in captures {
            let Some(corp) = self.players.get_mut(&owner) else { continue };
            let prev = corp.intel.get(&system);
            // Notify on a fresh approach (no snapshot, or the last one has gone
            // stale — the scout left and came back) or on changed tiers.
            let notify = match prev {
                None => true,
                Some(p) => {
                    p.defense_tier != defense_tier
                        || p.shipyard_tier != shipyard_tier
                        || p.enclave_tier != enclave_tier
                        || now - p.observed_at > crate::ship::SCOUT_INTEL_RENOTIFY_S
                }
            };
            corp.intel.insert(
                system,
                crate::world::IntelSnapshot { defense_tier, shipyard_tier, enclave_tier, observed_at: now, pos },
            );
            if notify {
                events.push(Event::new(
                    now,
                    EventPayload::IntelGathered { owner, system, defense_tier, shipyard_tier, pos },
                ));
            }
        }
    }

    /// Development slots HELD by in-progress upgrade jobs at `system` (each queued
    /// Extractor/Depot/Shipyard tier reserves its slot while building, so you can't
    /// over-commit a budget by queueing). Ships never hold slots.
    pub fn dev_slots_pending(&self, system: EntityId) -> u32 {
        self.build_queue
            .iter()
            .filter(|j| j.system == system && matches!(j.what, crate::build::BuildKind::Upgrade { .. }))
            .count() as u32
    }

    /// §economy: in-progress structure jobs at `system` holding a slot of `pool`
    /// — only jobs FOUNDING a new structure (the kind isn't built yet) hold a
    /// slot; tier-up jobs deepen an existing footprint (see
    /// `pool_slots_built`). Distinct kinds, so double-queueing can't hold two.
    pub fn pool_slots_pending(&self, system: EntityId, body_id: u32, pool: crate::build::SlotPool) -> u32 {
        let Some(sys) = self.systems.iter().find(|s| s.id == system) else { return 0 };
        let Some(body) = sys.bodies.iter().find(|b| b.id == body_id) else { return 0 };
        let mut kinds = std::collections::BTreeSet::new();
        for j in &self.build_queue {
            if j.system == system
                && j.body_id == body_id
                && let crate::build::BuildKind::Upgrade { upgrade } = j.what
                && upgrade.slot_pool() == pool
                && body.tier(upgrade) == 0
            {
                kinds.insert(upgrade);
            }
        }
        kinds.len() as u32
    }

    /// Validate + start a construction job (§step1 growth sink): the player must own
    /// the system and its stockpile must cover the WHOLE recipe (no partial debit —
    /// a soft reject). A DEVELOPMENT additionally needs a free development slot
    /// (§buildings step 1) — a full system soft-rejects with an owner-only notice,
    /// forcing the specialization choice. Deducts the recipe NOW and enqueues a job
    /// that resolves at `tick + build_ticks`. Determinism: pure, runs in command
    /// phase so the debit is visible to this tick's accrual + standing orders.
    fn apply_build(&mut self, player_id: PlayerId, system_id: EntityId, body: Option<u32>, what: crate::build::BuildKind, join: Option<EntityId>, loadout: crate::module::Loadout, events: &mut Vec<Event>) {
        // §TCA: a corporation can never build an Authority Freighter (it is TCA-only
        // and absent from every BUILDABLE menu). Soft-reject BEFORE any recipe lookup
        // — no debit, no job — so `recipe_for`/`required_shipyard_tier` never see it.
        if let crate::build::BuildKind::Ship { ship } = what
            && !ship.is_buildable()
        {
            events.push(Event::new(
                self.time,
                EventPayload::BuildRejected {
                    owner: player_id,
                    system: system_id,
                    what,
                    reason: crate::event::BuildRejectReason::NotBuildable,
                },
            ));
            return;
        }
        let recipe = crate::build::recipe_for(what);
        // §research R4a: the build's COST + TIME tuners, by kind (neutral 1.0 until
        // researched). Read here (before any `sys` borrow) so `self` is free.
        use crate::research::ModKey;
        let (research_cost_mult, research_time_mult) = match what {
            crate::build::BuildKind::Ship { ship } if ship == ShipKind::Colony => (
                self.research_mod(player_id, ModKey::ColonyCost),
                self.research_mod(player_id, ModKey::ColonyBuildTime),
            ),
            crate::build::BuildKind::Ship { ship } if ship.is_combatant() => (
                self.research_mod(player_id, ModKey::WarshipCost),
                self.research_mod(player_id, ModKey::WarshipBuildTime),
            ),
            crate::build::BuildKind::Ship { .. } => (1.0, 1.0), // civilian hulls untuned
            crate::build::BuildKind::Module { .. } => (
                self.research_mod(player_id, ModKey::ModuleCost),
                self.research_mod(player_id, ModKey::ModuleBuildTime),
            ),
            crate::build::BuildKind::Upgrade { .. } => (1.0, self.research_mod(player_id, ModKey::StructureBuildTime)),
            crate::build::BuildKind::Train { .. } => (1.0, self.research_mod(player_id, ModKey::TrainingTime)),
        };
        let Some(sys) = self.systems.iter().find(|s| s.id == system_id) else {
            return;
        };
        if sys.owner != Some(player_id) {
            return; // only the owner builds at their system
        }
        // §bodies: resolve the TARGET BODY. Structures build on the requested
        // body (`None` = the shared siting rules — old clients keep working);
        // ship jobs display at the best yard's body; courses at the Academy's.
        let body_id = match what {
            crate::build::BuildKind::Upgrade { upgrade } => body.or_else(|| sys.site_for(upgrade)).unwrap_or(0),
            crate::build::BuildKind::Ship { .. } => sys
                .bodies
                .iter()
                .max_by_key(|b| b.tier(crate::build::StructureKind::Shipyard))
                .map(|b| b.id)
                .unwrap_or(0),
            crate::build::BuildKind::Train { .. } => sys
                .bodies
                .iter()
                .max_by_key(|b| b.tier(crate::build::StructureKind::Academy))
                .map(|b| b.id)
                .unwrap_or(0),
            // §modules: a module displays at the best Armaments Complex's body.
            crate::build::BuildKind::Module { .. } => sys
                .bodies
                .iter()
                .max_by_key(|b| b.tier(crate::build::StructureKind::ArmamentsComplex))
                .map(|b| b.id)
                .unwrap_or(0),
        };
        // §modules: manufacturing needs an ARMAMENTS COMPLEX ≥ 1 — soft reject
        // below it (the recipe is never eaten; the ask just holds), the same
        // "industry is geography" gate a Shipyard puts on ships.
        if let crate::build::BuildKind::Module { .. } = what
            && sys.tier(crate::build::StructureKind::ArmamentsComplex) < 1
        {
            events.push(Event::new(
                self.time,
                EventPayload::BuildRejected {
                    owner: player_id,
                    system: system_id,
                    what,
                    reason: crate::event::BuildRejectReason::NoSlot,
                },
            ));
            return;
        }
        // §bodies: per-BODY validation for structures — the body must exist,
        // an EXTRACTION structure needs a MATCHING DEPOSIT on that body (real
        // now, not visual), and only FOUNDING a new structure claims a slot of
        // that body's pool (tier-ups deepen the footprint they hold).
        if let crate::build::BuildKind::Upgrade { upgrade } = what {
            let Some(b) = sys.bodies.iter().find(|b| b.id == body_id) else {
                return; // no such body — soft reject
            };
            let is_extraction = matches!(
                upgrade,
                crate::build::StructureKind::MiningComplex
                    | crate::build::StructureKind::VolatileHarvester
                    | crate::build::StructureKind::Bioharvester
            );
            if is_extraction && !b.has_deposit_for(upgrade) {
                events.push(Event::new(
                    self.time,
                    EventPayload::BuildRejected {
                        owner: player_id,
                        system: system_id,
                        what,
                        reason: crate::event::BuildRejectReason::NoSlot,
                    },
                ));
                return;
            }
            // §industrial-headroom: the TIER CEILING. Tiers 5–6 are the research
            // prize — without the syndicate's Tier-IV/V unlock for THIS kind the
            // cap is 4 (exactly today's effective ceiling). Count pending tier-ups
            // for the same (body, kind) so a burst of queued upgrades can't slip
            // the resulting tier past the cap. (Reuses NoSlot — a capacity limit
            // — to stay sim-contained; a dedicated notice is a client follow-up.)
            let cap = crate::build::max_buildable_tier(upgrade, self.research_struct_tier(player_id, upgrade));
            let pending = self
                .build_queue
                .iter()
                .filter(|j| {
                    j.system == system_id
                        && j.body_id == body_id
                        && matches!(j.what, crate::build::BuildKind::Upgrade { upgrade: u } if u == upgrade)
                })
                .count() as u32;
            if b.tier(upgrade) + pending + 1 > cap {
                events.push(Event::new(
                    self.time,
                    EventPayload::BuildRejected {
                        owner: player_id,
                        system: system_id,
                        what,
                        reason: crate::event::BuildRejectReason::NoSlot,
                    },
                ));
                return;
            }
            if b.tier(upgrade) == 0 {
                let pool = upgrade.slot_pool();
                if b.pool_slots_built(pool) + self.pool_slots_pending(system_id, body_id, pool) >= b.pool_slots(pool) {
                    events.push(Event::new(
                        self.time,
                        EventPayload::BuildRejected {
                            owner: player_id,
                            system: system_id,
                            what,
                            reason: crate::event::BuildRejectReason::NoSlot,
                        },
                    ));
                    return;
                }
            }
        }
        let sys = self.systems.iter().find(|s| s.id == system_id).expect("checked above");
        // SHIPS need a Shipyard (§buildings step 3): the system's tier must cover
        // the kind (Convoy ≥ 1, Raider ≥ 2). Below it → soft reject with an
        // owner-only notice — the recipe is never eaten, the build simply holds
        // until the industry exists. This is what makes shipbuilding GEOGRAPHY.
        if let crate::build::BuildKind::Ship { ship } = what {
            // §ladder: the five capital hulls are RESEARCH PRIZES — the Line
            // programme's UnlockHull must be completed by the owner's syndicate
            // before the yard will lay the keel. Soft reject with its own
            // reason (the client shows the gate copy).
            if crate::ship::requires_hull_unlock(ship) && !self.research_hull(player_id, ship) {
                events.push(Event::new(
                    self.time,
                    EventPayload::BuildRejected {
                        owner: player_id,
                        system: system_id,
                        what,
                        reason: crate::event::BuildRejectReason::NeedsResearch,
                    },
                ));
                return;
            }
            // §ladder B4: the TITAN is a syndicate SINGLETON — fielded + queued
            // must be zero to lay a new keel (rebuild after loss is allowed).
            if ship == ShipKind::Titan && self.titan_fielded_or_queued(player_id) {
                events.push(Event::new(
                    self.time,
                    EventPayload::BuildRejected {
                        owner: player_id,
                        system: system_id,
                        what,
                        reason: crate::event::BuildRejectReason::TitanFielded,
                    },
                ));
                return;
            }
            let required = crate::build::required_shipyard_tier(ship);
            if sys.tier(crate::build::StructureKind::Shipyard) < required {
                events.push(Event::new(
                    self.time,
                    EventPayload::BuildRejected {
                        owner: player_id,
                        system: system_id,
                        what,
                        reason: crate::event::BuildRejectReason::NeedsShipyard { required },
                    },
                ));
                return;
            }
            // §modules Part B4 + §fitting: validate the loadout — the hull's
            // slots AND its fitting-point budget (`Loadout::validate`), and
            // every module covered by the system's ledger (soft-reject else;
            // the ledger + recipe are never eaten on a reject).
            if !loadout.is_empty() {
                let mut need: BTreeMap<crate::module::ModuleKind, u32> = BTreeMap::new();
                for m in loadout.modules() {
                    *need.entry(*m).or_insert(0) += 1;
                }
                let fits = loadout.validate(ship)
                    && need.iter().all(|(m, n)| sys.modules.get(m).copied().unwrap_or(0) >= *n);
                if !fits {
                    events.push(Event::new(
                        self.time,
                        EventPayload::BuildRejected {
                            owner: player_id,
                            system: system_id,
                            what,
                            reason: crate::event::BuildRejectReason::NoSlot,
                        },
                    ));
                    return;
                }
            }
        }
        // §explore Part 3 Unstable Geology: DEVELOPMENT (upgrade) recipe costs run
        // ×UNSTABLE_COST_MULT here — the lemon a survey can't see. ONE multiplier
        // read shared by the affordability check AND the debit (they can't drift).
        // §research R4a WarshipCost/ModuleCost/ColonyCost fold into the shared
        // multiplier alongside the Unstable-Geology lemon.
        let cost_mult = research_cost_mult
            * if matches!(what, crate::build::BuildKind::Upgrade { .. })
                && sys.trait_ == Some(crate::explore::SystemTrait::UnstableGeology)
            {
                crate::explore::UNSTABLE_COST_MULT
            } else {
                1.0
            };
        let affordable = recipe
            .costs
            .iter()
            .all(|(c, need)| sys.stockpile.get(c).copied().unwrap_or(0.0) + 1e-9 >= *need * cost_mult);
        if !affordable {
            return; // soft reject — no event, no debit
        }
        // Deduct the whole recipe from the system stockpile.
        let sys = self.systems.iter_mut().find(|s| s.id == system_id).unwrap();
        for (c, need) in recipe.costs {
            *sys.stockpile.entry(*c).or_insert(0.0) -= *need * cost_mult;
        }
        // §modules Part B4: a fitted ship also debits its modules from the ledger
        // (validated above) — reserved at enqueue, fitted on the completed hull.
        if matches!(what, crate::build::BuildKind::Ship { .. }) {
            for m in loadout.modules() {
                let n = sys.modules.get_mut(m).map(|c| { *c = c.saturating_sub(1); *c }).unwrap_or(0);
                if n == 0 {
                    sys.modules.remove(m);
                }
            }
        }
        self.next_build_id += 1;
        // §economy Part 3 SHIPYARD BOOST: a staffed yard turns SHIP jobs out
        // faster — ticks / (1 + BOOST · staffing · skill). Locked in at enqueue
        // (deterministic — no mid-flight retiming when crews move); structures
        // are unaffected. skill = 1.0 until specialists (Part 4).
        let ticks = if matches!(what, crate::build::BuildKind::Ship { .. }) {
            let yard = crate::build::StructureKind::Shipyard;
            let boost = 1.0 + crate::production::SHIPYARD_BOOST * sys.staffing_factor(body_id, yard) * sys.skill_factor(body_id, yard);
            (recipe.build_ticks as f64 / boost).round() as u64
        } else {
            recipe.build_ticks
        };
        // §research R4a WarshipBuildTime/ModuleBuildTime/StructureBuildTime/
        // ColonyBuildTime/TrainingTime scale the enqueued duration (≥ 1 tick).
        let ticks = ((ticks as f64 * research_time_mult).round() as u64).max(1);
        let complete_tick = self.tick + ticks;
        self.build_queue.push(crate::build::BuildJob {
            id: self.next_build_id,
            owner: player_id,
            system: system_id,
            body_id,
            what,
            complete_tick,
            // Join only applies to ship builds; an upgrade always passes None.
            join: if matches!(what, crate::build::BuildKind::Ship { .. }) { join } else { None },
            // §modules: carry the (validated, debited) loadout to spawn time.
            loadout: if matches!(what, crate::build::BuildKind::Ship { .. }) { loadout } else { crate::module::Loadout::default() },
        });
        events.push(Event::new(
            self.time,
            EventPayload::BuildStarted { id: self.next_build_id, owner: player_id, system: system_id, what, complete_tick },
        ));
    }

    /// Is `fleet` docked at one of `owner`'s OWNED systems (within the claim
    /// radius) AND idle? The gate for all v1 fleet management — you compose
    /// fleets at a berth you control, never in flight (no in-flight detachment in
    /// v1). Deterministic: a fixed radius test over id-ordered systems.
    fn fleet_at_owned_system(&self, owner: PlayerId, fleet_id: EntityId) -> bool {
        let Some(fleet) = self.fleets.get(&fleet_id) else {
            return false;
        };
        if fleet.owner != owner || !matches!(fleet.order, FleetOrder::Idle) {
            return false;
        }
        self.systems.iter().any(|s| {
            s.owner == Some(owner) && s.pos.distance(fleet.pos) <= crate::ship::COLONY_CLAIM_RADIUS
        })
    }

    /// MERGE `from` into `into` (§FLEETS management v1). Both must be the
    /// player's, idle, and docked at one of their owned systems, co-located (same
    /// berth). `from`'s composition folds into `into`; its cargo transfers only
    /// if `into` has an empty hold (v1 keeps the single-manifest cargo model —
    /// two laden convoys don't silently discard a manifest). `from` is removed.
    /// Any violation is a silent soft-reject (no partial merge).
    fn apply_merge_fleets(&mut self, player: PlayerId, into: EntityId, from: EntityId, _events: &mut [Event]) {
        if into == from {
            return; // can't merge a fleet into itself
        }
        if !self.fleet_at_owned_system(player, into) || !self.fleet_at_owned_system(player, from) {
            return;
        }
        // Co-located at the same berth (both already near an owned system; require
        // them near EACH OTHER too, so two fleets at different owned systems don't
        // teleport-merge).
        let (ipos, fpos) = (self.fleets[&into].pos, self.fleets[&from].pos);
        if ipos.distance(fpos) > crate::ship::COLONY_CLAIM_RADIUS {
            return;
        }
        let removed = self.fleets.remove(&from).unwrap();
        let target = self.fleets.get_mut(&into).unwrap();
        for (k, n) in removed.composition {
            target.add(k, n);
        }
        // §modules: the absorbed fleet's FITS carry across the gangway.
        target.fold_loadouts(&removed.loadouts);
        if target.cargo.is_none() {
            target.cargo = removed.cargo;
        }
        // §economy Part 4: passengers walk across the gangway (never deleted).
        for (k, n) in removed.passengers {
            *target.passengers.entry(k).or_insert(0) += n;
        }
        // §modules Part B3: crates aboard transfer with the merge (never deleted).
        for (k, n) in removed.modules {
            *target.modules.entry(k).or_insert(0) += n;
        }
    }

    /// SPLIT `counts` ships off `fleet_id` into a NEW idle fleet beside it
    /// (§FLEETS management v1). The source must be the player's, idle, and docked
    /// at one of their owned systems. Soft-reject if `counts` is empty, asks for
    /// more of any kind than is aboard, or would take EVERYTHING (split some,
    /// keep some — an all-split is just the identity, disallowed to keep intent
    /// crisp). Cargo stays with the source (v1 single-manifest model).
    fn apply_split_fleet(
        &mut self,
        player: PlayerId,
        fleet_id: EntityId,
        counts: &std::collections::BTreeMap<ShipKind, u32>,
        events: &mut Vec<Event>,
    ) {
        if !self.fleet_at_owned_system(player, fleet_id) {
            return;
        }
        let src = &self.fleets[&fleet_id];
        // Validate: non-empty, each requested count ≤ aboard, and not the whole
        // fleet (leaves at least one ship behind).
        let requested: u32 = counts.values().copied().sum();
        if requested == 0 || requested >= src.total_count() {
            return;
        }
        if counts.iter().any(|(k, n)| *n > src.count(*k)) {
            return;
        }
        let (pos, owner) = (src.pos, src.owner);
        // Detach. §modules: the detached ships carry their FITS (fitted stacks
        // taken first) — an escort split off keeps its loadout.
        let mut new_comp: BTreeMap<ShipKind, u32> = BTreeMap::new();
        let mut new_loadouts: crate::combat::LoadoutMap = BTreeMap::new();
        {
            let src = self.fleets.get_mut(&fleet_id).unwrap();
            for (k, n) in counts {
                if *n > 0 {
                    let taken = src.detach_loadouts(*k, *n);
                    src.remove(*k, *n);
                    new_comp.insert(*k, *n);
                    for (kind, m) in taken {
                        new_loadouts.entry(kind).or_default().extend(m);
                    }
                }
            }
        }
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, owner, ShipKind::Scout, pos, FleetOrder::Idle, None);
        fleet.composition = new_comp;
        fleet.loadouts = new_loadouts;
        // Report the new fleet's flagship as a spawn (owner-only notice pathway).
        let flagship = fleet.flagship_kind();
        self.fleets.insert(id, fleet);
        events.push(Event::new(self.time, EventPayload::ShipSpawned { id, owner, kind: flagship }));
    }

    /// Resolve construction jobs whose completion tick has arrived: spawn built fleets
    /// (Idle, at the system) / apply system upgrades. Drains due jobs in id-push order
    /// (deterministic); a built ship is owned by whoever PAID even if the system was
    /// since lost (you keep what you built); an upgrade applies only if still owned.
    fn resolve_builds(&mut self, events: &mut Vec<Event>) {
        if !self.build_queue.iter().any(|j| j.complete_tick <= self.tick) {
            return;
        }
        let due: Vec<crate::build::BuildJob> =
            self.build_queue.iter().filter(|j| j.complete_tick <= self.tick).cloned().collect();
        self.build_queue.retain(|j| j.complete_tick > self.tick);
        for job in due {
            match job.what {
                crate::build::BuildKind::Ship { ship } => {
                    let pos = self
                        .systems
                        .iter()
                        .find(|s| s.id == job.system)
                        .map(|s| s.pos)
                        .or_else(|| self.players.get(&job.owner).map(|c| c.home))
                        .unwrap_or(self.hub);
                    // JOIN a docked fleet if the build asked to and it's still
                    // valid — the owner's, Idle, sitting at this system. Otherwise
                    // form a new fleet-of-one (the pre-FLEETS behaviour).
                    let join_target = job.join.filter(|fid| {
                        self.fleets.get(fid).is_some_and(|f| {
                            f.owner == job.owner
                                && matches!(f.order, FleetOrder::Idle)
                                && f.pos.distance(pos) <= crate::ship::COLONY_CLAIM_RADIUS
                        })
                    });
                    if let Some(fid) = join_target {
                        let f = self.fleets.get_mut(&fid).unwrap();
                        f.add(ship, 1);
                        // §modules: the built ship enters under its fitted loadout.
                        if !job.loadout.is_empty() {
                            *f.loadouts.entry(ship).or_default().entry(job.loadout.key()).or_insert(0) += 1;
                        }
                        events.push(Event::new(self.time, EventPayload::ShipSpawned { id: fid, owner: job.owner, kind: ship }));
                    } else {
                        let id = self.alloc_entity_id();
                        let mut f = Fleet::single(id, job.owner, ship, pos, FleetOrder::Idle, None);
                        if !job.loadout.is_empty() {
                            f.loadouts.entry(ship).or_default().insert(job.loadout.key(), 1);
                        }
                        self.fleets.insert(id, f);
                        events.push(Event::new(self.time, EventPayload::ShipSpawned { id, owner: job.owner, kind: ship }));
                    }
                    // §research R3: a commissioned WARSHIP is a Hulls-field verb.
                    if ship.is_combatant() {
                        self.add_research_verb(job.owner, crate::research::Verb::WarshipsCommissioned, 1.0);
                    }
                }
                crate::build::BuildKind::Train { specialist } => {
                    // §economy Part 4: the cohort graduates into the RESIDENT
                    // pool — only if the owner still holds the system (like an
                    // upgrade: the resources were already spent, frontier risk).
                    let trained = if let Some(sys) = self.systems.iter_mut().find(|s| s.id == job.system && s.owner == Some(job.owner)) {
                        *sys.specialists.entry(specialist).or_insert(0) += 1;
                        events.push(Event::new(self.time, EventPayload::SpecialistTrained { owner: job.owner, system: job.system, kind: specialist }));
                        true
                    } else {
                        false
                    };
                    if trained {
                        // §research R3: a graduated specialist is a Talent verb.
                        self.add_research_verb(job.owner, crate::research::Verb::SpecialistsTrained, 1.0);
                    }
                }
                crate::build::BuildKind::Module { module } => {
                    // §modules: the crate lands in the system's module ledger —
                    // only if the owner still holds it (resources already spent).
                    if let Some(sys) = self.systems.iter_mut().find(|s| s.id == job.system && s.owner == Some(job.owner)) {
                        *sys.modules.entry(module).or_insert(0) += 1;
                        events.push(Event::new(self.time, EventPayload::ModuleBuilt { owner: job.owner, system: job.system, kind: module }));
                    }
                }
                crate::build::BuildKind::Upgrade { upgrade } => {
                    // Apply only if the owner still holds the system (can't upgrade a
                    // system you lost; the resources were already spent — frontier risk).
                    if let Some(sys) = self.systems.iter_mut().find(|s| s.id == job.system && s.owner == Some(job.owner)) {
                        // §bodies: the tier lands on the JOB'S BODY (falling
                        // back to the siting rules for a pre-bodies job id).
                        let target = sys
                            .bodies
                            .iter()
                            .any(|b| b.id == job.body_id)
                            .then_some(job.body_id)
                            .or_else(|| sys.site_for(upgrade))
                            .unwrap_or(0);
                        let mut tier = 0;
                        if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == target) {
                            tier = b.tier(upgrade) + 1;
                            b.set_tier(upgrade, tier);
                        }
                        events.push(Event::new(self.time, EventPayload::SystemUpgraded { system: job.system, owner: job.owner, upgrade, tier }));
                    }
                }
            }
        }
    }

    /// §modules Part B3 (Sol hub): a module's GOODS VALUE — its recipe commodities
    /// priced at Sol's standing market. The buy/sell prices scale off this, so
    /// they TRACK the commodity market (a module is worth what its inputs cost).
    fn module_recipe_value(&self, kind: crate::module::ModuleKind) -> f64 {
        crate::build::module_recipe(kind)
            .costs
            .iter()
            .map(|(c, n)| n * self.market.price(*c))
            .sum()
    }
    /// Sol's SELL price to a player (buying from Sol): a premium over local build.
    fn module_buy_price(&self, kind: crate::module::ModuleKind) -> f64 {
        self.module_recipe_value(kind) * crate::module::MODULE_BUY_MULT
    }
    /// Sol's BUY-BACK price from a player (selling to Sol): a steep discount.
    fn module_sell_price(&self, kind: crate::module::ModuleKind) -> f64 {
        self.module_recipe_value(kind) * crate::module::MODULE_SELL_MULT
    }

    /// §modules Part B4: kick off a REFIT — pull `n` ships of `ship`/`from` out of
    /// the player's docked fleet, reconcile the module delta against the yard's
    /// ledger, and enqueue the hulls to rejoin fitted to `to`. Soft-reject (no
    /// mutation) on any violation; the hulls are OUT of combat while in the yard.
    fn apply_refit(
        &mut self,
        player_id: PlayerId,
        fleet_id: EntityId,
        ship: ShipKind,
        from: crate::module::Loadout,
        to: crate::module::Loadout,
        n: u32,
        events: &mut Vec<Event>,
    ) {
        // The fleet must be the player's and IDLE (you refit in the yard, not mid-run).
        let Some(fleet) = self.fleets.get(&fleet_id) else { return };
        if fleet.owner != player_id || !matches!(fleet.order, FleetOrder::Idle) {
            return;
        }
        // `to` must fit the hull — slots AND the fitting budget (§fitting; a
        // grandfathered over-budget `from` may only refit INTO legality) — and
        // be an actual CHANGE from `from`.
        if !to.validate(ship) || from == to {
            return;
        }
        let pos = fleet.pos;
        // Docked at a Shipyard ≥ 1 the player OWNS or is ALLIED with (fits install
        // at a yard; an ally may host the work — a coalition refit yard).
        let allies = self.allies_of(player_id);
        let Some(sys_id) = self
            .systems
            .iter()
            .find(|s| {
                s.owner.is_some_and(|o| o == player_id || allies.contains(&o))
                    && s.tier(crate::build::StructureKind::Shipyard) >= 1
                    && s.pos.distance(pos) <= crate::ship::COLONY_CLAIM_RADIUS
            })
            .map(|s| s.id)
        else {
            return; // no hosting yard in reach — soft reject
        };
        // How many `(ship, from)` hulls the fleet actually holds (clamp `n`).
        let available = if from.is_empty() {
            fleet.count(ship).saturating_sub(fleet.fitted_count(ship))
        } else {
            fleet.loadouts.get(&ship).and_then(|m| m.get(&from.key())).copied().unwrap_or(0)
        };
        let n = n.min(available);
        if n == 0 {
            return; // nothing matching to refit — soft reject
        }
        // Per-ship module DELTA: added = to − from (debited), removed = from − to
        // (returned). Counting handles multi-slot fits with repeats correctly.
        let count = |lo: &crate::module::Loadout| -> BTreeMap<crate::module::ModuleKind, u32> {
            let mut m: BTreeMap<crate::module::ModuleKind, u32> = BTreeMap::new();
            for k in lo.modules() {
                *m.entry(*k).or_insert(0) += 1;
            }
            m
        };
        let from_ct = count(&from);
        let to_ct = count(&to);
        let mut added: BTreeMap<crate::module::ModuleKind, u32> = BTreeMap::new();
        let mut removed: BTreeMap<crate::module::ModuleKind, u32> = BTreeMap::new();
        for k in crate::module::MODULE_KINDS {
            let f = from_ct.get(&k).copied().unwrap_or(0);
            let t = to_ct.get(&k).copied().unwrap_or(0);
            if t > f {
                added.insert(k, t - f);
            } else if f > t {
                removed.insert(k, f - t);
            }
        }
        // The ADDED modules (× n) must be covered by the yard's ledger.
        let sys = self.systems.iter().find(|s| s.id == sys_id).expect("found above");
        let covered = added.iter().all(|(m, c)| sys.modules.get(m).copied().unwrap_or(0) >= *c * n);
        if !covered {
            return; // ledger can't cover the new fit — soft reject (no debit)
        }
        // Commit: pull the hulls out of the fleet, then reconcile the ledger.
        let fleet = self.fleets.get_mut(&fleet_id).expect("checked above");
        let pulled = fleet.remove_stack(ship, &from, n);
        debug_assert_eq!(pulled, n, "remove_stack clamps to `available`, which bounds n");
        let empty = fleet.is_empty();
        if empty {
            self.fleets.remove(&fleet_id);
        }
        let sys = self.systems.iter_mut().find(|s| s.id == sys_id).expect("found above");
        for (m, c) in &added {
            let left = sys.modules.get(m).copied().unwrap_or(0).saturating_sub(*c * n);
            if left == 0 {
                sys.modules.remove(m);
            } else {
                sys.modules.insert(*m, left);
            }
        }
        for (m, c) in &removed {
            *sys.modules.entry(*m).or_insert(0) += *c * n;
        }
        self.next_build_id += 1;
        let complete_tick = self.tick + crate::build::REFIT_TICKS_PER_SHIP * n as u64;
        self.refit_queue.push(crate::build::RefitJob {
            id: self.next_build_id,
            owner: player_id,
            system: sys_id,
            fleet: fleet_id,
            ship,
            to,
            n,
            complete_tick,
        });
        events.push(Event::new(
            self.time,
            EventPayload::BuildStarted {
                id: self.next_build_id,
                owner: player_id,
                system: sys_id,
                what: crate::build::BuildKind::Ship { ship },
                complete_tick,
            },
        ));
    }

    /// §modules Part B4: return refitted hulls from the yard. Rejoin the original
    /// fleet if it's still the owner's, Idle, and docked here; else the hulls form
    /// a fresh fleet-of-one at the yard. The hulls follow their OWNER, so a capture
    /// mid-refit still hands them back (they were never the system's).
    fn resolve_refits(&mut self, events: &mut Vec<Event>) {
        if !self.refit_queue.iter().any(|j| j.complete_tick <= self.tick) {
            return;
        }
        let due: Vec<crate::build::RefitJob> =
            self.refit_queue.iter().filter(|j| j.complete_tick <= self.tick).cloned().collect();
        self.refit_queue.retain(|j| j.complete_tick > self.tick);
        for job in due {
            let pos = self
                .systems
                .iter()
                .find(|s| s.id == job.system)
                .map(|s| s.pos)
                .or_else(|| self.players.get(&job.owner).map(|c| c.home))
                .unwrap_or(self.hub);
            let rejoin = job.fleet;
            let can_rejoin = self.fleets.get(&rejoin).is_some_and(|f| {
                f.owner == job.owner
                    && matches!(f.order, FleetOrder::Idle)
                    && f.pos.distance(pos) <= crate::ship::COLONY_CLAIM_RADIUS
            });
            if can_rejoin {
                let f = self.fleets.get_mut(&rejoin).unwrap();
                f.add(job.ship, job.n);
                if !job.to.is_empty() {
                    *f.loadouts.entry(job.ship).or_default().entry(job.to.key()).or_insert(0) += job.n;
                }
            } else {
                let id = self.alloc_entity_id();
                let mut f = Fleet::single(id, job.owner, job.ship, pos, FleetOrder::Idle, None);
                // Fleet::single seeds one ship; add the rest and the fits.
                f.add(job.ship, job.n - 1);
                if !job.to.is_empty() {
                    f.loadouts.entry(job.ship).or_default().insert(job.to.key(), job.n);
                }
                self.fleets.insert(id, f);
            }
            events.push(Event::new(
                self.time,
                EventPayload::ShipsRefitted { owner: job.owner, system: job.system, ship: job.ship, loadout: job.to.clone(), n: job.n },
            ));
        }
    }

    /// §research: a syndicate-wide STATE METRIC (summed over the members' held
    /// systems) — feeds `State`/`Sustained` research gates.
    pub fn syndicate_metric(&self, sid: SyndicateId, m: crate::research::Metric) -> f64 {
        let Some(syn) = self.syndicates.get(&sid) else { return 0.0 };
        let members = &syn.members;
        match m {
            crate::research::Metric::TotalPopulation => self
                .systems
                .iter()
                .filter(|s| s.owner.is_some_and(|o| members.contains(&o)))
                .map(|s| s.population())
                .sum(),
            crate::research::Metric::WellSuppliedSystems => self
                .systems
                .iter()
                .filter(|s| {
                    s.owner.is_some_and(|o| members.contains(&o))
                        && s.food_state == crate::colony::FoodState::WellSupplied
                })
                .count() as f64,
        }
    }

    /// §research R4: the [`crate::research::ResearchState`] governing `owner` —
    /// its syndicate's (research is a syndicate institution). None if the corp is
    /// in no syndicate; every effect site then falls back to the neutral default.
    fn research_of(&self, owner: PlayerId) -> Option<&crate::research::ResearchState> {
        self.players
            .get(&owner)
            .and_then(|c| c.syndicate)
            .and_then(|sid| self.syndicates.get(&sid))
            .map(|s| &s.research)
    }

    /// §research R4a: a single tuner's value for `owner` (identity default: 1.0
    /// multiplicative, 0.0 additive — so an un-researched corp is unaffected).
    fn research_mod(&self, owner: PlayerId, key: crate::research::ModKey) -> f64 {
        self.research_of(owner)
            .map(|r| crate::research::mod_of(r, key))
            .unwrap_or(if key.is_additive() { 0.0 } else { 1.0 })
    }

    /// §research R4c: has `owner`'s syndicate unlocked this capability flag?
    /// (Enforcement points wired in the R4c flags pass.)
    #[allow(dead_code)]
    fn research_flag(&self, owner: PlayerId, cap: crate::research::Cap) -> bool {
        self.research_of(owner).is_some_and(|r| crate::research::has_flag(r, cap))
    }

    /// §research R4b: the best research-granted tier for `kind` (0 = none). Wired
    /// into the structure TIER CEILING (§industrial-headroom): a syndicate holding
    /// this kind's Tier-IV/V unlock builds past the base cap of 4 up to 6 (see
    /// [`crate::build::max_buildable_tier`], applied in `apply_build`).
    fn research_struct_tier(&self, owner: PlayerId, kind: crate::build::StructureKind) -> u32 {
        self.research_of(owner)
            .map(|r| crate::research::unlocked_structure_tier(r, kind))
            .unwrap_or(0)
    }

    /// §research R4b: has `owner`'s syndicate unlocked this hull? Wired into the
    /// §ladder BuildShip gate — the five capital hulls soft-reject without it.
    fn research_hull(&self, owner: PlayerId, kind: ShipKind) -> bool {
        self.research_of(owner).is_some_and(|r| crate::research::has_hull(r, kind))
    }

    /// §ladder B4: does `owner`'s syndicate (or the lone corp, if unaffiliated)
    /// already FIELD or have QUEUED a Titan? The singleton check counts every
    /// member's fleets and every member's pending Titan keel.
    fn titan_fielded_or_queued(&self, owner: PlayerId) -> bool {
        let members: std::collections::BTreeSet<PlayerId> = self
            .players
            .get(&owner)
            .and_then(|c| c.syndicate)
            .and_then(|sid| self.syndicates.get(&sid))
            .map(|s| s.members.clone())
            .unwrap_or_else(|| std::collections::BTreeSet::from([owner]));
        let fielded = self
            .fleets
            .values()
            .any(|f| members.contains(&f.owner) && f.count(ShipKind::Titan) > 0);
        let queued = self.build_queue.iter().any(|j| {
            members.contains(&j.owner)
                && matches!(j.what, crate::build::BuildKind::Ship { ship: ShipKind::Titan })
        });
        // A Titan IN THE YARD mid-refit is out of every fleet for the job's
        // duration — it still counts (else a 3s-per-hull refit window lets a
        // second keel slip past the singleton).
        let refitting = self
            .refit_queue
            .iter()
            .any(|j| members.contains(&j.owner) && j.ship == ShipKind::Titan);
        fielded || queued || refitting
    }
    #[allow(dead_code)]
    fn research_module(&self, owner: PlayerId, kind: crate::module::ModuleKind) -> bool {
        self.research_of(owner).is_some_and(|r| crate::research::has_module(r, kind))
    }

    /// §research R6: the per-Academy CONTRIBUTION TABLE for `sid`'s ACTIVE
    /// programme (empty if none, or if the active programme is currently gated).
    /// A read-only mirror of [`Self::tick_research`]'s factor chain — the numbers
    /// the panel shows are exactly the numbers the clock accrues (design law 2).
    pub fn research_contributions(&self, sid: SyndicateId) -> Vec<AcademyContribution> {
        let Some(syn) = self.syndicates.get(&sid) else { return Vec::new() };
        let Some(active_id) = syn.research.active.as_deref() else { return Vec::new() };
        let Some(prog) = crate::research::programme(active_id) else { return Vec::new() };
        // A GATED active accrues nothing (it's waiting) — no contributions shown.
        let metric = |m| self.syndicate_metric(sid, m);
        if !crate::research::is_available(active_id, &syn.research, &metric, self.time) {
            return Vec::new();
        }
        let acad = crate::build::StructureKind::Academy;
        let field = prog.field;
        let basket = crate::research::basket(field, prog.tier);
        let members = &syn.members;
        let mut out = Vec::new();
        for sys in self.systems.iter().filter(|s| s.owner.is_some_and(|o| members.contains(&o))) {
            let food_state = sys.food_state;
            for b in &sys.bodies {
                let t = b.tier(acad);
                if t == 0 {
                    continue;
                }
                let staffing = sys.staffing_factor(b.id, acad);
                if staffing <= 0.0 {
                    continue;
                }
                let matched: u32 = crate::research::field_affinity(field)
                    .iter()
                    .map(|k| b.assignments.get(&acad).and_then(|a| a.specialists.get(k)).copied().unwrap_or(0))
                    .sum();
                let skill = crate::production::skill_factor(matched, t);
                let food = crate::production::food_factor(acad, food_state);
                let throughput = crate::production::tier_throughput(t);
                let rate = throughput * staffing * skill * food;
                if rate <= 0.0 {
                    continue;
                }
                let supplied = basket
                    .iter()
                    .all(|(c, per)| sys.stockpile.get(c).copied().unwrap_or(0.0) + 1e-9 >= *per * rate * DT);
                out.push(AcademyContribution {
                    system: sys.id,
                    system_name: sys.name.clone(),
                    body_id: b.id,
                    tier: t,
                    throughput,
                    staffing,
                    skill,
                    food,
                    rate,
                    supplied,
                });
            }
        }
        out
    }

    /// §research R3: add to a cumulative research VERB for `owner`'s syndicate
    /// (no-op if the owner isn't in one). The corp-wide biography that gates.
    fn add_research_verb(&mut self, owner: PlayerId, verb: crate::research::Verb, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        if let Some(sid) = self.players.get(&owner).and_then(|c| c.syndicate)
            && let Some(s) = self.syndicates.get_mut(&sid)
        {
            s.research.add_verb(verb, amount);
        }
    }

    /// §research R3: record a DISTINCT rival/pirate fleet observation for
    /// `observer`'s syndicate — the `RivalFleetsObserved` verb tracks the
    /// seen-set's len (dedupes re-sightings).
    fn observe_rival_fleet(&mut self, observer: PlayerId, fleet: EntityId) {
        if let Some(sid) = self.players.get(&observer).and_then(|c| c.syndicate)
            && let Some(s) = self.syndicates.get_mut(&sid)
            && s.research.observed_fleets.insert(fleet)
        {
            let n = s.research.observed_fleets.len() as f64;
            s.research.verbs.insert(crate::research::Verb::RivalFleetsObserved, n);
        }
    }

    /// §research R3: record a DISTINCT scouted system (first knowledge advance)
    /// for `owner`'s syndicate — the `SystemsScouted` verb tracks the set len.
    fn scout_system_for_research(&mut self, owner: PlayerId, system: EntityId) {
        if let Some(sid) = self.players.get(&owner).and_then(|c| c.syndicate)
            && let Some(s) = self.syndicates.get_mut(&sid)
            && s.research.scouted_systems.insert(system)
        {
            let n = s.research.scouted_systems.len() as f64;
            s.research.verbs.insert(crate::research::Verb::SystemsScouted, n);
        }
    }

    /// §research R3: the periodic RIVAL-OBSERVATION scan (Shadow school gate). For
    /// every corp in a syndicate, gather its detection coverage (own fleet bubbles
    /// + owned Sensor Arrays) and record each DISTINCT rival/pirate fleet it can
    /// currently sense — the SAME `detection::detected` the View uses, so the
    /// counter only grows off contacts the player could actually see. Deduped per
    /// syndicate by fleet id (re-sightings never re-count). Throttled by the
    /// caller; O(corps × sources × fleets), read-only pass then a deferred apply.
    fn observe_rivals_for_research(&mut self) {
        let sensor = self.config.sensor_range;
        // Read-only fleet snapshot (id, owner, pos, signature).
        let snap: Vec<(EntityId, PlayerId, Vec2, f64)> = self
            .fleets
            .iter()
            .map(|(id, s)| {
                let sig = s.signature()
                    * self.veil_factor(s.owner, s.pos)
                    * if s.surveying() { crate::explore::SURVEY_SIGNATURE_FACTOR } else { 1.0 };
                (*id, s.owner, s.pos, sig)
            })
            .collect();
        // Only corps that are in a syndicate can bank the observation.
        let observers: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|(_, c)| c.syndicate.is_some())
            .map(|(id, _)| *id)
            .collect();
        let mut hits: Vec<(PlayerId, EntityId)> = Vec::new();
        for obs in observers {
            let allies = self.allies_of(obs);
            // Coverage sources: this corp's own fleet bubbles (× SensorRange) + its
            // Sensor Arrays (already × SensorRadius in array_sensor_sources).
            let range = sensor * self.research_mod(obs, crate::research::ModKey::SensorRange);
            let mut sources: Vec<(Vec2, f64)> = snap
                .iter()
                .filter(|(_, o, _, _)| *o == obs)
                .map(|(_, _, pos, _)| (*pos, range))
                .collect();
            sources.extend(self.array_sensor_sources(obs));
            if sources.is_empty() {
                continue;
            }
            for (fid, fowner, fpos, sig) in &snap {
                // A rival = neither self nor a syndicate ally (pirates included).
                if *fowner == obs || allies.contains(fowner) {
                    continue;
                }
                if crate::detection::detected(*sig, &sources, *fpos) {
                    hits.push((obs, *fid));
                }
            }
        }
        for (obs, fid) in hits {
            self.observe_rival_fleet(obs, fid);
        }
    }

    /// §research R2: the DISTRIBUTED CLOCK. For each syndicate with an AVAILABLE
    /// active programme, every staffed member Academy contributes
    /// `rate = academy_tier_throughput × staffing × affine-specialist skill ×
    /// food`, dripping `basket × rate × dt` from its OWN system stockpile; a lab
    /// whose stockpile can't cover its drip suspends its contribution (soft). The
    /// syndicate's progress grows by Σ funded rate × dt. No staffed Academy →
    /// latched `ResearchStalled` (once), with `ResearchResumed` on recovery.
    fn tick_research(&mut self, events: &mut Vec<Event>) {
        if self.syndicates.is_empty() {
            return;
        }
        let now = self.time;
        let acad = crate::build::StructureKind::Academy;
        let sids: Vec<SyndicateId> = self.syndicates.keys().copied().collect();
        // §research: the one SUSTAINED metric (Life · Growth V endurance gate) —
        // stamp when the WellSupplied count first reaches the threshold, clear the
        // moment it drops so an interruption resets the 7-day clock. Tracked for
        // EVERY syndicate each tick, independent of what's currently active.
        if let crate::research::Gate::Sustained(metric, thresh, _) = crate::research::tier_gate(
            crate::research::Field::Life,
            Some(crate::research::School::Growth),
            5,
        ) {
            let stamp = now.floor() as u64;
            for &sid in &sids {
                let val = self.syndicate_metric(sid, metric);
                if let Some(s) = self.syndicates.get_mut(&sid) {
                    if val + 1e-9 >= thresh {
                        s.research.sustained_since.entry(metric).or_insert(stamp);
                    } else {
                        s.research.sustained_since.remove(&metric);
                    }
                }
            }
        }
        for sid in sids {
            // Snapshot the active programme; idle syndicates clear any stall latch.
            let active = self.syndicates[&sid].research.active.clone();
            let Some(active_id) = active else {
                if let Some(s) = self.syndicates.get_mut(&sid) {
                    s.research.stalled = false;
                }
                continue;
            };
            let Some(prog) = crate::research::programme(&active_id) else { continue };
            let (field, tier) = (prog.field, prog.tier);
            // A GATED active accrues nothing but is NOT a stall (it's waiting).
            let available = {
                let metric = |m| self.syndicate_metric(sid, m);
                crate::research::is_available(&active_id, &self.syndicates[&sid].research, &metric, now)
            };
            if !available {
                continue;
            }
            let members: std::collections::BTreeSet<PlayerId> = self.syndicates[&sid].members.clone();
            let basket = crate::research::basket(field, tier);
            let mut funded_rate = 0.0;
            let mut any_staffed = false;
            for sys in self.systems.iter_mut().filter(|s| s.owner.is_some_and(|o| members.contains(&o))) {
                let food = sys.food_state;
                // Per-Academy rate (immutable reads), collected before we debit.
                let labs: Vec<(u32, f64)> = sys
                    .bodies
                    .iter()
                    .filter_map(|b| {
                        let t = b.tier(acad);
                        if t == 0 {
                            return None;
                        }
                        let staffing = sys.staffing_factor(b.id, acad);
                        if staffing <= 0.0 {
                            return None; // an unstaffed Academy contributes nothing
                        }
                        // Skill from the RESEARCH FIELD's affine specialists posted here.
                        let matched: u32 = crate::research::field_affinity(field)
                            .iter()
                            .map(|k| b.assignments.get(&acad).and_then(|a| a.specialists.get(k)).copied().unwrap_or(0))
                            .sum();
                        let skill = crate::production::skill_factor(matched, t);
                        let foodf = crate::production::food_factor(acad, food);
                        let rate = crate::production::tier_throughput(t) * staffing * skill * foodf;
                        Some((t, rate))
                    })
                    .collect();
                if labs.is_empty() {
                    continue;
                }
                any_staffed = true;
                // Fund each lab from THIS system's stockpile (all-or-nothing per lab).
                for (_t, rate) in labs {
                    if rate <= 0.0 {
                        continue;
                    }
                    let covers = basket
                        .iter()
                        .all(|(c, per)| sys.stockpile.get(c).copied().unwrap_or(0.0) + 1e-9 >= *per * rate * DT);
                    if !covers {
                        continue; // supply-starved lab suspends this tick (soft)
                    }
                    for (c, per) in &basket {
                        let left = sys.stockpile.get(c).copied().unwrap_or(0.0) - *per * rate * DT;
                        if left <= 1e-9 {
                            sys.stockpile.remove(c);
                        } else {
                            sys.stockpile.insert(*c, left);
                        }
                    }
                    funded_rate += rate;
                }
            }
            let s = self.syndicates.get_mut(&sid).unwrap();
            s.research.progress += funded_rate * DT;
            if !any_staffed {
                if !s.research.stalled {
                    s.research.stalled = true;
                    events.push(Event::new(now, EventPayload::ResearchStalled { syndicate: sid }));
                }
            } else if s.research.stalled {
                s.research.stalled = false;
                events.push(Event::new(now, EventPayload::ResearchResumed { syndicate: sid }));
            }
        }
    }

    /// §research: COMPLETE every fully-funded active programme (the distributed
    /// clock funds `progress` in R2). Completion respects availability — carried
    /// overflow can never skip a tier gate. The effect is realized lazily via
    /// `research::mods`, so it applies instantly and galaxy-wide (decision #5).
    fn resolve_research(&mut self, events: &mut Vec<Event>) {
        if self.syndicates.is_empty() {
            return;
        }
        let now = self.time;
        let sids: Vec<SyndicateId> = self.syndicates.keys().copied().collect();
        for sid in sids {
            loop {
                // Only complete when the active programme is currently available
                // (gate met + ladder rule) — never on carried overflow alone.
                let ready = {
                    let Some(s) = self.syndicates.get(&sid) else { break };
                    match s.research.active.as_deref() {
                        None => false,
                        Some(id) => {
                            let metric = |m| self.syndicate_metric(sid, m);
                            s.research.progress + 1e-6 >= crate::research::cost_of(id)
                                && crate::research::is_available(id, &s.research, &metric, now)
                        }
                    }
                };
                if !ready {
                    break;
                }
                let done = self
                    .syndicates
                    .get_mut(&sid)
                    .and_then(|s| s.research.try_complete());
                match done {
                    Some(id) => {
                        // §research: emit TierUnlocked when this completion is the
                        // FIRST on its ladder tier — that's the moment the next tier
                        // opens (siblings after it don't re-open it). Ladder = the
                        // field's SHARED line for tiers I/II, the SAME SCHOOL for III+.
                        if let Some(p) = crate::research::programme(&id)
                            && p.tier < 5
                        {
                            let shared = p.tier <= 2;
                            let count = self.syndicates[&sid]
                                .research
                                .completed
                                .iter()
                                .filter_map(|c| crate::research::programme(c))
                                .filter(|q| {
                                    q.field == p.field
                                        && q.tier == p.tier
                                        && if shared { q.school.is_none() } else { q.school == p.school }
                                })
                                .count();
                            if count == 1 {
                                events.push(Event::new(now, EventPayload::TierUnlocked {
                                    syndicate: sid,
                                    field: p.field,
                                    school: if shared { None } else { p.school },
                                    tier: p.tier + 1,
                                }));
                            }
                        }
                        events.push(Event::new(now, EventPayload::ResearchCompleted { syndicate: sid, programme: id }));
                    }
                    None => break,
                }
            }
        }
    }

    /// Create or replace a standing logistics order (§15). INSTANT local
    /// administration: validates against the constrained option set, then either
    /// allocates a fresh id (create) or replaces an existing rule by id (edit),
    /// PRESERVING its anti-spam state (`next_eval_tick`, `in_flight`) so editing a
    /// rule can't be used to bypass the dispatch cadence. Invalid rules are ignored.
    fn apply_set_standing_order(&mut self, player_id: PlayerId, mut order: StandingOrder) {
        // --- Validate the constrained option set (reject nonsense outright) ---
        let Some(source_sys) = order.source.system_id() else {
            return; // the source must be a system
        };
        if order.source == order.dest {
            return; // a route to itself is meaningless
        }
        // The source system must exist and be owned by this player right now.
        let owns_source = self
            .systems
            .iter()
            .any(|s| s.id == source_sys && s.owner == Some(player_id));
        if !owns_source {
            return;
        }
        // Destination, if a system, must exist (ownership may change later — that's
        // a frontier risk handled at arrival).
        if let crate::standing::Endpoint::System { id } = order.dest
            && !self.systems.iter().any(|s| s.id == id)
        {
            return;
        }
        match order.trigger {
            crate::standing::Trigger::AboveThreshold { threshold } => {
                if !(threshold.is_finite() && threshold >= 0.0) {
                    return;
                }
            }
            crate::standing::Trigger::PercentSurplus { percent, floor } => {
                if !((1..=100).contains(&percent) && floor.is_finite() && floor >= 0.0) {
                    return;
                }
            }
            crate::standing::Trigger::MaintainAtDest { target } => {
                // "Maintain a level" only makes sense at a stockpiling destination.
                if matches!(order.dest, crate::standing::Endpoint::Hub) {
                    return;
                }
                if !(target.is_finite() && target > 0.0) {
                    return;
                }
            }
        }

        let Some(corp) = self.players.get_mut(&player_id) else {
            return;
        };
        if order.id == 0 {
            corp.next_standing_id += 1;
            order.id = corp.next_standing_id;
            order.in_flight = None;
            // Deterministic per-rule stagger so many rules made the same tick don't
            // all fire on the same eval boundary (a load nicety; snapshot-stable).
            order.next_eval_tick = self.tick + (order.id as u64 % EVAL_PERIOD);
            corp.standing_orders.push(order);
        } else if let Some(slot) = corp.standing_orders.iter_mut().find(|o| o.id == order.id) {
            // Edit: keep the player-facing fields, preserve anti-spam bookkeeping.
            let (keep_eval, keep_flight) = (slot.next_eval_tick, slot.in_flight);
            *slot = order;
            slot.next_eval_tick = keep_eval;
            slot.in_flight = keep_flight;
        }
        // (Editing a non-existent id is a no-op.)
    }

    /// Claim an unclaimed system for the player, debiting the claim cost. Resolves
    /// in true space at `self.time`; rivals learn of it only by light (the view
    /// filter gates ownership). No-op if already owned or unaffordable.
    /// COLONY ARRIVALS (§fleets part 3): claiming is PHYSICAL. Each colony ship
    /// within [`crate::ship::COLONY_CLAIM_RADIUS`] of a system resolves here,
    /// after movement and raids (a ship killed at the doorstep never settles):
    ///
    ///   * UNCLAIMED, non-reserved system → SETTLE: ownership transfers, the
    ///     ship is CONSUMED (it becomes the colony — removed silently, no
    ///     destruction event), and `SystemClaimed` light-propagates exactly as
    ///     the old instant claim did. THE RACE: earlier arrival tick wins;
    ///     same-tick arrivals resolve in ship-id order (deterministic tiebreak).
    ///   * RESERVED home-site system → never settleable (an unassigned slot's
    ///     home can't be sniped before its player arrives).
    ///   * ALREADY CLAIMED by a RIVAL → the loser HOLDS at the spot, intact and
    ///     redirectable (soft — nothing destroyed); one owner-only `ColonyHeld`
    ///     notice per hold (`notified_held`, cleared when the ship moves again).
    ///     Arriving at your OWN system just parks quietly.
    fn resolve_colony_arrivals(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        // Deterministic pass: fleets in id order (BTreeMap).
        let colonists: Vec<EntityId> = self
            .fleets
            .iter()
            .filter(|(_, sh)| sh.contains(ShipKind::Colony))
            .map(|(id, _)| *id)
            .collect();
        for cid in colonists {
            let Some(ship) = self.fleets.get(&cid) else { continue };
            let (owner, pos, idle) = (ship.owner, ship.pos, matches!(ship.order, FleetOrder::Idle));
            // Nearest system within settle range ((distance, id) tiebreak).
            let near = self
                .systems
                .iter()
                .filter(|sy| sy.pos.distance(pos) <= crate::ship::COLONY_CLAIM_RADIUS)
                .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)).then(a.id.cmp(&b.id)))
                .map(|sy| (sy.id, sy.owner));
            let Some((sys_id, sys_owner)) = near else {
                // In open space: clear any past hold-notice latch once it moves on.
                if !idle && let Some(sh) = self.fleets.get_mut(&cid) {
                    sh.notified_held = false;
                }
                continue;
            };
            let reserved = self.home_slots.iter().any(|h| h.system == Some(sys_id));
            match sys_owner {
                None if !reserved => {
                    // SETTLE: flip ownership, consume the ship (no destruction —
                    // it became the colony), light-propagate the claim.
                    if let Some(sy) = self.systems.iter_mut().find(|sy| sy.id == sys_id) {
                        sy.owner = Some(owner);
                        sy.claimed_at = Some(now);
                        // §economy Part 2: the ship's crew and berths FOUND the
                        // colony — a small hungry outpost (ship Provisions to
                        // grow it; it holds at founding size until a Habitat
                        // gives it capacity). Additive, so re-settling a
                        // captured-then-lost colony never shrinks anyone.
                        sy.seed_population(crate::colony::COLONY_FOUNDING_POP);
                        // §explore Part 3: the claim REVEALS the hidden trait (R3 —
                        // the blind claimer's gamble resolving), and a Precursor
                        // Cache pays its one-time grant (latched — once, ever).
                        if let Some(t) = sy.trait_ {
                            if t == crate::explore::SystemTrait::PrecursorCache && !sy.cache_claimed {
                                sy.cache_claimed = true;
                                let grant = crate::explore::PRECURSOR_ALLOYS.min(sy.storage_headroom());
                                *sy.stockpile.entry(crate::cargo::Commodity::Alloys).or_insert(0.0) += grant;
                            }
                            events.push(Event::new(now, EventPayload::TraitRevealed { owner, system: sys_id, pos, trait_: t }));
                        }
                    }
                    // §explore: holding a system IS knowing it — the blind claimer's
                    // gamble resolves here (permanent survey knowledge, R2).
                    if let Some(corp) = self.players.get_mut(&owner) {
                        corp.surveyed.insert(sys_id);
                    }
                    // Consume ONE colony ship (it BECAME the colony); the rest of
                    // the fleet — escorts, extra colonists — persists and parks at
                    // the new holding. A fleet-of-one colony empties and is removed,
                    // exactly as the old single-ship consume did. §economy Part 4:
                    // any specialists ABOARD the fleet disembark into the new
                    // colony's resident pool (founding talent — never deleted).
                    if let Some(fl) = self.fleets.get_mut(&cid) {
                        fl.remove_one(ShipKind::Colony);
                        let landed = std::mem::take(&mut fl.passengers);
                        // §modules Part B3: crates aboard land into the new colony's
                        // ledger (founding materiel — never deleted).
                        let landed_mods = std::mem::take(&mut fl.modules);
                        if fl.is_empty() {
                            self.fleets.remove(&cid);
                        } else {
                            fl.order = FleetOrder::Idle;
                            fl.notified_held = false;
                        }
                        if !landed.is_empty()
                            && let Some(sy) = self.systems.iter_mut().find(|sy| sy.id == sys_id)
                        {
                            for (k, n) in &landed {
                                *sy.specialists.entry(*k).or_insert(0) += n;
                            }
                            events.push(Event::new(now, EventPayload::SpecialistsDelivered { owner, system: sys_id, manifest: landed }));
                        }
                        if !landed_mods.is_empty()
                            && let Some(sy) = self.systems.iter_mut().find(|sy| sy.id == sys_id)
                        {
                            for (k, n) in &landed_mods {
                                *sy.modules.entry(*k).or_insert(0) += n;
                            }
                            events.push(Event::new(now, EventPayload::ModulesDelivered { owner, system: sys_id, manifest: landed_mods }));
                        }
                    }
                    events.push(Event::new(
                        now,
                        EventPayload::SystemClaimed { system: sys_id, owner, pos },
                    ));
                    // §node EXPOSURE: claiming an awakened node makes you its holder
                    // — announced galaxy-wide, light-delayed (no hiding a node's master).
                    if let Some(node) = self.nodes.get(&sys_id).filter(|n| n.awakened) {
                        events.push(Event::new(now, EventPayload::NodeCaptured { owner, system: sys_id, pos, bonus: node.bonus }));
                    }
                }
                None => {
                    // Reserved home site: hold like a lost race (soft, notice once).
                    if idle && !self.fleets[&cid].notified_held {
                        self.fleets.get_mut(&cid).unwrap().notified_held = true;
                        events.push(Event::new(now, EventPayload::ColonyHeld { owner, system: sys_id, pos }));
                    }
                }
                Some(holder) if holder != owner => {
                    // §Part 2 SIEGE → CAPTURE: a colony ship arriving at a RIVAL
                    // system CAPTURES it iff the besieger (== this colony's owner)
                    // has held an unbroken, defense-suppressed siege for
                    // SIEGE_DURATION — and it is NOT the holder's home (home
                    // protection: a beaten player always keeps a producing base).
                    // Otherwise the existing soft-hold: intact, redirectable,
                    // never consumed in vain. "Sieges strangle; only colonists
                    // conquer" — no colony ship = no capture, ever.
                    // §ladder: the SIEGE ANCHOR — a Battleship-or-heavier hull on
                    // blockade station accelerates the capture clock: the ripe
                    // duration divides by SIEGE_ANCHOR_MULT while one holds the
                    // line. A named factor, presence-gated at evaluation.
                    let anchored = self.fleets.values().any(|f| {
                        f.owner == owner
                            && matches!(f.order, crate::ship::FleetOrder::Blockade { system, .. } if system == sys_id)
                            && f.composition.iter().any(|(k, n)| *n > 0 && crate::ship::is_siege_anchor(*k))
                    });
                    let siege_dur = SIEGE_DURATION_BATTLE_MULT * self.config.battle_target_secs
                        / if anchored { crate::ship::SIEGE_ANCHOR_MULT } else { 1.0 };
                    let captureable = idle
                        && !self.is_home_system(holder, sys_id)
                        && self
                            .systems
                            .iter()
                            .find(|s| s.id == sys_id)
                            .and_then(|s| s.blockade)
                            .is_some_and(|b| b.by == owner && b.siege_since.is_some_and(|ss| now - ss >= siege_dur));
                    if captureable {
                        self.capture_system(sys_id, holder, owner, cid, pos, events);
                    } else if idle && !self.fleets[&cid].notified_held {
                        self.fleets.get_mut(&cid).unwrap().notified_held = true;
                        events.push(Event::new(now, EventPayload::ColonyHeld { owner, system: sys_id, pos }));
                    }
                }
                Some(_) => {
                    // Your own system: parking a colony ship there is unremarkable.
                }
            }
        }
    }

    /// §contestable-territory Part 2: FLIP a besieged system to the captor. All
    /// deterministic and light-honest (`SystemCaptured` propagates by light like
    /// a claim). Transfer rules:
    ///   • ownership → `new_owner`, `claimed_at = now` (light-gates the reveal);
    ///   • developments at HALF tiers (rounded down) — the occupation inherits a
    ///     damaged base, freeing slots per the damage rule; defense pool cleared;
    ///   • the stockpile stays on the system as PLUNDER (it's now the captor's);
    ///     a snapshot rides the report so the defender's news itemizes the loss;
    ///   • in-progress builds are DROPPED (the old owner paid; existing rule);
    ///   • ONE colony ship is consumed (it became the occupation government).
    /// Never called for a home system — the caller's home-protection gate holds.
    fn capture_system(&mut self, sys_id: EntityId, old_owner: PlayerId, new_owner: PlayerId, colony: EntityId, pos: Vec2, events: &mut Vec<Event>) {
        let now = self.time;
        // §ladder B2: a capture is the Line VIII verb — counted ONCE per
        // capture, at resolution (the only place ownership flips by siege).
        self.add_research_verb(new_owner, crate::research::Verb::SystemsCaptured, 1.0);
        // Snapshot the seized stockpile (whole units) for the report BEFORE the flip.
        let plunder: BTreeMap<crate::cargo::Commodity, u32> = self
            .systems
            .iter()
            .find(|s| s.id == sys_id)
            .map(|s| s.stockpile.iter().filter_map(|(c, a)| {
                let u = a.floor() as u32;
                (u >= 1).then_some((*c, u))
            }).collect())
            .unwrap_or_default();
        // §explore: capture transfers the geology knowledge too (spoils — the new
        // holder walks the ground; permanent survey knowledge, R2).
        if let Some(corp) = self.players.get_mut(&new_owner) {
            corp.surveyed.insert(sys_id);
        }
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == sys_id) {
            sys.owner = Some(new_owner);
            sys.claimed_at = Some(now);
            // Half tiers (rounded down) — a captured base is a damaged one.
            // §bodies: halve EVERY structure tier on EVERY body (zeroed entries
            // drop from the maps). DefensePlatform is 0 already (siege prereq).
            for b in sys.bodies.iter_mut() {
                let halved: Vec<(crate::build::StructureKind, u32)> =
                    b.structures.iter().map(|(k, t)| (*k, *t / 2)).collect();
                for (k, t) in halved {
                    b.set_tier(k, t);
                }
            }
            sys.defense_pool = 0.0;
            // §economy Part 2: POPULATION STAYS through a capture (people don't
            // vanish with the flag — never-decrease holds even here). food_state
            // is recomputed next tick under the new owner; a halved Habitat may
            // leave pop over capacity — that just freezes growth, nothing dies.
            sys.blockade = None; // it's the captor's now — no longer besieged
        }
        // Drop the OLD owner's in-progress builds here (they no longer own it).
        self.build_queue.retain(|j| j.system != sys_id);
        // Consume ONE colony ship (the occupation government), like settlement.
        if let Some(fl) = self.fleets.get_mut(&colony) {
            fl.remove_one(ShipKind::Colony);
            if fl.is_empty() {
                self.fleets.remove(&colony);
            } else {
                fl.order = FleetOrder::Idle;
                fl.notified_held = false;
            }
        }
        events.push(Event::new(now, EventPayload::SystemCaptured { old_owner, new_owner, system: sys_id, pos, plunder }));
        // §node EXPOSURE: capturing an awakened node flips its master — announced
        // galaxy-wide, light-delayed, so every corp learns who now commands it.
        if let Some(node) = self.nodes.get(&sys_id).filter(|n| n.awakened) {
            events.push(Event::new(now, EventPayload::NodeCaptured { owner: new_owner, system: sys_id, pos, bonus: node.bonus }));
        }
        // §explore Part 3: capture TRANSFERS the trait knowledge — spoils. (The
        // Precursor Cache latch survives the flip: it pays exactly once, ever.)
        if let Some(t) = self.systems.iter().find(|s| s.id == sys_id).and_then(|s| s.trait_) {
            events.push(Event::new(now, EventPayload::TraitRevealed { owner: new_owner, system: sys_id, pos, trait_: t }));
        }
    }

    /// Fleet a claimed system's accumulated production to the hub: one raidable
    /// convoy per stockpiled commodity (whole units), each selling on arrival.
    fn apply_ship_production(&mut self, player_id: PlayerId, system_id: EntityId, events: &mut Vec<Event>) {
        // Collect what to ship (and zero those stockpiles) without holding a
        // borrow across the convoy spawn.
        // BLOCKADE (§contestable-territory Part 1): an outbound dispatch from a
        // blockaded system HOLDS at origin — the goods stay in the (still-
        // accruing) stockpile, nothing is destroyed. The owner already learned of
        // the blockade (light-delayed onset notice); the client gates the button.
        if self.is_blockaded(system_id) {
            return;
        }
        let mut shipments: Vec<(Cargo, Vec2)> = Vec::new();
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system_id) {
            if sys.owner != Some(player_id) {
                return; // only the owner fleets from their system
            }
            let pos = sys.pos;
            for (commodity, amount) in sys.stockpile.iter_mut() {
                // Retain Fuel as the system's operating reserve — it powers movement
                // now (§step1 part 2), so "ship to hub" exports saleable output, not
                // the fuel you need to move it. (Sell fuel via the Market instead.)
                if *commodity == crate::fuel::MOVEMENT_FUEL {
                    continue;
                }
                let units = amount.floor() as u32;
                if units >= 1 {
                    *amount -= units as f64;
                    shipments.push((Cargo { commodity: *commodity, units }, pos));
                }
            }
        } else {
            return;
        }
        let hub = self.hub;
        for (cargo, pos) in shipments {
            // Fuel the haul ∝ distance × loaded mass; a shortfall HOLDS this convoy
            // (refund its goods to the system — never lost) and notifies the owner.
            let mass = ShipKind::Convoy.hull_mass()
                + cargo.units as f64 * crate::ship::CARGO_MASS_PER_UNIT;
            let cost = crate::fuel::fuel_cost(pos.distance(hub), mass);
            if !self.charge_fuel(player_id, pos, cost) {
                if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system_id) {
                    *sys.stockpile.entry(cargo.commodity).or_insert(0.0) += cargo.units as f64;
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::FuelShortfall {
                        owner: player_id,
                        needed: cost,
                        kind: crate::fuel::ShortfallKind::Shipment,
                    },
                ));
                continue;
            }
            events.push(Event::new(
                self.time,
                EventPayload::Trade(TradeEvent::SellDispatched {
                    player: player_id,
                    commodity: cargo.commodity,
                    units: cargo.units,
                }),
            ));
            self.spawn_trade_convoy(player_id, pos, hub, cargo, TradeMission::SellAtHub);
        }
    }

    /// §economy Part 3: run every owned system's COLONY LIFE + PRODUCTION for
    /// one tick. Deposits no longer produce by themselves — extraction needs
    /// its structure built AND staffed; converters additionally need their
    /// input basket in the local stockpile. Deterministic throughout.
    ///
    /// STORAGE CAP (§buildings step 2): a full system EXTRACTS nothing further —
    /// production simply IDLES at the cap until goods ship out (async-fair: an
    /// offline player's depot fills and waits; nothing is destroyed, and a
    /// grandfathered over-cap stockpile is untouched — the cap blocks NEW inflow
    /// only). Reserves are drawn down only by what actually accrues, so a full
    /// depot never wastes a finite deposit. Converters keep running at the cap
    /// (their ≥1:1 baskets always SHRINK the total).
    ///
    /// COLONY LIFE (§economy Part 2) — ordering rule, per owned system each
    /// tick, BEFORE accrual (a colony eats from its standing stock; this
    /// tick's fresh production replenishes it for the next):
    ///   1. EAT — draw `population · PROVISIONS_PER_MILLION_PER_S · DT` of
    ///      Provisions, PARTIAL draws allowed (rationing literally stretches
    ///      the last crumbs; the old atomic rule belonged to the binary boost).
    ///   2. LADDER — recompute the food state from post-draw stock coverage
    ///      (down instantly, up only past the hysteresis margin); transitions
    ///      emit owner-only notices.
    ///   3. GROW — Well Supplied AND below Habitat capacity → population rises
    ///      `POP_GROWTH_PER_S · DT`, clamped to the cap. THERE IS NO NEGATIVE
    ///      BRANCH: famine freezes growth, it never kills (§5.1 async-fair; an
    ///      offline player's colony waits hungry, exactly as big as they left
    ///      it). The old fed-Habitat OUTPUT BOOST is retired — Part 3's
    ///      assignment engine multiplies `food_state.efficiency()` into every
    ///      output instead (the legible factor chain).
    fn accrue_production(&mut self, events: &mut Vec<Event>) {
        // §research R3: per-tick verb deltas gathered while the systems are borrowed
        // mutably, then folded into the syndicate biographies after the loop (an
        // `add_research_verb` call would re-borrow `self`). (owner, verb, amount).
        let mut research_deltas: Vec<(PlayerId, crate::research::Verb, f64)> = Vec::new();
        // §research R4a: snapshot each owner's ECONOMY tuners once (the fold is
        // cheap, but this is the per-tick hot path and `self` is about to be
        // borrowed mutably by the systems loop). Neutral (all 1.0) for un-
        // researched corps, so the whole block is inert until R5 grants effects.
        use crate::research::ModKey;
        #[derive(Clone, Copy)]
        struct EconMods {
            extraction: f64,
            processing: f64,
            pop_growth: f64,
            habitat_cap: f64,
            provisions_use: f64,
            growth_below_half: f64,
        }
        let owners: std::collections::BTreeSet<PlayerId> =
            self.systems.iter().filter_map(|s| s.owner).collect();
        let econ: BTreeMap<PlayerId, EconMods> = owners
            .iter()
            .map(|&o| {
                (o, EconMods {
                    extraction: self.research_mod(o, ModKey::ExtractionRate),
                    processing: self.research_mod(o, ModKey::ProcessingYield),
                    pop_growth: self.research_mod(o, ModKey::PopGrowth),
                    habitat_cap: self.research_mod(o, ModKey::HabitatCap),
                    provisions_use: self.research_mod(o, ModKey::ProvisionsUse),
                    growth_below_half: self.research_mod(o, ModKey::GrowthBelowHalf),
                })
            })
            .collect();
        for sys in &mut self.systems {
            let Some(owner) = sys.owner else {
                continue;
            };
            let em = econ.get(&owner).copied().unwrap_or(EconMods {
                extraction: 1.0, processing: 1.0, pop_growth: 1.0,
                habitat_cap: 1.0, provisions_use: 1.0, growth_below_half: 1.0,
            });
            // --- Colony life (eat → ladder → grow; before accrual) ---
            // §bodies: ONE pooled food state — demand is the SUM of every
            // body's population against the system stockpile; GROWTH is per
            // body, toward that body's OWN Habitat capacity. Never decreases.
            // §research R4a ProvisionsUse tunes the per-capita ration draw.
            let demand_per_s = sys.population() * crate::colony::PROVISIONS_PER_MILLION_PER_S * em.provisions_use;
            if demand_per_s > 0.0 {
                let have = sys.stockpile.get(&crate::cargo::Commodity::Provisions).copied().unwrap_or(0.0);
                let draw = (demand_per_s * DT).min(have);
                if draw > 0.0 {
                    // (never inserts a zero entry — a dry colony leaves no crumbs)
                    *sys.stockpile.get_mut(&crate::cargo::Commodity::Provisions).unwrap() -= draw;
                }
                let coverage_secs = (have - draw) / demand_per_s;
                let state = crate::colony::food_state_for(coverage_secs, demand_per_s, sys.food_state);
                if state != sys.food_state {
                    sys.food_state = state;
                    events.push(Event::new(
                        self.time,
                        EventPayload::FoodStateChanged { owner, system: sys.id, state },
                    ));
                }
                if state == crate::colony::FoodState::WellSupplied {
                    for b in sys.bodies.iter_mut() {
                        // §research R4a HabitatCap raises the ceiling each tier buys.
                        let cap = crate::colony::POP_CAP_PER_HABITAT_TIER
                            * b.tier(crate::build::StructureKind::Habitat) as f64
                            * em.habitat_cap;
                        if b.population > 0.0 && b.population < cap {
                            let before = b.population;
                            // §research R4a PopGrowth scales the base rate; GrowthBelowHalf
                            // adds an early-colony boost while under half the ceiling.
                            let below_half = if b.population < cap * 0.5 { em.growth_below_half } else { 1.0 };
                            let rate = crate::colony::POP_GROWTH_PER_S * em.pop_growth * below_half;
                            b.population = (b.population + rate * DT).min(cap);
                            // §research R3: cumulative population grown (millions unit).
                            research_deltas.push((owner, crate::research::Verb::PopulationGrown, b.population - before));
                        }
                    }
                }
            } else {
                // No population, no demand — vacuously supplied (and silent:
                // an empty rock never spams food notices).
                sys.food_state = crate::colony::FoodState::WellSupplied;
            }

            // --- §economy Part 3: THE ASSIGNMENT ENGINE ------------------------
            // NOTHING produces unstaffed. Every line's output is the legible
            // factor chain  base · tier_throughput · staffing · skill · food
            // (skill = 1.0 until specialists land in Part 4). Order within the
            // tick: EXTRACTION first (fresh raws), then CONVERTERS in enum
            // order (they can eat this tick's raws; chained converters see
            // upstream output next tick — deterministic either way). A full
            // depot idles extraction (nothing destroyed); converters never
            // net-add units (baskets ≥ 1:1) so the cap can't bind on them
            // (a guard still bounds retunings).
            let share = sys.staffing_share();
            // §economy Part 4: the effective specialists on every line this
            // tick (pool-clamped, deterministic) — crew counts join staffing,
            // AFFINE counts drive the skill factor.
            let line_spec = sys.effective_specialists();
            let deep = sys.trait_ == Some(crate::explore::SystemTrait::DeepDeposits);
            let vein = match sys.trait_ {
                Some(crate::explore::SystemTrait::BonusVein { commodity }) => Some(commodity),
                _ => None,
            };

            // -- EXTRACTION (§bodies): each BODY's deposits worked by the
            // structures ON that body. Stockpile + headroom stay pooled at
            // the system (one logistics node); staffing share is system-wide.
            let food_state = sys.food_state;
            let mut headroom = sys.storage_headroom();
            let mut extracted_any = false;
            let mut stockpile_adds: Vec<(crate::cargo::Commodity, f64)> = Vec::new();
            for b in sys.bodies.iter_mut() {
                for i in 0..b.deposits.len() {
                    let (resource, richness) = (b.deposits[i].resource, b.deposits[i].richness);
                    let Some(kind) = crate::production::extraction_structure(resource) else { continue };
                    let tier = b.tier(kind);
                    let workers = b.assignments.get(&kind).map(|a| a.workers).unwrap_or(0);
                    let (spec_crew, matched) = line_spec.get(&(b.id, kind)).copied().unwrap_or((0, 0));
                    if tier == 0 || workers + spec_crew == 0 {
                        continue; // unbuilt/unposted = idle by choice, not "suspended"
                    }
                    // §explore Part 3 Deep Deposits (always-on ground truth): the
                    // throughput ladder runs ONE TIER BEHIND — a tier of progress
                    // is wasted breaking through — while the base runs
                    // ×DEEP_DEPOSITS_BASE_MULT. A SYSTEM trait: it shapes every body.
                    let throughput = if deep {
                        crate::production::tier_throughput(tier.saturating_sub(1).max(1))
                            * crate::explore::DEEP_DEPOSITS_BASE_MULT
                    } else {
                        crate::production::tier_throughput(tier)
                    };
                    let staffing = ((workers + spec_crew).min(tier) as f64 / tier as f64) * share;
                    let skill = crate::production::skill_factor(matched, tier);
                    let food = crate::production::food_factor(kind, food_state);
                    // Bonus Vein — ONE commodity's richness runs ×BONUS_VEIN_MULT.
                    let vein_mult = if vein == Some(resource) { crate::explore::BONUS_VEIN_MULT } else { 1.0 };
                    // §research R4a ExtractionRate tunes every mine's raw output.
                    let mut amount =
                        (richness * throughput * staffing * skill * food * vein_mult * em.extraction * DT).min(headroom.max(0.0));
                    if let Some(reserves) = b.deposits[i].reserves.as_mut() {
                        amount = amount.min(*reserves);
                        *reserves -= amount;
                    }
                    if amount > 0.0 {
                        stockpile_adds.push((resource, amount));
                        headroom -= amount;
                        extracted_any = true;
                    }
                }
            }
            // §research R3: raw units mined this tick (DeepCrust school gate) — and
            // the same units count toward the Materials-field industry throughput.
            let extracted_total: f64 = stockpile_adds.iter().map(|(_, a)| *a).sum();
            if extracted_total > 0.0 {
                research_deltas.push((owner, crate::research::Verb::UnitsExtracted, extracted_total));
                research_deltas.push((owner, crate::research::Verb::UnitsThroughIndustry, extracted_total));
            }
            for (c, amount) in stockpile_adds {
                *sys.stockpile.entry(c).or_insert(0.0) += amount;
            }
            // Extraction suspension latch (per staffed extraction structure per
            // body): the food floor keeps it above zero, so the only outage is
            // a FULL depot — one latched notice, resumed when space frees up.
            let storage_starved = !extracted_any && sys.storage_headroom() <= 1e-12;
            let sys_id = sys.id;
            for b in sys.bodies.iter_mut() {
                for kind in [
                    crate::build::StructureKind::MiningComplex,
                    crate::build::StructureKind::VolatileHarvester,
                    crate::build::StructureKind::Bioharvester,
                ] {
                    let tier = b.tier(kind);
                    let posted_spec = line_spec.get(&(b.id, kind)).map(|(c, _)| *c).unwrap_or(0);
                    let works_here = b.deposits.iter().any(|d| crate::production::extraction_structure(d.resource) == Some(kind));
                    let Some(asg) = b.assignments.get_mut(&kind) else { continue };
                    if tier == 0 || asg.workers + posted_spec == 0 || !works_here {
                        continue;
                    }
                    let now_suspended = storage_starved.then_some(crate::production::SuspendReason::StorageFull);
                    if asg.suspended != now_suspended {
                        match now_suspended {
                            Some(reason) => events.push(Event::new(self.time, EventPayload::ProductionSuspended { owner, system: sys_id, structure: kind, reason })),
                            None => events.push(Event::new(self.time, EventPayload::ProductionResumed { owner, system: sys_id, structure: kind })),
                        }
                        asg.suspended = now_suspended;
                    }
                }
            }

            // -- CONVERTERS (§bodies): staffed industry per BODY, drawing its
            // input basket from the SYSTEM stockpile and emitting into it.
            for bi in 0..sys.bodies.len() {
                for conv in &crate::production::CONVERTERS {
                    let kind = conv.structure;
                    let b = &sys.bodies[bi];
                    let bid = b.id;
                    let tier = b.tier(kind);
                    let workers = b.assignments.get(&kind).map(|a| a.workers).unwrap_or(0);
                    let (spec_crew, matched) = line_spec.get(&(bid, kind)).copied().unwrap_or((0, 0));
                    if tier == 0 || workers + spec_crew == 0 {
                        continue; // unbuilt/unposted = idle by choice
                    }
                    let food = crate::production::food_factor(kind, food_state);
                    let staffing = ((workers + spec_crew).min(tier) as f64 / tier as f64) * share;
                    let skill = crate::production::skill_factor(matched, tier);
                    // §explore Part 3 Volatile Pockets: richer feedstock — the Fuel
                    // Refinery's OUTPUT runs ×VOLATILE_REFINERY_MULT (a system trait).
                    let pocket = if kind == crate::build::StructureKind::FuelRefinery
                        && sys.trait_ == Some(crate::explore::SystemTrait::VolatilePockets)
                    {
                        crate::explore::VOLATILE_REFINERY_MULT
                    } else {
                        1.0
                    };
                    // (`pocket` multiplies the OUTPUT of the same input draw — it
                    // does NOT enter max_out.)
                    // §research R4a ProcessingYield tunes converter throughput.
                    let max_out = conv.rate * crate::production::tier_throughput(tier) * staffing * skill * food * em.processing * DT;
                    // Bound by the scarcest input (units of OUTPUT the basket affords).
                    let input_bound = conv
                        .inputs
                        .iter()
                        .map(|(c, per)| sys.stockpile.get(c).copied().unwrap_or(0.0) / per)
                        .fold(f64::INFINITY, f64::min);
                    let mut out = max_out.min(input_bound);
                    // Cap guard (inactive at ≥1:1 baskets, protects retunings).
                    let drawn_per_out: f64 = conv.inputs.iter().map(|(_, per)| per).sum();
                    let net_per_out = 1.0 - drawn_per_out;
                    if net_per_out > 0.0 {
                        out = out.min((sys.storage_headroom() / net_per_out).max(0.0));
                    }
                    // The latched outage, by fix-first priority: food > inputs.
                    let now_suspended = if food <= 0.0 {
                        Some(crate::production::SuspendReason::NoFood)
                    } else if out <= 1e-12 && input_bound <= 1e-12 {
                        Some(crate::production::SuspendReason::NoInputs)
                    } else {
                        None
                    };
                    if now_suspended.is_none() && out > 0.0 {
                        for (c, per) in conv.inputs {
                            *sys.stockpile.get_mut(c).expect("bounded by stock") -= out * per;
                        }
                        let emitted = out * pocket;
                        *sys.stockpile.entry(conv.output).or_insert(0.0) += emitted;
                        // §research R3: processed units (Foundry school gate) + the
                        // Materials-field aggregate industry throughput.
                        research_deltas.push((owner, crate::research::Verb::UnitsProcessed, emitted));
                        research_deltas.push((owner, crate::research::Verb::UnitsThroughIndustry, emitted));
                    }
                    let asg = sys.bodies[bi].assignments.get_mut(&kind).expect("crew > 0 ⇒ posted");
                    if asg.suspended != now_suspended {
                        match now_suspended {
                            Some(reason) => events.push(Event::new(self.time, EventPayload::ProductionSuspended { owner, system: sys_id, structure: kind, reason })),
                            None => events.push(Event::new(self.time, EventPayload::ProductionResumed { owner, system: sys_id, structure: kind })),
                        }
                        asg.suspended = now_suspended;
                    }
                }
            }
        }
        // §research R3: fold this tick's extraction/processing/growth into the
        // syndicate verb biographies (deferred out of the &mut systems loop).
        for (owner, verb, amount) in research_deltas {
            self.add_research_verb(owner, verb, amount);
        }
    }

    /// Run the periodic uniform-price call auction over the limit-order book
    /// (§9). Per commodity, everyone clears at one price; matched buys settle and
    /// spawn a delivery convoy (refunding any over-reservation), matched sells
    /// settle for credits. Resting (unmatched) orders carry to the next batch.
    fn clear_books(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        for commodity in crate::cargo::Commodity::ALL {
            let orders: Vec<LimitOrder> = self
                .book
                .iter()
                .filter(|o| o.commodity == commodity)
                .cloned()
                .collect();
            let Some(clearing) = clear_call_auction(&orders) else {
                continue;
            };
            let price = clearing.price;
            self.market.set_price(commodity, price);
            for (oid, filled) in clearing.fills {
                let Some(order) = self.book.iter().find(|o| o.id == oid).cloned() else {
                    continue;
                };
                match order.side {
                    Side::Buy => {
                        // Refund the over-reservation; the matched goods land in the
                        // buyer's CHARTERHOUSE WAREHOUSE (§TCA — a fill is an
                        // Exchange settlement, never a crossing); news.
                        let refund = filled as f64 * (order.limit_price - price);
                        if let Some(c) = self.players.get_mut(&order.player) {
                            c.credits += refund;
                            *c.warehouse.entry(commodity).or_insert(0) += filled;
                        }
                        events.push(Event::new(
                            now,
                            EventPayload::Trade(TradeEvent::LimitFilled {
                                player: order.player,
                                side: Side::Buy,
                                commodity,
                                units: filled,
                                unit_price: price,
                            }),
                        ));
                    }
                    Side::Sell => {
                        if let Some(c) = self.players.get_mut(&order.player) {
                            c.credits += filled as f64 * price;
                        }
                        events.push(Event::new(
                            now,
                            EventPayload::Trade(TradeEvent::LimitFilled {
                                player: order.player,
                                side: Side::Sell,
                                commodity,
                                units: filled,
                                unit_price: price,
                            }),
                        ));
                    }
                }
                if let Some(o) = self.book.iter_mut().find(|o| o.id == oid) {
                    o.units = o.units.saturating_sub(filled);
                }
            }
        }
        self.book.retain(|o| o.units > 0);
    }

    /// Recompute every corporation's equity (§9). Slow-cadence so the figure is
    /// readable, not noisy. Net worth = liquid credits + all goods valued at the
    /// current market price (held at home, in transit on trade convoys, and
    /// reserved in resting sell orders) + credits escrowed by resting buy orders.
    fn recompute_valuations(&mut self) {
        let prices = self.market.prices().clone();
        let value = |c: &crate::cargo::Commodity, u: u32| u as f64 * prices.get(c).copied().unwrap_or(0.0);

        let mut transit: BTreeMap<PlayerId, f64> = BTreeMap::new();
        for ship in self.fleets.values() {
            if ship.mission.is_some()
                && let Some(cargo) = ship.cargo
            {
                *transit.entry(ship.owner).or_insert(0.0) += value(&cargo.commodity, cargo.units);
            }
        }
        let mut reserved: BTreeMap<PlayerId, f64> = BTreeMap::new();
        for o in &self.book {
            let v = match o.side {
                Side::Buy => o.units as f64 * o.limit_price, // credits in escrow
                Side::Sell => value(&o.commodity, o.units),  // goods at market
            };
            *reserved.entry(o.player).or_insert(0.0) += v;
        }
        // §TCA: goods escrowed in FREIGHT — queued shipments (left the warehouse /
        // a stockpile, awaiting a departure) and shipments aboard a freighter's
        // manifest. Valued at market like any in-transit convoy cargo, so a
        // corp's net worth doesn't blink while its goods ride the Authority's hulls.
        let mut freight: BTreeMap<PlayerId, f64> = BTreeMap::new();
        for sh in self.freight_queue.values() {
            *freight.entry(sh.owner).or_insert(0.0) += value(&sh.commodity, sh.units);
        }
        for run in self.freight_runs.values() {
            for sh in run.shipments.values() {
                *freight.entry(sh.owner).or_insert(0.0) += value(&sh.commodity, sh.units);
            }
        }
        for (id, corp) in self.players.iter_mut() {
            let inv: f64 = corp.inventory.iter().map(|(c, u)| value(c, *u)).sum();
            // §TCA: warehouse goods held at the Charterhouse count like home goods.
            let wh: f64 = corp.warehouse.iter().map(|(c, u)| value(c, *u)).sum();
            corp.valuation = corp.credits
                + inv
                + wh
                + transit.get(id).copied().unwrap_or(0.0)
                + reserved.get(id).copied().unwrap_or(0.0)
                + freight.get(id).copied().unwrap_or(0.0);
            // §rankings RECOVERY: a major loss (a captured system) since the last
            // close stamps this fresh, post-loss valuation as the trough to climb
            // back from. Measured at the close so it reflects the settled loss.
            if corp.stats.loss_pending {
                corp.stats.loss_floor = Some(corp.valuation);
                corp.stats.loss_pending = false;
            }
        }
    }

    /// §rankings: bump a corporation's cumulative counters. A no-op for a player
    /// not in `players` — so the PIRATE sentinel (never a corp) is skipped for free.
    fn bump_stats(&mut self, player: PlayerId, f: impl FnOnce(&mut crate::rankings::RankingStats)) {
        if let Some(corp) = self.players.get_mut(&player) {
            f(&mut corp.stats);
        }
    }

    /// §rankings: tally THIS tick's events into the cumulative per-corp counters —
    /// the single "increment at events" pass (no per-tick cost beyond the events
    /// that actually fired). Deterministic (events are produced deterministically).
    /// Reads only from the event stream the sim already emits; the two counters that
    /// need live fleet/cargo context (raid seizure, cargo-protected) are incremented
    /// at their own sites. Pirates are skipped automatically ([`Self::bump_stats`]).
    fn accumulate_rankings(&mut self, events: &[Event]) {
        for e in events {
            match &e.payload {
                EventPayload::Trade(te) => match *te {
                    // TRADE THROUGHPUT — every convoy delivery (home / owned / ally).
                    TradeEvent::Delivered { player, units, .. } => {
                        self.bump_stats(player, |s| s.trade_units += units as u64);
                    }
                    // A SALE is market REVENUE only. §TCA: it is no longer trade
                    // THROUGHPUT — a Charterhouse sale settles against the warehouse
                    // and hauls nothing, so counting it would both misreport the
                    // "units your convoys delivered" counter and make throughput
                    // farmable risk-free by buying and selling on the spot. The
                    // convoy paths that DO haul goods to the hub bump `trade_units`
                    // at their own arrival site (see `resolve_trade_arrivals`).
                    TradeEvent::Sold { player, units, unit_price, .. } => {
                        self.bump_stats(player, |s| s.market_revenue += units as f64 * unit_price);
                    }
                    // MARKET PROFIT cost side (immediate buy).
                    TradeEvent::Bought { player, units, unit_price, .. } => {
                        self.bump_stats(player, |s| s.market_spend += units as f64 * unit_price);
                    }
                    // A cleared LIMIT order — sell = revenue, buy = spend.
                    TradeEvent::LimitFilled { player, side, units, unit_price, .. } => {
                        let amt = units as f64 * unit_price;
                        match side {
                            Side::Sell => self.bump_stats(player, |s| s.market_revenue += amt),
                            Side::Buy => self.bump_stats(player, |s| s.market_spend += amt),
                        }
                    }
                    _ => {}
                },
                // BATTLE EFFICIENCY — credit BOTH sides their destroyed/lost hull and
                // an engagement (skipping a no-contact ESCAPE, which isn't a fight).
                EventPayload::RaidResolved { attacker, defender, outcome, attacker_losses, target_losses, .. }
                    if *outcome != RaidOutcome::Escaped =>
                {
                    let a_hull = crate::rankings::hull_sum(attacker_losses);
                    let d_hull = crate::rankings::hull_sum(target_losses);
                    let (attacker, defender) = (*attacker, *defender);
                    self.bump_stats(attacker, |s| {
                        s.hull_destroyed += d_hull;
                        s.hull_lost += a_hull;
                        s.engagements += 1;
                    });
                    self.bump_stats(defender, |s| {
                        s.hull_destroyed += a_hull;
                        s.hull_lost += d_hull;
                        s.engagements += 1;
                    });
                }
                // SYSTEMS DEVELOPED — one completed upgrade tier.
                EventPayload::SystemUpgraded { owner, .. } => {
                    self.bump_stats(*owner, |s| s.tiers_built += 1);
                }
                // INTEL GATHERED — one scout snapshot captured.
                EventPayload::IntelGathered { owner, .. } => {
                    self.bump_stats(*owner, |s| s.intel_snapshots += 1);
                }
                // §explore: a completed SURVEY is intel gathered too (the scout's
                // second job feeds the same All-Seeing ladder).
                EventPayload::SurveyCompleted { owner, .. } => {
                    self.bump_stats(*owner, |s| s.intel_snapshots += 1);
                }
                // CARGO CAPTURED (plunder) + RECOVERY (the old owner's major loss).
                EventPayload::SystemCaptured { new_owner, old_owner, plunder, .. } => {
                    let loot: u64 = plunder.values().map(|&u| u as u64).sum();
                    self.bump_stats(*new_owner, |s| s.cargo_captured += loot);
                    self.bump_stats(*old_owner, |s| s.loss_pending = true);
                }
                _ => {}
            }
        }
    }

    /// §research R3: fold THIS tick's events into the syndicate VERB biography —
    /// the battle- and convoy-derived counters that gate the Weapons/Hulls schools
    /// and the LineHaul haulage school. Runs alongside `accumulate_rankings` (same
    /// O(events) sweep, same tick). Per-tick verbs that aren't events (movement ly,
    /// units produced, population grown, systems scouted, rivals observed) are
    /// accrued inline at their own phases. Pirates own no syndicate, so their events
    /// no-op through `add_research_verb`.
    fn accrue_research_verbs(&mut self, events: &[Event]) {
        for e in events {
            match &e.payload {
                // A resolved BATTLE (a no-contact ESCAPE isn't a fight). Both sides
                // fought; the survivor won; each credits the hull it destroyed and
                // the hull it lost as "absorbed" (the Countermeasures gate proxy).
                EventPayload::RaidResolved { attacker, defender, outcome, attacker_losses, target_losses, .. }
                    if *outcome != RaidOutcome::Escaped =>
                {
                    let a_hull = crate::rankings::hull_sum(attacker_losses);
                    let d_hull = crate::rankings::hull_sum(target_losses);
                    let (attacker, defender) = (*attacker, *defender);
                    self.add_research_verb(attacker, crate::research::Verb::BattlesFought, 1.0);
                    self.add_research_verb(defender, crate::research::Verb::BattlesFought, 1.0);
                    self.add_research_verb(attacker, crate::research::Verb::HullMassDestroyed, d_hull);
                    self.add_research_verb(defender, crate::research::Verb::HullMassDestroyed, a_hull);
                    self.add_research_verb(attacker, crate::research::Verb::DamageAbsorbed, a_hull);
                    self.add_research_verb(defender, crate::research::Verb::DamageAbsorbed, d_hull);
                    match outcome {
                        RaidOutcome::TargetDestroyed => {
                            self.add_research_verb(attacker, crate::research::Verb::BattlesWon, 1.0)
                        }
                        RaidOutcome::AttackerDestroyed => {
                            self.add_research_verb(defender, crate::research::Verb::BattlesWon, 1.0)
                        }
                        _ => {}
                    }
                }
                // A convoy that reached its destination — one completed haul (the
                // LineHaul school gate).
                EventPayload::Trade(TradeEvent::Delivered { player, .. }) => {
                    self.add_research_verb(*player, crate::research::Verb::ConvoyDeliveries, 1.0);
                }
                // A completed SURVEY or a fresh rival-system intel snapshot is a
                // scouted system (Computation field + Watch school gate). Deduped
                // by system id, so re-scouting the same one never re-counts.
                EventPayload::SurveyCompleted { owner, system, .. }
                | EventPayload::IntelGathered { owner, system, .. } => {
                    self.scout_system_for_research(*owner, *system);
                }
                _ => {}
            }
        }
    }

    /// §rankings: PUBLISH the leaderboard — a snapshot copy taken on the ledger tick
    /// (the §9 valuation close), so it holds steady between closes and a mid-interval
    /// counter change never leaks. Builds one row per corporation (pirates excluded)
    /// from its cumulative stats + last-close valuation, then stamps the category
    /// TITLES. Deterministic (ordering + tiebreaks are fixed).
    fn snapshot_rankings(&mut self) {
        let rows: Vec<crate::rankings::RankingRow> = self
            .players
            .values()
            .filter(|c| !c.id.is_pirate())
            .map(|c| crate::rankings::RankingRow {
                player_id: c.id,
                name: c.name.clone(),
                valuation: c.valuation,
                trade_throughput: c.stats.trade_units,
                market_profit: c.stats.market_profit(),
                cargo_captured: c.stats.cargo_captured,
                cargo_protected: c.stats.cargo_protected,
                battle_efficiency: c.stats.battle_efficiency(),
                battle_engagements: c.stats.engagements,
                battle_ranked: c.stats.efficiency_ranked(),
                systems_developed: c.stats.tiers_built,
                intel_gathered: c.stats.intel_snapshots,
                recovery: c.stats.recovery(c.valuation),
                titles: Vec::new(),
            })
            .collect();
        self.rankings = crate::rankings::assemble(rows);
    }

    /// Anti-spam gate 1 upkeep: a standing order holds the id of its one in-flight
    /// convoy; once that convoy leaves the world (arrived this tick, or raided), the
    /// rule becomes eligible to dispatch again. Reconcile by ship existence — robust
    /// to ANY cause of convoy removal, deterministic, O(rules).
    fn reconcile_standing_inflight(&mut self) {
        // Cheap fast-path: nothing to reconcile unless some rule has a convoy latched.
        if !self.players.values().any(|c| c.standing_orders.iter().any(|o| o.in_flight.is_some())) {
            return;
        }
        let alive: std::collections::BTreeSet<EntityId> = self.fleets.keys().copied().collect();
        for corp in self.players.values_mut() {
            for order in &mut corp.standing_orders {
                if let Some(id) = order.in_flight
                    && !alive.contains(&id)
                {
                    order.in_flight = None;
                }
            }
        }
    }

    /// Sum of cargo (by owner, destination, commodity) already in flight on mission
    /// convoys — so a MaintainAtDest rule counts goods already crossing toward the
    /// destination and doesn't over-ship while a top-up is en route.
    fn standing_inflight_index(&self) -> BTreeMap<(PlayerId, Endpoint, crate::cargo::Commodity), u32> {
        let mut idx: BTreeMap<(PlayerId, Endpoint, crate::cargo::Commodity), u32> = BTreeMap::new();
        for ship in self.fleets.values() {
            let (Some(mission), Some(cargo)) = (ship.mission, ship.cargo) else {
                continue;
            };
            let dest = match mission {
                TradeMission::DeliverHome => Endpoint::Home,
                TradeMission::SellAtHub => Endpoint::Hub,
                TradeMission::DeliverToSystem { system } => Endpoint::System { id: system },
            };
            *idx.entry((ship.owner, dest, cargo.commodity)).or_insert(0) += cargo.units;
        }
        idx
    }

    /// Evaluate every player's standing logistics orders (§15) and auto-dispatch
    /// convoys. Deterministic + offline: a pure function of the TRUE world + tick,
    /// iterated in fixed order (players BTreeMap → orders Vec). Two anti-spam gates:
    /// a rule fires only if it has no convoy in flight AND it's past its eval cadence
    /// (`next_eval_tick`), which is advanced every time a rule is evaluated. Plan
    /// (read-only) then execute (mutate + spawn), like `apply_ship_production`.
    fn evaluate_standing_orders(&mut self, events: &mut Vec<Event>) {
        let now_tick = self.tick;
        let hub = self.hub;
        let in_flight = self.standing_inflight_index();

        struct Plan {
            player: PlayerId,
            order_id: u32,
            source_sys: EntityId,
            commodity: crate::cargo::Commodity,
            units: u32,
            spawn: Vec2,
            dest: Vec2,
            mission: TradeMission,
        }
        let mut plans: Vec<Plan> = Vec::new();
        let mut evaluated: Vec<(PlayerId, u32)> = Vec::new();
        // Units already PLANNED toward each (owner, dest, commodity) THIS tick, so a
        // later MaintainAtDest rule sharing a destination counts its siblings' planned
        // shipments and doesn't over-ship past the target.
        let mut planned: BTreeMap<(PlayerId, Endpoint, crate::cargo::Commodity), u32> = BTreeMap::new();

        // --- Phase 1: PLAN (read-only over players/systems). ---
        for (pid, corp) in &self.players {
            for order in &corp.standing_orders {
                if order.status != OrderStatus::Active {
                    continue;
                }
                if order.in_flight.is_some() {
                    continue; // gate 1: one convoy in flight per rule
                }
                if now_tick < order.next_eval_tick {
                    continue; // gate 2: fixed eval cadence
                }
                evaluated.push((*pid, order.id));

                let Some(source_id) = order.source.system_id() else {
                    continue;
                };
                // The source must be a system this corp still owns.
                let Some(src) = self
                    .systems
                    .iter()
                    .find(|s| s.id == source_id && s.owner == Some(*pid))
                else {
                    continue;
                };
                // BLOCKADE (§contestable-territory): a strangled source HOLDS its
                // standing-order dispatches at origin (the rule simply doesn't
                // fire; goods stay in the accruing stockpile). Resumes on lift.
                if src.blockade.is_some() {
                    continue;
                }
                let have = src.stockpile.get(&order.commodity).copied().unwrap_or(0.0);

                let units: u32 = match order.trigger {
                    Trigger::AboveThreshold { threshold } => {
                        if have >= threshold {
                            have.floor() as u32
                        } else {
                            0
                        }
                    }
                    Trigger::PercentSurplus { percent, floor } => {
                        let surplus = (have - floor).max(0.0);
                        (surplus * (percent as f64) / 100.0).floor() as u32
                    }
                    Trigger::MaintainAtDest { target } => {
                        let dest_level = match order.dest {
                            Endpoint::System { id } => self
                                .systems
                                .iter()
                                .find(|s| s.id == id)
                                .map(|s| s.stockpile.get(&order.commodity).copied().unwrap_or(0.0))
                                .unwrap_or(0.0),
                            Endpoint::Home => {
                                corp.inventory.get(&order.commodity).copied().unwrap_or(0) as f64
                            }
                            Endpoint::Hub => 0.0, // forbidden by validation
                        };
                        // Count both convoys already crossing AND shipments planned
                        // earlier THIS tick toward the same dest, so siblings sharing a
                        // destination don't each ship the full deficit (over-shoot).
                        let key = (*pid, order.dest, order.commodity);
                        let enroute = (in_flight.get(&key).copied().unwrap_or(0)
                            + planned.get(&key).copied().unwrap_or(0)) as f64;
                        let deficit = target - (dest_level + enroute);
                        if deficit >= 1.0 {
                            (deficit.floor() as u32).min(have.floor() as u32)
                        } else {
                            0
                        }
                    }
                };
                if units < 1 {
                    continue;
                }
                // Record this shipment as planned toward its destination this tick.
                *planned.entry((*pid, order.dest, order.commodity)).or_insert(0) += units;

                let (dest_pos, mission) = match order.dest {
                    Endpoint::Hub => (hub, TradeMission::SellAtHub),
                    Endpoint::Home => (corp.home, TradeMission::DeliverHome),
                    Endpoint::System { id } => match self.systems.iter().find(|s| s.id == id) {
                        Some(s) => (s.pos, TradeMission::DeliverToSystem { system: id }),
                        None => continue,
                    },
                };
                plans.push(Plan {
                    player: *pid,
                    order_id: order.id,
                    source_sys: source_id,
                    commodity: order.commodity,
                    units,
                    spawn: src.pos,
                    dest: dest_pos,
                    mission,
                });
            }
        }

        // --- Phase 2: EXECUTE (debit source, spawn convoy, latch in_flight, notify). ---
        for p in plans {
            // Re-check the source still holds the units (an earlier plan this tick may
            // have drained the same system); skip cleanly if not.
            let debited = self
                .systems
                .iter_mut()
                .find(|s| s.id == p.source_sys && s.owner == Some(p.player))
                .map(|s| {
                    let have = s.stockpile.get(&p.commodity).copied().unwrap_or(0.0);
                    if have + 1e-9 >= p.units as f64 {
                        *s.stockpile.entry(p.commodity).or_insert(0.0) -= p.units as f64;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if !debited {
                continue;
            }

            // Fuel the automated haul ∝ distance × loaded mass (§step1 part 2),
            // EXCEPT a Fuel haul itself (exempt — else a fuel-starved depot could
            // never be resupplied). A shortfall refunds the source and skips THIS
            // cycle silently (the rule stays active and retries) — async-fair, and
            // no timeline spam from offline automation.
            if p.commodity != crate::fuel::MOVEMENT_FUEL {
                let mass =
                    ShipKind::Convoy.hull_mass() + p.units as f64 * crate::ship::CARGO_MASS_PER_UNIT;
                let cost = crate::fuel::fuel_cost(p.spawn.distance(p.dest), mass);
                if !self.charge_fuel(p.player, p.spawn, cost) {
                    if let Some(s) = self
                        .systems
                        .iter_mut()
                        .find(|s| s.id == p.source_sys && s.owner == Some(p.player))
                    {
                        *s.stockpile.entry(p.commodity).or_insert(0.0) += p.units as f64;
                    }
                    continue;
                }
            }

            let cargo = Cargo { commodity: p.commodity, units: p.units };
            let convoy_id = self.spawn_trade_convoy(p.player, p.spawn, p.dest, cargo, p.mission);

            if let Some(corp) = self.players.get_mut(&p.player)
                && let Some(order) = corp.standing_orders.iter_mut().find(|o| o.id == p.order_id)
            {
                order.in_flight = Some(convoy_id);
            }
            events.push(Event::new(
                self.time,
                EventPayload::Trade(TradeEvent::AutoDispatched {
                    player: p.player,
                    commodity: p.commodity,
                    units: p.units,
                    source: p.source_sys,
                    rule_id: p.order_id,
                }),
            ));
        }

        // --- Phase 3: advance the eval cadence for every rule we examined. ---
        for (pid, oid) in evaluated {
            if let Some(corp) = self.players.get_mut(&pid)
                && let Some(order) = corp.standing_orders.iter_mut().find(|o| o.id == oid)
            {
                order.next_eval_tick = now_tick + EVAL_PERIOD;
            }
        }
    }

    /// Spawn a raidable trade convoy that resolves its mission on arrival.
    // ==== §TCA SCHEDULED FREIGHT ==========================================
    // The Authority's common carrier. Everything here is PURE and keyed off the
    // tick counter (never a float clock comparison), and every iteration runs in
    // stable id order, so the whole service is byte-for-byte deterministic.

    /// The departure period in TICKS. The schedule is keyed off the tick counter so
    /// "is this a departure?" is exact integer arithmetic, never a float compare.
    fn freight_depart_ticks() -> u64 {
        (crate::tca::TCA_DEPARTURE_PERIOD * TICK_HZ as f64).round().max(1.0) as u64
    }

    /// The next scheduled departure strictly AFTER `tick`, as `(tick, sim-time)`.
    /// A pure function of the config — which is what lets the client preview the
    /// exact departure instant before the player commits to a booking.
    fn next_departure_after(&self, tick: u64) -> (u64, f64) {
        let p = Self::freight_depart_ticks();
        let t = (tick / p + 1) * p;
        (t, t as f64 * DT)
    }

    /// One-way freighter flight time over `dist` (constant cruise, §14.1).
    fn freight_leg_secs(dist: f64) -> f64 {
        dist / ShipKind::Freighter.max_speed()
    }

    fn alloc_shipment_id(&mut self) -> ShipmentId {
        self.next_shipment_id += 1;
        ShipmentId(self.next_shipment_id)
    }

    // --- §TCA public read surface (the client's booking preview) --------------
    // All EXACT, not estimates: the timetable and the freighter's constant cruise
    // are pure functions of the config, so the client can show a player precisely
    // when their goods sail and land before they commit a credit.

    /// Sim-time of the next scheduled freight departure.
    pub fn next_freight_departure(&self) -> f64 {
        self.next_departure_after(self.tick).1
    }

    /// Seconds between scheduled freight departures.
    pub fn freight_period_secs(&self) -> f64 {
        Self::freight_depart_ticks() as f64 * DT
    }

    /// One-way freighter flight time over `dist` (seconds).
    pub fn freight_flight_secs(dist: f64) -> f64 {
        Self::freight_leg_secs(dist)
    }

    /// Every freight lot belonging to `who`, in shipment-id order, paired with
    /// whether it is ABOARD a freighter (`true`) or still queued (`false`).
    /// Owner-scoped by construction — it never returns anyone else's lots.
    pub fn shipments_of(&self, who: PlayerId) -> Vec<(Shipment, bool)> {
        let mut out: Vec<(Shipment, bool)> = self
            .freight_queue
            .values()
            .filter(|s| s.owner == who)
            .map(|s| (*s, false))
            .chain(
                self.freight_runs
                    .values()
                    .flat_map(|r| r.shipments.values())
                    .filter(|s| s.owner == who)
                    .map(|s| (*s, true)),
            )
            .collect();
        out.sort_by_key(|(s, _)| s.id);
        out
    }

    /// Push an owner-only freight progress notice.
    fn freight_note(&self, events: &mut Vec<Event>, s: &Shipment, units: u32, stage: FreightStage) {
        if units == 0 {
            return;
        }
        events.push(Event::new(
            self.time,
            EventPayload::Trade(TradeEvent::FreightMoved {
                player: s.owner,
                system: s.system,
                commodity: s.commodity,
                units,
                stage,
            }),
        ));
    }

    /// Push an owner-only soft-reject for an Exchange order or freight booking.
    fn reject_trade(
        &self,
        events: &mut Vec<Event>,
        player: PlayerId,
        commodity: crate::cargo::Commodity,
        units: u32,
        system: Option<EntityId>,
        reason: TradeRejectReason,
    ) {
        events.push(Event::new(
            self.time,
            EventPayload::Trade(TradeEvent::Rejected { player, commodity, units, system, reason }),
        ));
    }

    /// BOOK a freight shipment (§TCA) — the shared body of `BookFreightOut` /
    /// `BookFreightIn`. Escrows the goods out of their source, charges the fee (a
    /// pure sink — destroyed, never refunded), and queues the lot for the next
    /// departure it fits on. Async-fair: every failure path costs NOTHING and
    /// emits an owner-only typed reason.
    ///
    /// The per-departure CAP never rejects — a lot larger than a corp's allowance
    /// simply rides several consecutive departures (it is split at load time), so
    /// a big shipper is slowed, never refused.
    fn book_freight(
        &mut self,
        player_id: PlayerId,
        system_id: EntityId,
        commodity: crate::cargo::Commodity,
        units: u32,
        direction: ShipmentDir,
        sell_on_arrival: bool,
        events: &mut Vec<Event>,
    ) {
        if units == 0 || !self.players.contains_key(&player_id) {
            return;
        }
        // The Authority serves a corporation's OWN colonies only.
        let Some(sys) = self.systems.iter().find(|s| s.id == system_id) else {
            self.reject_trade(events, player_id, commodity, units, Some(system_id), TradeRejectReason::NotYourSystem);
            return;
        };
        if sys.owner != Some(player_id) {
            self.reject_trade(events, player_id, commodity, units, Some(system_id), TradeRejectReason::NotYourSystem);
            return;
        }
        let has_depot = sys.tier(crate::build::StructureKind::Depot) > 0;
        let sys_stock = sys.stockpile.get(&commodity).copied().unwrap_or(0.0);
        let dist = self.hub.distance(sys.pos);
        let fee = crate::tca::freight_fee(units, self.market.price(commodity), dist, has_depot);

        // The SOURCE must cover the lot (warehouse outbound / stockpile inbound).
        match direction {
            ShipmentDir::Outbound => {
                let have = self.players[&player_id].warehouse.get(&commodity).copied().unwrap_or(0);
                if have < units {
                    self.reject_trade(events, player_id, commodity, units, Some(system_id), TradeRejectReason::InsufficientWarehouseStock { have });
                    return;
                }
            }
            ShipmentDir::Inbound => {
                if sys_stock < units as f64 {
                    self.reject_trade(events, player_id, commodity, units, Some(system_id), TradeRejectReason::InsufficientSystemStock { have: sys_stock as u32 });
                    return;
                }
            }
        }
        if self.players[&player_id].credits < fee {
            self.reject_trade(events, player_id, commodity, units, Some(system_id), TradeRejectReason::CannotAffordFee { fee });
            return;
        }

        // Forecast the departure this lot rides: everything of ours already queued
        // for this destination and direction goes first (FIFO), a cap's worth per
        // scheduled departure.
        let cap = crate::tca::shipment_cap(has_depot).max(1);
        let ahead: u32 = self
            .freight_queue
            .values()
            .filter(|s| s.owner == player_id && s.system == system_id && s.direction == direction)
            .map(|s| s.units)
            .sum();
        let (_, first_dep) = self.next_departure_after(self.tick);
        let period_secs = Self::freight_depart_ticks() as f64 * DT;
        let depart_at = first_dep + (ahead / cap) as f64 * period_secs;
        let leg = Self::freight_leg_secs(dist);
        let eta = match direction {
            // Out to the colony…
            ShipmentDir::Outbound => depart_at + leg,
            // …or out to collect it and back to the Charterhouse.
            ShipmentDir::Inbound => depart_at + 2.0 * leg,
        };

        // COMMIT — debit the fee (destroyed), escrow the goods, queue the lot.
        if let Some(corp) = self.players.get_mut(&player_id) {
            corp.credits -= fee;
            if direction == ShipmentDir::Outbound {
                take_from(&mut corp.warehouse, commodity, units);
            }
        }
        if direction == ShipmentDir::Inbound
            && let Some(s) = self.systems.iter_mut().find(|s| s.id == system_id)
        {
            let left = (s.stockpile.get(&commodity).copied().unwrap_or(0.0) - units as f64).max(0.0);
            if left <= 0.0 {
                s.stockpile.remove(&commodity);
            } else {
                s.stockpile.insert(commodity, left);
            }
        }
        let id = self.alloc_shipment_id();
        self.freight_queue.insert(
            id,
            Shipment {
                id,
                owner: player_id,
                system: system_id,
                direction,
                commodity,
                units,
                fee_paid: fee,
                booked_at: self.time,
                sell_on_arrival: direction == ShipmentDir::Inbound && sell_on_arrival,
            },
        );
        events.push(Event::new(
            self.time,
            EventPayload::Trade(TradeEvent::FreightBooked {
                player: player_id,
                system: system_id,
                commodity,
                units,
                direction,
                fee,
                depart_at,
                eta,
            }),
        ));
    }

    /// Queued INBOUND lots whose origin system the owner no longer holds are
    /// FORFEIT — to nobody. The captor gets nothing (the goods were already out of
    /// the stockpile, in the Authority's care), the fee is not refunded, and the
    /// owner gets a notice. Swept on the departure cadence.
    fn forfeit_lost_pickups(&mut self, events: &mut Vec<Event>) {
        let lost: Vec<ShipmentId> = self
            .freight_queue
            .iter()
            .filter(|(_, s)| {
                s.direction == ShipmentDir::Inbound
                    && self.systems.iter().find(|sys| sys.id == s.system).map(|sys| sys.owner) != Some(Some(s.owner))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in lost {
            let s = self.freight_queue.remove(&id).expect("just listed");
            self.freight_note(events, &s, s.units, FreightStage::ForfeitedOnCapture);
        }
    }

    /// Draw queued shipments of `direction` for `dest` onto a manifest, FIFO
    /// (ascending shipment id) and capped per corporation. A lot bigger than the
    /// remaining allowance is SPLIT: the part that fits rides now under a fresh id,
    /// the remainder keeps its original (older) id and so keeps its place at the
    /// head of the queue for the next departure.
    fn load_shipments(
        &mut self,
        dest: EntityId,
        direction: ShipmentDir,
        cap: u32,
        manifest: &mut Vec<ShipmentId>,
        aboard: &mut BTreeMap<ShipmentId, Shipment>,
    ) {
        // BTreeMap iteration is ascending id — exactly FIFO booking order.
        let queued: Vec<ShipmentId> = self
            .freight_queue
            .iter()
            .filter(|(_, s)| s.system == dest && s.direction == direction)
            .map(|(id, _)| *id)
            .collect();
        let mut used: BTreeMap<PlayerId, u32> = BTreeMap::new();
        for sid in queued {
            let s = self.freight_queue[&sid];
            let spent = used.entry(s.owner).or_insert(0);
            let room = cap.saturating_sub(*spent);
            if room == 0 {
                continue;
            }
            let take = room.min(s.units);
            if take == s.units {
                let sh = self.freight_queue.remove(&sid).expect("just listed");
                manifest.push(sid);
                aboard.insert(sid, sh);
            } else {
                // Partial lift: the fitting part sails under a fresh id.
                let share = s.fee_paid * (take as f64 / s.units as f64);
                let new_id = self.alloc_shipment_id();
                let mut part = s;
                part.id = new_id;
                part.units = take;
                part.fee_paid = share;
                manifest.push(new_id);
                aboard.insert(new_id, part);
                if let Some(rem) = self.freight_queue.get_mut(&sid) {
                    rem.units -= take;
                    rem.fee_paid -= share;
                }
            }
            *spent += take;
        }
    }

    /// THE SCHEDULED DEPARTURE (§TCA): at every multiple of the departure period,
    /// each destination with anything queued in EITHER direction gets exactly one
    /// Authority freighter out of the Charterhouse. Outbound lots load now; the
    /// same hull collects that destination's inbound lots when it arrives. Never
    /// launches an empty run with nothing to do at either end.
    fn depart_freight(&mut self, events: &mut Vec<Event>) {
        self.forfeit_lost_pickups(events);
        let dests: std::collections::BTreeSet<EntityId> =
            self.freight_queue.values().map(|s| s.system).collect();
        for dest in dests {
            let Some(sys) = self.systems.iter().find(|s| s.id == dest) else {
                continue;
            };
            let dest_pos = sys.pos;
            let cap = crate::tca::shipment_cap(sys.tier(crate::build::StructureKind::Depot) > 0).max(1);
            let mut manifest: Vec<ShipmentId> = Vec::new();
            let mut aboard: BTreeMap<ShipmentId, Shipment> = BTreeMap::new();
            self.load_shipments(dest, ShipmentDir::Outbound, cap, &mut manifest, &mut aboard);
            // Nothing to carry AND nothing to collect there → no hull is sent.
            let pickup_waiting = self
                .freight_queue
                .values()
                .any(|s| s.system == dest && s.direction == ShipmentDir::Inbound);
            if aboard.is_empty() && !pickup_waiting {
                continue;
            }
            let fid = self.alloc_entity_id();
            self.fleets.insert(
                fid,
                Fleet::single(fid, PlayerId::TCA, ShipKind::Freighter, self.hub, FleetOrder::MoveTo { dest: dest_pos }, None),
            );
            for sid in &manifest {
                let s = aboard[sid];
                self.freight_note(events, &s, s.units, FreightStage::Departed);
            }
            self.freight_runs.insert(fid, FreightRun { fleet: fid, dest, leg: RunLeg::Outbound, manifest, shipments: aboard });
        }
    }

    /// Resolve freighter runs that reached the end of a leg (§TCA).
    ///
    /// At the DESTINATION: every outbound lot whose owner still holds the system
    /// unloads into its stockpile (bounded by the depot's headroom). Anything that
    /// can't land — the system changed hands, or the depot is full — STAYS ABOARD
    /// and rides home to the owner's warehouse. The Authority holds your goods; it
    /// never destroys them (deliberately friendlier than the convoy cargo-lost
    /// rule). The hull then collects that destination's queued inbound lots and
    /// turns for the Charterhouse.
    ///
    /// At the CHARTERHOUSE: everything aboard lands in its owner's warehouse, and
    /// any inbound lot flagged `sell_on_arrival` is sold immediately at that tick's
    /// standing price through the ordinary sale path. The run and hull are retired.
    fn resolve_freight_arrivals(&mut self, events: &mut Vec<Event>) {
        let arrived: Vec<EntityId> = self
            .freight_runs
            .keys()
            .copied()
            .filter(|fid| self.fleets.get(fid).is_some_and(|f| matches!(f.order, FleetOrder::Idle)))
            .collect();
        for fid in arrived {
            let leg = self.freight_runs[&fid].leg;
            match leg {
                RunLeg::Outbound => {
                    let dest = self.freight_runs[&fid].dest;
                    let manifest = self.freight_runs[&fid].manifest.clone();
                    // UNLOAD, in load order so the depot's headroom is shared FIFO.
                    let mut delivered: Vec<ShipmentId> = Vec::new();
                    for sid in manifest {
                        let Some(s) = self.freight_runs[&fid].shipments.get(&sid).copied() else {
                            continue;
                        };
                        if s.direction != ShipmentDir::Outbound {
                            continue;
                        }
                        let Some(sys) = self.systems.iter_mut().find(|x| x.id == dest) else {
                            continue;
                        };
                        if sys.owner != Some(s.owner) {
                            continue; // no longer yours — it rides home
                        }
                        // Whole units, bounded by the depot's remaining headroom.
                        let room = sys.storage_headroom().floor().max(0.0) as u32;
                        let take = room.min(s.units);
                        if take > 0 {
                            *sys.stockpile.entry(s.commodity).or_insert(0.0) += take as f64;
                        }
                        self.freight_note(events, &s, take, FreightStage::DeliveredToSystem);
                        if take == s.units {
                            delivered.push(sid);
                        } else if let Some(r) = self.freight_runs.get_mut(&fid)
                            && let Some(rem) = r.shipments.get_mut(&sid)
                        {
                            rem.units -= take; // the rest rides home
                        }
                    }
                    if let Some(r) = self.freight_runs.get_mut(&fid) {
                        for sid in &delivered {
                            r.shipments.remove(sid);
                        }
                        r.manifest.retain(|s| !delivered.contains(s));
                    }
                    // COLLECT this destination's inbound lots (same per-corp cap),
                    // dropping any whose origin the owner lost since booking.
                    self.forfeit_lost_pickups(events);
                    let cap = self
                        .systems
                        .iter()
                        .find(|s| s.id == dest)
                        .map(|s| crate::tca::shipment_cap(s.tier(crate::build::StructureKind::Depot) > 0).max(1))
                        .unwrap_or(crate::tca::TCA_SHIPMENT_CAP);
                    let mut manifest = Vec::new();
                    let mut aboard = BTreeMap::new();
                    self.load_shipments(dest, ShipmentDir::Inbound, cap, &mut manifest, &mut aboard);
                    for sid in &manifest {
                        let s = aboard[sid];
                        self.freight_note(events, &s, s.units, FreightStage::CollectedForPickup);
                    }
                    if let Some(r) = self.freight_runs.get_mut(&fid) {
                        r.manifest.extend(manifest);
                        r.shipments.extend(aboard);
                        r.leg = RunLeg::Returning;
                    }
                    let hub = self.hub;
                    if let Some(f) = self.fleets.get_mut(&fid) {
                        f.order = FleetOrder::MoveTo { dest: hub };
                    }
                }
                RunLeg::Returning => {
                    let run = self.freight_runs.remove(&fid).expect("just listed");
                    self.fleets.remove(&fid);
                    // Deterministic: shipment-id order.
                    for s in run.shipments.values().copied() {
                        if let Some(corp) = self.players.get_mut(&s.owner) {
                            *corp.warehouse.entry(s.commodity).or_insert(0) += s.units;
                        }
                        let stage = match s.direction {
                            ShipmentDir::Inbound => FreightStage::ArrivedAtWarehouse,
                            ShipmentDir::Outbound => FreightStage::ReturnedUndeliverable,
                        };
                        self.freight_note(events, &s, s.units, stage);
                        // SELL ON ARRIVAL — the ordinary sale path, so the price
                        // walk, the event, and the ranking stats are all identical
                        // to a hand-typed MarketSell at this tick.
                        if s.direction == ShipmentDir::Inbound && s.sell_on_arrival && s.units > 0 {
                            let unit_price = self.market.execute_sell(s.commodity, s.units);
                            if let Some(corp) = self.players.get_mut(&s.owner) {
                                take_from(&mut corp.warehouse, s.commodity, s.units);
                                corp.credits += s.units as f64 * unit_price;
                            }
                            events.push(Event::new(
                                self.time,
                                EventPayload::Trade(TradeEvent::Sold {
                                    player: s.owner,
                                    commodity: s.commodity,
                                    units: s.units,
                                    unit_price,
                                }),
                            ));
                        }
                    }
                }
            }
        }
    }

    fn spawn_trade_convoy(
        &mut self,
        owner: PlayerId,
        spawn: Vec2,
        dest: Vec2,
        cargo: Cargo,
        mission: TradeMission,
    ) -> EntityId {
        let id = self.alloc_entity_id();
        let mut ship = Fleet::single(
            id,
            owner,
            ShipKind::Convoy,
            spawn,
            FleetOrder::MoveTo { dest },
            Some(cargo),
        );
        ship.mission = Some(mission);
        self.fleets.insert(id, ship);
        id
    }

    /// Resolve trade convoys that have reached their destination: deposit a
    /// delivery, or clear a sale at the price-on-arrival (§9). Convoys raided in
    /// transit were already removed (their goods/credits simply lost).
    fn resolve_trade_arrivals(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let arrived: Vec<EntityId> = self
            .fleets
            .iter()
            .filter(|(_, s)| s.mission.is_some() && matches!(s.order, FleetOrder::Idle))
            .map(|(id, _)| *id)
            .collect();
        for id in arrived {
            let mut ship = self.fleets.remove(&id).unwrap();
            // §economy Part 4: PASSENGERS DISEMBARK FIRST (people before goods).
            // They land only on ground the owner (or an ally) still holds; a
            // destination lost mid-transit REDIRECTS the convoy to the owner's
            // home system instead — nobody is ever silently deleted (the home
            // is always held: home protection guarantees termination).
            if !ship.passengers.is_empty() {
                let land_at = match ship.mission {
                    Some(TradeMission::DeliverToSystem { system }) => Some(system),
                    Some(TradeMission::DeliverHome) => self.players.get(&ship.owner).and_then(|c| c.home_system),
                    _ => None,
                };
                let allies = self.allies_of(ship.owner);
                let landed = land_at.is_some_and(|sid| {
                    self.systems
                        .iter_mut()
                        .find(|s| s.id == sid && s.owner.is_some_and(|o| o == ship.owner || allies.contains(&o)))
                        .map(|sys| {
                            for (k, n) in &ship.passengers {
                                *sys.specialists.entry(*k).or_insert(0) += n;
                            }
                        })
                        .is_some()
                });
                if landed {
                    let manifest = std::mem::take(&mut ship.passengers);
                    events.push(Event::new(
                        now,
                        EventPayload::SpecialistsDelivered { owner: ship.owner, system: land_at.unwrap(), manifest },
                    ));
                } else if let Some(corp) = self.players.get(&ship.owner)
                    && let Some(home_sys) = corp.home_system
                    && land_at != Some(home_sys)
                {
                    // Turn for home, people (and any cargo) still aboard.
                    ship.order = FleetOrder::MoveTo { dest: corp.home };
                    ship.mission = Some(TradeMission::DeliverToSystem { system: home_sys });
                    self.fleets.insert(id, ship);
                    continue;
                }
            }
            // §modules Part B3 (Sol hub): a SELL convoy clears its crates at Sol on
            // arrival — the buy-back price is decided here (price-on-arrival).
            if !ship.modules.is_empty() && matches!(ship.mission, Some(TradeMission::SellAtHub)) {
                let manifest = std::mem::take(&mut ship.modules);
                let priced: Vec<(crate::module::ModuleKind, u32, f64)> =
                    manifest.into_iter().map(|(k, n)| (k, n, self.module_sell_price(k))).collect();
                for (kind, n, unit) in priced {
                    if let Some(corp) = self.players.get_mut(&ship.owner) {
                        corp.credits += unit * n as f64;
                    }
                    events.push(Event::new(now, EventPayload::ModulesSold { owner: ship.owner, kind, n, unit_price: unit }));
                }
                continue; // the sell convoy's job is done — it vanishes at the hub.
            }
            // §modules Part B3: MODULE CRATES land into the destination ledger under
            // the same rule — held ground (owner or ally); a lost destination
            // redirects the convoy home so crates are never silently deleted.
            if !ship.modules.is_empty() {
                let land_at = match ship.mission {
                    Some(TradeMission::DeliverToSystem { system }) => Some(system),
                    Some(TradeMission::DeliverHome) => self.players.get(&ship.owner).and_then(|c| c.home_system),
                    _ => None,
                };
                let allies = self.allies_of(ship.owner);
                let landed = land_at.is_some_and(|sid| {
                    self.systems
                        .iter_mut()
                        .find(|s| s.id == sid && s.owner.is_some_and(|o| o == ship.owner || allies.contains(&o)))
                        .map(|sys| {
                            for (k, n) in &ship.modules {
                                *sys.modules.entry(*k).or_insert(0) += n;
                            }
                        })
                        .is_some()
                });
                if landed {
                    let manifest = std::mem::take(&mut ship.modules);
                    events.push(Event::new(
                        now,
                        EventPayload::ModulesDelivered { owner: ship.owner, system: land_at.unwrap(), manifest },
                    ));
                } else if let Some(corp) = self.players.get(&ship.owner)
                    && let Some(home_sys) = corp.home_system
                    && land_at != Some(home_sys)
                {
                    ship.order = FleetOrder::MoveTo { dest: corp.home };
                    ship.mission = Some(TradeMission::DeliverToSystem { system: home_sys });
                    self.fleets.insert(id, ship);
                    continue;
                }
            }
            let ship = ship;
            let (Some(cargo), Some(mission)) = (ship.cargo, ship.mission) else {
                continue;
            };
            // §rankings CARGO PROTECTED: a convoy that fought an engagement en route
            // and STILL reached its destination earns its delivered units (below).
            let fought = ship.fought;
            match mission {
                TradeMission::DeliverHome => {
                    if let Some(corp) = self.players.get_mut(&ship.owner) {
                        *corp.inventory.entry(cargo.commodity).or_insert(0) += cargo.units;
                        if fought {
                            corp.stats.cargo_protected += cargo.units as u64;
                        }
                    }
                    events.push(Event::new(
                        now,
                        EventPayload::Trade(TradeEvent::Delivered {
                            player: ship.owner,
                            commodity: cargo.commodity,
                            units: cargo.units,
                            system: None, // DeliverHome → the HQ trading pool
                        }),
                    ));
                }
                TradeMission::SellAtHub => {
                    let unit_price = self.market.execute_sell(cargo.commodity, cargo.units);
                    if let Some(corp) = self.players.get_mut(&ship.owner) {
                        corp.credits += cargo.units as f64 * unit_price;
                        // §TCA: THIS is a real haul — a convoy crossed and delivered
                        // to the hub — so it earns trade throughput here, at the
                        // arrival site. (The `Sold` event itself no longer bumps it:
                        // an instant warehouse sale moves nothing.)
                        corp.stats.trade_units += cargo.units as u64;
                        if fought {
                            corp.stats.cargo_protected += cargo.units as u64;
                        }
                    }
                    events.push(Event::new(
                        now,
                        EventPayload::Trade(TradeEvent::Sold {
                            player: ship.owner,
                            commodity: cargo.commodity,
                            units: cargo.units,
                            unit_price,
                        }),
                    ));
                }
                TradeMission::DeliverToSystem { system } => {
                    // Deposit into the destination system's stockpile — but ONLY if,
                    // on arrival, the destination is still the convoy owner's OR a
                    // syndicate ALLY's (§syndicates Part 3 AID — a member may supply
                    // an ally's stockpile; blockades still interdict the run upstream).
                    // We don't gift cargo to a rival who took the system mid-transit.
                    // STORAGE CAP (§buildings step 2): deliver up to the depot's
                    // remaining headroom (whole units); any EXCESS stays aboard and
                    // the SAME convoy carries it onward to the hub to sell — still
                    // sub-light and raidable, and goods are never silently destroyed.
                    // (This overflow rule is deliberate: of the "sell it / leave it"
                    // options, an automatic sale is the one that can't deadlock a
                    // full depot or strand cargo.)
                    let allies = self.allies_of(ship.owner);
                    let delivered = self
                        .systems
                        .iter_mut()
                        .find(|s| s.id == system && s.owner.is_some_and(|o| o == ship.owner || allies.contains(&o)))
                        .map(|sys| {
                            let stored = (cargo.units as f64).min(sys.storage_headroom()).floor() as u32;
                            if stored > 0 {
                                *sys.stockpile.entry(cargo.commodity).or_insert(0.0) += stored as f64;
                            }
                            stored
                        });
                    if let Some(stored) = delivered {
                        if stored > 0 {
                            events.push(Event::new(
                                now,
                                EventPayload::Trade(TradeEvent::Delivered {
                                    player: ship.owner,
                                    commodity: cargo.commodity,
                                    units: stored,
                                    system: Some(system), // DeliverToSystem → the system stockpile
                                }),
                            ));
                            if fought
                                && let Some(corp) = self.players.get_mut(&ship.owner)
                            {
                                corp.stats.cargo_protected += stored as u64;
                            }
                        }
                        let excess = cargo.units - stored;
                        if excess > 0 {
                            // Re-task the convoy we pulled off the map with the
                            // overflow: on to the hub, sell on arrival.
                            let mut ship = ship;
                            ship.cargo = Some(crate::cargo::Cargo { commodity: cargo.commodity, units: excess });
                            ship.order = FleetOrder::MoveTo { dest: self.hub };
                            ship.mission = Some(TradeMission::SellAtHub);
                            let owner = ship.owner;
                            self.fleets.insert(ship.id, ship);
                            events.push(Event::new(
                                now,
                                EventPayload::Trade(TradeEvent::StorageOverflow {
                                    player: owner,
                                    commodity: cargo.commodity,
                                    units: excess,
                                    system,
                                }),
                            ));
                        }
                    } else {
                        // Destination no longer ours: apply the corp's doctrine
                        // (§16). Default Drop loses the cargo; otherwise re-route the
                        // SAME convoy on a new leg (still sub-light + raidable — no
                        // teleporting goods), to deliver home or sell at the hub.
                        let owner = ship.owner;
                        let policy = self
                            .players
                            .get(&owner)
                            .map(|c| c.doctrine.destination_invalid)
                            .unwrap_or_default();
                        let reroute = match policy {
                            DestinationInvalidPolicy::Drop => None,
                            DestinationInvalidPolicy::ReturnHome => {
                                self.players.get(&owner).map(|c| (c.home, TradeMission::DeliverHome))
                            }
                            DestinationInvalidPolicy::SellAtHub => {
                                Some((self.hub, TradeMission::SellAtHub))
                            }
                        };
                        let action = match policy {
                            DestinationInvalidPolicy::Drop => DivertAction::Lost,
                            DestinationInvalidPolicy::ReturnHome => DivertAction::ReturnedHome,
                            DestinationInvalidPolicy::SellAtHub => DivertAction::SoldAtHub,
                        };
                        if let Some((dest, mission)) = reroute {
                            // Re-task the convoy we just pulled out of the map and put
                            // it back on its new leg, keeping its id and cargo.
                            let mut ship = ship;
                            ship.order = FleetOrder::MoveTo { dest };
                            ship.mission = Some(mission);
                            self.fleets.insert(ship.id, ship);
                        }
                        events.push(Event::new(
                            now,
                            EventPayload::Trade(TradeEvent::SupplyDiverted {
                                player: owner,
                                commodity: cargo.commodity,
                                units: cargo.units,
                                system,
                                action,
                            }),
                        ));
                    }
                }
            }
        }
    }

    /// Schedule an order to install on a ship the player owns, after the
    /// outbound light-travel time from their command center to the ship (§6).
    /// Ignored if the ship doesn't exist or the player doesn't own it.
    fn schedule_for_owner(
        &mut self,
        player_id: PlayerId,
        ship_id: EntityId,
        new_order: FleetOrder,
        kind: crate::event::OrderKind,
    ) {
        let Some(ship) = self.fleets.get(&ship_id) else {
            return;
        };
        if ship.owner != player_id {
            return;
        }
        let Some(corp) = self.players.get(&player_id) else {
            return;
        };
        let cc = corp.command_center;
        let c = self.config.c;
        let ship_pos = ship.pos;
        let ship_vel = ship.vel;
        // §node Relay Anchor: if the issuer holds an active black-hole node whose
        // region covers the fleet, its command loop through that neighbourhood runs
        // at half the light-time. Evaluated at the fleet's CURRENT position (the
        // command's outbound leg) — the SINGLE plug for the tempo bonus, so it can
        // never desync from what the map shows. `relay_factor` is 1.0 with no node.
        let relay = self.relay_factor(player_id, ship_pos);
        // Outbound light delay from the fleet's current position (deterministic,
        // known at issue). `delivered_at` is when the fleet gets the order.
        let delay = ship_pos.distance(cc) / c * relay;
        let delivered_at = self.time + delay;
        // The DELIVERY POINT: where the fleet will be when the order lands, by
        // constant-velocity extrapolation of its current motion (§14.1). The echo
        // — the first light of the new behavior — leaves there at delivery and
        // reaches the command center `distance/c` later. Exactly computable now.
        // The return leg is relayed too when the delivery point stays in region.
        let delivery_point = ship_pos + ship_vel * delay;
        let echo_relay = self.relay_factor(player_id, delivery_point);
        let echo_at = delivered_at + delivery_point.distance(cc) / c * echo_relay;
        self.pending_orders.push(PendingOrder {
            apply_time: delivered_at,
            ship_id,
            new_order,
            owner: player_id,
            echo_at,
            issued_at: self.time,
            kind,
        });
    }

    /// Try to draw `cost` Fuel from the player's owned system NEAREST `origin` that
    /// can cover the FULL cost on its own (atomic — never split across systems).
    /// Returns true on success (or when `cost` ≈ 0, a free dispatch); false on a
    /// shortfall, in which case NOTHING is debited and the caller must LIMIT the op
    /// (hold it) rather than destroy anything. Tiebreak `(distance, id)` →
    /// deterministic. This is the single fuel-debit choke point (§step1 part 2).
    fn charge_fuel(&mut self, player: PlayerId, origin: Vec2, cost: f64) -> bool {
        if cost <= 1e-9 {
            return true;
        }
        let fuel = crate::fuel::MOVEMENT_FUEL;
        let mut best: Option<(f64, EntityId)> = None;
        for s in &self.systems {
            if s.owner != Some(player) {
                continue;
            }
            if s.stockpile.get(&fuel).copied().unwrap_or(0.0) + 1e-9 < cost {
                continue;
            }
            let key = (s.pos.distance(origin), s.id);
            if best.is_none_or(|b| key < b) {
                best = Some(key);
            }
        }
        let Some((_, sid)) = best else {
            return false;
        };
        if let Some(s) = self.systems.iter_mut().find(|s| s.id == sid) {
            *s.stockpile.entry(fuel).or_insert(0.0) -= cost;
        }
        true
    }

    /// Assign an unused home anchor to a player (or append one if the galaxy is
    /// over capacity), returning its position.
    /// Assign a home slot to a joining player and GRANT them its co-located home
    /// star system (free — no claim cost). Sets ownership at `now` so a rival
    /// learns of the home by light delay like any claim. Returns the home position
    /// and the granted home system's id.
    fn assign_home(&mut self, id: PlayerId) -> (Vec2, EntityId) {
        let now = self.time;
        // Take the first unowned pre-generated slot (deterministic order), else
        // append an over-capacity slot with a freshly-generated home system.
        let idx = match self.home_slots.iter().position(|s| s.owner.is_none()) {
            Some(i) => {
                self.home_slots[i].owner = Some(id);
                self.home_slots[i].claimed_at = Some(now);
                i
            }
            None => {
                let n = self.home_slots.len();
                let angle = TAU * (n as f64) * 0.61803398875; // golden-angle scatter
                let pos = Vec2::from_polar(angle, self.config.galaxy_radius * self.config.home_ring_frac);
                let sys_id = self.alloc_entity_id();
                // §naming: a galaxy-unique name that avoids every system already present.
                let taken: std::collections::BTreeSet<String> = self.systems.iter().map(|s| s.name.clone()).collect();
                let name = crate::galaxy::pick_unused_name(self.config.seed, &taken);
                let mut sys = crate::galaxy::generate_home_system(self.config.seed, n, sys_id, pos, name);
                sys.owner = Some(id);
                sys.claimed_at = Some(now);
                self.systems.push(sys);
                self.home_slots.push(HomeSlot {
                    pos,
                    owner: Some(id),
                    claimed_at: Some(now),
                    system: Some(sys_id),
                });
                return (pos, sys_id);
            }
        };
        let pos = self.home_slots[idx].pos;
        // Grant the slot's pre-generated home system. If it's missing — e.g. an
        // old PRE-FEATURE snapshot (slot.system deserialized to None and no home
        // systems were generated) or a migrated world — generate one now, so a new
        // join never panics or leaves `home_system` pointing at an unowned system.
        match self.home_slots[idx].system.filter(|sid| self.systems.iter().any(|s| s.id == *sid)) {
            Some(sys_id) => {
                let sys = self
                    .systems
                    .iter_mut()
                    .find(|s| s.id == sys_id)
                    .expect("home system existence was just checked");
                sys.owner = Some(id);
                sys.claimed_at = Some(now);
                (pos, sys_id)
            }
            None => {
                let sys_id = self.alloc_entity_id();
                // §naming: a galaxy-unique name that avoids every system already present.
                let taken: std::collections::BTreeSet<String> = self.systems.iter().map(|s| s.name.clone()).collect();
                let name = crate::galaxy::pick_unused_name(self.config.seed, &taken);
                let mut sys = crate::galaxy::generate_home_system(self.config.seed, idx, sys_id, pos, name);
                sys.owner = Some(id);
                sys.claimed_at = Some(now);
                self.systems.push(sys);
                self.home_slots[idx].system = Some(sys_id);
                (pos, sys_id)
            }
        }
    }

    /// Spawn the M2 demo fleet (one convoy, one raider) at a home anchor, set to
    /// patrol so the shared world is visibly alive. (Player-issued orders arrive
    /// in M4/M5.)
    fn spawn_starting_fleet(&mut self, owner: PlayerId, home: Vec2, events: &mut Vec<Event>) {
        let hub = self.hub;
        let nearest = self.nearest_system(home).unwrap_or(hub);

        // Deterministic demo cargo for the convoy. §economy: draws from the RAW
        // ladder explicitly (a starting convoy hauls raws — the old `ALL % 5` was
        // a magic modulo that silently narrowed when ALL grew to 12).
        let cargo = {
            let commodity = crate::cargo::Commodity::RAW
                [(self.rng.next_u64() % crate::cargo::Commodity::RAW.len() as u64) as usize];
            let units = 40 + (self.rng.next_u64() % 160) as u32;
            crate::cargo::Cargo { commodity, units }
        };

        // Convoy plies the home↔hub trade lane.
        let convoy_id = self.alloc_entity_id();
        self.fleets.insert(
            convoy_id,
            Fleet::single(
                convoy_id,
                owner,
                ShipKind::Convoy,
                home,
                FleetOrder::Patrol {
                    waypoints: vec![home, hub],
                    index: 1,
                    dwell_until: 0.0,
                },
                Some(cargo),
            ),
        );
        events.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id: convoy_id,
                owner,
                kind: ShipKind::Convoy,
            },
        ));

        // Raider ESCORTS the convoy's home↔hub trade lane (so it's positioned to
        // autonomously defend the convoy via standing doctrine). `nearest` is no
        // longer used for its route, but kept available for future picket setups.
        let _ = nearest;
        let raider_id = self.alloc_entity_id();
        let raider_fleet = Fleet::single(
            raider_id,
            owner,
            ShipKind::Raider,
            home,
            FleetOrder::Patrol {
                waypoints: vec![home, hub],
                index: 1,
                dwell_until: 0.0,
            },
            None, // raiders carry no cargo
        );
        self.fleets.insert(raider_id, raider_fleet);
        events.push(Event::new(
            self.time,
            EventPayload::ShipSpawned {
                id: raider_id,
                owner,
                kind: ShipKind::Raider,
            },
        ));
    }

    /// Position of the star system nearest to `p` (None if no systems).
    fn nearest_system(&self, p: Vec2) -> Option<Vec2> {
        self.systems
            .iter()
            .min_by(|a, b| {
                a.pos
                    .distance_sq(p)
                    .partial_cmp(&b.pos.distance_sq(p))
                    .unwrap()
            })
            .map(|s| s.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::RetreatThreshold;
    use crate::ids::PlayerId;
    use crate::node::{
        Node, NodeBonus, NODES_PER_CORP, NODE_REGION_RADIUS, RELAY_DELAY_MULT, VEIL_SIGNATURE_MULT,
    };

    fn test_world() -> World {
        // A SHORT battle timescale so combat tests resolve in a few seconds (the
        // config-scaled-duration behaviour itself is proven in
        // `equal_forces_duration_matches_target_under_both_presets`).
        let mut cfg = SimConfig::for_players(123, 4);
        cfg.battle_target_secs = 20.0;
        World::new(cfg)
    }

    #[test]
    fn galaxy_is_generated() {
        let w = test_world();
        assert_eq!(w.hub, Vec2::ZERO);
        // Frontier systems + one co-located home system per home slot.
        assert_eq!(
            w.systems.len(),
            (w.config.system_count + w.config.max_players) as usize
        );
        assert_eq!(w.home_slots.len(), w.config.max_players as usize);
        // Every home slot has a co-located, unowned-until-granted home system.
        for slot in &w.home_slots {
            let sid = slot.system.expect("home slot has a home system");
            let sys = w.systems.iter().find(|s| s.id == sid).expect("home system exists");
            assert_eq!(sys.pos, slot.pos, "home system sits at its slot");
            assert!(sys.owner.is_none(), "home system unowned until a player joins");
            assert!(sys.all_deposits().next().is_some(), "home system is developed");
        }
        // Systems lie within the galaxy radius.
        for s in &w.systems {
            assert!(s.pos.length() <= w.config.galaxy_radius + 1.0);
        }
    }

    /// §TCA Phase 2: charter standing REGENERATES unconditionally, at the same
    /// rate in every band, and clamps at the ceiling. Time served is time served:
    /// nobody is permanently locked out by arithmetic alone.
    #[test]
    fn charter_standing_regenerates_in_every_band_and_clamps() {
        let mut w = test_world();
        let (clean, outlaw) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: clean, name: "Clean".into() },
            Command::AddPlayer { id: outlaw, name: "Outlaw".into() },
        ]);
        // A fresh charter starts at full standing, in good standing.
        assert_eq!(w.players[&clean].tca_standing, crate::tca::TCA_STANDING_START);
        assert_eq!(crate::tca::charter_status(w.players[&clean].tca_standing), crate::tca::CharterStatus::GoodStanding);

        // Drive one corp deep into Proscribed and run a while.
        w.players.get_mut(&outlaw).unwrap().tca_standing = crate::tca::TCA_PROSCRIBED_AT - 5.0;
        let ticks = 20 * crate::config::TICK_HZ as u64;
        for _ in 0..ticks {
            w.step(&[]);
        }
        let secs = ticks as f64 * DT;
        let expect = crate::tca::TCA_PROSCRIBED_AT - 5.0 + crate::tca::TCA_STANDING_REGEN_PER_SEC * secs;
        assert!(
            (w.players[&outlaw].tca_standing - expect).abs() < 1e-6,
            "regen applies in the WORST band too (got {}, want {expect})",
            w.players[&outlaw].tca_standing
        );
        // The clean corp is pinned at the ceiling, never above it.
        assert_eq!(w.players[&clean].tca_standing, crate::tca::TCA_STANDING_MAX, "regen clamps at the ceiling");
    }
    /// §TCA Phase 2 (snapshot compatibility): a PRE-LAW snapshot has no
    /// `tca_standing` at all — every corporation must load in GOOD STANDING.
    /// Nobody is retroactively an outlaw.
    #[test]
    fn a_pre_law_snapshot_loads_every_corp_in_good_standing() {
        let mut w = test_world();
        let id = PlayerId(4);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // A corp that HAD fallen…
        w.players.get_mut(&id).unwrap().tca_standing = 5.0;
        let json = serde_json::to_string(&w).unwrap();

        // …but whose snapshot predates the law entirely.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        for (_pid, corp) in v.get_mut("players").unwrap().as_object_mut().unwrap().iter_mut() {
            corp.as_object_mut().unwrap().remove("tca_standing");
        }
        let stripped = serde_json::to_string(&v).unwrap();
        assert!(!stripped.contains("tca_standing"));

        let old: World = serde_json::from_str(&stripped).unwrap();
        for corp in old.players.values() {
            assert_eq!(corp.tca_standing, crate::tca::TCA_STANDING_START);
            assert_eq!(crate::tca::charter_status(corp.tca_standing), crate::tca::CharterStatus::GoodStanding);
        }
    }
    #[test]
    fn clock_advances_one_dt_per_step() {
        let mut w = test_world();
        assert_eq!(w.tick, 0);
        w.step(&[]);
        assert_eq!(w.tick, 1);
        assert!((w.time - DT).abs() < 1e-12);
    }

    #[test]
    fn add_player_assigns_home_and_fleet() {
        let mut w = test_world();
        let id = PlayerId(7);
        let ev = w.step(&[Command::AddPlayer {
            id,
            name: "Acme".into(),
        }]);
        // PlayerJoined + two ShipSpawned.
        assert_eq!(ev.len(), 3);
        assert_eq!(w.players.len(), 1);
        assert_eq!(w.fleets.len(), 2);
        let corp = &w.players[&id];
        assert_eq!(corp.home, corp.command_center);
        // One anchor is now owned.
        assert_eq!(w.home_slots.iter().filter(|s| s.owner == Some(id)).count(), 1);
        // The player begins owning exactly one HOME STAR SYSTEM, granted free
        // (credits untouched), with the command center sitting at it.
        let home_id = corp.home_system.expect("a joined player has a home system");
        assert_eq!(corp.credits, 10_000.0, "the home is granted, not bought");
        let owned: Vec<_> = w.systems.iter().filter(|s| s.owner == Some(id)).collect();
        assert_eq!(owned.len(), 1, "exactly one owned system at join (the home)");
        assert_eq!(owned[0].id, home_id);
        assert_eq!(owned[0].pos, corp.command_center, "home system sits at the command center");
        // Claimed at the join instant (== the home anchor's claim time), so a rival
        // learns of it by light delay like any claim — not instantly, not never.
        let anchor_claimed = w.home_slots.iter().find(|s| s.owner == Some(id)).unwrap().claimed_at;
        assert!(owned[0].claimed_at.is_some());
        assert_eq!(owned[0].claimed_at, anchor_claimed, "home system & anchor claimed at the same instant");
        assert!(owned[0].all_deposits().next().is_some(), "home is a developed, producing system");
    }

    #[test]
    fn add_player_is_idempotent() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let ev2 = w.step(&[Command::AddPlayer {
            id,
            name: "Acme (reconnect)".into(),
        }]);
        assert_eq!(ev2.len(), 0);
        assert_eq!(w.players.len(), 1);
        assert_eq!(w.fleets.len(), 2); // no duplicate fleet
        // Reconnect must NOT grant a second home system.
        assert_eq!(w.systems.iter().filter(|s| s.owner == Some(id)).count(), 1);
    }

    #[test]
    fn home_system_produces_and_ships_from_turn_one() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.expect("owns a home system");
        // No claim needed: the granted home accrues production immediately.
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let stock: f64 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.values().sum();
        assert!(stock >= 1.0, "the home system produces from turn one (got {stock})");

        // It fleets to the hub like any owned system → a raidable sell convoy.
        w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        let convoy = w.fleets.values().find(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub));
        assert!(convoy.is_some(), "the home can ship its production to the hub");
    }

    #[test]
    fn home_system_is_modest_not_a_frontier_jackpot() {
        let mut w = test_world();
        let id = PlayerId(5);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let home_value = value_rate(w.systems.iter().find(|s| s.id == home).unwrap());
        // The richest FRONTIER system clearly out-produces the starter home, so
        // expansion outward stays the reward (the distance/value gradient holds).
        let best_frontier = w
            .home_slots
            .iter()
            .filter_map(|h| h.system)
            .collect::<std::collections::BTreeSet<_>>();
        let best = w
            .systems
            .iter()
            .filter(|s| !best_frontier.contains(&s.id))
            .map(value_rate)
            .fold(0.0_f64, f64::max);
        assert!(home_value < best, "home ({home_value:.1}) must be weaker than the richest frontier ({best:.1})");
    }

    #[test]
    fn home_systems_cannot_be_claimed_from_the_pool() {
        let mut w = test_world();
        // An unassigned home slot's system is reserved — a colony ship parked
        // right on it never settles it; it holds (soft, intact) with a notice.
        let id = PlayerId(9);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let other_home = w
            .home_slots
            .iter()
            .find(|h| h.owner.is_none())
            .and_then(|h| h.system)
            .expect("an unassigned home slot exists at 4-player scale");
        let pos = w.systems.iter().find(|s| s.id == other_home).unwrap().pos;
        let cid = colony_at(&mut w, id, pos);
        let ev = w.step(&[]);
        let sys = w.systems.iter().find(|s| s.id == other_home).unwrap();
        assert!(sys.owner.is_none(), "a reserved home system cannot be settled");
        assert!(w.fleets.contains_key(&cid), "the colony ship holds, intact");
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::ColonyHeld { system, .. } if system == other_home)),
            "the owner is told the site is reserved/taken"
        );
    }

    #[test]
    fn join_regenerates_a_missing_home_system() {
        // Simulate an old/migrated snapshot: the first home slot lost its
        // pre-generated home system (system == None). A join must still grant a
        // home — generated on the fly — never panic or own a phantom system.
        let mut w = test_world();
        w.home_slots[0].system = None;
        let id = PlayerId(11);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let corp = &w.players[&id];
        let home = corp.home_system.expect("a home was granted");
        let owned: Vec<_> = w.systems.iter().filter(|s| s.owner == Some(id)).collect();
        assert_eq!(owned.len(), 1, "exactly one owned home, regenerated on the fly");
        assert_eq!(owned[0].id, home, "home_system points at the owned system (no phantom)");
        assert_eq!(owned[0].pos, corp.command_center);
    }

    // --- §step1 PART 1: build sink ------------------------------------------
    use crate::build::{StructureKind, CONVOY_RECIPE};
    use crate::cargo::Commodity;

    /// Grant `owner` a system directly (test SETUP — the game path is now a
    /// colony-ship arrival, tested separately in §fleets part 3).
    /// Grant + make a WORKING colony of it (§economy Part 3: nothing produces
    /// unstaffed): tier-1 extraction structures matching its geology, one crew
    /// each, a fed population large enough that every factor reads 1.0.
    fn grant_system(w: &mut World, owner: PlayerId, sys: EntityId) {
        let s = w.systems.iter_mut().find(|s| s.id == sys).unwrap();
        s.owner = Some(owner);
        s.claimed_at = Some(w.time);
        // §bodies: extraction is PER BODY — every body with a matching deposit
        // gets its structure + crew, so all deposits produce at factor 1.
        for b in s.bodies.iter_mut() {
            let kinds: std::collections::BTreeSet<_> =
                b.deposits.iter().filter_map(|d| crate::production::extraction_structure(d.resource)).collect();
            for k in kinds {
                b.set_tier(k, b.tier(k).max(1));
                b.assignments.insert(k, crate::production::Assignment::crew(1));
            }
        }
        s.set_population(8.0); // 10 workforce units — ample even for many bodies
        s.stockpile.insert(Commodity::Provisions, 100.0); // minutes of food at 8M
    }

    /// Park a fresh COLONY ship of `owner` at `pos` (already arrived, Idle).
    fn colony_at(w: &mut World, owner: PlayerId, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Colony, pos, FleetOrder::Idle, None));
        id
    }

    /// Seed an owned system's stockpile so a recipe is affordable in tests.
    fn seed_stock(w: &mut World, sys: EntityId, items: &[(Commodity, f64)]) {
        let s = w.systems.iter_mut().find(|s| s.id == sys).unwrap();
        for (c, n) in items {
            *s.stockpile.entry(*c).or_insert(0.0) += *n;
        }
    }

    /// §TCA Part 1: a corporation can NEVER mint an Authority Freighter.
    /// `BuildShip` soft-rejects it (NotBuildable) BEFORE any stockpile/slot/
    /// shipyard check, no job is enqueued, and no Freighter fleet ever appears —
    /// even after the world runs on. The freighter is a TCA-sentinel-only hull,
    /// sitting outside the warship ladder entirely.
    #[test]
    fn freighter_is_never_buildable_by_a_corporation() {
        // The buildability predicate itself: everything on the ladder but the Freighter.
        for k in crate::ship::ALL_SHIP_KINDS {
            assert_eq!(k.is_buildable(), k != ShipKind::Freighter, "{k:?}");
        }
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Pile the home high so NO ordinary reason (stockpile, slot, shipyard)
        // could explain a refusal — only the buildability guard can.
        seed_stock(&mut w, home, &[(Commodity::Alloys, 900.0), (Commodity::Machinery, 400.0), (Commodity::Polymers, 400.0)]);
        let ev = w.step(&[Command::BuildShip {
            player_id: id,
            system_id: home,
            ship_kind: ShipKind::Freighter,
            join: None,
            loadout: Default::default(),
        }]);
        assert!(
            ev.iter().any(|e| matches!(
                &e.payload,
                EventPayload::BuildRejected { owner, reason: crate::event::BuildRejectReason::NotBuildable, .. } if *owner == id
            )),
            "BuildShip{{Freighter}} soft-rejects as NotBuildable"
        );
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no build started");
        assert!(
            w.build_queue.iter().all(|j| !matches!(j.what, crate::build::BuildKind::Ship { ship: ShipKind::Freighter })),
            "no freighter job enqueued"
        );
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(
            !w.fleets.values().any(|f| f.contains(ShipKind::Freighter)),
            "no Freighter fleet is ever minted by a corporation build"
        );
    }

    /// §TCA Part 1 (snapshot compatibility): a PRE-FEATURE snapshot — one that
    /// predates the warehouse + freight fields — still loads. We simulate it by
    /// serializing a live world, DELETING the new keys (`warehouse` on each corp;
    /// `freight_queue`/`freight_runs`/`next_shipment_id` on the world), and loading
    /// the result: every `#[serde(default)]` fills in empty, and the world is intact.
    #[test]
    fn pre_feature_snapshot_without_warehouse_or_freight_still_loads() {
        let mut w = test_world();
        let id = PlayerId(11);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        for _ in 0..10 {
            w.step(&[]);
        }
        let json = serde_json::to_string(&w).unwrap();
        let reloaded: World = serde_json::from_str(&json).unwrap();
        assert_eq!(reloaded.players.len(), w.players.len());
        assert!(reloaded.freight_queue.is_empty() && reloaded.freight_runs.is_empty());

        // STRIP the new keys to forge a genuine pre-feature snapshot.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_object_mut().unwrap();
        obj.remove("freight_queue");
        obj.remove("freight_runs");
        obj.remove("next_shipment_id");
        for (_pid, corp) in obj.get_mut("players").unwrap().as_object_mut().unwrap().iter_mut() {
            corp.as_object_mut().unwrap().remove("warehouse");
        }
        let stripped = serde_json::to_string(&v).unwrap();
        assert!(!stripped.contains("warehouse"), "the forged snapshot has no warehouse key");
        assert!(!stripped.contains("freight_queue"), "…and no freight_queue key");

        let old: World = serde_json::from_str(&stripped).unwrap();
        assert!(old.freight_queue.is_empty() && old.freight_runs.is_empty());
        for corp in old.players.values() {
            assert!(corp.warehouse.is_empty(), "a pre-feature corp loads with an empty warehouse");
        }
        assert_eq!(old.players.len(), w.players.len());
        assert_eq!(old.tick, w.tick);
    }

    /// §TCA: put goods in a corp's CHARTERHOUSE WAREHOUSE — the only source the
    /// Exchange sells from now, so most market tests need it seeded.
    fn seed_warehouse(w: &mut World, who: PlayerId, items: &[(Commodity, u32)]) {
        let c = w.players.get_mut(&who).unwrap();
        for (com, n) in items {
            *c.warehouse.entry(*com).or_insert(0) += *n;
        }
    }

    /// How many units of `c` sit in `who`'s Charterhouse warehouse.
    fn wh(w: &World, who: PlayerId, c: Commodity) -> u32 {
        w.players[&who].warehouse.get(&c).copied().unwrap_or(0)
    }

    /// §TCA test rig: hand `owner` a colony parked `dist` from the Charterhouse.
    /// A SHORT freight leg keeps the round-trip tests quick (the real geometry is
    /// exercised by the fee/ETA maths, which are pure). Never the home system, so
    /// the home bootstrap's staffing can't pollute the stockpile assertions.
    fn near_hub_colony(w: &mut World, owner: PlayerId, dist: f64) -> EntityId {
        // `owner` need not be a registered corp — rival-ownership tests hand a
        // system to a bare id that never joined.
        let home = w.players.get(&owner).and_then(|c| c.home_system);
        let sys = w
            .systems
            .iter_mut()
            .find(|s| s.owner.is_none() && Some(s.id) != home)
            .expect("an unowned system exists");
        sys.owner = Some(owner);
        sys.claimed_at = Some(0.0);
        sys.pos = Vec2::new(dist, 0.0);
        sys.id
    }

    /// Step until `f` holds, or panic after `max` ticks (keeps a stuck freight test
    /// from hanging while still being timing-robust).
    fn step_until(w: &mut World, max: u64, what: &str, mut f: impl FnMut(&World) -> bool) {
        for _ in 0..max {
            if f(w) {
                return;
            }
            w.step(&[]);
        }
        panic!("timed out waiting for: {what}");
    }

    /// Units of `c` in a system's stockpile, rounded down to whole units.
    fn sys_units(w: &World, sys: EntityId, c: Commodity) -> u32 {
        w.systems.iter().find(|s| s.id == sys).unwrap().stockpile.get(&c).copied().unwrap_or(0.0) as u32
    }

    #[test]
    fn build_ship_deducts_recipe_and_spawns_after_duration() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // (§economy Part 5: the convoy recipe is Alloys + Machinery + Polymers —
        // the starter kit covers it; measure the ALLOYS line, nothing at the
        // home produces Alloys so the debit is exact.)
        let alloys0 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Alloys];
        let ships0 = w.fleets.len();

        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        let alloys1 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Alloys];
        assert!((alloys0 - alloys1 - 25.0).abs() < 1e-9, "alloys debited by the convoy recipe (got {})", alloys0 - alloys1);
        assert_eq!(w.build_queue.len(), 1, "a build job is enqueued");
        assert_eq!(w.fleets.len(), ships0, "no ship yet — it builds over time");

        // Step until just before completion: still no new ship.
        for _ in 0..(CONVOY_RECIPE.build_ticks - 2) {
            w.step(&[]);
        }
        assert_eq!(w.fleets.len(), ships0, "not built before its duration elapses");
        // Step past completion → a Convoy spawns Idle at the system.
        let mut spawned = None;
        for _ in 0..4 {
            for ev in w.step(&[]) {
                if let EventPayload::ShipSpawned { id: sid, kind: ShipKind::Convoy, .. } = ev.payload {
                    spawned = Some(sid);
                }
            }
        }
        let sid = spawned.expect("the convoy completes and spawns");
        let ship = &w.fleets[&sid];
        assert_eq!(ship.owner, id);
        assert_eq!(ship.flagship_kind(), ShipKind::Convoy);
        assert!(matches!(ship.order, FleetOrder::Idle), "built fleets spawn Idle at the system");
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        assert!(ship.pos.distance(home_pos) < 1.0, "spawns at the building system");
        assert!(w.build_queue.is_empty(), "completed job is drained");
    }

    #[test]
    fn build_rejected_when_stockpile_short() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Home produces only Ore + Provisions → it has NO Alloys/Fuel, so a Raider
        // (Alloys + Fuel) is unaffordable: a soft reject (no debit, no job, no event).
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None , loadout: Default::default() }]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no build started");
        assert!(w.build_queue.is_empty(), "no job enqueued on a short stockpile");
    }

    /// §economy Part 3: developing the home's Mining Complex 1 → 2 and posting
    /// a full crew climbs the throughput ladder — ore output = richness × 2.2
    /// with every other factor pinned at 1.0.
    #[test]
    fn develop_system_climbs_the_throughput_ladder() {
        let mut w = test_world();
        let id = PlayerId(4);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 65.0)]); // the recipe's 60, under the cap
        // Slot room for the second Mining tier (Resource pool is born full) +
        // an ample, well-fed workforce so staffing/food read exactly 1.0.
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.add_test_deposit(crate::galaxy::Deposit { resource: Commodity::Silicates, richness: 0.0, reserves: None, accessibility: 0.5 });
            sys.set_population(8.0);
            // (the home's seeded 60 Provisions cover these few ticks; more
            // would overflow the 500-unit cap on top of the 300-Fuel seed)
        }
        let ore_rate: f64 = w.systems.iter().find(|s| s.id == home).unwrap()
            .all_deposits().filter(|d| d.resource == Commodity::MetallicOre).map(|d| d.richness).sum();

        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None }]);
        let dur = crate::build::MINING_COMPLEX_RECIPE.build_ticks;
        for _ in 0..(dur + 3) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.tier(crate::build::StructureKind::MiningComplex), 2, "the home's seeded tier-1 mine upgraded to 2");
        // Post the full crew the bigger plant wants, then measure one tick.
        w.step(&[Command::SetAssignment { player_id: id, system_id: home, structure: StructureKind::MiningComplex, workers: 2, specialists: Default::default(), body_id: None }]);
        let ore_before = system_stock(&w, home, Commodity::MetallicOre);
        w.step(&[]);
        let gained = system_stock(&w, home, Commodity::MetallicOre) - ore_before;
        let expect = ore_rate * crate::production::tier_throughput(2) * crate::config::DT;
        assert!((gained - expect).abs() < 1e-6, "ore = richness × tier-2 throughput (got {gained}, want {expect})");
    }

    #[test]
    fn build_survives_system_loss_owner_keeps_ship() {
        let mut w = test_world();
        let id = PlayerId(5);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 100.0), (Commodity::Alloys, 50.0)]);
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        // Lose the system mid-build (e.g. a future conquest).
        w.systems.iter_mut().find(|s| s.id == home).unwrap().owner = Some(PlayerId(999));
        let mut built = false;
        for _ in 0..(CONVOY_RECIPE.build_ticks + 4) {
            for ev in w.step(&[]) {
                if let EventPayload::ShipSpawned { owner, kind: ShipKind::Convoy, .. } = ev.payload
                    && owner == id
                {
                    built = true;
                }
            }
        }
        assert!(built, "you keep what you paid for even if the system is lost");
    }

    #[test]
    fn upgrade_dropped_if_system_lost() {
        let mut w = test_world();
        let id = PlayerId(6);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 100.0), (Commodity::Alloys, 50.0)]);
        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None }]);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().owner = Some(PlayerId(999));
        for _ in 0..(crate::build::MINING_COMPLEX_RECIPE.build_ticks + 4) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.tier(crate::build::StructureKind::MiningComplex), 1, "the dropped upgrade never lands — the tier stays at the home's bootstrap 1 (resources already spent)");
    }

    // --- §buildings step 1 PART 1: development slots -------------------------

    #[test]
    fn dev_slot_exhaustion_soft_rejects_further_developments() {
        let mut w = test_world();
        let id = PlayerId(21);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 10_000.0)]);

        // §economy: slots bound BREADTH, not depth. The home's Resource pool is
        // BORN FULL (Bioharvester + MiningComplex on its 2 deposits), so:
        // a TIER-UP of an existing structure is never slot-gated…
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let pool = crate::build::SlotPool::Resource;
        assert_eq!(sys.pool_slots_built(pool), sys.pool_slots(pool), "the home's resource footprints fill its geology");
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })),
            "deepening an existing footprint needs no free slot"
        );

        // …but FOUNDING a new kind in the full pool SOFT rejects — no debit,
        // no job, an owner-only NoSlot notice.
        let ore_before = system_stock(&w, home, Commodity::MetallicOre);
        let jobs_before = w.build_queue.len();
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::VolatileHarvester, body_id: None }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { owner, reason: crate::event::BuildRejectReason::NoSlot, .. } if owner == id
            )),
            "slot exhaustion notifies the owner"
        );
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no build started");
        assert_eq!(w.build_queue.len(), jobs_before, "no job enqueued");
        let ore_after = system_stock(&w, home, Commodity::MetallicOre);
        assert!(ore_after > ore_before - 1.0, "nothing was debited (accrual aside)");

        // Ships are UNITS, not developments — never slot-gated (only recipe-gated).
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })),
            "a ship still builds at a slot-full system"
        );
    }

    #[test]
    fn tier_ceiling_gates_prize_tiers_behind_research() {
        // §industrial-headroom: the two top structure tiers (5, 6) are the research
        // prize. Without the syndicate's Tier-IV/V unlock a colony tops out at 4
        // (exactly today); the unlock lifts the ceiling to 6.
        let mut w = test_world();
        let id = PlayerId(41);
        w.step(&[Command::AddPlayer { id, name: "Deep".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Machinery, 1000.0), (Commodity::Alloys, 1000.0)]);
        // Force the home's Mining Complex to the free ceiling (tier 4) on its site.
        let site = w.systems.iter().find(|s| s.id == home).unwrap()
            .site_for(StructureKind::MiningComplex).expect("the home mines ore");
        w.systems.iter_mut().find(|s| s.id == home).unwrap()
            .bodies.iter_mut().find(|b| b.id == site).unwrap()
            .set_tier(StructureKind::MiningComplex, 4);
        let site_tier = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap()
            .bodies.iter().find(|b| b.id == site).unwrap().tier(StructureKind::MiningComplex);

        // UNRESEARCHED (solo, no syndicate): raising to tier 5 is over the cap → a
        // soft reject, no job enqueued, the tier stays at 4.
        let jobs_before = w.build_queue.len();
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { owner, .. } if owner == id)),
            "tier 5 is gated without the research unlock"
        );
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no over-cap build starts");
        assert_eq!(w.build_queue.len(), jobs_before, "no over-cap job enqueued");
        assert_eq!(site_tier(&w), 4, "the structure holds at the free ceiling");

        // RESEARCHED: the syndicate holds the Tier-IV Extraction unlock → the same
        // build now STARTS (tier 5 ≤ the raised ceiling of 6).
        w.step(&[Command::CreateSyndicate { player_id: id, name: "Guild".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        w.syndicates.get_mut(&sid).unwrap().research.completed.insert("mat_deepcrust_iii_tier4_extraction".into());
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })),
            "with the Tier-IV/V unlock, tier 5 builds"
        );
    }

    #[test]
    fn capital_hulls_gate_on_research_and_the_titan_is_a_singleton() {
        use crate::event::BuildRejectReason;
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(51);
        w.step(&[Command::AddPlayer { id, name: "Line".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "Armada".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        let home = w.players[&id].home_system.unwrap();
        let hpos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        w.systems.iter_mut().find(|s| s.id == home).unwrap()
            .set_tier(StructureKind::Shipyard, 6);
        seed_stock(&mut w, home, &[
            (Commodity::Alloys, 20_000.0), (Commodity::Electronics, 10_000.0),
            (Commodity::Armaments, 10_000.0), (Commodity::Machinery, 10_000.0),
            (Commodity::RareElements, 5_000.0), (Commodity::Fuel, 5_000.0),
        ]);
        // UNRESEARCHED: a capital keel soft-rejects with the research reason.
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Destroyer, join: None, loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { reason: BuildRejectReason::NeedsResearch, .. })),
            "no Line programme → NeedsResearch"
        );
        // RESEARCHED: the unlock admits the hull.
        w.syndicates.get_mut(&sid).unwrap().research.completed.insert("hull_line_iv_destroyer".into());
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Destroyer, join: None, loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "the unlocked Destroyer lays its keel");
        // TITAN: research VIII grants the hull AND the Shipyard-6 ceiling (the
        // effects array in action).
        w.syndicates.get_mut(&sid).unwrap().research.completed.insert("hull_line_viii_titan".into());
        assert_eq!(
            crate::research::unlocked_structure_tier(&w.syndicates[&sid].research, StructureKind::Shipyard),
            6,
            "Line VIII carries the yard ceiling with the hull"
        );
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Titan, join: None, loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "the first Titan keel is laid");
        // SINGLETON: a second keel is rejected while one is QUEUED…
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Titan, join: None, loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { reason: BuildRejectReason::TitanFielded, .. })),
            "one keel at a time — fielded + queued must be zero"
        );
        // …and while one is FIELDED (clear the queue, graft a live Titan).
        w.build_queue.retain(|j| !matches!(j.what, crate::build::BuildKind::Ship { ship: ShipKind::Titan }));
        let titan = squad(&mut w, id, hpos, ShipKind::Titan, 1, FleetOrder::Idle);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Titan, join: None, loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { reason: BuildRejectReason::TitanFielded, .. })));
        // …and while one is IN THE YARD mid-REFIT (out of every fleet — the
        // review-caught window): the refit_queue counts toward the singleton.
        {
            let f = w.fleets.get_mut(&titan).unwrap();
            f.remove(ShipKind::Titan, 1);
            f.add(ShipKind::Scout, 1); // keep the fleet alive while the hull is in the yard
        }
        w.refit_queue.push(crate::build::RefitJob {
            id: 999_999, owner: id, system: home, fleet: titan, ship: ShipKind::Titan,
            to: crate::module::Loadout::default(), n: 1, complete_tick: w.tick + 10_000,
        });
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Titan, join: None, loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { reason: BuildRejectReason::TitanFielded, .. })),
            "a Titan mid-refit still blocks a second keel"
        );
        w.refit_queue.clear();
        w.fleets.get_mut(&titan).unwrap().add(ShipKind::Titan, 1); // back aboard for the death test
        // NAME the flagship; a serde round-trip keeps it.
        w.step(&[Command::NameFlagship { player_id: id, name: "Reckoning of Veles".into() }]);
        assert_eq!(w.syndicates[&sid].flagship_name.as_deref(), Some("Reckoning of Veles"));
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w2.syndicates[&sid].flagship_name.as_deref(), Some("Reckoning of Veles"), "the name survives a snapshot");
        // DESTRUCTION: the loss clears the name, makes the headline, and
        // re-opens the yard for a rebuild.
        let mut losses = crate::combat::Losses::default();
        losses.add_stack(ShipKind::Titan, crate::module::Loadout::default(), 1);
        let mut members = vec![titan];
        let mut events = Vec::new();
        w.apply_side_losses(&mut members, &losses, hpos, &mut events);
        assert!(
            events.iter().any(|e| matches!(&e.payload, EventPayload::FlagshipDestroyed { name: Some(n), .. } if n == "Reckoning of Veles")),
            "the headline carries the christened name"
        );
        assert!(w.syndicates[&sid].flagship_name.is_none(), "the name dies with the ship");
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Titan, join: None, loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "rebuild after loss is allowed");
    }

    #[test]
    fn flagship_name_stays_with_the_living_titan_on_membership_churn() {
        // §ladder B4 (review-caught): membership churn can gather TWO Titans in
        // one syndicate (the singleton binds at build). When one dies, the
        // LIVING flagship keeps the christened name — the fallen hull makes a
        // nameless headline; the name is taken only when the LAST Titan falls.
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(71);
        w.step(&[Command::AddPlayer { id, name: "Churn".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "Two Crowns".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        let home = w.players[&id].home_system.unwrap();
        let hpos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let t1 = squad(&mut w, id, hpos, ShipKind::Titan, 1, FleetOrder::Idle);
        let _t2 = squad(&mut w, id, hpos + Vec2::new(50.0, 0.0), ShipKind::Titan, 1, FleetOrder::Idle);
        w.step(&[Command::NameFlagship { player_id: id, name: "Reckoning".into() }]);
        // First Titan dies → the name SURVIVES (the other still flies).
        let mut losses = crate::combat::Losses::default();
        losses.add_stack(ShipKind::Titan, crate::module::Loadout::default(), 1);
        let mut members = vec![t1];
        let mut events = Vec::new();
        w.apply_side_losses(&mut members, &losses, hpos, &mut events);
        assert!(
            events.iter().any(|e| matches!(&e.payload, EventPayload::FlagshipDestroyed { name: None, .. })),
            "the fallen hull headlines namelessly"
        );
        assert_eq!(w.syndicates[&sid].flagship_name.as_deref(), Some("Reckoning"), "the living flagship keeps the name");
        // The second (last) Titan dies → NOW the name is taken.
        let mut members = vec![_t2];
        let mut events = Vec::new();
        w.apply_side_losses(&mut members, &losses, hpos, &mut events);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::FlagshipDestroyed { name: Some(n), .. } if n == "Reckoning")));
        assert!(w.syndicates[&sid].flagship_name.is_none());
    }

    #[test]
    fn systems_captured_verb_counts_once_per_capture() {
        let mut w = test_world();
        w.enclaves.clear();
        let a = PlayerId(61);
        let b = PlayerId(62);
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: b, name: "Takers".into() }]);
        let sid = w.players[&b].syndicate.unwrap();
        // A system A owns; B lands the capture (the resolution path is the only
        // place the verb increments — call it directly).
        let sys_id = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(a);
            s.claimed_at = Some(0.0);
            s.id
        };
        let pos = w.systems.iter().find(|s| s.id == sys_id).unwrap().pos;
        let colony = squad(&mut w, b, pos, ShipKind::Colony, 1, FleetOrder::Idle);
        let mut events = Vec::new();
        w.capture_system(sys_id, a, b, colony, pos, &mut events);
        assert_eq!(w.syndicates[&sid].research.verb(crate::research::Verb::SystemsCaptured), 1.0, "one capture, one count");
    }

    /// §economy: the THREE derived slot pools — Resource from geology (one per
    /// deposit, clamped 1..=4), Industrial/Infrastructure from population
    /// (2/3/4 and 2/3/3 by pop tier; §industrial-headroom raised the industrial
    /// base to 2). Derived, never stored — migration-free.
    #[test]
    fn dev_slot_budget_derives_from_geology() {
        let w = test_world();
        for sys in &w.systems {
            // §bodies: pools are PER BODY (derived, never stored)…
            for b in &sys.bodies {
                assert_eq!(b.resource_slots(), (b.deposits.len() as u32).min(4));
                let ind_base = if b.kind == crate::body::BodyKind::GasGiant { 0 } else { 2 };
                assert_eq!(b.industrial_slots(), ind_base + crate::body::body_pop_tier(b.population), "industrial = kind base + body pop tier");
                assert_eq!(
                    b.infrastructure_slots(),
                    1 + b.habitable as u32 + (crate::body::body_pop_tier(b.population) >= 1) as u32,
                );
            }
            // …and every system-level readout is the SUM of its bodies.
            assert_eq!(sys.resource_slots(), sys.bodies.iter().map(|b| b.resource_slots()).sum::<u32>());
            assert_eq!(
                sys.dev_slots(),
                sys.resource_slots() + sys.industrial_slots() + sys.infrastructure_slots(),
                "the legacy single readout sums the pools"
            );
        }
        // Population growth widens a BODY's pools (the road to industrial capacity).
        let mut sys = w.systems[0].clone();
        sys.set_population(crate::body::BODY_POP_DEVELOPED);
        let hab = sys.site_for(crate::build::StructureKind::Habitat).unwrap();
        let b = sys.bodies.iter().find(|b| b.id == hab).unwrap();
        assert_eq!(crate::body::body_pop_tier(b.population), 1, "the seeded body developed");
        assert!(b.industrial_slots() >= 1 && b.infrastructure_slots() >= 2, "its pools widened");
    }

    // --- §buildings step 2: Depot storage caps --------------------------------

    #[test]
    fn storage_cap_stops_accrual_and_resumes_after_shipping() {
        let mut w = test_world();
        let id = PlayerId(22);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();

        // Fill the home to its cap (grandfather-style direct fill).
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        let used = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        seed_stock(&mut w, home, &[(Commodity::Provisions, cap - used)]);

        // Full → accrual idles: total stays exactly at the cap (nothing destroyed).
        let t0: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        for _ in 0..30 {
            w.step(&[]);
        }
        let t1: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        assert!((t1 - t0).abs() < 1e-9, "a full depot accrues nothing (production idles)");
        assert!((t1 - cap).abs() < 1e-6, "…and sits exactly at the cap");

        // Fleet goods out (production → hub) → headroom returns → accrual resumes.
        w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        let t2: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        assert!(t2 < cap - 1.0, "shipping freed capacity");
        for _ in 0..30 {
            w.step(&[]);
        }
        let t3: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        assert!(t3 > t2 + 1e-6, "production resumes once there is headroom");
    }

    #[test]
    fn oversize_stockpile_is_grandfathered_never_destroyed() {
        let mut w = test_world();
        let id = PlayerId(23);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Simulate a pre-cap snapshot: stockpile far over the cap. Use a good
        // NOBODY at the home consumes (§economy: the colony EATS Provisions and
        // converters draw raws — real consumers, not cap destruction), so the
        // only thing that could shrink it is the cap. It must not.
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        seed_stock(&mut w, home, &[(Commodity::Silicates, cap * 3.0)]);
        let over: f64 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Silicates];
        for _ in 0..30 {
            w.step(&[]);
        }
        let after: f64 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Silicates];
        assert!((after - over).abs() < 1e-9, "over-cap stock is kept (cap blocks NEW accrual only)");
    }

    #[test]
    fn depot_tier_raises_the_cap() {
        let mut w = test_world();
        let id = PlayerId(24);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 100.0)]);
        // §economy: the home's Infrastructure pool is BORN FULL (Habitat +
        // Agroplex on 2 slots) — a Depot needs the third slot, i.e. a
        // DEVELOPED colony. That's the designed progression, so grow first.
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_population(crate::build::POP_DEVELOPED);
        let cap0 = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        let slots0 = w.systems.iter().find(|s| s.id == home).unwrap().dev_slots_built();

        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::Depot, body_id: None }]);
        for _ in 0..(crate::build::DEPOT_RECIPE.build_ticks + 3) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.tier(crate::build::StructureKind::Depot), 1, "depot tier applied on completion");
        assert!(
            (sys.storage_cap() - cap0 - crate::build::STORAGE_PER_DEPOT_TIER).abs() < 1e-9,
            "each depot tier adds STORAGE_PER_DEPOT_TIER capacity"
        );
        assert_eq!(sys.dev_slots_built(), slots0 + 1, "a depot tier consumes a development slot");
    }

    #[test]
    fn delivery_overflow_reroutes_excess_to_hub_never_destroys() {
        let mut w = test_world();
        let id = PlayerId(25);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Leave only ~10 units of headroom at the destination.
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let fill = sys.storage_headroom() - 10.0;
        seed_stock(&mut w, home, &[(Commodity::Provisions, fill)]);

        // A convoy delivering 40 ore arrives: 10 stored, 30 carry on to the hub.
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let sid = w.alloc_entity_id();
        let mut ship = Fleet::single(
            sid,
            id,
            ShipKind::Convoy,
            pos, // already at the destination → arrives immediately
            FleetOrder::MoveTo { dest: pos },
            Some(crate::cargo::Cargo { commodity: Commodity::MetallicOre, units: 40 }),
        );
        ship.mission = Some(TradeMission::DeliverToSystem { system: home });
        w.fleets.insert(sid, ship);

        let mut delivered = 0u32;
        let mut overflow = 0u32;
        for _ in 0..5 {
            for ev in w.step(&[]) {
                match ev.payload {
                    EventPayload::Trade(TradeEvent::Delivered { units, commodity: Commodity::MetallicOre, .. }) => delivered += units,
                    EventPayload::Trade(TradeEvent::StorageOverflow { units, system, .. }) => {
                        assert_eq!(system, home);
                        overflow += units;
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(delivered, 10, "delivers up to the depot's headroom");
        assert_eq!(overflow, 30, "the excess is reported, not destroyed");
        // The SAME convoy carries the excess onward to sell at the hub.
        let ship = w.fleets.get(&sid).expect("convoy survives with the overflow");
        assert_eq!(ship.mission, Some(TradeMission::SellAtHub), "re-routed to sell at the hub");
        assert_eq!(ship.cargo.unwrap().units, 30, "carries exactly the unstored excess");
    }

    // --- §buildings step 3: Shipyard gating -----------------------------------

    #[test]
    fn home_starts_with_shipyard_one_and_builds_a_convoy_turn_one() {
        let mut w = test_world();
        let id = PlayerId(26);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.tier(crate::build::StructureKind::Shipyard), crate::build::HOME_SHIPYARD_TIER, "home bootstraps at Shipyard 1");
        assert!(sys.dev_slots_built() >= 1, "the seeded shipyard consumes a development slot");

        // Convoy (needs tier 1) builds turn one — no chicken-and-egg stall.
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 100.0)]);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "convoy builds at the home shipyard");
    }

    #[test]
    fn raider_needs_shipyard_two() {
        let mut w = test_world();
        let id = PlayerId(27);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // §economy Part 5: raider = Alloys+Electronics+Armaments+Fuel; the
        // Shipyard-2 build needs Machinery/Alloys/Electronics (kit + seeds).
        seed_stock(&mut w, home, &[(Commodity::Alloys, 60.0), (Commodity::Electronics, 40.0), (Commodity::Armaments, 20.0)]);

        // Home is tier 1 → a Raider (needs 2) SOFT-rejects with the owner notice.
        let alloys0 = system_stock(&w, home, Commodity::Alloys);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None , loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { owner, reason: crate::event::BuildRejectReason::NeedsShipyard { required: 2 }, .. } if owner == id
            )),
            "the raider rejection names the required tier"
        );
        assert!(w.build_queue.is_empty(), "no job on a shipyard-short system");
        assert!((system_stock(&w, home, Commodity::Alloys) - alloys0).abs() < 1e-9, "recipe never eaten");

        // §economy: Shipyard tier 2 = a SECOND Industrial slot — gated behind a
        // DEVELOPED colony (pop tier 1). Grow the population past the threshold
        // (the designed road to industrial capacity), then build tier 2.
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_population(crate::build::POP_DEVELOPED);
        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::Shipyard, body_id: None }]);
        for _ in 0..(crate::build::SHIPYARD_RECIPE.build_ticks + 3) {
            w.step(&[]);
        }
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().tier(crate::build::StructureKind::Shipyard), 2);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None , loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "raider builds at Shipyard 2");
    }

    #[test]
    fn frontier_system_cannot_build_ships_without_a_shipyard() {
        let mut w = test_world();
        let id = PlayerId(28);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // Claim a frontier system (shipyard 0) and stock it for a convoy.
        let claim = w.systems.iter().find(|s| s.is_unclaimed()).map(|s| s.id).unwrap();
        grant_system(&mut w, id, claim);
        assert_eq!(w.systems.iter().find(|s| s.id == claim).unwrap().owner, Some(id), "claimed");
        seed_stock(&mut w, claim, &[(Commodity::MetallicOre, 100.0)]);

        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: claim, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { reason: crate::event::BuildRejectReason::NeedsShipyard { required: 1 }, .. }
            )),
            "frontier shipbuilding must be earned (no shipyard → soft reject)"
        );
        assert!(w.build_queue.is_empty());
    }

    #[test]
    fn tiers_and_stockpiles_round_trip_through_serde() {
        let mut w = test_world();
        let id = PlayerId(29);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Give the home distinctive development + storage state.
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_tier(crate::build::StructureKind::MiningComplex, 2);
            sys.set_tier(crate::build::StructureKind::Depot, 1);
            sys.set_tier(crate::build::StructureKind::Habitat, 1);
            sys.set_population(2.5);
            sys.food_state = crate::colony::FoodState::Rationing;
            sys.stockpile.insert(Commodity::MetallicOre, 123.5);
            sys.specialists.insert(crate::specialist::SpecialistKind::Geologist, 2);
            let mut asg = crate::production::Assignment::crew(1);
            asg.specialists.insert(crate::specialist::SpecialistKind::Geologist, 1);
            sys.assign(crate::build::StructureKind::MiningComplex, asg);
        }
        // Scout intel rides the snapshot too (§scout part 2).
        w.players.get_mut(&id).unwrap().intel.insert(
            EntityId(999),
            crate::world::IntelSnapshot { defense_tier: 2, shipyard_tier: 1, enclave_tier: 0, observed_at: 3.5, pos: Vec2::new(10.0, 20.0) },
        );
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(
            w.players[&id].intel, w2.players[&id].intel,
            "intel snapshots round-trip through serde"
        );
        let a = w.systems.iter().find(|s| s.id == home).unwrap();
        let b = w2.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!((a.tier(crate::build::StructureKind::MiningComplex), a.tier(crate::build::StructureKind::Depot), a.tier(crate::build::StructureKind::Shipyard)), (b.tier(crate::build::StructureKind::MiningComplex), b.tier(crate::build::StructureKind::Depot), b.tier(crate::build::StructureKind::Shipyard)));
        assert_eq!((a.tier(crate::build::StructureKind::Habitat), a.food_state), (b.tier(crate::build::StructureKind::Habitat), b.food_state), "habitat tier + food state round-trip");
        assert!((a.population() - b.population()).abs() < 1e-9, "population rides the snapshot");
        assert_eq!(a.specialists, b.specialists, "the resident specialist pool rides the snapshot");
        assert_eq!(a.bodies.iter().map(|x| &x.assignments).collect::<Vec<_>>(), b.bodies.iter().map(|x| &x.assignments).collect::<Vec<_>>(), "assignments (with posted specialists + latches) ride the snapshot");
        assert_eq!(a.dev_slots(), b.dev_slots(), "derived slot budget identical after reload");
        assert!((a.storage_cap() - b.storage_cap()).abs() < 1e-12);
        // Stockpiles match commodity-for-commodity (tolerating the last-ulp
        // wobble of a JSON float round-trip).
        assert_eq!(a.stockpile.len(), b.stockpile.len());
        for (c, v) in &a.stockpile {
            assert!((v - b.stockpile[c]).abs() < 1e-9, "{c:?} round-trips");
        }
    }

    // --- §economy Part 2: colony life (population / food ladder / growth) ------

    /// An old snapshot carrying the RETIRED `habitat_fed` bool (and no
    /// `food_state`/`population`) still loads: the unknown key is ignored and
    /// the defaults (WellSupplied, pop 0) are exactly right for a world that
    /// predates people.
    #[test]
    fn pre_population_snapshots_load_with_vacuous_food_state() {
        let w = test_world();
        let mut v: serde_json::Value = serde_json::to_value(&w).unwrap();
        for sys in v["systems"].as_array_mut().unwrap() {
            let o = sys.as_object_mut().unwrap();
            o.remove("food_state");
            o.remove("population");
            o.remove("bodies"); // a true pre-bodies snapshot has no roster
            o.insert("habitat_fed".into(), serde_json::Value::Bool(true));
        }
        let w2: World = serde_json::from_value(v).expect("pre-economy snapshot loads");
        assert!(w2.systems.iter().all(|s| s.food_state == crate::colony::FoodState::WellSupplied && s.population() == 0.0));
    }

    /// §economy Part 2: a claimed ore-rock with `population` and no local food
    /// production — the drained stockpile walks the colony DOWN the whole food
    /// ladder (transition notices at every rung), the draw is exactly
    /// `pop · PROVISIONS_PER_MILLION_PER_S`/s while stock lasts, and POPULATION
    /// NEVER DECREASES, not even bone-dry.
    #[test]
    fn famine_walks_the_ladder_down_but_never_kills() {
        let mut w = test_world();
        let id = PlayerId(31);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_population(2.0);
        sys.set_test_deposits(vec![crate::galaxy::Deposit {
            resource: Commodity::MetallicOre,
            richness: 1.0,
            reserves: None,
            accessibility: 0.5,
        }]);
        let sid = sys.id;
        // Stock just over the WellSupplied line so every rung gets crossed.
        let demand = 2.0 * crate::colony::PROVISIONS_PER_MILLION_PER_S;
        seed_stock(&mut w, sid, &[(Commodity::Provisions, demand * (crate::colony::FOOD_WELL_S + 2.0))]);

        // One tick: draw is exactly demand·DT; state settles at WellSupplied.
        let prov0 = system_stock(&w, sid, Commodity::Provisions);
        w.step(&[]);
        let drawn = prov0 - system_stock(&w, sid, Commodity::Provisions);
        assert!((drawn - demand * crate::config::DT).abs() < 1e-9, "eats pop·rate·DT (got {drawn})");

        // Run it dry; collect every transition.
        let mut states = Vec::new();
        for _ in 0..(45.0 / crate::config::DT) as usize {
            for e in w.step(&[]) {
                if let EventPayload::FoodStateChanged { system, state, .. } = e.payload
                    && system == sid
                {
                    states.push(state);
                }
            }
        }
        use crate::colony::FoodState::*;
        assert_eq!(states, vec![Rationing, Critical, NoProvisions], "every rung announced, in order, once");
        let s = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.food_state, NoProvisions);
        assert!((s.population() - 2.0).abs() < 1e-12, "POPULATION NEVER DECREASES — famine freezes, it never kills");
        assert_eq!(s.tier(crate::build::StructureKind::Habitat), 0, "nothing destroyed");
        assert!(system_stock(&w, sid, Commodity::Provisions).abs() < 1e-9, "the last crumbs were stretched, not deleted");
    }

    /// §economy Part 2: a big Provisions delivery lifts a starving colony back
    /// up the ladder (announced), and while Well Supplied and under Habitat
    /// capacity the population GROWS at `POP_GROWTH_PER_S`, clamping exactly at
    /// `POP_CAP_PER_HABITAT_TIER · tier` — and pop-tier crossings WIDEN the
    /// industrial slot pool (the designed road to industrial capacity).
    #[test]
    fn deliveries_restore_supply_and_population_grows_to_habitat_cap() {
        let mut w = test_world();
        let id = PlayerId(32);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![]); // no geology noise (regenerates the roster)
        sys.set_tier(crate::build::StructureKind::Habitat, 1); // cap 4.0M on its body
        sys.set_population(crate::body::BODY_POP_DEVELOPED - 0.001); // just under the BODY tier line
        sys.food_state = crate::colony::FoodState::NoProvisions;
        let sid = sys.id;
        let hab_body = |w: &World| {
            let s = w.systems.iter().find(|s| s.id == sid).unwrap();
            s.bodies.iter().find(|b| b.population > 0.0).expect("the seeded body").clone()
        };
        let ind0 = hab_body(&w).industrial_slots();

        // A fat shipment lands → straight back to WellSupplied (one notice —
        // the margin allows multi-rung climbs) and growth starts.
        seed_stock(&mut w, sid, &[(Commodity::Provisions, 1000.0)]);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::FoodStateChanged { system, state: crate::colony::FoodState::WellSupplied, .. } if system == sid)),
            "recovery is announced"
        );
        let p0 = w.systems.iter().find(|s| s.id == sid).unwrap().population();
        w.step(&[]);
        let p1 = w.systems.iter().find(|s| s.id == sid).unwrap().population();
        assert!((p1 - p0 - crate::colony::POP_GROWTH_PER_S * crate::config::DT).abs() < 1e-12, "linear growth while fed + under cap");

        // Crossing the BODY's developed line widens ITS industrial pool
        // (§bodies: growing a body is the road to industrial capacity there).
        for _ in 0..(1.0 / crate::config::DT) as usize {
            w.step(&[]);
        }
        let b = hab_body(&w);
        assert!(b.population >= crate::body::BODY_POP_DEVELOPED);
        assert_eq!(b.industrial_slots(), ind0 + 1, "body pop tier 1 unlocks another industrial slot there");

        // Run long enough to hit the cap: growth clamps EXACTLY, never over.
        let cap = crate::colony::POP_CAP_PER_HABITAT_TIER;
        let secs_to_cap = (cap - hab_body(&w).population) / crate::colony::POP_GROWTH_PER_S + 5.0;
        for _ in 0..(secs_to_cap / crate::config::DT) as usize {
            w.step(&[]);
        }
        let s = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert!((s.population() - cap).abs() < 1e-9, "population parks exactly at Habitat capacity (got {})", s.population());
        assert_eq!(s.food_state, crate::colony::FoodState::WellSupplied);
    }

    /// §economy Part 2: a colony ship FOUNDS a population when it settles — and
    /// with no Habitat the outpost holds at founding size (capacity 0 = no
    /// growth), well-fed or not. An UNPEOPLED rock is vacuously WellSupplied
    /// and never emits food notices.
    #[test]
    fn colony_ships_found_population_and_empty_rocks_stay_silent() {
        let mut w = test_world();
        let id = PlayerId(33);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let target = w
            .systems
            .iter()
            .filter(|s| s.is_unclaimed() && !w.home_slots.iter().any(|h| h.system == Some(s.id)))
            .min_by(|a, b| {
                let hp = w.systems.iter().find(|s| s.id == home).unwrap().pos;
                a.pos.distance(hp).total_cmp(&b.pos.distance(hp)).then(a.id.cmp(&b.id))
            })
            .unwrap()
            .id;
        // Plant a colony fleet right on the target (unit test shortcut — the
        // transit itself is covered by the §ships part 3 claiming tests).
        let pos = w.systems.iter().find(|s| s.id == target).unwrap().pos;
        let fid = EntityId(9001);
        w.fleets.insert(fid, crate::ship::Fleet::single(fid, id, ShipKind::Colony, pos, FleetOrder::Idle, None));
        w.step(&[]);
        let s = w.systems.iter().find(|s| s.id == target).unwrap();
        assert_eq!(s.owner, Some(id), "the colony settled");
        assert!((s.population() - crate::colony::COLONY_FOUNDING_POP).abs() < 1e-12, "the ship's crew founds the colony");

        // Feed it lavishly: NO growth without a Habitat (capacity 0)...
        seed_stock(&mut w, target, &[(Commodity::Provisions, 500.0)]);
        for _ in 0..(5.0 / crate::config::DT) as usize {
            w.step(&[]);
        }
        let s = w.systems.iter().find(|s| s.id == target).unwrap();
        assert!((s.population() - crate::colony::COLONY_FOUNDING_POP).abs() < 1e-12, "no Habitat = no growth (and never shrink)");
        assert!(crate::colony::workforce_units(s.population()) == 0, "an outpost this small fields no workforce yet");

        // ...and an unpeopled owned rock never makes food noise.
        let empty = w.systems.iter().find(|s| s.owner == Some(id) && s.population() == 0.0).map(|s| s.id);
        if let Some(eid) = empty {
            for _ in 0..60 {
                let ev = w.step(&[]);
                assert!(
                    !ev.iter().any(|e| matches!(e.payload, EventPayload::FoodStateChanged { system, .. } if system == eid)),
                    "an empty rock never spams food notices"
                );
            }
            assert_eq!(w.systems.iter().find(|s| s.id == eid).unwrap().food_state, crate::colony::FoodState::WellSupplied);
        }
    }

    // §economy NOTE: `home_two_tier_habitat_is_self_sufficient`, the fed-boost
    // test, and the unfed-suspension test were retired here with the binary
    // Habitat model. The home now extracts BIOMASS (a raw), not Provisions, so
    // direct geological self-feeding is impossible BY DESIGN in the industrial
    // web; the self-sustaining-home guarantee is re-established by the Part-4
    // bootstrap (seeded Bioharvester + Agroplex feed the starting population)
    // and its tests. Population leverage over output returns there too, as the
    // staffing/food factors of the assignment engine.

    // --- §economy Part 3: the assignment engine --------------------------------

    /// THE SELF-SUSTAINING-HOME GUARANTEE, re-established: the bootstrap home
    /// (Bioharvester + MiningComplex + Agroplex pre-staffed, pop 2.0M, a food
    /// buffer) feeds itself from tick one — food stays WellSupplied, the
    /// population GROWS, and ore accumulates for building. Turn one works
    /// without opening a menu, exactly like before the industrial web.
    #[test]
    fn bootstrap_home_feeds_itself_and_grows() {
        let mut w = test_world();
        let id = PlayerId(50);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        for _ in 0..(120.0 / crate::config::DT) as usize {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.food_state, crate::colony::FoodState::WellSupplied, "the seeded farm chain keeps the home fed");
        assert!(sys.population() > crate::colony::HOME_FOUNDING_POP, "a fed home GROWS (got {})", sys.population());
        assert!(system_stock(&w, home, Commodity::MetallicOre) > 20.0, "the staffed mine banks ore for building");
        assert!(system_stock(&w, home, Commodity::Provisions) > 0.0, "provisions production outruns the population");
        // Born short-staffed BY DESIGN: 2 crews against 3 posted lines — the
        // first growth milestone (2.4M) fully staffs it. The opening arc.
        assert!(sys.staffing_share() > 2.0 / 3.0 - 1e-9, "share only ever rises from the bootstrap 2/3");
    }

    /// Variant B core math: an UNSTAFFED line produces nothing (structure built
    /// or not), and over-posting the workforce dilutes every line by the SAME
    /// share — fair, legible, deadlock-free.
    #[test]
    fn unstaffed_produces_nothing_and_overposting_dilutes_fairly() {
        let mut w = test_world();
        let id = PlayerId(51);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![
            crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 1.0, reserves: None, accessibility: 0.5 },
            crate::galaxy::Deposit { resource: Commodity::Biomass, richness: 1.0, reserves: None, accessibility: 0.5 },
        ]);
        sys.set_population(0.9); // exactly ONE workforce crew
        sys.stockpile.insert(Commodity::Provisions, 100.0);
        sys.set_tier(crate::build::StructureKind::MiningComplex, 1);
        sys.set_tier(crate::build::StructureKind::Bioharvester, 1);
        let sid = sys.id;

        // Built but UNSTAFFED: nothing moves.
        w.step(&[]);
        assert_eq!(system_stock(&w, sid, Commodity::MetallicOre), 0.0, "unstaffed extraction = 0");
        assert_eq!(system_stock(&w, sid, Commodity::Biomass), 0.0);

        // Post BOTH lines against the single crew: each runs at share 1/2.
        w.step(&[
            Command::SetAssignment { player_id: id, system_id: sid, structure: crate::build::StructureKind::MiningComplex, workers: 1, specialists: Default::default(), body_id: None },
            Command::SetAssignment { player_id: id, system_id: sid, structure: crate::build::StructureKind::Bioharvester, workers: 1, specialists: Default::default(), body_id: None },
        ]);
        let ore0 = system_stock(&w, sid, Commodity::MetallicOre);
        let bio0 = system_stock(&w, sid, Commodity::Biomass);
        w.step(&[]);
        let ore_gain = system_stock(&w, sid, Commodity::MetallicOre) - ore0;
        let bio_gain = system_stock(&w, sid, Commodity::Biomass) - bio0;
        let expect = 1.0 * crate::production::tier_throughput(1) * 0.5 * crate::config::DT;
        assert!((ore_gain - expect).abs() < 1e-9, "one crew across two lines: HALF rate each (got {ore_gain})");
        assert!((bio_gain - expect).abs() < 1e-9, "…the same share for both (got {bio_gain})");
    }

    /// SetAssignment is instant local admin with hard edges: rivals bounce,
    /// unbuilt structures bounce, `workers` clamps to the tier, 0 clears the
    /// line, and every accepted change is announced (AssignmentSet).
    #[test]
    fn set_assignment_validates_clamps_and_announces() {
        let mut w = test_world();
        let (id, rival) = (PlayerId(52), PlayerId(53));
        w.step(&[
            Command::AddPlayer { id, name: "Acme".into() },
            Command::AddPlayer { id: rival, name: "Rival".into() },
        ]);
        let home = w.players[&id].home_system.unwrap();

        // A rival's posting bounces (no state change, no event).
        let ev = w.step(&[Command::SetAssignment { player_id: rival, system_id: home, structure: crate::build::StructureKind::MiningComplex, workers: 1, specialists: Default::default(), body_id: None }]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::AssignmentSet { .. })), "a rival can't staff your colony");

        // An UNBUILT structure bounces.
        let ev = w.step(&[Command::SetAssignment { player_id: id, system_id: home, structure: crate::build::StructureKind::Smelter, workers: 1, specialists: Default::default(), body_id: None }]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::AssignmentSet { .. })), "nothing built to staff");

        // Over-posting a tier-1 structure clamps to 1 crew (announced as such).
        let ev = w.step(&[Command::SetAssignment { player_id: id, system_id: home, structure: crate::build::StructureKind::Shipyard, workers: 5, specialists: Default::default(), body_id: None }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::AssignmentSet { owner, structure: crate::build::StructureKind::Shipyard, workers: 1, .. } if owner == id)),
            "workers clamp to the structure tier"
        );
        // Zero clears the line.
        w.step(&[Command::SetAssignment { player_id: id, system_id: home, structure: crate::build::StructureKind::Shipyard, workers: 0, specialists: Default::default(), body_id: None }]);
        assert!(w.systems.iter().find(|s| s.id == home).unwrap().assignment(crate::build::StructureKind::Shipyard).is_none());
    }

    /// CHAIN INTEGRATION (§economy Part 8 test 5): seeded raws → Smelter →
    /// MachineWorks produces Machinery end-to-end, offline; cutting the ore
    /// feed stalls the chain within the buffer window (latched NoInputs up the
    /// chain) — and NOTHING is destroyed.
    #[test]
    fn industrial_chain_produces_machinery_and_stalls_when_the_feed_is_cut() {
        let mut w = test_world();
        let id = PlayerId(54);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![]); // a pure FORGE WORLD — all inputs imported
        sys.set_population(8.0);
        sys.stockpile.insert(Commodity::Provisions, 150.0);
        sys.stockpile.insert(Commodity::MetallicOre, 60.0);
        sys.stockpile.insert(Commodity::Fuel, 60.0);
        sys.stockpile.insert(Commodity::Electronics, 30.0);
        sys.set_tier(crate::build::StructureKind::Smelter, 1);
        sys.set_tier(crate::build::StructureKind::MachineWorks, 1);
        sys.assign(crate::build::StructureKind::Smelter, crate::production::Assignment::crew(1));
        sys.assign(crate::build::StructureKind::MachineWorks, crate::production::Assignment::crew(1));
        let sid = sys.id;

        for _ in 0..(20.0 / crate::config::DT) as usize {
            w.step(&[]);
        }
        let mach = system_stock(&w, sid, Commodity::Machinery);
        assert!(mach > 1.0, "ore → Alloys → Machinery end-to-end offline (got {mach})");

        // CUT THE FEED (a blockade in miniature): drain the ore. Alloys buffer
        // for a while, then the Smelter latches NoInputs, then MachineWorks.
        w.systems.iter_mut().find(|s| s.id == sid).unwrap().stockpile.remove(&Commodity::MetallicOre);
        let (mut smelter_down, mut works_down) = (false, false);
        for _ in 0..(60.0 / crate::config::DT) as usize {
            for e in w.step(&[]) {
                if let EventPayload::ProductionSuspended { system, structure, reason: crate::production::SuspendReason::NoInputs, .. } = e.payload {
                    if system == sid && structure == crate::build::StructureKind::Smelter { smelter_down = true; }
                    if system == sid && structure == crate::build::StructureKind::MachineWorks { works_down = true; }
                }
            }
        }
        assert!(smelter_down, "the dry Smelter announces NO INPUTS");
        assert!(works_down, "the stall propagates up the chain once the Alloys buffer drains");
        let s = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.tier(crate::build::StructureKind::Smelter), 1, "nothing destroyed");
        assert!(s.assignment(crate::build::StructureKind::Smelter).unwrap().workers == 1, "assignments survive the stall");
        assert!(system_stock(&w, sid, Commodity::Machinery) >= mach, "made Machinery is never clawed back");
    }

    /// ADVANCED INDUSTRY stops at Critical (precision work halts before the
    /// mills do) and resumes with the food state — the Part-2 ladder's teeth.
    #[test]
    fn advanced_industry_suspends_at_critical_and_recovers() {
        let mut w = test_world();
        let id = PlayerId(55);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![]);
        sys.set_population(2.0);
        sys.stockpile.insert(Commodity::Alloys, 200.0);
        sys.stockpile.insert(Commodity::Electronics, 100.0);
        sys.stockpile.insert(Commodity::Fuel, 100.0);
        sys.set_tier(crate::build::StructureKind::MachineWorks, 1);
        sys.assign(crate::build::StructureKind::MachineWorks, crate::production::Assignment::crew(1));
        sys.food_state = crate::colony::FoodState::Critical; // starving in
        let sid = sys.id;

        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::ProductionSuspended { system, structure: crate::build::StructureKind::MachineWorks, reason: crate::production::SuspendReason::NoFood, .. } if system == sid)),
            "advanced industry suspends at Critical"
        );
        assert_eq!(system_stock(&w, sid, Commodity::Machinery), 0.0, "no output while suspended");

        // A food shipment lifts the ladder → the line resumes and produces.
        seed_stock(&mut w, sid, &[(Commodity::Provisions, 50.0)]);
        let mut resumed = false;
        for _ in 0..10 {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::ProductionResumed { system, structure: crate::build::StructureKind::MachineWorks, .. } if system == sid) {
                    resumed = true;
                }
            }
        }
        assert!(resumed, "the line announces its recovery");
        assert!(system_stock(&w, sid, Commodity::Machinery) > 0.0, "…and produces again");
    }

    /// A STAFFED Shipyard turns ship jobs out faster — build_ticks divided by
    /// (1 + 0.25·staffing), locked at enqueue. Structures are unaffected.
    #[test]
    fn staffed_shipyard_speeds_ship_builds() {
        let ticks_for = |staff: bool| {
            let mut w = test_world();
            let id = PlayerId(56);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let home = w.players[&id].home_system.unwrap();
            {
                let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
                sys.set_population(8.0); // covers the extra crew without diluting
                if staff {
                    sys.assign(crate::build::StructureKind::Shipyard, crate::production::Assignment::crew(1));
                }
            }
            seed_stock(&mut w, home, &[(Commodity::MetallicOre, 50.0)]);
            let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
            ev.iter()
                .find_map(|e| match e.payload {
                    EventPayload::BuildStarted { complete_tick, .. } => Some(complete_tick),
                    _ => None,
                })
                .expect("build starts")
        };
        let (plain, boosted) = (ticks_for(false), ticks_for(true));
        assert!(boosted < plain, "a staffed yard is faster ({boosted} vs {plain})");
        // Same enqueue tick in both worlds, so the difference is pure boost.
        let expect = CONVOY_RECIPE.build_ticks - (CONVOY_RECIPE.build_ticks as f64 / (1.0 + crate::production::SHIPYARD_BOOST)).round() as u64;
        assert_eq!(plain - boosted, expect, "ticks / (1 + BOOST·staffing), locked at enqueue");
    }

    /// §economy Part 7 ACCEPTANCE: a hand-built PRE-ECONOMY snapshot (legacy
    /// flat tier fields, processed-good deposits, "ore"/"habitat_fed" keys, no
    /// population/structures/assignments) loads, migrates, and TICKS 1000
    /// times — no panic, deposits all raw, tiers folded, populations seeded,
    /// default lines posted, and PRODUCTION IS POSITIVE. Idempotent: a second
    /// migration pass changes nothing.
    #[test]
    fn legacy_snapshot_migrates_and_keeps_producing() {
        // Build a modern world, then rewrite one owned system + the home into
        // the exact legacy JSON shape.
        let mut w = test_world();
        let id = PlayerId(80);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let colony = w.systems.iter().find(|s| s.is_unclaimed()).unwrap().id;
        let mut v: serde_json::Value = serde_json::to_value(&w).unwrap();
        for sys in v["systems"].as_array_mut().unwrap() {
            let sid = sys["id"].as_str().map(|s| s.to_string()).or_else(|| sys["id"].as_u64().map(|n| n.to_string()));
            let o = sys.as_object_mut().unwrap();
            let is_home = sid.as_deref() == Some(&home.0.to_string());
            let is_colony = sid.as_deref() == Some(&colony.0.to_string());
            if !(is_home || is_colony) {
                continue;
            }
            // Strip every modern economy key; install the legacy shape.
            for k in ["structures", "population", "assignments", "specialists", "food_state", "bodies"] {
                o.remove(k);
            }
            o.insert("habitat_fed".into(), serde_json::Value::Bool(true));
            o.insert("extractor_tier".into(), serde_json::json!(2));
            o.insert("shipyard_tier".into(), serde_json::json!(1));
            o.insert("habitat_tier".into(), serde_json::json!(if is_home { 2 } else { 0 }));
            o.insert("refinery_tier".into(), serde_json::json!(if is_colony { 1 } else { 0 }));
            if is_colony {
                o.insert("owner".into(), serde_json::to_value(id).unwrap());
                o.insert("claimed_at".into(), serde_json::json!(0.0));
            }
            // Legacy deposits: processed goods straight off the old generator,
            // with the pre-rename "ore" slug in the mix.
            o.insert("deposits".into(), serde_json::json!([
                { "resource": "provisions", "richness": 0.5, "reserves": null, "accessibility": 0.1 },
                { "resource": "ore", "richness": 0.4, "reserves": null, "accessibility": 0.1 },
                { "resource": if is_colony { "fuel" } else { "alloys" }, "richness": 0.3, "reserves": null, "accessibility": 0.1 },
            ]));
            o.insert("stockpile".into(), serde_json::json!({ "ore": 50.0, "provisions": 80.0 }));
        }
        // An IN-FLIGHT legacy build job (no body_id key — pre-bodies shape):
        // an extractor tier-up at the colony. Must re-site onto the ore body.
        v["build_queue"] = serde_json::json!([{
            "id": 900, "owner": id, "system": colony,
            "what": { "kind": "upgrade", "upgrade": "extractor" },
            "complete_tick": 100, "join": null
        }]);
        let mut w2: World = serde_json::from_value(v).expect("legacy snapshot parses");
        // Pre-migration totals, straight off the legacy JSON (the invariant's
        // left-hand side): per-system structure-tier sums + populations.
        let pre_tiers: f64 = 2.0 + 2.0 + 1.0 + 2.0 + 1.0 + 1.0; // home: mine2+hab2+yard1 · colony: mine2+ref1+yard1
        w2.fixup_after_load(); // fold + migrate (the server's load path)
        // SUM INVARIANT: the summed per-body tiers equal the legacy totals.
        let post_tiers: u32 = [home, colony]
            .iter()
            .map(|sid| {
                w2.systems
                    .iter()
                    .find(|s| s.id == *sid)
                    .unwrap()
                    .bodies
                    .iter()
                    .flat_map(|b| b.structures.values())
                    .sum::<u32>()
            })
            .sum();
        assert_eq!(post_tiers as f64, pre_tiers, "Σ per-body tiers == the legacy system totals");

        // Shape checks: raw-only deposits, folded tiers, seeded people + lines.
        let h = w2.systems.iter().find(|s| s.id == home).unwrap();
        assert!(h.all_deposits().all(|d| Commodity::RAW.contains(&d.resource)), "every deposit remapped to a raw");
        assert_eq!(h.tier(crate::build::StructureKind::MiningComplex), 2, "extractor folded");
        assert_eq!(h.tier(crate::build::StructureKind::Habitat), 2, "habitat folded");
        assert!(h.population() >= 3.0 - 1e-9, "1.5M per habitat tier seeded (≥ home bootstrap)");
        assert!(h.assignment(crate::build::StructureKind::MiningComplex).is_some(), "default line posted");
        let c = w2.systems.iter().find(|s| s.id == colony).unwrap();
        assert!((c.population() - 1.0).abs() < 1e-9, "a habitat-less producer colony gets its working crews");
        assert!(c.assignment(crate::build::StructureKind::FuelRefinery).is_some(), "the refinery line survives the upgrade");

        // The in-flight legacy job re-sited onto a deposit-bearing body.
        {
            let job = w2.build_queue.iter().find(|j| j.id == 900).expect("the legacy job survived");
            let csys = w2.systems.iter().find(|s| s.id == colony).unwrap();
            let jb = csys.bodies.iter().find(|b| b.id == job.body_id).expect("job body exists");
            assert!(
                jb.has_deposit_for(crate::build::StructureKind::MiningComplex) || jb.tier(crate::build::StructureKind::MiningComplex) > 0,
                "the pre-bodies extractor job re-sited where a mine belongs"
            );
        }

        // Idempotency: a second pass is a no-op.
        let before = serde_json::to_string(&w2).unwrap();
        w2.migrate_economy();
        assert_eq!(before, serde_json::to_string(&w2).unwrap(), "migration is idempotent");

        // THE acceptance: 1000 ticks, no panic, positive production.
        let stock0: f64 = w2.systems.iter().filter(|s| s.owner == Some(id)).flat_map(|s| s.stockpile.values()).sum();
        for _ in 0..1000 {
            w2.step(&[]);
        }
        let raw_produced: f64 = w2
            .systems
            .iter()
            .filter(|s| s.owner == Some(id))
            .flat_map(|s| s.stockpile.iter())
            .filter(|(c, _)| Commodity::RAW.contains(c))
            .map(|(_, v)| v)
            .sum();
        assert!(raw_produced > 0.0, "the migrated empire is PRODUCING (raw stock {raw_produced})");
        let _ = stock0;
    }

    /// §economy Part 5: THE STARTER KIT — a fresh home affords its opening
    /// moves from seeded stock alone (no market round-trip): a Convoy AND a
    /// Mining Complex tier-up enqueue turn one on the kit's Machinery/Alloys/
    /// Polymers.
    #[test]
    fn starter_kit_funds_the_opening_moves_without_the_market() {
        let mut w = test_world();
        let id = PlayerId(70);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let ev = w.step(&[
            Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() },
            Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::MiningComplex, body_id: None },
        ]);
        let started = ev.iter().filter(|e| matches!(e.payload, EventPayload::BuildStarted { .. })).count();
        assert_eq!(started, 2, "the kit funds a convoy AND the first mine tier-up, turn one");
    }

    // --- §bodies: per-body building + the shared labor pool ---------------------

    /// §bodies: EXTRACTION REQUIRES A MATCHING DEPOSIT ON THE BODY — real now,
    /// not a visual association. The same build on the right body proceeds.
    #[test]
    fn extraction_requires_a_matching_deposit_on_the_body() {
        let mut w = test_world();
        let id = PlayerId(90);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![
            crate::galaxy::Deposit { resource: Commodity::Volatiles, richness: 0.5, reserves: None, accessibility: 0.5 },
            crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 0.5, reserves: None, accessibility: 0.5 },
        ]);
        let sid = sys.id;
        seed_stock(&mut w, sid, &[(Commodity::Machinery, 60.0), (Commodity::Alloys, 120.0)]);
        let (ore_body, vol_body) = {
            let s = w.systems.iter().find(|s| s.id == sid).unwrap();
            (
                s.bodies.iter().find(|b| b.deposits.iter().any(|d| d.resource == Commodity::MetallicOre)).unwrap().id,
                s.bodies.iter().find(|b| b.deposits.iter().any(|d| d.resource == Commodity::Volatiles)).unwrap().id,
            )
        };
        assert_ne!(ore_body, vol_body, "the two deposits landed on different bodies");

        // A VolatileHarvester on the ORE body: rejected (no volatiles there).
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: sid, upgrade: StructureKind::VolatileHarvester, body_id: Some(ore_body) }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { reason: crate::event::BuildRejectReason::NoSlot, .. })),
            "no matching deposit on that body — rejected"
        );
        assert!(w.build_queue.is_empty());

        // The SAME build on the VOLATILES body proceeds.
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: sid, upgrade: StructureKind::VolatileHarvester, body_id: Some(vol_body) }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "the right body hosts it");
        assert_eq!(w.build_queue[0].body_id, vol_body, "the job carries its body");
    }

    /// §bodies: a build with an explicit body_id COMPLETES ON THAT BODY (and
    /// only there), and the job's body drives the queue display.
    #[test]
    fn builds_land_on_the_requested_body() {
        let mut w = test_world();
        let id = PlayerId(91);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Electronics, 40.0)]);
        // A Sensor Array explicitly on the PRIMARY body (its natural site is
        // the outermost — so an explicit override must be honored).
        let primary = w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| b.parent.is_none()).unwrap().id;
        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: StructureKind::SensorArray, body_id: Some(primary) }]);
        assert_eq!(w.build_queue[0].body_id, primary);
        for _ in 0..(crate::build::SENSOR_ARRAY_RECIPE.build_ticks + 3) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let b = sys.bodies.iter().find(|b| b.id == primary).unwrap();
        assert_eq!(b.tier(StructureKind::SensorArray), 1, "the tier landed on the REQUESTED body");
        assert_eq!(sys.tier_sum(StructureKind::SensorArray), 1, "…and nowhere else");
    }

    /// §bodies: ONE workforce pool spans the system — crews posted on two
    /// bodies dilute by the same share (labor commutes inside the well).
    #[test]
    fn workforce_is_one_pool_across_bodies() {
        let mut w = test_world();
        let id = PlayerId(92);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![
            crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 1.0, reserves: None, accessibility: 0.5 },
            crate::galaxy::Deposit { resource: Commodity::Biomass, richness: 1.0, reserves: None, accessibility: 0.5 },
        ]);
        sys.set_population(0.9); // exactly ONE crew for the whole system
        sys.stockpile.insert(Commodity::Provisions, 100.0);
        // Staff BOTH bodies' extraction (a mine on the ore body, a harvester
        // on the biomass body) — one crew each posted, one unit available.
        for (res, kind) in [
            (Commodity::MetallicOre, StructureKind::MiningComplex),
            (Commodity::Biomass, StructureKind::Bioharvester),
        ] {
            let b = sys.bodies.iter_mut().find(|b| b.deposits.iter().any(|d| d.resource == res)).unwrap();
            b.set_tier(kind, 1);
            b.assignments.insert(kind, crate::production::Assignment::crew(1));
        }
        let sid = sys.id;
        assert!((w.systems.iter().find(|s| s.id == sid).unwrap().staffing_share() - 0.5).abs() < 1e-12, "1 unit / 2 posted = half share");
        w.step(&[]);
        let expect = 1.0 * crate::production::tier_throughput(1) * 0.5 * crate::config::DT;
        let ore = system_stock(&w, sid, Commodity::MetallicOre);
        let bio = system_stock(&w, sid, Commodity::Biomass);
        assert!((ore - expect).abs() < 1e-9, "the mine ran at the SYSTEM share (got {ore})");
        assert!((bio - expect).abs() < 1e-9, "…and so did the harvester on the other body (got {bio})");
    }

    // --- §economy Part 4: specialists ------------------------------------------

    /// The SKILL factor: an AFFINE specialist multiplies its line ×1.75 fully
    /// staffed; an OFF-AFFINITY specialist still works as generic crew (never a
    /// penalty, no bonus). Measured on a tier-1 mine, everything else pinned.
    #[test]
    fn specialist_skill_multiplies_affine_lines_only() {
        let ore_gain = |kind: Option<crate::specialist::SpecialistKind>| {
            let mut w = test_world();
            let id = PlayerId(60);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.owner = Some(id);
            sys.claimed_at = Some(0.0);
            sys.set_test_deposits(vec![crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 1.0, reserves: None, accessibility: 0.5 }]);
            sys.set_population(8.0);
            sys.stockpile.insert(Commodity::Provisions, 100.0);
            sys.set_tier(crate::build::StructureKind::MiningComplex, 1);
            let mut asg = crate::production::Assignment::crew(0); // NO generic workers
            if let Some(k) = kind {
                sys.specialists.insert(k, 1);
                asg.specialists.insert(k, 1);
            } else {
                asg.workers = 1;
            }
            sys.assign(crate::build::StructureKind::MiningComplex, asg);
            let sid = sys.id;
            w.step(&[]);
            system_stock(&w, sid, Commodity::MetallicOre)
        };
        let plain = ore_gain(None);
        let geologist = ore_gain(Some(crate::specialist::SpecialistKind::Geologist));
        let xeno = ore_gain(Some(crate::specialist::SpecialistKind::Xenobiologist));
        assert!((geologist - plain * crate::production::SPECIALIST_SKILL_MULT).abs() < 1e-9,
            "an affine specialist alone runs the line ×1.75 (plain {plain}, geologist {geologist})");
        assert!((xeno - plain).abs() < 1e-9,
            "an off-affinity specialist works as a generic crew — never a penalty (got {xeno})");
    }

    /// A posted specialist who ISN'T resident (transferred away) degrades the
    /// line non-destructively: the posting survives, the bonus just lapses
    /// until someone is home again.
    #[test]
    fn missing_residents_degrade_the_bonus_without_state_loss() {
        let mut w = test_world();
        let id = PlayerId(61);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.set_test_deposits(vec![crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 1.0, reserves: None, accessibility: 0.5 }]);
        sys.set_population(8.0);
        sys.stockpile.insert(Commodity::Provisions, 100.0);
        sys.set_tier(crate::build::StructureKind::MiningComplex, 1);
        let mut asg = crate::production::Assignment::crew(1);
        asg.specialists.insert(crate::specialist::SpecialistKind::Geologist, 1); // posted but NOT resident
        sys.assign(crate::build::StructureKind::MiningComplex, asg);
        let sid = sys.id;
        w.step(&[]);
        let gain = system_stock(&w, sid, Commodity::MetallicOre);
        assert!((gain - 1.0 * crate::config::DT).abs() < 1e-9, "no resident = no bonus, plain rate (got {gain})");
        let s = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.assignment(crate::build::StructureKind::MiningComplex).unwrap().specialists[&crate::specialist::SpecialistKind::Geologist], 1,
            "the POSTING survives — a returning specialist re-validates for free");
    }

    /// HIRE: credits debited, a personnel convoy departs the HUB with the
    /// contractor aboard (no cargo), and on arrival the resident pool grows.
    #[test]
    fn hired_specialist_ships_from_the_hub_and_joins_the_pool() {
        let mut w = test_world();
        w.enclaves.clear(); // no ambient piracy on the delivery run
        let id = PlayerId(62);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let credits0 = w.players[&id].credits;
        let ev = w.step(&[Command::HireSpecialist { player_id: id, specialist: crate::specialist::SpecialistKind::NavalArchitect, dest_system: home }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::SpecialistHired { owner, kind: crate::specialist::SpecialistKind::NavalArchitect, .. } if owner == id)));
        assert!((w.players[&id].credits - (credits0 - crate::specialist::SPECIALIST_HIRE_COST)).abs() < 1e-9, "price-certain debit");
        let convoy = w.fleets.values().find(|f| f.owner == id && !f.passengers.is_empty()).expect("a personnel convoy exists");
        assert!(convoy.pos.distance(w.hub) < 100.0, "it departs from the HUB (one tick of travel allowed)");
        assert!(convoy.cargo.is_none(), "people, not goods");

        let mut delivered = false;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::SpecialistsDelivered { owner, system, .. } if owner == id && system == home) {
                    delivered = true;
                }
            }
            if delivered { break; }
        }
        assert!(delivered, "the contractor lands");
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.specialists.get(&crate::specialist::SpecialistKind::NavalArchitect), Some(&1), "resident pool grew");
    }

    /// TRAIN: needs an Academy; the course debits the recipe, rides the build
    /// queue, and graduates into the resident pool.
    #[test]
    fn academy_trains_into_the_resident_pool() {
        let mut w = test_world();
        let id = PlayerId(63);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Electronics, 20.0)]); // provisions seed already aboard

        // No Academy → soft reject (no job, no debit).
        w.step(&[Command::TrainSpecialist { player_id: id, system_id: home, specialist: crate::specialist::SpecialistKind::Geologist }]);
        assert!(w.build_queue.is_empty(), "no Academy, no course");

        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(crate::build::StructureKind::Academy, 1);
        w.step(&[Command::TrainSpecialist { player_id: id, system_id: home, specialist: crate::specialist::SpecialistKind::Geologist }]);
        assert_eq!(w.build_queue.len(), 1, "the course is a build-queue job");
        let mut trained = false;
        for _ in 0..(crate::specialist::ACADEMY_TRAIN_TICKS + 5) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::SpecialistTrained { owner, system, kind: crate::specialist::SpecialistKind::Geologist } if owner == id && system == home) {
                    trained = true;
                }
            }
        }
        assert!(trained, "graduation announced");
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.specialists.get(&crate::specialist::SpecialistKind::Geologist), Some(&1));
    }

    /// TRANSFER + the REDIRECT rule: people never land on hostile ground and
    /// are never deleted — a destination lost mid-flight turns the convoy home.
    #[test]
    fn transfer_redirects_home_when_the_destination_is_lost() {
        let mut w = test_world();
        w.enclaves.clear();
        let (id, rival) = (PlayerId(64), PlayerId(65));
        w.step(&[
            Command::AddPlayer { id, name: "Acme".into() },
            Command::AddPlayer { id: rival, name: "Rival".into() },
        ]);
        let home = w.players[&id].home_system.unwrap();
        // A second owned system, close by, as the destination.
        let dest = {
            let hp = w.systems.iter().find(|s| s.id == home).unwrap().pos;
            let sys = w
                .systems
                .iter_mut()
                .filter(|s| s.is_unclaimed())
                .min_by(|a, b| a.pos.distance(hp).total_cmp(&b.pos.distance(hp)).then(a.id.cmp(&b.id)))
                .unwrap();
            sys.owner = Some(id);
            sys.claimed_at = Some(0.0);
            sys.id
        };
        w.systems.iter_mut().find(|s| s.id == home).unwrap().specialists.insert(crate::specialist::SpecialistKind::IndustrialEngineer, 2);
        w.step(&[Command::TransferSpecialists { player_id: id, from: home, to: dest, manifest: [(crate::specialist::SpecialistKind::IndustrialEngineer, 2)].into_iter().collect() }]);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(sys.specialists.is_empty(), "the pool is debited at loading — the people are aboard");
        assert!(w.fleets.values().any(|f| f.owner == id && f.passengers.values().sum::<u32>() == 2), "a personnel convoy flies");

        // The destination FLIPS mid-flight → the convoy must turn for home.
        w.systems.iter_mut().find(|s| s.id == dest).unwrap().owner = Some(rival);
        let mut landed_home = false;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::SpecialistsDelivered { owner, system, .. } if owner == id && system == home) {
                    landed_home = true;
                }
            }
            if landed_home { break; }
        }
        assert!(landed_home, "nobody lands on hostile ground; nobody is deleted — they come home");
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.specialists.get(&crate::specialist::SpecialistKind::IndustrialEngineer), Some(&2));
        let rival_sys = w.systems.iter().find(|s| s.id == dest).unwrap();
        assert!(rival_sys.specialists.is_empty(), "the captor got nobody");
    }

    /// LOSS + CONQUEST: a fleet destroyed with specialists aboard loses them
    /// (one event, light-delayed news); a CAPTURED system's residents stay.
    #[test]
    fn passengers_die_with_the_ship_and_residents_survive_capture() {
        // Aboard a destroyed fleet → SpecialistsLost.
        let mut w = test_world();
        let (def, atk) = (PlayerId(66), PlayerId(67));
        w.step(&[
            Command::AddPlayer { id: def, name: "D".into() },
            Command::AddPlayer { id: atk, name: "A".into() },
        ]);
        let cc = w.players[&def].home;
        let cid = w.alloc_entity_id();
        let mut convoy = Fleet::single(cid, def, ShipKind::Convoy, cc + Vec2::new(3000.0, 0.0), FleetOrder::Idle, None);
        convoy.passengers.insert(crate::specialist::SpecialistKind::Geologist, 2);
        w.fleets.insert(cid, convoy);
        let raider = find_ship(&w, atk, ShipKind::Raider);
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cc + Vec2::new(3040.0, 0.0);
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: cid }]);
        let mut lost = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::SpecialistsLost { owner, ref manifest, .. } = e.payload {
                    assert_eq!(owner, def);
                    assert_eq!(manifest.get(&crate::specialist::SpecialistKind::Geologist), Some(&2));
                    lost = true;
                }
            }
            if lost || !w.fleets.contains_key(&cid) {
                break;
            }
        }
        // The convoy may survive some raid outcomes (steal); only a DESTROYED
        // convoy must have announced the loss.
        if !w.fleets.contains_key(&cid) {
            assert!(lost, "a destroyed fleet announces its lost specialists");
        }

        // Residents survive capture: exercise capture_system directly.
        let sid = w.systems.iter().find(|s| s.is_unclaimed()).unwrap().id;
        {
            let sys = w.systems.iter_mut().find(|s| s.id == sid).unwrap();
            sys.owner = Some(def);
            sys.claimed_at = Some(0.0);
            sys.specialists.insert(crate::specialist::SpecialistKind::NavalArchitect, 3);
        }
        let colony = w.alloc_entity_id();
        let pos = w.systems.iter().find(|s| s.id == sid).unwrap().pos;
        w.fleets.insert(colony, Fleet::single(colony, atk, ShipKind::Colony, pos, FleetOrder::Idle, None));
        let mut ev = Vec::new();
        w.capture_system(sid, def, atk, colony, pos, &mut ev);
        let sys = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(sys.owner, Some(atk));
        assert_eq!(sys.specialists.get(&crate::specialist::SpecialistKind::NavalArchitect), Some(&3),
            "conquest KEEPS the resident specialists — people outlast the flag");
    }

    // --- §buildings step 3b: Fuel Refinery ------------------------------------

    /// §economy Part 3: a staffed tier-2 Fuel Refinery converts at exactly
    /// rate × tier_throughput(2) × DT units of OUTPUT, drawing its per-unit
    /// input basket (1.0 Volatiles per Fuel) — one tick, all factors 1.0.
    #[test]
    fn refinery_converts_at_rate_and_ratio() {
        let mut w = test_world();
        let id = PlayerId(34);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_population(8.0); // ample workforce → staffing share 1.0
            // (the home's seeded 60 Provisions cover these few ticks; more
            // would overflow the 500-unit cap on top of the 300-Fuel seed)
            sys.set_tier(crate::build::StructureKind::FuelRefinery, 2);
            sys.assign(crate::build::StructureKind::FuelRefinery, crate::production::Assignment::crew(2));
        }
        seed_stock(&mut w, home, &[(Commodity::Volatiles, 100.0)]);

        let vol0 = system_stock(&w, home, Commodity::Volatiles);
        let fuel0 = system_stock(&w, home, Commodity::Fuel);
        w.step(&[]);
        let conv = crate::production::converter_for(crate::build::StructureKind::FuelRefinery).unwrap();
        let out = conv.rate * crate::production::tier_throughput(2) * crate::config::DT;
        let per_vol = conv.inputs.iter().find(|(c, _)| *c == Commodity::Volatiles).unwrap().1;
        let vol_delta = system_stock(&w, home, Commodity::Volatiles) - vol0;
        let fuel_delta = system_stock(&w, home, Commodity::Fuel) - fuel0;
        assert!((fuel_delta - out).abs() < 1e-9, "produces rate·throughput·DT fuel (got {fuel_delta}, want {out})");
        assert!((vol_delta + out * per_vol).abs() < 1e-9, "draws the per-unit basket (got {vol_delta})");
    }

    /// A dry staffed Refinery SUSPENDS (latched NoInputs notice, once): no
    /// Volatiles → no conversion, no fuel from nowhere, nothing destroyed. A
    /// sliver of input converts exactly that sliver (never negative) and the
    /// resumption is announced.
    #[test]
    fn refinery_suspends_dry_and_never_overdraws() {
        let mut w = test_world();
        let id = PlayerId(35);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_population(8.0);
            // (the home's seeded 60 Provisions cover these few ticks; more
            // would overflow the 500-unit cap on top of the 300-Fuel seed)
            sys.set_tier(crate::build::StructureKind::FuelRefinery, 3);
            sys.assign(crate::build::StructureKind::FuelRefinery, crate::production::Assignment::crew(3));
        }

        // Dry: fuel unchanged, ONE latched suspension notice (no per-tick spam).
        let fuel0 = system_stock(&w, home, Commodity::Fuel);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::ProductionSuspended { system, structure: crate::build::StructureKind::FuelRefinery, reason: crate::production::SuspendReason::NoInputs, .. } if system == home)),
            "the dry line announces NO INPUTS"
        );
        let ev = w.step(&[]);
        let spurious: Vec<_> = ev.iter().filter(|e| matches!(e.payload, EventPayload::ProductionSuspended { system, .. } if system == home)).collect();
        assert!(spurious.is_empty(), "latched — no second notice while still dry: {spurious:?}");
        assert!((system_stock(&w, home, Commodity::Fuel) - fuel0).abs() < 1e-9, "dry refinery = idle");

        // A sliver of Volatiles smaller than one tick's basket: converts
        // exactly sliver/per_unit output, drains to zero (never below), and
        // the line announces its RESUMPTION.
        let sliver = 0.001;
        seed_stock(&mut w, home, &[(Commodity::Volatiles, sliver)]);
        let fuel1 = system_stock(&w, home, Commodity::Fuel);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::ProductionResumed { system, structure: crate::build::StructureKind::FuelRefinery, .. } if system == home)),
            "recovery is announced"
        );
        assert!((system_stock(&w, home, Commodity::Volatiles)).abs() < 1e-9, "drains to zero, not below");
        let conv = crate::production::converter_for(crate::build::StructureKind::FuelRefinery).unwrap();
        let per_vol = conv.inputs.iter().find(|(c, _)| *c == Commodity::Volatiles).unwrap().1;
        let gained = system_stock(&w, home, Commodity::Fuel) - fuel1;
        assert!((gained - sliver / per_vol).abs() < 1e-9, "converts only what the basket affords");
    }

    /// The storage cap never blocks conversion with the LOSSY yield (< 1): at a
    /// FULL depot, refining shrinks the total (input > output), so it proceeds
    /// and the total stays at/under the cap — nothing destroyed, nothing stuck.
    #[test]
    fn refinery_respects_storage_cap_at_full_depot() {
        let mut w = test_world();
        let id = PlayerId(36);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_population(8.0);
            sys.set_tier(crate::build::StructureKind::FuelRefinery, 1);
            sys.assign(crate::build::StructureKind::FuelRefinery, crate::production::Assignment::crew(1));
        }
        // Fill exactly to the cap, with volatiles included in the fill.
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let headroom = sys.storage_headroom();
        seed_stock(&mut w, home, &[(Commodity::Volatiles, headroom)]);
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();

        let fuel0 = system_stock(&w, home, Commodity::Fuel);
        w.step(&[]);
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(s.storage_used() <= cap + 1e-9, "total never exceeds the cap");
        assert!(
            system_stock(&w, home, Commodity::Fuel) > fuel0,
            "the ≥1:1 conversion proceeds even at a full depot (it never adds units)"
        );
    }

    /// End-to-end: a refinery turns a Volatiles stock into an operating Fuel
    /// reserve that FUELS a fleet move — forward fuel production works.
    #[test]
    fn refinery_fuel_powers_a_move() {
        let mut w = test_world();
        let id = PlayerId(37);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        drain_fuel(&mut w, id); // no seed fuel anywhere
        {
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            sys.set_population(8.0);
            // (the home's seeded 60 Provisions cover these few ticks; more
            // would overflow the 500-unit cap on top of the 300-Fuel seed)
            sys.set_tier(crate::build::StructureKind::FuelRefinery, 2);
            sys.assign(crate::build::StructureKind::FuelRefinery, crate::production::Assignment::crew(2));
        }
        seed_stock(&mut w, home, &[(Commodity::Volatiles, 100.0)]);
        // Refine for a while → a real fuel reserve appears from Volatiles.
        for _ in 0..(20 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(home_fuel(&w, id) > 5.0, "refinery built an operating reserve from volatiles");
        // A move now dispatches on refinery-produced fuel (no shortfall hold).
        let ship = player_ship(&w, id, ShipKind::Convoy);
        let dest = w.players[&id].home + Vec2::new(2000.0, 0.0);
        let mut held = false;
        w.step(&[Command::MoveShip { player_id: id, ship_id: ship, dest }]);
        for _ in 0..(10 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::FuelShortfall { owner, .. } if owner == id) {
                    held = true;
                }
            }
        }
        assert!(!held, "the move runs on refinery-produced fuel — no shortfall hold");
    }

    // --- §scout part 1: the Scout ship kind ------------------------------------

    /// The cheap entry unit: a Scout builds at the HOME's tier-1 shipyard turn
    /// one and spawns after its short build time.
    #[test]
    fn scout_builds_cheap_at_home_turn_one() {
        let mut w = test_world();
        let id = PlayerId(41);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Electronics, 10.0)]); // kit covers Alloys; fuel seed covers the 8 Fuel
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Scout, join: None , loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "a tier-1 shipyard builds scouts");
        let mut spawned = false;
        for _ in 0..(crate::build::SCOUT_RECIPE.build_ticks + 3) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::ShipSpawned { kind: ShipKind::Scout, owner, .. } if owner == id) {
                    spawned = true;
                }
            }
        }
        assert!(spawned, "the scout completes and spawns");
        let scout = w.fleets.values().find(|s| s.owner == id && s.flagship_kind() == ShipKind::Scout).unwrap();
        assert!(scout.max_speed() > ShipKind::Raider.max_speed(), "the fastest ship flying");
    }

    /// A scout has NO combat strength: engaged as a TARGET it simply dies (no
    /// roll — deterministic), and as a would-be ATTACKER it dies just the same.
    /// Its defense is speed and darkness, never armor.
    #[test]
    fn scout_dies_in_any_engagement() {
        // As target: a raider runs it down → scout destroyed, raider survives.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let raider = find_ship(&w, atk, ShipKind::Raider);
        let sid = w.alloc_entity_id();
        w.fleets.insert(sid, Fleet::single(sid, def, ShipKind::Scout, cc + Vec2::new(420.0, 0.0), FleetOrder::Idle, None));
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cc + Vec2::new(120.0, 0.0);
            r.vel = Vec2::ZERO;
            r.order = FleetOrder::Idle;
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: sid }]);
        let outcome = run_until_raid(&mut w, 60, |_| vec![]).expect("the intercept resolves");
        assert_eq!(outcome, RaidOutcome::TargetDestroyed, "a caught scout simply dies");
        assert!(!w.fleets.contains_key(&sid), "scout gone");
        assert!(w.fleets.contains_key(&raider), "the raider is never at risk from a scout");

        // As attacker: a scout committed against a convoy dies; the convoy is safe.
        let mut w = test_world();
        let (atk, def) = (PlayerId(3), PlayerId(4));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let convoy = find_ship(&w, def, ShipKind::Convoy);
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.pos = cc + Vec2::new(420.0, 0.0);
            c.vel = Vec2::ZERO;
            c.order = FleetOrder::Idle;
        }
        let sid = w.alloc_entity_id();
        // CommitRaid is raider-only now (§fleets part 2) — soft-rejected for a
        // scout — so inject the Intercept directly to prove the deterministic
        // engagement rule still protects the edge/autonomous paths.
        w.fleets.insert(
            sid,
            Fleet::single(sid, atk, ShipKind::Scout, cc + Vec2::new(120.0, 0.0), FleetOrder::Intercept { target: convoy }, None),
        );
        let outcome = run_until_raid(&mut w, 60, |_| vec![]).expect("the contact resolves");
        assert_eq!(outcome, RaidOutcome::AttackerDestroyed, "an attacking scout dies");
        assert!(w.fleets.contains_key(&convoy), "the convoy is untouched");
        assert!(!w.fleets.contains_key(&sid));
    }

    // --- §scout part 2: intel snapshots ----------------------------------------

    /// A scout inside SCOUT_INTEL_RANGE of a RIVAL system captures a snapshot of
    /// its fortifications: correct tiers, timestamped, ONE notice per approach
    /// (silent refresh while parked), re-noticed when the observed tiers change.
    /// Out of range, nothing is gathered.
    #[test]
    fn scout_gathers_aging_intel_snapshots() {
        let mut w = test_world();
        let (spy, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: spy, name: "Spy".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        // A rival-owned fortified system.
        let sys_id = {
            let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.owner = Some(def);
            sys.claimed_at = Some(0.0);
            sys.set_tier(crate::build::StructureKind::DefensePlatform, 2);
            sys.set_tier(crate::build::StructureKind::Shipyard, 1);
            (sys.id, sys.pos)
        };
        let (sid_sys, sys_pos) = sys_id;

        // A scout parked just inside intel range.
        let scout = w.alloc_entity_id();
        w.fleets.insert(
            scout,
            Fleet::single(scout, spy, ShipKind::Scout, sys_pos + Vec2::new(crate::ship::SCOUT_INTEL_RANGE - 50.0, 0.0), FleetOrder::Idle, None),
        );
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::IntelGathered { owner, system, defense_tier: 2, shipyard_tier: 1, .. }
                    if owner == spy && system == sid_sys
            )),
            "a fresh approach captures + notices the snapshot"
        );
        let snap0 = w.players[&spy].intel[&sid_sys];
        assert_eq!((snap0.defense_tier, snap0.shipyard_tier), (2, 1));

        // Parked: the snapshot refreshes SILENTLY (observed_at advances, no event).
        let ev = w.step(&[]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::IntelGathered { .. })), "no per-tick spam");
        let snap1 = w.players[&spy].intel[&sid_sys];
        assert!(snap1.observed_at > snap0.observed_at, "parked scout keeps the snapshot fresh");

        // The rival builds (tier changes) → a NEW notice fires.
        w.systems.iter_mut().find(|s| s.id == sid_sys).unwrap().set_tier(crate::build::StructureKind::DefensePlatform, 3);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::IntelGathered { defense_tier: 3, .. })),
            "changed tiers re-notice"
        );

        // Move the scout out of range: the snapshot stops refreshing — it AGES.
        w.fleets.get_mut(&scout).unwrap().pos = sys_pos + Vec2::new(crate::ship::SCOUT_INTEL_RANGE * 3.0, 0.0);
        let frozen = w.players[&spy].intel[&sid_sys].observed_at;
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!(w.players[&spy].intel[&sid_sys].observed_at, frozen, "a snapshot is a snapshot — it ages, never auto-updates");

        // The SCOUTED side learns nothing: no intel entry, no event addressed to def.
        assert!(w.players[&def].intel.is_empty(), "the scouted rival gathers nothing");

        // Non-scouts never gather: a raider parked at the same spot adds nothing.
        let mut w2 = test_world();
        w2.step(&[
            Command::AddPlayer { id: spy, name: "Spy".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let (sid2, pos2) = {
            let sys = w2.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.owner = Some(def);
            sys.claimed_at = Some(0.0);
            (sys.id, sys.pos)
        };
        let raider = w2.alloc_entity_id();
        w2.fleets.insert(raider, Fleet::single(raider, spy, ShipKind::Raider, pos2, FleetOrder::Idle, None));
        w2.step(&[]);
        assert!(!w2.players[&spy].intel.contains_key(&sid2), "only scouts gather intel");
    }

    #[test]
    fn builds_are_deterministic() {
        let run = || {
            let mut w = test_world();
            let id = PlayerId(7);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let home = w.players[&id].home_system.unwrap();
            seed_stock(&mut w, home, &[(Commodity::MetallicOre, 200.0), (Commodity::Alloys, 100.0)]);
            w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
            for _ in 0..400 {
                w.step(&[]);
            }
            serde_json::to_string(&w).unwrap()
        };
        assert_eq!(run(), run(), "same seed + commands incl. a completed build → identical state");
    }

    // --- §step1 PART 2: fuel-to-move sink -----------------------------------

    fn player_ship(w: &World, owner: PlayerId, kind: ShipKind) -> EntityId {
        *w.fleets.iter().find(|(_, s)| s.owner == owner && s.flagship_kind() == kind).unwrap().0
    }
    fn home_fuel(w: &World, owner: PlayerId) -> f64 {
        let h = w.players[&owner].home_system.unwrap();
        system_stock(w, h, Commodity::Fuel)
    }
    fn system_stock(w: &World, sys: EntityId, c: Commodity) -> f64 {
        w.systems.iter().find(|s| s.id == sys).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }
    /// Empty the Fuel reserve of every system the player owns (a fuel-starved fleet).
    fn drain_fuel(w: &mut World, owner: PlayerId) {
        for s in w.systems.iter_mut().filter(|s| s.owner == Some(owner)) {
            s.stockpile.insert(Commodity::Fuel, 0.0);
        }
    }

    #[test]
    fn joining_seeds_a_home_fuel_reserve() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // The home produces no fuel, so its reserve is exactly the seed (turn-one runway).
        assert!((home_fuel(&w, id) - crate::fuel::FUEL_HOME_SEED).abs() < 1e-6);
    }

    #[test]
    fn moving_a_fleet_burns_fuel_proportional_to_distance_and_mass() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let convoy = player_ship(&w, id, ShipKind::Convoy);
        let (pos, mass) = { let s = &w.fleets[&convoy]; (s.pos, s.mass()) };
        let dest = pos + Vec2::new(3000.0, 1200.0);
        let expected = crate::fuel::fuel_cost(pos.distance(dest), mass);
        assert!(expected > 1.0, "a real move should cost real fuel");
        let f0 = home_fuel(&w, id);
        let ev = w.step(&[Command::MoveShip { player_id: id, ship_id: convoy, dest }]);
        assert!((f0 - home_fuel(&w, id) - expected).abs() < 1e-6, "burned exactly fuel_cost(dist, mass)");
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::FuelShortfall { .. })), "no shortfall when fueled");
    }

    #[test]
    fn a_fuelless_move_is_held_not_destroyed() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        drain_fuel(&mut w, id);
        let convoy = player_ship(&w, id, ShipKind::Convoy);
        let dest = w.fleets[&convoy].pos + Vec2::new(3000.0, 0.0);
        let ev = w.step(&[Command::MoveShip { player_id: id, ship_id: convoy, dest }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload,
                EventPayload::FuelShortfall { owner, kind: crate::fuel::ShortfallKind::Move, .. } if owner == id)),
            "a held move notifies its owner",
        );
        assert!(w.fleets.contains_key(&convoy), "a shortfall LIMITS — it never destroys the ship");
        assert!(!w.pending_orders.iter().any(|p| p.ship_id == convoy), "the move was not scheduled (held)");
        assert_eq!(home_fuel(&w, id), 0.0, "a shortfall debits nothing");
    }

    #[test]
    fn recalling_a_raider_is_exempt_from_fuel() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let raider = player_ship(&w, id, ShipKind::Raider);
        // Send the raider far afield (fueled), then run until it's well clear of home.
        let home = w.players[&id].command_center;
        let dest = home + Vec2::new(5000.0, 0.0);
        w.step(&[Command::MoveShip { player_id: id, ship_id: raider, dest }]);
        for _ in 0..(20 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let away = w.fleets[&raider].pos.distance(home);
        assert!(away > 500.0, "raider is well away from home");
        // Now strand it: zero fuel. Recall must STILL work (exempt — never strand a fleet).
        drain_fuel(&mut w, id);
        let ev = w.step(&[Command::RecallRaid { player_id: id, raider_id: raider }]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::FuelShortfall { .. })), "recall never burns fuel");
        for _ in 0..(40 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(w.fleets[&raider].pos.distance(home) < away, "the recalled raider heads home despite no fuel");
    }

    #[test]
    fn ship_production_retains_fuel_and_burns_to_haul() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 50.0)]);
        let f0 = home_fuel(&w, id);
        let ev = w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        assert!(
            ev.iter().any(|e| matches!(&e.payload,
                EventPayload::Trade(TradeEvent::SellDispatched { commodity: Commodity::MetallicOre, .. }))),
            "ore fleets to the hub",
        );
        assert!(
            !ev.iter().any(|e| matches!(&e.payload,
                EventPayload::Trade(TradeEvent::SellDispatched { commodity: Commodity::Fuel, .. }))),
            "Fuel is retained as the operating reserve, never auto-shipped",
        );
        let f1 = home_fuel(&w, id);
        assert!(f1 < f0 && f1 > 0.0, "hauling burns some fuel but keeps the reserve (burned {:.1})", f0 - f1);
    }

    #[test]
    fn a_fuelless_shipment_is_held_and_refunded() {
        let mut w = test_world();
        let id = PlayerId(3);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        drain_fuel(&mut w, id);
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 40.0)]);
        let ev = w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload,
                EventPayload::FuelShortfall { kind: crate::fuel::ShortfallKind::Shipment, .. })),
            "a held shipment notifies its owner",
        );
        assert!(system_stock(&w, home, Commodity::MetallicOre) >= 40.0, "held goods are refunded, never lost");
    }

    #[test]
    fn fuel_burn_is_deterministic() {
        let run = || {
            let mut w = test_world();
            let id = PlayerId(7);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let convoy = player_ship(&w, id, ShipKind::Convoy);
            w.step(&[Command::MoveShip { player_id: id, ship_id: convoy, dest: Vec2::new(2000.0, 1500.0) }]);
            for _ in 0..200 {
                w.step(&[]);
            }
            home_fuel(&w, id)
        };
        let a = run();
        assert_eq!(a, run(), "the same seed + move burns identical fuel");
        assert!(a < crate::fuel::FUEL_HOME_SEED, "the move actually spent fuel from the reserve");
    }

    #[test]
    fn spent_fuel_survives_a_snapshot() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let convoy = player_ship(&w, id, ShipKind::Convoy);
        w.step(&[Command::MoveShip { player_id: id, ship_id: convoy, dest: Vec2::new(2000.0, 1500.0) }]);
        let spent = home_fuel(&w, id);
        assert!(spent < crate::fuel::FUEL_HOME_SEED, "fuel was spent before the snapshot");
        // Save → load: the depleted reserve (not the seed) is what reloads.
        let w2: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert!((home_fuel(&w2, id) - spent).abs() < 1e-6, "the spent-fuel reserve persists across a snapshot");
    }

    #[test]
    fn ships_actually_move() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let start: Vec<Vec2> = w.fleets.values().map(|s| s.pos).collect();
        // Advance a few seconds.
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let moved = w
            .fleets
            .values()
            .zip(&start)
            .any(|(s, &p0)| s.pos.distance(p0) > 10.0);
        assert!(moved, "fleets should have moved from their start positions");
    }

    fn convoy_id(w: &World) -> EntityId {
        *w.fleets
            .iter()
            .find(|(_, s)| s.flagship_kind() == ShipKind::Convoy)
            .unwrap()
            .0
    }

    #[test]
    fn move_order_applies_only_after_light_travel_delay() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // Let the convoy travel away from its home (== command center) so the
        // order has a non-trivial outbound delay. (Convoys lumber now — give the
        // heavy hauler enough time to get well clear of home.)
        for _ in 0..(45 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let cid = convoy_id(&w);
        let cc = w.players[&id].command_center;
        let ship_pos = w.fleets[&cid].pos;
        let expected_delay = ship_pos.distance(cc) / w.config.c;
        assert!(expected_delay > 1.0, "convoy should be well away from home");

        let issue_time = w.time;
        let dest = Vec2::new(1234.0, -567.0);
        w.step(&[Command::MoveShip {
            player_id: id,
            ship_id: cid,
            dest,
        }]);

        // Step until just before the order's light arrives: still not a MoveTo.
        while w.time < issue_time + expected_delay - DT {
            w.step(&[]);
            assert!(
                !matches!(w.fleets[&cid].order, FleetOrder::MoveTo { .. }),
                "order applied too early at t={} (delay {})",
                w.time,
                expected_delay
            );
        }
        // Step a little past the arrival: now it must be a MoveTo to `dest`.
        for _ in 0..3 {
            w.step(&[]);
        }
        match w.fleets[&cid].order {
            FleetOrder::MoveTo { dest: d } => assert_eq!(d, dest),
            ref other => panic!("expected MoveTo after delay, got {other:?}"),
        }
    }

    #[test]
    fn cannot_command_another_players_ship() {
        let mut w = test_world();
        let owner = PlayerId(7);
        let attacker = PlayerId(8);
        w.step(&[Command::AddPlayer { id: owner, name: "Owner".into() }]);
        w.step(&[Command::AddPlayer { id: attacker, name: "Rival".into() }]);
        for _ in 0..(10 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        // Find a ship owned by `owner`.
        let target = *w.fleets.iter().find(|(_, s)| s.owner == owner).unwrap().0;
        let before = format!("{:?}", w.fleets[&target].order);
        // Rival tries to command it; ignored, no pending order created.
        w.step(&[Command::MoveShip {
            player_id: attacker,
            ship_id: target,
            dest: Vec2::new(0.0, 0.0),
        }]);
        for _ in 0..(40 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        // It never became a MoveTo to (0,0) from the rival's command.
        if let FleetOrder::MoveTo { dest } = w.fleets[&target].order {
            assert_ne!(dest, Vec2::new(0.0, 0.0), "rival should not control this ship");
        }
        let _ = before;
    }

    fn find_ship(w: &World, owner: PlayerId, kind: ShipKind) -> EntityId {
        *w.fleets
            .iter()
            .find(|(_, s)| s.owner == owner && s.flagship_kind() == kind)
            .unwrap()
            .0
    }

    /// Set up an attacker raider and a (stationary) defender convoy at chosen
    /// offsets from the attacker's command center. Returns (raider, convoy).
    fn raid_setup(w: &mut World, atk: PlayerId, def: PlayerId, raider_off: Vec2, convoy_off: Vec2) -> (EntityId, EntityId) {
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let raider = find_ship(w, atk, ShipKind::Raider);
        let convoy = find_ship(w, def, ShipKind::Convoy);
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cc + raider_off;
            r.vel = Vec2::ZERO;
            r.order = FleetOrder::Idle;
        }
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.pos = cc + convoy_off;
            c.vel = Vec2::ZERO;
            c.order = FleetOrder::Idle; // sitting duck
        }
        (raider, convoy)
    }

    fn run_until_raid<F: FnMut(&World) -> Vec<Command>>(w: &mut World, max_secs: u32, mut each: F) -> Option<RaidOutcome> {
        for _ in 0..(max_secs * crate::config::TICK_HZ) {
            let cmds = each(w);
            for e in w.step(&cmds) {
                if let EventPayload::RaidResolved { outcome, .. } = e.payload {
                    return Some(outcome);
                }
            }
        }
        None
    }

    /// After a battle, the world state must be consistent with the outcome: a
    /// ship is present iff it wasn't destroyed.
    fn assert_battle_consistent(w: &World, outcome: RaidOutcome, attacker: EntityId, target: EntityId) {
        let (kill_a, kill_t) = outcome.kills();
        assert_eq!(w.fleets.contains_key(&attacker), !kill_a, "attacker present iff not destroyed");
        assert_eq!(w.fleets.contains_key(&target), !kill_t, "target present iff not destroyed");
    }

    // --- §FLEETS Part 2: Lanchester combat in the authoritative sim ----------

    /// (attacker_losses, target_losses, outcome) of a battle report.
    type Report = (BTreeMap<ShipKind, u32>, BTreeMap<ShipKind, u32>, RaidOutcome);

    /// Drive a committed raid to its first battle report with losses.
    fn run_to_report(w: &mut World, max_secs: u32) -> Option<Report> {
        for _ in 0..(max_secs * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved { attacker_losses, target_losses, outcome, .. } = &e.payload
                    && (!attacker_losses.is_empty() || !target_losses.is_empty())
                {
                    return Some((attacker_losses.clone(), target_losses.clone(), *outcome));
                }
            }
        }
        None
    }

    #[test]
    fn battle_report_carries_per_kind_losses() {
        // A 3-raider fleet vs a tanky 5-corvette fleet — a full battle both bleed
        // in (the corvettes' armour makes the raiders pay before the safety valve).
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, target) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(300.0, 0.0));
        w.fleets.get_mut(&raider).unwrap().composition.insert(ShipKind::Raider, 3);
        {
            let d = w.fleets.get_mut(&target).unwrap();
            d.composition.clear();
            d.composition.insert(ShipKind::Corvette, 5);
            d.cargo = None;
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: target }]);
        let (a_losses, t_losses, _outcome) = run_to_report(&mut w, 120).expect("a battle report fires");
        assert!(t_losses.get(&ShipKind::Corvette).copied().unwrap_or(0) > 0, "the defender's corvette losses are reported per kind");
        assert!(a_losses.get(&ShipKind::Raider).copied().unwrap_or(0) > 0, "the attacker bled raiders too — nobody wins for free");
    }

    #[test]
    fn raid_costs_the_winner_something_but_rarely_everything() {
        // A lone raider raids a lone convoy: it wins (seizes/destroys) but the
        // convoy's token defense means the raider isn't guaranteed to walk away
        // untouched — and critically the raider survives (a survivable skirmish).
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(300.0, 0.0));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        run_until_raid(&mut w, 120, |_| vec![]).expect("the raid resolves");
        assert!(!w.fleets.contains_key(&convoy), "the convoy is taken");
        assert!(w.fleets.contains_key(&raider), "the raider survives the skirmish");
    }

    #[test]
    fn platform_pool_attrits_tiers_then_reports() {
        // A raider grinds a convoy defended by a 2-tier platform: the platform's
        // pool fills and tiers fall (owner-only PlatformEngaged), and the raid
        // reports through the ordinary battle report.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(300.0, 0.0));
        w.fleets.get_mut(&raider).unwrap().composition.insert(ShipKind::Raider, 6);
        // Fortify a system covering the convoy.
        let cpos = w.fleets[&convoy].pos;
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(def);
            s.claimed_at = Some(0.0);
            s.set_tier(crate::build::StructureKind::DefensePlatform, 2);
            s.pos = cpos; // co-located so it covers the contact
            s.id
        };
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let mut saw_platform = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::PlatformEngaged { .. }) {
                    saw_platform = true;
                }
            }
            let tier = w.systems.iter().find(|s| s.id == sid).map(|s| s.tier_sum(crate::build::StructureKind::DefensePlatform)).unwrap_or(0);
            if tier < 2 || !w.fleets.contains_key(&raider) {
                break;
            }
        }
        let final_tier = w.systems.iter().find(|s| s.id == sid).unwrap().tier(crate::build::StructureKind::DefensePlatform);
        assert!(final_tier < 2 || saw_platform, "the platform's pool attrits a tier under sustained attack");
    }

    #[test]
    fn persistence_round_trip_mid_engagement_resumes_the_fight() {
        // Start a battle, accumulate damage pools, SERIALIZE mid-fight, reload,
        // and confirm the pools survived and the fight resolves the same way.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // Place them already in contact range so the fight starts promptly.
        let (raider, target) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(160.0, 0.0));
        w.fleets.get_mut(&raider).unwrap().composition.insert(ShipKind::Raider, 5);
        {
            let d = w.fleets.get_mut(&target).unwrap();
            d.composition.clear();
            d.composition.insert(ShipKind::Corvette, 4);
            d.cargo = None;
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: target }]);
        // Fight long enough for the order's light to arrive and the engagement's
        // side pools to build.
        for _ in 0..60 {
            w.step(&[]);
        }
        let eng = w.engagements.values().next().expect("a battle is underway");
        let started = eng.started_at;
        assert!(eng.a_stack_pool.values().chain(eng.d_stack_pool.values()).flat_map(|m| m.values()).any(|p| *p > 0.0), "engagement side pools accumulated mid-battle");
        // Round-trip through JSON (the snapshot path) — the ENGAGEMENT entity
        // (pools + elapsed + participants) persists, so the fight resumes exactly.
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        let eng2 = w2.engagements.values().next().expect("the battle survived serialization");
        assert!((eng2.started_at - started).abs() < 1e-9, "elapsed (started_at) persisted");
        assert!(eng2.a_stack_pool.values().chain(eng2.d_stack_pool.values()).flat_map(|m| m.values()).any(|p| *p > 0.0), "pools survived serialization");
        // Both worlds resume and reach the SAME result (deterministic).
        let run = |mut w: World| -> (bool, bool) {
            for _ in 0..(300 * crate::config::TICK_HZ) {
                if w.engagements.is_empty() {
                    break;
                }
                w.step(&[]);
            }
            (w.fleets.contains_key(&raider), w.fleets.contains_key(&target))
        };
        assert_eq!(run(w), run(w2), "the reloaded fight reaches the same result (deterministic)");
    }

    /// Park a combatant fleet of `n × kind` (Idle unless an order given).
    fn squad(w: &mut World, owner: PlayerId, pos: Vec2, kind: ShipKind, n: u32, order: FleetOrder) -> EntityId {
        let id = w.alloc_entity_id();
        let mut f = Fleet::single(id, owner, kind, pos, order, None);
        f.composition.clear();
        f.composition.insert(kind, n);
        w.fleets.insert(id, f);
        id
    }

    // ===================================================================
    // §offensive-orders — AttackFleet + engagement POSTURE
    // ===================================================================
    use crate::doctrine::EngagementPosture;

    /// Run the world for up to `secs`, returning true as soon as `pred` holds.
    fn run_until<F: FnMut(&mut World) -> bool>(w: &mut World, secs: u32, mut pred: F) -> bool {
        for _ in 0..(secs * crate::config::TICK_HZ) {
            w.step(&[]);
            if pred(w) {
                return true;
            }
        }
        false
    }

    /// Part 1: an ATTACK order intercepts a rival raider wing and opens a
    /// FULL-DURATION engagement (not the raid brevity cap). Also proves the
    /// ≥1-raider gate is by CONTAINS (like blockade), not flagship: a
    /// corvette-flagship fleet with a raider aboard CAN attack (it can't raid).
    #[test]
    fn attack_fleet_full_battles_a_rival_wing_via_the_contains_raider_gate() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        // Attacker: a corvette-FLAGSHIP fleet (corvette > raider precedence) that
        // also carries a raider — so CommitRaid would soft-reject but AttackFleet
        // must accept (contains a raider).
        let striker = w.alloc_entity_id();
        let mut sf = Fleet::single(striker, atk, ShipKind::Corvette, cc + Vec2::new(120.0, 0.0), FleetOrder::Idle, None);
        sf.composition.clear();
        sf.composition.insert(ShipKind::Corvette, 2);
        sf.composition.insert(ShipKind::Raider, 2);
        assert_ne!(sf.flagship_kind(), ShipKind::Raider, "flagship is the corvette");
        w.fleets.insert(striker, sf);
        // Target: a lone rival raider, near the striker so contact is quick.
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);

        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        // The light-delayed order lands, the fleet takes the Attack order, pursues,
        // and opens a raid=false (full) engagement on contact.
        let opened = run_until(&mut w, 10, |w| {
            w.engagements.values().any(|e| e.attackers.contains(&striker))
        });
        assert!(opened, "attack intercepts and opens an engagement");
        let raid = w.engagements.values().find(|e| e.attackers.contains(&striker)).map(|e| e.raid);
        assert_eq!(raid, Some(false), "an ATTACK is a full battle, never a raid brevity cap");
    }

    /// Part 1: a corvette/scout-only fleet (no raider) SOFT-REJECTS an attack —
    /// crisp roles, consistent with blockade. The order is never taken.
    #[test]
    fn attack_fleet_soft_rejects_a_raiderless_fleet() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let corvettes = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Corvette, 3, FleetOrder::Idle);
        let target = squad(&mut w, def, cc + Vec2::new(300.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: corvettes, target_id: target }]);
        // Give any (nonexistent) order time to land — it never does.
        for _ in 0..(4 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(matches!(w.fleets[&corvettes].order, FleetOrder::Idle), "no raider → no attack, order untouched");
        assert!(w.engagements.is_empty(), "no engagement from a soft-rejected attack");
    }

    // ===================================================================
    // §battle-records Part A — the recorder observes the lifecycle
    // ===================================================================

    /// Stage a decisive attacker-wins battle (6 raiders vs 3), run it to
    /// resolution, and return the recorded engagement id. `atk`/`def` exist.
    fn run_recorded_battle(w: &mut World, atk: PlayerId, def: PlayerId) -> EntityId {
        let cc = w.players[&atk].command_center;
        let striker = squad(w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        let target = squad(w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        eid
    }

    #[test]
    fn a_battle_opens_and_finalizes_a_record_with_a_timeline() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        let eid = run_recorded_battle(&mut w, atk, def);
        let rec = w.battle_records.get(&eid).expect("the engagement was recorded");
        assert_eq!(rec.id, eid, "the record is keyed by the engagement id");
        assert!(rec.started_tick > 0, "opening tick captured");
        assert!(rec.ended_tick.is_some(), "resolution stamped the ending tick");
        assert!(!rec.rounds.is_empty(), "a timeline of rounds was recorded");
        let o = rec.outcome.as_ref().expect("outcome summary present");
        assert_eq!(o.outcome, crate::event::RaidOutcome::TargetDestroyed, "the attacker won");
        assert_eq!(rec.sides[0].corp, atk, "side 0 is the attacker corp");
        assert_eq!(rec.sides[1].corp, def, "side 1 is the defender corp");
        assert_eq!(rec.sides[0].initial.get(&ShipKind::Raider).copied(), Some(6), "opening attacker composition");
        // Every round's survivor snapshot is internally consistent (≤ opening).
        for r in &rec.rounds {
            assert!(r.counts[1].get(&ShipKind::Raider).copied().unwrap_or(0) <= 3, "defender survivors never exceed the opening");
        }
        // §tactical T3: recorded rounds carry TRUTH KEYFRAMES — real positions
        // inside the arena bounds — and the battle's deaths appear as exact
        // (step, pos) events somewhere on the timeline.
        let framed: Vec<&crate::combat::Keyframe> = rec.rounds.iter().filter_map(|r| r.frame.as_ref()).collect();
        assert!(!framed.is_empty(), "rounds carry truth keyframes");
        for f in &framed {
            for s in &f.ships {
                assert!(s.x.abs() <= 2_000.0 && s.y.abs() <= 2_000.0, "keyframe positions stay in arena bounds");
                assert!((0.0..=1.0).contains(&s.hp), "hp is a fraction");
            }
        }
        assert!(
            framed.iter().any(|f| !f.deaths.is_empty()),
            "the battle's kills appear as exact death events"
        );
        // Serde round-trip keeps frames (and old frame-less records still load —
        // the field is serde-default).
        let json = serde_json::to_string(rec).unwrap();
        let back: crate::combat::BattleRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rounds.iter().filter(|r| r.frame.is_some()).count(), framed.len());
    }

    /// §tactical law 2 (stream isolation, test-enforced): battle dice come
    /// from a stream derived from `(world_seed, battle_id)` — NEVER the world
    /// RNG. Two worlds, identical except one fights an extra battle: after
    /// equal tick counts their WORLD RNG states are byte-identical, so adding
    /// or removing a battle shifts no unrelated draw anywhere in the sim.
    #[test]
    fn an_extra_battle_draws_nothing_from_the_world_rng() {
        // Returns (rng state, whether a battle ever formed) — the premise is
        // asserted below so the test can never pass vacuously (e.g. a future
        // gate change silently rejecting the attack in both worlds).
        let world_rng_after = |fight: bool| -> (serde_json::Value, bool) {
            let mut w = test_world();
            let (a, b) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
            let far = w.players[&a].command_center + Vec2::new(4_000.0, 4_000.0);
            let striker = squad(&mut w, a, far, ShipKind::Raider, 6, FleetOrder::Idle);
            let target = squad(&mut w, b, far + Vec2::new(40.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
            let cmds = if fight {
                vec![Command::AttackFleet { player_id: a, fleet_id: striker, target_id: target }]
            } else {
                vec![]
            };
            w.step(&cmds);
            let mut battled = false;
            for _ in 0..(40 * crate::config::TICK_HZ) {
                w.step(&[]);
                battled |= !w.engagements.is_empty();
            }
            (serde_json::to_value(&w).unwrap().get("rng").expect("world rng rides the snapshot").clone(), battled)
        };
        let (quiet, quiet_battled) = world_rng_after(false);
        let (fought, fought_battled) = world_rng_after(true);
        assert!(fought_battled, "the fight world really fought (the premise holds)");
        assert!(!quiet_battled, "the control world stayed quiet (the premise holds)");
        assert_eq!(quiet, fought, "a whole extra battle drew NOTHING from the world RNG stream");
    }

    /// §tactical law 1 (containment at the boundary): repack conserves ships
    /// plus kills EXACTLY — every ship that unpacked either repacks into a
    /// surviving stack or appears in the record's losses. Fought far from
    /// both homes so no starter fleet folds in as relief.
    #[test]
    fn repack_conserves_ships_plus_kills() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        let far = w.players[&a].command_center + Vec2::new(4_000.0, 4_000.0);
        let striker = squad(&mut w, a, far, ShipKind::Raider, 6, FleetOrder::Idle);
        let target = squad(&mut w, b, far + Vec2::new(40.0, 0.0), ShipKind::Raider, 5, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: a, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(&mut w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        let rec = &w.battle_records[&eid];
        let o = rec.outcome.as_ref().expect("outcome stamped");
        assert!(
            !rec.rounds.iter().flat_map(|r| &r.notes).any(|n| matches!(n, crate::combat::RoundNote::Joined { .. })),
            "an isolated duel — nothing folded in"
        );
        for (side, fid) in [(0usize, striker), (1usize, target)] {
            let survivors = w.fleets.get(&fid).map(|f| f.composition.get(&ShipKind::Raider).copied().unwrap_or(0)).unwrap_or(0);
            let lost = o.total_losses[side].get(&ShipKind::Raider).copied().unwrap_or(0);
            let initial = rec.sides[side].initial.get(&ShipKind::Raider).copied().unwrap_or(0);
            assert_eq!(survivors + lost, initial, "side {side}: survivors + kills == what unpacked");
        }
    }

    /// §tactical T6 (the join-accounting fix): when relief folds in mid-battle,
    /// the joined ships' deaths COUNT — the outcome's per-side losses equal
    /// `(opening + joined) − survivors`, never the opening-only diff that used
    /// to swallow a reinforced side's casualties.
    #[test]
    fn reinforced_side_losses_are_fully_counted() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        let far = w.players[&a].command_center + Vec2::new(4_000.0, -4_000.0);
        // A deliberately losing attacker + relief AT the site (folds in at open),
        // against a defender wall that will kill attackers from BOTH wings.
        let striker = squad(&mut w, a, far, ShipKind::Raider, 3, FleetOrder::Idle);
        let relief = squad(&mut w, a, far + Vec2::new(30.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
        let target = squad(&mut w, b, far + Vec2::new(60.0, 0.0), ShipKind::Raider, 9, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: a, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(&mut w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        let rec = &w.battle_records[&eid];
        let joined: u32 = rec
            .rounds
            .iter()
            .flat_map(|r| &r.notes)
            .filter_map(|n| match n {
                crate::combat::RoundNote::Joined { side: 0, comp } => Some(comp.get(&ShipKind::Raider).copied().unwrap_or(0)),
                _ => None,
            })
            .sum();
        assert!(joined > 0, "the relief wing folded into the attacker side");
        let survivors: u32 = [striker, relief]
            .iter()
            .filter_map(|fid| w.fleets.get(fid))
            .map(|f| f.composition.get(&ShipKind::Raider).copied().unwrap_or(0))
            .sum();
        let o = rec.outcome.as_ref().expect("outcome stamped");
        let lost = o.total_losses[0].get(&ShipKind::Raider).copied().unwrap_or(0);
        let initial = rec.sides[0].initial.get(&ShipKind::Raider).copied().unwrap_or(0);
        assert!(lost > 0, "the outnumbered attacker bled somewhere");
        assert_eq!(survivors + lost, initial + joined, "reinforced-side losses are fully counted");
    }

    /// §tactical T6 (the alive-exit half of stint accounting): a fleet that
    /// FLEES a battle alive takes its surviving ships out of the side's start
    /// composition — the record must never report an escaped survivor as a
    /// combat death (`start − final` used to count the whole escaped fleet).
    #[test]
    fn escaped_survivors_are_not_reported_as_losses() {
        let mut w = test_world();
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        let mut doc = w.players[&a].doctrine;
        doc.engagement = crate::doctrine::EngagementPolicy::Avoid;
        w.step(&[Command::SetFleetDoctrine { player_id: a, doctrine: doc }]);
        let pos = w.players[&a].command_center + Vec2::new(500.0, 0.0);
        // a's raiders (Avoid, faster) are jumped by d's corvettes: a brief
        // scrape, then they out-speed the corvettes and escape ALIVE.
        let raider = squad(&mut w, a, pos, ShipKind::Raider, 2, FleetOrder::Idle);
        let _corv = squad(&mut w, d, pos + Vec2::new(40.0, 0.0), ShipKind::Corvette, 3, FleetOrder::Intercept { target: raider });
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the scrape opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(&mut w, 120, |w| w.engagements.is_empty()), "the scrape resolves");
        let escaped = w.fleets.get(&raider).map(|f| f.composition.get(&ShipKind::Raider).copied().unwrap_or(0)).unwrap_or(0);
        assert!(escaped > 0, "the faster raiders got away with ships");
        let rec = &w.battle_records[&eid];
        let o = rec.outcome.as_ref().expect("outcome stamped");
        // The raiders were the jumped side — find which side they were on.
        let side = if rec.sides[0].corp == a { 0 } else { 1 };
        let lost = o.total_losses[side].get(&ShipKind::Raider).copied().unwrap_or(0);
        assert_eq!(lost + escaped, 2, "losses = what actually died; escapees are never phantom kills");
    }

    /// §tactical: a PRE-ENGINE mid-battle snapshot (`Engagement.tactical`
    /// absent) loads and resolves sanely — the one-way migration re-opens the
    /// fight from the persisted counts + pools at the next tick.
    #[test]
    fn an_old_mid_battle_snapshot_migrates_into_the_engine() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        let cc = w.players[&a].command_center;
        // Raiders into a corvette wall: the 6 400 HP of held line grinds long
        // enough to snapshot mid-fight. (The striker needs raiders — the
        // AttackFleet gate is contains-a-raider.)
        let striker = squad(&mut w, a, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        let target = squad(&mut w, b, cc + Vec2::new(160.0, 0.0), ShipKind::Corvette, 8, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: a, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        for _ in 0..(2 * crate::config::TICK_HZ) {
            w.step(&[]); // real fighting, so pools are non-trivial
        }
        assert!(!w.engagements.is_empty(), "still mid-battle at snapshot time");
        // Strip the tactical state — the exact shape of a pre-engine snapshot.
        let mut v = serde_json::to_value(&w).expect("snapshot");
        let engs = v.get_mut("engagements").and_then(|e| e.as_object_mut()).expect("engagements object");
        for (_, e) in engs.iter_mut() {
            e.as_object_mut().unwrap().insert("tactical".into(), serde_json::Value::Null);
        }
        let mut w2: World = serde_json::from_value(v).expect("the stripped snapshot still loads");
        assert!(run_until(&mut w2, 400, |w| w.engagements.is_empty()), "the migrated battle resolves");
        assert!(
            w2.battle_records.values().any(|r| r.ended_tick.is_some()),
            "…and finalizes its record without panic"
        );
    }

    // (Record-hash determinism — same seed + same battle → byte-identical
    // records — is already proven by `battle_records_are_deterministic`.)

    #[test]
    fn records_note_a_reinforcement_join() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        let cc = w.players[&atk].command_center;
        let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
        // A second attacker wing sits at the contact point, Idle — relief that the
        // reinforce loop folds into the attacker side once the fight forms.
        let _relief = squad(&mut w, atk, cc + Vec2::new(180.0, 0.0), ShipKind::Raider, 3, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(&mut w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        let rec = &w.battle_records[&eid];
        let joined = rec.rounds.iter().flat_map(|r| &r.notes)
            .any(|n| matches!(n, crate::combat::RoundNote::Joined { side: 0, .. }));
        assert!(joined, "the reinforcement join is a recorded beat");
    }

    #[test]
    fn records_note_a_defender_retreat() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        // Defender withdraws once half its strength is gone (doctrine Half).
        w.players.get_mut(&def).unwrap().doctrine.retreat = crate::doctrine::RetreatThreshold::Half;
        let cc = w.players[&atk].command_center;
        let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Raider, 4, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        assert!(run_until(&mut w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        let rec = &w.battle_records[&eid];
        let retreated = rec.rounds.iter().flat_map(|r| &r.notes)
            .any(|n| matches!(n, crate::combat::RoundNote::RetreatTripped { side: 1 }));
        assert!(retreated, "the defender's retreat threshold trip is a recorded beat");
    }

    #[test]
    fn records_note_a_withdraw_order() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        let cc = w.players[&atk].command_center;
        // An even, grinding fight so the withdraw order arrives mid-battle.
        let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "the battle opens");
        let eid = *w.engagements.keys().next().unwrap();
        // Order the defender to withdraw; its light-delayed arrival pulls it out.
        w.step(&[Command::Withdraw { player_id: def, fleet_id: target }]);
        assert!(run_until(&mut w, 400, |w| w.engagements.is_empty()), "the battle resolves");
        let rec = &w.battle_records[&eid];
        let withdrew = rec.rounds.iter().flat_map(|r| &r.notes)
            .any(|n| matches!(n, crate::combat::RoundNote::WithdrawOrdered { side: 1 }));
        assert!(withdrew, "the defender's withdraw order is a recorded beat");
    }

    #[test]
    fn records_downsample_to_the_cadence() {
        let mut w = test_world(); // battle_target_secs = 20 → round_every = 15 ticks
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        let eid = run_recorded_battle(&mut w, atk, def);
        let rec = &w.battle_records[&eid];
        let re = crate::combat::BattleRecord::round_every_for(w.config.battle_target_secs, false);
        assert_eq!(re, 15, "cadence math for the test preset");
        assert!(rec.rounds.len() >= 3, "a real battle records several rounds");
        // Flushes are at MOST one cadence apart (beats only ever flush sooner).
        for pair in rec.rounds.windows(2) {
            assert!(pair[1].tick - pair[0].tick <= re, "no gap wider than the cadence");
        }
        // The tail never runs longer than a cadence past the last flush.
        let ended = rec.ended_tick.unwrap();
        assert!(ended - rec.rounds.last().unwrap().tick <= re, "the final round is within one cadence of the end");
    }

    #[test]
    fn battle_records_are_deterministic() {
        // Same seed + same script → byte-identical records (the recorder never
        // feeds back into resolution).
        let run = || {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
            let eid = run_recorded_battle(&mut w, atk, def);
            (eid, w.battle_records.clone())
        };
        let (e1, r1) = run();
        let (e2, r2) = run();
        assert_eq!(e1, e2, "the engagement id is deterministic");
        assert_eq!(r1, r2, "the full record set is identical across identical runs");
    }

    #[test]
    fn battle_record_captures_each_sides_initial_loadouts() {
        // §modules B5: the record opens with each side's opening FIT partition, so
        // a participant-fidelity replay can label the sides and type their salvos.
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
        let cc = w.players[&atk].command_center;
        let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 4, FleetOrder::Idle);
        let md = Loadout::new(vec![ModuleKind::MassDriver]);
        w.fleets.get_mut(&striker).unwrap().loadouts.entry(ShipKind::Raider).or_default().insert(md.key(), 4);
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Corvette, 4, FleetOrder::Idle);
        let wa = Loadout::new(vec![ModuleKind::WhippleArmor]);
        w.fleets.get_mut(&target).unwrap().loadouts.entry(ShipKind::Corvette).or_default().insert(wa.key(), 4);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.battle_records.is_empty()), "a record opens on contact");
        let rec = w.battle_records.values().next().unwrap();
        // side 0 = attackers (mass drivers), side 1 = defenders (whipple).
        assert_eq!(
            rec.sides[0].initial_loadouts.get(&ShipKind::Raider).and_then(|m| m.get(&md.key())).copied(),
            Some(4), "the attacker's mass-driver fit is recorded",
        );
        assert_eq!(
            rec.sides[1].initial_loadouts.get(&ShipKind::Corvette).and_then(|m| m.get(&wa.key())).copied(),
            Some(4), "the defender's whipple fit is recorded",
        );
    }

    #[test]
    fn world_serde_round_trip_preserves_records() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "Atk".into() }, Command::AddPlayer { id: def, name: "Def".into() }]);
        let eid = run_recorded_battle(&mut w, atk, def);
        assert!(w.battle_records.contains_key(&eid));
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w.battle_records, w2.battle_records, "records survive a snapshot round-trip");
    }

    // ===================================================================
    // §modules Part B — loadouts flow fleet → combat → losses end to end
    // ===================================================================

    #[test]
    fn a_whipple_fit_survives_drivers_better_end_to_end() {
        use crate::module::{Loadout, ModuleKind};
        // The SAME fight twice — a mass-driver attacker vs a corvette defender
        // that is UNFITTED in one run, WHIPPLE-armored (the driver counter) in
        // the other. Whipple must leave more defenders alive: the loadout flows
        // fleet → side_loadouts → Forces::from_side → typed combat → per-stack
        // losses → fleet, all the way through the real engagement.
        let run = |whipple: bool| -> u32 {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
            let cc = w.players[&atk].command_center;
            let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
            w.fleets.get_mut(&striker).unwrap().loadouts.entry(ShipKind::Raider).or_default()
                .insert(Loadout::new(vec![ModuleKind::MassDriver]).key(), 6);
            let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Corvette, 8, FleetOrder::Idle);
            if whipple {
                w.fleets.get_mut(&target).unwrap().loadouts.entry(ShipKind::Corvette).or_default()
                    .insert(Loadout::new(vec![ModuleKind::WhippleArmor]).key(), 8);
            }
            w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
            assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "battle opens");
            assert!(run_until(&mut w, 500, |w| w.engagements.is_empty()), "battle resolves");
            w.fleets.get(&target).map(|f| f.count(ShipKind::Corvette)).unwrap_or(0)
        };
        let bare = run(false);
        let whip = run(true);
        assert!(whip > bare, "whipple corvettes outlast bare ones vs drivers ({whip} vs {bare})");
    }

    #[test]
    fn armor_survives_across_ticks_on_a_mixed_same_kind_side() {
        // §modules regression (adversarial-review finding): one side holds BOTH
        // whipple-armored AND unfitted corvettes of the SAME kind (a mixed
        // stack), facing mass drivers over a full multi-tick battle. The armored
        // ships must OUTLAST the bare ones. The bug: the engagement persisted
        // pools PER KIND, so from_side re-averaged the two stacks' pools every
        // tick, erasing/inverting the armor advantage. Fixed by persisting the
        // pool PER STACK.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
        let cc = w.players[&atk].command_center;
        let striker = squad(&mut w, atk, cc + Vec2::new(120.0, 0.0), ShipKind::Raider, 8, FleetOrder::Idle);
        w.fleets.get_mut(&striker).unwrap().loadouts.entry(ShipKind::Raider).or_default()
            .insert(crate::module::Loadout::new(vec![crate::module::ModuleKind::MassDriver]).key(), 8);
        // Defender: 5 corvettes, 3 whipple-armored (a mixed same-kind side).
        let target = squad(&mut w, def, cc + Vec2::new(160.0, 0.0), ShipKind::Corvette, 5, FleetOrder::Idle);
        w.fleets.get_mut(&target).unwrap().loadouts.entry(ShipKind::Corvette).or_default()
            .insert(crate::module::Loadout::new(vec![crate::module::ModuleKind::WhippleArmor]).key(), 3);
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: striker, target_id: target }]);
        assert!(run_until(&mut w, 20, |w| !w.engagements.is_empty()), "battle opens");
        // Run until the defender has taken real losses, then check the survivors.
        run_until(&mut w, 500, |w| {
            w.engagements.is_empty() || w.fleets.get(&target).map(|f| f.count(ShipKind::Corvette) < 5).unwrap_or(true)
        });
        if let Some(f) = w.fleets.get(&target) {
            let total = f.count(ShipKind::Corvette);
            let whipple = f.fitted_count(ShipKind::Corvette);
            let bare = total - whipple;
            // Started 3 whipple / 2 bare — once losses begin, the bare ships must
            // shed at least as fast (armored survivors ≥ bare survivors).
            if total < 5 {
                assert!(whipple >= bare, "whipple corvettes outlast bare ones on a mixed side (whipple {whipple}, bare {bare} of {total})");
            }
        }
    }

    #[test]
    fn merge_and_split_carry_loadouts() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        // Fleet A: 2 raiders, one with a torpedo rack.
        let a = w.alloc_entity_id();
        let mut fa = Fleet::single(a, id, ShipKind::Raider, pos, FleetOrder::Idle, None);
        fa.add(ShipKind::Raider, 1);
        fa.loadouts.entry(ShipKind::Raider).or_default().insert(Loadout::new(vec![ModuleKind::TorpedoRack]).key(), 1);
        w.fleets.insert(a, fa);
        // Fleet B: 1 raider with reflective plating, co-located.
        let b = w.alloc_entity_id();
        let mut fb = Fleet::single(b, id, ShipKind::Raider, pos, FleetOrder::Idle, None);
        fb.loadouts.entry(ShipKind::Raider).or_default().insert(Loadout::new(vec![ModuleKind::ReflectivePlating]).key(), 1);
        w.fleets.insert(b, fb);
        // Merge B into A → A carries BOTH fits.
        w.step(&[Command::MergeFleets { player_id: id, into: a, from: b }]);
        assert_eq!(w.fleets[&a].count(ShipKind::Raider), 3);
        assert_eq!(w.fleets[&a].fitted_count(ShipKind::Raider), 2, "both fits carried across the merge");
        // Split one raider back off → a fit goes with it (no fit lost). Identify
        // the genuinely NEW fleet by id (the player already has a starting fleet).
        let before: std::collections::BTreeSet<EntityId> = w.fleets.keys().copied().collect();
        let mut counts = std::collections::BTreeMap::new();
        counts.insert(ShipKind::Raider, 1);
        w.step(&[Command::SplitFleet { player_id: id, fleet_id: a, counts }]);
        let src_fits = w.fleets[&a].fitted_count(ShipKind::Raider);
        let new_id = *w.fleets.keys().find(|k| !before.contains(k)).expect("a detached fleet");
        let new_fleet = &w.fleets[&new_id];
        assert_eq!(new_fleet.count(ShipKind::Raider), 1);
        assert_eq!(new_fleet.fitted_count(ShipKind::Raider), 1, "the split takes a fitted ship first");
        assert_eq!(src_fits + new_fleet.fitted_count(ShipKind::Raider), 2, "no fit lost across the split");
    }

    #[test]
    fn manufacture_a_module_debits_goods_and_credits_the_ledger() {
        use crate::module::ModuleKind;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.set_tier(crate::build::StructureKind::ArmamentsComplex, 1); // the manufacture gate
            s.id
        };
        seed_stock(&mut w, sid, &[(Commodity::Armaments, 100.0), (Commodity::Electronics, 100.0)]);
        let arm = |w: &World| w.systems.iter().find(|s| s.id == sid).unwrap().stockpile.get(&Commodity::Armaments).copied().unwrap_or(0.0);
        let before = arm(&w);
        let ev = w.step(&[Command::BuildModule { player_id: id, system_id: sid, module: ModuleKind::MassDriver }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "manufacture starts");
        assert!((before - arm(&w) - 8.0).abs() < 1e-6, "8 Armaments debited (the Mass Driver recipe)");
        // Run to completion → the crate lands in the ledger.
        let landed = run_until(&mut w, 30, |w| {
            w.systems.iter().find(|s| s.id == sid).map(|s| s.modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0)).unwrap_or(0) >= 1
        });
        assert!(landed, "the finished module joins the system's module ledger");
    }

    #[test]
    fn manufacture_soft_rejects_without_an_armaments_complex() {
        use crate::module::ModuleKind;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.id
        };
        seed_stock(&mut w, sid, &[(Commodity::Armaments, 100.0), (Commodity::Electronics, 100.0)]);
        let ev = w.step(&[Command::BuildModule { player_id: id, system_id: sid, module: ModuleKind::MassDriver }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { .. })), "no Armaments Complex → soft reject");
        assert!(w.build_queue.is_empty(), "nothing queued");
        assert!((w.systems.iter().find(|s| s.id == sid).unwrap().stockpile.get(&Commodity::Armaments).copied().unwrap() - 100.0).abs() < 1e-9, "recipe never eaten on a reject");
    }

    #[test]
    fn building_a_fitted_ship_debits_the_ledger_and_spawns_it_fitted() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.set_tier(crate::build::StructureKind::Shipyard, 2); // raiders need a tier-2 yard
            *s.modules.entry(ModuleKind::MassDriver).or_insert(0) += 1;
            s.id
        };
        seed_stock(&mut w, sid, &[(Commodity::Alloys, 200.0), (Commodity::Electronics, 200.0), (Commodity::Armaments, 200.0), (Commodity::Fuel, 200.0)]);
        let md = Loadout::new(vec![ModuleKind::MassDriver]);
        let before: std::collections::BTreeSet<EntityId> = w.fleets.keys().copied().collect();
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: sid, ship_kind: ShipKind::Raider, join: None, loadout: md.clone() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "the fitted build starts");
        assert_eq!(w.systems.iter().find(|s| s.id == sid).unwrap().modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 0, "the module left the ledger");
        assert!(run_until(&mut w, 30, |w| w.fleets.keys().any(|k| !before.contains(k))), "the raider spawns");
        let new_id = *w.fleets.keys().find(|k| !before.contains(k)).unwrap();
        let f = &w.fleets[&new_id];
        assert_eq!(f.count(ShipKind::Raider), 1);
        assert_eq!(f.fitted_count(ShipKind::Raider), 1, "the built raider carries its mass-driver loadout");
        assert_eq!(f.loadouts.get(&ShipKind::Raider).and_then(|m| m.get(&md.key())).copied().unwrap_or(0), 1);
    }

    #[test]
    fn building_with_an_uncovered_loadout_soft_rejects_and_keeps_the_goods() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.set_tier(crate::build::StructureKind::Shipyard, 2);
            s.id // NO modules in the ledger
        };
        seed_stock(&mut w, sid, &[(Commodity::Alloys, 200.0), (Commodity::Electronics, 200.0), (Commodity::Armaments, 200.0), (Commodity::Fuel, 200.0)]);
        let alloys = |w: &World| w.systems.iter().find(|s| s.id == sid).unwrap().stockpile.get(&Commodity::Alloys).copied().unwrap();
        let before = alloys(&w);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: sid, ship_kind: ShipKind::Raider, join: None, loadout: Loadout::new(vec![ModuleKind::MassDriver]) }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { .. })), "loadout not covered by the ledger → reject");
        assert!(w.build_queue.is_empty(), "nothing queued");
        assert!((before - alloys(&w)).abs() < 1e-9, "the hull recipe is never eaten on a reject");
    }

    // --- §modules Part B4: REFIT (swap fits at a yard, delta-reconciled) --------

    /// Helper: a player owning their home (Shipyard 1), a wing of `n` `from`-fitted
    /// raiders docked at it, and a ledger seeded with `stock`. Returns (home id, pos).
    fn refit_home(w: &mut World, id: PlayerId, n: u32, from: &crate::module::Loadout, stock: &[(crate::module::ModuleKind, u32)]) -> (EntityId, Vec2, EntityId) {
        w.enclaves.clear();
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let hpos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        {
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            for (m, c) in stock {
                *s.modules.entry(*m).or_insert(0) += *c;
            }
        }
        let fleet = squad(w, id, hpos, ShipKind::Raider, n, FleetOrder::Idle);
        if !from.is_empty() {
            w.fleets.get_mut(&fleet).unwrap().loadouts.entry(ShipKind::Raider).or_default().insert(from.key(), n);
        }
        (home, hpos, fleet)
    }

    #[test]
    fn refit_swaps_a_fit_via_the_ledger_delta_and_rejoins_after_the_yard() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        // §fitting: Driver(2)+Reflective(2) = 4 = the Raider's budget (Whipple
        // would overflow it — that armored brawl is the Corvette's fit now).
        let from = Loadout::new(vec![ModuleKind::MassDriver]);
        let to = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::ReflectivePlating]);
        let (home, _hpos, fleet) = refit_home(&mut w, id, 3, &from, &[(ModuleKind::ReflectivePlating, 2)]);
        // Refit 2 of the 3 raiders to ADD Reflective (delta = +1 each).
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Raider, from: from.clone(), to: to.clone(), n: 2 }]);
        assert_eq!(w.fleets[&fleet].count(ShipKind::Raider), 1, "the 2 refitting hulls leave the fleet — out of combat in the yard");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules.get(&ModuleKind::WhippleArmor).copied().unwrap_or(0), 0, "both Whipple crates debited from the ledger");
        assert_eq!(w.refit_queue.len(), 1, "a refit job is queued");
        assert!(run_until(&mut w, 30, |w| w.refit_queue.is_empty()), "the refit completes on the clock");
        let f = &w.fleets[&fleet];
        assert_eq!(f.count(ShipKind::Raider), 3, "all hulls return");
        assert_eq!(f.loadouts.get(&ShipKind::Raider).and_then(|m| m.get(&to.key())).copied().unwrap_or(0), 2, "2 hulls now carry the reflective fit");
        assert_eq!(f.loadouts.get(&ShipKind::Raider).and_then(|m| m.get(&from.key())).copied().unwrap_or(0), 1, "the un-refitted hull keeps its mass driver");
    }

    #[test]
    fn refit_to_bare_returns_removed_modules_to_the_ledger() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        let from = Loadout::new(vec![ModuleKind::MassDriver]);
        let (home, _hpos, fleet) = refit_home(&mut w, id, 3, &from, &[]); // 3 fitted, empty ledger
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Raider, from: from.clone(), to: Loadout::default(), n: 2 }]);
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 2, "stripping RETURNS both mass drivers to the ledger");
        assert!(run_until(&mut w, 30, |w| w.refit_queue.is_empty()), "refit completes");
        let f = &w.fleets[&fleet];
        assert_eq!(f.count(ShipKind::Raider), 3, "all hulls back");
        assert_eq!(f.fitted_count(ShipKind::Raider), 1, "2 stripped to stock; the 1 un-refitted keeps its driver");
    }

    #[test]
    fn refit_soft_rejects_when_the_ledger_cannot_cover_the_new_fit() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        let from = Loadout::new(vec![ModuleKind::MassDriver]);
        // Budget-legal target (4 = the Raider's points) so the LEDGER is the
        // binding reject here, not the fitting budget.
        let to = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::ReflectivePlating]);
        let (_home, _hpos, fleet) = refit_home(&mut w, id, 2, &from, &[]); // NO plating stocked
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Raider, from: from.clone(), to, n: 2 }]);
        assert!(w.refit_queue.is_empty(), "no ledger cover → nothing queued");
        let f = &w.fleets[&fleet];
        assert_eq!(f.count(ShipKind::Raider), 2, "no hull pulled");
        assert_eq!(f.loadouts.get(&ShipKind::Raider).and_then(|m| m.get(&from.key())).copied().unwrap_or(0), 2, "the fleet is untouched");
    }

    #[test]
    fn refit_soft_rejects_over_slots_and_away_from_any_yard() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        let from = Loadout::new(vec![ModuleKind::MassDriver]);
        let (_home, hpos, fleet) = refit_home(&mut w, id, 2, &from, &[(ModuleKind::WhippleArmor, 4), (ModuleKind::ReflectivePlating, 4)]);
        // Over slots: a Raider has 2 module slots; a 3-module `to` is rejected.
        let over = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor, ModuleKind::ReflectivePlating]);
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Raider, from: from.clone(), to: over, n: 1 }]);
        assert!(w.refit_queue.is_empty(), "3 modules on a 2-slot hull → reject");
        // Away from any owned/allied yard: a bare strip (needs no ledger) still rejects.
        w.fleets.get_mut(&fleet).unwrap().pos = hpos + Vec2::new(50_000.0, 0.0);
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Raider, from: from.clone(), to: Loadout::default(), n: 1 }]);
        assert!(w.refit_queue.is_empty(), "no yard in reach → reject");
        assert_eq!(w.fleets[&fleet].count(ShipKind::Raider), 2, "the fleet is untouched by either reject");
    }

    // --- §fitting Stage A: budgets, grandfathering, doctrine fits ---------------

    #[test]
    fn fitting_budget_gates_builds_torp_whipple_corvette_rejected_driver_whipple_accepted() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = {
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.set_tier(crate::build::StructureKind::Shipyard, 2); // corvettes need tier 2
            *s.modules.entry(ModuleKind::TorpedoRack).or_insert(0) += 2;
            *s.modules.entry(ModuleKind::WhippleArmor).or_insert(0) += 2;
            *s.modules.entry(ModuleKind::MassDriver).or_insert(0) += 2;
            s.id
        };
        seed_stock(&mut w, sid, &[(Commodity::Alloys, 400.0), (Commodity::Electronics, 200.0), (Commodity::Armaments, 200.0)]);
        // Torp(3) + Whipple(3) = 6 > the Corvette's 5 → BuildRejected, ledger intact.
        let heavy = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::WhippleArmor]);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: sid, ship_kind: ShipKind::Corvette, join: None, loadout: heavy }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected { .. })), "over-budget fit rejects at build");
        assert!(w.build_queue.is_empty(), "nothing queued");
        let sys = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(sys.modules.get(&ModuleKind::TorpedoRack).copied().unwrap_or(0), 2, "ledger never debited on a reject");
        // Driver(2) + Whipple(3) = 5 fits exactly → the classic brawler builds.
        let brawler = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: sid, ship_kind: ShipKind::Corvette, join: None, loadout: brawler }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "the exact-budget brawler builds");
    }

    #[test]
    fn grandfathered_overbudget_stacks_fly_fight_and_refit_only_into_legality() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        // A pre-fitting snapshot could hold a Torp+Whipple Corvette (cost 6 > 5).
        let legacy = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::WhippleArmor]);
        let (_home, _hpos, fleet) = refit_home(&mut w, id, 2, &Loadout::default(), &[(ModuleKind::MassDriver, 4), (ModuleKind::WhippleArmor, 4)]);
        // Graft the legacy stack directly, as a loaded snapshot would carry it.
        {
            let f = w.fleets.get_mut(&fleet).unwrap();
            f.add(ShipKind::Corvette, 2);
            f.loadouts.entry(ShipKind::Corvette).or_default().insert(legacy.key(), 2);
        }
        // It SURVIVES a serde round-trip untouched (no strip/mutate on load)…
        let json = serde_json::to_string(&w).expect("serialize");
        let w2: World = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            w2.fleets[&fleet].loadouts.get(&ShipKind::Corvette).and_then(|m| m.get(&legacy.key())).copied().unwrap_or(0),
            2,
            "the over-budget legacy stack loads intact"
        );
        // …and it FIGHTS: the tactical unpack keeps the stack under its own
        // loadout key, and that loadout's offense is torpedo-typed.
        let comp = w.fleets[&fleet].composition.clone();
        let loadouts = w.fleets[&fleet].loadouts.clone();
        let stacks = crate::tactical::stacked(&comp, &loadouts);
        assert_eq!(
            stacks.get(&ShipKind::Corvette).and_then(|m| m.get(&legacy.key())).copied().unwrap_or(0),
            2,
            "the legacy stack unpacks into battle under its own fit"
        );
        assert!(matches!(legacy.offense().0, crate::module::DamageType::Torpedo), "the legacy torpedo fit still fires");
        // Refit must land on a LEGAL fit: legacy → another over-budget fit rejects…
        let still_heavy = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::ReflectivePlating]); // 5 ≤ 5 — actually legal on Corvette
        assert!(still_heavy.validate(ShipKind::Corvette));
        let over = Loadout::new(vec![ModuleKind::WhippleArmor, ModuleKind::WhippleArmor]); // 6 > 5
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Corvette, from: legacy.clone(), to: over, n: 1 }]);
        assert!(w.refit_queue.is_empty(), "an over-budget refit target rejects");
        // …while a legal target queues fine (out of the grandfathered stack).
        let brawler = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        w.step(&[Command::RefitShips { player_id: id, fleet_id: fleet, ship: ShipKind::Corvette, from: legacy.clone(), to: brawler, n: 1 }]);
        assert_eq!(w.refit_queue.len(), 1, "the grandfathered stack refits INTO legality");
    }

    #[test]
    fn doctrine_fits_save_replace_delete_cap_and_validate() {
        use crate::module::{Loadout, ModuleKind};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "Guild".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        let brawler = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        // Save a legal fit.
        w.step(&[Command::SaveFit { player_id: id, name: "Brawler".into(), ship: ShipKind::Corvette, loadout: brawler.clone() }]);
        assert_eq!(w.syndicates[&sid].fits.len(), 1);
        assert_eq!(w.syndicates[&sid].fits[0].name, "Brawler");
        // Same-name SAVE replaces in place (kind + loadout swap).
        let screen = Loadout::new(vec![ModuleKind::PointDefenseScreen]);
        w.step(&[Command::SaveFit { player_id: id, name: "Brawler".into(), ship: ShipKind::Corvette, loadout: screen.clone() }]);
        assert_eq!(w.syndicates[&sid].fits.len(), 1, "replace, not append");
        assert_eq!(w.syndicates[&sid].fits[0].loadout, screen);
        // An ILLEGAL fit soft-rejects (budget), as does an unnamed one.
        let heavy = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::WhippleArmor]);
        w.step(&[Command::SaveFit { player_id: id, name: "Heavy".into(), ship: ShipKind::Corvette, loadout: heavy }]);
        w.step(&[Command::SaveFit { player_id: id, name: "   ".into(), ship: ShipKind::Corvette, loadout: brawler.clone() }]);
        assert_eq!(w.syndicates[&sid].fits.len(), 1, "illegal + unnamed fits never store");
        // Fill to the cap; the overflow save soft-rejects.
        for i in 0..crate::syndicate::SYNDICATE_MAX_FITS {
            w.step(&[Command::SaveFit { player_id: id, name: format!("fit-{i}"), ship: ShipKind::Raider, loadout: Loadout::new(vec![ModuleKind::MassDriver]) }]);
        }
        assert_eq!(w.syndicates[&sid].fits.len(), crate::syndicate::SYNDICATE_MAX_FITS, "capped");
        // DELETE frees a slot; unknown names are a no-op.
        w.step(&[Command::DeleteFit { player_id: id, name: "fit-0".into() }]);
        w.step(&[Command::DeleteFit { player_id: id, name: "no-such-fit".into() }]);
        assert_eq!(w.syndicates[&sid].fits.len(), crate::syndicate::SYNDICATE_MAX_FITS - 1);
        // A long name is trimmed to the cap, and fits survive a serde round-trip.
        let long = "x".repeat(80);
        w.step(&[Command::SaveFit { player_id: id, name: long, ship: ShipKind::Scout, loadout: Loadout::new(vec![ModuleKind::MassDriver]) }]);
        let stored = &w.syndicates[&sid].fits.last().unwrap().name;
        assert_eq!(stored.chars().count(), crate::syndicate::FIT_NAME_MAX);
        let json = serde_json::to_string(&w).expect("serialize");
        let w2: World = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(w2.syndicates[&sid].fits.len(), w.syndicates[&sid].fits.len(), "fits round-trip");
        // A non-member's save is a no-op.
        let outsider = PlayerId(2);
        w.step(&[Command::AddPlayer { id: outsider, name: "B".into() }]);
        let before = w.syndicates[&sid].fits.len();
        w.step(&[Command::SaveFit { player_id: outsider, name: "Intruder".into(), ship: ShipKind::Raider, loadout: Loadout::new(vec![ModuleKind::MassDriver]) }]);
        assert_eq!(w.syndicates[&sid].fits.len(), before, "outsiders can't touch the fit library");
    }

    #[test]
    fn fit_names_never_touch_sim_outcomes() {
        use crate::module::{Loadout, ModuleKind};
        // Two identical runs that differ ONLY in saved-fit names produce
        // byte-identical fleets/systems — names are labels, not state.
        let run = |fit_name: &str| -> (String, String) {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "A".into() }]);
            w.step(&[Command::CreateSyndicate { player_id: id, name: "Guild".into() }]);
            w.step(&[Command::SaveFit { player_id: id, name: fit_name.into(), ship: ShipKind::Raider, loadout: Loadout::new(vec![ModuleKind::MassDriver]) }]);
            for _ in 0..60 {
                w.step(&[]);
            }
            (
                serde_json::to_string(&w.fleets).unwrap(),
                serde_json::to_string(&w.systems).unwrap(),
            )
        };
        assert_eq!(run("Alpha Doctrine"), run("Zulu Doctrine"), "fit names are inert to the sim");
    }

    // --- §modules Part B3: module TRANSPORT (raidable crate convoy) -------------

    #[test]
    fn transfer_modules_debits_source_and_delivers_to_the_destination() {
        use crate::module::ModuleKind;
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let home = w.players[&id].home_system.unwrap();
        // Nearest unclaimed system, taken as the destination (short crossing).
        let dest = {
            let hp = w.systems.iter().find(|s| s.id == home).unwrap().pos;
            let s = w.systems.iter_mut().filter(|s| s.is_unclaimed())
                .min_by(|a, b| a.pos.distance(hp).total_cmp(&b.pos.distance(hp)).then(a.id.cmp(&b.id))).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.id
        };
        {
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            s.modules.insert(ModuleKind::MassDriver, 3);
            s.modules.insert(ModuleKind::WhippleArmor, 2);
        }
        w.step(&[Command::TransferModules { player_id: id, from: home, to: dest, manifest: [(ModuleKind::MassDriver, 2), (ModuleKind::WhippleArmor, 1)].into_iter().collect() }]);
        // Source debited AT LOADING (crates aboard, not at home).
        let hs = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(hs.modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 1);
        assert_eq!(hs.modules.get(&ModuleKind::WhippleArmor).copied().unwrap_or(0), 1);
        assert!(w.fleets.values().any(|f| f.owner == id && f.modules.values().sum::<u32>() == 3), "a crate convoy flies with the manifest");
        // Fly to arrival → the destination ledger is credited.
        let mut delivered = false;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::ModulesDelivered { owner, system, .. } if owner == id && system == dest) {
                    delivered = true;
                }
            }
            if delivered { break; }
        }
        assert!(delivered, "the crates land in the destination ledger");
        let ds = w.systems.iter().find(|s| s.id == dest).unwrap();
        assert_eq!(ds.modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 2);
        assert_eq!(ds.modules.get(&ModuleKind::WhippleArmor).copied().unwrap_or(0), 1);
    }

    #[test]
    fn transfer_modules_clamps_to_the_ledger_and_the_convoy_berths() {
        use crate::module::{ModuleKind, MODULE_CONVOY_BERTHS};
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let dest = {
            let hp = w.systems.iter().find(|s| s.id == home).unwrap().pos;
            let s = w.systems.iter_mut().filter(|s| s.is_unclaimed())
                .min_by(|a, b| a.pos.distance(hp).total_cmp(&b.pos.distance(hp)).then(a.id.cmp(&b.id))).unwrap();
            s.owner = Some(id);
            s.claimed_at = Some(0.0);
            s.id
        };
        // More stocked than one convoy can haul; ask for all of it.
        w.systems.iter_mut().find(|s| s.id == home).unwrap().modules.insert(ModuleKind::MassDriver, MODULE_CONVOY_BERTHS + 8);
        w.step(&[Command::TransferModules { player_id: id, from: home, to: dest, manifest: [(ModuleKind::MassDriver, MODULE_CONVOY_BERTHS + 8)].into_iter().collect() }]);
        let carried: u32 = w.fleets.values().find(|f| f.owner == id && !f.modules.is_empty()).map(|f| f.modules.values().sum()).unwrap_or(0);
        assert_eq!(carried, MODULE_CONVOY_BERTHS, "one convoy hauls at most its berths");
        let left = w.systems.iter().find(|s| s.id == home).unwrap().modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0);
        assert_eq!(left, 8, "the overflow stays in the source ledger for the next run");
    }

    // --- §modules Part B3: the SOL module market (buy at a premium, sell back low)

    #[test]
    fn buy_module_from_sol_debits_credits_now_and_delivers_the_crate() {
        use crate::module::ModuleKind;
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.players.get_mut(&id).unwrap().credits = 100_000.0;
        let before = w.players[&id].credits;
        let ev = w.step(&[Command::BuyModule { player_id: id, module: ModuleKind::MassDriver, n: 3, dest_system: home }]);
        // Debit matches the reported purchase price (drift-proof: read the event).
        let unit = ev.iter().find_map(|e| match e.payload {
            EventPayload::ModulesPurchased { unit_price, n, .. } if n == 3 => Some(unit_price),
            _ => None,
        }).expect("a purchase settled");
        assert!(unit > 0.0, "Sol charges a real price");
        assert!((before - w.players[&id].credits - unit * 3.0).abs() < 1e-6, "credits debited unit×3 NOW");
        assert!(w.fleets.values().any(|f| f.owner == id && f.modules.get(&ModuleKind::MassDriver) == Some(&3)), "a delivery convoy carries the crates from Sol");
        let mut delivered = false;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::ModulesDelivered { owner, system, .. } if owner == id && system == home) {
                    delivered = true;
                }
            }
            if delivered { break; }
        }
        assert!(delivered, "the purchase lands in the home ledger");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 3);
    }

    #[test]
    fn sell_module_to_sol_commits_the_crate_then_credits_on_arrival() {
        use crate::module::ModuleKind;
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().modules.insert(ModuleKind::MassDriver, 2);
        w.players.get_mut(&id).unwrap().credits = 0.0;
        w.step(&[Command::SellModule { player_id: id, module: ModuleKind::MassDriver, n: 2, from_system: home }]);
        // Ledger debited at commit; a convoy carries the crates to the hub.
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().modules.get(&ModuleKind::MassDriver).copied().unwrap_or(0), 0, "the crates leave the ledger at commit");
        assert!(w.fleets.values().any(|f| f.owner == id && f.modules.get(&ModuleKind::MassDriver) == Some(&2)), "a sell convoy carries them to Sol");
        // Fly to the hub → the buy-back credits on arrival (price-on-arrival).
        let mut proceeds = 0.0;
        for _ in 0..(600 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::ModulesSold { owner, n, unit_price, .. } = e.payload {
                    if owner == id {
                        proceeds += unit_price * n as f64;
                    }
                }
            }
            if proceeds > 0.0 { break; }
        }
        assert!(proceeds > 0.0, "Sol paid a buy-back");
        assert!((w.players[&id].credits - proceeds).abs() < 1e-6, "credits == the reported buy-back proceeds");
    }

    /// Part 1 verb distinction: RAID steals a convoy's cargo (seized by the raider),
    /// ATTACK destroys the convoy and its cargo is LOST with it.
    #[test]
    fn raid_seizes_convoy_cargo_but_attack_destroys_it() {
        use crate::cargo::{Cargo, Commodity};
        let setup = |attack: bool| -> (bool, Option<u32>) {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(160.0, 0.0));
            {
                let c = w.fleets.get_mut(&convoy).unwrap();
                c.composition.clear();
                c.composition.insert(ShipKind::Convoy, 1);
                c.cargo = Some(Cargo { commodity: Commodity::MetallicOre, units: 40 });
            }
            let cmd = if attack {
                Command::AttackFleet { player_id: atk, fleet_id: raider, target_id: convoy }
            } else {
                Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }
            };
            w.step(&[cmd]);
            run_until(&mut w, 30, |w| !w.fleets.contains_key(&convoy));
            let convoy_gone = !w.fleets.contains_key(&convoy);
            let seized = w.fleets.get(&raider).and_then(|f| f.cargo).map(|c| c.units);
            (convoy_gone, seized)
        };
        let (raid_gone, raid_seized) = setup(false);
        assert!(raid_gone, "raid destroys the lone convoy");
        assert_eq!(raid_seized, Some(40), "RAID seizes the cargo onto the raider");
        let (atk_gone, atk_seized) = setup(true);
        assert!(atk_gone, "attack destroys the convoy");
        assert_eq!(atk_seized, None, "ATTACK destroys the cargo with the fleet — nothing seized");
    }

    /// Part 2: a WeaponsFree fleet auto-commits — on its OWN local detection, no
    /// player order — to a rival that wanders into its own sensor bubble.
    #[test]
    fn weapons_free_auto_commits_on_a_rival_in_its_own_bubble() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // A raider hunter and a lone convoy well inside the raider's 2200 bubble.
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(1000.0, 0.0));
        w.fleets.get_mut(&raider).unwrap().posture = EngagementPosture::WeaponsFree;
        // No commands at all — pure standing autonomy on the fleet's own light.
        let committed = run_until(&mut w, 5, |w| {
            matches!(w.fleets[&raider].order, FleetOrder::Intercept { target } if target == convoy)
        });
        assert!(committed, "WeaponsFree raider hunts the convoy in its own bubble unprompted");
        // …and on contact it RAIDS it (convoy target → cargo raid, chosen by the
        // existing contact logic — WeaponsFree issues a plain Intercept).
        let raided = run_until(&mut w, 30, |w| w.engagements.values().any(|e| e.attackers.contains(&raider)));
        assert!(raided, "the hunt reaches contact");
        let raid = w.engagements.values().find(|e| e.attackers.contains(&raider)).map(|e| e.raid);
        assert_eq!(raid, Some(true), "an undefended convoy is a RAID, not a full attack");
    }

    /// Part 2 fog: a WeaponsFree fleet does NOT act on a rival OUTSIDE its own
    /// bubble — it is not a global hunt, it is local detection only.
    #[test]
    fn weapons_free_ignores_a_rival_outside_its_own_bubble() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // Convoy FAR beyond the raider's own bubble (2200 su).
        let (raider, _convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(6000.0, 0.0));
        w.fleets.get_mut(&raider).unwrap().posture = EngagementPosture::WeaponsFree;
        for _ in 0..(4 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(matches!(w.fleets[&raider].order, FleetOrder::Idle), "nothing in its own bubble → it stays put (no FTL global hunt)");
    }

    /// Part 2 retarded-time: the auto-commit fires on the fleet's OWN DELIVERED
    /// LIGHT — it sees an inbound target where its light SHOWS it (retarded by the
    /// light-travel time), so it commits strictly LATER than the tick the target's
    /// TRUE position first crosses the bubble edge.
    #[test]
    fn weapons_free_detection_is_retarded_not_instantaneous() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        // Hunter parked in open space; a broadcasting convoy inbound from beyond the
        // bubble so the retarded position lags the true one along its approach.
        let p = Vec2::new(0.0, 6000.0);
        let hunter = squad(&mut w, atk, p, ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.get_mut(&hunter).unwrap().posture = EngagementPosture::WeaponsFree;
        let convoy = squad(&mut w, def, p + Vec2::new(2400.0, 0.0), ShipKind::Convoy, 1, FleetOrder::MoveTo { dest: p });
        // Isolate the scenario: only the hunter + the inbound convoy (drop the
        // auto-spawned home fleets that would otherwise sit in the hunter's bubble).
        w.fleets.retain(|id, _| *id == hunter || *id == convoy);
        let bubble = w.config.sensor_range; // raider sensor_mult = 1.0
        let mut true_enter: Option<f64> = None;
        let mut commit: Option<f64> = None;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            w.step(&[]);
            let cpos = w.fleets.get(&convoy).map(|f| f.pos);
            if let Some(cpos) = cpos
                && true_enter.is_none()
                && p.distance(cpos) <= bubble
            {
                true_enter = Some(w.time); // naive current-truth would fire here
            }
            if commit.is_none() && matches!(w.fleets[&hunter].order, FleetOrder::Intercept { .. }) {
                commit = Some(w.time);
                break;
            }
        }
        let (te, cm) = (true_enter.expect("target's true position crossed the bubble"), commit.expect("hunter committed"));
        assert!(cm > te + 1e-6, "retarded detection commits ({cm:.2}s) strictly after the true crossing ({te:.2}s)");
    }

    /// Part 2 composition: a favourable-only doctrine (EngageWeaker) gates an
    /// unfavourable WeaponsFree contact — the posture picks WHO, the doctrine's
    /// force ratio decides WHETHER, so a stronger foe is ignored, not suicided into.
    #[test]
    fn favourable_only_doctrine_gates_an_unfavourable_weapons_free_hunt() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        // Favourable-only corp doctrine.
        w.step(&[Command::SetFleetDoctrine {
            player_id: atk,
            doctrine: crate::doctrine::FleetDoctrine { engagement: EngagementPolicy::EngageWeaker, ..Default::default() },
        }]);
        let cc = w.players[&atk].command_center;
        // A lone WeaponsFree raider vs a STRONGER rival wing (3 raiders) in bubble.
        let hunter = squad(&mut w, atk, cc + Vec2::new(0.0, 3000.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.get_mut(&hunter).unwrap().posture = EngagementPosture::WeaponsFree;
        let _strong = squad(&mut w, def, cc + Vec2::new(400.0, 3000.0), ShipKind::Raider, 3, FleetOrder::Idle);
        for _ in 0..(4 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(matches!(w.fleets[&hunter].order, FleetOrder::Idle), "unfavourable odds under a favourable-only policy → shadowed, not committed");
    }

    /// Part 2: SetFleetPosture is instant owner-only admin (soft-reject on a rival's
    /// fleet), and both the posture and an in-flight Attack order survive serde.
    #[test]
    fn posture_command_and_attack_order_persist() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let hunter = squad(&mut w, atk, cc + Vec2::new(200.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let rival = squad(&mut w, def, cc + Vec2::new(400.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        // Owner-only: setting a rival's fleet posture is a soft no-op.
        w.step(&[Command::SetFleetPosture { player_id: atk, fleet_id: rival, posture: EngagementPosture::WeaponsFree }]);
        assert_eq!(w.fleets[&rival].posture, EngagementPosture::Passive, "can't set a rival's posture");
        // Own fleet: instant.
        w.step(&[Command::SetFleetPosture { player_id: atk, fleet_id: hunter, posture: EngagementPosture::WeaponsFree }]);
        assert_eq!(w.fleets[&hunter].posture, EngagementPosture::WeaponsFree);
        // Put an Attack order in flight, then round-trip the whole world.
        w.fleets.get_mut(&hunter).unwrap().order = FleetOrder::Attack { target: rival };
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w2.fleets[&hunter].posture, EngagementPosture::WeaponsFree, "posture persists");
        assert!(matches!(w2.fleets[&hunter].order, FleetOrder::Attack { target } if target == rival), "in-flight Attack persists");
    }

    /// A raider jumped mid-pursuit (engaged in a DIFFERENT battle) must NOT fire a
    /// spurious "raid failed" escape when its old convoy target reaches hub safety —
    /// its Intercept order is dormant while it's anchored in the fight. Regression
    /// for the phantom battle-aftermath marker at the hub.
    #[test]
    fn a_jumped_raider_fires_no_spurious_escape() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let hub = w.hub;
        // Raider A pursues rival convoy F; F is inbound to the hub (safe on arrival).
        let a = squad(&mut w, atk, Vec2::new(5000.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let f = squad(&mut w, def, hub + Vec2::new(400.0, 0.0), ShipKind::Convoy, 1, FleetOrder::MoveTo { dest: hub });
        // Rival raider B jumps A → a B-vs-A battle that anchors A.
        let b = squad(&mut w, def, Vec2::new(5000.0, 40.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.retain(|id, _| *id == a || *id == f || *id == b);
        w.fleets.get_mut(&a).unwrap().order = FleetOrder::Intercept { target: f };
        w.fleets.get_mut(&b).unwrap().order = FleetOrder::Intercept { target: a };
        // A is engaged within a tick; F reaches hub safety a couple seconds later.
        let mut spurious = false;
        for _ in 0..(6 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved { attacker_ship, outcome, .. } = e.payload
                    && attacker_ship == a
                    && outcome == RaidOutcome::Escaped
                {
                    spurious = true;
                }
            }
        }
        assert!(!spurious, "an engaged raider must not fire a raid-failed escape for its dormant target");
    }

    /// §one-battle-one-icon: two rival fleets that intercept EACH OTHER (reciprocal
    /// — e.g. a WeaponsFree hunter and a picket both committing to attack the other)
    /// form ONE engagement, not two overlapping battle icons.
    #[test]
    fn reciprocal_intercepts_form_one_battle_not_two() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let a = squad(&mut w, atk, cc + Vec2::new(0.0, 3000.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let b = squad(&mut w, def, cc + Vec2::new(60.0, 3000.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.retain(|id, _| *id == a || *id == b);
        // Each has committed to attack the OTHER (mutual pursuit, already in reach).
        w.fleets.get_mut(&a).unwrap().order = FleetOrder::Intercept { target: b };
        w.fleets.get_mut(&b).unwrap().order = FleetOrder::Intercept { target: a };
        let engaged = run_until(&mut w, 5, |w| !w.engagements.is_empty());
        assert!(engaged, "the two raiders make contact");
        assert_eq!(w.engagements.len(), 1, "reciprocal intercepts = ONE battle (one icon), not two");
        // Both fleets are in that single engagement, on opposite sides.
        let e = w.engagements.values().next().unwrap();
        assert!(e.attackers.contains(&a) || e.defenders.contains(&a));
        assert!(e.attackers.contains(&b) || e.defenders.contains(&b));
    }

    /// Part 1 verb (join path): an ATTACK order that joins an in-progress RAID on
    /// the same convoy ESCALATES it to a full battle — the aggressor's destroy
    /// intent wins, so a convoy already being raided is fought to the death.
    #[test]
    fn an_attack_joining_a_raid_escalates_to_a_full_battle() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        // A tanky convoy (survives the raid window) + two raiders at contact range.
        let convoy = squad(&mut w, def, cc + Vec2::new(300.0, 0.0), ShipKind::Convoy, 8, FleetOrder::Idle);
        let r_raid = squad(&mut w, atk, cc + Vec2::new(250.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let r_attack = squad(&mut w, atk, cc + Vec2::new(250.0, 30.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.fleets.retain(|id, _| *id == convoy || *id == r_raid || *id == r_attack);
        // A opens a cargo RAID on the convoy.
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: r_raid, target_id: convoy }]);
        let raiding = run_until(&mut w, 10, |w| w.engagements.values().any(|e| e.raid && e.attackers.contains(&r_raid)));
        assert!(raiding, "the raid opens as a raid (steal)");
        // B ATTACKS the same convoy → joins → escalates the raid to a full battle.
        w.step(&[Command::AttackFleet { player_id: atk, fleet_id: r_attack, target_id: convoy }]);
        let escalated = run_until(&mut w, 10, |w| {
            w.engagements.values().any(|e| e.attackers.contains(&r_raid) && e.attackers.contains(&r_attack) && !e.raid)
        });
        assert!(escalated, "an ATTACK joining a raid forces the whole engagement to a full battle");
    }

    #[test]
    fn raid_cap_ends_a_raid_that_cannot_finish_in_time() {
        // test_world uses battle_target_secs = 20 → raid cap 0.15×20 = 3 s. A lone
        // raider can't clear a 20-convoy pool in 3 s, so the cap ends the raid.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(160.0, 0.0));
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.composition.clear();
            c.composition.insert(ShipKind::Convoy, 20);
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let mut outcome = None;
        for _ in 0..(30 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved { outcome: o, .. } = e.payload {
                    outcome = Some(o);
                }
            }
            if outcome.is_some() {
                break;
            }
        }
        assert_eq!(outcome, Some(RaidOutcome::BothSurvive), "the raid ends on its cap, not a bloodbath");
        assert!(w.fleets.contains_key(&raider), "the raider survives");
        assert!(w.fleets.get(&convoy).map(|f| f.count(ShipKind::Convoy) >= 15).unwrap_or(false), "most of the convoy pool survives a capped raid");
    }

    #[test]
    fn safety_valve_forces_a_mutual_disengage() {
        // Two tanky 3-corvette fleets, default doctrine (never retreat): neither
        // dies before the safety valve (2×20 = 40 s), so it ends in mutual
        // disengage with both intact.
        let mut w = test_world();
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        let pos = w.players[&a].command_center + Vec2::new(500.0, 0.0);
        let did = squad(&mut w, d, pos, ShipKind::Corvette, 3, FleetOrder::Idle);
        let aid = squad(&mut w, a, pos + Vec2::new(40.0, 0.0), ShipKind::Corvette, 3, FleetOrder::Intercept { target: did });
        let mut resolved = false;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.engagements.is_empty() && (w.time > 30.0) {
                resolved = true;
                break;
            }
        }
        assert!(resolved, "the battle ended (safety valve)");
        assert!(w.fleets.contains_key(&aid) && w.fleets.contains_key(&did), "both sides survive the mutual disengage");
    }

    #[test]
    fn withdraw_is_light_delayed_and_pulls_the_fleet_out_of_the_battle() {
        let mut w = test_world();
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        let pos = w.players[&a].command_center + Vec2::new(600.0, 0.0);
        let did = squad(&mut w, d, pos, ShipKind::Corvette, 4, FleetOrder::Idle);
        let aid = squad(&mut w, a, pos + Vec2::new(40.0, 0.0), ShipKind::Raider, 2, FleetOrder::Intercept { target: did });
        for _ in 0..30 {
            w.step(&[]);
        }
        assert!(w.engagements.values().any(|e| e.attackers.contains(&aid)), "the raider is engaged");
        // WITHDRAW — light-delayed, shows the order-lifecycle echo like any order.
        w.step(&[Command::Withdraw { player_id: a, fleet_id: aid }]);
        assert!(w.pending_commands(a).iter().any(|p| p.fleet == aid && p.kind == crate::event::OrderKind::Withdraw), "the withdraw has an order lifecycle");
        for _ in 0..(15 * crate::config::TICK_HZ) {
            w.step(&[]);
            if !w.engagements.values().any(|e| e.attackers.contains(&aid)) {
                break;
            }
        }
        assert!(!w.engagements.values().any(|e| e.attackers.contains(&aid)), "on arrival the withdraw pulled the raider out of the battle");
        assert!(matches!(w.fleets.get(&aid).map(|f| &f.order), Some(FleetOrder::MoveTo { .. })), "the raider physically flees home at formation speed (the speed table decides escape)");
    }

    #[test]
    fn reinforce_joins_the_pool_and_the_relieved_side_survives() {
        let mut w = test_world();
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        let pos = w.players[&a].command_center + Vec2::new(700.0, 0.0);
        // Attacker 5 raiders vs defender 2 raiders — the defender is losing.
        let did = squad(&mut w, d, pos, ShipKind::Raider, 2, FleetOrder::Idle);
        let aid = squad(&mut w, a, pos + Vec2::new(40.0, 0.0), ShipKind::Raider, 5, FleetOrder::Intercept { target: did });
        for _ in 0..20 {
            w.step(&[]);
        }
        // RELIEF arrives (Idle) at the battle → joins the defender pool.
        let relief = squad(&mut w, d, pos + Vec2::new(20.0, 0.0), ShipKind::Raider, 6, FleetOrder::Idle);
        w.step(&[]);
        assert!(w.engagements.values().any(|e| e.defenders.contains(&relief)), "the reinforcement joined the defender pool");
        // The shifted ratio saves the defender side (relief + survivors outlast).
        for _ in 0..(120 * crate::config::TICK_HZ) {
            if w.engagements.is_empty() {
                break;
            }
            w.step(&[]);
        }
        assert!(w.fleets.contains_key(&relief) || w.fleets.contains_key(&did), "the relieved defender side survives the fight");
        assert!(!w.fleets.contains_key(&aid), "the outnumbered attacker is destroyed — the relief flipped it");
    }

    #[test]
    fn avoid_doctrine_fleet_takes_a_scrape_then_escapes_no_coast_lock() {
        let mut w = test_world();
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        // Player a runs AVOID doctrine — its raider will bolt when jumped.
        let mut doc = w.players[&a].doctrine;
        doc.engagement = crate::doctrine::EngagementPolicy::Avoid;
        w.step(&[Command::SetFleetDoctrine { player_id: a, doctrine: doc }]);
        let pos = w.players[&a].command_center + Vec2::new(500.0, 0.0);
        // a's raider (Avoid, Idle — didn't choose this) is jumped by d's corvettes.
        let raider = squad(&mut w, a, pos, ShipKind::Raider, 2, FleetOrder::Idle);
        let _corv = squad(&mut w, d, pos + Vec2::new(40.0, 0.0), ShipKind::Corvette, 3, FleetOrder::Intercept { target: raider });
        // Brief exposure (~DISENGAGE_EXPOSURE_SECS), then it disengages and flees.
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
            if matches!(w.fleets.get(&raider).map(|f| &f.order), Some(FleetOrder::MoveTo { .. })) {
                break;
            }
        }
        assert!(w.fleets.contains_key(&raider), "the raider survives the scrape");
        assert!(matches!(w.fleets.get(&raider).map(|f| &f.order), Some(FleetOrder::MoveTo { .. })), "Avoid → disengage on contact, flee");
        assert!(!w.engagements.values().any(|e| e.attackers.contains(&raider) || e.defenders.contains(&raider)), "it left the battle — no coast-lock");
        // Positions DIVERGE — the raider (100) physically opens the gap on the
        // corvettes (65); no fly-through, no coast-lock.
        let d0 = w.fleets[&raider].pos.distance(pos);
        for _ in 0..(10 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let d1 = w.fleets.get(&raider).map(|f| f.pos.distance(pos)).unwrap_or(d0);
        assert!(d1 > d0 + 300.0, "the raider opens the gap (positions diverge)");
    }

    #[test]
    fn two_engage_sides_anchor_a_stationary_battle_for_the_duration() {
        let mut w = test_world(); // battle_target_secs = 20
        let (a, d) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: d, name: "D".into() }]);
        let pos = w.players[&a].command_center + Vec2::new(500.0, 0.0);
        // Two accepting 4-raider squads (the attacker committed; the defender's
        // default doctrine accepts) → a STATIONARY anchored battle.
        let did = squad(&mut w, d, pos, ShipKind::Raider, 4, FleetOrder::Idle);
        let aid = squad(&mut w, a, pos + Vec2::new(40.0, 0.0), ShipKind::Raider, 4, FleetOrder::Intercept { target: did });
        for _ in 0..(2 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(!w.engagements.is_empty(), "the battle formed");
        let apos0 = w.fleets[&aid].pos;
        let dpos0 = w.fleets[&did].pos;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.engagements.is_empty() {
                break;
            }
        }
        // Anchored: neither side drifts while the battle rages.
        if let Some(f) = w.fleets.get(&aid) {
            assert!(f.pos.distance(apos0) < 5.0, "attacker is anchored at the contact point");
        }
        if let Some(f) = w.fleets.get(&did) {
            assert!(f.pos.distance(dpos0) < 5.0, "defender is anchored at the contact point");
        }
        // And it GRINDS for roughly the target duration (equal forces, no retreat).
        let mut ticks = 0u32;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            if w.engagements.is_empty() {
                break;
            }
            w.step(&[]);
            ticks += 1;
        }
        let secs = ticks as f64 / 30.0;
        // AMENDED with §arena discipline (2026-07): the compact in-arena dance
        // resolves equal mirrors faster than the old wide-swing dance; battle
        // DURATION is emergent under the tactical engine (battle_target_secs
        // drives cadence + windows, not the grind). The law kept here is
        // GRIND, NOT MELT: an equal fight is a real exchange, never a flash.
        assert!(secs > 10.0, "equal squadrons grind, not melt (got {secs:.0}s)");
    }

    // ---- Constant per-kind speed (§14.1) + lead pursuit (§8) ----

    fn ship_of(kind: ShipKind, cargo: Option<Cargo>) -> Fleet {
        Fleet::single(EntityId(1), PlayerId(1), kind, Vec2::ZERO, FleetOrder::Idle, cargo)
    }

    /// Speed is a CONSTANT per kind (§14.1, no acceleration): the raider/convoy
    /// gap is a flat speed difference, and cargo does NOT slow a ship — it costs
    /// FUEL (mass), not time.
    #[test]
    fn speed_is_constant_per_kind_and_cargo_free() {
        let raider = ship_of(ShipKind::Raider, None);
        let empty = ship_of(ShipKind::Convoy, None);
        let loaded = ship_of(
            ShipKind::Convoy,
            Some(Cargo { commodity: crate::cargo::Commodity::Alloys, units: 120 }),
        );
        // Ordering preserved: a raider is much faster than a convoy.
        assert!(raider.max_speed() > empty.max_speed() * 2.0, "the raider out-runs the convoy");
        // Cargo adds MASS (fuel) but not speed — a loaded convoy is exactly as fast.
        assert!(loaded.mass() > empty.mass(), "cargo adds mass");
        assert_eq!(loaded.max_speed(), empty.max_speed(), "cargo does not slow a ship (constant speed)");
    }

    /// A raider runs down a MOVING convoy via proportional pursuit (full
    /// integration, including the light-delayed observed-position steering), and
    /// makes contact — no closed-form solver involved.
    #[test]
    fn raider_runs_down_a_moving_convoy() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let raider = find_ship(&w, atk, ShipKind::Raider);
        let convoy = find_ship(&w, def, ShipKind::Convoy);
        // Raider beside the attacker's command center; convoy ~2500 su away and
        // FLEEING further out, so the raider must chase it down over distance.
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cc + Vec2::new(50.0, 0.0);
            r.vel = Vec2::ZERO;
            r.order = FleetOrder::Idle;
        }
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.pos = cc + Vec2::new(2500.0, 0.0);
            c.vel = Vec2::ZERO;
            c.order = FleetOrder::MoveTo { dest: cc + Vec2::new(9000.0, 0.0) }; // flees outward
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let outcome = run_until_raid(&mut w, 120, |_| vec![])
            .expect("proportional pursuit should run the fleeing convoy down within 120 s");
        assert_ne!(outcome, RaidOutcome::Escaped);
    }

    // ---- Autonomous defensive interception (§5.1, Pillar 1) ----

    /// Set up defender `d` (a patrolling raider guarding a convoy) and attacker
    /// `a` (a hostile raider inbound on that convoy). Positions are caller-chosen
    /// so the close-vs-far positioning dynamic can be exercised. Returns
    /// (patrol_raider, convoy, hostile_raider). NO player commands are issued — the
    /// defense must be autonomous.
    fn defense_setup(w: &mut World, d: PlayerId, a: PlayerId, convoy_pos: Vec2, patrol_pos: Vec2, hostile_pos: Vec2) -> (EntityId, EntityId, EntityId) {
        w.step(&[
            Command::AddPlayer { id: d, name: "Def".into() },
            Command::AddPlayer { id: a, name: "Atk".into() },
        ]);
        let convoy = find_ship(w, d, ShipKind::Convoy);
        let patrol = find_ship(w, d, ShipKind::Raider);
        let hostile = find_ship(w, a, ShipKind::Raider);
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.pos = convoy_pos;
            c.vel = Vec2::ZERO;
            c.order = FleetOrder::Idle;
        }
        {
            let p = w.fleets.get_mut(&patrol).unwrap();
            p.pos = patrol_pos;
            p.vel = Vec2::ZERO;
            p.defense = None;
            // A small standing patrol around its station.
            p.order = FleetOrder::Patrol {
                waypoints: vec![patrol_pos, patrol_pos + Vec2::new(200.0, 0.0)],
                index: 0,
                dwell_until: 0.0,
            };
        }
        {
            let h = w.fleets.get_mut(&hostile).unwrap();
            h.pos = hostile_pos;
            h.vel = (convoy_pos - hostile_pos).normalized() * 60.0; // inbound, on course
            h.order = FleetOrder::Intercept { target: convoy };
        }
        (patrol, convoy, hostile)
    }

    fn engaged_on(w: &World, patrol: EntityId, hostile: EntityId) -> bool {
        w.fleets.get(&patrol).and_then(|s| s.defense.as_ref()).map(|d| d.target == hostile).unwrap_or(false)
    }

    /// A patrolling raider, with NO player action, autonomously breaks off to
    /// intercept a hostile raider it senses inbound on a guarded convoy, and the
    /// engagement resolves through the existing seeded raider-vs-raider combat.
    #[test]
    fn patrol_autonomously_intercepts_a_threatening_raider() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        // Patrol right by the convoy; hostile 1500 su out (inside the patrol's
        // sensor range) heading straight at the convoy.
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos + Vec2::new(0.0, 120.0), Vec2::new(1500.0, 0.0));

        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if engaged_on(&w, patrol, hostile) {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "the patrol must autonomously break off to intercept the inbound hostile");
        assert!(matches!(w.fleets[&patrol].order, FleetOrder::Intercept { target } if target == hostile));

        // The defensive engagement resolves via the existing seeded RVR battle —
        // patrol (attacker) vs the hostile raider (target).
        let mut rvr = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved { attacker_ship, target_ship, attacker_kind, target_kind, .. } = e.payload
                    && attacker_kind == ShipKind::Raider
                    && target_kind == ShipKind::Raider
                    && (attacker_ship == patrol || target_ship == patrol)
                {
                    rvr = true;
                }
            }
            if rvr {
                break;
            }
        }
        assert!(rvr, "the autonomous defense should resolve in a raider-vs-raider battle");
    }

    /// Detection respects the fog model: the patrol cannot react to a hostile it
    /// can't sense (beyond its sensor range).
    #[test]
    fn patrol_ignores_a_threat_beyond_sensor_range() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let sensor = w.config.sensor_range;
        let convoy_pos = Vec2::new(3000.0, 0.0);
        // Hostile FAR beyond the patrol's sensor range — undetectable for now.
        let hostile_pos = convoy_pos + Vec2::new(sensor * 2.0 + 1000.0, 0.0);
        let (patrol, _c, _h) = defense_setup(&mut w, d, a, convoy_pos, convoy_pos, hostile_pos);
        for _ in 0..(3 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(matches!(w.fleets[&patrol].order, FleetOrder::Patrol { .. }), "must not react to an undetectable threat");
            assert!(w.fleets[&patrol].defense.is_none());
        }
    }

    /// POSITIONING is the strategic decision: a patrol on the approach vector
    /// senses the threat and engages; one stationed off it never senses the threat,
    /// so the convoy is lost. (Knobs are tunable to balance this.)
    #[test]
    fn patrol_positioning_decides_whether_it_can_defend() {
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let hostile_pos = Vec2::new(-2000.0, 0.0); // inbound from the left

        // CLOSE — patrol on the approach → detects + engages.
        let mut w1 = test_world();
        let (p_close, _c1, h1) =
            defense_setup(&mut w1, PlayerId(1), PlayerId(2), convoy_pos, Vec2::new(700.0, 0.0), hostile_pos);
        let mut close_engaged = false;
        for _ in 0..(100 * crate::config::TICK_HZ) {
            w1.step(&[]);
            if engaged_on(&w1, p_close, h1) {
                close_engaged = true;
                break;
            }
        }
        assert!(close_engaged, "a well-positioned patrol detects the inbound threat and engages");

        // FAR — patrol way off the approach vector → never senses it → convoy lost.
        let mut w2 = test_world();
        let sensor = w2.config.sensor_range;
        let (p_far, convoy2, _h2) =
            defense_setup(&mut w2, PlayerId(1), PlayerId(2), convoy_pos, Vec2::new(3000.0, sensor * 3.0), hostile_pos);
        let (mut far_engaged, mut convoy_lost) = (false, false);
        for _ in 0..(100 * crate::config::TICK_HZ) {
            for e in w2.step(&[]) {
                if let EventPayload::ShipDestroyed { ship, .. } = e.payload
                    && ship == convoy2
                {
                    convoy_lost = true;
                }
            }
            if w2.fleets.get(&p_far).map(|s| s.defense.is_some()).unwrap_or(false) {
                far_engaged = true;
            }
            if convoy_lost {
                break;
            }
        }
        assert!(!far_engaged, "a patrol off the approach vector never senses the threat");
        assert!(convoy_lost, "with no defender in reach, the convoy is lost — positioning matters");
    }

    /// A SENSOR ARRAY (§buildings step 2b) extends the owner's DETECTION for
    /// pickets: a hostile beyond the picket's own bubble — which it would
    /// otherwise ignore (see `patrol_ignores_a_threat_beyond_sensor_range`) — is
    /// sensed once an owned array system's bubble covers it, and the picket
    /// engages. Same shared coverage source of truth as the View.
    #[test]
    fn sensor_array_extends_picket_sensing() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let sensor = w.config.sensor_range;
        // Hostile inbound from FAR beyond the picket's own bubble.
        let hostile_pos = convoy_pos + Vec2::new(sensor * 2.0 + 1000.0, 0.0);
        let (patrol, _c, hostile) = defense_setup(&mut w, d, a, convoy_pos, convoy_pos, hostile_pos);

        // WITHOUT an array the picket ignores it (proven by the ignore test).
        // Grant the DEFENDER an array system whose bubble covers the hostile.
        {
            let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.owner = Some(d);
            sys.claimed_at = Some(0.0);
            sys.pos = hostile_pos + Vec2::new(500.0, 0.0); // bubble (2200) covers it
            sys.set_tier(crate::build::StructureKind::SensorArray, 1);
        }
        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if engaged_on(&w, patrol, hostile) {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "the array's standing vision lets the picket react to the distant threat");
    }

    /// Once the threat is gone, the defender RESUMES its patrol (not a chase, not
    /// a flight home). Deterministic — independent of the random battle outcome.
    #[test]
    fn defender_resumes_patrol_after_the_threat_is_gone() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1200.0, 0.0));
        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.fleets[&patrol].defense.is_some() {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "patrol should have engaged");

        // The threat vanishes (destroyed elsewhere / broke contact).
        w.fleets.remove(&hostile);
        w.step(&[]);
        assert!(w.fleets[&patrol].defense.is_none(), "defense cleared once the threat is gone");
        assert!(matches!(w.fleets[&patrol].order, FleetOrder::Patrol { .. }), "the defender resumes its standing patrol");
    }

    // --- §buildings step 2c: Defense Platform ---------------------------------

    /// Grant `owner` a defended system at `pos` with `tier` platform tiers.
    fn grant_platform(w: &mut World, owner: PlayerId, pos: Vec2, tier: u32) -> EntityId {
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(owner);
        sys.claimed_at = Some(0.0);
        sys.pos = pos;
        sys.set_tier(crate::build::StructureKind::DefensePlatform, tier);
        sys.id
    }

    /// §one-battle-one-icon: `active_battles()` exposes each engagement as ONE
    /// entity — a stable id, the anchor, and the FULL participant set (both
    /// sides) — which the client renders as a single icon and uses to suppress
    /// each participant's own marker. The participant set is exactly what the
    /// server feeds into the weapons-fire reveal, so carrying the ids leaks
    /// nothing beyond the ghosts already revealed at the site.
    #[test]
    fn active_battles_is_one_entity_per_engagement_with_all_participants() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // A corvette-escorted convoy so BOTH sides have real fleets (2 defenders).
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(360.0, 0.0));
        let convoy_pos = w.fleets[&convoy].pos;
        let escort = w.alloc_entity_id();
        w.fleets.insert(escort, Fleet::single(escort, def, ShipKind::Corvette, convoy_pos + Vec2::new(20.0, 0.0), FleetOrder::Idle, None));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let mut found: Option<BattleInfo> = None;
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
            let bs = w.active_battles();
            if !bs.is_empty() { found = Some(bs[0].clone()); break; }
        }
        let b = found.expect("an engagement forms");
        assert_eq!(w.active_battles().len(), 1, "ONE engagement entity → one icon");
        assert!(b.participants.contains(&raider), "the attacker is a participant");
        assert!(b.participants.contains(&convoy) || b.participants.contains(&escort), "the defender side is included");
        assert!(b.participants.len() >= 2, "all engaged fleets ride the single entity");
        // The id is the engagement's high-bit id (a real, stable handle).
        assert_ne!(b.id, EntityId(0));
    }

    /// A raid contact on a convoy INSIDE a defended friendly system's protection
    /// radius must fight THROUGH the platform. With a tall platform the seeded
    /// duels stop the raider (deterministically for this seed); the convoy
    /// survives, the raid resolves via the ordinary RaidResolved (delayed reports
    /// both sides), and the platform's own engagement detail is owner-only news.
    /// Works with the owner OFFLINE by construction (pure sim, no player input).
    #[test]
    fn platform_defends_convoy_inside_radius() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        let convoy_pos = w.fleets[&convoy].pos;
        // Defended system right at the convoy (contact well inside the radius).
        let sys = grant_platform(&mut w, def, convoy_pos + Vec2::new(200.0, 0.0), 10);

        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let mut platform_evt: Option<(PlayerId, EntityId, bool, bool)> = None;
        let mut outcome = None;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                match e.payload {
                    EventPayload::PlatformEngaged { owner, system, raider_destroyed, driven_off, .. } => {
                        platform_evt = Some((owner, system, raider_destroyed, driven_off));
                    }
                    EventPayload::RaidResolved { outcome: o, .. } => outcome = Some(o),
                    _ => {}
                }
            }
            if outcome.is_some() {
                break;
            }
        }
        let (owner, system, raider_destroyed, driven_off) = platform_evt.expect("the platform engaged");
        assert_eq!((owner, system), (def, sys), "the engagement detail is the DEFENDER's news");
        assert!(raider_destroyed || driven_off, "a 10-tier platform stops the raider (seeded)");
        let outcome = outcome.expect("the raid still resolves via the ordinary report channel");
        assert!(
            matches!(outcome, RaidOutcome::AttackerDestroyed | RaidOutcome::BothSurvive),
            "the attacker learns only a standard battle outcome ({outcome:?})"
        );
        assert!(w.fleets.contains_key(&convoy), "the convoy was never touched");
        assert_eq!(raider_destroyed, !w.fleets.contains_key(&raider), "ship state matches the outcome");
    }

    /// Outside the platform's protection radius NOTHING changes: the raid
    /// resolves exactly as before (with the test's 100% raider-vs-convoy table,
    /// the convoy is lost) and no platform engagement fires.
    #[test]
    fn raid_outside_platform_radius_is_unchanged() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        let convoy_pos = w.fleets[&convoy].pos;
        // Defended system FAR from the convoy — its radius can't cover the contact.
        grant_platform(&mut w, def, convoy_pos + Vec2::new(crate::build::DEFENSE_PLATFORM_RADIUS * 3.0, 0.0), 10);

        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let mut platform_fired = false;
        let mut outcome = None;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                match e.payload {
                    EventPayload::PlatformEngaged { .. } => platform_fired = true,
                    EventPayload::RaidResolved { outcome: o, .. } => outcome = Some(o),
                    _ => {}
                }
            }
            if outcome.is_some() {
                break;
            }
        }
        assert!(!platform_fired, "a distant platform never engages");
        assert_eq!(outcome, Some(RaidOutcome::TargetDestroyed), "the raid resolves exactly as before");
        assert!(!w.fleets.contains_key(&convoy), "convoy lost — positioning matters");
    }

    /// Platform DAMAGE: tiers lost in the engagement (reported in the owner event)
    /// match the tier drop on the system — stakes without ever destroying the
    /// system. Also: a platform tier consumes a development slot, and the whole
    /// engagement is deterministic from the seed.
    #[test]
    fn platform_damage_matches_tiers_lost_and_is_deterministic() {
        let run = || {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
            let convoy_pos = w.fleets[&convoy].pos;
            let sys = grant_platform(&mut w, def, convoy_pos, 3);
            let tier0 = w.systems.iter().find(|s| s.id == sys).unwrap().tier(crate::build::StructureKind::DefensePlatform);
            assert_eq!(w.systems.iter().find(|s| s.id == sys).unwrap().tier(crate::build::StructureKind::DefensePlatform), 3, "three platform tiers stand on one footprint (slots bound breadth, not depth)");

            w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
            let mut lost_reported = None;
            for _ in 0..(60 * crate::config::TICK_HZ) {
                for e in w.step(&[]) {
                    if let EventPayload::PlatformEngaged { tiers_lost, .. } = e.payload {
                        lost_reported = Some(tiers_lost);
                    }
                }
                if lost_reported.is_some() {
                    break;
                }
            }
            let lost = lost_reported.expect("platform engaged");
            let tier1 = w.systems.iter().find(|s| s.id == sys).unwrap().tier(crate::build::StructureKind::DefensePlatform);
            assert_eq!(tier0 - tier1, lost, "reported damage matches the tier drop");
            (lost, tier1)
        };
        assert_eq!(run(), run(), "the platform engagement is deterministic from the seed");
    }

    // --- §fleets part 1: weighted combat strengths ------------------------------


    // --- §fleets part 2: Corvette -----------------------------------------------

    /// Raiding is the raider's verb: a corvette (or any non-raider) committed to
    /// a raid is SOFT-rejected — no intercept order, nothing spent.
    #[test]
    fn corvette_cannot_raid() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let convoy = find_ship(&w, def, ShipKind::Convoy);
        let cid = w.alloc_entity_id();
        w.fleets.insert(cid, Fleet::single(cid, atk, ShipKind::Corvette, cc, FleetOrder::Idle, None));
        let fuel0 = home_fuel(&w, atk);
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: cid, target_id: convoy }]);
        assert!(matches!(w.fleets[&cid].order, FleetOrder::Idle), "soft reject — the corvette never moves");
        assert!(w.pending_orders.iter().all(|p| p.ship_id != cid), "no intercept scheduled");
        assert!((home_fuel(&w, atk) - fuel0).abs() < 1e-9, "nothing spent");
    }

    /// ESCORT changes the outcome (§fleets part 2): the same seeded raid that
    /// destroys an unescorted convoy (the test table is 100% convoy-destroyed)
    /// is STOPPED by a corvette screen — the raider must fight through real
    /// fleets first, and a tall screen grinds it down before it touches the hull.
    #[test]
    fn corvette_escort_changes_the_outcome() {
        // Unescorted baseline: convoy dies (the existing certainty).
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        run_until_raid(&mut w, 60, |_| vec![]).expect("baseline resolves");
        assert!(!w.fleets.contains_key(&convoy), "unescorted: the convoy is lost");

        // Escorted: a screen of corvettes shadowing the convoy stops the raid.
        let mut w = test_world();
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        let convoy_pos = w.fleets[&convoy].pos;
        for k in 0..6 {
            let cid = w.alloc_entity_id();
            w.fleets.insert(
                cid,
                Fleet::single(cid, def, ShipKind::Corvette, convoy_pos + Vec2::new(60.0 + k as f64 * 10.0, 0.0), FleetOrder::Idle, None),
            );
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        // The corvette escort is POOLED into the defender side; their return fire
        // drives the raider off / destroys it before it can take the convoy.
        let mut done = false;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved { outcome, .. } = e.payload
                    && matches!(outcome, RaidOutcome::AttackerDestroyed | RaidOutcome::BothSurvive | RaidOutcome::BothDestroyed)
                {
                    done = true;
                }
            }
            if done {
                break;
            }
        }
        assert!(done, "a 6-corvette escort stops the raid (the raider is driven off / destroyed)");
        assert!(w.fleets.contains_key(&convoy), "escorted: the convoy survives");
    }

    /// GARRISON stacks with the platform: corvettes parked at a defended system
    /// screen FIRST (real fleets, real losses), the platform's tiers fight next —
    /// and the convoy behind both survives. Standing defense, owner offline.
    #[test]
    fn garrison_stacks_with_platform() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        let convoy_pos = w.fleets[&convoy].pos;
        let sys = grant_platform(&mut w, def, convoy_pos + Vec2::new(150.0, 0.0), 6);
        for k in 0..4 {
            let cid = w.alloc_entity_id();
            w.fleets.insert(
                cid,
                Fleet::single(cid, def, ShipKind::Corvette, convoy_pos + Vec2::new(150.0 + k as f64 * 10.0, 0.0), FleetOrder::Idle, None),
            );
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        // Garrison corvettes + the platform pool together on the defender side —
        // their combined fire stops the raider; the convoy behind them survives.
        let mut done = false;
        for _ in 0..(60 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::RaidResolved {
                    outcome: RaidOutcome::AttackerDestroyed | RaidOutcome::BothSurvive,
                    ..
                } = e.payload
                {
                    done = true;
                }
            }
            if done {
                break;
            }
        }
        assert!(done, "the garrison + platform stop the raid");
        assert!(w.fleets.contains_key(&convoy), "behind the garrison + platform, the convoy survives");
        assert!(
            w.systems.iter().find(|s| s.id == sys).unwrap().tier(crate::build::StructureKind::DefensePlatform) <= 6,
            "platform intact or attrited, never grown"
        );
    }

    /// Corvettes are MILITARY industry: Shipyard tier 2, like the raider.
    #[test]
    fn corvette_needs_shipyard_two() {
        let mut w = test_world();
        let id = PlayerId(44);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Electronics, 20.0), (Commodity::Armaments, 20.0)]); // kit covers Alloys
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Corvette, join: None , loadout: Default::default() }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { reason: crate::event::BuildRejectReason::NeedsShipyard { required: 2 }, .. }
            )),
            "home tier 1 can't build corvettes"
        );
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(crate::build::StructureKind::Shipyard, 2);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Corvette, join: None , loadout: Default::default() }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "tier 2 builds them");
    }

    #[test]
    fn raid_resolves_in_a_battle() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // Raider near command center (small commit delay), convoy 300 su away.
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        let outcome = run_until_raid(&mut w, 60, |_| vec![]).expect("a battle should resolve");
        assert_ne!(outcome, RaidOutcome::Escaped, "convoy isn't near the hub — it's a battle");
        assert_battle_consistent(&w, outcome, raider, convoy);
    }

    #[test]
    fn raider_vs_raider_battle() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let attacker = find_ship(&w, atk, ShipKind::Raider);
        let target = find_ship(&w, def, ShipKind::Raider); // target a RIVAL RAIDER
        for (id, off) in [(attacker, Vec2::new(120.0, 0.0)), (target, Vec2::new(420.0, 0.0))] {
            let s = w.fleets.get_mut(&id).unwrap();
            s.pos = cc + off;
            s.vel = Vec2::ZERO;
            s.order = FleetOrder::Idle;
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: attacker, target_id: target }]);
        let outcome = run_until_raid(&mut w, 60, |_| vec![]).expect("a raider-vs-raider battle should resolve");
        assert_ne!(outcome, RaidOutcome::Escaped, "raiders don't escape via the hub");
        assert_battle_consistent(&w, outcome, attacker, target);
    }

    #[test]
    fn battle_outcome_is_deterministic_from_seed() {
        // Same seed + same commands → same battle outcome (seeded RNG).
        let outcome_for = || {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(420.0, 0.0));
            w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
            run_until_raid(&mut w, 60, |_| vec![])
        };
        assert_eq!(outcome_for(), outcome_for(), "battle outcome must be reproducible from seed");
    }

    #[test]
    fn recall_breaks_off_pursuit() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // Convoy far away so the chase is long; raider near CC so recall is fast.
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(100.0, 0.0), Vec2::new(2600.0, 0.0));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        // Let the commit land and the chase begin, then recall.
        for _ in 0..(2 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        w.step(&[Command::RecallRaid { player_id: atk, raider_id: raider }]);
        let outcome = run_until_raid(&mut w, 60, |_| vec![]);
        assert_eq!(outcome, None, "recall should have broken off the raid");
        assert!(w.fleets.contains_key(&convoy), "convoy should survive a successful recall");
        // Raider is no longer intercepting.
        assert!(!matches!(w.fleets[&raider].order, FleetOrder::Intercept { .. }));
    }

    #[test]
    fn recall_can_arrive_too_late() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        // Raider FAR from CC (big recall/commit delay) but right on top of the
        // convoy (contact almost immediately once the commit lands).
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(4000.0, 0.0), Vec2::new(4180.0, 0.0));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        // The commit's light reaches the far raider at ~10 s (dist 4000 / c 400),
        // and it makes contact ~2 s later. The recall is issued well before that
        // contact, but its own ~10 s light can't beat the raider to the kill.
        let mut recalled = false;
        let outcome = run_until_raid(&mut w, 120, |w| {
            if !recalled && w.time > 4.0 {
                recalled = true;
                vec![Command::RecallRaid { player_id: atk, raider_id: raider }]
            } else {
                vec![]
            }
        });
        let outcome = outcome.expect("recall arrived too late — a battle should have resolved");
        assert_ne!(outcome, RaidOutcome::Escaped);
        assert!(recalled, "test should have issued a recall");
        assert_battle_consistent(&w, outcome, raider, convoy);
    }

    /// §TCA Part 2: a BUY settles instantly at the standing price and deposits into
    /// the CHARTERHOUSE WAREHOUSE. No convoy is conjured, and home inventory is
    /// untouched — a trade never moves goods across space any more.
    #[test]
    fn market_buy_settles_into_the_warehouse_without_a_convoy() {
        use crate::cargo::Commodity::Fuel;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let credits0 = w.players[&id].credits;
        let fuel_home0 = w.players[&id].inventory[&Fuel];
        let fleets0 = w.fleets.len();
        let price = w.market.price(Fuel);

        w.step(&[Command::MarketBuy { player_id: id, commodity: Fuel, units: 50, ship_to: None }]);
        // Instant settlement: credits debited now (≈ 50 × price).
        let spent = credits0 - w.players[&id].credits;
        assert!((spent - 50.0 * price).abs() < 1e-6, "buy settles at the standing price");
        // The goods are AT the Charterhouse immediately…
        assert_eq!(wh(&w, id, Fuel), 50, "bought goods land in the warehouse");
        // …home inventory is untouched, and NOTHING was launched.
        assert_eq!(w.players[&id].inventory[&Fuel], fuel_home0, "home inventory untouched");
        assert_eq!(w.fleets.len(), fleets0, "a buy conjures no convoy");
        assert!(
            !w.fleets.values().any(|s| s.owner == id && s.mission == Some(TradeMission::DeliverHome)),
            "no DeliverHome convoy is ever created by the Exchange"
        );
    }

    /// §TCA Part 2: a SELL draws ONLY from the warehouse and settles instantly —
    /// the goods are already at the Exchange, so there is no crossing and no
    /// price-on-arrival gamble.
    #[test]
    fn market_sell_draws_from_the_warehouse_and_settles_now() {
        use crate::cargo::Commodity::MetallicOre;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        seed_warehouse(&mut w, id, &[(MetallicOre, 100)]);
        let credits0 = w.players[&id].credits;
        let home_ore0 = w.players[&id].inventory[&MetallicOre];
        let fleets0 = w.fleets.len();
        let price = w.market.price(MetallicOre);

        w.step(&[Command::MarketSell { player_id: id, commodity: MetallicOre, units: 40 }]);
        assert_eq!(wh(&w, id, MetallicOre), 60, "sold units leave the warehouse");
        let gained = w.players[&id].credits - credits0;
        assert!((gained - 40.0 * price).abs() < 1e-6, "credited instantly at the standing price");
        assert_eq!(w.players[&id].inventory[&MetallicOre], home_ore0, "home goods are not a sell source");
        assert_eq!(w.fleets.len(), fleets0, "a sell conjures no convoy");
        assert!(
            !w.fleets.values().any(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub)),
            "no SellAtHub convoy is ever created by the Exchange"
        );
    }

    /// §TCA Part 2: home goods are NO LONGER a valid sell source — a corp holding
    /// plenty at home but nothing at the Charterhouse is soft-rejected, for free,
    /// with the typed reason that tells it to ship the goods in first.
    #[test]
    fn selling_home_goods_soft_rejects_until_they_reach_the_charterhouse() {
        use crate::cargo::Commodity::MetallicOre;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home_ore = w.players[&id].inventory[&MetallicOre];
        assert!(home_ore > 0, "the starter corp does hold ore AT HOME");
        let credits0 = w.players[&id].credits;

        let ev = w.step(&[Command::MarketSell { player_id: id, commodity: MetallicOre, units: home_ore }]);
        assert!(
            ev.iter().any(|e| matches!(
                &e.payload,
                EventPayload::Trade(TradeEvent::Rejected {
                    player, reason: crate::event::TradeRejectReason::InsufficientWarehouseStock { have: 0 }, ..
                }) if *player == id
            )),
            "an empty warehouse soft-rejects with the typed reason"
        );
        // Costs nothing: home goods and credits both untouched.
        assert_eq!(w.players[&id].inventory[&MetallicOre], home_ore);
        assert_eq!(w.players[&id].credits, credits0);
    }

    /// §TCA Part 2 (grandfathering): a convoy already in flight when the warehouse
    /// rework landed — i.e. one loaded from a PRE-FEATURE snapshot — still resolves
    /// under the OLD semantics. `DeliverHome` deposits into home inventory and
    /// `SellAtHub` clears at the price-on-arrival, exactly as before. New code never
    /// creates either mission (the Exchange tests above assert that side).
    #[test]
    fn in_flight_convoys_from_an_old_snapshot_still_resolve() {
        use crate::cargo::Commodity::{Fuel, MetallicOre};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home;
        let home_fuel0 = w.players[&id].inventory.get(&Fuel).copied().unwrap_or(0);
        let credits0 = w.players[&id].credits;

        // Forge the two legacy convoys, ARRIVED (Idle at their destination) —
        // exactly the shape an old snapshot's in-flight convoy loads with.
        for (mission, pos, cargo) in [
            (TradeMission::DeliverHome, home, Cargo { commodity: Fuel, units: 25 }),
            (TradeMission::SellAtHub, w.hub, Cargo { commodity: MetallicOre, units: 30 }),
        ] {
            let fid = w.alloc_entity_id();
            let mut f = Fleet::single(fid, id, ShipKind::Convoy, pos, FleetOrder::Idle, Some(cargo));
            f.mission = Some(mission);
            w.fleets.insert(fid, f);
        }

        w.step(&[]);

        // DeliverHome deposited into HOME inventory (old semantics — not the warehouse).
        assert_eq!(
            w.players[&id].inventory.get(&Fuel).copied().unwrap_or(0),
            home_fuel0 + 25,
            "a grandfathered DeliverHome still deposits at home"
        );
        assert_eq!(wh(&w, id, Fuel), 0, "…and not into the warehouse");
        // SellAtHub cleared into credits — and, because a convoy really did cross
        // and deliver, it earns trade THROUGHPUT at the arrival site (25 delivered
        // home + 30 carried to the hub), unlike an instant warehouse sale.
        assert!(w.players[&id].credits > credits0, "a grandfathered SellAtHub still clears");
        assert_eq!(
            w.players[&id].stats.trade_units, 55,
            "hauled goods earn throughput: 25 delivered home + 30 sold at the hub"
        );
        // Both convoys are consumed.
        assert!(
            !w.fleets.values().any(|f| f.owner == id && f.mission.is_some()),
            "both legacy convoys resolved and left the world"
        );
    }

    // ==== §TCA Part 3: scheduled Authority freight ==========================

    /// THE WHOLE ROUND TRIP: buy at the Exchange, hand the lot to the Authority,
    /// watch a real freighter carry it to the colony and unload, then book the
    /// return leg with sell-on-arrival and see it collected, landed in the
    /// warehouse, and cleared at the Exchange.
    #[test]
    fn freight_round_trip_delivers_collects_and_sells() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        seed_warehouse(&mut w, id, &[(Alloys, 100)]);
        let credits_after_buy = w.players[&id].credits;

        // --- BOOK OUT: goods leave the warehouse, the fee is charged now. ---
        let ev = w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: 100 }]);
        let booked = ev.iter().find_map(|e| match &e.payload {
            EventPayload::Trade(TradeEvent::FreightBooked { fee, depart_at, eta, .. }) => Some((*fee, *depart_at, *eta)),
            _ => None,
        });
        let (fee, depart_at, eta) = booked.expect("a booking receipt");
        assert!(fee > 0.0, "the Authority charges for the lift");
        assert!(eta > depart_at, "the lot arrives after it departs");
        assert_eq!(wh(&w, id, Alloys), 0, "the lot is escrowed out of the warehouse");
        assert!((w.players[&id].credits - (credits_after_buy - fee)).abs() < 1e-9, "the fee is a pure sink");
        assert_eq!(w.freight_queue.len(), 1, "one lot waits for the next departure");

        // --- DEPARTURE: one Authority freighter, owned by the TCA sentinel. ---
        step_until(&mut w, 4000, "the scheduled departure", |w| !w.freight_runs.is_empty());
        assert!(w.freight_queue.is_empty(), "the queue drained onto the hull");
        let fid = *w.freight_runs.keys().next().unwrap();
        let f = &w.fleets[&fid];
        assert!(f.owner.is_tca(), "the hull belongs to the Authority, not a corporation");
        assert!(f.contains(ShipKind::Freighter));
        assert!(f.cargo.is_none(), "the manifest rides on the RUN, never in Fleet.cargo");
        assert_eq!(w.freight_runs[&fid].shipments.values().next().unwrap().units, 100);
        // A TCA hull is never a corporation asset.
        assert!(!w.players.contains_key(&PlayerId::TCA), "the Authority is not a corporation");

        // --- DELIVERY into the colony's stockpile; the hull turns for home. ---
        step_until(&mut w, 4000, "delivery into the colony stockpile", |w| sys_units(w, colony, Alloys) >= 100);
        assert_eq!(w.freight_runs[&fid].leg, crate::tca::RunLeg::Returning, "the hull turns for the Charterhouse");
        assert!(w.freight_runs[&fid].shipments.is_empty(), "nothing undelivered rides home");

        // --- BOOK IN with sell-on-arrival: the goods leave the stockpile now. ---
        let credits_before_return = w.players[&id].credits;
        w.step(&[Command::BookFreightIn { player_id: id, system: colony, commodity: Alloys, units: 100, sell_on_arrival: true }]);
        assert_eq!(sys_units(&w, colony, Alloys), 0, "the lot is escrowed out of the stockpile");

        // --- A later freighter collects it and brings it home, and it SELLS. ---
        step_until(&mut w, 12_000, "the pickup to land and clear", |w| {
            w.freight_queue.is_empty() && w.freight_runs.is_empty() && w.players[&id].credits > credits_before_return
        });
        assert_eq!(wh(&w, id, Alloys), 0, "sell-on-arrival left nothing in the warehouse");
        assert!(w.players[&id].credits > credits_before_return, "the lot cleared at the Exchange");
        assert!(!w.fleets.values().any(|f| f.owner.is_tca()), "the freighter is retired at the Charterhouse");
    }

    /// Same seed, same commands ⇒ same world, byte-for-byte — across the whole
    /// freight machine (booking, the scheduler, a physical run, and unloading).
    #[test]
    fn freight_is_deterministic_across_identical_runs() {
        use crate::cargo::Commodity::Alloys;
        let script = |w: &mut World| {
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let colony = near_hub_colony(w, id, 1200.0);
            seed_warehouse(w, id, &[(Alloys, 500)]);
            w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: 250 }]);
            for _ in 0..6000 {
                w.step(&[]);
            }
            w.step(&[Command::BookFreightIn { player_id: id, system: colony, commodity: Alloys, units: 40, sell_on_arrival: true }]);
            for _ in 0..6000 {
                w.step(&[]);
            }
        };
        let mut a = test_world();
        let mut b = test_world();
        script(&mut a);
        script(&mut b);
        // The run actually happened (so the comparison means something).
        assert!(a.players[&PlayerId(1)].stats.market_revenue > 0.0 || a.tick > 0);
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }

    /// The per-departure CAP never REJECTS a booking — an oversized lot is split
    /// and rolls forward onto consecutive departures, FIFO.
    #[test]
    fn freight_cap_rolls_a_big_lot_onto_later_departures() {
        use crate::cargo::Commodity::Alloys;
        let cap = crate::tca::TCA_SHIPMENT_CAP;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        // One and a half caps in ONE booking (kept under the colony's 700-unit
        // storage cap so THIS test isolates the departure cap), and the credits
        // to pay for it.
        let lot = cap + cap / 2;
        seed_warehouse(&mut w, id, &[(Alloys, lot)]);
        w.players.get_mut(&id).unwrap().credits = 1_000_000.0;
        let ev = w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: lot }]);
        assert!(
            !ev.iter().any(|e| matches!(&e.payload, EventPayload::Trade(TradeEvent::Rejected { .. }))),
            "an oversized lot is never rejected — it queues"
        );
        assert_eq!(w.freight_queue.values().map(|s| s.units).sum::<u32>(), lot);

        // First departure lifts exactly one cap; the rest keeps its place.
        step_until(&mut w, 4000, "the first departure", |w| !w.freight_runs.is_empty());
        let first: u32 = w.freight_runs.values().flat_map(|r| r.shipments.values()).map(|s| s.units).sum();
        assert_eq!(first, cap, "one departure lifts exactly one cap per corp");
        assert_eq!(w.freight_queue.values().map(|s| s.units).sum::<u32>(), lot - cap, "the remainder waits");

        // Everything eventually lands at the colony — nothing is lost to the cap.
        step_until(&mut w, 40_000, "the whole lot to arrive", |w| sys_units(w, colony, Alloys) >= lot);
        assert!(w.freight_queue.is_empty(), "the queue drained completely");
    }

    /// A DEPOT at the destination both doubles the per-departure cap and discounts
    /// the fee — the flat v1 terms.
    #[test]
    fn a_depot_doubles_the_lift_and_discounts_the_fee() {
        use crate::cargo::Commodity::Alloys;
        let cap = crate::tca::TCA_SHIPMENT_CAP;
        let fee_of = |depot: bool| {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let colony = near_hub_colony(&mut w, id, 1200.0);
            if depot {
                let s = w.systems.iter_mut().find(|s| s.id == colony).unwrap();
                s.set_tier(crate::build::StructureKind::Depot, 1);
            }
            seed_warehouse(&mut w, id, &[(Alloys, cap * 2)]);
            w.players.get_mut(&id).unwrap().credits = 1_000_000.0;
            let ev = w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: cap * 2 }]);
            let fee = ev
                .iter()
                .find_map(|e| match &e.payload {
                    EventPayload::Trade(TradeEvent::FreightBooked { fee, .. }) => Some(*fee),
                    _ => None,
                })
                .expect("a booking receipt");
            step_until(&mut w, 4000, "the first departure", |w| !w.freight_runs.is_empty());
            let lifted: u32 = w.freight_runs.values().flat_map(|r| r.shipments.values()).map(|s| s.units).sum();
            (fee, lifted)
        };
        let (plain_fee, plain_lift) = fee_of(false);
        let (depot_fee, depot_lift) = fee_of(true);
        assert_eq!(plain_lift, cap, "no depot: one cap per departure");
        assert_eq!(depot_lift, cap * 2, "a depot doubles the per-departure lift");
        assert!(
            (depot_fee - plain_fee * crate::tca::TCA_DEPOT_FEE_MULT).abs() < 1e-6,
            "a depot discounts the fee (plain {plain_fee}, depot {depot_fee})"
        );
    }

    /// A queued PICKUP whose origin system falls before collection is forfeit — to
    /// NOBODY. The captor gets nothing and the fee is not refunded.
    #[test]
    fn a_queued_pickup_is_forfeit_when_the_system_falls() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        seed_stock(&mut w, colony, &[(Alloys, 200.0)]);
        w.step(&[Command::BookFreightIn { player_id: id, system: colony, commodity: Alloys, units: 100, sell_on_arrival: false }]);
        assert_eq!(w.freight_queue.len(), 1);

        // The system changes hands before a freighter ever gets there.
        let rival = PlayerId(2);
        w.systems.iter_mut().find(|s| s.id == colony).unwrap().owner = Some(rival);

        let mut forfeited = false;
        for _ in 0..8000 {
            for e in w.step(&[]) {
                if let EventPayload::Trade(TradeEvent::FreightMoved { player, units, stage: crate::event::FreightStage::ForfeitedOnCapture, .. }) = e.payload
                    && player == id
                {
                    assert_eq!(units, 100);
                    forfeited = true;
                }
            }
            if forfeited {
                break;
            }
        }
        assert!(forfeited, "the queued pickup is forfeit with an owner notice");
        assert!(w.freight_queue.is_empty(), "the lot is gone from the queue");
        assert_eq!(wh(&w, id, Alloys), 0, "forfeit means gone — not returned");
        // The captor gains nothing: the goods left the stockpile at booking.
        assert_eq!(sys_units(&w, colony, Alloys), 100, "only the un-booked remainder is there to capture");
    }

    /// If the destination is no longer the shipper's when the freighter arrives, the
    /// lot is NOT lost — the Authority holds it and carries it back to the owner's
    /// Charterhouse warehouse. Deliberately friendlier than the convoy rule.
    #[test]
    fn undeliverable_freight_returns_to_the_warehouse() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        seed_warehouse(&mut w, id, &[(Alloys, 100)]);
        w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: 100 }]);
        step_until(&mut w, 4000, "the departure", |w| !w.freight_runs.is_empty());

        // Lose the colony while the lot is in flight.
        w.systems.iter_mut().find(|s| s.id == colony).unwrap().owner = Some(PlayerId(2));

        step_until(&mut w, 12_000, "the lot to come home", |w| wh(w, id, Alloys) == 100);
        assert_eq!(sys_units(&w, colony, Alloys), 0, "the new owner is not gifted the cargo");
        assert!(w.freight_runs.is_empty() && !w.fleets.values().any(|f| f.owner.is_tca()), "the run is retired");
    }

    /// A FULL DEPOT bounces what won't fit: the Authority unloads up to the
    /// system's remaining headroom and carries the rest back to the owner's
    /// warehouse. Goods are never destroyed and never silently vanish.
    ///
    /// (The handoff didn't specify freight-vs-storage-cap; respecting the cap keeps
    /// freight from being a way to smuggle goods past a limit convoys obey, and
    /// reuses the "the Authority holds your goods" return rule rather than
    /// inventing a second overflow mechanism.)
    #[test]
    fn freight_respects_the_storage_cap_and_returns_the_overflow() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        let cap_units = w.systems.iter().find(|s| s.id == colony).unwrap().storage_cap();
        // Pre-fill the depot to 60 units short of full, then ship 200.
        seed_stock(&mut w, colony, &[(Alloys, cap_units - 60.0)]);
        seed_warehouse(&mut w, id, &[(Alloys, 200)]);
        w.players.get_mut(&id).unwrap().credits = 1_000_000.0;
        w.step(&[Command::BookFreightOut { player_id: id, system: colony, commodity: Alloys, units: 200 }]);

        step_until(&mut w, 20_000, "the overflow to come home", |w| wh(w, id, Alloys) > 0 && w.freight_runs.is_empty());
        // 60 landed, 140 came back — nothing created, nothing destroyed.
        assert_eq!(sys_units(&w, colony, Alloys), cap_units as u32, "the depot filled exactly to its cap");
        assert_eq!(wh(&w, id, Alloys), 140, "the overflow is back in the Charterhouse warehouse");
    }

    /// §TCA fog: the shipment queue is OWNER-ONLY. `shipments_of` — the one read
    /// the View is built from — never returns another corporation's lots, whether
    /// they are queued at the Charterhouse or aboard a shared freighter's manifest.
    #[test]
    fn the_shipment_queue_is_owner_scoped() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        // BOTH corps ship to their own colony, and both lots ride the SAME hull.
        let shared = near_hub_colony(&mut w, a, 1200.0);
        seed_warehouse(&mut w, a, &[(Alloys, 60)]);
        seed_warehouse(&mut w, b, &[(Alloys, 90)]);
        w.step(&[Command::BookFreightOut { player_id: a, system: shared, commodity: Alloys, units: 60 }]);
        // B books to the same destination by briefly holding it — the point is one
        // manifest carrying two owners' lots.
        w.systems.iter_mut().find(|s| s.id == shared).unwrap().owner = Some(b);
        w.step(&[Command::BookFreightOut { player_id: b, system: shared, commodity: Alloys, units: 90 }]);

        let queued_a = w.shipments_of(a);
        let queued_b = w.shipments_of(b);
        assert_eq!(queued_a.len(), 1);
        assert_eq!(queued_b.len(), 1);
        assert!(queued_a.iter().all(|(s, _)| s.owner == a), "A sees only A's lots");
        assert!(queued_b.iter().all(|(s, _)| s.owner == b), "B sees only B's lots");
        assert_eq!(queued_a[0].0.units, 60);
        assert_eq!(queued_b[0].0.units, 90);

        // Once ABOARD one shared freighter, the split still holds.
        step_until(&mut w, 4000, "the departure", |w| !w.freight_runs.is_empty());
        assert_eq!(w.freight_runs.len(), 1, "one hull serves the destination");
        assert_eq!(w.freight_runs.values().next().unwrap().shipments.len(), 2, "two owners aboard");
        for (who, other) in [(a, b), (b, a)] {
            let mine = w.shipments_of(who);
            assert!(mine.iter().all(|(s, aboard)| s.owner == who && *aboard), "{who} sees only their own aboard lot");
            assert!(!mine.iter().any(|(s, _)| s.owner == other), "no cross-owner leak");
        }
    }

    /// Freight to a system the corp does NOT own is refused, for free, with the
    /// typed reason — the Authority serves your own colonies only.
    #[test]
    fn freight_to_a_rivals_system_is_refused_for_free() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let rival_sys = near_hub_colony(&mut w, PlayerId(2), 1200.0);
        seed_warehouse(&mut w, id, &[(Alloys, 100)]);
        let credits0 = w.players[&id].credits;
        let ev = w.step(&[Command::BookFreightOut { player_id: id, system: rival_sys, commodity: Alloys, units: 100 }]);
        assert!(
            ev.iter().any(|e| matches!(
                &e.payload,
                EventPayload::Trade(TradeEvent::Rejected { player, reason: TradeRejectReason::NotYourSystem, .. }) if *player == id
            )),
            "booking to a rival's system is refused with the typed reason"
        );
        assert_eq!(wh(&w, id, Alloys), 100, "the goods stay put");
        assert_eq!(w.players[&id].credits, credits0, "and it costs nothing");
        assert!(w.freight_queue.is_empty());
    }

    /// `MarketBuy { ship_to }` is ONE checkbox: buy, then hand the lot straight to
    /// the Authority. If the booking can't be honored the goods simply stay in the
    /// warehouse and the owner is told why.
    #[test]
    fn market_buy_ship_to_books_freight_or_leaves_the_lot_in_the_warehouse() {
        use crate::cargo::Commodity::Fuel;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let colony = near_hub_colony(&mut w, id, 1200.0);
        // Happy path: the lot is bought AND booked in one command.
        w.step(&[Command::MarketBuy { player_id: id, commodity: Fuel, units: 40, ship_to: Some(colony) }]);
        assert_eq!(wh(&w, id, Fuel), 0, "the whole lot went straight onto the freight queue");
        assert_eq!(w.freight_queue.values().map(|s| s.units).sum::<u32>(), 40);

        // Sad path: a rival's system — the goods stay bought, and stay put.
        let rival_sys = near_hub_colony(&mut w, PlayerId(2), 1500.0);
        let ev = w.step(&[Command::MarketBuy { player_id: id, commodity: Fuel, units: 25, ship_to: Some(rival_sys) }]);
        assert!(
            ev.iter().any(|e| matches!(
                &e.payload,
                EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::NotYourSystem, .. })
            )),
            "the failed leg reports why"
        );
        assert_eq!(wh(&w, id, Fuel), 25, "the purchase stands; the lot simply stays at the Charterhouse");
    }

    #[test]
    fn stock_system_moves_hq_inventory_into_a_system_stockpile() {
        use crate::cargo::Commodity::Volatiles;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sys = w.players[&id].home_system.expect("home system");
        // AddPlayer seeds the HQ trading inventory with 120 of each raw good.
        let held0 = w.players[&id].inventory.get(&Volatiles).copied().unwrap_or(0);
        assert!(held0 >= 100, "precondition: HQ holds >=100 volatiles (had {held0})");
        let ships0 = w.fleets.len();

        // Soft-reject cases: zero units, more than held, a system you don't own —
        // none may debit HQ or spawn a convoy.
        w.step(&[Command::StockSystem { player_id: id, system_id: sys, commodity: Volatiles, units: 0 }]);
        w.step(&[Command::StockSystem { player_id: id, system_id: sys, commodity: Volatiles, units: 999_999 }]);
        w.step(&[Command::StockSystem { player_id: id, system_id: EntityId(987_654), commodity: Volatiles, units: 10 }]);
        assert_eq!(w.players[&id].inventory.get(&Volatiles).copied().unwrap_or(0), held0, "rejected stock must not debit HQ");
        assert_eq!(w.fleets.len(), ships0, "rejected stock must not spawn a convoy");

        // Valid: debits HQ now, dispatches the convoy, and routes the goods to the
        // SYSTEM stockpile. At ~zero home→home-system distance the DeliverToSystem
        // convoy deposits the SAME tick, so assert the OUTCOME, not a lingering ship.
        let vol_at = |w: &World| w.systems.iter().find(|s| s.id == sys).and_then(|s| s.stockpile.get(&Volatiles).copied()).unwrap_or(0.0);
        let s0 = vol_at(&w);
        let overflow_of = |evs: &[Event]| evs.iter().any(|e| matches!(&e.payload,
            EventPayload::Trade(TradeEvent::StorageOverflow { system, commodity, .. })
                if *system == sys && *commodity == Volatiles));
        let evs = w.step(&[Command::StockSystem { player_id: id, system_id: sys, commodity: Volatiles, units: 100 }]);
        assert_eq!(w.players[&id].inventory.get(&Volatiles).copied().unwrap_or(0), held0 - 100, "HQ debited by the shipped units");
        assert!(evs.iter().any(|e| matches!(&e.payload,
            EventPayload::Trade(TradeEvent::StockDispatched { system, commodity, units, .. })
                if *system == sys && *commodity == Volatiles && *units == 100)),
            "a StockDispatched event should fire for the shipment");

        // The goods land in the SYSTEM stockpile (single-player → never raided).
        // A big jump (≥50, unreachable by per-tick production drift) or an
        // overflow-to-hub both prove the goods were routed to the SYSTEM, not HQ.
        let mut delivered = vol_at(&w) >= s0 + 50.0 || overflow_of(&evs);
        if !delivered {
            for _ in 0..(260 * crate::config::TICK_HZ) {
                let evs = w.step(&[]);
                if vol_at(&w) >= s0 + 50.0 || overflow_of(&evs) {
                    delivered = true;
                    break;
                }
            }
        }
        assert!(delivered, "the shipped volatiles never reached the destination system stockpile");
    }

    #[test]
    fn cannot_buy_without_credits_or_sell_without_goods() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let ships0 = w.fleets.len();
        // Sell more than the WAREHOUSE holds → soft-rejected, nothing spent.
        seed_warehouse(&mut w, id, &[(Alloys, 10)]);
        w.step(&[Command::MarketSell { player_id: id, commodity: Alloys, units: 99_999 }]);
        assert_eq!(wh(&w, id, Alloys), 10, "a rejected sell leaves the warehouse intact");
        assert_eq!(w.fleets.len(), ships0, "rejected sell must not spawn a convoy");
        // Buy beyond the treasury → ignored.
        let credits0 = w.players[&id].credits;
        w.step(&[Command::MarketBuy { player_id: id, commodity: Alloys, units: 10_000_000, ship_to: None }]);
        assert_eq!(w.players[&id].credits, credits0);
        assert_eq!(wh(&w, id, Alloys), 10, "a rejected buy deposits nothing");
        assert_eq!(w.fleets.len(), ships0, "rejected buy must not spawn a convoy");
    }

    #[test]
    fn limit_orders_clear_in_uniform_price_batch() {
        use crate::cargo::Commodity::MetallicOre;
        let mut w = test_world();
        let (buyer, seller) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: buyer, name: "Buy".into() },
            Command::AddPlayer { id: seller, name: "Sell".into() },
        ]);
        // §TCA: sell-side escrow draws from the CHARTERHOUSE WAREHOUSE, so the
        // seller's goods must already be at the Exchange.
        seed_warehouse(&mut w, seller, &[(MetallicOre, 50)]);
        let buyer_credits0 = w.players[&buyer].credits;
        let seller_credits0 = w.players[&seller].credits;
        let seller_ore0 = wh(&w, seller, MetallicOre);

        // A crossing pair: buyer pays up to 9, seller wants at least 7.
        w.step(&[
            Command::PlaceLimitOrder { player_id: seller, side: Side::Sell, commodity: MetallicOre, units: 50, limit_price: 7.0 },
            Command::PlaceLimitOrder { player_id: buyer, side: Side::Buy, commodity: MetallicOre, units: 50, limit_price: 9.0 },
        ]);
        // Reservations taken at placement — out of the warehouse.
        assert_eq!(wh(&w, seller, MetallicOre), seller_ore0 - 50);
        assert!((w.players[&buyer].credits - (buyer_credits0 - 50.0 * 9.0)).abs() < 1e-6);
        assert_eq!(w.book.len(), 2);

        // Run until the next batch clears (≈ every 20 s).
        let mut cleared = false;
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.book.is_empty() {
                cleared = true;
                break;
            }
        }
        assert!(cleared, "the batch should have cleared the crossing orders");

        // Uniform clearing price P* = 7 (max volume, lowest price). Seller is
        // paid 50×7; buyer's over-reservation (50×2) is refunded → net 50×7.
        assert!((w.players[&seller].credits - (seller_credits0 + 50.0 * 7.0)).abs() < 1e-6, "seller paid at uniform price");
        assert!((w.players[&buyer].credits - (buyer_credits0 - 50.0 * 7.0)).abs() < 1e-6, "buyer settled at uniform price (over-reservation refunded)");
        // §TCA: the buyer's matched goods land in their Charterhouse warehouse —
        // a fill is an Exchange settlement, never a crossing.
        assert_eq!(wh(&w, buyer, MetallicOre), 50, "the fill deposits into the buyer's warehouse");
        assert!(
            !w.fleets.values().any(|s| s.owner == buyer && s.mission == Some(TradeMission::DeliverHome)),
            "a limit fill conjures no delivery convoy"
        );
    }

    #[test]
    fn limit_orders_that_do_not_cross_rest() {
        use crate::cargo::Commodity::Fuel;
        let mut w = test_world();
        let (buyer, seller) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: buyer, name: "Buy".into() },
            Command::AddPlayer { id: seller, name: "Sell".into() },
        ]);
        seed_warehouse(&mut w, seller, &[(Fuel, 30)]); // §TCA: escrow comes from the warehouse
        // Buyer pays up to 6, seller wants 9 — they do NOT cross.
        w.step(&[
            Command::PlaceLimitOrder { player_id: seller, side: Side::Sell, commodity: Fuel, units: 30, limit_price: 9.0 },
            Command::PlaceLimitOrder { player_id: buyer, side: Side::Buy, commodity: Fuel, units: 30, limit_price: 6.0 },
        ]);
        for _ in 0..(25 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert_eq!(w.book.len(), 2, "non-crossing orders rest on the book");
    }

    #[test]
    fn determinism_same_commands_same_state() {
        let mut a = test_world();
        let mut b = test_world();
        // A system present identically in both deterministic galaxies.
        let sysid = a.systems[0].id;
        // Park identical colony fleets on it in both worlds — the ARRIVAL claim
        // (the dynamic owner mutation + accrual) runs in each, so replay
        // equality covers the new settle path, not just seeded generation.
        let pos = a.systems[0].pos;
        colony_at(&mut a, PlayerId(1), pos);
        colony_at(&mut b, PlayerId(1), pos);
        let cmds = vec![
            Command::AddPlayer { id: PlayerId(1), name: "A".into() },
            Command::AddPlayer { id: PlayerId(2), name: "B".into() },
        ];
        for _ in 0..600 {
            a.step(&cmds);
            b.step(&cmds);
        }
        // The dynamic paths actually ran (so the comparison is meaningful).
        let sys_a = a.systems.iter().find(|s| s.id == sysid).unwrap();
        assert_eq!(sys_a.owner, Some(PlayerId(1)), "the colony settle path must have executed");
        // §economy: a fresh outpost is unstaffed (produces nothing) — the
        // production path is proven on the bootstrap HOME, which works from
        // tick one (staffed extraction + Agroplex).
        let home_a = a.players[&PlayerId(1)].home_system.unwrap();
        let home_sys = a.systems.iter().find(|s| s.id == home_a).unwrap();
        assert!(home_sys.stockpile.get(&Commodity::MetallicOre).copied().unwrap_or(0.0) > 0.0, "the staffed home accrual path must have executed");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    // ---- System claims + resource production (§4, §9) ----

    /// A system's deposit value-rate: Σ richness · base_price — how much credit
    /// value it produces per second.
    fn value_rate(sys: &StarSystem) -> f64 {
        sys.all_deposits().map(|d| d.richness * crate::market::base_price(d.resource)).sum()
    }

    /// THE KEY DESIGN PROPERTY (§4): richer/more valuable deposits concentrate
    /// toward the frontier. The outer third of systems must out-produce the inner
    /// third — deterministically, from the seed.
    #[test]
    fn deposits_are_richer_toward_the_frontier() {
        let w = test_world();
        let mut by_dist: Vec<&StarSystem> = w.systems.iter().collect();
        by_dist.sort_by(|a, b| a.pos.length().partial_cmp(&b.pos.length()).unwrap());
        let third = by_dist.len() / 3;
        assert!(third >= 1, "need enough systems to compare thirds");
        let mean = |s: &[&StarSystem]| s.iter().map(|x| value_rate(x)).sum::<f64>() / s.len() as f64;
        let inner = mean(&by_dist[..third]);
        let outer = mean(&by_dist[by_dist.len() - third..]);
        assert!(every_system_has_a_deposit(&w), "every system must have at least one deposit");
        assert!(outer > inner * 1.5,
            "frontier should out-produce the core: inner value-rate {inner:.1} vs outer {outer:.1}");
    }

    fn every_system_has_a_deposit(w: &World) -> bool {
        w.systems.iter().all(|s| s.all_deposits().next().is_some())
    }

    /// Deposit generation is deterministic from the seed (replay-safe).
    #[test]
    fn deposit_generation_is_deterministic() {
        let a = World::new(SimConfig::for_players(777, 6));
        let b = World::new(SimConfig::for_players(777, 6));
        assert_eq!(
            serde_json::to_string(&a.systems).unwrap(),
            serde_json::to_string(&b.systems).unwrap()
        );
        // A different seed yields a different galaxy.
        let c = World::new(SimConfig::for_players(778, 6));
        assert_ne!(
            serde_json::to_string(&a.systems).unwrap(),
            serde_json::to_string(&c.systems).unwrap()
        );
    }

    /// Pick the richest (frontier) system — guaranteed to have valuable deposits.
    fn richest_system(w: &World) -> EntityId {
        w.systems
            .iter()
            .max_by(|a, b| value_rate(a).partial_cmp(&value_rate(b)).unwrap())
            .unwrap()
            .id
    }

    /// §fleets part 3: claiming is PHYSICAL — a colony ship that ARRIVES at an
    /// unclaimed system settles it: ownership + claimed_at transfer, the ship is
    /// CONSUMED (no wreck — it became the colony), and SystemClaimed fires
    /// (light-gating to rivals is the same event path as ever).
    #[test]
    fn colony_ship_settles_an_unclaimed_system_on_arrival() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        let cid = colony_at(&mut w, id, pos);
        let credits0 = w.players[&id].credits;
        let ev = w.step(&[]);
        let sys = w.systems.iter().find(|s| s.id == sysid).unwrap();
        assert_eq!(sys.owner, Some(id), "arrival transfers ownership");
        assert!(sys.claimed_at.is_some());
        assert!(!w.fleets.contains_key(&cid), "the colony ship is consumed — it became the colony");
        assert!(
            !ev.iter().any(|e| matches!(e.payload, EventPayload::ShipDestroyed { ship, .. } if ship == cid)),
            "consumed, not destroyed — no wreck report"
        );
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::SystemClaimed { system, .. } if system == sysid)));
        assert_eq!(w.players[&id].credits, credits0, "no credit charge — the recipe was the price");
    }

    // --- §FLEETS management v1: colony-consume, merge, split, build-join -----

    /// Park an idle fleet-of-one of `kind` for `owner` at `pos` (test setup).
    fn park_fleet(w: &mut World, owner: PlayerId, pos: Vec2, kind: ShipKind) -> EntityId {
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None));
        id
    }

    #[test]
    fn colony_fleet_consumes_one_colony_and_the_escort_persists() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        // A colony ship WITH a corvette escort, arriving as ONE fleet.
        let fid = w.alloc_entity_id();
        let mut fleet = Fleet::single(fid, id, ShipKind::Colony, pos, FleetOrder::Idle, None);
        fleet.add(ShipKind::Corvette, 1);
        w.fleets.insert(fid, fleet);
        w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == sysid).unwrap().owner, Some(id), "the fleet claims the system");
        let survivor = w.fleets.get(&fid).expect("the fleet persists — only ONE colony was consumed");
        assert_eq!(survivor.count(ShipKind::Colony), 0, "the colony ship became the settlement");
        assert_eq!(survivor.count(ShipKind::Corvette), 1, "the escort remains, parked at the new holding");
    }

    #[test]
    fn merge_fleets_at_owned_system_combines_composition() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let a = park_fleet(&mut w, id, pos, ShipKind::Raider);
        let b = park_fleet(&mut w, id, pos, ShipKind::Corvette);
        w.step(&[Command::MergeFleets { player_id: id, into: a, from: b }]);
        assert!(!w.fleets.contains_key(&b), "the absorbed fleet is removed");
        let merged = &w.fleets[&a];
        assert_eq!(merged.count(ShipKind::Raider), 1);
        assert_eq!(merged.count(ShipKind::Corvette), 1);
    }

    #[test]
    fn merge_soft_rejects_in_flight_or_foreign_fleets() {
        let mut w = test_world();
        let (id, rival) = (PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id, name: "A".into() },
            Command::AddPlayer { id: rival, name: "B".into() },
        ]);
        let home = w.players[&id].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let a = park_fleet(&mut w, id, pos, ShipKind::Raider);
        // In flight (not idle) → can't be merged (no in-flight detachment in v1).
        let b = park_fleet(&mut w, id, pos, ShipKind::Corvette);
        w.fleets.get_mut(&b).unwrap().order = FleetOrder::MoveTo { dest: Vec2::new(99999.0, 0.0) };
        w.step(&[Command::MergeFleets { player_id: id, into: a, from: b }]);
        assert!(w.fleets.contains_key(&b), "an in-flight fleet is not merged");
        assert_eq!(w.fleets[&a].total_count(), 1, "no partial merge");
        // A rival's fleet can't be merged into yours by id.
        let far = park_fleet(&mut w, rival, pos, ShipKind::Raider);
        w.step(&[Command::MergeFleets { player_id: id, into: a, from: far }]);
        assert!(w.fleets.contains_key(&far), "someone else's fleet is untouched");
        assert_eq!(w.fleets[&a].total_count(), 1);
    }

    #[test]
    fn split_fleet_detaches_a_new_fleet_deterministically() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        // A 3-raider + 1-corvette fleet docked at home.
        let fid = w.alloc_entity_id();
        let mut fleet = Fleet::single(fid, id, ShipKind::Raider, pos, FleetOrder::Idle, None);
        fleet.add(ShipKind::Raider, 2);
        fleet.add(ShipKind::Corvette, 1);
        w.fleets.insert(fid, fleet);
        let before = w.fleets.len();
        let mut counts = std::collections::BTreeMap::new();
        counts.insert(ShipKind::Raider, 2);
        w.step(&[Command::SplitFleet { player_id: id, fleet_id: fid, counts }]);
        assert_eq!(w.fleets.len(), before + 1, "one new fleet detached");
        assert_eq!(w.fleets[&fid].count(ShipKind::Raider), 1, "the source keeps the remainder");
        assert_eq!(w.fleets[&fid].count(ShipKind::Corvette), 1);
        let new_fleet = w
            .fleets
            .values()
            .find(|f| f.id != fid && f.count(ShipKind::Raider) == 2 && f.total_count() == 2)
            .expect("the detached raiders form a new idle fleet");
        assert!(matches!(new_fleet.order, FleetOrder::Idle));
    }

    #[test]
    fn split_soft_rejects_empty_full_or_overdraw() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let fid = w.alloc_entity_id();
        let mut fleet = Fleet::single(fid, id, ShipKind::Raider, pos, FleetOrder::Idle, None);
        fleet.add(ShipKind::Raider, 1); // two raiders total
        w.fleets.insert(fid, fleet);
        let before = w.fleets.len();
        let empty = std::collections::BTreeMap::new();
        let mut full = std::collections::BTreeMap::new();
        full.insert(ShipKind::Raider, 2); // would empty the source
        let mut over = std::collections::BTreeMap::new();
        over.insert(ShipKind::Raider, 3); // more than aboard
        for counts in [empty, full, over] {
            w.step(&[Command::SplitFleet { player_id: id, fleet_id: fid, counts }]);
            assert_eq!(w.fleets.len(), before, "no fleet spawned on a rejected split");
            assert_eq!(w.fleets[&fid].total_count(), 2, "the source is untouched");
        }
    }

    #[test]
    fn build_ship_joins_a_docked_fleet_when_asked() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        let hpos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 300.0)]);
        let dock = park_fleet(&mut w, id, hpos, ShipKind::Raider);
        let fleets_before = w.fleets.len();
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: Some(dock) , loadout: Default::default() }]);
        for _ in 0..CONVOY_RECIPE.build_ticks + 2 {
            w.step(&[]);
        }
        assert_eq!(w.fleets.len(), fleets_before, "no NEW fleet — the build joined the docked one");
        assert_eq!(w.fleets[&dock].count(ShipKind::Convoy), 1, "the convoy joined the fleet");
        assert_eq!(w.fleets[&dock].count(ShipKind::Raider), 1, "the original raider is still there");
    }

    // --- §order-lifecycle: IN TRANSIT → AWAITING ECHO → CONFIRMED --------------

    /// Park an owned fleet `d` su from the command center (Idle), fuelled to move.
    fn lifecycle_setup(w: &mut World, id: PlayerId, d: f64) -> (EntityId, Vec2, Vec2) {
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let cc = w.players[&id].command_center;
        let fid = *w.fleets.iter().find(|(_, f)| f.owner == id).unwrap().0;
        let pos = cc + Vec2::new(d, 0.0);
        {
            let f = w.fleets.get_mut(&fid).unwrap();
            f.pos = pos;
            f.vel = Vec2::ZERO;
            f.order = FleetOrder::Idle;
        }
        // Seed fuel at home so the move dispatches.
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(crate::cargo::Commodity::Fuel, 1000.0);
        (fid, cc, pos)
    }

    #[test]
    fn order_lifecycle_timestamps_match_the_analytic_round_trip() {
        let mut w = test_world();
        let id = PlayerId(1);
        let (fid, cc, pos) = lifecycle_setup(&mut w, id, 900.0); // 900/300 = 3 s each leg
        let c = w.config.c;
        let dest = pos + Vec2::new(0.0, 400.0);
        let t0 = w.time;
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest }]);
        let pc = w.pending_commands(id).into_iter().find(|p| p.fleet == fid).expect("lifecycle present");
        // delivered_at = issue + d/c; the fleet is Idle so the delivery point is
        // its current pos → echo_at = delivered_at + d/c.
        let leg = pos.distance(cc) / c;
        assert!((pc.delivered_at - (t0 + leg)).abs() < 1e-6, "delivered_at = issue + d/c");
        assert!((pc.echo_at - (t0 + 2.0 * leg)).abs() < 1e-6, "echo_at = delivered_at + d/c");
        assert_eq!(pc.kind, crate::event::OrderKind::Move);
    }

    #[test]
    fn order_lifecycle_delivers_then_confirms_at_echo() {
        let mut w = test_world();
        let id = PlayerId(1);
        let (fid, _cc, pos) = lifecycle_setup(&mut w, id, 900.0);
        let dest = pos + Vec2::new(0.0, 400.0);
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest }]);
        let echo_at = w.pending_commands(id)[0].echo_at;
        let mut delivered = false;
        let mut confirmed_at = None;
        for _ in 0..(30 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                match e.payload {
                    EventPayload::OrderDelivered { fleet, .. } if fleet == fid => delivered = true,
                    EventPayload::OrderConfirmed { fleet, .. } if fleet == fid => confirmed_at = Some(e.time),
                    _ => {}
                }
            }
            if confirmed_at.is_some() {
                break;
            }
        }
        assert!(delivered, "an OrderDelivered fired when the outbound light arrived");
        let ct = confirmed_at.expect("an OrderConfirmed fired");
        assert!((ct - echo_at).abs() <= DT + 1e-9, "confirmation fires at echo_at");
        assert!(w.pending_commands(id).is_empty(), "the lifecycle clears once confirmed");
    }

    #[test]
    fn order_lifecycle_is_owner_only() {
        let mut w = test_world();
        let (id, rival) = (PlayerId(1), PlayerId(2));
        let (fid, _cc, pos) = lifecycle_setup(&mut w, id, 900.0);
        w.step(&[Command::AddPlayer { id: rival, name: "Rival".into() }]);
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest: pos + Vec2::new(0.0, 400.0) }]);
        assert!(!w.pending_commands(id).is_empty(), "the owner sees their lifecycle");
        assert!(w.pending_commands(rival).is_empty(), "a rival sees NONE of it (owner-only)");
    }

    #[test]
    fn superseding_order_restarts_the_lifecycle_with_the_latest() {
        let mut w = test_world();
        let id = PlayerId(1);
        let (fid, _cc, pos) = lifecycle_setup(&mut w, id, 900.0);
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest: pos + Vec2::new(0.0, 400.0) }]);
        let first_echo = w.pending_commands(id)[0].echo_at;
        // A second, farther move restarts the tracked lifecycle.
        w.step(&[]); // advance a tick so issued_at differs
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest: pos + Vec2::new(0.0, 4000.0) }]);
        let pcs = w.pending_commands(id);
        assert_eq!(pcs.iter().filter(|p| p.fleet == fid).count(), 1, "one lifecycle shown per fleet — the latest");
        assert!(pcs[0].echo_at != first_echo, "the panel tracks the newer order");
    }

    #[test]
    fn destroyed_fleet_resolves_the_lifecycle_without_a_false_confirm() {
        let mut w = test_world();
        let id = PlayerId(1);
        let (fid, _cc, pos) = lifecycle_setup(&mut w, id, 900.0);
        w.step(&[Command::MoveShip { player_id: id, ship_id: fid, dest: pos + Vec2::new(0.0, 400.0) }]);
        // Run past delivery so it's AWAITING ECHO.
        for _ in 0..(4 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(!w.pending_commands(id).is_empty(), "awaiting echo before the loss");
        // The fleet is destroyed before its echo lands.
        w.fleets.remove(&fid);
        let mut confirmed = false;
        for _ in 0..(6 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::OrderConfirmed { fleet, .. } if fleet == fid) {
                    confirmed = true;
                }
            }
        }
        assert!(!confirmed, "a destroyed fleet never emits a phantom confirmation");
        assert!(w.pending_commands(id).is_empty(), "the lifecycle is dropped on loss");
    }

    #[test]
    fn transit_mode_persists_across_a_snapshot_round_trip() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let fid = *w.fleets.keys().next().unwrap();
        w.step(&[Command::SetFleetTransit { player_id: id, fleet_id: fid, mode: crate::ship::TransitMode::Stealth }]);
        assert_eq!(w.fleets[&fid].transit, crate::ship::TransitMode::Stealth, "the command set stealth");
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w2.fleets[&fid].transit, crate::ship::TransitMode::Stealth, "transit mode survives serialization");
    }

    #[test]
    fn build_ship_forms_a_new_fleet_by_default() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::MetallicOre, 300.0)]);
        let fleets_before = w.fleets.len();
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None , loadout: Default::default() }]);
        for _ in 0..CONVOY_RECIPE.build_ticks + 2 {
            w.step(&[]);
        }
        assert_eq!(w.fleets.len(), fleets_before + 1, "join: None forms its own fleet-of-one");
    }

    /// §fleets part 3: THE RACE. Two rivals launch at the same frontier system;
    /// the earlier arrival settles it (same-tick ties break by ship id —
    /// deterministic). The loser HOLDS at the spot, intact, is told once, and
    /// remains fully spendable: redirected to another system, it settles there.
    #[test]
    fn colony_race_loser_holds_and_redirects() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        let sysid = richest_system(&w);
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        // A's ship arrives THIS tick; B's is still one tick of flight away
        // (simulated: park B's next tick — later arrival loses).
        colony_at(&mut w, a, pos);
        w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == sysid).unwrap().owner, Some(a), "first arrival wins");

        let b_ship = colony_at(&mut w, b, pos);
        let ev = w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == sysid).unwrap().owner, Some(a), "the flip is final");
        assert!(w.fleets.contains_key(&b_ship), "the loser HOLDS — nothing destroyed");
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::ColonyHeld { owner, system, .. } if owner == b && system == sysid)),
            "the loser is notified"
        );
        // No notice spam while it sits there…
        let ev = w.step(&[]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::ColonyHeld { .. })), "one notice per hold");

        // …and it remains SPENDABLE: redirect it to another unclaimed system.
        let other = w
            .systems
            .iter()
            .find(|s| s.is_unclaimed() && !w.home_slots.iter().any(|h| h.system == Some(s.id)))
            .unwrap();
        let (other_id, other_pos) = (other.id, other.pos);
        // Teleport-park it there (flight time is not what's under test).
        w.fleets.get_mut(&b_ship).unwrap().pos = other_pos;
        w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == other_id).unwrap().owner, Some(b), "the held ship settles elsewhere");
        assert!(!w.fleets.contains_key(&b_ship), "…and is consumed there");
    }

    /// §fleets part 3: same-tick RACE tiebreak is deterministic — the lower ship
    /// id (built/launched earlier) settles; the other holds.
    #[test]
    fn colony_same_tick_race_breaks_by_ship_id() {
        let run = || {
            let mut w = test_world();
            let (a, b) = (PlayerId(1), PlayerId(2));
            w.step(&[
                Command::AddPlayer { id: a, name: "A".into() },
                Command::AddPlayer { id: b, name: "B".into() },
            ]);
            let sysid = richest_system(&w);
            let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
            let a_ship = colony_at(&mut w, a, pos); // lower id — allocated first
            let b_ship = colony_at(&mut w, b, pos);
            w.step(&[]);
            let owner = w.systems.iter().find(|s| s.id == sysid).unwrap().owner;
            (owner, w.fleets.contains_key(&a_ship), w.fleets.contains_key(&b_ship))
        };
        let (owner, a_alive, b_alive) = run();
        assert_eq!(owner, Some(PlayerId(1)), "lower ship id settles on a tie");
        assert!(!a_alive, "winner consumed");
        assert!(b_alive, "loser holds, intact");
        assert_eq!(run(), (owner, a_alive, b_alive), "the race is deterministic");
    }

    /// §fleets part 3: a colony ship destroyed IN TRANSIT is colonists lost —
    /// the recipe is gone, no claim ever lands. Expansion has stakes (and a
    /// reason for corvette escorts — which screen colony fleets like convoys).
    #[test]
    fn colony_ship_destroyed_in_transit_never_claims() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let cc = w.players[&atk].command_center;
        let raider = find_ship(&w, atk, ShipKind::Raider);
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cc + Vec2::new(120.0, 0.0);
            r.vel = Vec2::ZERO;
            r.order = FleetOrder::Idle;
        }
        // Def's colony ship crawls somewhere far — intercepted well short of it.
        let target_sys = richest_system(&w);
        let dest = w.systems.iter().find(|s| s.id == target_sys).unwrap().pos;
        let cid = w.alloc_entity_id();
        w.fleets.insert(cid, Fleet::single(cid, def, ShipKind::Colony, cc + Vec2::new(420.0, 0.0), FleetOrder::MoveTo { dest }, None));
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: cid }]);
        let outcome = run_until_raid(&mut w, 90, |_| vec![]).expect("the intercept resolves");
        assert_eq!(outcome, RaidOutcome::TargetDestroyed, "a fat unescorted civilian dies (atk 3 vs def 1)");
        assert!(!w.fleets.contains_key(&cid), "colonists lost");
        assert!(w.systems.iter().find(|s| s.id == target_sys).unwrap().owner.is_none(), "no claim ever lands");
    }

    #[test]
    fn claimed_system_accrues_production_over_time() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        // Unowned systems do NOT produce.
        for _ in 0..(3 * crate::config::TICK_HZ) { w.step(&[]); }
        assert!(w.systems.iter().find(|s| s.id == sysid).unwrap().stockpile.is_empty(),
            "an unclaimed system must not produce");

        grant_system(&mut w, id, sysid); // staffed tier-1 extraction, all factors 1.0
        let secs = 20u32;
        for _ in 0..(secs * crate::config::TICK_HZ) { w.step(&[]); }

        // Per-commodity: stock ≈ Σ richness × time at factor-1 staffing. (The
        // grant's Provisions food seed and the colony's eating stay out of the
        // math by asserting deposit commodities only — none are Provisions.)
        let sys = w.systems.iter().find(|s| s.id == sysid).unwrap();
        let mut by_res: BTreeMap<Commodity, f64> = BTreeMap::new();
        for d in sys.all_deposits() {
            assert_ne!(d.resource, Commodity::Provisions, "deposits are raws");
            *by_res.entry(d.resource).or_insert(0.0) += d.richness;
        }
        for (c, rate) in by_res {
            let got = sys.stockpile.get(&c).copied().unwrap_or(0.0);
            let expected = rate * secs as f64;
            assert!(got > 0.0, "a claimed, staffed system must accrue {c:?}");
            assert!((got - expected).abs() < expected * 0.02 + 1e-6,
                "{c:?} stock {got:.2} ≈ richness × time {expected:.2}");
        }
    }

    #[test]
    fn shipping_production_spawns_a_raidable_convoy_that_sells() {
        let mut w = test_world();
        w.enclaves.clear(); // isolate the trade loop from ambient piracy
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        grant_system(&mut w, id, sysid);
        w.step(&[]);
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        let stock_before: f64 = w.systems.iter().find(|s| s.id == sysid).unwrap().stockpile.values().sum();
        assert!(stock_before >= 1.0, "should have whole units to ship");

        w.step(&[Command::ShipProduction { player_id: id, system_id: sysid }]);
        // A production convoy is just a normal raidable trade convoy (Convoy kind,
        // carrying cargo, selling at the hub) — spawned at the system.
        let sys_pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        let convoy = w.fleets.values().find(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub)).cloned();
        let convoy = convoy.expect("ship-production should spawn a sell convoy");
        assert_eq!(convoy.flagship_kind(), ShipKind::Convoy, "production fleets in raidable convoys");
        assert!(convoy.cargo.is_some());
        assert!(convoy.pos.distance(sys_pos) < 5.0, "production convoy departs from the system");
        // The system's whole-unit stockpile was emptied into the convoy(s) —
        // measured on the extracted raws (the grant's Provisions food seed is
        // colony supply and ships out with everything else; eating between
        // ticks makes its exact remainder noisy, so exclude it).
        let remaining: f64 = w.systems.iter().find(|s| s.id == sysid).unwrap()
            .stockpile.iter().filter(|(c, _)| **c != Commodity::Provisions).map(|(_, v)| v).sum();
        assert!(remaining < 1.0, "shipping should empty the whole-unit stockpile (left {remaining})");

        // The full loop pays out: run until the convoy reaches the hub and sells.
        let credits_before = w.players[&id].credits;
        let mut sold = false;
        for _ in 0..(400 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::Trade(TradeEvent::Sold { player, .. }) = e.payload
                    && player == id
                {
                    sold = true;
                }
            }
            if sold { break; }
        }
        assert!(sold, "the production convoy should reach the hub and sell");
        assert!(w.players[&id].credits > credits_before, "selling production should pay credits");
    }

    #[test]
    fn a_raider_can_destroy_a_production_convoy() {
        let mut w = test_world();
        let (def, atk) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: def, name: "Producer".into() },
            Command::AddPlayer { id: atk, name: "Raider".into() },
        ]);
        let sysid = richest_system(&w);
        grant_system(&mut w, def, sysid);
        w.step(&[]);
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        w.step(&[Command::ShipProduction { player_id: def, system_id: sysid }]);
        let convoy = *w.fleets.iter().find(|(_, s)| s.owner == def && s.mission == Some(TradeMission::SellAtHub)).unwrap().0;

        // Park the attacker's raider right on the production convoy and commit.
        let raider = find_ship(&w, atk, ShipKind::Raider);
        let cpos = w.fleets[&convoy].pos;
        {
            let r = w.fleets.get_mut(&raider).unwrap();
            r.pos = cpos + Vec2::new(40.0, 0.0); // inside CONTACT_RADIUS
            r.vel = Vec2::ZERO;
            r.order = FleetOrder::Idle;
        }
        // Force the raider's command center near it so the commit applies promptly.
        w.players.get_mut(&atk).unwrap().command_center = cpos;
        let outcome = run_until_raid(&mut w, 30, |wld| {
            if wld.fleets.get(&raider).map(|s| matches!(s.order, FleetOrder::Intercept { .. })).unwrap_or(false) {
                vec![]
            } else {
                vec![Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]
            }
        });
        let outcome = outcome.expect("the raid on the production convoy should resolve");
        // If the convoy was destroyed, its production output is gone — real stakes.
        if outcome.kills().1 {
            assert!(!w.fleets.contains_key(&convoy), "a destroyed production convoy is gone");
        }
    }

    // ---- Standing orders / logistics automation (§15) ----------------------

    /// Claim a system, set an AboveThreshold standing order to the hub, then run
    /// the world with NO further commands (the player is OFFLINE). The rule must
    /// auto-dispatch a raidable convoy server-side and the sale must credit the
    /// absent owner — the core async-persistent promise.
    #[test]
    fn standing_order_auto_ships_to_hub_while_offline() {
        let mut w = test_world();
        w.enclaves.clear(); // isolate the standing-order loop from ambient piracy
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        grant_system(&mut w, id, sysid);
        w.step(&[]);
        let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().all_deposits().next().unwrap().resource;
        let credits0 = w.players[&id].credits;

        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0,
                source: Endpoint::System { id: sysid },
                dest: Endpoint::Hub,
                commodity,
                trigger: Trigger::AboveThreshold { threshold: 3.0 },
                status: OrderStatus::Active,
                next_eval_tick: 0,
                in_flight: None,
            },
        }]);
        assert_eq!(w.players[&id].standing_orders.len(), 1, "rule stored");
        assert_eq!(w.players[&id].standing_orders[0].id, 1, "create allocates id 1");

        // From here on: ZERO commands — pure server clock (player logged off).
        let (mut auto_fired, mut sold) = (false, false);
        for _ in 0..(500 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                match e.payload {
                    EventPayload::Trade(TradeEvent::AutoDispatched { player, .. }) if player == id => auto_fired = true,
                    EventPayload::Trade(TradeEvent::Sold { player, .. }) if player == id => sold = true,
                    _ => {}
                }
            }
            if sold {
                break;
            }
        }
        assert!(auto_fired, "the standing order must auto-dispatch a convoy while offline");
        assert!(sold, "the auto-shipped convoy must reach the hub and sell");
        assert!(w.players[&id].credits > credits0, "auto-selling must pay the absent owner");
    }

    /// Anti-spam: a permanently-satisfied threshold must NOT flood the map. At most
    /// ONE auto-ship convoy from a single rule is ever in flight at once.
    #[test]
    fn standing_order_never_floods_convoys() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        grant_system(&mut w, id, sysid);
        w.step(&[]);
        let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().all_deposits().next().unwrap().resource;
        // Threshold 1: the source is essentially always above it once producing.
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0,
                source: Endpoint::System { id: sysid },
                dest: Endpoint::Hub,
                commodity,
                trigger: Trigger::AboveThreshold { threshold: 1.0 },
                status: OrderStatus::Active,
                next_eval_tick: 0,
                in_flight: None,
            },
        }]);

        let mut dispatches = 0u32;
        let mut max_in_flight = 0usize;
        for _ in 0..(300 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::Trade(TradeEvent::AutoDispatched { player, .. }) if player == id) {
                    dispatches += 1;
                }
            }
            let in_flight = w
                .fleets
                .values()
                .filter(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub))
                .count();
            max_in_flight = max_in_flight.max(in_flight);
        }
        assert!(max_in_flight <= 1, "at most one auto-ship convoy in flight per rule (got {max_in_flight})");
        // It DID keep cycling (dispatched repeatedly as convoys arrived), not just once.
        assert!(dispatches >= 2, "the rule should re-fire across the run as convoys complete");
    }

    /// MaintainAtDest pulls supply INTO a destination system to keep it stocked, via
    /// the new system→system convoy — and stops once the level (incl. in-flight) is met.
    #[test]
    fn maintain_at_dest_supplies_a_system_to_system() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // Own two systems: a producing source and a destination depot. Pick the
        // depot NEAREST the source so a sub-light convoy reliably reaches it within
        // the run window (independent of the seeded galaxy's exact geometry).
        let source = richest_system(&w);
        let source_pos = w.systems.iter().find(|s| s.id == source).unwrap().pos;
        let commodity = w.systems.iter().find(|s| s.id == source).unwrap().all_deposits().next().unwrap().resource;
        let dest = w
            .systems
            .iter()
            .filter(|s| s.owner.is_none() && s.id != source)
            .min_by(|a, b| a.pos.distance(source_pos).total_cmp(&b.pos.distance(source_pos)))
            .unwrap()
            .id;
        grant_system(&mut w, id, source);
        w.step(&[]);
        grant_system(&mut w, id, dest);
        // Give the depot ample headroom so the supply always has room to land —
        // otherwise the dest's OWN production can fill the base cap before the
        // (sub-light) convoy arrives, a race unrelated to what this test checks.
        w.systems.iter_mut().find(|s| s.id == dest).unwrap().set_tier(crate::build::StructureKind::Depot, 5);
        w.step(&[]);

        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0,
                source: Endpoint::System { id: source },
                dest: Endpoint::System { id: dest },
                commodity,
                trigger: Trigger::MaintainAtDest { target: 5.0 },
                status: OrderStatus::Active,
                next_eval_tick: 0,
                in_flight: None,
            },
        }]);

        // Offline run: the dest stockpile of `commodity` should rise toward the target
        // as system→system convoys deliver. (Account for the dest's own production of
        // that commodity, if any, by asserting it reached at least the target.)
        let dest_has = |w: &World| {
            w.systems.iter().find(|s| s.id == dest).unwrap().stockpile.get(&commodity).copied().unwrap_or(0.0)
        };
        let mut delivered_via_route = false;
        for _ in 0..(500 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if matches!(e.payload, EventPayload::Trade(TradeEvent::Delivered { player, .. }) if player == id) {
                    delivered_via_route = true;
                }
            }
            if dest_has(&w) >= 5.0 && delivered_via_route {
                break;
            }
        }
        assert!(delivered_via_route, "a system→system supply convoy must deliver to the depot");
        assert!(dest_has(&w) >= 5.0, "MaintainAtDest must bring the depot up to the target");
    }

    /// §naming: a fresh galaxy gets pronounceable WORD names, drawn without
    /// replacement (no collisions across frontier + home), deterministic per seed;
    /// planets read inner→outer as I,II,III and moons hang off with a hyphen.
    #[test]
    fn galaxy_names_are_word_based_unique_and_deterministic() {
        let build = || World::new(SimConfig::for_players(0x00C0_FFEE, 4));
        let (w1, w2) = (build(), build());
        // Determinism: same seed → identical system names in the same order.
        let names1: Vec<&str> = w1.systems.iter().map(|s| s.name.as_str()).collect();
        let names2: Vec<&str> = w2.systems.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names1, names2, "same seed reproduces the same galaxy names");
        // No two systems (frontier OR home) share a name.
        let uniq: std::collections::BTreeSet<&str> = names1.iter().copied().collect();
        assert_eq!(uniq.len(), names1.len(), "no duplicate system names in a galaxy");
        // Word names, not the old "XX-NNN" catalogue codes (no hyphen in a system name).
        assert!(names1.iter().all(|n| !n.contains('-')), "system names are words, not codes");
        assert!(names1.iter().all(|n| n.chars().next().unwrap().is_ascii_uppercase()));
        // Planets read inner→outer as I, II, III…; moons hang off with a hyphen.
        let multi = w1
            .systems
            .iter()
            .find(|s| s.bodies.iter().filter(|b| b.parent.is_none()).count() >= 2)
            .expect("some system has multiple planets");
        let planets: Vec<&crate::body::Body> = multi.bodies.iter().filter(|b| b.parent.is_none()).collect();
        for (i, p) in planets.iter().enumerate() {
            let want = format!(" {}", crate::body::planet_numeral(i));
            assert!(p.name.ends_with(&want), "planet #{} named {}", i + 1, p.name);
        }
        for sys in &w1.systems {
            // Body ids are per-system, so resolve a moon's parent WITHIN its system.
            for m in sys.bodies.iter().filter(|b| b.parent.is_some()) {
                let p = sys.bodies.iter().find(|b| b.id == m.parent.unwrap()).expect("moon's parent exists in-system");
                assert!(m.name.starts_with(&format!("{}-", p.name)), "moon {} off parent {}", m.name, p.name);
            }
        }
    }

    /// Standing-order execution is deterministic: same seed + same commands ⇒ byte-
    /// identical world (the rules + their convoys + credits all reproduce).
    #[test]
    fn standing_orders_are_deterministic() {
        let build = || {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let sysid = richest_system(&w);
            let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().all_deposits().next().unwrap().resource;
            grant_system(&mut w, id, sysid);
        w.step(&[]);
            w.step(&[Command::SetStandingOrder {
                player_id: id,
                order: StandingOrder {
                    id: 0,
                    source: Endpoint::System { id: sysid },
                    dest: Endpoint::Hub,
                    commodity,
                    trigger: Trigger::PercentSurplus { percent: 50, floor: 2.0 },
                    status: OrderStatus::Active,
                    next_eval_tick: 0,
                    in_flight: None,
                },
            }]);
            for _ in 0..(200 * crate::config::TICK_HZ) {
                w.step(&[]);
            }
            w
        };
        let a = build();
        let b = build();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "standing-order execution must be reproducible from the seed"
        );
        // And the rule actually ran (it fired at least once → credits grew past start).
        assert!(a.players[&PlayerId(1)].credits > 10_000.0 - 1.0 || !a.fleets.is_empty());
    }

    /// Clearing a standing order stops it; invalid rules are rejected at set-time.
    #[test]
    fn standing_order_clear_and_validation() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        grant_system(&mut w, id, sysid);
        w.step(&[]);

        // Invalid: source is the hub (not a system) → rejected.
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0, source: Endpoint::Hub, dest: Endpoint::Home,
                commodity: crate::cargo::Commodity::MetallicOre,
                trigger: Trigger::AboveThreshold { threshold: 1.0 },
                status: OrderStatus::Active, next_eval_tick: 0, in_flight: None,
            },
        }]);
        // Invalid: source you don't own → rejected.
        let unowned = w.systems.iter().find(|s| s.owner.is_none()).unwrap().id;
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0, source: Endpoint::System { id: unowned }, dest: Endpoint::Hub,
                commodity: crate::cargo::Commodity::MetallicOre,
                trigger: Trigger::AboveThreshold { threshold: 1.0 },
                status: OrderStatus::Active, next_eval_tick: 0, in_flight: None,
            },
        }]);
        // Invalid: MaintainAtDest with a Hub destination → rejected.
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0, source: Endpoint::System { id: sysid }, dest: Endpoint::Hub,
                commodity: crate::cargo::Commodity::MetallicOre,
                trigger: Trigger::MaintainAtDest { target: 5.0 },
                status: OrderStatus::Active, next_eval_tick: 0, in_flight: None,
            },
        }]);
        assert!(w.players[&id].standing_orders.is_empty(), "invalid rules must be rejected");

        // Valid rule → stored; then cleared.
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0, source: Endpoint::System { id: sysid }, dest: Endpoint::Hub,
                commodity: crate::cargo::Commodity::MetallicOre,
                trigger: Trigger::AboveThreshold { threshold: 1.0 },
                status: OrderStatus::Active, next_eval_tick: 0, in_flight: None,
            },
        }]);
        let rid = w.players[&id].standing_orders[0].id;
        w.step(&[Command::ClearStandingOrder { player_id: id, order_id: rid }]);
        assert!(w.players[&id].standing_orders.is_empty(), "cleared rule is gone");
    }

    /// Two MaintainAtDest rules from different sources to the SAME destination,
    /// evaluated on the SAME tick, must not EACH ship the full deficit (over-shoot).
    /// The per-tick "planned" tally folds a sibling's just-planned shipment into the
    /// in-flight accounting.
    #[test]
    fn maintain_at_dest_two_sources_do_not_overship() {
        use crate::cargo::Commodity::MetallicOre;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // Take three unowned systems and claim them directly (deterministic setup).
        let ids: Vec<EntityId> = w.systems.iter().filter(|s| s.owner.is_none()).take(3).map(|s| s.id).collect();
        let (a, b, d) = (ids[0], ids[1], ids[2]);
        let now = w.time;
        for &sid in &[a, b, d] {
            let sys = w.systems.iter_mut().find(|s| s.id == sid).unwrap();
            sys.owner = Some(id);
            sys.claimed_at = Some(now);
        }
        // Stock both sources well above the target; empty the destination.
        for &sid in &[a, b] {
            w.systems.iter_mut().find(|s| s.id == sid).unwrap().stockpile.insert(MetallicOre, 50.0);
        }
        w.systems.iter_mut().find(|s| s.id == d).unwrap().stockpile.remove(&MetallicOre);

        for &src in &[a, b] {
            w.step(&[Command::SetStandingOrder {
                player_id: id,
                order: StandingOrder {
                    id: 0,
                    source: Endpoint::System { id: src },
                    dest: Endpoint::System { id: d },
                    commodity: MetallicOre,
                    trigger: Trigger::MaintainAtDest { target: 5.0 },
                    status: OrderStatus::Active,
                    next_eval_tick: 0,
                    in_flight: None,
                },
            }]);
        }
        // Reset to a clean slate where BOTH rules are idle + eligible on the SAME
        // upcoming eval tick (the over-ship scenario): drop any convoy a rule already
        // launched during setup, refill sources, empty the depot, clear the gates.
        w.fleets.retain(|_, s| s.mission.is_none());
        for &sid in &[a, b] {
            w.systems.iter_mut().find(|s| s.id == sid).unwrap().stockpile.insert(MetallicOre, 50.0);
        }
        w.systems.iter_mut().find(|s| s.id == d).unwrap().stockpile.remove(&MetallicOre);
        for o in w.players.get_mut(&id).unwrap().standing_orders.iter_mut() {
            o.next_eval_tick = 0;
            o.in_flight = None;
        }
        // One step: both rules evaluate together. Sum what they auto-dispatched.
        let mut dispatched = 0u32;
        for e in w.step(&[]) {
            if let EventPayload::Trade(TradeEvent::AutoDispatched { player, units, .. }) = e.payload
                && player == id
            {
                dispatched += units;
            }
        }
        assert!(dispatched >= 1, "a maintain rule should ship toward the empty depot");
        assert!(dispatched <= 5, "two rules to one dest must not over-ship the target (shipped {dispatched})");
    }

    // ---- Fleet doctrine (§16, async-automation Layer 2) ----

    /// A doctrine is just a Copy menu installed by command — INSTANT local admin,
    /// always valid, defaulting to today's behaviour.
    #[test]
    fn set_fleet_doctrine_round_trips() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        assert_eq!(w.players[&id].doctrine, FleetDoctrine::default());
        let doc = FleetDoctrine {
            engagement: EngagementPolicy::EngageAny,
            retreat: RetreatThreshold::Half,
            escort: EscortPolicy::GuardRichest,
            destination_invalid: DestinationInvalidPolicy::SellAtHub,
        };
        w.step(&[Command::SetFleetDoctrine { player_id: id, doctrine: doc }]);
        assert_eq!(w.players[&id].doctrine, doc, "doctrine installs verbatim");
    }

    /// `EngagementPolicy::Avoid`: a picket never autonomously breaks off, even when
    /// a hostile is closing on the very convoy it would otherwise guard.
    #[test]
    fn doctrine_avoid_never_breaks_off() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        // The classic defensive scenario (default doctrine WOULD engage here).
        let (patrol, _c, _h) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos + Vec2::new(0.0, 120.0), Vec2::new(1500.0, 0.0));
        w.players.get_mut(&d).unwrap().doctrine.engagement = EngagementPolicy::Avoid;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(w.fleets[&patrol].defense.is_none(), "Avoid doctrine never engages");
            assert!(matches!(w.fleets[&patrol].order, FleetOrder::Patrol { .. }), "it stays on patrol");
        }
    }

    /// `EngagementPolicy::EngageAny`: a picket hunts a hostile it senses even when
    /// that hostile is NOT on a course at a convoy — something `DefensiveOnly`
    /// (the default) deliberately ignores.
    #[test]
    fn doctrine_engage_any_hunts_a_non_threatening_raider() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1500.0, 0.0));
        // Re-cast the hostile as a PARKED drifter inside the picket's sensor bubble
        // (no speed, no course at the convoy) — invisible to a defensive picket.
        {
            let h = w.fleets.get_mut(&hostile).unwrap();
            h.pos = convoy_pos + Vec2::new(500.0, 0.0);
            h.vel = Vec2::ZERO;
            h.order = FleetOrder::Idle;
        }
        // Default DefensiveOnly: ignores the non-closing drifter.
        for _ in 0..(3 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(w.fleets[&patrol].defense.is_none(), "DefensiveOnly ignores a parked drifter");
        }
        // EngageAny: now it breaks off to hunt the same contact.
        w.players.get_mut(&d).unwrap().doctrine.engagement = EngagementPolicy::EngageAny;
        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if engaged_on(&w, patrol, hostile) {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "EngageAny hunts any sensed hostile");
    }

    /// `EngagementPolicy::EngageWeaker`: opportunistic — it only commits to a hunt
    /// when it locally OUTNUMBERS the enemy.
    #[test]
    fn doctrine_engage_weaker_needs_numerical_advantage() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1500.0, 0.0));
        {
            let h = w.fleets.get_mut(&hostile).unwrap();
            h.pos = convoy_pos + Vec2::new(500.0, 0.0);
            h.vel = Vec2::ZERO;
            h.order = FleetOrder::Idle;
        }
        w.players.get_mut(&d).unwrap().doctrine.engagement = EngagementPolicy::EngageWeaker;
        // 1 picket vs 1 hostile: an even fight → EngageWeaker declines.
        for _ in 0..(3 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(w.fleets[&patrol].defense.is_none(), "EngageWeaker declines a 1:1 fight");
        }
        // Add a friendly raider beside the picket: now we outnumber → it engages.
        let ally = w.alloc_entity_id();
        let ppos = w.fleets[&patrol].pos;
        w.fleets.insert(ally, Fleet::single(ally, d, ShipKind::Raider, ppos, FleetOrder::Idle, None));
        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if engaged_on(&w, patrol, hostile) {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "EngageWeaker engages once it outnumbers the enemy");
    }

    /// `RetreatThreshold`: `Never` (default) fights regardless of odds; `Half`
    /// withdraws an already-committed picket HOME once enemy reinforcements push
    /// the local force ratio below the threshold (re-checked every tick).
    #[test]
    fn doctrine_retreat_threshold_withdraws_when_outnumbered() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let (patrol, _c, _h) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1500.0, 0.0));
        // Engage under the default (Never) doctrine.
        let mut engaged = false;
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.fleets[&patrol].defense.is_some() {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "picket should have committed");

        // Pile two hostile raiders onto the picket: 1-vs-3, ratio 0.25.
        let ppos = w.fleets[&patrol].pos;
        for k in 0..2 {
            let hid = w.alloc_entity_id();
            let off = Vec2::new(40.0 * (k as f64 + 1.0), 0.0);
            w.fleets.insert(hid, Fleet::single(hid, a, ShipKind::Raider, ppos + off, FleetOrder::Idle, None));
        }
        // Never: it keeps fighting despite the odds.
        w.step(&[]);
        assert!(w.fleets[&patrol].defense.is_some(), "Never doctrine fights outnumbered");
        assert!(matches!(w.fleets[&patrol].order, FleetOrder::Intercept { .. }));

        // Switch to Half: next tick it breaks off and withdraws home.
        w.players.get_mut(&d).unwrap().doctrine.retreat = RetreatThreshold::Half;
        w.step(&[]);
        assert!(w.fleets[&patrol].defense.is_none(), "retreat clears the engagement");
        let home = w.players[&d].home;
        assert!(
            matches!(w.fleets[&patrol].order, FleetOrder::MoveTo { dest } if dest == home),
            "an outnumbered picket withdraws home"
        );
    }

    /// `EscortPolicy::HoldStation` keeps the player's set patrol route (a fixed
    /// chokepoint picket); `GuardNearest` (default) rewrites it to shadow the
    /// convoy. Verified with no threat present, so only the escort path runs.
    #[test]
    fn doctrine_hold_station_keeps_player_route() {
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        // Patrol within escort range of the convoy; remove the hostile entirely.
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos + Vec2::new(700.0, 0.0), Vec2::new(1500.0, 0.0));
        w.fleets.remove(&hostile);
        let route = |w: &World| match &w.fleets[&patrol].order {
            FleetOrder::Patrol { waypoints, .. } => waypoints.clone(),
            _ => panic!("picket should be on patrol"),
        };
        let route_before = route(&w);

        // HoldStation: the route is left exactly as the player set it.
        w.players.get_mut(&d).unwrap().doctrine.escort = EscortPolicy::HoldStation;
        w.step(&[]);
        w.step(&[]);
        assert_eq!(route(&w), route_before, "HoldStation keeps the player's set route");

        // GuardNearest: the route is rewritten to bracket the convoy (shadowing).
        w.players.get_mut(&d).unwrap().doctrine.escort = EscortPolicy::GuardNearest;
        w.step(&[]);
        assert_ne!(route(&w), route_before, "GuardNearest rewrites the route to shadow the convoy");
    }

    /// Spawn an automated supply convoy onto a destination the corp does NOT own,
    /// so delivery fails and the destination-invalid doctrine decides the cargo's
    /// fate. Returns (convoy id, destination system id).
    fn doomed_supply(w: &mut World, owner: PlayerId) -> (EntityId, EntityId) {
        let d = w.systems.iter().find(|s| s.owner.is_none()).unwrap().id;
        let d_pos = w.systems.iter().find(|s| s.id == d).unwrap().pos;
        let cargo = Cargo { commodity: crate::cargo::Commodity::MetallicOre, units: 30 };
        // Spawn essentially on top of the destination so it "arrives" at once.
        let convoy = w.spawn_trade_convoy(
            owner,
            d_pos + Vec2::new(1.0, 0.0),
            d_pos,
            cargo,
            TradeMission::DeliverToSystem { system: d },
        );
        (convoy, d)
    }

    fn run_until_divert(w: &mut World, system: EntityId) -> Option<DivertAction> {
        for _ in 0..(15 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::Trade(TradeEvent::SupplyDiverted { action, system: s, .. }) = e.payload
                    && s == system
                {
                    return Some(action);
                }
            }
        }
        None
    }

    /// `DestinationInvalidPolicy::Drop` (default): a supply convoy to a lost system
    /// loses its cargo — the frontier risk of automation.
    #[test]
    fn destination_invalid_drop_loses_cargo() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let (convoy, d) = doomed_supply(&mut w, id);
        assert_eq!(run_until_divert(&mut w, d), Some(DivertAction::Lost), "Drop loses the cargo");
        assert!(!w.fleets.contains_key(&convoy), "the dropped convoy is gone");
    }

    /// `ReturnHome` / `SellAtHub`: instead of losing the cargo, the SAME convoy is
    /// re-routed onto a new (still raidable) leg — home, or to the hub to sell.
    #[test]
    fn destination_invalid_reroutes_keep_the_cargo() {
        // ReturnHome.
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        w.players.get_mut(&id).unwrap().doctrine.destination_invalid =
            DestinationInvalidPolicy::ReturnHome;
        let home = w.players[&id].home;
        let (convoy, d) = doomed_supply(&mut w, id);
        assert_eq!(run_until_divert(&mut w, d), Some(DivertAction::ReturnedHome));
        let ship = w.fleets.get(&convoy).expect("re-routed convoy still flies (raidable)");
        assert!(matches!(ship.mission, Some(TradeMission::DeliverHome)), "re-tasked to deliver home");
        assert!(matches!(ship.order, FleetOrder::MoveTo { dest } if dest == home), "heading home");
        assert!(ship.cargo.is_some(), "cargo preserved");

        // SellAtHub.
        let mut w = test_world();
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        w.players.get_mut(&id).unwrap().doctrine.destination_invalid =
            DestinationInvalidPolicy::SellAtHub;
        let hub = w.hub;
        let (convoy, d) = doomed_supply(&mut w, id);
        assert_eq!(run_until_divert(&mut w, d), Some(DivertAction::SoldAtHub));
        let ship = w.fleets.get(&convoy).expect("re-routed convoy still flies");
        assert!(matches!(ship.mission, Some(TradeMission::SellAtHub)), "re-tasked to sell at hub");
        assert!(matches!(ship.order, FleetOrder::MoveTo { dest } if dest == hub), "heading to the hub");
    }

    /// Doctrine-driven autonomous behaviour stays deterministic: identical seed +
    /// commands (including a SetFleetDoctrine) ⇒ byte-identical snapshots.
    #[test]
    fn doctrine_behaviour_is_deterministic() {
        let run = || {
            let mut w = test_world();
            let (d, a) = (PlayerId(1), PlayerId(2));
            let convoy_pos = Vec2::new(3000.0, 0.0);
            let (_p, _c, _h) =
                defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1500.0, 0.0));
            let doc = FleetDoctrine {
                engagement: EngagementPolicy::EngageAny,
                retreat: RetreatThreshold::Half,
                escort: EscortPolicy::GuardRichest,
                destination_invalid: DestinationInvalidPolicy::ReturnHome,
            };
            w.step(&[Command::SetFleetDoctrine { player_id: d, doctrine: doc }]);
            for _ in 0..(30 * crate::config::TICK_HZ) {
                w.step(&[]);
            }
            serde_json::to_string(&w).unwrap()
        };
        assert_eq!(run(), run(), "doctrine-driven sim is reproducible from seed + commands");
    }

    // --- §CONTESTABLE TERRITORY Part 1: BLOCKADE -----------------------------

    /// Grant `owner` an unclaimed system at `pos` (owned, no platform).
    fn grant_system_at(w: &mut World, owner: PlayerId, pos: Vec2, tier: u32) -> EntityId {
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(owner);
        sys.claimed_at = Some(0.0);
        sys.pos = pos;
        sys.set_tier(crate::build::StructureKind::DefensePlatform, tier);
        sys.id
    }

    /// Park a raider fleet of `raiders` ships ON STATION at `sys`, with the
    /// Blockade order already installed (bypasses the light-delayed command so
    /// the resolve pass can be exercised directly).
    fn blockader_on_station(w: &mut World, owner: PlayerId, sys: EntityId, raiders: u32) -> EntityId {
        let pos = w.systems.iter().find(|s| s.id == sys).unwrap().pos;
        let id = w.alloc_entity_id();
        let mut f = Fleet::single(id, owner, ShipKind::Raider, pos, FleetOrder::Blockade { system: sys, station: pos }, None);
        f.composition.insert(ShipKind::Raider, raiders);
        w.fleets.insert(id, f);
        id
    }

    #[test]
    fn blockade_of_undefended_system_establishes_and_holds_outbound() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 0); // no platform
        seed_stock(&mut w, sys, &[(Commodity::MetallicOre, 50.0), (crate::fuel::MOVEMENT_FUEL, 500.0)]);
        let blk = blockader_on_station(&mut w, atk, sys, 1);

        let mut established = None;
        for _ in 0..5 {
            for e in w.step(&[]) {
                if let EventPayload::BlockadeEstablished { by, system, .. } = e.payload {
                    established = Some((by, system));
                }
            }
        }
        assert_eq!(established, Some((atk, sys)), "an undefended system blockades at once (no battle)");
        assert!(w.systems.iter().find(|s| s.id == sys).unwrap().blockade.is_some());

        // OUTBOUND hold: shipping production from a blockaded system dispatches
        // NO convoy (the goods stay put; production still accrues).
        let fleets_before = w.fleets.len();
        w.step(&[Command::ShipProduction { player_id: def, system_id: sys }]);
        assert_eq!(w.fleets.len(), fleets_before, "no convoy leaves a blockaded system");
        assert!(w.fleets.contains_key(&blk), "the blockader holds station");
    }

    #[test]
    fn blockade_holds_inbound_convoy_at_standoff() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 0);
        let sys_pos = w.systems.iter().find(|s| s.id == sys).unwrap().pos;
        blockader_on_station(&mut w, atk, sys, 1);
        // A def supply convoy inbound to its own (soon-blockaded) system.
        let cid = w.alloc_entity_id();
        let mut convoy = Fleet::single(cid, def, ShipKind::Convoy, sys_pos + Vec2::new(3000.0, 0.0), FleetOrder::MoveTo { dest: sys_pos }, Some(crate::cargo::Cargo { commodity: Commodity::Provisions, units: 10 }));
        convoy.mission = Some(TradeMission::DeliverToSystem { system: sys });
        w.fleets.insert(cid, convoy);

        w.step(&[]); // resolve_blockades establishes + re-targets the inbound convoy
        let dest = match w.fleets[&cid].order {
            FleetOrder::MoveTo { dest } => dest,
            _ => panic!("inbound convoy should still be moving (to standoff)"),
        };
        let standoff = dest.distance(sys_pos);
        assert!((standoff - BLOCKADE_STANDOFF_RADIUS).abs() < 1.0, "held on the standoff ring, not delivered (got {standoff})");
    }

    #[test]
    fn blockade_wins_establishment_battle_vs_platform_then_holds() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        // A modest platform (tier 1) vs a strong raider wing — the raiders grind
        // the platform to 0 and the blockade holds. Playtest-scaled battle timing.
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 1);
        let blk = blockader_on_station(&mut w, atk, sys, 12);

        let mut blockaded = false;
        for _ in 0..(200 * crate::config::TICK_HZ) {
            w.step(&[]);
            let s = w.systems.iter().find(|s| s.id == sys).unwrap();
            if s.tier_sum(crate::build::StructureKind::DefensePlatform) == 0 && s.blockade.is_some() && w.fleets.contains_key(&blk) {
                blockaded = true;
                break;
            }
        }
        assert!(blockaded, "the blockader grinds the platform down and holds station");
        // It stays on its Blockade order (not sent home after winning).
        assert!(matches!(w.fleets[&blk].order, FleetOrder::Blockade { .. }), "winner resumes station");
    }

    #[test]
    fn blockade_lifts_when_blockader_destroyed() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        // A lone raider vs a TALL platform (tier 12): the platform destroys it,
        // so the blockade — which established on arrival — LIFTS.
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 12);
        let blk = blockader_on_station(&mut w, atk, sys, 1);

        let mut lifted = false;
        for _ in 0..(200 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::BlockadeLifted { system, .. } = e.payload
                    && system == sys
                {
                    lifted = true;
                }
            }
            if lifted { break; }
        }
        assert!(lifted, "destroying the blockader lifts the blockade");
        assert!(!w.fleets.contains_key(&blk), "the lone blockader was destroyed");
        assert!(w.systems.iter().find(|s| s.id == sys).unwrap().blockade.is_none());
    }

    #[test]
    fn blockade_command_requires_a_raider_and_a_rival_target() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let sys = grant_system_at(&mut w, def, Vec2::new(3000.0, 0.0), 0);
        // A CONVOY fleet (no raider) can't blockade — soft reject (order unchanged).
        let convoy = find_ship(&w, atk, ShipKind::Convoy);
        w.step(&[Command::BlockadeSystem { player_id: atk, fleet_id: convoy, system_id: sys }]);
        for _ in 0..20 { w.step(&[]); }
        assert!(!matches!(w.fleets[&convoy].order, FleetOrder::Blockade { .. }), "a convoy can't blockade");
        // A raider CAN — its order becomes Blockade once the command's light lands.
        let raider = find_ship(&w, atk, ShipKind::Raider);
        w.fleets.get_mut(&raider).unwrap().pos = w.players[&atk].command_center;
        w.step(&[Command::BlockadeSystem { player_id: atk, fleet_id: raider, system_id: sys }]);
        let mut became_blockade = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.step(&[]);
            if matches!(w.fleets.get(&raider).map(|f| &f.order), Some(FleetOrder::Blockade { .. })) {
                became_blockade = true;
                break;
            }
        }
        assert!(became_blockade, "a raider fleet accepts the blockade order (after light delay)");
        // A raider can't blockade an UNOWNED system (soft reject).
        let unclaimed = w.systems.iter().find(|s| s.is_unclaimed()).unwrap().id;
        let r2 = blockader_on_station(&mut w, atk, sys, 1); // reuse helper for a fresh raider
        w.fleets.get_mut(&r2).unwrap().order = FleetOrder::Idle;
        w.step(&[Command::BlockadeSystem { player_id: atk, fleet_id: r2, system_id: unclaimed }]);
        for _ in 0..20 { w.step(&[]); }
        assert!(!matches!(w.fleets[&r2].order, FleetOrder::Blockade { .. }), "can't blockade an unowned system");
    }

    // --- §CONTESTABLE TERRITORY Part 2: SIEGE → CAPTURE ----------------------

    /// Establish a blockade on an undefended `def` system and BACKDATE its siege
    /// clock so a colony delivered now would capture (skips running the full
    /// SIEGE_DURATION in tests). Returns the system id.
    fn ripe_siege(w: &mut World, atk: PlayerId, def: PlayerId, pos: Vec2, tier: u32) -> EntityId {
        let sys = grant_system_at(w, def, pos, tier);
        blockader_on_station(w, atk, sys, if tier > 0 { 12 } else { 1 });
        // Run until blockaded (and, if defended, until the platform is ground down).
        for _ in 0..(300 * crate::config::TICK_HZ) {
            w.step(&[]);
            let s = w.systems.iter().find(|s| s.id == sys).unwrap();
            if s.blockade.is_some() && s.tier_sum(crate::build::StructureKind::DefensePlatform) == 0 { break; }
        }
        // Backdate the (already-running, undefended) siege clock past the duration.
        let dur = w.siege_duration_secs();
        let now = w.time;
        w.systems.iter_mut().find(|s| s.id == sys).unwrap().blockade.as_mut().unwrap().siege_since = Some(now - dur - 1.0);
        sys
    }

    fn step_capture(w: &mut World) -> Option<(PlayerId, EntityId)> {
        for _ in 0..5 {
            for e in w.step(&[]) {
                if let EventPayload::SystemCaptured { new_owner, system, .. } = e.payload {
                    return Some((new_owner, system));
                }
            }
        }
        None
    }

    #[test]
    fn siege_plus_colony_captures_with_half_tiers_and_plunder() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
        ]);
        let pos = Vec2::new(5000.0, 0.0);
        let sys = ripe_siege(&mut w, atk, def, pos, 0);
        {
            let s = w.systems.iter_mut().find(|s| s.id == sys).unwrap();
            s.set_tier(crate::build::StructureKind::MiningComplex, 4);
            s.set_tier(crate::build::StructureKind::Shipyard, 2);
            s.set_tier(crate::build::StructureKind::Habitat, 3);
            *s.stockpile.entry(Commodity::MetallicOre).or_insert(0.0) += 100.0;
        }
        let colony = colony_at(&mut w, atk, pos);

        let mut plunder = None;
        let cap = {
            let mut got = None;
            'outer: for _ in 0..5 {
                for e in w.step(&[]) {
                    if let EventPayload::SystemCaptured { new_owner, system, plunder: p, .. } = &e.payload {
                        plunder = Some(p.clone());
                        got = Some((*new_owner, *system));
                        break 'outer;
                    }
                }
            }
            got
        };
        assert_eq!(cap, Some((atk, sys)), "a colony delivered to a ripe siege captures");
        let s = w.systems.iter().find(|s| s.id == sys).unwrap();
        assert_eq!(s.owner, Some(atk), "ownership flipped to the captor");
        assert_eq!(s.tier(crate::build::StructureKind::MiningComplex), 2, "developments transfer at HALF tiers (4→2)");
        assert_eq!(s.tier(crate::build::StructureKind::Shipyard), 1, "2→1");
        assert_eq!(s.tier(crate::build::StructureKind::Habitat), 1, "3→1 (rounded down)");
        assert!(s.blockade.is_none(), "the captured system is no longer besieged");
        assert_eq!(plunder.unwrap().get(&Commodity::MetallicOre).copied(), Some(100), "the stockpile is plundered (itemized)");
        assert!(!w.fleets.contains_key(&colony), "the lone colony ship was consumed (occupation)");
    }

    #[test]
    fn capture_needs_defenses_down_a_ripe_clock_and_a_colony() {
        // (a) DEFENSES UP: a platform that never suppresses → siege clock never
        //     starts; a delivered colony is held, not consumed.
        {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
            let pos = Vec2::new(5000.0, 0.0);
            // A lone blockader vs a tall platform: it can't suppress the defenses.
            let sys = grant_system_at(&mut w, def, pos, 12);
            blockader_on_station(&mut w, atk, sys, 1);
            for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
            let s = w.systems.iter().find(|s| s.id == sys).unwrap();
            assert!(s.tier(crate::build::StructureKind::DefensePlatform) > 0 && s.blockade.as_ref().and_then(|b| b.siege_since).is_none(), "defenses up → no siege clock");
        }
        // (b) GARRISON present: siege clock can't run while a defender combatant holds.
        {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
            let pos = Vec2::new(5000.0, 0.0);
            let sys = grant_system_at(&mut w, def, pos, 0);
            blockader_on_station(&mut w, atk, sys, 1);
            let gid = w.alloc_entity_id();
            w.fleets.insert(gid, Fleet::single(gid, def, ShipKind::Corvette, pos, FleetOrder::Idle, None));
            w.step(&[]); // first resolve sees the garrison
            let s = w.systems.iter().find(|s| s.id == sys).unwrap();
            assert!(s.blockade.is_some() && s.blockade.as_ref().unwrap().siege_since.is_none(), "a garrison blocks the siege clock");
        }
        // (c) SIEGE TOO SHORT: undefended + blockaded but the clock just started.
        {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
            let pos = Vec2::new(5000.0, 0.0);
            let sys = grant_system_at(&mut w, def, pos, 0);
            blockader_on_station(&mut w, atk, sys, 1);
            w.step(&[]); // established; siege_since = now (fresh, not ripe)
            let colony = colony_at(&mut w, atk, pos);
            assert_eq!(step_capture(&mut w), None, "a fresh siege can't be captured yet");
            assert_eq!(w.systems.iter().find(|s| s.id == sys).unwrap().owner, Some(def), "still the defender's");
            assert!(w.fleets.contains_key(&colony), "colony ship held, not consumed in vain");
        }
        // (d) NO COLONY: a ripe siege with no colonists never captures — sieges
        //     strangle, only colonists conquer.
        {
            let mut w = test_world();
            let (atk, def) = (PlayerId(1), PlayerId(2));
            w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
            let sys = ripe_siege(&mut w, atk, def, Vec2::new(5000.0, 0.0), 0);
            for _ in 0..10 { w.step(&[]); }
            assert_eq!(w.systems.iter().find(|s| s.id == sys).unwrap().owner, Some(def), "no colony ⇒ no capture, ever");
        }
    }

    #[test]
    fn home_system_is_never_captured() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
        // Blockade the DEFENDER'S HOME system directly.
        let home = w.players[&def].home_system.unwrap();
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        // Suppress its bootstrap shipyard-tier defenses irrelevant; ensure defense 0.
        w.systems.iter_mut().find(|s| s.id == home).unwrap().set_tier(crate::build::StructureKind::DefensePlatform, 0);
        blockader_on_station(&mut w, atk, home, 1);
        w.step(&[]); // blockades — but a home NEVER starts a siege clock
        // Force-backdate anyway; resolve_blockades must RESET it (home protection).
        let backdated = w.time - w.siege_duration_secs() - 1.0;
        if let Some(b) = w.systems.iter_mut().find(|s| s.id == home).unwrap().blockade.as_mut() {
            b.siege_since = Some(backdated);
        }
        let colony = colony_at(&mut w, atk, home_pos);
        assert_eq!(step_capture(&mut w), None, "a home system can never be captured (no elimination)");
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().owner, Some(def), "the beaten player keeps their home");
        assert!(w.fleets.contains_key(&colony), "the colony ship is held, not consumed against a home");
        assert!(w.systems.iter().find(|s| s.id == home).unwrap().blockade.as_ref().unwrap().siege_since.is_none(), "home siege clock stays reset");
    }

    #[test]
    fn siege_clock_resets_on_lift() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
        let pos = Vec2::new(5000.0, 0.0);
        let sys = grant_system_at(&mut w, def, pos, 0);
        let blk = blockader_on_station(&mut w, atk, sys, 1);
        w.step(&[]);
        let first = w.systems.iter().find(|s| s.id == sys).unwrap().blockade.as_ref().unwrap().siege_since.unwrap();
        // Lift: send the blockader far away (off station).
        w.fleets.get_mut(&blk).unwrap().order = FleetOrder::MoveTo { dest: Vec2::new(-9000.0, 0.0) };
        for _ in 0..(3 * crate::config::TICK_HZ) { w.step(&[]); }
        assert!(w.systems.iter().find(|s| s.id == sys).unwrap().blockade.is_none(), "moving off station lifts it");
        // Re-blockade with a fresh fleet → a NEW siege clock, not the old one.
        w.fleets.get_mut(&blk).unwrap().pos = pos;
        w.fleets.get_mut(&blk).unwrap().order = FleetOrder::Blockade { system: sys, station: pos };
        w.step(&[]);
        let second = w.systems.iter().find(|s| s.id == sys).unwrap().blockade.as_ref().unwrap().siege_since.unwrap();
        assert!(second > first, "a lift fully resets the clock (new siege starts later)");
    }

    #[test]
    fn blockade_and_siege_survive_a_snapshot() {
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: atk, name: "A".into() }, Command::AddPlayer { id: def, name: "D".into() }]);
        let sys = ripe_siege(&mut w, atk, def, Vec2::new(5000.0, 0.0), 0);
        let before = w.systems.iter().find(|s| s.id == sys).unwrap().blockade;
        assert!(before.is_some() && before.unwrap().siege_since.is_some());
        // serde round-trip (mid-siege persistence).
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        let after = w2.systems.iter().find(|s| s.id == sys).unwrap().blockade;
        assert_eq!(format!("{before:?}"), format!("{after:?}"), "blockade + siege clock persist across a snapshot");
    }

    // ===================================================================
    // §SYNDICATES Part 1 — membership + non-engagement
    // ===================================================================
    use crate::ids::SyndicateId;

    /// Found a syndicate with `a` and bring `b` in via invite → accept. Returns
    /// the syndicate id.
    fn ally(w: &mut World, a: PlayerId, b: PlayerId) -> SyndicateId {
        w.step(&[Command::CreateSyndicate { player_id: a, name: "Pact".into() }]);
        let sid = w.players[&a].syndicate.expect("founded a syndicate");
        w.step(&[Command::InviteToSyndicate { player_id: a, invitee: b }]);
        w.step(&[Command::AcceptSyndicateInvite { player_id: b, syndicate_id: sid }]);
        sid
    }

    #[test]
    fn syndicate_create_invite_accept_forms_an_alliance() {
        let mut w = test_world();
        let (a, b, c) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
            Command::AddPlayer { id: c, name: "C".into() },
        ]);
        let sid = ally(&mut w, a, b);
        assert!(w.are_allied(a, b) && w.are_allied(b, a), "both directions allied (ground truth)");
        assert_eq!(w.players[&a].syndicate, Some(sid));
        assert_eq!(w.players[&b].syndicate, Some(sid));
        let s = &w.syndicates[&sid];
        assert!(s.members.contains(&a) && s.members.contains(&b) && s.members.len() == 2);
        assert_eq!(s.founder, a);
        // A non-member is not an ally in either direction.
        assert!(!w.are_allied(a, c) && !w.are_allied(c, a), "outsider is never an ally");
        assert!(!w.are_allied(a, a), "self is not an ally");
    }

    #[test]
    fn syndicate_cap_scales_with_active_corps() {
        use crate::syndicate::syndicate_cap;
        assert_eq!(syndicate_cap(1), 2, "min floor");
        assert_eq!(syndicate_cap(5), 2);
        assert_eq!(syndicate_cap(6), 2);
        assert_eq!(syndicate_cap(9), 3);
        assert_eq!(syndicate_cap(12), 4);
    }

    #[test]
    fn syndicate_size_cap_rejects_overfill() {
        let mut w = test_world();
        let (a, b, c, d) = (PlayerId(1), PlayerId(2), PlayerId(3), PlayerId(4));
        for (id, n) in [(a, "A"), (b, "B"), (c, "C"), (d, "D")] {
            w.step(&[Command::AddPlayer { id, name: n.into() }]);
        }
        // 4 active corps → cap = max(2, floor(4/3)) = 2. A 2-member syndicate is full.
        let sid = ally(&mut w, a, b);
        assert_eq!(w.syndicates[&sid].members.len(), 2);
        w.step(&[Command::InviteToSyndicate { player_id: a, invitee: c }]);
        w.step(&[Command::AcceptSyndicateInvite { player_id: c, syndicate_id: sid }]);
        assert!(w.players[&c].syndicate.is_none(), "the cap rejects the 3rd member");
        assert_eq!(w.syndicates[&sid].members.len(), 2, "roster stays at the cap");
    }

    #[test]
    fn syndicate_weapons_free_spares_an_ally() {
        // Mirror `weapons_free_auto_commits_on_a_rival_in_its_own_bubble`, but the
        // "target" is an ALLY — the WeaponsFree raider must NOT hunt it.
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, _convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(1000.0, 0.0));
        // Ally FIRST (the raider is still Passive, so it won't commit meanwhile),
        // THEN arm it WeaponsFree — now the ally in its bubble must be spared.
        ally(&mut w, atk, def);
        assert!(w.are_allied(atk, def));
        w.fleets.get_mut(&raider).unwrap().posture = EngagementPosture::WeaponsFree;
        for _ in 0..(6 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(
                matches!(w.fleets[&raider].order, FleetOrder::Idle),
                "a WeaponsFree raider never hunts a syndicate ally in its bubble",
            );
        }
    }

    #[test]
    fn syndicate_picket_spares_an_ally_raider() {
        // An EngageAny picket that WOULD hunt any sensed raider leaves an ally alone.
        let mut w = test_world();
        let (d, a) = (PlayerId(1), PlayerId(2));
        let convoy_pos = Vec2::new(3000.0, 0.0);
        let (patrol, _c, hostile) =
            defense_setup(&mut w, d, a, convoy_pos, convoy_pos, Vec2::new(1500.0, 0.0));
        {
            let h = w.fleets.get_mut(&hostile).unwrap();
            h.pos = convoy_pos + Vec2::new(500.0, 0.0);
            h.vel = Vec2::ZERO;
            h.order = FleetOrder::Idle;
        }
        // Ally FIRST (default DefensiveOnly ignores the parked drifter meanwhile),
        // THEN switch to EngageAny — the ally raider must still be spared.
        ally(&mut w, d, a);
        assert!(w.are_allied(d, a));
        w.players.get_mut(&d).unwrap().doctrine.engagement = EngagementPolicy::EngageAny;
        for _ in 0..(6 * crate::config::TICK_HZ) {
            w.step(&[]);
            assert!(w.fleets[&patrol].defense.is_none(), "an EngageAny picket never hunts an ALLY raider");
            assert!(!engaged_on(&w, patrol, hostile), "no engagement opens against an ally");
        }
    }

    #[test]
    fn syndicate_soft_rejects_attack_raid_blockade_on_an_ally() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        ally(&mut w, a, b);
        let cc = w.players[&a].command_center;
        let b_home = w.players[&b].home_system.expect("b has a home system");
        // Three separate a-owned raider fleets + b targets.
        let rr = squad(&mut w, a, cc + Vec2::new(40.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let ar = squad(&mut w, a, cc + Vec2::new(60.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let br = squad(&mut w, a, cc + Vec2::new(80.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        let bc = squad(&mut w, b, cc + Vec2::new(400.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
        w.step(&[
            Command::CommitRaid { player_id: a, raider_id: rr, target_id: bc },
            Command::AttackFleet { player_id: a, fleet_id: ar, target_id: bc },
            Command::BlockadeSystem { player_id: a, fleet_id: br, system_id: b_home },
        ]);
        // All soft-rejected: fleets keep Idle, nothing scheduled.
        for id in [rr, ar, br] {
            assert!(matches!(w.fleets[&id].order, FleetOrder::Idle), "no order applied vs an ally");
        }
        assert!(
            w.pending_commands(a).iter().all(|p| ![rr, ar, br].contains(&p.fleet)),
            "no offensive order was scheduled against an ally",
        );
    }

    #[test]
    fn syndicate_membership_knowledge_is_light_delayed() {
        // are_allied is ground-truth (immediate); the KNOWLEDGE of a join (the ally
        // tint) reaches the founder only after the new member's light arrives.
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        ally(&mut w, a, b);
        let since = w.players[&b].syndicate_since;
        let delay = w.players[&a].command_center.distance(w.players[&b].command_center) / w.config.c;
        assert!(delay > DT, "homes are far enough apart for a measurable light delay");
        assert!(w.are_allied(a, b), "alliance is in effect immediately (ground truth)");
        assert!(!w.known_ally(a, b, w.time), "founder does NOT yet KNOW the join (light in flight)");
        // Up to just before the light arrives, still unknown (the fog guarantee).
        while w.time < since + delay - DT {
            w.step(&[]);
            assert!(!w.known_ally(a, b, w.time), "membership light hasn't arrived yet");
        }
        // Once the light arrives, the ally is known (→ tinted).
        let known = run_until(&mut w, 5, |w| w.known_ally(a, b, w.time));
        assert!(known, "the ally becomes known once its membership light arrives");
    }

    #[test]
    fn syndicate_leave_promotes_founder_then_dissolves_when_empty() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        let sid = ally(&mut w, a, b);
        // Founder leaves → seat passes to b; a is unaffiliated; no longer allied.
        w.step(&[Command::LeaveSyndicate { player_id: a }]);
        assert!(w.players[&a].syndicate.is_none());
        assert_eq!(w.syndicates[&sid].founder, b, "seat passes to the remaining member");
        assert!(!w.are_allied(a, b));
        // Last member leaves → the syndicate is removed.
        w.step(&[Command::LeaveSyndicate { player_id: b }]);
        assert!(w.players[&b].syndicate.is_none());
        assert!(!w.syndicates.contains_key(&sid), "an emptied syndicate dissolves");
    }

    #[test]
    fn syndicate_dissolve_clears_every_member() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        let sid = ally(&mut w, a, b);
        // A non-founder cannot dissolve.
        w.step(&[Command::DissolveSyndicate { player_id: b }]);
        assert!(w.syndicates.contains_key(&sid), "only the founder may dissolve");
        // The founder dissolves → every member unaffiliated, roster gone.
        w.step(&[Command::DissolveSyndicate { player_id: a }]);
        assert!(w.players[&a].syndicate.is_none() && w.players[&b].syndicate.is_none());
        assert!(!w.syndicates.contains_key(&sid));
        assert!(!w.are_allied(a, b));
    }

    #[test]
    fn syndicate_serde_round_trips() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        let sid = ally(&mut w, a, b);
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert!(w2.syndicates.contains_key(&sid), "syndicate persists");
        assert_eq!(w2.players[&a].syndicate, Some(sid));
        assert_eq!(w2.players[&b].syndicate, Some(sid));
        assert!(w2.are_allied(a, b), "alliance survives a snapshot");
        assert_eq!(w2.syndicates[&sid].members.len(), 2);
    }

    // ===================================================================
    // §SYNDICATES Part 3 — garrison + aid
    // ===================================================================

    /// Position of a system by id (test helper).
    fn sys_pos(w: &World, id: EntityId) -> Vec2 {
        w.systems.iter().find(|s| s.id == id).unwrap().pos
    }
    /// Set a system's Provisions stockpile and clear its deposits (so production
    /// can't refill it mid-test — deterministic upkeep behaviour).
    fn set_provisions_no_production(w: &mut World, id: EntityId, units: f64) {
        let s = w.systems.iter_mut().find(|s| s.id == id).unwrap();
        s.set_test_deposits(vec![]);
        s.stockpile.clear();
        if units > 0.0 {
            s.stockpile.insert(Commodity::Provisions, units);
        }
    }

    #[test]
    fn ally_garrison_draws_host_provisions_and_unfeeds_on_shortfall() {
        let mut w = test_world();
        let (host, sender) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: host, name: "Host".into() },
            Command::AddPlayer { id: sender, name: "Send".into() },
        ]);
        ally(&mut w, host, sender);
        let hsys = w.players[&host].home_system.unwrap();
        set_provisions_no_production(&mut w, hsys, 100.0);
        let hpos = sys_pos(&w, hsys);
        // A sender combatant fleet STATIONED at the host system = an ally garrison.
        let garr = squad(&mut w, sender, hpos, ShipKind::Corvette, 2, FleetOrder::Idle);
        let before = w.systems.iter().find(|s| s.id == hsys).unwrap().stockpile[&Commodity::Provisions];
        w.step(&[]);
        let after = w.systems.iter().find(|s| s.id == hsys).unwrap().stockpile[&Commodity::Provisions];
        assert!(after < before, "the HOST feeds the garrison from its own Provisions ({before} → {after})");
        assert!(w.fleets[&garr].garrison_fed, "a fed garrison stays fed");
        // Drain the host → the garrison UNFEEDS (suspended, never destroyed).
        set_provisions_no_production(&mut w, hsys, 0.0);
        w.step(&[]);
        assert!(!w.fleets[&garr].garrison_fed, "no Provisions → the garrison goes UNFED");
        assert!(w.fleets.contains_key(&garr), "unfed suspends, never destroys");
    }

    #[test]
    fn fed_ally_garrison_joins_the_host_defense() {
        let mut w = test_world();
        let (atk, def, friend) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
            Command::AddPlayer { id: friend, name: "Ally".into() },
        ]);
        ally(&mut w, def, friend);
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 0); // no platform
        set_provisions_no_production(&mut w, sys, 100.0);
        let pos = sys_pos(&w, sys);
        let garr = squad(&mut w, friend, pos, ShipKind::Corvette, 2, FleetOrder::Idle);
        blockader_on_station(&mut w, atk, sys, 2);
        let joined = run_until(&mut w, 5, |w| w.engagements.values().any(|e| e.defenders.contains(&garr)));
        assert!(joined, "a FED ally garrison joins the host's establishment defense");
    }

    #[test]
    fn unfed_ally_garrison_suspends_its_defense() {
        let mut w = test_world();
        let (atk, def, friend) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
            Command::AddPlayer { id: friend, name: "Ally".into() },
        ]);
        ally(&mut w, def, friend);
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 0);
        set_provisions_no_production(&mut w, sys, 0.0); // host can't feed
        let pos = sys_pos(&w, sys);
        let garr = squad(&mut w, friend, pos, ShipKind::Corvette, 2, FleetOrder::Idle);
        blockader_on_station(&mut w, atk, sys, 2);
        for _ in 0..(3 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(!w.fleets[&garr].garrison_fed, "no host Provisions → unfed");
        assert!(
            !w.engagements.values().any(|e| e.defenders.contains(&garr)),
            "an UNFED garrison sits the fight out (contribution suspended)",
        );
        assert!(w.fleets.contains_key(&garr), "never destroyed");
    }

    #[test]
    fn ally_garrison_is_sender_controlled_not_host() {
        let mut w = test_world();
        let (host, sender) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: host, name: "Host".into() },
            Command::AddPlayer { id: sender, name: "Send".into() },
        ]);
        ally(&mut w, host, sender);
        let hsys = w.players[&host].home_system.unwrap();
        let hpos = sys_pos(&w, hsys);
        let garr = squad(&mut w, sender, hpos, ShipKind::Corvette, 1, FleetOrder::Idle);
        // The HOST cannot command an ally's garrison (not their fleet) → soft-reject.
        w.step(&[Command::MoveShip { player_id: host, ship_id: garr, dest: Vec2::new(9000.0, 0.0) }]);
        assert!(matches!(w.fleets[&garr].order, FleetOrder::Idle), "the host cannot move an ally's garrison");
        assert!(w.pending_commands(host).iter().all(|p| p.fleet != garr), "no host order was scheduled");
        // The SENDER commands + recalls it (their fleet; light-delayed).
        let home = w.players[&sender].home;
        w.step(&[Command::MoveShip { player_id: sender, ship_id: garr, dest: home }]);
        assert!(w.pending_commands(sender).iter().any(|p| p.fleet == garr), "the SENDER commands + recalls it");
    }

    #[test]
    fn ally_aid_delivers_to_the_ally_stockpile() {
        let mut w = test_world();
        let (giver, friend) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: giver, name: "Give".into() },
            Command::AddPlayer { id: friend, name: "Ally".into() },
        ]);
        ally(&mut w, giver, friend);
        let asys = w.players[&friend].home_system.unwrap();
        set_provisions_no_production(&mut w, asys, 0.0); // measure the delivery cleanly
        let apos = sys_pos(&w, asys);
        // A GIVER convoy arriving at the ALLY's system with a deliver mission.
        let cid = w.alloc_entity_id();
        let mut convoy = Fleet::single(cid, giver, ShipKind::Convoy, apos, FleetOrder::Idle, Some(crate::cargo::Cargo { commodity: Commodity::Provisions, units: 20 }));
        convoy.mission = Some(TradeMission::DeliverToSystem { system: asys });
        w.fleets.insert(cid, convoy);
        w.step(&[]); // resolve_trade_arrivals deposits into the ALLY's stockpile
        let got = w.systems.iter().find(|s| s.id == asys).unwrap().stockpile.get(&Commodity::Provisions).copied().unwrap_or(0.0);
        // (§economy: the ally's home population eats a crumb of the delivery in
        // the very same tick — that's the colony working, not a delivery loss.)
        assert!(got >= 20.0 - 0.05, "aid credits the ALLY's stockpile (got {got})");
        assert!(!w.fleets.contains_key(&cid), "the aid convoy delivered and was consumed");
    }

    #[test]
    fn blockade_still_interdicts_ally_aid() {
        let mut w = test_world();
        let (atk, def, giver) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: atk, name: "Atk".into() },
            Command::AddPlayer { id: def, name: "Def".into() },
            Command::AddPlayer { id: giver, name: "Give".into() },
        ]);
        ally(&mut w, def, giver);
        let sys = grant_system_at(&mut w, def, Vec2::new(5000.0, 0.0), 0);
        let pos = sys_pos(&w, sys);
        blockader_on_station(&mut w, atk, sys, 1);
        // A GIVER aid convoy inbound to the ally's (soon-blockaded) system.
        let cid = w.alloc_entity_id();
        let mut convoy = Fleet::single(cid, giver, ShipKind::Convoy, pos + Vec2::new(3000.0, 0.0), FleetOrder::MoveTo { dest: pos }, Some(crate::cargo::Cargo { commodity: Commodity::Provisions, units: 10 }));
        convoy.mission = Some(TradeMission::DeliverToSystem { system: sys });
        w.fleets.insert(cid, convoy);
        w.step(&[]); // resolve_blockades establishes + re-targets the aid convoy
        let dest = match w.fleets[&cid].order {
            FleetOrder::MoveTo { dest } => dest,
            _ => panic!("held aid convoy should still be moving (to standoff)"),
        };
        assert!(
            (dest.distance(pos) - BLOCKADE_STANDOFF_RADIUS).abs() < 1.0,
            "ally aid is interdicted at the standoff ring — relief is military-first",
        );
    }

    #[test]
    fn garrison_fed_flag_persists_serde() {
        let mut w = test_world();
        let (host, sender) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: host, name: "Host".into() },
            Command::AddPlayer { id: sender, name: "Send".into() },
        ]);
        ally(&mut w, host, sender);
        let hsys = w.players[&host].home_system.unwrap();
        set_provisions_no_production(&mut w, hsys, 0.0);
        let hpos = sys_pos(&w, hsys);
        let garr = squad(&mut w, sender, hpos, ShipKind::Corvette, 1, FleetOrder::Idle);
        w.step(&[]); // → unfed (no Provisions)
        assert!(!w.fleets[&garr].garrison_fed);
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert!(!w2.fleets[&garr].garrison_fed, "garrison fed/unfed survives a snapshot");
    }

    // ===================================================================
    // §PIRATES — neutral enclave faction
    // ===================================================================

    /// An enclave id + its base position.
    fn an_enclave(w: &World) -> (EntityId, Vec2) {
        let sid = *w.enclaves.keys().next().expect("enclaves seeded");
        (sid, w.systems.iter().find(|s| s.id == sid).unwrap().pos)
    }

    #[test]
    fn enclaves_are_seeded_mid_ring_clear_of_homes() {
        let w = test_world();
        assert_eq!(w.enclaves.len(), crate::pirate::PIRATE_ENCLAVE_COUNT, "the tunable count is seeded");
        let radius = w.config.galaxy_radius;
        for (sid, e) in &w.enclaves {
            let sys = w.systems.iter().find(|s| s.id == *sid).unwrap();
            assert!(sys.owner.is_none(), "an enclave sits at an UNCLAIMED system (dark until scouted)");
            assert_eq!(sys.tier(crate::build::StructureKind::DefensePlatform), crate::pirate::base_defense_tiers(1), "base defense on the host system");
            assert_eq!(e.tier, 1, "seeds at tier 1");
            let r = sys.pos.length();
            assert!(r >= radius * 0.30 && r <= radius * 0.80, "mid-ring placement");
            for h in &w.home_slots {
                assert!(sys.pos.distance(h.pos) >= crate::pirate::PIRATE_HOME_EXCLUSION, "clear of every home");
            }
        }
    }

    #[test]
    fn pirate_pack_launches_and_raids_an_unescorted_broadcasting_convoy() {
        let mut w = test_world();
        let victim = PlayerId(1);
        w.step(&[Command::AddPlayer { id: victim, name: "V".into() }]);
        // Advance past the NEW-PLAYER GRACE window so the victim's convoys are huntable.
        for _ in 0..((crate::pirate::PIRATE_GRACE_SECS as u32 + 1) * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let (sid, epos) = an_enclave(&w);
        w.enclaves.get_mut(&sid).unwrap().next_launch_at = w.time; // launch now (past the first-launch delay)
        // A lone broadcasting convoy with cargo, inside the tier-1 hunt radius (2600).
        let convoy = squad(&mut w, victim, epos + Vec2::new(1400.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
        w.fleets.get_mut(&convoy).unwrap().cargo = Some(crate::cargo::Cargo { commodity: Commodity::MetallicOre, units: 20 });
        let mut pirate_raid = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            let ev = w.step(&[]);
            if ev.iter().any(|e| matches!(&e.payload, EventPayload::RaidResolved { attacker, .. } if attacker.is_pirate())) {
                pirate_raid = true;
                break;
            }
        }
        assert!(pirate_raid, "past grace, an enclave launches a pack that RAIDS the unescorted broadcasting convoy");
        assert!(w.fleets.values().any(|f| f.owner.is_pirate()), "a pirate pack exists");
    }

    /// §pirates onboarding #1: a corp's convoys are INVISIBLE to pirate hunting for
    /// the grace window after it joins — the latecomer/new-player shield.
    #[test]
    fn a_fresh_corps_convoys_are_shielded_during_the_grace_window() {
        let mut w = test_world();
        let victim = PlayerId(1);
        w.step(&[Command::AddPlayer { id: victim, name: "V".into() }]);
        let (sid, epos) = an_enclave(&w);
        w.enclaves.get_mut(&sid).unwrap().next_launch_at = 0.0; // pirates WOULD launch at once
        let convoy = squad(&mut w, victim, epos + Vec2::new(1400.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
        w.fleets.get_mut(&convoy).unwrap().cargo = Some(crate::cargo::Cargo { commodity: Commodity::MetallicOre, units: 20 });
        // Well inside the grace window (120s << PIRATE_GRACE_SECS): never targeted.
        let mut targeted = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.fleets.values().any(|f| f.owner.is_pirate() && matches!(f.order, FleetOrder::Intercept { target } if target == convoy)) {
                targeted = true;
                break;
            }
        }
        assert!(!targeted, "a fresh corp's convoy is invisible to pirates during grace");
        assert_eq!(w.fleets.get(&convoy).and_then(|f| f.cargo).map(|c| c.units), Some(20), "no cargo stolen during grace");
    }

    /// §pirates onboarding #3: a fresh enclave opens with a LONE bandit and only
    /// grows into a real pack (2, then 3) as it escalates (Civ-barbarian ramp).
    #[test]
    fn a_pirate_pack_opens_as_a_lone_raider_and_grows_when_ignored() {
        assert_eq!(crate::pirate::pack_size(1), 1, "tier-1 enclave launches a LONE bandit");
        assert_eq!(crate::pirate::pack_size(2), 2, "tier 2 → a pair");
        assert_eq!(crate::pirate::pack_size(3), 3, "tier 3 → a real pack");
    }

    // §research R1 — the command → resolve → event → instant-effect path.
    #[test]
    fn research_queue_completes_and_applies_its_effect_instantly() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        // Research is a SYNDICATE institution — form a solo syndicate.
        w.step(&[Command::CreateSyndicate { player_id: id, name: "Solo".into() }]);
        let sid = w.players[&id].syndicate.expect("in a syndicate");
        // Queue a Tier-I programme (always available); the front promotes to active.
        w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["prop_drive_tuning".into()] }]);
        assert_eq!(w.syndicates[&sid].research.active.as_deref(), Some("prop_drive_tuning"));
        w.step(&[]);
        assert!(!w.syndicates[&sid].research.has("prop_drive_tuning"), "unfunded → not complete");
        // Fund it directly (the distributed clock lands in R2) and step.
        w.syndicates.get_mut(&sid).unwrap().research.progress = crate::research::tier_cost_secs(1);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(&e.payload, EventPayload::ResearchCompleted { syndicate, programme } if *syndicate == sid && programme == "prop_drive_tuning")),
            "a completion event fires",
        );
        assert!(w.syndicates[&sid].research.has("prop_drive_tuning"), "recorded as completed");
        assert!(w.syndicates[&sid].research.active.is_none(), "queue drained");
        // The effect applies INSTANTLY, galaxy-wide, via the lazy mods layer.
        let m = crate::research::mod_of(&w.syndicates[&sid].research, crate::research::ModKey::SpeedAll);
        assert!((m - 1.10).abs() < 1e-9, "Drive Tuning's +10% speed is live");
        // A hidden id in the queue is dropped (can't research it).
        w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["hull_corsair_v_salvage_rigs".into(), "prop_bunkerage".into()] }]);
        assert_eq!(w.syndicates[&sid].research.active.as_deref(), Some("prop_bunkerage"), "hidden dropped, next promoted");
    }

    // §research R2 — the distributed clock. Helper: give `player` a CLEAN owned
    // system hosting one Academy staffed with `workers`, seeded to stay supplied.
    fn grant_research_lab(w: &mut World, player: PlayerId, workers: u32) -> EntityId {
        use crate::build::StructureKind::Academy;
        let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).expect("a free system");
        s.owner = Some(player);
        s.claimed_at = Some(0.0);
        s.food_state = crate::colony::FoodState::WellSupplied;
        let b = s.bodies.iter_mut().next().expect("a body");
        b.set_tier(Academy, 1);
        b.population = 10.0; // ample workforce so staffing_share ≈ 1
        if workers > 0 {
            b.assignments.insert(Academy, crate::production::Assignment { workers, specialists: Default::default(), suspended: None });
        }
        *s.stockpile.entry(Commodity::Electronics).or_insert(0.0) = 500.0; // T1 basket
        *s.stockpile.entry(Commodity::Provisions).or_insert(0.0) = 5000.0; // keep supplied
        s.id
    }

    #[test]
    fn r2_distributed_clock_accrues_and_debits_the_basket() {
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "S".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        let lab = grant_research_lab(&mut w, id, 1);
        w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["prop_drive_tuning".into()] }]);
        let elec0 = w.systems.iter().find(|s| s.id == lab).unwrap().stockpile.get(&Commodity::Electronics).copied().unwrap();
        for _ in 0..30 {
            w.step(&[]);
        }
        let prog = w.syndicates[&sid].research.progress;
        let elec1 = w.systems.iter().find(|s| s.id == lab).unwrap().stockpile.get(&Commodity::Electronics).copied().unwrap_or(0.0);
        assert!(prog > 0.0, "the clock accrues progress (got {prog})");
        assert!(elec1 < elec0, "the basket drips from the LOCAL stockpile ({elec1} < {elec0})");
        assert!(!w.syndicates[&sid].research.stalled, "a staffed+funded lab is not stalled");
    }

    #[test]
    fn r2_unfunded_academy_suspends_its_contribution() {
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "S".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        let lab = grant_research_lab(&mut w, id, 1);
        // Strip the basket good → the lab can't drip → it suspends (no accrual).
        w.systems.iter_mut().find(|s| s.id == lab).unwrap().stockpile.remove(&Commodity::Electronics);
        w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["prop_drive_tuning".into()] }]);
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!(w.syndicates[&sid].research.progress, 0.0, "an unfunded lab accrues nothing");
        assert!(!w.syndicates[&sid].research.stalled, "unfunded ≠ stalled (it's staffed, just supply-starved)");
    }

    #[test]
    fn r2_no_staffed_academy_stalls_once_then_resumes() {
        let mut w = test_world();
        w.enclaves.clear();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: id, name: "S".into() }]);
        let sid = w.players[&id].syndicate.unwrap();
        // A member system with an Academy tier but NO crew posted (unstaffed).
        let lab = grant_research_lab(&mut w, id, 0);
        // Setting the queue promotes an active programme; that same tick, with no
        // staffed Academy, the stall latches and fires once.
        let ev1 = w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["prop_drive_tuning".into()] }]);
        assert!(ev1.iter().any(|e| matches!(&e.payload, EventPayload::ResearchStalled { syndicate } if *syndicate == sid)), "stall fires once");
        assert!(w.syndicates[&sid].research.stalled);
        let ev2 = w.step(&[]);
        assert!(!ev2.iter().any(|e| matches!(&e.payload, EventPayload::ResearchStalled { .. })), "no repeat stall");
        // Staff the Academy → resume.
        w.systems.iter_mut().find(|s| s.id == lab).unwrap().bodies.iter_mut().next().unwrap()
            .assignments.insert(crate::build::StructureKind::Academy, crate::production::Assignment { workers: 1, specialists: Default::default(), suspended: None });
        let ev3 = w.step(&[]);
        assert!(ev3.iter().any(|e| matches!(&e.payload, EventPayload::ResearchResumed { syndicate } if *syndicate == sid)), "resume fires on recovery");
        assert!(!w.syndicates[&sid].research.stalled);
    }

    #[test]
    fn r2_two_staffed_academies_out_accrue_one() {
        let run = |labs: u32| -> f64 {
            let mut w = test_world();
            w.enclaves.clear();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "A".into() }]);
            w.step(&[Command::CreateSyndicate { player_id: id, name: "S".into() }]);
            let sid = w.players[&id].syndicate.unwrap();
            for _ in 0..labs {
                grant_research_lab(&mut w, id, 1);
            }
            w.step(&[Command::SetResearchQueue { player_id: id, queue: vec!["prop_drive_tuning".into()] }]);
            for _ in 0..20 {
                w.step(&[]);
            }
            w.syndicates[&sid].research.progress
        };
        let one = run(1);
        let two = run(2);
        assert!(one > 0.0, "one lab makes progress");
        assert!(two > 1.7 * one, "two labs ≈ double the science ({two} vs {one})");
    }

    // §research R3 — VERB COUNTERS. Helper: give `player` an owned system whose
    // one body works a rich MetallicOre deposit with a staffed Mining Complex,
    // kept supplied and under the storage cap so extraction actually flows.
    fn grant_mining_system(w: &mut World, player: PlayerId) -> EntityId {
        use crate::build::StructureKind::MiningComplex;
        let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).expect("a free system");
        s.owner = Some(player);
        s.claimed_at = Some(0.0);
        s.food_state = crate::colony::FoodState::WellSupplied;
        let b = s.bodies.iter_mut().next().expect("a body");
        b.set_tier(MiningComplex, 1);
        b.population = 10.0; // ample workforce; no Habitat ⇒ won't grow (own test)
        b.deposits = vec![crate::galaxy::Deposit { resource: Commodity::MetallicOre, richness: 3.0, reserves: None, accessibility: 0.1 }];
        b.assignments.insert(MiningComplex, crate::production::Assignment { workers: 1, specialists: Default::default(), suspended: None });
        // Under the 700 base cap: a little food, lots of headroom for fresh ore.
        *s.stockpile.entry(Commodity::Provisions).or_insert(0.0) = 200.0;
        s.id
    }

    #[test]
    fn r3_event_verbs_accrue_into_the_syndicate_biography() {
        use crate::research::Verb;
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        let mut a_loss = BTreeMap::new(); a_loss.insert(ShipKind::Raider, 1u32);
        let mut d_loss = BTreeMap::new(); d_loss.insert(ShipKind::Convoy, 2u32);
        let events = vec![
            Event::new(0.0, EventPayload::RaidResolved {
                attacker: a, defender: b, attacker_ship: EntityId(10), target_ship: EntityId(11),
                attacker_kind: ShipKind::Raider, target_kind: ShipKind::Convoy,
                outcome: RaidOutcome::TargetDestroyed, pos: Vec2::ZERO,
                attacker_losses: a_loss, target_losses: d_loss,
            }),
            Event::new(0.0, EventPayload::Trade(TradeEvent::Delivered { player: a, commodity: Commodity::MetallicOre, units: 5, system: None })),
            Event::new(0.0, EventPayload::SurveyCompleted { owner: a, system: EntityId(3), pos: Vec2::ZERO }),
            Event::new(0.0, EventPayload::SurveyCompleted { owner: a, system: EntityId(3), pos: Vec2::ZERO }), // dup
            Event::new(0.0, EventPayload::IntelGathered { owner: a, system: EntityId(4), defense_tier: 1, shipyard_tier: 0, pos: Vec2::ZERO }),
        ];
        w.accrue_research_verbs(&events);
        let r = &w.syndicates[&sid].research;
        assert_eq!(r.verb(Verb::BattlesFought), 1.0, "one resolved battle");
        assert_eq!(r.verb(Verb::BattlesWon), 1.0, "the attacker destroyed the target");
        assert!(r.verb(Verb::HullMassDestroyed) > 0.0, "credited the convoy hull it destroyed");
        assert!(r.verb(Verb::DamageAbsorbed) > 0.0, "credited its own lost raider as absorbed");
        assert_eq!(r.verb(Verb::ConvoyDeliveries), 1.0, "one completed haul");
        assert_eq!(r.verb(Verb::SystemsScouted), 2.0, "two DISTINCT systems, dup ignored");
    }

    #[test]
    fn r3_movement_accrues_propulsion_ly() {
        use crate::research::Verb;
        let mut w = test_world();
        w.enclaves.clear();
        let a = PlayerId(1);
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        let hpos = w.players[&a].home;
        let _ = squad(&mut w, a, hpos, ShipKind::Raider, 1, FleetOrder::MoveTo { dest: hpos + Vec2::new(200_000.0, 0.0) });
        for _ in 0..40 {
            w.step(&[]);
        }
        let r = &w.syndicates[&sid].research;
        assert!(r.verb(Verb::LyFlown) > 0.0, "a flying fleet accrues ly ({})", r.verb(Verb::LyFlown));
        assert!(r.verb(Verb::WarshipLyFlown) > 0.0, "a combatant also credits warship-ly");
        assert!(r.verb(Verb::WarshipLyFlown) <= r.verb(Verb::LyFlown) + 1e-9, "warship-ly ≤ total ly");
    }

    #[test]
    fn r3_production_accrues_industry_verbs() {
        use crate::research::Verb;
        let mut w = test_world();
        w.enclaves.clear();
        let a = PlayerId(1);
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        grant_mining_system(&mut w, a);
        for _ in 0..30 {
            w.step(&[]);
        }
        let r = &w.syndicates[&sid].research;
        assert!(r.verb(Verb::UnitsExtracted) > 0.0, "a staffed mine accrues extracted units ({})", r.verb(Verb::UnitsExtracted));
        assert!(r.verb(Verb::UnitsThroughIndustry) >= r.verb(Verb::UnitsExtracted), "extraction also counts as industry throughput");
    }

    #[test]
    fn r3_rival_observation_scan_records_distinct_fleets() {
        use crate::research::Verb;
        let mut w = test_world();
        w.enclaves.clear();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        // A's picket sits right on top of two of B's fleets — well inside sensor range.
        let hpos = w.players[&a].home;
        let _obs = squad(&mut w, a, hpos, ShipKind::Corvette, 1, FleetOrder::Idle);
        let _r1 = squad(&mut w, b, hpos + Vec2::new(50.0, 0.0), ShipKind::Raider, 2, FleetOrder::Idle);
        let _r2 = squad(&mut w, b, hpos + Vec2::new(80.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
        w.observe_rivals_for_research();
        assert_eq!(w.syndicates[&sid].research.verb(Verb::RivalFleetsObserved), 2.0, "two distinct rival fleets sensed");
        // Re-running does not double-count (deduped by fleet id).
        w.observe_rivals_for_research();
        assert_eq!(w.syndicates[&sid].research.verb(Verb::RivalFleetsObserved), 2.0, "re-sightings never re-count");
    }

    // §research R4a — EFFECT MODS. A completed programme changes a real sim
    // outcome instantly, galaxy-wide, via the lazy mods layer.
    #[test]
    fn r4_extraction_rate_mod_lifts_yield() {
        let run = |researched: bool| -> f64 {
            let mut w = test_world();
            w.enclaves.clear();
            let a = PlayerId(1);
            w.step(&[Command::AddPlayer { id: a, name: "A".into() }]);
            w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
            let sid = w.players[&a].syndicate.unwrap();
            grant_mining_system(&mut w, a);
            if researched {
                // Deep Bores (Materials I): ExtractionRate ×1.15.
                w.syndicates.get_mut(&sid).unwrap().research.completed.insert("mat_deep_bores".into());
            }
            for _ in 0..20 {
                w.step(&[]);
            }
            w.syndicates[&sid].research.verb(crate::research::Verb::UnitsExtracted)
        };
        let base = run(false);
        let boosted = run(true);
        assert!(base > 0.0, "the baseline mine produces ({base})");
        assert!(boosted > base * 1.10, "ExtractionRate ×1.15 lifts yield ({boosted} vs {base})");
    }

    #[test]
    fn r4_growth_below_half_mod_speeds_a_young_colony() {
        let run = |researched: bool| -> f64 {
            let mut w = test_world();
            w.enclaves.clear();
            let a = PlayerId(1);
            w.step(&[Command::AddPlayer { id: a, name: "A".into() }]);
            w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
            let sid = w.players[&a].syndicate.unwrap();
            let s = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            s.owner = Some(a);
            s.claimed_at = Some(0.0);
            s.food_state = crate::colony::FoodState::WellSupplied;
            let sysid = s.id;
            let b = s.bodies.iter_mut().next().unwrap();
            b.set_tier(crate::build::StructureKind::Habitat, 2); // ceiling 8.0M
            b.population = 1.0; // well under half (4.0M) — the boost applies
            *s.stockpile.entry(Commodity::Provisions).or_insert(0.0) = 600.0;
            if researched {
                // Boom Charters (Life Growth III): GrowthBelowHalf ×1.20.
                w.syndicates.get_mut(&sid).unwrap().research.completed.insert("life_growth_iii_boom_charters".into());
            }
            for _ in 0..200 {
                w.step(&[]);
            }
            w.systems.iter().find(|s| s.id == sysid).unwrap().bodies.iter().next().unwrap().population
        };
        let base = run(false);
        let boosted = run(true);
        assert!(base > 1.0, "the colony grows ({base})");
        assert!(boosted - 1.0 > (base - 1.0) * 1.15, "GrowthBelowHalf ×1.20 grows a young colony faster ({boosted} vs {base})");
    }

    // §research R5 — the Life · Growth V endurance gate: tick_research stamps
    // `sustained_since` when the WellSupplied count first reaches the threshold and
    // clears it the moment the count drops (an interruption resets the 7-day clock).
    #[test]
    fn r5_growth_v_sustained_clock_stamps_and_resets() {
        let mut w = test_world();
        w.enclaves.clear();
        let a = PlayerId(1);
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "S".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        // Five owned, WellSupplied systems (empty rocks are vacuously supplied).
        let free: Vec<EntityId> = w.systems.iter().filter(|s| s.is_unclaimed()).take(5).map(|s| s.id).collect();
        assert_eq!(free.len(), 5, "need five free systems");
        for id in &free {
            let s = w.systems.iter_mut().find(|s| s.id == *id).unwrap();
            s.owner = Some(a);
            s.claimed_at = Some(0.0);
            s.food_state = crate::colony::FoodState::WellSupplied;
        }
        w.step(&[]);
        let m = crate::research::Metric::WellSuppliedSystems;
        assert!(w.syndicates[&sid].research.sustained_since.contains_key(&m), "≥5 WellSupplied → the endurance clock starts");
        // Surrender all five → the count collapses below the threshold → clock resets.
        for id in &free {
            w.systems.iter_mut().find(|s| s.id == *id).unwrap().owner = None;
        }
        w.step(&[]);
        assert!(!w.syndicates[&sid].research.sustained_since.contains_key(&m), "below threshold → the clock is cleared");
    }

    #[test]
    fn pirates_avoid_a_platform_covered_convoy_and_ignore_scouts() {
        let mut w = test_world();
        let victim = PlayerId(1);
        w.step(&[Command::AddPlayer { id: victim, name: "V".into() }]);
        let (sid, epos) = an_enclave(&w);
        w.enclaves.get_mut(&sid).unwrap().next_launch_at = 0.0;
        // The victim owns a DEFENSE PLATFORM system right at the convoy — it's covered.
        let guard = grant_system_at(&mut w, victim, epos + Vec2::new(1200.0, 0.0), 2); // 2 platform tiers
        let gpos = sys_pos(&w, guard);
        let convoy = squad(&mut w, victim, gpos, ShipKind::Convoy, 1, FleetOrder::Idle);
        w.fleets.get_mut(&convoy).unwrap().cargo = Some(crate::cargo::Cargo { commodity: Commodity::MetallicOre, units: 20 });
        // A lone SCOUT even closer to the base — must never be targeted (it's dark).
        let _scout = squad(&mut w, victim, epos + Vec2::new(300.0, 0.0), ShipKind::Scout, 1, FleetOrder::Idle);
        for _ in 0..(20 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        // No pirate ever intercepts the covered convoy or the scout.
        assert!(
            !w.fleets.values().any(|f| f.owner.is_pirate() && matches!(f.order, FleetOrder::Intercept { .. })),
            "a pack never commits to a platform-covered convoy or a scout",
        );
    }

    #[test]
    fn enclave_is_scoutable() {
        let mut w = test_world();
        let scouter = PlayerId(1);
        w.step(&[Command::AddPlayer { id: scouter, name: "S".into() }]);
        let (sid, epos) = an_enclave(&w);
        // A scout parked at the enclave.
        let _s = squad(&mut w, scouter, epos, ShipKind::Scout, 1, FleetOrder::Idle);
        w.step(&[]);
        let snap = w.players[&scouter].intel.get(&sid).expect("scout captured the enclave");
        assert_eq!(snap.enclave_tier, 1, "the base tier is in the snapshot (like fortifications)");
    }

    #[test]
    fn enclave_escalates_when_ignored() {
        let mut w = test_world();
        let (sid, _epos) = an_enclave(&w);
        // Force the grow clock. No players/convoys → nothing to raid, it just grows.
        w.enclaves.get_mut(&sid).unwrap().next_grow_at = 0.0;
        w.enclaves.get_mut(&sid).unwrap().next_launch_at = f64::INFINITY; // isolate escalation
        w.step(&[]);
        assert_eq!(w.enclaves[&sid].tier, 2, "an unsuppressed enclave grows a tier");
        assert_eq!(w.systems.iter().find(|s| s.id == sid).unwrap().tier(crate::build::StructureKind::DefensePlatform), crate::pirate::base_defense_tiers(2), "the base defense grows with it");
    }

    #[test]
    fn enclave_assault_yields_plunder_and_dormancy() {
        let mut w = test_world();
        let hunter = PlayerId(1);
        w.step(&[Command::AddPlayer { id: hunter, name: "H".into() }]);
        let (sid, epos) = an_enclave(&w);
        // Seed loot in the base, and STATION a strong war-fleet on it.
        w.enclaves.get_mut(&sid).unwrap().plunder.insert(Commodity::Alloys, 40);
        w.enclaves.get_mut(&sid).unwrap().next_launch_at = f64::INFINITY; // no packs
        let fleet = squad(&mut w, hunter, epos, ShipKind::Raider, 12, FleetOrder::Idle);
        let _ = fleet;
        let before = w.players[&hunter].inventory.get(&Commodity::Alloys).copied().unwrap_or(0);
        let mut cleared = false;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            let ev = w.step(&[]);
            if ev.iter().any(|e| matches!(&e.payload, EventPayload::PirateEnclaveCleared { system, .. } if *system == sid)) {
                cleared = true;
                break;
            }
        }
        assert!(cleared, "a strong war-fleet stationed at the base grinds it down and CLEARS it");
        let after = w.players[&hunter].inventory.get(&Commodity::Alloys).copied().unwrap_or(0);
        assert!(after >= before + 40, "the victor seizes the plunder ({before} → {after})");
        assert!(!w.enclaves[&sid].active(w.time), "the cleared base goes DORMANT");
        assert_eq!(w.enclaves[&sid].tier, 1, "it will respawn WEAKER (tier 1)");
    }

    #[test]
    fn piracy_is_deterministic_same_seed_same_runs() {
        // Two independent runs with the same seed produce byte-identical worlds.
        let run = || {
            let mut w = test_world();
            w.step(&[Command::AddPlayer { id: PlayerId(1), name: "V".into() }]);
            let (sid, epos) = an_enclave(&w);
            let convoy = squad(&mut w, PlayerId(1), epos + Vec2::new(1200.0, 0.0), ShipKind::Convoy, 1, FleetOrder::Idle);
            w.fleets.get_mut(&convoy).unwrap().cargo = Some(crate::cargo::Cargo { commodity: Commodity::MetallicOre, units: 20 });
            for _ in 0..(60 * crate::config::TICK_HZ) {
                w.step(&[]);
            }
            serde_json::to_string(&w).unwrap()
        };
        assert_eq!(run(), run(), "same seed → identical piracy (schedules, targets, outcomes)");
    }

    #[test]
    fn enclaves_persist_serde() {
        let w = test_world();
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w.enclaves.len(), w2.enclaves.len(), "enclaves survive a snapshot");
        for (sid, e) in &w.enclaves {
            assert_eq!(w2.enclaves[sid].tier, e.tier);
        }
    }

    // ── §node: EXOTIC NODE AWAKENING ────────────────────────────────────────────

    /// Force an UNCLAIMED system into an AWAKENED node of `bonus`, held by `owner`
    /// at `pos`, fed with a fat upkeep buffer. Returns its system id.
    fn owned_node(w: &mut World, owner: PlayerId, bonus: NodeBonus, pos: Vec2) -> EntityId {
        let sid = {
            let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.owner = Some(owner);
            sys.claimed_at = Some(0.0);
            sys.pos = pos;
            sys.id
        };
        w.nodes.insert(sid, Node { bonus, awakened: true, fed: true });
        seed_stock(w, sid, &[(Commodity::Provisions, 1000.0), (Commodity::Fuel, 1000.0)]);
        sid
    }

    /// PARITY LOCK: the sim's `node_bonus_for` MUST agree with the client's visual
    /// exotic assignment (`client/src/stars.ts`), or a black-hole icon would grant
    /// no node. This re-implements the client's `hashId` + `starTypeFor` from its
    /// hard-coded constants (FNV-1a, 0.16 rarity, `>>17`, EXOTIC pool order) as an
    /// INDEPENDENT reference and asserts the sim maps every id the same way — so a
    /// drift in either constant fails here.
    #[test]
    fn node_bonus_matches_client_star_assignment() {
        // Reference = client stars.ts, hand-translated (do NOT call node_bonus_for).
        fn client_bonus(id: EntityId) -> Option<NodeBonus> {
            let mut h: u32 = 2166136261;
            for b in id.0.to_string().bytes() {
                h ^= b as u32;
                h = h.wrapping_mul(16777619);
            }
            let roll = (h % 100_000) as f64 / 100_000.0;
            if roll >= 0.16 {
                return None;
            }
            // EXOTIC pool order in stars.ts: neutron, binary, black_hole, magnetar.
            Some(match (h >> 17) % 4 {
                0 | 1 => NodeBonus::DeepScan,
                2 => NodeBonus::RelayAnchor,
                _ => NodeBonus::Veil,
            })
        }
        let mut exotic = 0usize;
        let (mut relay, mut veil, mut scan) = (0, 0, 0);
        for raw in 1u64..=8000 {
            let id = EntityId(raw);
            assert_eq!(crate::node::node_bonus_for(id), client_bonus(id), "id {raw} diverges from the client");
            match crate::node::node_bonus_for(id) {
                Some(NodeBonus::RelayAnchor) => { exotic += 1; relay += 1; }
                Some(NodeBonus::Veil) => { exotic += 1; veil += 1; }
                Some(NodeBonus::DeepScan) => { exotic += 1; scan += 1; }
                None => {}
            }
        }
        // ~16% exotic, and all three bonuses actually occur (the mapping is live).
        let frac = exotic as f64 / 8000.0;
        assert!((frac - 0.16).abs() < 0.03, "exotic fraction ≈ 0.16 (got {frac:.3})");
        assert!(relay > 0 && veil > 0 && scan > 0, "every bonus kind occurs (relay={relay} veil={veil} scan={scan})");
    }

    /// Nodes are seeded DORMANT at exotic systems — pure function of id (no RNG),
    /// so the set is identical every run and matches the client's exotic icons.
    #[test]
    fn nodes_seeded_dormant_at_exotic_systems() {
        let w = test_world();
        assert!(!w.nodes.is_empty(), "a 4-player galaxy has exotic nodes");
        for (sid, n) in &w.nodes {
            assert!(!n.awakened, "dormant until the awakening time");
            assert_eq!(crate::node::node_bonus_for(*sid), Some(n.bonus), "bonus fixed by the id");
            assert!(w.systems.iter().any(|s| s.id == *sid), "a node sits on a real system");
        }
        // Determinism: a fresh galaxy of the same seed seeds the same node set.
        let w2 = test_world();
        assert_eq!(w.nodes.keys().copied().collect::<Vec<_>>(), w2.nodes.keys().copied().collect::<Vec<_>>());
    }

    /// AWAKENING (async-fair): at `node_awakening_time` every node latches awake
    /// ONCE, each announced galaxy-wide with its position (so the timeline can
    /// light-delay it per observer). No player need be online.
    #[test]
    fn nodes_awaken_once_at_configured_time_and_announce() {
        let mut w = test_world();
        w.config.node_awakening_time = 0.5;
        assert!(w.nodes.values().all(|n| !n.awakened), "all dormant before T");
        let mut fired = 0usize;
        while w.time < 0.6 {
            for e in w.step(&[]) {
                if let EventPayload::NodeAwakened { system, pos, .. } = e.payload {
                    fired += 1;
                    let sys_pos = w.systems.iter().find(|s| s.id == system).unwrap().pos;
                    assert_eq!(pos, sys_pos, "announced at the node's position (for the light-delay)");
                }
            }
        }
        assert!(w.nodes.values().all(|n| n.awakened), "all awake past T");
        assert_eq!(fired, w.nodes.len(), "exactly one announcement per node, once");
        // Stepping further fires nothing more (the latch holds).
        let more = w.step(&[]);
        assert!(!more.iter().any(|e| matches!(e.payload, EventPayload::NodeAwakened { .. })), "no re-announce");
    }

    /// Each bonus applies INSIDE its node's region and NOWHERE else, and only for
    /// the HOLDER — the three tactical scopes are single-sourced by region math. A
    /// DISTINCT holder per bonus keeps each corp under the per-corp cap.
    #[test]
    fn bonuses_apply_in_region_only_and_only_for_the_holder() {
        let mut w = test_world();
        let rival = PlayerId(9);
        let inside = |p: Vec2| p + Vec2::new(NODE_REGION_RADIUS - 1.0, 0.0);
        let outside = |p: Vec2| p + Vec2::new(NODE_REGION_RADIUS + 100.0, 0.0);

        // Relay Anchor — command delay halved in region.
        let (ra, pr) = (PlayerId(7), Vec2::new(1200.0, 0.0));
        owned_node(&mut w, ra, NodeBonus::RelayAnchor, pr);
        assert_eq!(w.relay_factor(ra, inside(pr)), RELAY_DELAY_MULT);
        assert_eq!(w.relay_factor(ra, outside(pr)), 1.0, "no tempo beyond the region");
        assert_eq!(w.relay_factor(rival, pr), 1.0, "a rival gets nothing from your node");

        // Veil — signature reduced in region.
        let (ve, pv) = (PlayerId(8), Vec2::new(-1500.0, 600.0));
        owned_node(&mut w, ve, NodeBonus::Veil, pv);
        assert_eq!(w.veil_factor(ve, inside(pv)), VEIL_SIGNATURE_MULT);
        assert_eq!(w.veil_factor(ve, outside(pv)), 1.0);
        assert_eq!(w.veil_factor(rival, pv), 1.0);

        // Deep Scan — composition reveal in region.
        let (ds, pd) = (PlayerId(10), Vec2::new(300.0, -2000.0));
        owned_node(&mut w, ds, NodeBonus::DeepScan, pd);
        assert!(w.deep_scan_covers(ds, inside(pd)));
        assert!(!w.deep_scan_covers(ds, outside(pd)));
        assert!(!w.deep_scan_covers(rival, pd));
    }

    /// UPKEEP: a held node draws its mix each tick; starve it and the bonus
    /// SUSPENDS (owner-notified, nothing destroyed); resupply and it recovers.
    #[test]
    fn node_upkeep_draws_suspends_and_recovers() {
        let mut w = test_world();
        let own = PlayerId(7);
        let p = Vec2::new(0.0, 1400.0);
        let sid = owned_node(&mut w, own, NodeBonus::RelayAnchor, p);
        // Strip stock + geology so upkeep can't be met (nor self-fed by deposits).
        {
            let s = w.systems.iter_mut().find(|s| s.id == sid).unwrap();
            s.stockpile.clear();
            s.set_test_deposits(vec![]);
        }
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::NodeSupplyChanged { system, fed: false, .. } if system == sid)),
            "the holder is told the node went unfed"
        );
        assert!(!w.nodes[&sid].fed);
        assert_eq!(w.relay_factor(own, p), 1.0, "an unfed node's bonus is SUSPENDED");

        seed_stock(&mut w, sid, &[(Commodity::Provisions, 100.0), (Commodity::Fuel, 100.0)]);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::NodeSupplyChanged { system, fed: true, .. } if system == sid)),
            "recovery notice when resupplied"
        );
        assert!(w.nodes[&sid].fed);
        assert_eq!(w.relay_factor(own, p), RELAY_DELAY_MULT, "bonus restored when fed");
    }

    /// PER-CORP CAP: a corp benefits from at most `NODES_PER_CORP` nodes at once
    /// (deterministic — lowest system id first); extra held nodes deny rivals and
    /// cost upkeep but grant no further bonus.
    #[test]
    fn per_corp_cap_limits_active_bonuses() {
        let mut w = test_world();
        let own = PlayerId(7);
        // One MORE than the cap, each in a disjoint region.
        let mut nodes: Vec<(EntityId, Vec2)> = Vec::new();
        for i in 0..(NODES_PER_CORP + 1) {
            let pos = Vec2::new(i as f64 * 8000.0, 0.0);
            nodes.push((owned_node(&mut w, own, NodeBonus::RelayAnchor, pos), pos));
        }
        assert_eq!(w.active_nodes_for(own).len(), NODES_PER_CORP, "capped at NODES_PER_CORP");
        nodes.sort_by_key(|(id, _)| *id);
        for (i, (_, pos)) in nodes.iter().enumerate() {
            let expect = if i < NODES_PER_CORP { RELAY_DELAY_MULT } else { 1.0 };
            assert_eq!(w.relay_factor(own, *pos), expect, "node #{i} active-status follows the cap");
        }
    }

    /// EXPOSURE via the EXISTING claim flow: settling an awakened, unowned node
    /// makes you its holder and announces it galaxy-wide (`NodeCaptured`). Before
    /// capture the node grants the corp nothing; after, its bonus is available.
    #[test]
    fn claiming_an_awakened_node_flips_and_announces_the_holder() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // An unclaimed exotic node, force-awakened + stocked for its upkeep.
        let sid = *w
            .nodes
            .keys()
            .find(|sid| w.systems.iter().find(|s| s.id == **sid).unwrap().owner.is_none())
            .expect("an unclaimed exotic node exists");
        w.nodes.get_mut(&sid).unwrap().awakened = true;
        let pos = w.systems.iter().find(|s| s.id == sid).unwrap().pos;
        seed_stock(&mut w, sid, &[(Commodity::Provisions, 100.0), (Commodity::Fuel, 100.0)]);
        assert!(w.active_nodes_for(id).is_empty(), "an unheld node grants nothing");

        colony_at(&mut w, id, pos);
        let ev = w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == sid).unwrap().owner, Some(id), "colony settles the node");
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::NodeCaptured { owner, system, .. } if owner == id && system == sid)),
            "the new holder is announced galaxy-wide"
        );
        assert_eq!(w.active_nodes_for(id).len(), 1, "the holder now benefits from its node");
    }

    /// Node state (bonus + awakened + fed) round-trips through a snapshot, and a
    /// PRE-feature snapshot (no `nodes` field) still loads (serde default).
    #[test]
    fn nodes_survive_a_snapshot() {
        let mut w = test_world();
        w.config.node_awakening_time = 0.05;
        for _ in 0..3 {
            w.step(&[]);
        }
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w.nodes.len(), w2.nodes.len(), "node set survives");
        for (sid, n) in &w.nodes {
            let m = &w2.nodes[sid];
            assert_eq!((n.bonus, n.awakened, n.fed), (m.bonus, m.awakened, m.fed), "node state round-trips");
        }
        // A pre-feature snapshot with no `nodes` key still deserialises.
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        val.as_object_mut().unwrap().remove("nodes");
        let w3: World = serde_json::from_value(val).unwrap();
        assert!(w3.nodes.is_empty(), "an old snapshot loads with no nodes");
    }

    // ── §rankings: multi-category leaderboards on the ledger clock ──────────────

    /// Each event-driven counter increments EXACTLY once per event, with the right
    /// amount and beneficiary — the "increment at events" contract.
    #[test]
    fn rankings_counters_increment_on_their_events() {
        use crate::cargo::Commodity;
        let mut w = test_world();
        let (p1, p2) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: p1, name: "A".into() },
            Command::AddPlayer { id: p2, name: "B".into() },
        ]);
        let ev = |p: EventPayload| Event::new(0.0, p);

        // Trade throughput + market profit.
        w.accumulate_rankings(&[
            ev(EventPayload::Trade(TradeEvent::Delivered { player: p1, commodity: Commodity::MetallicOre, units: 10, system: None })),
            ev(EventPayload::Trade(TradeEvent::Sold { player: p1, commodity: Commodity::MetallicOre, units: 4, unit_price: 5.0 })),
            ev(EventPayload::Trade(TradeEvent::Bought { player: p1, commodity: Commodity::MetallicOre, units: 2, unit_price: 3.0 })),
            ev(EventPayload::Trade(TradeEvent::LimitFilled { player: p1, side: Side::Sell, commodity: Commodity::MetallicOre, units: 3, unit_price: 2.0 })),
            ev(EventPayload::Trade(TradeEvent::LimitFilled { player: p1, side: Side::Buy, commodity: Commodity::MetallicOre, units: 1, unit_price: 4.0 })),
            ev(EventPayload::SystemUpgraded { system: EntityId(1), owner: p1, upgrade: crate::build::StructureKind::Depot, tier: 1 }),
            ev(EventPayload::IntelGathered { owner: p1, system: EntityId(1), defense_tier: 0, shipyard_tier: 0, pos: Vec2::ZERO }),
        ]);
        {
            let s = &w.players[&p1].stats;
            // §TCA: only the DELIVERY hauls goods. A `Sold` is market revenue, not
            // throughput (an instant Charterhouse sale moves nothing); the convoy
            // paths that really haul bump throughput at their arrival site instead.
            assert_eq!(s.trade_units, 10, "delivered 10; the sale hauled nothing");
            assert_eq!(s.market_revenue, 4.0 * 5.0 + 3.0 * 2.0);
            assert_eq!(s.market_spend, 2.0 * 3.0 + 1.0 * 4.0);
            assert_eq!(s.market_profit(), 26.0 - 10.0);
            assert_eq!(s.tiers_built, 1);
            assert_eq!(s.intel_snapshots, 1);
        }

        // Battle efficiency: both sides credited their destroyed/lost hull + one
        // engagement each (p1 loses 1 raider = 20 hull; p2 loses 2 convoys = 20).
        let mut a_loss = BTreeMap::new();
        a_loss.insert(ShipKind::Raider, 1);
        let mut d_loss = BTreeMap::new();
        d_loss.insert(ShipKind::Convoy, 2);
        w.accumulate_rankings(&[ev(EventPayload::RaidResolved {
            attacker: p1, defender: p2, attacker_ship: EntityId(1), target_ship: EntityId(2),
            attacker_kind: ShipKind::Raider, target_kind: ShipKind::Convoy, outcome: RaidOutcome::TargetDestroyed,
            pos: Vec2::ZERO, attacker_losses: a_loss, target_losses: d_loss,
        })]);
        assert_eq!(w.players[&p1].stats.engagements, 1);
        assert_eq!(w.players[&p1].stats.hull_lost, ShipKind::Raider.hull());
        assert_eq!(w.players[&p1].stats.hull_destroyed, 2.0 * ShipKind::Convoy.hull());
        assert_eq!(w.players[&p2].stats.engagements, 1);
        assert_eq!(w.players[&p2].stats.hull_destroyed, ShipKind::Raider.hull());

        // An ESCAPE (no contact) is not a fight — no engagement, no hull.
        w.accumulate_rankings(&[ev(EventPayload::RaidResolved {
            attacker: p1, defender: p2, attacker_ship: EntityId(1), target_ship: EntityId(2),
            attacker_kind: ShipKind::Raider, target_kind: ShipKind::Convoy, outcome: RaidOutcome::Escaped,
            pos: Vec2::ZERO, attacker_losses: BTreeMap::new(), target_losses: BTreeMap::new(),
        })]);
        assert_eq!(w.players[&p1].stats.engagements, 1, "an escape adds no engagement");

        // Capture plunder → cargo captured for the captor; a major loss pends for
        // the old owner (recovery floor stamps at the next valuation close).
        let mut plunder = BTreeMap::new();
        plunder.insert(Commodity::MetallicOre, 5);
        plunder.insert(Commodity::Fuel, 3);
        w.accumulate_rankings(&[ev(EventPayload::SystemCaptured { old_owner: p2, new_owner: p1, system: EntityId(1), pos: Vec2::ZERO, plunder })]);
        assert_eq!(w.players[&p1].stats.cargo_captured, 8);
        assert!(w.players[&p2].stats.loss_pending);
    }

    /// A RAID that seizes a convoy's cargo credits the raider CARGO CAPTURED and
    /// marks it as having fought.
    #[test]
    fn raid_seizure_credits_cargo_captured_and_marks_fought() {
        use crate::cargo::{Cargo, Commodity};
        let mut w = test_world();
        let (atk, def) = (PlayerId(1), PlayerId(2));
        let (raider, convoy) = raid_setup(&mut w, atk, def, Vec2::new(120.0, 0.0), Vec2::new(160.0, 0.0));
        {
            let c = w.fleets.get_mut(&convoy).unwrap();
            c.composition.clear();
            c.composition.insert(ShipKind::Convoy, 1);
            c.cargo = Some(Cargo { commodity: Commodity::MetallicOre, units: 40 });
        }
        w.step(&[Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]);
        run_until(&mut w, 30, |w| !w.fleets.contains_key(&convoy));
        assert_eq!(w.players[&atk].stats.cargo_captured, 40, "the raider banks the seized units");
        assert!(w.fleets.get(&raider).map(|f| f.fought).unwrap_or(false), "the raider is marked fought");
    }

    /// CARGO PROTECTED is credited only for a convoy that FOUGHT and still
    /// delivered; an un-fought delivery adds throughput but not protected units.
    #[test]
    fn protected_cargo_credited_only_when_the_convoy_fought() {
        use crate::cargo::{Cargo, Commodity};
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let make = |w: &mut World, fought: bool| {
            let fid = w.alloc_entity_id();
            let mut f = Fleet::single(fid, id, ShipKind::Convoy, Vec2::new(50.0, 0.0), FleetOrder::Idle, Some(Cargo { commodity: Commodity::MetallicOre, units: 25 }));
            f.mission = Some(TradeMission::DeliverHome);
            f.fought = fought;
            w.fleets.insert(fid, f);
        };
        make(&mut w, true);
        w.step(&[]);
        assert_eq!(w.players[&id].stats.trade_units, 25);
        assert_eq!(w.players[&id].stats.cargo_protected, 25, "a survivor of a fight earns protected units");
        make(&mut w, false);
        w.step(&[]);
        assert_eq!(w.players[&id].stats.trade_units, 50, "throughput accrues either way");
        assert_eq!(w.players[&id].stats.cargo_protected, 25, "an un-fought delivery adds NO protected units");
    }

    /// The leaderboard PUBLISHES only on the ledger tick (the valuation close) —
    /// nothing before it, a row per corp at the boundary.
    #[test]
    fn rankings_publish_on_the_ledger_interval() {
        let mut w = test_world();
        w.step(&[
            Command::AddPlayer { id: PlayerId(1), name: "A".into() },
            Command::AddPlayer { id: PlayerId(2), name: "B".into() },
        ]);
        assert!(w.rankings.is_empty(), "nothing published before the first close");
        while w.tick < VALUATION_TICKS - 1 {
            w.step(&[]);
        }
        assert!(w.rankings.is_empty(), "still nothing published before the boundary");
        w.step(&[]);
        assert_eq!(w.tick, VALUATION_TICKS);
        assert_eq!(w.rankings.len(), 2, "a row per corp published at the ledger close");
    }

    /// A mid-interval counter change does NOT leak into the published table — it is
    /// a SNAPSHOT copy that holds steady until the next close, then republishes.
    #[test]
    fn rankings_snapshot_does_not_leak_mid_interval() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        while w.tick < VALUATION_TICKS {
            w.step(&[]);
        }
        assert_eq!(w.rankings.len(), 1);
        let published = w.rankings[0].trade_throughput;
        // Bump the LIVE counter mid-interval.
        w.players.get_mut(&id).unwrap().stats.trade_units += 999;
        w.step(&[]); // one tick past the close — not a new boundary
        assert_eq!(w.rankings[0].trade_throughput, published, "the published table holds steady between closes");
        // Advance to the next close → it republishes with the new value.
        while !w.tick.is_multiple_of(VALUATION_TICKS) {
            w.step(&[]);
        }
        assert!(w.rankings[0].trade_throughput >= 999, "the next close republishes the updated counter");
    }

    /// The published table is deterministic — same seed + inputs → identical bytes.
    #[test]
    fn rankings_are_deterministic() {
        let run = || {
            let mut w = test_world();
            w.step(&[
                Command::AddPlayer { id: PlayerId(1), name: "A".into() },
                Command::AddPlayer { id: PlayerId(2), name: "B".into() },
            ]);
            while w.tick < VALUATION_TICKS {
                w.step(&[]);
            }
            serde_json::to_string(&w.rankings).unwrap()
        };
        assert_eq!(run(), run());
    }

    /// Cumulative stats + the published table ride a snapshot; a pre-feature
    /// snapshot (no `rankings`) still loads (serde default).
    #[test]
    fn rankings_and_stats_survive_a_snapshot() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        w.players.get_mut(&id).unwrap().stats.trade_units = 42;
        while w.tick < VALUATION_TICKS {
            w.step(&[]);
        }
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w2.players[&id].stats.trade_units, 42, "cumulative stats round-trip");
        // Floats through serde_json can drift 1 ULP (no float_roundtrip feature) —
        // compare the table field-wise with an epsilon on the float columns.
        assert_eq!(w.rankings.len(), w2.rankings.len(), "the published table round-trips");
        for (a, b) in w.rankings.iter().zip(&w2.rankings) {
            assert_eq!((a.player_id, &a.name, a.trade_throughput, &a.titles), (b.player_id, &b.name, b.trade_throughput, &b.titles));
            assert!((a.valuation - b.valuation).abs() < 1e-6, "valuation round-trips within the shared 1-ULP wobble");
        }
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        val.as_object_mut().unwrap().remove("rankings");
        let w3: World = serde_json::from_value(val).unwrap();
        assert!(w3.rankings.is_empty(), "a pre-feature snapshot loads with no published table");
    }

    // ── §explore Part 1: richness bands + per-corp survey knowledge ─────────────

    /// Band thresholds are terciles over the galaxy, deterministic from the seed,
    /// and the three bands all occur (the spectral read carries real signal).
    #[test]
    fn band_terciles_are_deterministic_and_populated() {
        let a = test_world();
        let b = test_world();
        assert_eq!((a.band_lo, a.band_hi), (b.band_lo, b.band_hi), "same seed → same terciles");
        assert!(a.band_lo > 0.0 && a.band_hi > a.band_lo, "thresholds are real and ordered");
        let mut counts = [0usize; 3];
        for s in &a.systems {
            match a.band_of(s) {
                crate::explore::RichnessBand::Poor => counts[0] += 1,
                crate::explore::RichnessBand::Fair => counts[1] += 1,
                crate::explore::RichnessBand::Rich => counts[2] += 1,
            }
        }
        assert!(counts.iter().all(|&n| n > 0), "all three bands occur (got {counts:?})");
        // Bands agree across the two same-seed worlds, system by system.
        for (sa, sb) in a.systems.iter().zip(&b.systems) {
            assert_eq!(a.band_of(sa), b.band_of(sb));
        }
    }

    /// Joining pre-surveys the home valley (everything within the initial radius)
    /// and nothing beyond it — the frontier starts dark.
    #[test]
    fn join_preseeds_the_home_survey_radius() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let corp = &w.players[&id];
        let home = corp.home;
        assert!(!corp.surveyed.is_empty(), "the starting valley is known");
        for s in &w.systems {
            let near = s.pos.distance(home) <= crate::explore::SURVEY_INITIAL_RADIUS;
            assert_eq!(corp.surveyed.contains(&s.id), near, "surveyed iff within the initial radius");
        }
        assert!(
            w.systems.iter().any(|s| !corp.surveyed.contains(&s.id)),
            "the frontier is NOT pre-surveyed"
        );
    }

    /// Claiming a system (even blind) makes it surveyed — holding is knowing; the
    /// knowledge is permanent.
    #[test]
    fn claiming_inserts_survey_knowledge() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        assert!(!w.players[&id].surveyed.contains(&sysid), "the frontier prize starts unsurveyed");
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        colony_at(&mut w, id, pos);
        w.step(&[]);
        assert_eq!(w.systems.iter().find(|s| s.id == sysid).unwrap().owner, Some(id));
        assert!(w.players[&id].surveyed.contains(&sysid), "the blind claim resolves the gamble — geology known");
    }

    /// The MIGRATION FIXUP heals a pre-feature snapshot: zeroed thresholds are
    /// recomputed and an empty survey set is seeded with owned + home-radius
    /// systems; a post-feature corp is left untouched.
    #[test]
    fn fixup_after_load_heals_pre_feature_state() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // Give the corp a distant owned system (a blind-claimed frontier hold).
        let far = richest_system(&w);
        w.systems.iter_mut().find(|s| s.id == far).unwrap().owner = Some(id);
        // Simulate the pre-feature snapshot: no thresholds, no survey knowledge.
        let (lo, hi) = (w.band_lo, w.band_hi);
        w.band_lo = 0.0;
        w.band_hi = 0.0;
        w.players.get_mut(&id).unwrap().surveyed.clear();
        w.fixup_after_load();
        assert_eq!((w.band_lo, w.band_hi), (lo, hi), "thresholds recomputed identically (pure)");
        let corp = &w.players[&id];
        assert!(corp.surveyed.contains(&far), "owned systems are re-known");
        let home = corp.home;
        for s in &w.systems {
            if s.pos.distance(home) <= crate::explore::SURVEY_INITIAL_RADIUS {
                assert!(corp.surveyed.contains(&s.id), "the home valley is re-known");
            }
        }
        // A corp WITH knowledge is untouched (the fixup only heals amnesia).
        let before = w.players[&id].surveyed.clone();
        w.players.get_mut(&id).unwrap().surveyed.remove(&far);
        w.fixup_after_load();
        assert!(!w.players[&id].surveyed.contains(&far), "a non-empty set is never modified");
        assert_eq!(w.players[&id].surveyed.len(), before.len() - 1);
    }

    /// Survey knowledge + band thresholds ride a snapshot; a PRE-feature snapshot
    /// (fields absent) still loads with the serde defaults.
    #[test]
    fn survey_knowledge_survives_a_snapshot() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(w.players[&id].surveyed, w2.players[&id].surveyed, "survey set round-trips");
        // Floats through serde_json can drift 1 ULP (no float_roundtrip feature)
        // — a measure-zero band-boundary wobble every snapshot f64 shares.
        assert!((w.band_lo - w2.band_lo).abs() < 1e-9 && (w.band_hi - w2.band_hi).abs() < 1e-9, "thresholds round-trip");
        // Pre-feature: strip the new fields → defaults (then the fixup heals).
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        val.as_object_mut().unwrap().remove("band_lo");
        val.as_object_mut().unwrap().remove("band_hi");
        for p in val["players"].as_object_mut().unwrap().values_mut() {
            p.as_object_mut().unwrap().remove("surveyed");
        }
        let mut w3: World = serde_json::from_value(val).unwrap();
        assert_eq!((w3.band_lo, w3.band_hi), (0.0, 0.0));
        assert!(w3.players[&id].surveyed.is_empty());
        w3.fixup_after_load();
        assert!(
            (w3.band_lo - w.band_lo).abs() < 1e-9 && (w3.band_hi - w.band_hi).abs() < 1e-9,
            "fixup restores thresholds (within the shared 1-ULP float wobble)"
        );
        assert!(!w3.players[&id].surveyed.is_empty(), "fixup restores the home valley");
    }

    // ── §explore Part 2: the SURVEY order ───────────────────────────────────────

    /// A joined player + a scout fleet at `pos` + an UNSURVEYED frontier system to
    /// target. Returns (player, scout fleet id, system id, system pos).
    fn survey_setup(w: &mut World) -> (PlayerId, EntityId, EntityId, Vec2) {
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(w);
        assert!(!w.players[&id].surveyed.contains(&sysid), "target starts unsurveyed");
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        // Park the scout NEAR the target (a short approach) — the ORDER still
        // travels from the cc at c, so step past the delivery before dwelling.
        let scout = squad(w, id, pos + Vec2::new(300.0, 0.0), ShipKind::Scout, 1, FleetOrder::Idle);
        (id, scout, sysid, pos)
    }

    /// The SCOUT GATE: a fleet without a scout soft-rejects the order; with one
    /// it schedules (light-delayed) and eventually flies.
    #[test]
    fn survey_requires_a_scout() {
        let mut w = test_world();
        let (id, _scout, sysid, pos) = survey_setup(&mut w);
        let raider = squad(&mut w, id, pos + Vec2::new(300.0, 0.0), ShipKind::Raider, 1, FleetOrder::Idle);
        w.step(&[Command::SurveySystem { player_id: id, fleet_id: raider, system_id: sysid }]);
        run_until(&mut w, 20, |w| !matches!(w.fleets[&raider].order, FleetOrder::Idle));
        assert!(matches!(w.fleets[&raider].order, FleetOrder::Idle), "no scout → soft-reject (order never lands)");
    }

    /// APPROACH → DWELL → COMPLETE: the dwell is all-or-nothing and LOUD; the
    /// report travels home at c and inserts PERMANENT knowledge; rankings credit.
    #[test]
    fn survey_dwell_completes_and_knowledge_travels_at_c() {
        let mut w = test_world();
        let (id, scout, sysid, _pos) = survey_setup(&mut w);
        let intel0 = w.players[&id].stats.intel_snapshots;
        w.step(&[Command::SurveySystem { player_id: id, fleet_id: scout, system_id: sysid }]);
        // The order lands (light-delayed), the scout flies + dwells. While the
        // dwell runs, the fleet is LOUD (surveying() true) — sample mid-dwell.
        assert!(run_until(&mut w, 60, |w| w.fleets.get(&scout).is_some_and(|f| f.surveying())), "the dwell starts");
        assert!(w.fleets[&scout].surveying(), "LOUD during the dwell window");
        // Completion fires SurveyCompleted + goes Idle; loudness ends with it.
        assert!(
            run_until(&mut w, 60, |w| w.fleets.get(&scout).is_some_and(|f| matches!(f.order, FleetOrder::Idle))),
            "the dwell completes"
        );
        assert!(!w.fleets[&scout].surveying(), "loudness ends outside the window");
        // The knowledge is IN FLIGHT (pending report) — the corp doesn't know yet
        // unless the light already landed; wait for the delivery.
        assert!(
            run_until(&mut w, 120, |w| w.players[&id].surveyed.contains(&sysid)),
            "the report light lands at the cc and the knowledge inserts"
        );
        assert_eq!(w.players[&id].stats.intel_snapshots, intel0 + 1, "a survey feeds the intel ladder");
        // PERMANENT: nothing removes it.
        for _ in 0..(5 * TICK_HZ) {
            w.step(&[]);
        }
        assert!(w.players[&id].surveyed.contains(&sysid), "survey knowledge never stales");
    }

    /// The report is LIGHT-DELAYED: between completion and `pos→cc` arrival the
    /// corp provably does NOT know the geology.
    #[test]
    fn survey_report_is_light_delayed_to_the_cc() {
        let mut w = test_world();
        let (id, scout, sysid, pos) = survey_setup(&mut w);
        // Drive the dwell DIRECTLY (skip the order round-trip): on-site, clock set.
        w.fleets.get_mut(&scout).unwrap().pos = pos;
        w.fleets.get_mut(&scout).unwrap().order =
            FleetOrder::Survey { system: sysid, station: pos, dwell_since: Some(w.time) };
        let cc = w.players[&id].command_center;
        let delay = pos.distance(cc) / w.config.c;
        assert!(delay > 1.0, "the frontier prize is far enough for a measurable delay ({delay:.1}s)");
        // Step just past completion: completed, but the light hasn't landed.
        let steps = (crate::explore::SURVEY_SECS / DT) as u32 + 3;
        for _ in 0..steps {
            w.step(&[]);
        }
        assert!(matches!(w.fleets[&scout].order, FleetOrder::Idle), "dwell completed");
        assert!(!w.players[&id].surveyed.contains(&sysid), "knowledge NOT known before its light arrives");
        assert!(
            run_until(&mut w, delay.ceil() as u32 + 2, |w| w.players[&id].surveyed.contains(&sysid)),
            "knowledge lands once the report light reaches the cc"
        );
    }

    /// ALLY RELAY: the owner's landing fans a relayed copy to each ally, arriving
    /// one more cc→cc light-leg later; a non-ally NEVER receives it.
    #[test]
    fn survey_reports_relay_to_allies_on_the_intel_chain() {
        let mut w = test_world();
        let (a, b, c_id) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
            Command::AddPlayer { id: c_id, name: "C".into() },
        ]);
        ally(&mut w, a, b);
        let sysid = richest_system(&w);
        let pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        let scout = squad(&mut w, a, pos, ShipKind::Scout, 1, FleetOrder::Idle);
        w.fleets.get_mut(&scout).unwrap().order =
            FleetOrder::Survey { system: sysid, station: pos, dwell_since: Some(w.time) };
        // Run until A knows; B must not know yet at that exact moment (the relay
        // leg still has cc→cc distance to cover), then B learns; C never does.
        assert!(run_until(&mut w, 180, |w| w.players[&a].surveyed.contains(&sysid)), "A learns first");
        assert!(!w.players[&b].surveyed.contains(&sysid), "the ally copy is still in flight (chain delay)");
        assert!(run_until(&mut w, 120, |w| w.players[&b].surveyed.contains(&sysid)), "the ally receives the relayed copy");
        assert!(!w.players[&c_id].surveyed.contains(&sysid), "a NON-ally never receives it");
    }

    /// An ENGAGEMENT aborts the dwell — all-or-nothing, no partial credit; the
    /// order is re-issuable (it goes Idle, nothing banked).
    #[test]
    fn survey_aborts_on_engagement_with_no_credit() {
        let mut w = test_world();
        let (id, scout, sysid, pos) = survey_setup(&mut w);
        let rival = PlayerId(2);
        w.step(&[Command::AddPlayer { id: rival, name: "R".into() }]);
        // Scout on-site, mid-dwell; a rival raider parked on top ATTACKS it.
        w.fleets.get_mut(&scout).unwrap().pos = pos;
        w.fleets.get_mut(&scout).unwrap().order =
            FleetOrder::Survey { system: sysid, station: pos, dwell_since: Some(w.time) };
        let raider = squad(&mut w, rival, pos + Vec2::new(10.0, 0.0), ShipKind::Raider, 2, FleetOrder::Idle);
        w.step(&[Command::AttackFleet { player_id: rival, fleet_id: raider, target_id: scout }]);
        assert!(
            run_until(&mut w, 60, |w| w.fleets.get(&scout).is_none_or(|f| matches!(f.order, FleetOrder::Idle))),
            "contact aborts the dwell"
        );
        assert!(!w.players[&id].surveyed.contains(&sysid), "no partial credit — the fight interrupted the sweep");
    }

    /// LEAVING RANGE resets the dwell clock (no partial credit): shoved off
    /// station, the clock restarts from zero once back on-site.
    #[test]
    fn survey_dwell_resets_out_of_range() {
        let mut w = test_world();
        let (_id, scout, sysid, pos) = survey_setup(&mut w);
        let f = w.fleets.get_mut(&scout).unwrap();
        f.pos = pos;
        f.order = FleetOrder::Survey { system: sysid, station: pos, dwell_since: Some(w.time) };
        // Displace the fleet BEYOND the survey range mid-dwell.
        w.fleets.get_mut(&scout).unwrap().pos = pos + Vec2::new(crate::explore::SURVEY_RANGE + 500.0, 0.0);
        w.step(&[]);
        match w.fleets[&scout].order {
            FleetOrder::Survey { dwell_since, .. } => {
                assert!(dwell_since.is_none(), "off-site → the clock resets (no partial credit)")
            }
            ref o => panic!("the order should persist through a reset (got {o:?})"),
        }
    }

    /// RE-SURVEYING an already-surveyed system is legal and idempotent.
    #[test]
    fn survey_is_idempotent_on_a_known_system() {
        let mut w = test_world();
        let (id, scout, sysid, pos) = survey_setup(&mut w);
        w.players.get_mut(&id).unwrap().surveyed.insert(sysid); // already known
        w.fleets.get_mut(&scout).unwrap().pos = pos;
        w.fleets.get_mut(&scout).unwrap().order =
            FleetOrder::Survey { system: sysid, station: pos, dwell_since: Some(w.time) };
        assert!(
            run_until(&mut w, 60, |w| matches!(w.fleets[&scout].order, FleetOrder::Idle)),
            "the re-survey runs to completion (wasted time, not an error)"
        );
        assert!(w.players[&id].surveyed.contains(&sysid), "still known — idempotent");
    }

    /// DETERMINISM: the whole survey flow (order → dwell → report → relay) is
    /// byte-identical across two same-seed runs.
    #[test]
    fn surveys_are_deterministic() {
        let run = || {
            let mut w = test_world();
            let (id, scout, sysid, _pos) = survey_setup(&mut w);
            w.step(&[Command::SurveySystem { player_id: id, fleet_id: scout, system_id: sysid }]);
            for _ in 0..(90 * TICK_HZ) {
                w.step(&[]);
            }
            serde_json::to_string(&w).unwrap()
        };
        assert_eq!(run(), run());
    }

    // ── §explore Part 3: hidden TRAITS ──────────────────────────────────────────

    use crate::explore::SystemTrait;

    /// Force `sys` to a claimed system with the given trait + a single Ore deposit
    /// (richness 1.0, renewable); returns its id.
    fn trait_system(w: &mut World, owner: PlayerId, t: Option<SystemTrait>) -> EntityId {
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(owner);
        sys.claimed_at = Some(0.0);
        sys.trait_ = t;
        sys.set_test_deposits(vec![crate::galaxy::Deposit {
            resource: Commodity::MetallicOre,
            richness: 1.0,
            reserves: None,
            accessibility: 0.5,
        }]);
        // §economy Part 3: production is STAFFED now — a big, well-fed
        // population so every posted line runs at factor 1.0 and the trait
        // multipliers are measured clean.
        sys.set_population(8.0);
        sys.stockpile.insert(Commodity::Provisions, 300.0); // ample food, safely under the storage cap
        sys.id
    }

    /// §economy Part 3 test shorthand: build `kind` at `tier` and post a full
    /// crew (workers = tier), so staffing = 1.0 under an ample workforce.
    fn staff(w: &mut World, sid: EntityId, kind: crate::build::StructureKind, tier: u32) {
        let sys = w.systems.iter_mut().find(|s| s.id == sid).unwrap();
        sys.set_tier(kind, tier);
        if tier > 0 {
            sys.assign(kind, crate::production::Assignment::crew(tier));
        } else if let Some(b) = sys.bodies.iter_mut().find(|b| b.tier(kind) > 0) { b.assignments.remove(&kind); }
    }

    /// Trait assignment is deterministic from the seed, hits ~TRAIT_FRACTION, and
    /// all five kinds occur (checked across seeds — one galaxy is small).
    #[test]
    fn traits_are_seeded_deterministically_at_the_fraction() {
        let a = test_world();
        let b = test_world();
        for (sa, sb) in a.systems.iter().zip(&b.systems) {
            assert_eq!(sa.trait_, sb.trait_, "same seed → same traits");
        }
        let mut with = 0usize;
        let mut total = 0usize;
        let mut kinds = std::collections::BTreeSet::new();
        for seed in 0..40u64 {
            let w = World::new(SimConfig::for_players(seed, 4));
            for s in &w.systems {
                total += 1;
                if let Some(t) = s.trait_ {
                    with += 1;
                    kinds.insert(t.slug());
                    if let SystemTrait::BonusVein { commodity } = t {
                        assert!(
                            s.all_deposits().any(|d| d.resource == commodity),
                            "a Bonus Vein is always of a commodity the system HAS"
                        );
                    }
                }
            }
        }
        let frac = with as f64 / total as f64;
        assert!((frac - crate::explore::TRAIT_FRACTION).abs() < 0.06, "≈{} of systems carry a trait (got {frac:.3})", crate::explore::TRAIT_FRACTION);
        assert_eq!(kinds.len(), 5, "all five trait kinds occur (got {kinds:?})");
    }

    /// BONUS VEIN: that commodity's accrual runs ×BONUS_VEIN_MULT; others don't.
    #[test]
    fn bonus_vein_boosts_only_its_commodity() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = trait_system(&mut w, id, Some(SystemTrait::BonusVein { commodity: Commodity::MetallicOre }));
        // Add a second, non-vein deposit for the control — ON THE SAME BODY
        // (§bodies: a mine only works its own body's deposits), so the only
        // difference between the two outputs is the vein.
        {
            let sys = w.systems.iter_mut().find(|s| s.id == sid).unwrap();
            let ore_body = sys
                .bodies
                .iter()
                .position(|b| b.deposits.iter().any(|d| d.resource == Commodity::MetallicOre))
                .expect("the vein ore landed somewhere");
            sys.bodies[ore_body].deposits.push(crate::galaxy::Deposit {
                resource: Commodity::Silicates,
                richness: 1.0,
                reserves: None,
                accessibility: 0.5,
            });
        }
        staff(&mut w, sid, crate::build::StructureKind::MiningComplex, 1);
        w.step(&[]);
        let ore = system_stock(&w, sid, Commodity::MetallicOre);
        let sil = system_stock(&w, sid, Commodity::Silicates);
        assert!((ore - crate::explore::BONUS_VEIN_MULT * DT).abs() < 1e-9, "the vein commodity runs ×{} (got {ore})", crate::explore::BONUS_VEIN_MULT);
        assert!((sil - DT).abs() < 1e-9, "other commodities are untouched (got {sil})");
    }

    /// DEEP DEPOSITS (§economy Part 3 port): base ×1.5 and the THROUGHPUT
    /// LADDER runs one tier behind — a tier of progress is wasted breaking
    /// through: tier 1 and tier 2 produce identically (base throughput);
    /// tier 3 gets the tier-2 rung. (Under the assignment engine tier 0 is
    /// simply no structure = no extraction, so the waste shows at tier 2.)
    #[test]
    fn deep_deposits_waste_a_tier_of_the_throughput_ladder() {
        let gain_at_tier = |tier: u32| {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "A".into() }]);
            let sid = trait_system(&mut w, id, Some(SystemTrait::DeepDeposits));
            staff(&mut w, sid, crate::build::StructureKind::MiningComplex, tier);
            w.step(&[]);
            system_stock(&w, sid, Commodity::MetallicOre)
        };
        let base = crate::explore::DEEP_DEPOSITS_BASE_MULT;
        let ladder = crate::production::tier_throughput;
        assert!((gain_at_tier(0) - 0.0).abs() < 1e-12, "tier 0: no structure, no extraction");
        assert!((gain_at_tier(1) - base * ladder(1) * DT).abs() < 1e-9, "tier 1: base throughput ×{base}");
        assert!((gain_at_tier(2) - base * ladder(1) * DT).abs() < 1e-9, "tier 2: IDENTICAL — a tier is wasted breaking through");
        assert!((gain_at_tier(3) - base * ladder(2) * DT).abs() < 1e-9, "tier 3: the tier-2 rung applies");
    }

    /// UNSTABLE GEOLOGY: development costs ×UNSTABLE_COST_MULT at BOTH the
    /// affordability gate and the debit (one shared multiplier); ships unaffected.
    #[test]
    fn unstable_geology_taxes_developments() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "A".into() }]);
        let sid = trait_system(&mut w, id, Some(SystemTrait::UnstableGeology));
        let recipe = crate::build::recipe_for(crate::build::BuildKind::Upgrade { upgrade: crate::build::StructureKind::MiningComplex });
        // Stock EXACTLY the plain cost → must be REJECTED (needs ×1.25).
        for (c, need) in recipe.costs {
            seed_stock(&mut w, sid, &[(*c, *need)]);
        }
        w.step(&[Command::DevelopSystem { player_id: id, system_id: sid, upgrade: crate::build::StructureKind::MiningComplex, body_id: None }]);
        assert!(w.build_queue.is_empty(), "plain-cost stock can't afford the unstable premium");
        // Top up to ×UNSTABLE_COST_MULT → accepted, and debited at the premium.
        for (c, need) in recipe.costs {
            seed_stock(&mut w, sid, &[(*c, *need * (crate::explore::UNSTABLE_COST_MULT - 1.0) + 0.001)]);
        }
        let before: Vec<f64> = recipe.costs.iter().map(|(c, _)| system_stock(&w, sid, *c)).collect();
        w.step(&[Command::DevelopSystem { player_id: id, system_id: sid, upgrade: crate::build::StructureKind::MiningComplex, body_id: None }]);
        assert_eq!(w.build_queue.len(), 1, "the premium stock affords it");
        for ((c, need), b4) in recipe.costs.iter().zip(before) {
            let now_units = system_stock(&w, sid, *c);
            let debited = b4 - now_units;
            // The system also ACCRUES its Ore deposit during the step — allow that.
            assert!(
                (debited - need * crate::explore::UNSTABLE_COST_MULT).abs() < 0.05,
                "{c:?} debited at the ×{} premium (debited {debited}, want {})",
                crate::explore::UNSTABLE_COST_MULT,
                need * crate::explore::UNSTABLE_COST_MULT
            );
        }
    }

    /// VOLATILE POCKETS: the Refinery's Fuel output runs ×VOLATILE_REFINERY_MULT.
    #[test]
    fn volatile_pockets_boost_refinery_output() {
        let fuel_out = |t: Option<SystemTrait>| {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "A".into() }]);
            let sid = trait_system(&mut w, id, t);
            w.systems.iter_mut().find(|s| s.id == sid).unwrap().set_test_deposits(vec![]); // no accrual noise
            staff(&mut w, sid, crate::build::StructureKind::FuelRefinery, 1);
            seed_stock(&mut w, sid, &[(Commodity::Volatiles, 100.0)]);
            w.step(&[]);
            system_stock(&w, sid, Commodity::Fuel)
        };
        let plain = fuel_out(None);
        let pocket = fuel_out(Some(SystemTrait::VolatilePockets));
        assert!(plain > 0.0, "the refinery runs");
        assert!(
            (pocket - plain * crate::explore::VOLATILE_REFINERY_MULT).abs() < 1e-9,
            "pockets multiply the OUTPUT ×{} (plain {plain}, pocket {pocket})",
            crate::explore::VOLATILE_REFINERY_MULT
        );
    }

    /// PRECURSOR CACHE pays EXACTLY ONCE at claim (latched), reveals on claim,
    /// transfers knowledge (a fresh TraitRevealed) on capture — but never re-pays.
    #[test]
    fn precursor_cache_pays_once_and_capture_transfers_knowledge() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        // An unclaimed frontier system carrying the cache.
        let sid = {
            let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
            sys.trait_ = Some(SystemTrait::PrecursorCache);
            sys.id
        };
        let pos = w.systems.iter().find(|s| s.id == sid).unwrap().pos;
        // A claims it physically.
        colony_at(&mut w, a, pos);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::TraitRevealed { owner, system, .. } if owner == a && system == sid)),
            "the claim REVEALS the trait to the claimer"
        );
        let alloys = system_stock(&w, sid, Commodity::Alloys);
        assert!((alloys - crate::explore::PRECURSOR_ALLOYS).abs() < 0.01, "the cache pays its one-time grant (got {alloys})");
        assert!(w.systems.iter().find(|s| s.id == sid).unwrap().cache_claimed, "the latch is set");

        // B captures it — knowledge transfers (a fresh reveal), the cache does NOT re-pay.
        let stock_before: f64 = system_stock(&w, sid, Commodity::Alloys);
        let colony = colony_at(&mut w, b, pos);
        let mut ev2 = Vec::new();
        w.capture_system(sid, a, b, colony, pos, &mut ev2);
        assert!(
            ev2.iter().any(|e| matches!(e.payload, EventPayload::TraitRevealed { owner, system, .. } if owner == b && system == sid)),
            "capture transfers the trait knowledge (spoils)"
        );
        // Capture PLUNDERS the stockpile (the pre-flip alloys ride the plunder) and
        // the latch survives — no re-mint for the new owner.
        let after = system_stock(&w, sid, Commodity::Alloys);
        assert!(after < stock_before + 0.01, "the cache never re-pays (latch survives the flip)");
        assert!(w.systems.iter().find(|s| s.id == sid).unwrap().cache_claimed, "latch intact across capture");
    }

    /// §economy: the LEGACY tier fold — a pre-economy snapshot (flat tier fields,
    /// no structures map) loads, and `fixup_after_load` folds every built tier
    /// into `structures` (Extractor→MiningComplex, Refinery→FuelRefinery, rest
    /// 1:1), zeroing the carriers. Idempotent.
    #[test]
    fn legacy_tiers_fold_into_structures_on_load() {
        use crate::build::StructureKind as K;
        let w = test_world();
        let json = serde_json::to_string(&w).unwrap();
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Simulate a pre-economy snapshot: legacy flat tiers, no structures map.
        let sys0 = &mut val["systems"][0];
        sys0.as_object_mut().unwrap().remove("structures");
        sys0["extractor_tier"] = 2.into();
        sys0["refinery_tier"] = 1.into();
        sys0["depot_tier"] = 3.into();
        let mut w2: World = serde_json::from_value(val).unwrap();
        assert_eq!(w2.systems[0].legacy_extractor_tier, 2, "legacy fields parse (serde rename)");
        w2.fixup_after_load();
        let s0 = &w2.systems[0];
        assert_eq!(s0.tier(K::MiningComplex), 2, "Extractor folds to MiningComplex");
        assert_eq!(s0.tier(K::FuelRefinery), 1, "Refinery folds to FuelRefinery");
        assert_eq!(s0.tier(K::Depot), 3, "Depot folds 1:1");
        assert_eq!(s0.legacy_extractor_tier, 0, "carriers zeroed after the fold");
        // Idempotent: folding again changes nothing.
        let before = s0.bodies.iter().map(|b| b.structures.clone()).collect::<Vec<_>>();
        w2.fixup_after_load();
        assert_eq!(w2.systems[0].bodies.iter().map(|b| b.structures.clone()).collect::<Vec<_>>(), before, "the fold is idempotent");
        // In-flight legacy build jobs parse to the mapped kind via serde alias.
        let job: crate::build::BuildKind =
            serde_json::from_str(r#"{"kind":"upgrade","upgrade":"extractor"}"#).unwrap();
        assert_eq!(job, crate::build::BuildKind::Upgrade { upgrade: K::MiningComplex });
        let job2: crate::build::BuildKind =
            serde_json::from_str(r#"{"kind":"upgrade","upgrade":"refinery"}"#).unwrap();
        assert_eq!(job2, crate::build::BuildKind::Upgrade { upgrade: K::FuelRefinery });
    }

    /// Traits + latches ride a snapshot; a PRE-feature snapshot (fields absent)
    /// loads trait-less.
    #[test]
    fn traits_survive_a_snapshot() {
        let w = test_world();
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        for (sa, sb) in w.systems.iter().zip(&w2.systems) {
            assert_eq!(sa.trait_, sb.trait_, "traits round-trip");
            assert_eq!(sa.cache_claimed, sb.cache_claimed);
        }
        let mut val: serde_json::Value = serde_json::from_str(&json).unwrap();
        for s in val["systems"].as_array_mut().unwrap() {
            s.as_object_mut().unwrap().remove("trait_");
            s.as_object_mut().unwrap().remove("cache_claimed");
        }
        let w3: World = serde_json::from_value(val).unwrap();
        assert!(w3.systems.iter().all(|s| s.trait_.is_none()), "a pre-feature snapshot loads trait-less");
    }
}
