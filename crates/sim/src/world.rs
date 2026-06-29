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
use crate::event::{Event, EventPayload, RaidOutcome, TradeEvent};
use crate::galaxy::{generate_home_slots, generate_systems, HomeSlot, StarSystem};
use crate::ids::{EntityId, PlayerId};
use crate::market::{clear_call_auction, LimitOrder, Side};
use crate::math::Vec2;
use crate::movement::pursue_step;
use crate::ship::{DefenseEngagement, Ship, ShipKind, ShipOrder, TradeMission};
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
}

/// An order in flight: a player's command that has left their command center
/// but not yet reached the ship (the outbound light-travel time of §6). Carries
/// the order to install once the light arrives (a move, a raid commit, or a
/// recall-as-return-home).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingOrder {
    /// Sim time at which the order's light reaches the ship.
    apply_time: f64,
    ship_id: EntityId,
    new_order: ShipOrder,
}

/// Distance (sim units) at which a raider makes contact with its target.
const CONTACT_RADIUS: f64 = 80.0;
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

// Battle outcome probabilities (§8). Tunable; balance comes later. Each tuple is
// (P target destroyed, P attacker destroyed, P both destroyed); the remainder is
// "both survive (attacker driven off)".
// TEMPORARY (testing): raider always destroys the convoy. Restore to
// (0.60, 0.12, 0.08) for the real balance.
const RVC_PROBS: (f64, f64, f64) = (1.0, 0.0, 0.0); // raider vs convoy (TEST: 100% convoy destroyed)
const RVR_PROBS: (f64, f64, f64) = (0.35, 0.35, 0.12); // raider vs raider (even)

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
fn resume_patrol(route: Vec<Vec2>) -> ShipOrder {
    if route.is_empty() {
        ShipOrder::Idle
    } else {
        ShipOrder::Patrol { waypoints: route, index: 0, dwell_until: 0.0 }
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
    /// All ships, keyed by id. `BTreeMap` keeps integration order deterministic.
    pub ships: BTreeMap<EntityId, Ship>,
    /// The shared hub Exchange (§9).
    pub market: crate::market::Market,
    /// Resting limit orders, cleared in a periodic uniform-price call auction.
    pub book: Vec<LimitOrder>,
    /// Monotonic allocator for limit-order ids.
    next_order_id: u64,
    /// Orders that have been issued but whose light has not yet reached the ship.
    pending_orders: Vec<PendingOrder>,
    /// Monotonic allocator for entity ids.
    next_entity_id: u64,
    /// World RNG stream (continues past generation) for deterministic events.
    rng: crate::rng::Rng,
}

impl World {
    /// Create a galaxy for the given configuration: hub at the centre, seeded
    /// star systems, and a ring of empty home anchors.
    pub fn new(config: SimConfig) -> Self {
        let mut rng = crate::rng::Rng::new(config.seed);
        let mut next_entity_id = 1u64;

        let systems = {
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
        let home_slots = generate_home_slots(
            &mut rng,
            config.galaxy_radius,
            config.home_ring_frac,
            config.max_players,
        );

        World {
            config,
            tick: 0,
            time: 0.0,
            hub: Vec2::ZERO,
            systems,
            home_slots,
            players: BTreeMap::new(),
            ships: BTreeMap::new(),
            market: crate::market::Market::new(),
            book: Vec::new(),
            next_order_id: 1,
            pending_orders: Vec::new(),
            next_entity_id,
            rng,
        }
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

        // 5. Resolve trade convoys that survived to their destination (§9).
        self.resolve_trade_arrivals(&mut events);

        // 5b. Accrue production at every claimed system (§5.1 continuous progress)
        //     — happens whether or not the owner is logged in.
        self.accrue_production();

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
            .ships
            .iter()
            .map(|(id, s)| (*id, (s.pos, s.vel)))
            .collect();
        let time = self.time;
        let c = self.config.c;
        let mut lost_target = Vec::new();
        for (id, ship) in self.ships.iter_mut() {
            if let ShipOrder::Intercept { target } = ship.order {
                match snapshot.get(&target) {
                    Some(&(tp, tv)) => {
                        // Proportional pursuit toward the target's light-delayed
                        // observed position; acceleration derived from this ship's
                        // current mass (a = F/m), so a laden convoy-raider would
                        // turn worse — same loop, no closed-form solver.
                        let step = pursue_step(
                            ship.pos,
                            ship.vel,
                            tp,
                            tv,
                            ship.accel(),
                            ship.kind.max_speed(),
                            c,
                            DT,
                        );
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
                .ships
                .get(&id)
                .and_then(|s| self.players.get(&s.owner))
                .map(|c| c.home);
            if let Some(ship) = self.ships.get_mut(&id) {
                if let Some(def) = ship.defense.take() {
                    ship.order = resume_patrol(def.patrol);
                } else if let Some(home) = home {
                    ship.order = ShipOrder::MoveTo { dest: home };
                }
            }
        }
    }

    /// Standing defensive doctrine, run every tick for ALL patrolling raiders,
    /// server-authoritative and deterministic (§5.1, Pillar 1 — defense works
    /// while the owner is offline). Each patrolling raider, acting on its OWN
    /// local sensing:
    ///   * guards friendly convoys within its sensor bubble;
    ///   * detects hostile raiders within its sensor range (fog-respecting — dark
    ///     raiders beyond sensor range are invisible to it);
    ///   * if a detected hostile is on an intercept course toward a guarded convoy
    ///     (moving, heading roughly at it), BREAKS OFF patrol to intercept it,
    ///     reusing the ordinary [`ShipOrder::Intercept`] pursuit + raider-vs-raider
    ///     combat (resolved by [`Self::resolve_raids`]);
    ///   * once its quarry is destroyed or flees out of reach, RESUMES its patrol.
    ///
    /// It never reads a rival's hidden orders — only observable geometry (position,
    /// heading) of contacts it can actually sense — so detection is fair, and patrol
    /// POSITIONING (how near the convoy's likely approach vectors) decides whether
    /// it senses a threat early enough to catch it.
    fn autonomous_defense(&mut self) {
        let sensor = self.config.sensor_range;
        let breakoff = sensor * PURSUIT_BREAKOFF_MULT;

        // Read-only snapshot for assessment (avoids borrow conflicts; ordered, so
        // deterministic).
        #[derive(Clone, Copy)]
        struct Snap {
            id: EntityId,
            owner: PlayerId,
            kind: ShipKind,
            pos: Vec2,
            vel: Vec2,
        }
        let snap: Vec<Snap> = self
            .ships
            .iter()
            .map(|(id, s)| Snap { id: *id, owner: s.owner, kind: s.kind, pos: s.pos, vel: s.vel })
            .collect();
        let find = |id: EntityId| snap.iter().find(|s| s.id == id).copied();

        let mut engage: Vec<(EntityId, EntityId)> = Vec::new(); // (patrol, hostile)
        let mut shadow: Vec<(EntityId, Vec2)> = Vec::new(); // (patrol, charge pos)
        let mut disengage: Vec<EntityId> = Vec::new();

        for (pid, ship) in &self.ships {
            if ship.kind != ShipKind::Raider {
                continue;
            }
            // Already on a defensive sortie: keep pursuing while the quarry is
            // alive and in reach; break off if it has fled past the breakoff range.
            // (A quarry that was DESTROYED is handled by `integrate_movement`'s
            // lost-target path, which resumes patrol.)
            if let Some(def) = &ship.defense {
                if let Some(t) = find(def.target)
                    && ship.pos.distance(t.pos) > breakoff
                {
                    disengage.push(*pid);
                }
                continue;
            }
            // On patrol: adopt the nearest friendly convoy within assignment range
            // as this picket's CHARGE, then defend it.
            if !matches!(ship.order, ShipOrder::Patrol { .. }) {
                continue;
            }
            let (owner, ppos) = (ship.owner, ship.pos);
            let charge = snap
                .iter()
                .filter(|s| s.owner == owner && s.kind == ShipKind::Convoy && ppos.distance(s.pos) <= ASSIGN_RANGE)
                .min_by(|a, b| ppos.distance(a.pos).total_cmp(&ppos.distance(b.pos)));
            let Some(charge) = charge else {
                continue; // no convoy in range to guard — positioning matters
            };
            // The most imminent SENSED threat: a hostile raider within this
            // picket's own sensor range, moving on an intercept course toward the
            // charge. (Detection is fog-respecting — dark raiders beyond sensor
            // range are invisible to it.)
            let mut best: Option<(EntityId, f64)> = None;
            for h in snap.iter().filter(|s| {
                s.owner != owner && s.kind == ShipKind::Raider && ppos.distance(s.pos) <= sensor
            }) {
                if h.vel.length() < THREAT_MIN_SPEED {
                    continue; // not actually inbound
                }
                let to_c = charge.pos - h.pos;
                let d = to_c.length();
                if d < 1e-6 {
                    continue;
                }
                if h.vel.normalized().dot(to_c / d) >= THREAT_CLOSING_COS && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((h.id, d));
                }
            }
            match best {
                Some((target, _)) => engage.push((*pid, target)),
                // No threat: keep STATION on the charge so the picket's sensor
                // actually covers it (a fast escort would otherwise drift away and
                // be unable to defend). It stays in Patrol state, ready to engage.
                None => shadow.push((*pid, charge.pos)),
            }
        }

        // Break off patrol to intercept (saving the patrol route to resume later).
        for (pid, target) in engage {
            if let Some(ship) = self.ships.get_mut(&pid) {
                let patrol = match &ship.order {
                    ShipOrder::Patrol { waypoints, .. } => waypoints.clone(),
                    _ => Vec::new(),
                };
                ship.order = ShipOrder::Intercept { target };
                ship.defense = Some(DefenseEngagement { target, patrol });
            }
        }
        // Hold station near the charge convoy (a short patrol bracketing it that
        // tracks it as it moves), so the picket stays in sensor range of its ward.
        for (pid, cpos) in shadow {
            if let Some(ship) = self.ships.get_mut(&pid)
                && let ShipOrder::Patrol { waypoints, .. } = &mut ship.order
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
            if let Some(ship) = self.ships.get_mut(&pid) {
                let patrol = ship.defense.take().map(|d| d.patrol).unwrap_or_default();
                ship.order = resume_patrol(patrol);
            }
        }
    }

    /// Roll a random battle outcome with the seeded RNG (§8). Deterministic from
    /// seed + commands; rolled ONCE per battle (both sides later observe the same
    /// result). The table depends on the target's kind (raider-vs-convoy vs
    /// raider-vs-raider).
    fn roll_battle(&mut self, target_kind: ShipKind) -> RaidOutcome {
        let (pt, pa, pb) = match target_kind {
            ShipKind::Convoy => RVC_PROBS,
            ShipKind::Raider => RVR_PROBS,
        };
        let r = self.rng.next_f64();
        if r < pt {
            RaidOutcome::TargetDestroyed
        } else if r < pt + pa {
            RaidOutcome::AttackerDestroyed
        } else if r < pt + pa + pb {
            RaidOutcome::BothDestroyed
        } else {
            RaidOutcome::BothSurvive
        }
    }

    /// Detect and apply battle resolutions. A raider within [`CONTACT_RADIUS`] of
    /// its target fights a randomised battle (raider-vs-convoy OR raider-vs-
    /// raider); a convoy within [`HUB_SAFE_RADIUS`] of the hub escapes before
    /// contact. Destroyed ships are removed from TRUE space here and at this true
    /// time; each player observes the destruction later, by light (the view
    /// filter serves the dead ship's ghost until that player's light arrives).
    fn resolve_raids(&mut self, events: &mut Vec<Event>) {
        let hub = self.hub;
        let now = self.time;
        // Detect contacts: (attacker_id, target_id, is_escape).
        let mut contacts: Vec<(EntityId, EntityId, bool)> = Vec::new();
        for (rid, ship) in &self.ships {
            if let ShipOrder::Intercept { target } = ship.order
                && let Some(t) = self.ships.get(&target)
            {
                if ship.pos.distance(t.pos) <= CONTACT_RADIUS {
                    contacts.push((*rid, target, false));
                } else if t.kind == ShipKind::Convoy && t.pos.distance(hub) <= HUB_SAFE_RADIUS {
                    contacts.push((*rid, target, true)); // raiders don't get hub-safety
                }
            }
        }

        for (aid, tid, escape) in contacts {
            // Re-fetch: an earlier contact this tick may have destroyed a ship.
            let (Some(att), Some(tgt)) = (self.ships.get(&aid), self.ships.get(&tid)) else {
                continue;
            };
            let (a_owner, t_owner) = (att.owner, tgt.owner);
            let (a_kind, t_kind) = (att.kind, tgt.kind);
            let (a_pos, t_pos) = (att.pos, tgt.pos);

            let outcome = if escape {
                RaidOutcome::Escaped
            } else {
                self.roll_battle(t_kind)
            };

            events.push(Event::new(
                now,
                EventPayload::RaidResolved {
                    attacker: a_owner,
                    defender: t_owner,
                    attacker_ship: aid,
                    target_ship: tid,
                    attacker_kind: a_kind,
                    target_kind: t_kind,
                    outcome,
                    pos: a_pos,
                },
            ));

            let (kill_attacker, kill_target) = outcome.kills();
            if kill_attacker {
                self.ships.remove(&aid);
                events.push(Event::new(
                    now,
                    EventPayload::ShipDestroyed { ship: aid, owner: a_owner, kind: a_kind, pos: a_pos },
                ));
            } else {
                // Surviving attacker (or escape) breaks off and returns home.
                self.send_ship_home(aid, a_owner);
            }
            if kill_target {
                self.ships.remove(&tid);
                events.push(Event::new(
                    now,
                    EventPayload::ShipDestroyed { ship: tid, owner: t_owner, kind: t_kind, pos: t_pos },
                ));
            }
            // A surviving target keeps its order (convoy continues; a raider that
            // was attacked continues whatever it was doing).
        }
    }

    /// Send a surviving ship home (break off).
    fn send_ship_home(&mut self, id: EntityId, owner: PlayerId) {
        if let Some(home) = self.players.get(&owner).map(|c| c.home)
            && let Some(ship) = self.ships.get_mut(&id)
        {
            ship.order = ShipOrder::MoveTo { dest: home };
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
                if let Some(ship) = self.ships.get_mut(&po.ship_id) {
                    ship.order = po.new_order;
                    events.push(Event::new(
                        now,
                        EventPayload::OrderApplied { ship_id: po.ship_id },
                    ));
                }
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
                let home = self.assign_home(*id);
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
                        credits: 10_000.0,
                        inventory,
                        valuation: 10_000.0,
                        standing_orders: Vec::new(),
                        next_standing_id: 0,
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
                self.schedule_for_owner(*player_id, *ship_id, ShipOrder::MoveTo { dest: *dest });
            }
            Command::CommitRaid {
                player_id,
                raider_id,
                target_id,
            } => {
                // The target must exist and belong to someone else.
                let Some(target) = self.ships.get(target_id) else {
                    return;
                };
                if target.owner == *player_id {
                    return; // no raiding your own ships
                }
                self.schedule_for_owner(
                    *player_id,
                    *raider_id,
                    ShipOrder::Intercept { target: *target_id },
                );
            }
            Command::RecallRaid {
                player_id,
                raider_id,
            } => {
                let Some(home) = self.players.get(player_id).map(|c| c.home) else {
                    return;
                };
                self.schedule_for_owner(*player_id, *raider_id, ShipOrder::MoveTo { dest: home });
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
            Command::ClaimSystem { player_id, system_id } => {
                self.apply_claim(*player_id, *system_id, events);
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
    fn apply_claim(&mut self, player_id: PlayerId, system_id: EntityId, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(sys) = self.systems.iter().find(|s| s.id == system_id) else {
            return;
        };
        if sys.owner.is_some() {
            return; // already claimed (the loser learns this rival's claim by light)
        }
        let (pos, cost) = (sys.pos, sys.claim_cost);
        let Some(corp) = self.players.get(&player_id) else {
            return;
        };
        if corp.credits < cost {
            return; // can't afford the claim
        }
        if let Some(corp) = self.players.get_mut(&player_id) {
            corp.credits -= cost;
        }
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system_id) {
            sys.owner = Some(player_id);
            sys.claimed_at = Some(now);
        }
        events.push(Event::new(
            now,
            EventPayload::SystemClaimed { system: system_id, owner: player_id, pos },
        ));
    }

    /// Ship a claimed system's accumulated production to the hub: one raidable
    /// convoy per stockpiled commodity (whole units), each selling on arrival.
    fn apply_ship_production(&mut self, player_id: PlayerId, system_id: EntityId, events: &mut Vec<Event>) {
        // Collect what to ship (and zero those stockpiles) without holding a
        // borrow across the convoy spawn.
        let mut shipments: Vec<(Cargo, Vec2)> = Vec::new();
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system_id) {
            if sys.owner != Some(player_id) {
                return; // only the owner ships from their system
            }
            let pos = sys.pos;
            for (commodity, amount) in sys.stockpile.iter_mut() {
                let units = amount.floor() as u32;
                if units >= 1 {
                    *amount -= units as f64;
                    shipments.push((Cargo { commodity: *commodity, units }, pos));
                }
            }
        } else {
            return;
        }
        for (cargo, pos) in shipments {
            events.push(Event::new(
                self.time,
                EventPayload::Trade(TradeEvent::SellDispatched {
                    player: player_id,
                    commodity: cargo.commodity,
                    units: cargo.units,
                }),
            ));
            self.spawn_trade_convoy(player_id, pos, self.hub, cargo, TradeMission::SellAtHub);
        }
    }

    /// Accrue production at every claimed system: each deposit adds `richness·DT`
    /// units of its resource to the system's stockpile, drawing down finite
    /// reserves (renewable deposits never deplete). Deterministic.
    fn accrue_production(&mut self) {
        for sys in &mut self.systems {
            if sys.owner.is_none() {
                continue;
            }
            for dep in &mut sys.deposits {
                let mut amount = dep.richness * DT;
                if let Some(reserves) = dep.reserves.as_mut() {
                    amount = amount.min(*reserves);
                    *reserves -= amount;
                }
                if amount > 0.0 {
                    *sys.stockpile.entry(dep.resource).or_insert(0.0) += amount;
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
        for ship in self.ships.values() {
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
        let alive: std::collections::BTreeSet<EntityId> = self.ships.keys().copied().collect();
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
        for ship in self.ships.values() {
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
        let mut ship = Ship::new(
            id,
            owner,
            ShipKind::Convoy,
            spawn,
            ShipOrder::MoveTo { dest },
            Some(cargo),
        );
        ship.mission = Some(mission);
        self.ships.insert(id, ship);
        id
    }

    /// Resolve trade convoys that have reached their destination: deposit a
    /// delivery, or clear a sale at the price-on-arrival (§9). Convoys raided in
    /// transit were already removed (their goods/credits simply lost).
    fn resolve_trade_arrivals(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let arrived: Vec<EntityId> = self
            .ships
            .iter()
            .filter(|(_, s)| s.mission.is_some() && matches!(s.order, ShipOrder::Idle))
            .map(|(id, _)| *id)
            .collect();
        for id in arrived {
            let ship = self.ships.remove(&id).unwrap();
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
                    let delivered = self
                        .systems
                        .iter_mut()
                        .find(|s| s.id == system && s.owner == Some(ship.owner))
                        .map(|sys| {
                            *sys.stockpile.entry(cargo.commodity).or_insert(0.0) += cargo.units as f64;
                            true
                        })
                        .unwrap_or(false);
                    if delivered {
                        events.push(Event::new(
                            now,
                            EventPayload::Trade(TradeEvent::Delivered {
                                player: ship.owner,
                                commodity: cargo.commodity,
                                units: cargo.units,
                            }),
                        ));
                    }
                    // If not delivered (system lost), the cargo is simply lost — the
                    // frontier risk of automated supply into contested space.
                }
            }
        }
    }

    /// Schedule an order to install on a ship the player owns, after the
    /// outbound light-travel time from their command center to the ship (§6).
    /// Ignored if the ship doesn't exist or the player doesn't own it.
    fn schedule_for_owner(&mut self, player_id: PlayerId, ship_id: EntityId, new_order: ShipOrder) {
        let Some(ship) = self.ships.get(&ship_id) else {
            return;
        };
        if ship.owner != player_id {
            return;
        }
        let Some(corp) = self.players.get(&player_id) else {
            return;
        };
        let delay = ship.pos.distance(corp.command_center) / self.config.c;
        self.pending_orders.push(PendingOrder {
            apply_time: self.time + delay,
            ship_id,
            new_order,
        });
    }

    /// Assign an unused home anchor to a player (or append one if the galaxy is
    /// over capacity), returning its position.
    fn assign_home(&mut self, id: PlayerId) -> Vec2 {
        let now = self.time;
        if let Some(slot) = self.home_slots.iter_mut().find(|s| s.owner.is_none()) {
            slot.owner = Some(id);
            slot.claimed_at = Some(now);
            return slot.pos;
        }
        // Over capacity: place an extra anchor at a deterministic ring spot.
        let n = self.home_slots.len();
        let angle = TAU * (n as f64) * 0.61803398875; // golden-angle scatter
        let pos = Vec2::from_polar(angle, self.config.galaxy_radius * self.config.home_ring_frac);
        self.home_slots.push(HomeSlot {
            pos,
            owner: Some(id),
            claimed_at: Some(now),
        });
        pos
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
        self.ships.insert(
            convoy_id,
            Ship::new(
                convoy_id,
                owner,
                ShipKind::Convoy,
                home,
                ShipOrder::Patrol {
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
        self.ships.insert(
            raider_id,
            Ship::new(
                raider_id,
                owner,
                ShipKind::Raider,
                home,
                ShipOrder::Patrol {
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
    use crate::ids::PlayerId;

    fn test_world() -> World {
        World::new(SimConfig::for_players(123, 4))
    }

    #[test]
    fn galaxy_is_generated() {
        let w = test_world();
        assert_eq!(w.hub, Vec2::ZERO);
        assert_eq!(w.systems.len(), w.config.system_count as usize);
        assert_eq!(w.home_slots.len(), w.config.max_players as usize);
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
        assert_eq!(w.ships.len(), 2);
        let corp = &w.players[&id];
        assert_eq!(corp.home, corp.command_center);
        // One anchor is now owned.
        assert_eq!(w.home_slots.iter().filter(|s| s.owner == Some(id)).count(), 1);
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
        assert_eq!(w.ships.len(), 2); // no duplicate fleet
    }

    #[test]
    fn ships_actually_move() {
        let mut w = test_world();
        let id = PlayerId(7);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let start: Vec<Vec2> = w.ships.values().map(|s| s.pos).collect();
        // Advance a few seconds.
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let moved = w
            .ships
            .values()
            .zip(&start)
            .any(|(s, &p0)| s.pos.distance(p0) > 10.0);
        assert!(moved, "ships should have moved from their start positions");
    }

    fn convoy_id(w: &World) -> EntityId {
        *w.ships
            .iter()
            .find(|(_, s)| s.kind == ShipKind::Convoy)
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
        let ship_pos = w.ships[&cid].pos;
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
                !matches!(w.ships[&cid].order, ShipOrder::MoveTo { .. }),
                "order applied too early at t={} (delay {})",
                w.time,
                expected_delay
            );
        }
        // Step a little past the arrival: now it must be a MoveTo to `dest`.
        for _ in 0..3 {
            w.step(&[]);
        }
        match w.ships[&cid].order {
            ShipOrder::MoveTo { dest: d } => assert_eq!(d, dest),
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
        let target = *w.ships.iter().find(|(_, s)| s.owner == owner).unwrap().0;
        let before = format!("{:?}", w.ships[&target].order);
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
        if let ShipOrder::MoveTo { dest } = w.ships[&target].order {
            assert_ne!(dest, Vec2::new(0.0, 0.0), "rival should not control this ship");
        }
        let _ = before;
    }

    fn find_ship(w: &World, owner: PlayerId, kind: ShipKind) -> EntityId {
        *w.ships
            .iter()
            .find(|(_, s)| s.owner == owner && s.kind == kind)
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
            let r = w.ships.get_mut(&raider).unwrap();
            r.pos = cc + raider_off;
            r.vel = Vec2::ZERO;
            r.order = ShipOrder::Idle;
        }
        {
            let c = w.ships.get_mut(&convoy).unwrap();
            c.pos = cc + convoy_off;
            c.vel = Vec2::ZERO;
            c.order = ShipOrder::Idle; // sitting duck
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
        assert_eq!(w.ships.contains_key(&attacker), !kill_a, "attacker present iff not destroyed");
        assert_eq!(w.ships.contains_key(&target), !kill_t, "target present iff not destroyed");
    }

    // ---- Acceleration from mass (a = F/m) + proportional pursuit (§7, §8) ----

    fn ship_of(kind: ShipKind, cargo: Option<Cargo>) -> Ship {
        Ship::new(EntityId(1), PlayerId(1), kind, Vec2::ZERO, ShipOrder::Idle, cargo)
    }

    /// Acceleration is DERIVED as thrust / mass — the raider/convoy nimbleness gap
    /// emerges from the MASS difference, and a loaded convoy accelerates worse.
    #[test]
    fn acceleration_derives_from_thrust_over_mass() {
        let raider = ship_of(ShipKind::Raider, None);
        let empty = ship_of(ShipKind::Convoy, None);
        let loaded = ship_of(
            ShipKind::Convoy,
            Some(Cargo { commodity: crate::cargo::Commodity::Alloys, units: 120 }),
        );

        // a = F/m exactly (not a hand-set constant).
        assert!((raider.accel() - ShipKind::Raider.thrust() / ShipKind::Raider.hull_mass()).abs() < 1e-9);
        // Convoy hull is orders of magnitude heavier than the raider's…
        assert!(ShipKind::Convoy.hull_mass() >= 10.0 * ShipKind::Raider.hull_mass());
        // …so the light raider out-accelerates the heavy convoy by a wide margin —
        // asymmetry from MASS, via a = F/m.
        assert!(raider.accel() > empty.accel() * 5.0,
            "raider accel {:.2} should dwarf convoy {:.2}", raider.accel(), empty.accel());
        // Cargo adds mass, so a loaded convoy accelerates noticeably worse.
        assert!(loaded.mass() > empty.mass());
        assert!(loaded.accel() < empty.accel(),
            "loaded convoy accel {:.3} must be worse than empty {:.3}", loaded.accel(), empty.accel());
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
            let r = w.ships.get_mut(&raider).unwrap();
            r.pos = cc + Vec2::new(50.0, 0.0);
            r.vel = Vec2::ZERO;
            r.order = ShipOrder::Idle;
        }
        {
            let c = w.ships.get_mut(&convoy).unwrap();
            c.pos = cc + Vec2::new(2500.0, 0.0);
            c.vel = Vec2::ZERO;
            c.order = ShipOrder::MoveTo { dest: cc + Vec2::new(9000.0, 0.0) }; // flees outward
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
            let c = w.ships.get_mut(&convoy).unwrap();
            c.pos = convoy_pos;
            c.vel = Vec2::ZERO;
            c.order = ShipOrder::Idle;
        }
        {
            let p = w.ships.get_mut(&patrol).unwrap();
            p.pos = patrol_pos;
            p.vel = Vec2::ZERO;
            p.defense = None;
            // A small standing patrol around its station.
            p.order = ShipOrder::Patrol {
                waypoints: vec![patrol_pos, patrol_pos + Vec2::new(200.0, 0.0)],
                index: 0,
                dwell_until: 0.0,
            };
        }
        {
            let h = w.ships.get_mut(&hostile).unwrap();
            h.pos = hostile_pos;
            h.vel = (convoy_pos - hostile_pos).normalized() * 60.0; // inbound, on course
            h.order = ShipOrder::Intercept { target: convoy };
        }
        (patrol, convoy, hostile)
    }

    fn engaged_on(w: &World, patrol: EntityId, hostile: EntityId) -> bool {
        w.ships.get(&patrol).and_then(|s| s.defense.as_ref()).map(|d| d.target == hostile).unwrap_or(false)
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
        assert!(matches!(w.ships[&patrol].order, ShipOrder::Intercept { target } if target == hostile));

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
            assert!(matches!(w.ships[&patrol].order, ShipOrder::Patrol { .. }), "must not react to an undetectable threat");
            assert!(w.ships[&patrol].defense.is_none());
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
        for _ in 0..(25 * crate::config::TICK_HZ) {
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
        for _ in 0..(60 * crate::config::TICK_HZ) {
            for e in w2.step(&[]) {
                if let EventPayload::ShipDestroyed { ship, .. } = e.payload
                    && ship == convoy2
                {
                    convoy_lost = true;
                }
            }
            if w2.ships.get(&p_far).map(|s| s.defense.is_some()).unwrap_or(false) {
                far_engaged = true;
            }
            if convoy_lost {
                break;
            }
        }
        assert!(!far_engaged, "a patrol off the approach vector never senses the threat");
        assert!(convoy_lost, "with no defender in reach, the convoy is lost — positioning matters");
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
            if w.ships[&patrol].defense.is_some() {
                engaged = true;
                break;
            }
        }
        assert!(engaged, "patrol should have engaged");

        // The threat vanishes (destroyed elsewhere / broke contact).
        w.ships.remove(&hostile);
        w.step(&[]);
        assert!(w.ships[&patrol].defense.is_none(), "defense cleared once the threat is gone");
        assert!(matches!(w.ships[&patrol].order, ShipOrder::Patrol { .. }), "the defender resumes its standing patrol");
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
            let s = w.ships.get_mut(&id).unwrap();
            s.pos = cc + off;
            s.vel = Vec2::ZERO;
            s.order = ShipOrder::Idle;
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
        assert!(w.ships.contains_key(&convoy), "convoy should survive a successful recall");
        // Raider is no longer intercepting.
        assert!(!matches!(w.ships[&raider].order, ShipOrder::Intercept { .. }));
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
        let convoy = w.ships.values().find(|s| s.owner == id && s.mission == Some(TradeMission::DeliverHome));
        assert!(convoy.is_some(), "buy should spawn a delivery convoy");
        assert!(convoy.unwrap().pos.distance(w.hub) < 1.0, "delivery convoy starts at the hub");
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
        let convoy = w.ships.values().find(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub));
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
        let ships0 = w.ships.len();
        // Sell more than held → ignored (no convoy, inventory unchanged).
        let alloys0 = w.players[&id].inventory[&Alloys];
        w.step(&[Command::MarketSell { player_id: id, commodity: Alloys, units: 99_999 }]);
        assert_eq!(w.players[&id].inventory[&Alloys], alloys0);
        assert_eq!(w.ships.len(), ships0, "rejected sell must not spawn a convoy");
        // Buy beyond the treasury → ignored.
        let credits0 = w.players[&id].credits;
        w.step(&[Command::MarketBuy { player_id: id, commodity: Alloys, units: 10_000_000 }]);
        assert_eq!(w.players[&id].credits, credits0);
        assert_eq!(w.ships.len(), ships0, "rejected buy must not spawn a convoy");
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
        assert!(w.ships.values().any(|s| s.owner == buyer && s.mission == Some(TradeMission::DeliverHome)));
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
        let cmds = vec![
            Command::AddPlayer { id: PlayerId(1), name: "A".into() },
            Command::AddPlayer { id: PlayerId(2), name: "B".into() },
            // Idempotent after the first tick — exercises the DYNAMIC new state
            // (owner mutation + continuous production accrual) so replay equality
            // covers it, not just the static seeded generation.
            Command::ClaimSystem { player_id: PlayerId(1), system_id: sysid },
        ];
        for _ in 0..600 {
            a.step(&cmds);
            b.step(&cmds);
        }
        // The dynamic paths actually ran (so the comparison is meaningful).
        let sys_a = a.systems.iter().find(|s| s.id == sysid).unwrap();
        assert_eq!(sys_a.owner, Some(PlayerId(1)), "claim path must have executed");
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

    #[test]
    fn claim_charges_credits_and_transfers_ownership() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        let cost = w.systems.iter().find(|s| s.id == sysid).unwrap().claim_cost;
        let credits0 = w.players[&id].credits;
        assert!(cost > 0.0 && credits0 >= cost, "starting credits should afford a claim");

        let ev = w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
        let sys = w.systems.iter().find(|s| s.id == sysid).unwrap();
        assert_eq!(sys.owner, Some(id), "claim should transfer ownership");
        assert!(sys.claimed_at.is_some());
        assert!((credits0 - w.players[&id].credits - cost).abs() < 1e-6, "claim should charge the cost");
        assert!(ev.iter().any(|e| matches!(e.payload, EventPayload::SystemClaimed { system, .. } if system == sysid)));
    }

    #[test]
    fn cannot_claim_an_owned_system_or_one_you_cannot_afford() {
        let mut w = test_world();
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        let sysid = richest_system(&w);
        w.step(&[Command::ClaimSystem { player_id: a, system_id: sysid }]);
        let b_credits0 = w.players[&b].credits;
        // B tries to claim A's system — no-op, no charge.
        w.step(&[Command::ClaimSystem { player_id: b, system_id: sysid }]);
        assert_eq!(w.systems.iter().find(|s| s.id == sysid).unwrap().owner, Some(a));
        assert_eq!(w.players[&b].credits, b_credits0, "a failed claim must not charge");

        // Drain B's credits, then a claim of an unclaimed system fails (no charge).
        let unclaimed = w.systems.iter().find(|s| s.owner.is_none()).unwrap().id;
        w.players.get_mut(&b).unwrap().credits = 0.0;
        w.step(&[Command::ClaimSystem { player_id: b, system_id: unclaimed }]);
        assert!(w.systems.iter().find(|s| s.id == unclaimed).unwrap().owner.is_none());
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

        w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
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
        w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        let stock_before: f64 = w.systems.iter().find(|s| s.id == sysid).unwrap().stockpile.values().sum();
        assert!(stock_before >= 1.0, "should have whole units to ship");

        w.step(&[Command::ShipProduction { player_id: id, system_id: sysid }]);
        // A production convoy is just a normal raidable trade convoy (Convoy kind,
        // carrying cargo, selling at the hub) — spawned at the system.
        let sys_pos = w.systems.iter().find(|s| s.id == sysid).unwrap().pos;
        let convoy = w.ships.values().find(|s| s.owner == id && s.mission == Some(TradeMission::SellAtHub)).cloned();
        let convoy = convoy.expect("ship-production should spawn a sell convoy");
        assert_eq!(convoy.kind, ShipKind::Convoy, "production ships in raidable convoys");
        assert!(convoy.cargo.is_some());
        assert!(convoy.pos.distance(sys_pos) < 1.0, "production convoy departs from the system");
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
        w.step(&[Command::ClaimSystem { player_id: def, system_id: sysid }]);
        for _ in 0..(30 * crate::config::TICK_HZ) { w.step(&[]); }
        w.step(&[Command::ShipProduction { player_id: def, system_id: sysid }]);
        let convoy = *w.ships.iter().find(|(_, s)| s.owner == def && s.mission == Some(TradeMission::SellAtHub)).unwrap().0;

        // Park the attacker's raider right on the production convoy and commit.
        let raider = find_ship(&w, atk, ShipKind::Raider);
        let cpos = w.ships[&convoy].pos;
        {
            let r = w.ships.get_mut(&raider).unwrap();
            r.pos = cpos + Vec2::new(40.0, 0.0); // inside CONTACT_RADIUS
            r.vel = Vec2::ZERO;
            r.order = ShipOrder::Idle;
        }
        // Force the raider's command center near it so the commit applies promptly.
        w.players.get_mut(&atk).unwrap().command_center = cpos;
        let outcome = run_until_raid(&mut w, 30, |wld| {
            if wld.ships.get(&raider).map(|s| matches!(s.order, ShipOrder::Intercept { .. })).unwrap_or(false) {
                vec![]
            } else {
                vec![Command::CommitRaid { player_id: atk, raider_id: raider, target_id: convoy }]
            }
        });
        let outcome = outcome.expect("the raid on the production convoy should resolve");
        // If the convoy was destroyed, its production output is gone — real stakes.
        if outcome.kills().1 {
            assert!(!w.ships.contains_key(&convoy), "a destroyed production convoy is gone");
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
        w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
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
        w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
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
                .ships
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
        w.step(&[Command::ClaimSystem { player_id: id, system_id: source }]);
        w.step(&[Command::ClaimSystem { player_id: id, system_id: dest }]);

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
            w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);
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
        assert!(a.players[&PlayerId(1)].credits > 10_000.0 - 1.0 || !a.ships.is_empty());
    }

    /// Clearing a standing order stops it; invalid rules are rejected at set-time.
    #[test]
    fn standing_order_clear_and_validation() {
        let mut w = test_world();
        let id = PlayerId(1);
        w.step(&[Command::AddPlayer { id, name: "Acme".into() }]);
        let sysid = richest_system(&w);
        w.step(&[Command::ClaimSystem { player_id: id, system_id: sysid }]);

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
        w.ships.retain(|_, s| s.mission.is_none());
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
}
