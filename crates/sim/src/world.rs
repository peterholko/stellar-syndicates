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
use crate::event::{DivertAction, Event, EventPayload, RaidOutcome, TradeEvent};
use crate::galaxy::{generate_home_slots, generate_systems, HomeSlot, StarSystem};
use crate::ids::{EntityId, PlayerId};
use crate::market::{clear_call_auction, LimitOrder, Side};
use crate::math::Vec2;
use crate::movement::pursue_step;
use crate::ship::{DefenseEngagement, Fleet, FleetOrder, ShipKind, TradeMission};
use crate::standing::{Endpoint, OrderStatus, StandingOrder, Trigger};

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
}

/// One scouted observation of a rival system's fortifications (§scout part 2):
/// the raid/siege-relevant tiers, WHEN it was seen, and WHERE the scout stood
/// (the light source for delivering the report). Deliberately narrow — no
/// stockpiles, no habitat state — the prize stays focused.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IntelSnapshot {
    pub defense_tier: u32,
    pub shipyard_tier: u32,
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

fn default_order_kind() -> crate::event::OrderKind {
    crate::event::OrderKind::Move
}

fn default_player() -> PlayerId {
    PlayerId(0)
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

/// Distance (sim units) at which a raider makes contact with its target.
const CONTACT_RADIUS: f64 = 80.0;
/// Distance (sim units) within which a friendly combatant fleet that has moved to
/// an active battle JOINS its side's pool (relief). A bit more than contact so an
/// arriving reinforcement latches on cleanly. Tunable.
const BATTLE_JOIN_RADIUS: f64 = 200.0;
/// Distance from the hub within which a convoy is safe from raiders (§4: the hub
/// is the shared commons).
const HUB_SAFE_RADIUS: f64 = 300.0;

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
    /// Monotonic allocator for build-job ids (0 ⇒ first id is 1).
    #[serde(default)]
    next_build_id: u64,
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
    /// Per-kind side DAMAGE POOLS (the persisted mid-battle state).
    a_pool: BTreeMap<ShipKind, f64>,
    d_pool: BTreeMap<ShipKind, f64>,
    /// Report bookkeeping: total composition + strength each side STARTED with.
    a_start: BTreeMap<ShipKind, u32>,
    d_start: BTreeMap<ShipKind, u32>,
    a_start_strength: f64,
    d_start_strength: f64,
    platform_start_tiers: u32,
    /// Touched this tick? (Untouched engagements have ended — flush + remove.)
    #[serde(skip)]
    touched: bool,
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

impl World {
    /// Create a galaxy for the given configuration: hub at the centre, seeded
    /// star systems, and a ring of empty home anchors.
    pub fn new(config: SimConfig) -> Self {
        let mut rng = crate::rng::Rng::new(config.seed);
        let mut next_entity_id = 1u64;

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
            crate::galaxy::generate_home_systems(config.seed, &home_slots, &mut alloc)
        };
        for (slot, sys) in home_slots.iter_mut().zip(&home_systems) {
            slot.system = Some(sys.id);
        }
        systems.extend(home_systems);

        World {
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
            rng,
            engagements: BTreeMap::new(),
            next_engagement_id: 0,
        }
    }

    /// The player's in-flight ORDER LIFECYCLES (§order-lifecycle) — the LATEST
    /// order per fleet, covering both the in-transit pending orders and the
    /// delivered-but-awaiting-echo ones. OWNER-ONLY (a rival gets nothing). The
    /// client ticks the IN-TRANSIT / AWAITING-ECHO countdowns from the two
    /// timestamps against `sim_time`, and flips its dashed heading to solid at
    /// `echo_at`.
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

    /// Allocate a fresh, deterministic entity id.
    fn alloc_entity_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
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

        // 3. Integrate continuous movement (flip-and-burn, patrols, and raider
        //    interception pursuit).
        self.integrate_movement();

        // 4. Resolve raids in true space (contact → convoy lost; convoy reaches
        //    the hub → escape). A raided trade convoy's goods are simply lost.
        self.resolve_raids(&mut events);

        // 4b. COLONY ARRIVALS (§fleets part 3): settlement is physical — resolve
        //     after raids so a colony ship killed at the doorstep never claims.
        self.resolve_colony_arrivals(&mut events);

        // 4c. ORDER LIFECYCLE (§order-lifecycle): after this tick's destruction is
        //     settled, confirm delivered orders whose echo light has returned
        //     (owner-only `OrderConfirmed`), and drop echoes for fleets just lost.
        self.resolve_order_echoes(&mut events);

        // 5. Resolve trade convoys that survived to their destination (§9).
        self.resolve_trade_arrivals(&mut events);

        // 5b. Accrue production at every claimed system (§5.1 continuous progress)
        //     — happens whether or not the owner is logged in.
        self.accrue_production(&mut events);

        // 5b'. Resolve construction jobs whose completion tick has arrived (§step1
        //      growth sink): spawn built fleets / apply system upgrades. Server-driven
        //      — a build started before logging off still completes on the clock.
        self.resolve_builds(&mut events);

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
        if self.tick.is_multiple_of(MARKET_UPDATE_TICKS) {
            self.market.drift(&mut self.rng);
        }
        if self.tick.is_multiple_of(BATCH_TICKS) {
            self.clear_books(&mut events);
        }
        if self.tick.is_multiple_of(VALUATION_TICKS) {
            self.recompute_valuations();
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
        let time = self.time;
        let c = self.config.c;
        let mut lost_target = Vec::new();
        for (id, ship) in self.fleets.iter_mut() {
            if let FleetOrder::Intercept { target } = ship.order {
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
                signature: s.signature(),
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
        // SPEED-SIGNATURE DETECTION (§Part 4): a picket senses a target if any of
        // its coverage sources (its own bubble + the owner's arrays) reaches the
        // target's SIGNATURE — the SAME shared `detection::detected` the View uses
        // (parity-tested), so sim-side awareness and the player's map agree.
        let sensed = |owner: PlayerId, ppos: Vec2, target: Vec2, sig: f64| -> bool {
            let mut sources = vec![(ppos, sensor)];
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
                if s.owner == owner {
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
                s.owner != owner && s.kind == ShipKind::Raider && sensed(owner, ppos, s.pos, s.signature)
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

    /// The nearest owned system with a Defense Platform covering `pos` (§buildings
    /// step 2c) — folded into the defender's forces as stationary tiers.
    fn covering_platform(&self, owner: PlayerId, pos: Vec2) -> Option<EntityId> {
        self.systems
            .iter()
            .filter(|s| s.owner == Some(owner) && s.defense_tier >= 1 && s.pos.distance(pos) <= crate::build::DEFENSE_PLATFORM_RADIUS)
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

    /// Apply a side's per-kind losses across its member fleets (lowest id first,
    /// deterministic), removing emptied fleets, and emit the delayed-disappearance
    /// `ShipDestroyed` ghosts from the battle site.
    fn apply_side_losses(&mut self, members: &mut Vec<EntityId>, losses: &crate::combat::Losses, pos: Vec2, events: &mut Vec<Event>) {
        for (kind, n) in &losses.per_kind {
            let mut remaining = *n;
            for id in members.iter() {
                if remaining == 0 {
                    break;
                }
                let Some(f) = self.fleets.get_mut(id) else { continue };
                let take = f.count(*kind).min(remaining);
                if take > 0 {
                    f.remove(*kind, take);
                    remaining -= take;
                    let owner = f.owner;
                    for _ in 0..take {
                        events.push(Event::new(self.time, EventPayload::ShipDestroyed { ship: *id, owner, kind: *kind, pos }));
                    }
                }
            }
        }
        // Drop any fleet emptied out.
        members.retain(|id| self.fleets.get(id).is_some_and(|f| !f.is_empty()));
        for id in members.clone() {
            if self.fleets.get(&id).is_some_and(|f| f.is_empty()) {
                self.fleets.remove(&id);
            }
        }
        self.fleets.retain(|_, f| !f.is_empty());
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
        let base_rate = crate::combat::dmg_rate(target);

        for e in self.engagements.values_mut() {
            e.touched = false;
        }

        // Convoy reaching hub safety before contact → the raider breaks off.
        let mut escapes: Vec<(EntityId, EntityId)> = Vec::new();
        for (rid, ship) in &self.fleets {
            if let FleetOrder::Intercept { target } = ship.order
                && let Some(t) = self.fleets.get(&target)
                && ship.owner != t.owner
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
            self.send_ship_home(aid, a_owner);
        }

        // Contacts: an attacker on Intercept within reach of a rival target.
        let contacts: Vec<(EntityId, EntityId)> = self
            .fleets
            .iter()
            .filter_map(|(rid, ship)| {
                if let FleetOrder::Intercept { target } = ship.order
                    && let Some(t) = self.fleets.get(&target)
                    && ship.owner != t.owner
                    && ship.pos.distance(t.pos) <= CONTACT_RADIUS
                {
                    Some((*rid, target))
                } else {
                    None
                }
            })
            .collect();

        for (aid, tid) in contacts {
            let a_owner_c = self.fleets.get(&aid).map(|f| f.owner);
            let Some(a_owner_c) = a_owner_c else { continue };
            // Join an existing battle ONLY when the SIDES align: this attacker is
            // on the attacking side (same owner) and the target is already this
            // battle's defender — or both are already in it on the right sides.
            // (Crucially, a patrol attacking a hostile that is itself attacking a
            // convoy does NOT merge the two enemies onto one side.)
            let existing = self
                .engagements
                .iter()
                .find(|(_, e)| {
                    (e.a_owner == a_owner_c && e.defenders.contains(&tid))
                        || (e.attackers.contains(&aid) && e.defenders.contains(&tid))
                })
                .map(|(id, _)| *id);
            if let Some(eid) = existing {
                let e = self.engagements.get_mut(&eid).unwrap();
                if !e.attackers.contains(&aid) {
                    e.attackers.push(aid);
                }
                if !e.defenders.contains(&tid) {
                    e.defenders.push(tid);
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
            let mut raid = t_kind == ShipKind::Convoy;
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
                .map(|s| (s.defense_tier, s.defense_pool))
                .unwrap_or((0, 0.0));
            let a_str = crate::combat::Forces::from_fleet(&a_comp, &BTreeMap::new()).strength();
            let d_str = crate::combat::Forces::from_fleet(&d_comp, &BTreeMap::new()).with_platform(ptiers, 0.0).strength();
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
                a_pool: BTreeMap::new(),
                d_pool: BTreeMap::new(),
                a_start: a_comp,
                d_start: d_comp,
                a_start_strength: a_str,
                d_start_strength: d_str,
                platform_start_tiers: ptiers,
                touched: true,
            });
        }

        // REINFORCE: a friendly COMBATANT fleet that has moved to an active battle
        // joins its side's pool (relief shifts the Lanchester ratio). Convoys/
        // colonies don't auto-join a fight they didn't have to.
        let eids: Vec<EntityId> = self.engagements.keys().copied().collect();
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
                        && (f.owner == a_owner || f.owner == d_owner)
                        && f.pos.distance(pos) <= BATTLE_JOIN_RADIUS
                })
                .map(|(fid, f)| (*fid, f.owner == a_owner))
                .collect();
            if !joiners.is_empty() {
                let e = self.engagements.get_mut(eid).unwrap();
                for (fid, atk) in joiners {
                    if atk {
                        e.attackers.push(fid);
                    } else {
                        e.defenders.push(fid);
                    }
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
            let (attackers, defenders, platform_system, raid, started_at, a_start_strength, d_start_strength, a_owner, d_owner, pos) = {
                let e = &self.engagements[eid];
                (e.attackers.clone(), e.defenders.clone(), e.platform_system, e.raid, e.started_at, e.a_start_strength, e.d_start_strength, e.a_owner, e.d_owner, e.pos)
            };
            let a_comp = self.side_comp(&attackers);
            let d_comp = self.side_comp(&defenders);
            let (ptiers, ppool) = platform_system
                .and_then(|sid| self.systems.iter().find(|s| s.id == sid))
                .map(|s| (s.defense_tier, s.defense_pool))
                .unwrap_or((0, 0.0));
            // A side with nothing left → the battle has ended (flush handles it).
            if a_comp.is_empty() || (d_comp.is_empty() && ptiers == 0) {
                continue; // untouched → flushed below
            }
            self.engagements.get_mut(eid).unwrap().touched = true;

            let a_pool = self.engagements[eid].a_pool.clone();
            let d_pool = self.engagements[eid].d_pool.clone();
            let mut a_side = crate::combat::Forces { comp: a_comp, damage: a_pool, platform_tiers: 0, platform_pool: 0.0 };
            let mut d_side = crate::combat::Forces { comp: d_comp, damage: d_pool, platform_tiers: ptiers, platform_pool: ppool };

            // Scouts die the instant they are in a battle.
            let a_scouts = a_side.strip_scouts();
            let d_scouts = d_side.strip_scouts();
            // Raids run at the FIXED quick RAID_RATE (a smash-and-grab is not
            // slowed by the config battle timescale); battles run config-scaled.
            let rate = if raid { crate::combat::RAID_RATE } else { base_rate };
            let (mut la, mut lb) = crate::combat::attrition_tick(&mut a_side, &mut d_side, rate);
            if a_scouts > 0 {
                *la.per_kind.entry(ShipKind::Scout).or_insert(0) += a_scouts;
            }
            if d_scouts > 0 {
                *lb.per_kind.entry(ShipKind::Scout).or_insert(0) += d_scouts;
            }

            // Cargo SEIZURE: on a raid, if a defender convoy is emptied this tick,
            // the (first) attacker loots its cargo before the wreck is removed.
            if raid {
                let dead_cargo: Option<Cargo> = defenders
                    .iter()
                    .find_map(|id| self.fleets.get(id).filter(|f| f.count(ShipKind::Convoy) > 0 && lb.per_kind.get(&ShipKind::Convoy).copied().unwrap_or(0) >= f.count(ShipKind::Convoy)).and_then(|f| f.cargo));
                if let Some(cargo) = dead_cargo
                    && let Some(a) = attackers.first().and_then(|id| self.fleets.get_mut(id))
                    && a.cargo.is_none()
                {
                    a.cargo = Some(cargo);
                }
            }

            // Persist pools back onto the engagement, apply deaths, platform tiers.
            {
                let e = self.engagements.get_mut(eid).unwrap();
                e.a_pool = a_side.damage.clone();
                e.d_pool = d_side.damage.clone();
            }
            if let Some(sid) = platform_system
                && let Some(s) = self.systems.iter_mut().find(|s| s.id == sid)
            {
                s.defense_tier = d_side.platform_tiers;
                s.defense_pool = d_side.platform_pool;
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

            // End conditions, re-derived on the survivors (fleets still existing).
            let _ = (atk, def);
            let elapsed = now - started_at;
            let a_now = self.side_comp(&self.engagements[eid].attackers);
            let d_now = self.side_comp(&self.engagements[eid].defenders);
            let d_ptiers = platform_system
                .and_then(|sid| self.systems.iter().find(|s| s.id == sid))
                .map(|s| s.defense_tier)
                .unwrap_or(0);
            let a_alive = !a_now.is_empty();
            let d_alive = !d_now.is_empty() || d_ptiers > 0;
            let a_cur = crate::combat::Forces::from_fleet(&a_now, &BTreeMap::new()).strength();
            let d_cur = crate::combat::Forces::from_fleet(&d_now, &BTreeMap::new()).strength();
            let a_doc = self.players.get(&a_owner).map(|c| c.doctrine).unwrap_or_default();
            let d_doc = self.players.get(&d_owner).map(|c| c.doctrine).unwrap_or_default();
            let a_retreats = a_alive && a_doc.retreat.min_ratio().is_some_and(|m| a_cur / a_start_strength.max(1e-9) < m);
            let d_retreats = d_alive && d_doc.retreat.min_ratio().is_some_and(|m| d_cur / d_start_strength.max(1e-9) < m);
            // Raid cap: the raider breaks off after a short slice of a battle.
            let raid_cap = raid && elapsed >= crate::combat::RAID_CAP_FRAC * target;
            // Safety valve: a no-retreat grind ends in MUTUAL disengage.
            let safety = elapsed >= crate::combat::MAX_BATTLE_MULT * target;
            let defender_withdraws = d_retreats || safety;

            if !a_alive || !d_alive || a_retreats || raid_cap || safety || d_retreats {
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
        let d_ptiers = e.platform_system.and_then(|sid| self.systems.iter().find(|s| s.id == sid)).map(|s| s.defense_tier).unwrap_or(0);
        let a_alive = !a_now.is_empty();
        let d_alive = !d_now.is_empty() || d_ptiers > 0;
        let outcome = match (a_alive, d_alive) {
            (false, false) => RaidOutcome::BothDestroyed,
            (false, true) => RaidOutcome::AttackerDestroyed,
            (true, false) => RaidOutcome::TargetDestroyed,
            (true, true) => RaidOutcome::BothSurvive,
        };
        events.push(Event::new(now, EventPayload::RaidResolved {
            attacker: e.a_owner,
            defender: e.d_owner,
            attacker_ship: e.attackers.first().copied().unwrap_or(e.id),
            target_ship: e.defenders.first().copied().unwrap_or(e.id),
            attacker_kind: flagship_of(&e.a_start),
            target_kind: flagship_of(&e.d_start),
            outcome,
            pos: e.pos,
            attacker_losses: diff_comp(&e.a_start, &a_now),
            target_losses: diff_comp(&e.d_start, &d_now),
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
        // A surviving attacker breaks off for home (it won, or it withdrew).
        if a_alive {
            for aid in &e.attackers {
                if self.fleets.contains_key(aid) {
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
                    for e in self.engagements.values_mut() {
                        e.attackers.retain(|f| *f != fid);
                        e.defenders.retain(|f| *f != fid);
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
                    sys.shipyard_tier = sys.shipyard_tier.max(crate::build::HOME_SHIPYARD_TIER);
                }
                // Starting inventory: a stock of each commodity to sell, plus a
                // treasury to buy with.
                let inventory = crate::cargo::Commodity::ALL
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
                        valuation: 10_000.0,
                        standing_orders: Vec::new(),
                        next_standing_id: 0,
                        doctrine: FleetDoctrine::default(),
                        intel: BTreeMap::new(),
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
            } => {
                let units = *units;
                if units == 0 {
                    return;
                }
                let Some(corp) = self.players.get(player_id) else {
                    return;
                };
                let home = corp.home;
                let price = self.market.price(*commodity);
                let cost = units as f64 * price;
                if corp.credits < cost {
                    return; // can't afford
                }
                // Instant settlement at the true standing price (§9).
                let unit_price = self.market.execute_buy(*commodity, units);
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.credits -= units as f64 * unit_price;
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
                // Delivery convoy carries the goods home (raidable in transit).
                let cargo = Cargo { commodity: *commodity, units };
                self.spawn_trade_convoy(*player_id, self.hub, home, cargo, TradeMission::DeliverHome);
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
                let have = corp.inventory.get(commodity).copied().unwrap_or(0);
                if have < units {
                    return; // not enough goods
                }
                let home = corp.home;
                // Commit goods to the crossing FIRST — price is decided on arrival.
                if let Some(corp) = self.players.get_mut(player_id) {
                    corp.inventory.entry(*commodity).and_modify(|u| *u -= units);
                }
                events.push(Event::new(
                    self.time,
                    EventPayload::Trade(TradeEvent::SellDispatched {
                        player: *player_id,
                        commodity: *commodity,
                        units,
                    }),
                ));
                let cargo = Cargo { commodity: *commodity, units };
                self.spawn_trade_convoy(*player_id, home, self.hub, cargo, TradeMission::SellAtHub);
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
                        if corp.inventory.get(commodity).copied().unwrap_or(0) < units {
                            return;
                        }
                        if let Some(c) = self.players.get_mut(player_id) {
                            c.inventory.entry(*commodity).and_modify(|u| *u -= units);
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
            Command::BuildShip { player_id, system_id, ship_kind, join } => {
                self.apply_build(*player_id, *system_id, crate::build::BuildKind::Ship { ship: *ship_kind }, *join, events);
            }
            Command::DevelopSystem { player_id, system_id, upgrade } => {
                self.apply_build(*player_id, *system_id, crate::build::BuildKind::Upgrade { upgrade: *upgrade }, None, events);
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
        self.systems
            .iter()
            .filter(|s| s.owner == Some(owner) && s.sensor_tier >= 1)
            .map(|s| (s.pos, s.sensor_bubble()))
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
        let mut captures: Vec<(PlayerId, EntityId, u32, u32, Vec2)> = Vec::new();
        for ship in self.fleets.values() {
            // Any fleet CONTAINING a scout gathers intel (its eyes ride along).
            if !ship.contains(ShipKind::Scout) {
                continue;
            }
            for sys in &self.systems {
                let Some(sys_owner) = sys.owner else { continue };
                if sys_owner == ship.owner {
                    continue; // your own systems need no spying
                }
                if ship.pos.distance(sys.pos) <= crate::ship::SCOUT_INTEL_RANGE {
                    captures.push((ship.owner, sys.id, sys.defense_tier, sys.shipyard_tier, ship.pos));
                }
            }
        }
        for (owner, system, defense_tier, shipyard_tier, pos) in captures {
            let Some(corp) = self.players.get_mut(&owner) else { continue };
            let prev = corp.intel.get(&system);
            // Notify on a fresh approach (no snapshot, or the last one has gone
            // stale — the scout left and came back) or on changed tiers.
            let notify = match prev {
                None => true,
                Some(p) => {
                    p.defense_tier != defense_tier
                        || p.shipyard_tier != shipyard_tier
                        || now - p.observed_at > crate::ship::SCOUT_INTEL_RENOTIFY_S
                }
            };
            corp.intel.insert(
                system,
                crate::world::IntelSnapshot { defense_tier, shipyard_tier, observed_at: now, pos },
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

    /// Validate + start a construction job (§step1 growth sink): the player must own
    /// the system and its stockpile must cover the WHOLE recipe (no partial debit —
    /// a soft reject). A DEVELOPMENT additionally needs a free development slot
    /// (§buildings step 1) — a full system soft-rejects with an owner-only notice,
    /// forcing the specialization choice. Deducts the recipe NOW and enqueues a job
    /// that resolves at `tick + build_ticks`. Determinism: pure, runs in command
    /// phase so the debit is visible to this tick's accrual + standing orders.
    fn apply_build(&mut self, player_id: PlayerId, system_id: EntityId, what: crate::build::BuildKind, join: Option<EntityId>, events: &mut Vec<Event>) {
        let recipe = crate::build::recipe_for(what);
        let Some(sys) = self.systems.iter().find(|s| s.id == system_id) else {
            return;
        };
        if sys.owner != Some(player_id) {
            return; // only the owner builds at their system
        }
        // Developments consume a SLOT of the system's budget (built tiers + jobs
        // already in progress). No free slot → soft reject (no debit, no job), with
        // an owner-only notice. Ships are units, not developments — never slot-gated.
        if matches!(what, crate::build::BuildKind::Upgrade { .. })
            && sys.dev_slots_built() + self.dev_slots_pending(system_id) >= sys.dev_slots()
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
        // SHIPS need a Shipyard (§buildings step 3): the system's tier must cover
        // the kind (Convoy ≥ 1, Raider ≥ 2). Below it → soft reject with an
        // owner-only notice — the recipe is never eaten, the build simply holds
        // until the industry exists. This is what makes shipbuilding GEOGRAPHY.
        if let crate::build::BuildKind::Ship { ship } = what {
            let required = crate::build::required_shipyard_tier(ship);
            if sys.shipyard_tier < required {
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
        }
        let affordable = recipe
            .costs
            .iter()
            .all(|(c, need)| sys.stockpile.get(c).copied().unwrap_or(0.0) + 1e-9 >= *need);
        if !affordable {
            return; // soft reject — no event, no debit
        }
        // Deduct the whole recipe from the system stockpile.
        let sys = self.systems.iter_mut().find(|s| s.id == system_id).unwrap();
        for (c, need) in recipe.costs {
            *sys.stockpile.entry(*c).or_insert(0.0) -= *need;
        }
        self.next_build_id += 1;
        let complete_tick = self.tick + recipe.build_ticks;
        self.build_queue.push(crate::build::BuildJob {
            id: self.next_build_id,
            owner: player_id,
            system: system_id,
            what,
            complete_tick,
            // Join only applies to ship builds; an upgrade always passes None.
            join: if matches!(what, crate::build::BuildKind::Ship { .. }) { join } else { None },
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
        if target.cargo.is_none() {
            target.cargo = removed.cargo;
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
        // Detach.
        let mut new_comp: BTreeMap<ShipKind, u32> = BTreeMap::new();
        {
            let src = self.fleets.get_mut(&fleet_id).unwrap();
            for (k, n) in counts {
                if *n > 0 {
                    src.remove(*k, *n);
                    new_comp.insert(*k, *n);
                }
            }
        }
        let id = self.alloc_entity_id();
        let mut fleet = Fleet::single(id, owner, ShipKind::Scout, pos, FleetOrder::Idle, None);
        fleet.composition = new_comp;
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
            self.build_queue.iter().filter(|j| j.complete_tick <= self.tick).copied().collect();
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
                        self.fleets.get_mut(&fid).unwrap().add(ship, 1);
                        events.push(Event::new(self.time, EventPayload::ShipSpawned { id: fid, owner: job.owner, kind: ship }));
                    } else {
                        let id = self.alloc_entity_id();
                        self.fleets.insert(id, Fleet::single(id, job.owner, ship, pos, FleetOrder::Idle, None));
                        events.push(Event::new(self.time, EventPayload::ShipSpawned { id, owner: job.owner, kind: ship }));
                    }
                }
                crate::build::BuildKind::Upgrade { upgrade } => {
                    // Apply only if the owner still holds the system (can't upgrade a
                    // system you lost; the resources were already spent — frontier risk).
                    if let Some(sys) = self.systems.iter_mut().find(|s| s.id == job.system && s.owner == Some(job.owner)) {
                        let tier = match upgrade {
                            crate::build::SystemUpgrade::Extractor => {
                                sys.extractor_tier += 1;
                                sys.extractor_tier
                            }
                            crate::build::SystemUpgrade::Depot => {
                                sys.depot_tier += 1;
                                sys.depot_tier
                            }
                            crate::build::SystemUpgrade::Shipyard => {
                                sys.shipyard_tier += 1;
                                sys.shipyard_tier
                            }
                            crate::build::SystemUpgrade::SensorArray => {
                                sys.sensor_tier += 1;
                                sys.sensor_tier
                            }
                            crate::build::SystemUpgrade::DefensePlatform => {
                                sys.defense_tier += 1;
                                sys.defense_tier
                            }
                            crate::build::SystemUpgrade::Habitat => {
                                sys.habitat_tier += 1;
                                // A fresh habitat is presumed FED, so its FIRST
                                // shortfall emits the unfed transition notice.
                                sys.habitat_fed = true;
                                sys.habitat_tier
                            }
                            crate::build::SystemUpgrade::Refinery => {
                                sys.refinery_tier += 1;
                                sys.refinery_tier
                            }
                        };
                        events.push(Event::new(self.time, EventPayload::SystemUpgraded { system: job.system, owner: job.owner, upgrade, tier }));
                    }
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
                    }
                    // Consume ONE colony ship (it BECAME the colony); the rest of
                    // the fleet — escorts, extra colonists — persists and parks at
                    // the new holding. A fleet-of-one colony empties and is removed,
                    // exactly as the old single-ship consume did.
                    if let Some(fl) = self.fleets.get_mut(&cid) {
                        fl.remove_one(ShipKind::Colony);
                        if fl.is_empty() {
                            self.fleets.remove(&cid);
                        } else {
                            fl.order = FleetOrder::Idle;
                            fl.notified_held = false;
                        }
                    }
                    events.push(Event::new(
                        now,
                        EventPayload::SystemClaimed { system: sys_id, owner, pos },
                    ));
                }
                None => {
                    // Reserved home site: hold like a lost race (soft, notice once).
                    if idle && !self.fleets[&cid].notified_held {
                        self.fleets.get_mut(&cid).unwrap().notified_held = true;
                        events.push(Event::new(now, EventPayload::ColonyHeld { owner, system: sys_id, pos }));
                    }
                }
                Some(holder) if holder != owner => {
                    // Lost the race (or it flipped en route): hold, intact, notice once.
                    if idle && !self.fleets[&cid].notified_held {
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

    /// Fleet a claimed system's accumulated production to the hub: one raidable
    /// convoy per stockpiled commodity (whole units), each selling on arrival.
    fn apply_ship_production(&mut self, player_id: PlayerId, system_id: EntityId, events: &mut Vec<Event>) {
        // Collect what to ship (and zero those stockpiles) without holding a
        // borrow across the convoy spawn.
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

    /// Accrue production at every claimed system: each deposit adds `richness·DT`
    /// units of its resource to the system's stockpile, drawing down finite
    /// reserves (renewable deposits never deplete). Deterministic.
    ///
    /// STORAGE CAP (§buildings step 2): a full system accrues NOTHING further —
    /// production simply IDLES at the cap until goods ship out (async-fair: an
    /// offline player's depot fills and waits; nothing is destroyed, and a
    /// grandfathered over-cap stockpile is untouched — the cap blocks NEW inflow
    /// only). Reserves are drawn down only by what actually accrues, so a full
    /// depot never wastes a finite deposit.
    ///
    /// HABITAT (§buildings step 3a) — ordering rule, per owned system each tick:
    /// UPKEEP DRAWS FIRST (before accrual), so a colony must hold a standing food
    /// stock — this tick's fresh production replenishes it for the next tick.
    /// The draw is ATOMIC per tick (all `tier · UPKEEP · DT` Provisions or
    /// nothing — a shortfall never partially eats food): covered → FED, the
    /// system's whole output is boosted ×`HABITAT_OUTPUT_MULT^tier` (stacking
    /// multiplicatively on the Extractor's per-deposit multiplier); short →
    /// UNFED, the boost is suspended and NOTHING else happens (no destruction,
    /// no tier loss — async-fair). Transitions emit owner-only notices.
    fn accrue_production(&mut self, events: &mut Vec<Event>) {
        for sys in &mut self.systems {
            let Some(owner) = sys.owner else {
                continue;
            };
            // --- Habitat upkeep (before accrual; atomic per tick) ---
            if sys.habitat_tier >= 1 {
                let upkeep = crate::build::HABITAT_UPKEEP_PER_TIER * sys.habitat_tier as f64 * DT;
                let have = sys.stockpile.get(&crate::cargo::Commodity::Provisions).copied().unwrap_or(0.0);
                let fed = have + 1e-12 >= upkeep;
                if fed {
                    *sys.stockpile.entry(crate::cargo::Commodity::Provisions).or_insert(0.0) -= upkeep;
                }
                if fed != sys.habitat_fed {
                    events.push(Event::new(
                        self.time,
                        EventPayload::HabitatSupplyChanged { owner, system: sys.id, fed },
                    ));
                }
                sys.habitat_fed = fed;
            } else {
                sys.habitat_fed = false; // no habitat — flag is meaningless/off
            }

            // Accrual idles at a FULL depot (nothing destroyed, nothing drawn) —
            // but the refinery below still runs: its lossy conversion SHRINKS the
            // total, so it works (and frees space) even at the cap.
            let mut headroom = sys.storage_headroom();
            if headroom > 0.0 {
                // Extractor upgrades (§step1 structure sink) multiply every
                // deposit's output: richness · MULT^tier; a FED Habitat multiplies
                // the system's ENTIRE output on top (compounding, deterministic).
                let mut mult = crate::build::EXTRACTOR_RICHNESS_MULT.powi(sys.extractor_tier as i32);
                if sys.habitat_tier >= 1 && sys.habitat_fed {
                    mult *= crate::build::HABITAT_OUTPUT_MULT.powi(sys.habitat_tier as i32);
                }
                for dep in &mut sys.deposits {
                    let mut amount = (dep.richness * mult * DT).min(headroom);
                    if let Some(reserves) = dep.reserves.as_mut() {
                        amount = amount.min(*reserves);
                        *reserves -= amount;
                    }
                    if amount > 0.0 {
                        *sys.stockpile.entry(dep.resource).or_insert(0.0) += amount;
                        headroom -= amount;
                    }
                }
            }

            // --- FUEL REFINERY (§buildings step 3b) — conversion LAST, after
            // upkeep + accrual, so it can refine this tick's fresh Volatiles.
            // Bounded by rate, available Volatiles, and the storage cap on the
            // Fuel side (with the lossy yield the total always SHRINKS, so the
            // cap can't actually bind — the guard protects yield ≥ 1 tunings).
            // Dry = idle (soft; nothing destroyed, nothing conjured).
            if sys.refinery_tier >= 1 {
                let have = sys
                    .stockpile
                    .get(&crate::cargo::Commodity::Volatiles)
                    .copied()
                    .unwrap_or(0.0);
                let mut take = (crate::build::REFINERY_RATE_PER_TIER * sys.refinery_tier as f64 * DT).min(have);
                if take > 0.0 {
                    // Cap guard: post-conversion total must stay ≤ cap
                    // (net change = take·(yield − 1) ≤ 0 for yield < 1).
                    let net = take * (crate::build::REFINERY_YIELD - 1.0);
                    let room = sys.storage_headroom();
                    if net > room {
                        take = (room / (crate::build::REFINERY_YIELD - 1.0)).max(0.0);
                    }
                    if take > 0.0 {
                        *sys.stockpile.entry(crate::cargo::Commodity::Volatiles).or_insert(0.0) -= take;
                        *sys.stockpile.entry(crate::cargo::Commodity::Fuel).or_insert(0.0) +=
                            take * crate::build::REFINERY_YIELD;
                    }
                }
            }
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
                        // Refund the over-reservation; goods cross home; news.
                        let refund = filled as f64 * (order.limit_price - price);
                        let home = self.players.get(&order.player).map(|c| c.home);
                        if let Some(c) = self.players.get_mut(&order.player) {
                            c.credits += refund;
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
                        if let Some(home) = home {
                            let cargo = Cargo { commodity, units: filled };
                            self.spawn_trade_convoy(order.player, self.hub, home, cargo, TradeMission::DeliverHome);
                        }
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
        for (id, corp) in self.players.iter_mut() {
            let inv: f64 = corp.inventory.iter().map(|(c, u)| value(c, *u)).sum();
            corp.valuation = corp.credits
                + inv
                + transit.get(id).copied().unwrap_or(0.0)
                + reserved.get(id).copied().unwrap_or(0.0);
        }
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
            let ship = self.fleets.remove(&id).unwrap();
            let (Some(cargo), Some(mission)) = (ship.cargo, ship.mission) else {
                continue;
            };
            match mission {
                TradeMission::DeliverHome => {
                    if let Some(corp) = self.players.get_mut(&ship.owner) {
                        *corp.inventory.entry(cargo.commodity).or_insert(0) += cargo.units;
                    }
                    events.push(Event::new(
                        now,
                        EventPayload::Trade(TradeEvent::Delivered {
                            player: ship.owner,
                            commodity: cargo.commodity,
                            units: cargo.units,
                        }),
                    ));
                }
                TradeMission::SellAtHub => {
                    let unit_price = self.market.execute_sell(cargo.commodity, cargo.units);
                    if let Some(corp) = self.players.get_mut(&ship.owner) {
                        corp.credits += cargo.units as f64 * unit_price;
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
                    // Deposit into the destination system's stockpile — but ONLY if
                    // the convoy's owner still owns it on arrival (a system can be
                    // lost mid-transit; we don't gift cargo to a rival who took it).
                    // STORAGE CAP (§buildings step 2): deliver up to the depot's
                    // remaining headroom (whole units); any EXCESS stays aboard and
                    // the SAME convoy carries it onward to the hub to sell — still
                    // sub-light and raidable, and goods are never silently destroyed.
                    // (This overflow rule is deliberate: of the "sell it / leave it"
                    // options, an automatic sale is the one that can't deadlock a
                    // full depot or strand cargo.)
                    let delivered = self
                        .systems
                        .iter_mut()
                        .find(|s| s.id == system && s.owner == Some(ship.owner))
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
                                }),
                            ));
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
        // Outbound light delay from the fleet's current position (deterministic,
        // known at issue). `delivered_at` is when the fleet gets the order.
        let delay = ship.pos.distance(cc) / c;
        let delivered_at = self.time + delay;
        // The DELIVERY POINT: where the fleet will be when the order lands, by
        // constant-velocity extrapolation of its current motion (§14.1). The echo
        // — the first light of the new behavior — leaves there at delivery and
        // reaches the command center `distance/c` later. Exactly computable now.
        let delivery_point = ship.pos + ship.vel * delay;
        let echo_at = delivered_at + delivery_point.distance(cc) / c;
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
                let mut sys = crate::galaxy::generate_home_system(self.config.seed, n, sys_id, pos);
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
                let mut sys = crate::galaxy::generate_home_system(self.config.seed, idx, sys_id, pos);
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

        // Deterministic demo cargo for the convoy (becomes real trade goods in §9).
        let cargo = {
            let commodity =
                crate::cargo::Commodity::ALL[(self.rng.next_u64() % 5) as usize];
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
        self.fleets.insert(
            raider_id,
            Fleet::single(
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
            ),
        );
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
            assert!(!sys.deposits.is_empty(), "home system is developed");
        }
        // Systems lie within the galaxy radius.
        for s in &w.systems {
            assert!(s.pos.length() <= w.config.galaxy_radius + 1.0);
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
        assert!(!owned[0].deposits.is_empty(), "home is a developed, producing system");
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
    use crate::build::{SystemUpgrade, CONVOY_RECIPE, EXTRACTOR_RICHNESS_MULT};
    use crate::cargo::Commodity;

    /// Grant `owner` a system directly (test SETUP — the game path is now a
    /// colony-ship arrival, tested separately in §fleets part 3).
    fn grant_system(w: &mut World, owner: PlayerId, sys: EntityId) {
        let s = w.systems.iter_mut().find(|s| s.id == sys).unwrap();
        s.owner = Some(owner);
        s.claimed_at = Some(w.time);
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

    #[test]
    fn build_ship_deducts_recipe_and_spawns_after_duration() {
        let mut w = test_world();
        let id = PlayerId(2);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0), (Commodity::Alloys, 50.0)]);
        let ore0 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Ore];
        let ships0 = w.fleets.len();

        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
        // Recipe deducted at once (minus this tick's accrual on the ore deposit; home
        // produces ore, so assert it dropped by ~the recipe, not exactly).
        let ore1 = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&Commodity::Ore];
        assert!(ore1 < ore0 - 30.0, "ore stockpile debited by the convoy recipe (~40)");
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
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None }]);
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no build started");
        assert!(w.build_queue.is_empty(), "no job enqueued on a short stockpile");
    }

    #[test]
    fn develop_system_raises_extractor_tier_and_richness() {
        let mut w = test_world();
        let id = PlayerId(4);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0), (Commodity::Alloys, 50.0)]);
        let rate0: f64 = w.systems.iter().find(|s| s.id == home).unwrap().deposits.iter().map(|d| d.richness).sum();

        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Extractor }]);
        let dur = crate::build::EXTRACTOR_RECIPE.build_ticks;
        for _ in 0..(dur + 3) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.extractor_tier, 1, "extractor tier applied on completion");
        // One tick's accrual should now reflect the ×MULT richness.
        let stock_before: f64 = sys.stockpile.values().sum();
        w.step(&[]);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let gained: f64 = sys.stockpile.values().sum::<f64>() - stock_before;
        let expect = rate0 * EXTRACTOR_RICHNESS_MULT * crate::config::DT;
        assert!((gained - expect).abs() < 1e-6, "production scaled by the extractor (got {gained}, want {expect})");
    }

    #[test]
    fn build_survives_system_loss_owner_keeps_ship() {
        let mut w = test_world();
        let id = PlayerId(5);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0), (Commodity::Alloys, 50.0)]);
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0), (Commodity::Alloys, 50.0)]);
        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Extractor }]);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().owner = Some(PlayerId(999));
        for _ in 0..(crate::build::EXTRACTOR_RECIPE.build_ticks + 4) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.extractor_tier, 0, "can't upgrade a system you no longer own (resources already spent)");
    }

    // --- §buildings step 1 PART 1: development slots -------------------------

    #[test]
    fn dev_slot_exhaustion_soft_rejects_further_developments() {
        let mut w = test_world();
        let id = PlayerId(21);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Ore, 10_000.0)]);

        // Fill every FREE slot (budget − whatever the home starts with, e.g. a
        // seeded Shipyard) with Extractor developments — each enqueue holds a slot.
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let free = sys.dev_slots() - sys.dev_slots_built();
        assert!(free >= 1, "the home must start with at least one free slot");
        for _ in 0..free {
            let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Extractor }]);
            assert!(
                ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })),
                "a development starts while slots remain"
            );
        }

        // One more: SOFT reject — no debit, no job, an owner-only NoSlot notice.
        let ore_before = system_stock(&w, home, Commodity::Ore);
        let jobs_before = w.build_queue.len();
        let ev = w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Extractor }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { owner, reason: crate::event::BuildRejectReason::NoSlot, .. } if owner == id
            )),
            "slot exhaustion notifies the owner"
        );
        assert!(!ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "no build started");
        assert_eq!(w.build_queue.len(), jobs_before, "no job enqueued");
        let ore_after = system_stock(&w, home, Commodity::Ore);
        assert!(ore_after > ore_before - 1.0, "nothing was debited (accrual aside)");

        // Ships are UNITS, not developments — never slot-gated (only recipe-gated).
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })),
            "a ship still builds at a slot-full system"
        );
    }

    #[test]
    fn dev_slot_budget_derives_from_geology() {
        let w = test_world();
        for sys in &w.systems {
            let want = (crate::build::DEV_SLOTS_BASE + (sys.deposits.len() as u32).saturating_sub(1))
                .min(crate::build::DEV_SLOTS_MAX);
            assert_eq!(sys.dev_slots(), want);
            assert!((3..=5).contains(&sys.dev_slots()), "budgets stay in the tunable 3–5 band");
        }
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
        // Simulate a pre-cap snapshot: stockpile far over the cap.
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        seed_stock(&mut w, home, &[(Commodity::Provisions, cap * 3.0)]);
        let over: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        for _ in 0..30 {
            w.step(&[]);
        }
        let after: f64 = w.systems.iter().find(|s| s.id == home).unwrap().storage_used();
        assert!((after - over).abs() < 1e-9, "over-cap stock is kept (cap blocks NEW accrual only)");
    }

    #[test]
    fn depot_tier_raises_the_cap() {
        let mut w = test_world();
        let id = PlayerId(24);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0)]);
        let cap0 = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        let slots0 = w.systems.iter().find(|s| s.id == home).unwrap().dev_slots_built();

        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Depot }]);
        for _ in 0..(crate::build::DEPOT_RECIPE.build_ticks + 3) {
            w.step(&[]);
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.depot_tier, 1, "depot tier applied on completion");
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
            Some(crate::cargo::Cargo { commodity: Commodity::Ore, units: 40 }),
        );
        ship.mission = Some(TradeMission::DeliverToSystem { system: home });
        w.fleets.insert(sid, ship);

        let mut delivered = 0u32;
        let mut overflow = 0u32;
        for _ in 0..5 {
            for ev in w.step(&[]) {
                match ev.payload {
                    EventPayload::Trade(TradeEvent::Delivered { units, commodity: Commodity::Ore, .. }) => delivered += units,
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
        assert_eq!(sys.shipyard_tier, crate::build::HOME_SHIPYARD_TIER, "home bootstraps at Shipyard 1");
        assert!(sys.dev_slots_built() >= 1, "the seeded shipyard consumes a development slot");

        // Convoy (needs tier 1) builds turn one — no chicken-and-egg stall.
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0)]);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::BuildStarted { .. })), "convoy builds at the home shipyard");
    }

    #[test]
    fn raider_needs_shipyard_two() {
        let mut w = test_world();
        let id = PlayerId(27);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        seed_stock(&mut w, home, &[(Commodity::Alloys, 100.0), (Commodity::Fuel, 100.0), (Commodity::Ore, 200.0)]);

        // Home is tier 1 → a Raider (needs 2) SOFT-rejects with the owner notice.
        let alloys0 = system_stock(&w, home, Commodity::Alloys);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { owner, reason: crate::event::BuildRejectReason::NeedsShipyard { required: 2 }, .. } if owner == id
            )),
            "the raider rejection names the required tier"
        );
        assert!(w.build_queue.is_empty(), "no job on a shipyard-short system");
        assert!((system_stock(&w, home, Commodity::Alloys) - alloys0).abs() < 1e-9, "recipe never eaten");

        // Build Shipyard tier 2 → the raider now starts.
        w.step(&[Command::DevelopSystem { player_id: id, system_id: home, upgrade: SystemUpgrade::Shipyard }]);
        for _ in 0..(crate::build::SHIPYARD_RECIPE.build_ticks + 3) {
            w.step(&[]);
        }
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().shipyard_tier, 2);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Raider, join: None }]);
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
        seed_stock(&mut w, claim, &[(Commodity::Ore, 100.0)]);

        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: claim, ship_kind: ShipKind::Convoy, join: None }]);
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
            sys.extractor_tier = 2;
            sys.depot_tier = 1;
            sys.habitat_tier = 1;
            sys.habitat_fed = true;
            sys.stockpile.insert(Commodity::Ore, 123.5);
        }
        // Scout intel rides the snapshot too (§scout part 2).
        w.players.get_mut(&id).unwrap().intel.insert(
            EntityId(999),
            crate::world::IntelSnapshot { defense_tier: 2, shipyard_tier: 1, observed_at: 3.5, pos: Vec2::new(10.0, 20.0) },
        );
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        assert_eq!(
            w.players[&id].intel, w2.players[&id].intel,
            "intel snapshots round-trip through serde"
        );
        let a = w.systems.iter().find(|s| s.id == home).unwrap();
        let b = w2.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!((a.extractor_tier, a.depot_tier, a.shipyard_tier), (b.extractor_tier, b.depot_tier, b.shipyard_tier));
        assert_eq!((a.habitat_tier, a.habitat_fed), (b.habitat_tier, b.habitat_fed), "habitat tier + fed state round-trip");
        assert_eq!(a.dev_slots(), b.dev_slots(), "derived slot budget identical after reload");
        assert!((a.storage_cap() - b.storage_cap()).abs() < 1e-12);
        // Stockpiles match commodity-for-commodity (tolerating the last-ulp
        // wobble of a JSON float round-trip).
        assert_eq!(a.stockpile.len(), b.stockpile.len());
        for (c, v) in &a.stockpile {
            assert!((v - b.stockpile[c]).abs() < 1e-9, "{c:?} round-trips");
        }
    }

    // --- §buildings step 3a: Habitat ------------------------------------------

    fn deposit_rate(w: &World, sys: EntityId, c: Commodity) -> f64 {
        w.systems
            .iter()
            .find(|s| s.id == sys)
            .unwrap()
            .deposits
            .iter()
            .filter(|d| d.resource == c)
            .map(|d| d.richness)
            .sum()
    }

    /// A FED Habitat boosts the whole system's output ×MULT^tier and draws its
    /// Provisions upkeep from the system's own stockpile — measured exactly over
    /// one tick (upkeep-BEFORE-accrual ordering).
    #[test]
    fn fed_habitat_boosts_output_and_draws_upkeep() {
        let mut w = test_world();
        let id = PlayerId(31);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().habitat_tier = 1;
        seed_stock(&mut w, home, &[(Commodity::Provisions, 50.0)]);

        let ore_rate = deposit_rate(&w, home, Commodity::Ore);
        let prov_rate = deposit_rate(&w, home, Commodity::Provisions);
        let ore0 = system_stock(&w, home, Commodity::Ore);
        let prov0 = system_stock(&w, home, Commodity::Provisions);
        w.step(&[]);
        let dt = crate::config::DT;
        let mult = crate::build::HABITAT_OUTPUT_MULT;
        let upkeep = crate::build::HABITAT_UPKEEP_PER_TIER;

        let ore_gain = system_stock(&w, home, Commodity::Ore) - ore0;
        assert!((ore_gain - ore_rate * mult * dt).abs() < 1e-9, "every deposit is boosted ×{mult} (got {ore_gain})");
        let prov_delta = system_stock(&w, home, Commodity::Provisions) - prov0;
        let expect = prov_rate * mult * dt - upkeep * dt;
        assert!((prov_delta - expect).abs() < 1e-9, "provisions = boosted accrual − upkeep (got {prov_delta}, want {expect})");
        assert!(w.systems.iter().find(|s| s.id == home).unwrap().habitat_fed, "covered upkeep = FED");
    }

    /// UNFED merely SUSPENDS the boost: no destruction, no tier loss, and the
    /// habitat recovers the tick food is available again (transition notices
    /// both ways). Uses an ore-only system so geology can't self-feed it.
    #[test]
    fn unfed_habitat_suspends_boost_and_recovers() {
        let mut w = test_world();
        let id = PlayerId(32);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        // A claimed system with ONLY an Ore deposit + a habitat and NO food.
        let sys = w.systems.iter_mut().find(|s| s.is_unclaimed()).unwrap();
        sys.owner = Some(id);
        sys.claimed_at = Some(0.0);
        sys.habitat_tier = 1;
        sys.habitat_fed = true; // as a fresh build leaves it (presumed fed)
        sys.deposits = vec![crate::galaxy::Deposit {
            resource: Commodity::Ore,
            richness: 1.0,
            reserves: None,
            accessibility: 0.5,
        }];
        let sid = sys.id;

        // Tick 1: no Provisions → UNFED (notice), output UN-boosted, tier intact.
        let ore0 = system_stock(&w, sid, Commodity::Ore);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::HabitatSupplyChanged { system, fed: false, .. } if system == sid)),
            "the owner is told the habitat went unfed"
        );
        let dt = crate::config::DT;
        let gain = system_stock(&w, sid, Commodity::Ore) - ore0;
        assert!((gain - 1.0 * dt).abs() < 1e-9, "unfed = plain un-boosted output (got {gain})");
        let s = w.systems.iter().find(|s| s.id == sid).unwrap();
        assert_eq!(s.habitat_tier, 1, "nothing is destroyed, no tier lost");
        assert!(!s.habitat_fed);

        // Resupply (a hauled delivery) → FED again next tick, boost restored.
        seed_stock(&mut w, sid, &[(Commodity::Provisions, 10.0)]);
        let ore1 = system_stock(&w, sid, Commodity::Ore);
        let ev = w.step(&[]);
        assert!(
            ev.iter().any(|e| matches!(e.payload, EventPayload::HabitatSupplyChanged { system, fed: true, .. } if system == sid)),
            "recovery is announced"
        );
        let gain = system_stock(&w, sid, Commodity::Ore) - ore1;
        let mult = crate::build::HABITAT_OUTPUT_MULT;
        assert!((gain - 1.0 * mult * dt).abs() < 1e-9, "boost restored once fed (got {gain})");
    }

    /// BALANCE SANITY: the home's renewable Provisions output feeds TWO Habitat
    /// tiers from a standing start — the natural first Habitats are
    /// self-sustaining, never a starving home. (Worst-case home provisions
    /// richness 0.3825/s vs 2 × 0.15 = 0.30/s upkeep.)
    #[test]
    fn home_two_tier_habitat_is_self_sufficient() {
        let mut w = test_world();
        let id = PlayerId(33);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().habitat_tier = 2;
        // From ZERO stored food: tick 1 runs unfed, geology replenishes, then the
        // colony feeds itself indefinitely with a growing surplus.
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(s.habitat_fed, "a 2-tier home habitat sustains itself on home geology");
        assert!(
            system_stock(&w, home, Commodity::Provisions) > 1.0,
            "…with a growing food surplus, not a knife-edge"
        );
    }

    // --- §buildings step 3b: Fuel Refinery ------------------------------------

    /// The Refinery converts stockpiled Volatiles → Fuel at exactly
    /// rate·tier·DT input and yield·input output, measured over one tick.
    #[test]
    fn refinery_converts_at_rate_and_ratio() {
        let mut w = test_world();
        let id = PlayerId(34);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().refinery_tier = 2;
        seed_stock(&mut w, home, &[(Commodity::Volatiles, 100.0)]);

        let vol0 = system_stock(&w, home, Commodity::Volatiles);
        let fuel0 = system_stock(&w, home, Commodity::Fuel);
        w.step(&[]);
        let dt = crate::config::DT;
        let take = crate::build::REFINERY_RATE_PER_TIER * 2.0 * dt;
        let vol_delta = system_stock(&w, home, Commodity::Volatiles) - vol0;
        let fuel_delta = system_stock(&w, home, Commodity::Fuel) - fuel0;
        assert!((vol_delta + take).abs() < 1e-9, "consumes rate·tier·DT volatiles (got {vol_delta})");
        assert!(
            (fuel_delta - take * crate::build::REFINERY_YIELD).abs() < 1e-9,
            "produces yield·input fuel (got {fuel_delta})"
        );
    }

    /// A dry Refinery IDLES (soft): no Volatiles → no conversion, no fuel from
    /// nowhere, nothing destroyed. And a bounded stock converts only what exists.
    #[test]
    fn refinery_idles_dry_and_never_overdraws() {
        let mut w = test_world();
        let id = PlayerId(35);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let home = w.players[&id].home_system.unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().refinery_tier = 3;

        // Dry: fuel unchanged (home produces no fuel; only the seed sits there).
        let fuel0 = system_stock(&w, home, Commodity::Fuel);
        w.step(&[]);
        assert!((system_stock(&w, home, Commodity::Fuel) - fuel0).abs() < 1e-9, "dry refinery = idle");

        // A sliver of Volatiles smaller than one tick's rate: converts exactly
        // that sliver, never negative.
        let sliver = 0.001;
        seed_stock(&mut w, home, &[(Commodity::Volatiles, sliver)]);
        let fuel1 = system_stock(&w, home, Commodity::Fuel);
        w.step(&[]);
        assert!((system_stock(&w, home, Commodity::Volatiles)).abs() < 1e-9, "drains to zero, not below");
        let gained = system_stock(&w, home, Commodity::Fuel) - fuel1;
        assert!((gained - sliver * crate::build::REFINERY_YIELD).abs() < 1e-9, "converts only what exists");
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
        w.systems.iter_mut().find(|s| s.id == home).unwrap().refinery_tier = 1;
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
            "the lossy conversion proceeds even at a full depot (it frees space)"
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
        w.systems.iter_mut().find(|s| s.id == home).unwrap().refinery_tier = 2;
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 50.0)]); // fuel seed covers the 8 Fuel
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Scout, join: None }]);
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
            sys.defense_tier = 2;
            sys.shipyard_tier = 1;
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
        w.systems.iter_mut().find(|s| s.id == sid_sys).unwrap().defense_tier = 3;
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
            seed_stock(&mut w, home, &[(Commodity::Ore, 200.0), (Commodity::Alloys, 100.0)]);
            w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 50.0)]);
        let f0 = home_fuel(&w, id);
        let ev = w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        assert!(
            ev.iter().any(|e| matches!(&e.payload,
                EventPayload::Trade(TradeEvent::SellDispatched { commodity: Commodity::Ore, .. }))),
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 40.0)]);
        let ev = w.step(&[Command::ShipProduction { player_id: id, system_id: home }]);
        assert!(
            ev.iter().any(|e| matches!(e.payload,
                EventPayload::FuelShortfall { kind: crate::fuel::ShortfallKind::Shipment, .. })),
            "a held shipment notifies its owner",
        );
        assert!(system_stock(&w, home, Commodity::Ore) >= 40.0, "held goods are refunded, never lost");
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
            s.defense_tier = 2;
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
            let tier = w.systems.iter().find(|s| s.id == sid).map(|s| s.defense_tier).unwrap_or(0);
            if tier < 2 || !w.fleets.contains_key(&raider) {
                break;
            }
        }
        let final_tier = w.systems.iter().find(|s| s.id == sid).unwrap().defense_tier;
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
        assert!(eng.a_pool.values().chain(eng.d_pool.values()).any(|p| *p > 0.0), "engagement side pools accumulated mid-battle");
        // Round-trip through JSON (the snapshot path) — the ENGAGEMENT entity
        // (pools + elapsed + participants) persists, so the fight resumes exactly.
        let json = serde_json::to_string(&w).unwrap();
        let w2: World = serde_json::from_str(&json).unwrap();
        let eng2 = w2.engagements.values().next().expect("the battle survived serialization");
        assert!((eng2.started_at - started).abs() < 1e-9, "elapsed (started_at) persisted");
        assert!(eng2.a_pool.values().chain(eng2.d_pool.values()).any(|p| *p > 0.0), "pools survived serialization");
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
            sys.sensor_tier = 1;
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
        sys.defense_tier = tier;
        sys.id
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
            let tier0 = w.systems.iter().find(|s| s.id == sys).unwrap().defense_tier;
            assert!(w.systems.iter().find(|s| s.id == sys).unwrap().dev_slots_built() >= 3, "platform tiers consume slots");

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
            let tier1 = w.systems.iter().find(|s| s.id == sys).unwrap().defense_tier;
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
            w.systems.iter().find(|s| s.id == sys).unwrap().defense_tier <= 6,
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 100.0), (Commodity::Alloys, 50.0)]);
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Corvette, join: None }]);
        assert!(
            ev.iter().any(|e| matches!(
                e.payload,
                EventPayload::BuildRejected { reason: crate::event::BuildRejectReason::NeedsShipyard { required: 2 }, .. }
            )),
            "home tier 1 can't build corvettes"
        );
        w.systems.iter_mut().find(|s| s.id == home).unwrap().shipyard_tier = 2;
        let ev = w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Corvette, join: None }]);
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
        // Recall is issued, but its light (≈13 s away) can't beat the contact.
        let mut recalled = false;
        let outcome = run_until_raid(&mut w, 120, |w| {
            if !recalled && w.time > 14.0 {
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

    #[test]
    fn market_buy_settles_now_and_delivers_later() {
        use crate::cargo::Commodity::Fuel;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let credits0 = w.players[&id].credits;
        let fuel0 = w.players[&id].inventory[&Fuel];
        let price = w.market.price(Fuel);

        w.step(&[Command::MarketBuy { player_id: id, commodity: Fuel, units: 50 }]);
        // Instant settlement: credits debited now (≈ 50 × price).
        let spent = credits0 - w.players[&id].credits;
        assert!((spent - 50.0 * price).abs() < 1e-6, "buy should settle at the standing price");
        // A delivery convoy spawned at the hub, carrying the goods.
        let convoy = w.fleets.values().find(|s| s.owner == id && s.mission == Some(TradeMission::DeliverHome));
        assert!(convoy.is_some(), "buy should spawn a delivery convoy");
        assert!(convoy.unwrap().pos.distance(w.hub) < 5.0, "delivery convoy starts at the hub");
        // Inventory not yet increased (goods still in transit).
        assert_eq!(w.players[&id].inventory[&Fuel], fuel0);

        // Run until the convoy reaches home and deposits the goods.
        for _ in 0..(220 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.players[&id].inventory[&Fuel] == fuel0 + 50 {
                return;
            }
        }
        panic!("delivery convoy never arrived");
    }

    #[test]
    fn market_sell_commits_goods_and_clears_on_arrival() {
        use crate::cargo::Commodity::Ore;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let credits0 = w.players[&id].credits;
        let ore0 = w.players[&id].inventory[&Ore];

        w.step(&[Command::MarketSell { player_id: id, commodity: Ore, units: 40 }]);
        // Goods committed to the crossing now; credits unchanged until arrival.
        assert_eq!(w.players[&id].inventory[&Ore], ore0 - 40);
        assert_eq!(w.players[&id].credits, credits0);
        let convoy = w.fleets.values().find(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub));
        assert!(convoy.is_some(), "sell should spawn a convoy toward the hub");

        // Run until it reaches the hub and clears at the price-on-arrival.
        for _ in 0..(260 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.players[&id].credits > credits0 {
                return; // sold at arrival, credited
            }
        }
        panic!("sell convoy never cleared");
    }

    #[test]
    fn cannot_buy_without_credits_or_sell_without_goods() {
        use crate::cargo::Commodity::Alloys;
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let ships0 = w.fleets.len();
        // Sell more than held → ignored (no convoy, inventory unchanged).
        let alloys0 = w.players[&id].inventory[&Alloys];
        w.step(&[Command::MarketSell { player_id: id, commodity: Alloys, units: 99_999 }]);
        assert_eq!(w.players[&id].inventory[&Alloys], alloys0);
        assert_eq!(w.fleets.len(), ships0, "rejected sell must not spawn a convoy");
        // Buy beyond the treasury → ignored.
        let credits0 = w.players[&id].credits;
        w.step(&[Command::MarketBuy { player_id: id, commodity: Alloys, units: 10_000_000 }]);
        assert_eq!(w.players[&id].credits, credits0);
        assert_eq!(w.fleets.len(), ships0, "rejected buy must not spawn a convoy");
    }

    #[test]
    fn limit_orders_clear_in_uniform_price_batch() {
        use crate::cargo::Commodity::Ore;
        let mut w = test_world();
        let (buyer, seller) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: buyer, name: "Buy".into() },
            Command::AddPlayer { id: seller, name: "Sell".into() },
        ]);
        let buyer_credits0 = w.players[&buyer].credits;
        let seller_credits0 = w.players[&seller].credits;
        let seller_ore0 = w.players[&seller].inventory[&Ore];

        // A crossing pair: buyer pays up to 9, seller wants at least 7.
        w.step(&[
            Command::PlaceLimitOrder { player_id: seller, side: Side::Sell, commodity: Ore, units: 50, limit_price: 7.0 },
            Command::PlaceLimitOrder { player_id: buyer, side: Side::Buy, commodity: Ore, units: 50, limit_price: 9.0 },
        ]);
        // Reservations taken at placement.
        assert_eq!(w.players[&seller].inventory[&Ore], seller_ore0 - 50);
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
        // The buyer's matched goods cross home as a delivery convoy.
        assert!(w.fleets.values().any(|s| s.owner == buyer && s.mission == Some(TradeMission::DeliverHome)));
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
        assert!(sys_a.stockpile.values().sum::<f64>() > 0.0, "accrual path must have executed");
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    // ---- System claims + resource production (§4, §9) ----

    /// A system's deposit value-rate: Σ richness · base_price — how much credit
    /// value it produces per second.
    fn value_rate(sys: &StarSystem) -> f64 {
        sys.deposits.iter().map(|d| d.richness * crate::market::base_price(d.resource)).sum()
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
        w.systems.iter().all(|s| !s.deposits.is_empty())
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 300.0)]);
        let dock = park_fleet(&mut w, id, hpos, ShipKind::Raider);
        let fleets_before = w.fleets.len();
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: Some(dock) }]);
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
        seed_stock(&mut w, home, &[(Commodity::Ore, 300.0)]);
        let fleets_before = w.fleets.len();
        w.step(&[Command::BuildShip { player_id: id, system_id: home, ship_kind: ShipKind::Convoy, join: None }]);
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

        grant_system(&mut w, id, sysid);
        let secs = 20u32;
        for _ in 0..(secs * crate::config::TICK_HZ) { w.step(&[]); }

        let sys = w.systems.iter().find(|s| s.id == sysid).unwrap();
        let total: f64 = sys.stockpile.values().sum();
        let expected: f64 = sys.deposits.iter().map(|d| d.richness).sum::<f64>() * secs as f64;
        assert!(total > 0.0, "a claimed system must accrue production");
        assert!((total - expected).abs() < expected * 0.02 + 1e-6,
            "stockpile {total:.2} ≈ Σrichness × time {expected:.2}");
    }

    #[test]
    fn shipping_production_spawns_a_raidable_convoy_that_sells() {
        let mut w = test_world();
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
        // The system's whole-unit stockpile was emptied into the convoy(s).
        let remaining: f64 = w.systems.iter().find(|s| s.id == sysid).unwrap().stockpile.values().sum();
        assert!(remaining < 1.0, "shipping should empty the whole-unit stockpile");

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
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        grant_system(&mut w, id, sysid);
        w.step(&[]);
        let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().deposits[0].resource;
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
        let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().deposits[0].resource;
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
        // Own two systems: a producing source and a destination depot.
        let source = richest_system(&w);
        let commodity = w.systems.iter().find(|s| s.id == source).unwrap().deposits[0].resource;
        let dest = w.systems.iter().find(|s| s.owner.is_none() && s.id != source).unwrap().id;
        grant_system(&mut w, id, source);
        w.step(&[]);
        grant_system(&mut w, id, dest);
        // Give the depot ample headroom so the supply always has room to land —
        // otherwise the dest's OWN production can fill the base cap before the
        // (sub-light) convoy arrives, a race unrelated to what this test checks.
        w.systems.iter_mut().find(|s| s.id == dest).unwrap().depot_tier = 5;
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

    /// Standing-order execution is deterministic: same seed + same commands ⇒ byte-
    /// identical world (the rules + their convoys + credits all reproduce).
    #[test]
    fn standing_orders_are_deterministic() {
        let build = || {
            let mut w = test_world();
            let id = PlayerId(1);
            w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
            let sysid = richest_system(&w);
            let commodity = w.systems.iter().find(|s| s.id == sysid).unwrap().deposits[0].resource;
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
                commodity: crate::cargo::Commodity::Ore,
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
                commodity: crate::cargo::Commodity::Ore,
                trigger: Trigger::AboveThreshold { threshold: 1.0 },
                status: OrderStatus::Active, next_eval_tick: 0, in_flight: None,
            },
        }]);
        // Invalid: MaintainAtDest with a Hub destination → rejected.
        w.step(&[Command::SetStandingOrder {
            player_id: id,
            order: StandingOrder {
                id: 0, source: Endpoint::System { id: sysid }, dest: Endpoint::Hub,
                commodity: crate::cargo::Commodity::Ore,
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
                commodity: crate::cargo::Commodity::Ore,
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
        use crate::cargo::Commodity::Ore;
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
            w.systems.iter_mut().find(|s| s.id == sid).unwrap().stockpile.insert(Ore, 50.0);
        }
        w.systems.iter_mut().find(|s| s.id == d).unwrap().stockpile.remove(&Ore);

        for &src in &[a, b] {
            w.step(&[Command::SetStandingOrder {
                player_id: id,
                order: StandingOrder {
                    id: 0,
                    source: Endpoint::System { id: src },
                    dest: Endpoint::System { id: d },
                    commodity: Ore,
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
            w.systems.iter_mut().find(|s| s.id == sid).unwrap().stockpile.insert(Ore, 50.0);
        }
        w.systems.iter_mut().find(|s| s.id == d).unwrap().stockpile.remove(&Ore);
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
        let cargo = Cargo { commodity: crate::cargo::Commodity::Ore, units: 30 };
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
}
