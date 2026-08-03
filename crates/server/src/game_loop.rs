//! The authoritative game loop — the heartbeat of the server (§14).
//!
//! A single Tokio task owns the [`World`] and the [`Sessions`] registry. Because
//! nothing else can touch them, there are no locks and no data races on game
//! state. The loop:
//!   1. ticks at a fixed [`TICK_HZ`] rate, advancing the world via the pure core;
//!   2. folds player intents / session events into sim commands at tick
//!      boundaries;
//!   3. pushes every connection its own per-player message (M1: the live tick;
//!      from M3: the delayed/fogged view);
//!   4. hands events and periodic snapshots to the off-hot-path persistence task.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, info};

use sim::{Command, DT, PlayerId, TICK_HZ, World};

use crate::persistence::{PersistJob, PersistenceHandle, to_json};
use crate::protocol::{
    BuildOptionView, ClientMsg, GalaxyInfo, InvSlot, MarketView, OrderView, PriceView, ServerMsg,
    StockSlot, SystemInfo, WalletView,
};
use crate::reports::ReportScheduler;
use crate::session::{ConnId, ConnInfo, GameInput, ServerStatus, Sessions};
use crate::timeline::Timeline;
use crate::view::{self, PositionHistory, PriceHistory};

/// Push a per-player message every N sim ticks. At 30 Hz, N=3 → ~10 Hz network
/// updates: visibly live without flooding the socket.
const BROADCAST_EVERY: u64 = 3;

/// Default full-world snapshot cadence: every 10 s at the tick rate. Bounds how
/// much progress a restart can lose (the snapshot is the restart basis, §14).
pub const DEFAULT_SNAPSHOT_EVERY: u64 = 10 * TICK_HZ as u64;

/// Build the straight warp-light clock for an outbound command aimed at the
/// moving meeting point projected from the player's current ghost of a ship.
/// The purple comet and the quoted arrival share this one solve.
#[derive(Debug, Clone)]
struct CommandSignalPlan {
    travel_time: f64,
    meeting_point: sim::Vec2,
    hops: Vec<crate::protocol::SignalHopView>,
}

fn command_signal_plan(
    c: f64,
    cc: sim::Vec2,
    ghost_pos: sim::Vec2,
    ghost_vel: sim::Vec2,
) -> CommandSignalPlan {
    // §6: solve only from the served sighting. Authoritative delivery uses the
    // same equation against truth; the animation never reads the hidden hull.
    let (travel_time, meeting_point) =
        sim::transit::command_meeting_delay(cc, ghost_pos, ghost_vel, c, 1.0);
    CommandSignalPlan {
        travel_time,
        meeting_point,
        hops: Vec::new(), // the client fallback draws the single straight leg
    }
}

#[derive(Debug, Clone)]
struct ObservedOrderPlan {
    arrives_at: f64,
    response_at: f64,
    intent_path: Vec<sim::Vec2>,
}

fn observed_order_plan(
    c: f64,
    cc: sim::Vec2,
    ghost_pos: sim::Vec2,
    ghost_vel: sim::Vec2,
    depart_time: f64,
    _response_course: Option<(sim::Vec2, f64)>,
) -> (ObservedOrderPlan, CommandSignalPlan) {
    let signal = command_signal_plan(c, cc, ghost_pos, ghost_vel);
    let arrives_at = depart_time + signal.travel_time;
    let response_at = arrives_at + sim::transit::delay(signal.meeting_point, cc, c);
    (
        ObservedOrderPlan {
            arrives_at,
            response_at,
            intent_path: Vec::new(),
        },
        signal,
    )
}

/// Plot the intended route from the last SERVED sighting to a fixed destination.
/// This is plan geometry only: it carries no timestamps and never claims the
/// observed marker has advanced along it. Authoritative position never enters.
fn intended_route(ghost_pos: sim::Vec2, dest: sim::Vec2, _transit_speed: f64) -> Vec<sim::Vec2> {
    vec![ghost_pos, dest]
}

fn has_fixed_intent_path(kind: sim::event::OrderKind, target: Option<sim::EntityId>) -> bool {
    target.is_none()
        && matches!(
            kind,
            sim::event::OrderKind::Move
                | sim::event::OrderKind::Recall
                | sim::event::OrderKind::Withdraw
        )
}

fn has_fixed_response_course(kind: sim::event::OrderKind) -> bool {
    matches!(
        kind,
        sim::event::OrderKind::Move
            | sim::event::OrderKind::Recall
            | sim::event::OrderKind::Withdraw
            | sim::event::OrderKind::Construct
    )
}

fn disclosed_order_loss(
    loss: Option<sim::world::PendingCommandLossView>,
    now: f64,
) -> Option<sim::world::PendingCommandLossView> {
    loss.filter(|loss| now >= loss.news_at)
}

fn pending_order_views(
    world: &World,
    viewer: PlayerId,
    observed_plans: &HashMap<(PlayerId, u64), ObservedOrderPlan>,
    now: f64,
) -> Vec<crate::protocol::PendingOrderView> {
    world
        .pending_commands(viewer)
        .into_iter()
        .filter_map(|pending| {
            let observed = observed_plans.get(&(viewer, pending.id))?;
            let disclosed_loss = disclosed_order_loss(pending.loss, now);
            Some(crate::protocol::PendingOrderView {
                id: pending.id,
                fleet_id: pending.fleet,
                issued_at: pending.issued_at,
                arrives_at: observed.arrives_at,
                response_at: observed.response_at,
                kind: pending.kind,
                dest: pending.dest,
                target_id: pending.target,
                emplacement: pending.emplacement,
                intent_path: observed.intent_path.clone(),
                lost: disclosed_loss.is_some(),
                loss_relay: disclosed_loss.map(|loss| loss.relay),
                loss_break: disclosed_loss.map(|loss| loss.break_pos),
            })
        })
        .collect()
}

/// A battle whose engagement has CONCLUDED in true space but whose conclusion
/// light hasn't yet reached every viewer (§battles-take-time). The sim removes
/// the engagement the instant it ends, so `active_battles()` drops it at the
/// TRUE end-time; but the aftermath report only lands `distance/c` later, when
/// the conclusion's light arrives. Without bridging that gap the "battle in
/// progress" icon vanishes FTL and, for the `distance/c` seconds until the
/// aftermath, the participant fleet ghosts (which the icon had been suppressing)
/// briefly re-appear at the site. We retain the concluded battle here so each
/// viewer keeps seeing the in-progress icon — and its participants suppressed —
/// until `ended_at + distance/c`, the exact instant the aftermath lands: a clean
/// in-progress → aftermath handoff with no re-appearing fleets.
/// §emplacements: a structure that is GONE from true space but whose loss has
/// not yet been seen by everyone who could see it standing. Retained exactly
/// like [`ConcludedBattle`]: the wreck is served to a viewer until the news
/// reaches their command center, so nobody learns of a demolition faster than
/// light. Pruned once even the farthest possible viewer must have heard.
struct GoneEmplacement {
    id: sim::EntityId,
    owner: sim::PlayerId,
    kind: sim::EmplacementKind,
    pos: sim::Vec2,
    /// Sim time it came down.
    at: f64,
}

struct ConcludedBattle {
    id: sim::EntityId,
    pos: sim::Vec2,
    started_at: f64,
    /// Sim-time the battle ended = the `RaidResolved` event time, so the icon's
    /// disappearance rides the SAME light wavefront as the aftermath report.
    ended_at: f64,
    a_owner: PlayerId,
    d_owner: PlayerId,
    participants: Vec<sim::EntityId>,
}

/// The Market Hub account picture carried by light to one corporation. Only
/// fields whose source is the hub live here; system Fuel remains a separate
/// owner-local summary in `WalletView`.
#[derive(Debug, Clone, PartialEq)]
struct MarketAccountSample {
    credits: f64,
    valuation: f64,
    warehouse: BTreeMap<sim::Commodity, u32>,
    orders: Vec<sim::market::LimitOrder>,
}

/// Change-compressed hub-account history. Recording only when truth changes
/// keeps the cost proportional to economic events rather than tick count, while
/// retaining one sample before the light horizon makes every delayed lookup
/// total across quiet periods.
struct MarketAccountHistory {
    samples: HashMap<PlayerId, VecDeque<(f64, MarketAccountSample)>>,
    horizon: f64,
}

impl MarketAccountHistory {
    fn for_world(world: &World) -> Self {
        let max_delay = sim::transit::delay(
            sim::Vec2::ZERO,
            sim::Vec2::new(2.0 * world.config.galaxy_radius, 0.0),
            world.config.c,
        );
        Self {
            samples: HashMap::new(),
            horizon: max_delay * 1.25 + 1.0,
        }
    }

    fn record(&mut self, world: &World) {
        let now = world.time;
        for (&player, corp) in &world.players {
            let sample = MarketAccountSample {
                credits: corp.credits,
                valuation: corp.valuation,
                warehouse: corp.warehouse.clone(),
                orders: world
                    .book
                    .iter()
                    .filter(|o| o.player == player)
                    .cloned()
                    .collect(),
            };
            let history = self.samples.entry(player).or_default();
            if history
                .back()
                .is_none_or(|(_, previous)| previous != &sample)
            {
                history.push_back((now, sample));
            }
            let cutoff = now - self.horizon;
            while history.len() > 1 && history.get(1).is_some_and(|(t, _)| *t <= cutoff) {
                history.pop_front();
            }
        }
    }

    fn at(&self, player: PlayerId, target: f64) -> Option<&MarketAccountSample> {
        let history = self.samples.get(&player)?;
        history
            .iter()
            .rev()
            .find(|(time, _)| *time <= target)
            .or_else(|| history.front())
            .map(|(_, sample)| sample)
    }
}

struct ScheduledTradeReport {
    arrives_at: f64,
    trade: sim::TradeEvent,
}

fn system_pos(world: &World, id: sim::EntityId) -> Option<sim::Vec2> {
    world
        .systems
        .iter()
        .find(|system| system.id == id)
        .map(|system| system.pos)
}

/// Where the fact represented by an economy receipt physically occurred. Most
/// Exchange administration originates at the Market Hub; local loading,
/// delivery and automation originates at the named system. This position is the
/// one source for both live receipt delay and the retained check-in timeline.
pub(crate) fn trade_report_origin(world: &World, trade: &sim::TradeEvent) -> sim::Vec2 {
    match *trade {
        sim::TradeEvent::Delivered {
            system: Some(id), ..
        }
        | sim::TradeEvent::SupplyDiverted { system: id, .. }
        | sim::TradeEvent::StorageOverflow { system: id, .. }
        | sim::TradeEvent::AutoDispatched { source: id, .. }
        | sim::TradeEvent::Loaded {
            system: Some(id), ..
        }
        | sim::TradeEvent::Unloaded {
            system: Some(id), ..
        } => system_pos(world, id).unwrap_or(world.hub),
        sim::TradeEvent::FreightMoved {
            system,
            stage: sim::FreightStage::CollectedForPickup | sim::FreightStage::DeliveredToSystem,
            ..
        } => system_pos(world, system).unwrap_or(world.hub),
        _ => world.hub,
    }
}

impl ConcludedBattle {
    /// Should a command center at `cc` still see this battle's IN-PROGRESS icon at
    /// wall-time `now`? True on the half-open window `[started_at + delay,
    /// ended_at + delay)` where `delay = |pos − cc| / warp_light`:
    ///
    /// * the lower bound is the same light-gate the live icon used (never show a
    ///   battle whose start-light hasn't arrived), and
    /// * the upper bound is the conclusion's light-arrival — the exact instant the
    ///   aftermath report lands (`event_time + delay`), so the in-progress icon
    ///   flips to aftermath on one wavefront with neither gap nor overlap.
    fn shows_in_progress(&self, cc: sim::Vec2, c: f64, now: f64) -> bool {
        let delay = sim::transit::delay(self.pos, cc, c);
        now >= self.started_at + delay && now < self.ended_at + delay
    }
}

struct GameLoop {
    world: World,
    sessions: Sessions,
    /// Per-player lightspeed view filter — keeps position history and builds
    /// each player's delayed/fogged view (§14).
    history: PositionHistory,
    /// Lagged hub-price ticker history (§9) — each player reads prices delayed
    /// by their light-distance from the hub.
    prices: PriceHistory,
    /// Market Warehouse balances and resting orders as actually known after the
    /// Market Hub's report reaches each command center.
    market_accounts: MarketAccountHistory,
    /// Owner-facing economy receipts travel from the physical event site rather
    /// than being pushed straight from simulation truth.
    trade_reports: Vec<ScheduledTradeReport>,
    /// Delayed delivery of discrete reports (raid outcomes) — each player learns
    /// them on their own clock (§8).
    reports: ReportScheduler,
    /// Per-player retained check-in timeline (§16, Layer 3) — what became
    /// observable, buffered across disconnects, for the "welcome back" digest.
    timeline: Timeline,
    /// Last timeline length pushed to each player, so we only re-send when it grows.
    timeline_sent: HashMap<PlayerId, usize>,
    /// Battles that have concluded but whose conclusion light is still in flight
    /// to some viewer — kept so the in-progress icon lingers until the aftermath
    /// lands (see [`ConcludedBattle`]). Ephemeral awareness state, like `reports`.
    concluded_battles: Vec<ConcludedBattle>,
    /// §emplacements: demolished structures still visible on old light.
    gone_emplacements: Vec<GoneEmplacement>,
    /// Commands accumulated since the last tick, applied at the next boundary.
    pending: Vec<Command>,
    /// Owner-only order clocks solved once from the served ghost at issue time.
    /// The sim's authoritative delivery/echo stamps never enter a player View.
    observed_order_plans: HashMap<(PlayerId, u64), ObservedOrderPlan>,
    persistence: PersistenceHandle,
    /// Take a snapshot every this many ticks.
    snapshot_every: u64,
    /// Publishes server/ops status for the `/status` endpoint (meta channel).
    status_tx: watch::Sender<ServerStatus>,
    /// Connections with an engagement-estimate rollout currently running on a
    /// blocking thread. One in flight per connection — repeat clicks are dropped
    /// rather than piling up blocking tasks. Cleared when the task reports done
    /// via `estimate_done_tx`.
    estimate_inflight: HashSet<ConnId>,
    /// A completed rollout signals its connection id here so the loop can clear
    /// the in-flight flag (the estimate itself is sent to the client directly
    /// from the blocking task, never routed back through the loop).
    estimate_done_tx: mpsc::UnboundedSender<ConnId>,
}

impl GameLoop {
    fn new(
        world: World,
        persistence: PersistenceHandle,
        snapshot_every: u64,
        status_tx: watch::Sender<ServerStatus>,
        estimate_done_tx: mpsc::UnboundedSender<ConnId>,
    ) -> Self {
        let history = PositionHistory::for_world(&world);
        let prices = PriceHistory::for_world(&world);
        let mut market_accounts = MarketAccountHistory::for_world(&world);
        market_accounts.record(&world);
        GameLoop {
            world,
            sessions: Sessions::new(),
            history,
            prices,
            market_accounts,
            trade_reports: Vec::new(),
            reports: ReportScheduler::new(),
            timeline: Timeline::new(),
            timeline_sent: HashMap::new(),
            concluded_battles: Vec::new(),
            gone_emplacements: Vec::new(),
            pending: Vec::new(),
            observed_order_plans: HashMap::new(),
            persistence,
            snapshot_every: snapshot_every.max(1),
            status_tx,
            estimate_inflight: HashSet::new(),
            estimate_done_tx,
        }
    }

    /// Send the issuing player the outbound command-signal feedback for an order
    /// to one of THEIR ships. The comet's duration is the player's OBSERVED
    /// staleness of that ship (its ghost age), so it meets the ghost and reveals
    /// no true distance. Skipped if the player doesn't own the ship or it's
    /// currently dark to them.
    fn emit_command_signal(
        &mut self,
        player_id: PlayerId,
        ship_id: sim::EntityId,
        order_id: u64,
        depart_time: f64,
    ) {
        let Some(corp) = self.world.players.get(&player_id) else {
            return;
        };
        let owns = self
            .world
            .fleets
            .get(&ship_id)
            .map(|s| s.owner == player_id)
            .unwrap_or(false);
        if !owns {
            return;
        }
        let cc = corp.command_center;
        let c = self.world.config.c;
        // Aim from the player's observed ship position and velocity, never its
        // hidden true state. A just-spawned hull at home degenerates to a
        // zero-length signal.
        let sighting = self
            .history
            .observed_sighting(ship_id, cc, c, depart_time)
            .unwrap_or(view::ObservedSighting {
                pos: cc,
                vel: sim::Vec2::ZERO,
            });
        let subject = self
            .world
            .pending_commands(player_id)
            .into_iter()
            .find(|pending| pending.id == order_id);
        let transit_speed = self
            .world
            .fleets
            .get(&ship_id)
            .map(|fleet| fleet.transit_speed());
        let response_course = subject
            .as_ref()
            .filter(|pending| has_fixed_response_course(pending.kind))
            .and_then(|pending| pending.dest)
            .zip(transit_speed);
        let (mut observed, signal) = observed_order_plan(
            c,
            cc,
            sighting.pos,
            sighting.vel,
            depart_time,
            response_course,
        );
        if let Some(subject) = subject
            && has_fixed_intent_path(subject.kind, subject.target)
            && let Some(dest) = subject.dest
            && let Some(transit_speed) = transit_speed
        {
            observed.intent_path = intended_route(sighting.pos, dest, transit_speed);
        }
        self.observed_order_plans
            .insert((player_id, order_id), observed.clone());
        self.sessions.send_to_player(
            player_id,
            ServerMsg::CommandSignal {
                order_id,
                ship_id,
                depart_time,
                arrive_time: observed.arrives_at,
                hops: signal.hops,
            },
        );
    }

    /// Publish current session/ops status (cheap; replaces the watched value).
    fn publish_status(&self) {
        let _ = self.status_tx.send(ServerStatus {
            online_players: self.sessions.online_player_count(),
            connections: self.sessions.connection_count(),
            tick: self.world.tick,
            sim_time: self.world.time,
        });
    }

    fn schedule_trade_reports(&mut self, events: &[sim::Event]) {
        for event in events {
            let sim::EventPayload::Trade(trade) = &event.payload else {
                continue;
            };
            let Some(corp) = self.world.players.get(&trade.player()) else {
                continue;
            };
            let origin = trade_report_origin(&self.world, trade);
            let delay = sim::transit::delay(origin, corp.command_center, self.world.config.c);
            self.trade_reports.push(ScheduledTradeReport {
                arrives_at: event.time + delay,
                trade: *trade,
            });
        }
    }

    fn deliver_trade_reports(&mut self) {
        let now = self.world.time;
        let mut waiting = Vec::with_capacity(self.trade_reports.len());
        for report in self.trade_reports.drain(..) {
            if report.arrives_at <= now + 1e-9 {
                self.sessions.send_to_player(
                    report.trade.player(),
                    ServerMsg::Trade {
                        trade: report.trade,
                    },
                );
            } else {
                waiting.push(report);
            }
        }
        self.trade_reports = waiting;
    }

    fn handle_input(&mut self, input: GameInput) {
        match input {
            GameInput::Connect {
                conn_id,
                player_id,
                name,
                outbound,
                view_tx,
            } => {
                let newly_online = self.sessions.insert(
                    conn_id,
                    ConnInfo {
                        player_id,
                        name: name.clone(),
                        outbound,
                        view_tx,
                        // Fresh delivery cursors: this connection's first broadcast
                        // sends full state (records, sections) — reconnect-safe.
                        sent: Default::default(),
                    },
                );
                // Greet this connection immediately with its identity, clock,
                // and the static galaxy geography.
                self.sessions.send_to_conn(
                    conn_id,
                    ServerMsg::Welcome {
                        player_id,
                        name: name.clone(),
                        protocol_version: crate::protocol::PROTOCOL_VERSION,
                        tick_hz: TICK_HZ,
                        tick: self.world.tick,
                        sim_time: self.world.time,
                        galaxy: GalaxyInfo {
                            hub: self.world.hub,
                            radius: self.world.config.galaxy_radius,
                            c: self.world.config.c,
                            jump_range: sim::transit::JUMP_RANGE,
                            jump_spool_s: sim::transit::JUMP_SPOOL_S,
                            hyperlimit: sim::transit::HYPERLIMIT,
                            sensor_range: self.world.config.sensor_range,
                            raider_speed: sim::ShipKind::Raider.max_speed(),
                            // Array-bubble tunables so the client renders its own
                            // arrays' coverage (§buildings step 2b).
                            // Scout bubble multiplier, for the client's coverage draw.
                            scout_sensor_mult: sim::ship::SCOUT_SENSOR_MULT,
                            sensor_array_base: sim::build::SENSOR_ARRAY_BASE,
                            sensor_array_per_tier: sim::build::SENSOR_ARRAY_PER_TIER,
                            // Platform protection radius, for the owner's own
                            // defended-system ring (§buildings step 2c).
                            defense_platform_radius: sim::build::DEFENSE_PLATFORM_RADIUS,
                            // §economy Part 2 colony tunables, for the owner-only
                            // population/food readout.
                            provisions_per_million_per_s: sim::colony::PROVISIONS_PER_MILLION_PER_S,
                            pop_cap_per_habitat_tier: sim::colony::POP_CAP_PER_HABITAT_TIER,
                            pop_growth_per_s: sim::colony::POP_GROWTH_PER_S,
                            specialist_hire_cost: sim::specialist::SPECIALIST_HIRE_COST,
                            // §economy Part 3: the refinery hint rate (full converter table on the wire in Part 6).
                            fuel_refinery_rate: sim::production::converter_for(
                                sim::StructureKind::FuelRefinery,
                            )
                            .expect("refinery converts")
                            .rate,
                            // §contestable-territory Part 2: the siege duration.
                            siege_secs: self.world.siege_duration_secs(),
                            pirate_id: sim::PlayerId::PIRATE,
                            // §node: the awakening countdown + region radius so the
                            // client can telegraph and draw the holder's region ring.
                            node_awakening_time: self.world.config.node_awakening_time,
                            node_region_radius: sim::NODE_REGION_RADIUS,
                            // Static geography + geology (deposits, claim cost).
                            // Dynamic ownership/stockpile comes light-gated in View.
                            // §explore: PUBLIC geography only — the exact deposits
                            // are corp knowledge now (SystemStateView.deposits,
                            // surveyed-or-owner); the free spectral read is the BAND.
                            systems: system_infos(&self.world),
                            // What can be built + each recipe's cost/time (§step1).
                            build_options: build_options(),
                        },
                        // §perf Part B: the two static tables that used to ride
                        // every 10 Hz View — sent once here instead.
                        charter_ladder: sim::tca::status_ladder().to_vec(),
                        research_catalog: research_catalog(),
                    },
                );
                // Welcome-back: the check-in digest of what became observable while
                // away (§16, Layer 3). `away_since` is their last-online time, so the
                // client can mark entries newer than it as "while you were away".
                let (entries, away_since) = self.timeline.digest(player_id);
                self.timeline_sent.insert(player_id, entries.len());
                self.sessions.send_to_conn(
                    conn_id,
                    ServerMsg::Timeline {
                        entries,
                        away_since,
                    },
                );

                // Ensure the corporation exists in the sim (idempotent).
                self.pending.push(Command::AddPlayer {
                    id: player_id,
                    name,
                });
                info!(
                    %player_id, conn_id, newly_online,
                    online_players = self.sessions.online_player_count(),
                    connections = self.sessions.connection_count(),
                    "player connected"
                );
            }
            GameInput::Disconnect { conn_id } => {
                if let Some((player_id, now_offline)) = self.sessions.remove(conn_id) {
                    info!(
                        %player_id, conn_id, now_offline,
                        online_players = self.sessions.online_player_count(),
                        "player disconnected"
                    );
                }
            }
            GameInput::Intent { conn_id, msg } => match msg {
                ClientMsg::Ping => {
                    debug!(conn_id, "ping");
                }
                ClientMsg::MoveShip { ship_id, dest } => {
                    // Attach the issuing player (the sim enforces ownership).
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MoveShip {
                            player_id,
                            ship_id,
                            dest,
                        });
                    }
                }
                ClientMsg::JumpShip { ship_id, dest } => {
                    // As with ordinary movement, the player supplies intent and
                    // the sim judges the true fleet when its signal arrives.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::JumpShip {
                            player_id,
                            ship_id,
                            dest,
                        });
                    }
                }
                ClientMsg::DemolishEmplacement { fleet, target } => {
                    // §emplacements: same shape as the build order — the signal
                    // travels to the FLEET; the sim validates and runs the clock.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DemolishEmplacement {
                            player_id,
                            fleet,
                            target,
                        });
                    }
                }
                ClientMsg::BuildEmplacement {
                    builder,
                    emplacement,
                } => {
                    // §emplacements: same shape as MoveShip — the order signal
                    // travels to the BUILDER, which builds where it is parked;
                    // the sim sites, charges, refuses.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuildEmplacement {
                            player_id,
                            builder,
                            emplacement,
                        });
                    }
                }
                ClientMsg::CommitRaid {
                    raider_id,
                    target_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::CommitRaid {
                            player_id,
                            raider_id,
                            target_id,
                        });
                    }
                }
                ClientMsg::BlockadeSystem {
                    fleet_id,
                    system_id,
                } => {
                    // §contestable-territory Part 1: light-delayed like a move.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BlockadeSystem {
                            player_id,
                            fleet_id,
                            system_id,
                        });
                    }
                }
                ClientMsg::SurveySystem {
                    fleet_id,
                    system_id,
                } => {
                    // §explore Part 2: light-delayed like a move.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SurveySystem {
                            player_id,
                            fleet_id,
                            system_id,
                        });
                    }
                }
                ClientMsg::AttackFleet {
                    fleet_id,
                    target_id,
                } => {
                    // §offensive-orders Part 1: light-delayed like a raid.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AttackFleet {
                            player_id,
                            fleet_id,
                            target_id,
                        });
                    }
                }
                ClientMsg::SetFleetPosture { fleet_id, posture } => {
                    // §offensive-orders Part 2: instant per-fleet standing policy.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetPosture {
                            player_id,
                            fleet_id,
                            posture,
                        });
                    }
                }
                // §syndicates Part 1: instant owner-only alliance admin.
                ClientMsg::CreateSyndicate { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending
                            .push(Command::CreateSyndicate { player_id, name });
                    }
                }
                ClientMsg::InviteToSyndicate { name } => {
                    // Invite BY NAME: the invitee's stable id IS the hash of their
                    // corp name (the same function `Join` uses), so the server can
                    // resolve it without exposing a corp directory. A non-joined
                    // name resolves to an id the sim soft-rejects.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        let invitee = crate::protocol::player_id_from_name(&name);
                        self.pending
                            .push(Command::InviteToSyndicate { player_id, invitee });
                    }
                }
                ClientMsg::AcceptSyndicateInvite { syndicate_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AcceptSyndicateInvite {
                            player_id,
                            syndicate_id,
                        });
                    }
                }
                ClientMsg::LeaveSyndicate => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::LeaveSyndicate { player_id });
                    }
                }
                ClientMsg::DissolveSyndicate => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DissolveSyndicate { player_id });
                    }
                }
                ClientMsg::SetResearchQueue { queue } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending
                            .push(Command::SetResearchQueue { player_id, queue });
                    }
                }
                ClientMsg::SaveFit {
                    name,
                    ship,
                    loadout,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SaveFit {
                            player_id,
                            name,
                            ship,
                            loadout,
                        });
                    }
                }
                ClientMsg::DeleteFit { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DeleteFit { player_id, name });
                    }
                }
                ClientMsg::NameFlagship { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::NameFlagship { player_id, name });
                    }
                }
                ClientMsg::RecallRaid { raider_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RecallRaid {
                            player_id,
                            raider_id,
                        });
                    }
                }
                ClientMsg::MarketBuy {
                    commodity,
                    units,
                    max_unit_price,
                    ship_to,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketBuy {
                            player_id,
                            commodity,
                            units,
                            max_unit_price,
                            ship_to,
                        });
                    }
                }
                ClientMsg::HubLoad {
                    fleet_id,
                    commodity,
                    units,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HubLoad {
                            player_id,
                            fleet_id,
                            commodity,
                            units,
                        });
                    }
                }
                ClientMsg::HubUnload { fleet_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HubUnload {
                            player_id,
                            fleet_id,
                        });
                    }
                }
                ClientMsg::SystemLoad {
                    fleet_id,
                    system,
                    commodity,
                    units,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SystemLoad {
                            player_id,
                            fleet_id,
                            system,
                            commodity,
                            units,
                        });
                    }
                }
                ClientMsg::SystemUnload { fleet_id, system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SystemUnload {
                            player_id,
                            fleet_id,
                            system,
                        });
                    }
                }
                ClientMsg::HaulToMarketHub {
                    fleet_id,
                    sell_on_arrival,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HaulToMarketHub {
                            player_id,
                            fleet_id,
                            sell_on_arrival,
                        });
                    }
                }
                ClientMsg::PayReinstatement { points } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending
                            .push(Command::PayReinstatement { player_id, points });
                    }
                }
                ClientMsg::SetEngageFreight { fleet_id, on } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetEngageFreight {
                            player_id,
                            fleet_id,
                            on,
                        });
                    }
                }
                ClientMsg::BookFreightOut {
                    system,
                    commodity,
                    units,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BookFreightOut {
                            player_id,
                            system,
                            commodity,
                            units,
                        });
                    }
                }
                ClientMsg::BookFreightIn {
                    system,
                    commodity,
                    units,
                    sell_on_arrival,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BookFreightIn {
                            player_id,
                            system,
                            commodity,
                            units,
                            sell_on_arrival,
                        });
                    }
                }
                ClientMsg::MarketSell {
                    commodity,
                    units,
                    min_unit_price,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketSell {
                            player_id,
                            commodity,
                            units,
                            min_unit_price,
                        });
                    }
                }
                ClientMsg::PlaceLimitOrder {
                    side,
                    commodity,
                    units,
                    limit_price,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::PlaceLimitOrder {
                            player_id,
                            side,
                            commodity,
                            units,
                            limit_price,
                        });
                    }
                }
                ClientMsg::CancelLimitOrder { order_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::CancelLimitOrder {
                            player_id,
                            order_id,
                        });
                    }
                }
                ClientMsg::ShipProduction { system_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ShipProduction {
                            player_id,
                            system_id,
                        });
                    }
                }
                ClientMsg::StockSystem {
                    system_id,
                    commodity,
                    units,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::StockSystem {
                            player_id,
                            system_id,
                            commodity,
                            units,
                        });
                    }
                }
                ClientMsg::SetStandingOrder { order } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending
                            .push(Command::SetStandingOrder { player_id, order });
                    }
                }
                ClientMsg::ClearStandingOrder { order_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ClearStandingOrder {
                            player_id,
                            order_id,
                        });
                    }
                }
                ClientMsg::DismissLostOrder { order_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DismissLostOrder {
                            player_id,
                            order_id,
                        });
                    }
                }
                ClientMsg::SetFleetDoctrine { doctrine } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetDoctrine {
                            player_id,
                            doctrine,
                        });
                    }
                }
                ClientMsg::BuildShip {
                    system_id,
                    ship_kind,
                    join,
                    loadout,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuildShip {
                            player_id,
                            system_id,
                            ship_kind,
                            join,
                            loadout,
                        });
                    }
                }
                ClientMsg::BuildModule { system_id, module } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuildModule {
                            player_id,
                            system_id,
                            module,
                        });
                    }
                }
                ClientMsg::RefitShips {
                    fleet_id,
                    ship,
                    from,
                    to,
                    n,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RefitShips {
                            player_id,
                            fleet_id,
                            ship,
                            from,
                            to,
                            n,
                        });
                    }
                }
                ClientMsg::TransferModules { from, to, manifest } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TransferModules {
                            player_id,
                            from,
                            to,
                            manifest,
                        });
                    }
                }
                ClientMsg::BuyModule {
                    module,
                    n,
                    dest_system,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuyModule {
                            player_id,
                            module,
                            n,
                            dest_system,
                        });
                    }
                }
                ClientMsg::SellModule {
                    module,
                    n,
                    from_system,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SellModule {
                            player_id,
                            module,
                            n,
                            from_system,
                        });
                    }
                }
                ClientMsg::DevelopSystem {
                    system_id,
                    upgrade,
                    body_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DevelopSystem {
                            player_id,
                            system_id,
                            upgrade,
                            body_id,
                        });
                    }
                }
                ClientMsg::SetAssignment {
                    system_id,
                    structure,
                    workers,
                    specialists,
                    body_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetAssignment {
                            player_id,
                            system_id,
                            structure,
                            workers,
                            specialists,
                            body_id,
                        });
                    }
                }
                ClientMsg::HireSpecialist {
                    specialist,
                    dest_system,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HireSpecialist {
                            player_id,
                            specialist,
                            dest_system,
                        });
                    }
                }
                ClientMsg::TrainSpecialist {
                    system_id,
                    specialist,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TrainSpecialist {
                            player_id,
                            system_id,
                            specialist,
                        });
                    }
                }
                ClientMsg::TransferSpecialists { from, to, manifest } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TransferSpecialists {
                            player_id,
                            from,
                            to,
                            manifest,
                        });
                    }
                }
                ClientMsg::Withdraw { fleet_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::Withdraw {
                            player_id,
                            fleet_id,
                        });
                    }
                }
                ClientMsg::SetFleetTransit { fleet_id, mode } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetTransit {
                            player_id,
                            fleet_id,
                            mode,
                        });
                    }
                }
                ClientMsg::MergeFleets { into, from } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MergeFleets {
                            player_id,
                            into,
                            from,
                        });
                    }
                }
                ClientMsg::SplitFleet { fleet_id, counts } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SplitFleet {
                            player_id,
                            fleet_id,
                            counts,
                        });
                    }
                }
                ClientMsg::EstimateEngagement { attacker, target } => {
                    // A read-only QUERY (§FLEETS Part 3): project from this
                    // player's OWN view. Touches no authoritative state. The CHEAP
                    // read-out runs here on the loop; the EXPENSIVE 32-rollout
                    // Monte Carlo is handed to a blocking thread so a burst of
                    // estimate clicks can never stall the tick. One in flight per
                    // connection — repeat clicks drop until the current one lands.
                    if !self.estimate_inflight.contains(&conn_id)
                        && let Some(player_id) = self.sessions.player_of(conn_id)
                        && let Some(corp) = self.world.players.get(&player_id)
                    {
                        let cc = corp.command_center;
                        let c = self.world.config.c;
                        let now = self.world.time;
                        let arrays = self.world.array_sensor_sources(player_id);
                        if let Some(inputs) = crate::estimate::prepare_estimate(
                            &self.world,
                            &self.history,
                            player_id,
                            cc,
                            c,
                            now,
                            &arrays,
                            attacker,
                            target,
                        ) && let Some(outbound) = self.sessions.outbound_of(conn_id)
                        {
                            self.estimate_inflight.insert(conn_id);
                            let done_tx = self.estimate_done_tx.clone();
                            tokio::task::spawn_blocking(move || {
                                let est = crate::estimate::run_estimate(inputs);
                                // Deliver straight to the connection's own stream;
                                // then free it to request another (best-effort —
                                // if either end is gone the connection is closing).
                                let _ = outbound.try_send(ServerMsg::EngagementEstimate(est));
                                let _ = done_tx.send(conn_id);
                            });
                        }
                    }
                }
                // Join is handled at the WebSocket layer before the loop ever
                // sees intents on this connection; ignore a stray re-join.
                ClientMsg::Join { .. } => {
                    debug!(conn_id, "ignoring redundant join intent");
                }
            },
        }
    }

    /// Advance one tick: apply pending commands, integrate, persist, broadcast.
    fn tick(&mut self) {
        let commands = std::mem::take(&mut self.pending);
        // Snapshot the battles active BEFORE this step, keyed by id — any that are
        // gone AFTER the step concluded this tick, and we retain them so their
        // in-progress icon lingers until each viewer's conclusion light arrives
        // (§battles-take-time; see [`ConcludedBattle`]).
        let before: HashMap<sim::EntityId, sim::BattleInfo> = self
            .world
            .active_battles()
            .into_iter()
            .map(|b| (b.id, b))
            .collect();
        let systems_before = self.world.systems.len();
        let events = self.world.step(&commands);
        // A comet is emitted only after the sim has validated and scheduled the
        // command, so its id is the authoritative queue identity allocated at
        // `schedule_for_owner`. Do this before recording the post-step sample:
        // the route remains rooted in the player's picture at issue time, just
        // as it was when signals were emitted directly from `handle_input`.
        for event in &events {
            if let sim::EventPayload::OrderScheduled { id, owner, fleet } = event.payload {
                self.emit_command_signal(owner, fleet, id, event.time);
            }
        }
        // §over-capacity homes: a join past the pre-generated slot pool MINTS a
        // new home system mid-run — public geography that every connected
        // client's Welcome snapshot predates. Re-broadcast the star chart so
        // the new star is drawable and selectable everywhere (not least by its
        // own new owner, whose first click otherwise falls through to the
        // command-center anchor).
        if self.world.systems.len() != systems_before {
            let update = ServerMsg::GalaxyUpdate {
                systems: system_infos(&self.world),
            };
            for (_conn_id, info) in self.sessions.iter_conns() {
                let _ = info.outbound.try_send(update.clone());
            }
        }
        // Every battle ends inside `resolve_raids`, which runs BEFORE the clock
        // advances in `step`; so a battle that concluded this tick ended at
        // `world.time - DT` — exactly the `RaidResolved` event time the aftermath
        // report is stamped with. Riding that same instant makes the icon's
        // disappearance and the aftermath's arrival one light wavefront.
        if !before.is_empty() {
            let ended_at = self.world.time - DT;
            let still_active: std::collections::BTreeSet<sim::EntityId> =
                self.world.active_battles().iter().map(|b| b.id).collect();
            for (id, b) in before {
                if !still_active.contains(&id) {
                    self.concluded_battles.push(ConcludedBattle {
                        id,
                        pos: b.pos,
                        started_at: b.started_at,
                        ended_at,
                        a_owner: b.a_owner,
                        d_owner: b.d_owner,
                        participants: b.participants,
                    });
                }
            }
        }
        // Drop concluded battles whose conclusion light has reached even the
        // farthest possible viewer (galaxy diameter / warp light) — their icon has flipped
        // to aftermath everywhere, so nothing more references them.
        if !self.concluded_battles.is_empty() {
            let max_delay = 2.0 * self.world.config.galaxy_radius
                / sim::transit::signal_speed(self.world.config.c);
            let now = self.world.time;
            self.concluded_battles
                .retain(|cb| now - cb.ended_at <= max_delay + 1.0);
        }
        // §emplacements: remember demolished structures so their owner (and any
        // rival who had eyes on them) keeps seeing the wreck standing until the
        // news arrives. Same retention bound as concluded battles: once even the
        // slowest possible signal must have landed, nothing references them.
        for e in &events {
            if let sim::EventPayload::EmplacementDestroyed {
                emplacement,
                owner,
                kind,
                pos,
                ..
            } = e.payload
            {
                self.gone_emplacements.push(GoneEmplacement {
                    id: emplacement,
                    owner,
                    kind,
                    pos,
                    at: e.time,
                });
            }
        }
        if !self.gone_emplacements.is_empty() {
            let max_delay = 2.0 * self.world.config.galaxy_radius
                / sim::transit::signal_speed(self.world.config.c);
            let now = self.world.time;
            self.gone_emplacements
                .retain(|g| now - g.at <= max_delay + 1.0);
        }

        // Record true positions into the view filter's history every tick so
        // the retarded-time boundary resolves at full temporal resolution.
        self.history.record(&self.world);
        self.prices.record(&self.world);
        self.market_accounts.record(&self.world);
        // Queue any discrete events (raid outcomes) for delayed per-player
        // delivery.
        self.reports.ingest(&events);
        // Record events into the per-player check-in timeline (§16, Layer 3) at
        // their observable time, then promote any whose light has now arrived —
        // for ALL players, online or off (offline buffering is the whole point).
        self.timeline.ingest(&events, &self.world);
        self.timeline.promote(self.world.time);
        self.schedule_trade_reports(&events);
        self.deliver_trade_reports();
        for ev in &events {
            match &ev.payload {
                // Economy receipts are scheduled above from their physical
                // origin. Sending one here would reveal Market Hub truth before
                // its light and duplicate the eventual receipt.
                sim::EventPayload::Trade(_) => {}
                // A ship was destroyed in true space: tell the view filter so it
                // keeps serving the ghost until each player's light arrives, then
                // vanishes it (delayed, per-viewer — never FTL).
                sim::EventPayload::ShipDestroyed { ship, pos, .. } => {
                    self.history.mark_destroyed(*ship, ev.time, *pos);
                }
                sim::EventPayload::OrderConfirmed {
                    id,
                    owner,
                    fleet,
                    kind,
                } => {
                    self.sessions.send_to_player(
                        *owner,
                        ServerMsg::OrderConfirmed {
                            order_id: *id,
                            ship_id: *fleet,
                            kind: *kind,
                        },
                    );
                }
                _ => {}
            }
        }

        // Off-hot-path: append events to the log.
        if !events.is_empty() {
            let payloads = events.iter().map(to_json).collect::<Vec<_>>();
            self.persistence.submit(PersistJob::Events {
                tick: self.world.tick,
                time: self.world.time,
                events: payloads,
            });
        }

        if self.world.tick.is_multiple_of(BROADCAST_EVERY) {
            self.broadcast();
            self.publish_status();
        }

        if self.world.tick.is_multiple_of(self.snapshot_every) {
            self.persistence.submit(PersistJob::Snapshot {
                tick: self.world.tick,
                time: self.world.time,
                world: to_json(&self.world),
            });
        }
    }

    /// Push every connection its own per-player delayed/fogged view, each
    /// computed from THAT player's command center (§6, §14). No player ever
    /// receives true positions or another player's view — the fairness
    /// guarantee, enforced by [`PositionHistory::view_for`].
    fn broadcast(&mut self) {
        // These issue-time estimates are ephemeral view state. Retire them with
        // the authoritative lifecycle they describe so completed orders cannot
        // accumulate forever, while still keeping every owner's private plan.
        let active_orders: HashSet<(PlayerId, u64)> = self
            .world
            .players
            .keys()
            .flat_map(|owner| {
                self.world
                    .pending_commands(*owner)
                    .into_iter()
                    .map(move |pending| (*owner, pending.id))
            })
            .collect();
        self.observed_order_plans
            .retain(|key, _| active_orders.contains(key));

        let c = self.world.config.c;
        // Every corporation gets its own retarded picture, priced from that
        // corporation's command center over the shared straight-light model.
        let now = self.world.time;
        let tick = self.world.tick;
        let hub = self.world.hub;

        // Build each online player's view ONCE (shared across their
        // connections), plus any delayed reports whose light has now reached
        // them. Everything is computed from THIS player's command center and
        // light-gated. A connection whose corporation isn't in the world yet
        // (AddPlayer not processed) simply gets nothing this tick.
        let mut views: HashMap<PlayerId, ServerMsg> = HashMap::new();
        let mut reports: HashMap<PlayerId, Vec<ServerMsg>> = HashMap::new();
        let mut timelines: HashMap<PlayerId, ServerMsg> = HashMap::new();
        // Ordinary order confirmation is derived from these exact own-ghost
        // pictures after every player's View has been materialized. Keeping the
        // emission clocks beside the View prevents a second response timer from
        // claiming evidence the map has not served.
        let mut served_order_evidence: HashMap<PlayerId, Vec<(sim::EntityId, f64, bool)>> =
            HashMap::new();
        // §perf Part A: per-player battle-record specs (what each player MAY see
        // right now) — diffed per CONNECTION against its delivery cursor below.
        let mut record_specs: HashMap<PlayerId, Vec<view::RecordSpec>> = HashMap::new();
        // §ground G2: the same, for landing records.
        let mut ground_specs: HashMap<PlayerId, Vec<view::GroundSpec>> = HashMap::new();
        // §perf Part B: per-player slow-moving sections + their content
        // signatures — sent per connection only when a signature changed.
        let mut sections: HashMap<PlayerId, SectionData> = HashMap::new();
        // The published rankings are identical for everyone: one signature.
        let rankings_sig = sig_of(&self.world.rankings);
        for player_id in self.sessions.online_players() {
            let Some(corp) = self.world.players.get(&player_id) else {
                continue;
            };
            let cc = corp.command_center;
            // The viewer's standing SENSOR-ARRAY bubbles (§buildings step 2b) join
            // their coverage — same shared source of truth as the sim's pickets.
            let arrays = self.world.array_sensor_sources(player_id);
            // BATTLES (§battles-take-time), STRICTLY light-gated: a battle (and its
            // participants, revealed by weapons fire) appears only once the light
            // of its start has reached THIS player's command center.
            let mut battles: Vec<crate::protocol::BattleView> = Vec::new();
            let mut battle_reveal: std::collections::BTreeSet<sim::EntityId> =
                std::collections::BTreeSet::new();
            for b in self.world.active_battles() {
                let delay = sim::transit::delay(b.pos, cc, c);
                if now >= b.started_at + delay {
                    battle_reveal.extend(b.participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: b.id,
                        pos: b.pos,
                        age: delay,
                        started_at: b.started_at,
                        own: player_id == b.a_owner || player_id == b.d_owner,
                        // All participants are revealed to any observer of the
                        // battle (the weapons-fire site-reveal above), so their
                        // ids carry no more than the ghosts already sent.
                        participants: b.participants,
                    });
                }
            }
            // CONCLUDED battles whose conclusion light hasn't arrived yet: keep
            // showing the in-progress icon (and suppressing the participant ghosts
            // via `battle_reveal`) until `ended_at + delay` — the exact instant the
            // aftermath report lands. This bridges the FTL gap that used to let the
            // participant fleet icons re-appear between "battle ends" and "aftermath
            // arrives" (§battles-take-time). The `started_at + delay` lower bound
            // means a viewer whose START light never arrived (battle began and ended
            // faster than its light could reach them) still never sees a phantom icon.
            for cb in &self.concluded_battles {
                if cb.shows_in_progress(cc, c, now) {
                    battle_reveal.extend(cb.participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: cb.id,
                        pos: cb.pos,
                        age: sim::transit::delay(cb.pos, cc, c),
                        started_at: cb.started_at,
                        own: player_id == cb.a_owner || player_id == cb.d_owner,
                        participants: cb.participants.clone(),
                    });
                }
            }
            // §node: this viewer's regional dark-fleet effects (Veil quiets its
            // holders' dark fleets; Deep Scan resolves exact composition in-region).
            let veil_regions = self.world.active_veil_regions();
            let deep_scan_regions = self.world.deep_scan_regions(player_id);
            let mut ghosts = self.history.view_for_with_arrays(
                player_id,
                cc,
                c,
                now,
                &arrays,
                &battle_reveal,
                view::NodeEffects {
                    veil: &veil_regions,
                    deep_scan: &deep_scan_regions,
                },
            );
            served_order_evidence.insert(
                player_id,
                ghosts
                    .iter()
                    .filter(|ghost| ghost.own)
                    .map(|ghost| (ghost.id, now - ghost.age, ghost.jumped))
                    .collect(),
            );
            // §emplacements: WHICH STRUCTURES THIS VIEWER CAN SEE.
            //
            // Yours are always listed (your own installations report on their own
            // channel, like a shipyard's build queue). A RIVAL's appears only
            // inside your sensor coverage — the same union that detects dark
            // fleets, at your fleets' RETARDED positions — and that visibility is
            // what makes it a target: you cannot order a demolition on something
            // you have never seen.
            //
            // Structures are STATIONARY, so once a rival's is in coverage there is
            // nothing further to learn about where it is; the delay that matters is
            // the LOSS, handled below.
            let coverage = self.history.coverage_for(player_id, cc, c, now, &arrays);
            let mut emplacements_view: Vec<crate::protocol::EmplacementView> = self
                .world
                .emplacements
                .iter()
                .filter(|e| e.owner == player_id || view::within_coverage(&coverage, e.pos))
                .map(|e| crate::protocol::EmplacementView {
                    id: e.id,
                    kind: e.kind,
                    pos: e.pos,
                    sensor_range: e.kind.sensor_range(),
                    own: e.owner == player_id,
                })
                .collect();
            // …plus the ones already TORN DOWN whose news has not reached this
            // command center. The wreck keeps standing on the map exactly as a
            // destroyed ship's ghost keeps flying: learning of a demolition is
            // itself information, and it travels at straight warp-light speed.
            for g in &self.gone_emplacements {
                let seen_standing = g.owner == player_id || view::within_coverage(&coverage, g.pos);
                if !seen_standing {
                    continue;
                }
                let disappears_at = g.at + sim::transit::delay(g.pos, cc, c);
                if now < disappears_at {
                    emplacements_view.push(crate::protocol::EmplacementView {
                        id: g.id,
                        kind: g.kind,
                        pos: g.pos,
                        sensor_range: g.kind.sensor_range(),
                        own: g.owner == player_id,
                    });
                }
            }
            // §offensive-orders Part 2: attach each OWN fleet's engagement posture
            // (owner-only, fresh — a private standing policy like the corp doctrine;
            // rivals keep `None`, so it never leaks). The history-view can't see the
            // authoritative fleet, so fill it from the world here.
            for g in ghosts.iter_mut() {
                if g.own {
                    g.posture = self.world.fleets.get(&g.id).map(|f| f.posture);
                    // §TCA: owner-only blockade policy — does this fleet engage
                    // Authority freight arriving at the system it strangles?
                    // §TCA: the engage-freight choice is only MEANINGFUL for a
                    // fleet holding a Blockade (that's the only order the sim
                    // consults it under) — expose it only there, so the client
                    // never offers a toggle that does nothing.
                    g.engage_freight = self.world.fleets.get(&g.id).and_then(|f| {
                        matches!(f.order, sim::FleetOrder::Blockade { .. })
                            .then_some(f.engage_freight)
                    });
                    // §syndicates Part 3: OWNER-ONLY garrison status — if this fleet
                    // is stationed as an ally garrison, its host + fed state.
                    if let Some(host) = self.world.garrison_host_of(g.id) {
                        g.garrison_host = Some(host);
                        g.garrison_fed =
                            self.world.fleets.get(&g.id).is_some_and(|f| f.garrison_fed);
                    }
                    // §upkeep: OWNER-ONLY supply state — an unsupplied fleet is
                    // immobilized, and the panel must say so or a refused order
                    // looks like a bug.
                    g.supplied = self.world.fleets.get(&g.id).is_none_or(|f| f.supplied);
                    // §explore Part 2: OWNER-ONLY survey-dwell progress (0..1) for
                    // the progress ring — a rival never sees your order state.
                    g.survey_progress = self.world.fleets.get(&g.id).and_then(|f| match f.order {
                        sim::FleetOrder::Survey {
                            dwell_since: Some(since),
                            ..
                        } => Some(((now - since) / sim::explore::SURVEY_SECS).clamp(0.0, 1.0)),
                        _ => None,
                    });
                }
                // §syndicates Part 1: friendly ALLY tint — the owner (already on
                // the ghost) is a syndicate member as THIS viewer knows it
                // (light-delayed membership; `known_ally` returns false for own).
                g.ally = self.world.known_ally(player_id, g.owner, now);
                // §TCA: an Authority freighter's MANIFEST is two-tier PER ENTRY —
                // your own lots are always yours to see (they're your property),
                // everyone else's only from inside sensor range (`revealed`, the
                // same Tier-2 gate that governs a convoy's cargo). A distant rival
                // sees the hull go by and learns nothing about who ships what.
                if let Some(run) = self.world.freight_runs.get(&g.id) {
                    g.manifest = crate::view::visible_manifest(run, player_id, g.revealed);
                }
            }
            // §battle-aftermath: this player's RETAINED concluded-battle reports
            // (delivered = their light provably arrived). Strictly per-
            // participant — the scheduler holds them keyed by recipient.
            // §perf Part B: these ride the change-gated Sections lane now; the
            // retained sets change only by membership (entries are immutable
            // once delivered), so an id signature detects every change.
            let battle_reports: Vec<crate::protocol::BattleReportView> = self
                .reports
                .retained_for(player_id)
                .iter()
                .map(|r| crate::protocol::BattleReportView {
                    id: r.id,
                    pos: r.pos,
                    at_time: r.event_time,
                    learned_at: r.arrival_time,
                    you: r.you,
                    attacker_kind: r.attacker_kind,
                    target_kind: r.target_kind,
                    outcome: r.outcome,
                    attacker_losses: r.attacker_losses.clone(),
                    target_losses: r.target_losses.clone(),
                })
                .collect();
            // §contestable-territory Part 2: retained CAPTURE reports (per-participant).
            let capture_reports: Vec<crate::protocol::CaptureReportView> = self
                .reports
                .retained_captures_for(player_id)
                .iter()
                .map(|r| crate::protocol::CaptureReportView {
                    id: r.id,
                    pos: r.pos,
                    at_time: r.event_time,
                    learned_at: r.arrival_time,
                    captor: r.captor,
                    plunder: r.plunder.clone(),
                })
                .collect();
            let reports_sig = sig_of(&battle_reports.iter().map(|r| r.id).collect::<Vec<_>>());
            let captures_sig = sig_of(&capture_reports.iter().map(|r| r.id).collect::<Vec<_>>());
            // Signature over the USER-VISIBLE fields only — deliberately not the
            // whole struct: `next_eval_tick` is anti-spam bookkeeping the sim
            // bumps every evaluation period per active rule, and hashing it
            // would re-send the "change-gated" list every few seconds forever
            // (the client renders none of it; its own list signature skips it too).
            let standing_sig = sig_of(
                &corp
                    .standing_orders
                    .iter()
                    .map(|o| {
                        (
                            o.id,
                            &o.source,
                            &o.dest,
                            o.commodity,
                            &o.trigger,
                            &o.status,
                            o.in_flight,
                            o.sell_on_arrival,
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            sections.insert(
                player_id,
                SectionData {
                    standing: corp.standing_orders.clone(),
                    standing_sig,
                    reports: battle_reports,
                    reports_sig,
                    captures: capture_reports,
                    captures_sig,
                },
            );
            let anchors = view::filter_anchors(&self.world.home_slots, player_id, cc, c, now);
            // §syndicates Part 2: each syndicate ally's relayable scout intel (their
            // command center is the relay source). The View chain-light-delays each
            // ally's snapshots to this viewer, provenance preserved.
            let ally_intel: Vec<view::AllyIntel> = self
                .world
                .allies_of(player_id)
                .iter()
                .filter_map(|a| {
                    self.world.players.get(a).map(|ac| view::AllyIntel {
                        id: *a,
                        cc: ac.command_center,
                        intel: &ac.intel,
                    })
                })
                .collect();
            let mut systems = view::filter_systems(
                &self.world.systems,
                player_id,
                cc,
                c,
                now,
                &self.world.build_queue,
                self.world.tick,
                DT,
                &corp.intel,
                &ally_intel,
                &corp.surveyed,
            );
            // §syndicates Part 1: friendly ALLY tint on systems whose (light-gated
            // known) owner is a syndicate member as THIS viewer knows it. Composes
            // both light-gates; grants no owner-only data (Part 1 is tint only).
            for sv in systems.iter_mut() {
                sv.ally = sv
                    .owner
                    .is_some_and(|o| self.world.known_ally(player_id, o, now));
                // §ground G4: the PRE-COMMIT LANDING ESTIMATE. Only for a
                // besieger who actually has marines in orbit here — the person
                // making the decision, and nobody else. It is sampled from the
                // REAL ground engine, so it can never drift from the fight it
                // predicts, and it is computed from state this viewer already
                // holds (their own troops, the garrison their `ground` readout
                // already shows), so it discloses nothing new.
                if let Some(g) = sv.ground.as_mut()
                    && sv.owner != Some(player_id)
                    && let Some(sys) = self.world.systems.iter().find(|s| s.id == sv.id)
                    && sys.blockade.is_some_and(|b| b.by == player_id)
                {
                    let marines: u32 = self
                        .world
                        .fleets
                        .values()
                        .filter(|f| {
                            f.owner == player_id
                                && f.pos.distance(sys.pos) <= sim::ship::COLONY_CLAIM_RADIUS
                        })
                        .map(|f| f.marines())
                        .sum();
                    if marines > 0 {
                        let o = sim::ground::project_landing(
                            marines,
                            sys.tier_sum(sim::StructureKind::Garrison),
                            if sys.garrison_fed {
                                sys.garrison_suppression
                            } else {
                                1.0
                            },
                            self.world.config.battle_target_secs,
                            sys.id.0,
                            sim::ground::LANDING_ROLLOUTS,
                        );
                        g.landing = Some(crate::protocol::LandingOddsView {
                            marines,
                            win: o.win,
                            win_if_guns_leave: o.win_if_guns_leave,
                            expected_losses: o.expected_marine_losses.round() as u32,
                            expected_secs: o.expected_secs,
                        });
                    }
                }
                // §syndicates Part 3: OWNER-ONLY hosted-garrison indicator (the
                // coalition shield you're feeding). Only for your OWN systems.
                if sv.owner == Some(player_id)
                    && let Some((ships, fed)) = self.world.hosted_garrison(sv.id)
                {
                    sv.ally_garrison_ships = ships;
                    sv.ally_garrison_fed = fed;
                }
                // §node: attach the system's EXOTIC NODE, if any. Bonus + awakened
                // are PUBLIC (an awakened node is a galaxy-wide landmark; its awaken
                // time is public config, so the flag leaks nothing); `fed` and the
                // region ring are OWNER-ONLY.
                if let Some(n) = self.world.nodes.get(&sv.id) {
                    let own = sv.owner == Some(player_id);
                    sv.node = Some(crate::protocol::NodeStateView {
                        bonus: n.bonus.slug().to_string(),
                        title: n.bonus.title().to_string(),
                        awakened: n.awakened,
                        fed: own && n.fed,
                        region_radius: if own { sim::NODE_REGION_RADIUS } else { 0.0 },
                    });
                }
            }
            // §syndicates Part 1: the viewer's OWN roster + pending invites (fresh
            // private state, never a rival's private roster).
            let syndicate = corp
                .syndicate
                .and_then(|sid| self.world.syndicates.get(&sid))
                .map(|s| {
                    Box::new(crate::protocol::SyndicateView {
                        id: s.id,
                        name: s.name.clone(),
                        founder: s.founder,
                        is_founder: s.founder == player_id,
                        members: s
                            .members
                            .iter()
                            .map(|m| crate::protocol::SyndicateMember {
                                id: *m,
                                name: self
                                    .world
                                    .players
                                    .get(m)
                                    .map(|c| c.name.clone())
                                    .unwrap_or_default(),
                            })
                            .collect(),
                        invited: s
                            .invites
                            .iter()
                            .filter_map(|i| self.world.players.get(i).map(|c| c.name.clone()))
                            .collect(),
                        // §fitting: the shared doctrine-fit library (owner-only).
                        fits: s
                            .fits
                            .iter()
                            .map(|f| crate::protocol::FitView {
                                name: f.name.clone(),
                                kind: f.kind,
                                modules: f.loadout.modules().to_vec(),
                            })
                            .collect(),
                        // §ladder B4: the christened Titan (owner-only here).
                        flagship_name: s.flagship_name.clone(),
                    })
                });
            let syndicate_invites: Vec<crate::protocol::SyndicateInviteView> = self
                .world
                .syndicates
                .values()
                .filter(|s| s.invites.contains(&player_id))
                .map(|s| crate::protocol::SyndicateInviteView {
                    id: s.id,
                    name: s.name.clone(),
                })
                .collect();
            // §research R6: the viewer's OWN research picture (owner-only), present
            // only while affiliated (research is a syndicate institution).
            let research = corp
                .syndicate
                .filter(|sid| self.world.syndicates.contains_key(sid))
                .map(|sid| Box::new(research_view(&self.world, sid)));

            // Lagged hub ticker: prices as of the light that has reached this
            // player's command center from the hub.
            let staleness = sim::transit::delay(hub, cc, c);
            let lagged = self.prices.at(now - staleness);
            let prices = lagged
                .map(|m| {
                    m.prices
                        .iter()
                        .map(|(commodity, price)| PriceView {
                            commodity: *commodity,
                            price: *price,
                            available_buy: m.available_buy.get(commodity).copied().unwrap_or(0),
                            available_sell: m.available_sell.get(commodity).copied().unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let market = MarketView { prices, staleness };

            // Market account truth is emitted at the physical hub, just like its
            // ticker. Serving credits, warehouse or resting orders fresh would
            // disclose a fill before the settlement receipt's light arrived.
            // §TCA Phase 2: the player's own charter standing. The BAND is always
            // derived (never stored), so this can't desync from the sim.
            let standing = corp.tca_standing;
            let charter = crate::protocol::CharterView {
                standing,
                max_standing: sim::tca::TCA_STANDING_MAX,
                status: sim::charter_status(standing),
                title: sim::charter_status(standing).title(),
                // (§perf Part B: the static ladder rides Welcome now.)
                tariff_mult: sim::tca::tariff_mult(standing),
                market_penalty_frac: sim::tca::market_penalty_frac(standing),
                reinstate_cost_per_point: sim::tca::TCA_REINSTATE_FEE_PER_POINT,
            };

            let known_account = self.market_accounts.at(player_id, now - staleness);
            let wallet = WalletView {
                credits: known_account.map_or(corp.credits, |account| account.credits),
                valuation: known_account.map_or(corp.valuation, |account| account.valuation),
                warehouse: known_account
                    .map(|account| &account.warehouse)
                    .unwrap_or(&corp.warehouse)
                    .iter()
                    .map(|(commodity, units)| InvSlot {
                        commodity: *commodity,
                        units: *units,
                    })
                    .collect(),
                orders: if let Some(account) = known_account {
                    account
                        .orders
                        .iter()
                        .map(|o| OrderView {
                            id: o.id,
                            side: o.side,
                            commodity: o.commodity,
                            units: o.units,
                            limit_price: o.limit_price,
                        })
                        .collect()
                } else {
                    self.world
                        .book
                        .iter()
                        .filter(|o| o.player == player_id)
                        .map(|o| OrderView {
                            id: o.id,
                            side: o.side,
                            commodity: o.commodity,
                            units: o.units,
                            limit_price: o.limit_price,
                        })
                        .collect()
                },
                // The fleet's fuel reserve: sum Fuel across this player's systems
                // (owner-only — read off systems we own, so it never leaks).
                fuel_total: self
                    .world
                    .systems
                    .iter()
                    .filter(|s| s.owner == Some(player_id))
                    .map(|s| {
                        s.stockpile
                            .get(&sim::Commodity::Fuel)
                            .copied()
                            .unwrap_or(0.0)
                    })
                    .sum(),
            };

            // §battle-records A2: the viewer's CURRENT sensor coverage (command
            // center + standing arrays + their own fleets' bubbles) gates a
            // third party's bucket access to a battle site.
            let mut coverage: Vec<(sim::Vec2, f64)> = vec![(cc, self.world.config.sensor_range)];
            coverage.extend_from_slice(&arrays);
            for f in self.world.fleets.values() {
                if f.owner == player_id {
                    coverage.push((f.pos, self.world.config.sensor_range * f.sensor_mult()));
                }
            }
            // §perf Part A: records no longer ride the View. Enumerate what this
            // player MAY see (cheap — no round materialization); the send loop
            // below diffs each of their connections' cursors against this and
            // ships only the increments, on the reliable discrete lane.
            let specs = view::visible_record_specs(
                &self.world.battle_records,
                player_id,
                cc,
                c,
                now,
                &coverage,
                &|corp| {
                    // §ladder B4: resolve a side's christened Titan name.
                    self.world
                        .players
                        .get(&corp)
                        .and_then(|p| p.syndicate)
                        .and_then(|sid| self.world.syndicates.get(&sid))
                        .and_then(|s| s.flagship_name.clone())
                },
            );
            record_specs.insert(player_id, specs);
            // §ground G2: which landings this player may see, at what fidelity.
            ground_specs.insert(
                player_id,
                view::visible_ground_specs(
                    &self.world.ground_records,
                    player_id,
                    cc,
                    c,
                    now,
                    &coverage,
                ),
            );
            // §TCA: the Market Hub freight desk. Terms for every system this
            // player owns (the only valid destinations), plus their OWN lots.
            let freight = crate::protocol::FreightView {
                next_departure: self.world.next_freight_departure(),
                period: self.world.freight_period_secs(),
                fee_frac: sim::tca::TCA_FREIGHT_FEE_FRAC,
                fee_per_unit_dist: sim::tca::TCA_FREIGHT_FEE_PER_UNIT_DIST,
                terms: self
                    .world
                    .systems
                    .iter()
                    .filter(|s| s.owner == Some(player_id))
                    .map(|s| {
                        let distance = hub.distance(s.pos);
                        let secs_out = sim::World::freight_flight_secs(distance);
                        crate::protocol::FreightTermsView {
                            system: s.id,
                            distance,
                            cap: sim::tca::TCA_SHIPMENT_CAP,
                            secs_out,
                            secs_round: secs_out * 2.0,
                        }
                    })
                    .collect(),
                shipments: self
                    .world
                    .shipments_of(player_id)
                    .into_iter()
                    .map(|(s, aboard)| crate::protocol::ShipmentView {
                        id: s.id.0,
                        system: s.system,
                        commodity: s.commodity,
                        units: s.units,
                        direction: s.direction,
                        sell_on_arrival: s.sell_on_arrival,
                        fee_paid: s.fee_paid,
                        booked_at: s.booked_at,
                        aboard,
                    })
                    .collect(),
            };

            views.insert(
                player_id,
                ServerMsg::View {
                    tick,
                    sim_time: now,
                    command_center: cc,
                    anchors,
                    systems,
                    ghosts,
                    emplacements: emplacements_view,
                    market,
                    wallet,
                    charter,
                    freight,
                    // The player's own fleet doctrine (fresh private policy).
                    doctrine: corp.doctrine,
                    // The player's own in-flight order lifecycles (§order-lifecycle)
                    // — owner-only private command data, like the wallet.
                    pending_orders: pending_order_views(
                        &self.world,
                        player_id,
                        &self.observed_order_plans,
                        now,
                    ),
                    battles,
                    syndicate,
                    syndicate_invites,
                    research,
                },
            );
            let due = self.reports.due_for(player_id, cc, c, now);
            if !due.is_empty() {
                reports.insert(
                    player_id,
                    due.into_iter()
                        .map(|r| ServerMsg::Report { report: r })
                        .collect(),
                );
            }

            // Mark the player online (advances their "away" boundary), and if their
            // check-in timeline gained entries since we last pushed (e.g. an
            // auto-dispatch or a battle whose light just arrived), re-send the digest.
            self.timeline.mark_seen(player_id, now);
            let jlen = self.timeline.journal_len(player_id);
            if self.timeline_sent.get(&player_id).copied().unwrap_or(0) != jlen {
                self.timeline_sent.insert(player_id, jlen);
                let (entries, away_since) = self.timeline.digest(player_id);
                timelines.insert(
                    player_id,
                    ServerMsg::Timeline {
                        entries,
                        away_since,
                    },
                );
            }
        }

        // §comms-v3.7 ONE CONFIRMATION CLOCK: the served own-fleet picture above
        // is the evidence. The sim's `echo_at` remains a panel estimate only;
        // expiry can say "overdue" but can never manufacture OrderConfirmed.
        // Remove each row from the already-built View on this same broadcast,
        // then emit the reliable lifecycle event for its toast/timeline.
        let mut confirmation_events = Vec::new();
        for (owner, evidence) in &served_order_evidence {
            confirmation_events.extend(self.world.confirm_orders_from_served(*owner, evidence));
        }
        if !confirmation_events.is_empty() {
            let mut confirmed_by_owner: HashMap<PlayerId, HashSet<u64>> = HashMap::new();
            for event in &confirmation_events {
                if let sim::EventPayload::OrderConfirmed {
                    id,
                    owner,
                    fleet,
                    kind,
                } = &event.payload
                {
                    confirmed_by_owner.entry(*owner).or_default().insert(*id);
                    self.observed_order_plans.remove(&(*owner, *id));
                    self.sessions.send_to_player(
                        *owner,
                        ServerMsg::OrderConfirmed {
                            order_id: *id,
                            ship_id: *fleet,
                            kind: *kind,
                        },
                    );
                }
            }
            for (owner, confirmed) in &confirmed_by_owner {
                if let Some(ServerMsg::View { pending_orders, .. }) = views.get_mut(owner) {
                    pending_orders.retain(|pending| !confirmed.contains(&pending.id));
                }
            }
            self.reports.ingest(&confirmation_events);
            self.timeline.ingest(&confirmation_events, &self.world);
            self.timeline.promote(now);
            for owner in confirmed_by_owner.keys().copied() {
                let jlen = self.timeline.journal_len(owner);
                if self.timeline_sent.get(&owner).copied().unwrap_or(0) != jlen {
                    self.timeline_sent.insert(owner, jlen);
                    let (entries, away_since) = self.timeline.digest(owner);
                    timelines.insert(
                        owner,
                        ServerMsg::Timeline {
                            entries,
                            away_since,
                        },
                    );
                }
            }
            self.persistence.submit(PersistJob::Events {
                tick,
                time: now,
                events: confirmation_events.iter().map(to_json).collect(),
            });
        }

        for (_conn_id, info) in self.sessions.iter_conns_mut() {
            if let Some(view) = views.get(&info.player_id) {
                // Last-write-wins: overwrite this connection's latest-View slot.
                // A slow client simply never sees the frames it fell behind on —
                // the writer always emits the freshest, never a stale backlog.
                // (Err only if the writer task is already gone; harmless.)
                let _ = info.view_tx.send(Some(view.clone()));
            }
            // §perf Part A: this connection's battle-record increments, on the
            // RELIABLE lane (cursor committed only when the send succeeds).
            if let Some(specs) = record_specs.get(&info.player_id) {
                send_record_deltas(&self.world.battle_records, specs, info);
            }
            // §ground G2: this connection's landing-record increments.
            if let Some(specs) = ground_specs.get(&info.player_id) {
                send_ground_deltas(&self.world.ground_records, specs, info);
            }
            // §perf Part B: the change-gated slow sections, same reliable lane.
            if let Some(sec) = sections.get(&info.player_id) {
                send_sections(sec, rankings_sig, &self.world.rankings, info);
            }
            if let Some(reps) = reports.get(&info.player_id) {
                for r in reps {
                    let _ = info.outbound.try_send(r.clone());
                }
            }
            if let Some(tl) = timelines.get(&info.player_id) {
                let _ = info.outbound.try_send(tl.clone());
            }
        }
    }
}

/// §perf Part B: one player's slow-moving sections for this broadcast, with
/// their content signatures (computed once per player, compared per connection).
struct SectionData {
    standing: Vec<sim::StandingOrder>,
    standing_sig: u64,
    reports: Vec<crate::protocol::BattleReportView>,
    reports_sig: u64,
    captures: Vec<crate::protocol::CaptureReportView>,
    captures_sig: u64,
}

/// §perf Part B: a cheap content signature — the serialized JSON hashed. Used on
/// small, slow-moving payloads only (standing orders, report id lists, the
/// ~20-row rankings), never on the big per-tick state.
fn sig_of<T: serde::Serialize>(v: &T) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// §perf Part B: send this connection whichever sections changed since it last
/// received them (a present field REPLACES the client copy; absent = unchanged).
/// Signatures are committed only when the send succeeds — a full queue retries
/// next broadcast, never silently losing a once-per-change section.
fn send_sections(
    sec: &SectionData,
    rankings_sig: u64,
    rankings: &[sim::RankingRow],
    info: &mut ConnInfo,
) {
    let sent = &info.sent;
    let send_standing = sent.standing_sig != Some(sec.standing_sig);
    let send_reports = sent.reports_sig != Some(sec.reports_sig);
    let send_captures = sent.captures_sig != Some(sec.captures_sig);
    let send_ranks = sent.rankings_sig != Some(rankings_sig);
    if !(send_standing || send_reports || send_captures || send_ranks) {
        return;
    }
    let msg = ServerMsg::Sections {
        standing_orders: send_standing.then(|| sec.standing.clone()),
        battle_reports: send_reports.then(|| sec.reports.clone()),
        capture_reports: send_captures.then(|| sec.captures.clone()),
        rankings: send_ranks.then(|| rankings.to_vec()),
    };
    if info.outbound.try_send(msg).is_ok() {
        if send_standing {
            info.sent.standing_sig = Some(sec.standing_sig);
        }
        if send_reports {
            info.sent.reports_sig = Some(sec.reports_sig);
        }
        if send_captures {
            info.sent.captures_sig = Some(sec.captures_sig);
        }
        if send_ranks {
            info.sent.rankings_sig = Some(rankings_sig);
        }
    }
}

/// §perf Part A: diff one connection's delivery cursors against what its player
/// may see right now, and send exactly the increments. Everything visible-related
/// was already decided upstream (specs come from the light/fidelity filter); this
/// function only decides HOW MUCH of it this connection still needs.
///
/// Delivery is atomic per broadcast: cursors/removals are committed only when
/// `try_send` succeeds, so a full queue (stalled client) retries next broadcast
/// and can never silently lose an increment.
fn send_record_deltas(
    records: &std::collections::BTreeMap<sim::EntityId, sim::BattleRecord>,
    specs: &[view::RecordSpec],
    info: &mut ConnInfo,
) {
    use crate::protocol::BattleRecordUpdate;
    use crate::session::RecordCursor;

    let mut updates: Vec<BattleRecordUpdate> = Vec::new();
    // Cursor writes staged here, applied only if the send lands.
    let mut staged: Vec<(sim::EntityId, RecordCursor)> = Vec::new();
    for spec in specs {
        let Some(r) = records.get(&spec.id) else {
            continue;
        };
        let participant = matches!(spec.fidelity, crate::protocol::BattleFidelity::Participant);
        let cur = info.sent.records.get(&spec.id);
        let is_new = cur.is_none();
        let names_changed = cur.is_some_and(|c| c.names != spec.names);
        // Never regress: if the cursor is somehow ahead (it can't be for a fixed
        // command center — arrival is strictly increasing), send nothing extra.
        let from = cur.map_or(0, |c| c.rounds_sent).min(spec.arrived_len);
        let new_rounds = if spec.arrived_len > from {
            view::record_rounds_range(r, from, spec.arrived_len, participant)
        } else {
            Vec::new()
        };
        let send_outcome = spec.outcome.is_some() && !cur.is_some_and(|c| c.outcome_sent);
        if is_new || names_changed || !new_rounds.is_empty() || send_outcome {
            updates.push(BattleRecordUpdate {
                id: spec.id,
                header: (is_new || names_changed).then(|| view::record_header(r, spec)),
                new_rounds,
                light_frontier_tick: spec.frontier_tick,
                outcome: if send_outcome { spec.outcome } else { None },
            });
            staged.push((
                spec.id,
                RecordCursor {
                    rounds_sent: spec.arrived_len.max(cur.map_or(0, |c| c.rounds_sent)),
                    outcome_sent: send_outcome || cur.is_some_and(|c| c.outcome_sent),
                    names: spec.names.clone(),
                },
            ));
        }
    }
    // Records this connection holds that are no longer visible to its player:
    // pruned server-side, or a bucket viewer's coverage of the site lapsed.
    // Exactly the entries that vanished from the old full-set View; if coverage
    // resumes, the empty cursor re-sends the record in full — as before.
    let visible: std::collections::HashSet<sim::EntityId> = specs.iter().map(|s| s.id).collect();
    let removed: Vec<sim::EntityId> = info
        .sent
        .records
        .keys()
        .filter(|id| !visible.contains(id))
        .copied()
        .collect();

    if updates.is_empty() && removed.is_empty() {
        return;
    }
    let msg = ServerMsg::BattleRecords {
        updates,
        removed: removed.clone(),
    };
    if info.outbound.try_send(msg).is_ok() {
        for (id, cursor) in staged {
            info.sent.records.insert(id, cursor);
        }
        for id in removed {
            info.sent.records.remove(&id);
        }
    }
}

/// §ground G2: one connection's LANDING-record increments, on the same reliable
/// lane and the same cursor discipline as battle records — new records get a
/// header, known ones get only the rounds whose light has newly arrived, and the
/// cursor advances only if the send lands.
fn send_ground_deltas(
    records: &std::collections::BTreeMap<sim::EntityId, sim::ground::GroundRecord>,
    specs: &[view::GroundSpec],
    info: &mut ConnInfo,
) {
    use crate::protocol::{GroundFidelity, GroundRecordUpdate};
    use crate::session::RecordCursor;

    let mut updates: Vec<GroundRecordUpdate> = Vec::new();
    let mut staged: Vec<(sim::EntityId, RecordCursor)> = Vec::new();
    for spec in specs {
        let Some(r) = records.get(&spec.id) else {
            continue;
        };
        let participant = matches!(spec.fidelity, GroundFidelity::Participant);
        let cur = info.sent.ground_records.get(&spec.id);
        let is_new = cur.is_none();
        let from = cur.map_or(0, |c| c.rounds_sent).min(spec.arrived_len);
        let new_rounds = if spec.arrived_len > from {
            view::ground_rounds_range(r, from, spec.arrived_len, participant)
        } else {
            Vec::new()
        };
        let send_outcome = spec.outcome.is_some() && !cur.is_some_and(|c| c.outcome_sent);
        if is_new || !new_rounds.is_empty() || send_outcome {
            updates.push(GroundRecordUpdate {
                id: spec.id,
                header: is_new.then(|| view::ground_header(r, spec)),
                new_rounds,
                light_frontier_tick: spec.frontier_tick,
                outcome: if send_outcome {
                    spec.outcome.clone()
                } else {
                    None
                },
            });
            staged.push((
                spec.id,
                RecordCursor {
                    rounds_sent: spec.arrived_len.max(cur.map_or(0, |c| c.rounds_sent)),
                    outcome_sent: send_outcome || cur.is_some_and(|c| c.outcome_sent),
                    names: [None, None],
                },
            ));
        }
    }
    let visible: std::collections::HashSet<sim::EntityId> = specs.iter().map(|s| s.id).collect();
    let removed: Vec<sim::EntityId> = info
        .sent
        .ground_records
        .keys()
        .filter(|id| !visible.contains(id))
        .copied()
        .collect();

    if updates.is_empty() && removed.is_empty() {
        return;
    }
    let msg = ServerMsg::GroundRecords {
        updates,
        removed: removed.clone(),
    };
    if info.outbound.try_send(msg).is_ok() {
        for (id, cursor) in staged {
            info.sent.ground_records.insert(id, cursor);
        }
        for id in removed {
            info.sent.ground_records.remove(&id);
        }
    }
}

/// The buildable options + their recipes (§step1), built from the sim's const
/// The public star chart as SystemInfo rows — the Welcome galaxy's `systems`
/// and every GalaxyUpdate re-broadcast share this one mapper, so the two can
/// never drift.
fn system_infos(world: &sim::World) -> Vec<SystemInfo> {
    world
        .systems
        .iter()
        .map(|s| SystemInfo {
            id: s.id,
            pos: s.pos,
            name: s.name.clone(),
            band: world.band_of(s).slug(),
            claim_cost: s.claim_cost,
        })
        .collect()
}

/// recipes and sent once in the Welcome galaxy. Whole-unit costs for the UI.
fn build_options() -> Vec<BuildOptionView> {
    use sim::{BuildKind, ShipKind, StructureKind};
    // §economy: every buildable hull + ALL structures, data-driven (keys = slugs;
    // a legacy client sending an old slug still parses via the serde aliases).
    // Structures come from `StructureKind::ALL`, so a new one appears here for
    // free — a new SHIP does not, and has to be listed below.
    let ships = [
        (
            "convoy",
            "Convoy",
            BuildKind::Ship {
                ship: ShipKind::Convoy,
            },
        ),
        // §emplacements: the crane. Communications and sensors are placed BY this hull —
        // built here, then dispatched to the site from the map.
        (
            "builder",
            "Construction Ship",
            BuildKind::Ship {
                ship: ShipKind::Builder,
            },
        ),
        (
            "raider",
            "Raider",
            BuildKind::Ship {
                ship: ShipKind::Raider,
            },
        ),
        (
            "scout",
            "Scout",
            BuildKind::Ship {
                ship: ShipKind::Scout,
            },
        ),
        (
            "corvette",
            "Corvette",
            BuildKind::Ship {
                ship: ShipKind::Corvette,
            },
        ),
        (
            "colony",
            "Colony Ship",
            BuildKind::Ship {
                ship: ShipKind::Colony,
            },
        ),
        // §ladder: the warship ladder (research-gated hulls; the client shows
        // the gate copy, the sim enforces UnlockHull at BuildShip).
        (
            "destroyer",
            "Destroyer",
            BuildKind::Ship {
                ship: ShipKind::Destroyer,
            },
        ),
        (
            "cruiser",
            "Cruiser",
            BuildKind::Ship {
                ship: ShipKind::Cruiser,
            },
        ),
        (
            "battleship",
            "Battleship",
            BuildKind::Ship {
                ship: ShipKind::Battleship,
            },
        ),
        (
            "dreadnought",
            "Dreadnought",
            BuildKind::Ship {
                ship: ShipKind::Dreadnought,
            },
        ),
        (
            "titan",
            "Titan",
            BuildKind::Ship {
                ship: ShipKind::Titan,
            },
        ),
        // §ground: the troopship. Gated by a Garrison rather than a yard, but it
        // is an ordinary ship job otherwise.
        (
            "transport",
            "Troop Transport",
            BuildKind::Ship {
                ship: ShipKind::Transport,
            },
        ),
        // §ground: the troopship. Gated by a Garrison rather than a yard, but it
        // is an ordinary ship job otherwise.
    ];
    // §modules Part B3: the 5 modules, keyed `module:<slug>` so the client routes
    // them to BuildModule (not BuildShip/DevelopSystem) while reusing the same
    // recipe-cost channel. They hold no slot and gate on an Armaments Complex.
    let modules = sim::module::MODULE_KINDS.map(|m| {
        (
            format!("module:{}", m.slug()),
            m.label().to_string(),
            BuildKind::Module { module: m },
        )
    });
    ships
        .into_iter()
        .map(|(k, l, w)| (k.to_string(), l.to_string(), w))
        .chain(StructureKind::ALL.into_iter().map(|k| {
            (
                k.slug().to_string(),
                k.title().to_string(),
                BuildKind::Upgrade { upgrade: k },
            )
        }))
        .chain(modules)
        .map(|(key, label, what)| {
            let r = sim::build::recipe_for(what);
            BuildOptionView {
                key,
                label,
                costs: r
                    .costs
                    .iter()
                    .map(|(c, n)| StockSlot {
                        commodity: *c,
                        units: *n as u32,
                    })
                    .collect(),
                build_secs: r.build_ticks as f64 / TICK_HZ as f64,
            }
        })
        .collect()
}

/// §research R6: the gate progress bar for a SEALED node — the verb/metric the
/// tier waits on, current vs threshold. `None` when the tier carries no verb gate
/// (Tier I, or a IV/V node gated only by its ladder predecessor).
fn gate_progress(
    p: &sim::research::Programme,
    rs: &sim::research::ResearchState,
    metric: &dyn Fn(sim::research::Metric) -> f64,
    now: f64,
) -> Option<crate::protocol::GateProgressView> {
    use sim::research::Gate;
    match sim::research::tier_gate(p.field, p.school, p.tier) {
        Gate::None => None,
        Gate::Cumulative(v, t) => Some(crate::protocol::GateProgressView {
            label: v.label().to_string(),
            current: rs.verb(v),
            threshold: t,
        }),
        Gate::State(m, t) => Some(crate::protocol::GateProgressView {
            label: m.label().to_string(),
            current: metric(m),
            threshold: t,
        }),
        Gate::Sustained(m, _t, secs) => {
            // The endurance clock: days held continuously vs the required window.
            let held = rs
                .sustained_since
                .get(&m)
                .map(|since| (now - *since as f64).max(0.0) / 86_400.0)
                .unwrap_or(0.0);
            Some(crate::protocol::GateProgressView {
                label: format!("days holding {}", m.label()),
                current: held,
                threshold: secs as f64 / 86_400.0,
            })
        }
    }
}

/// §perf Part B: the STATIC programme catalog — names, blurbs, board topology,
/// costs. The same constant table for every client (public rulebook, no one's
/// progress), sent once in Welcome.
fn research_catalog() -> Vec<crate::protocol::ProgrammeInfo> {
    sim::research::visible_ids()
        .filter_map(|id| {
            let p = sim::research::programme(id)?;
            Some(crate::protocol::ProgrammeInfo {
                id: id.to_string(),
                field: p.field.slug().to_string(),
                school: p.school.map(|s| s.slug().to_string()),
                tier: p.tier,
                name: p.name.to_string(),
                blurb: p.blurb.to_string(),
                cost: sim::research::cost_of(id),
            })
        })
        .collect()
}

/// §research R6: build the viewer's OWN syndicate research picture (owner-only).
fn research_view(world: &sim::World, sid: sim::SyndicateId) -> crate::protocol::ResearchView {
    use crate::protocol::{AcademyRow, ActiveResearchView, ProgrammeDynView, ResearchView};
    let syn = &world.syndicates[&sid];
    let rs = &syn.research;
    let now = world.time;
    let metric = |m| world.syndicate_metric(sid, m);

    // The per-Academy contribution table (the same factor chain the clock uses).
    let contribs = world.research_contributions(sid);
    let rate: f64 = contribs.iter().filter(|c| c.supplied).map(|c| c.rate).sum();
    let academies = contribs
        .iter()
        .map(|c| AcademyRow {
            system: c.system_name.clone(),
            body_id: c.body_id,
            tier: c.tier,
            throughput: c.throughput,
            staffing: c.staffing,
            skill: c.skill,
            food: c.food,
            rate: c.rate,
            supplied: c.supplied,
        })
        .collect();

    // The active programme banner (with a live ETA at the current rate).
    let active = rs.active.as_deref().and_then(|id| {
        sim::research::programme(id).map(|p| {
            let cost = sim::research::cost_of(id);
            let eta_secs = if rate > 1e-9 {
                Some((cost - rs.progress).max(0.0) / rate)
            } else {
                None
            };
            ActiveResearchView {
                id: id.to_string(),
                name: p.name.to_string(),
                progress: rs.progress,
                cost,
                eta_secs,
            }
        })
    });

    // The whole visible tree, each node tagged with the viewer's state + gate.
    // §perf Part B: DYNAMIC slice only — the static catalog rode Welcome once.
    let programmes = sim::research::visible_ids()
        .filter_map(|id| {
            let p = sim::research::programme(id)?;
            let state = if rs.has(id) {
                "completed"
            } else if rs.active.as_deref() == Some(id) {
                "active"
            } else if rs.queue.iter().any(|q| q == id) {
                "queued"
            } else if sim::research::is_available(id, rs, &metric, now) {
                "available"
            } else {
                "locked"
            };
            let gate = if state == "locked" {
                gate_progress(p, rs, &metric, now)
            } else {
                None
            };
            Some(ProgrammeDynView {
                id: id.to_string(),
                state: state.to_string(),
                gate,
            })
        })
        .collect();

    ResearchView {
        active,
        queue: rs.queue.clone(),
        rate,
        stalled: rs.stalled,
        academies,
        programmes,
    }
}

/// Run the authoritative loop until all [`GameHandle`]s are dropped.
pub async fn run(
    world: World,
    persistence: PersistenceHandle,
    snapshot_every: u64,
    status_tx: watch::Sender<ServerStatus>,
    mut rx: mpsc::UnboundedReceiver<GameInput>,
) {
    // Off-thread engagement-estimate rollouts report completion here so the loop
    // can clear the connection's in-flight flag (unbounded, but each in-flight
    // estimate emits exactly one id, and there is at most one per connection).
    let (estimate_done_tx, mut estimate_done_rx) = mpsc::unbounded_channel::<ConnId>();
    let mut game = GameLoop::new(
        world,
        persistence,
        snapshot_every,
        status_tx,
        estimate_done_tx,
    );

    let mut ticker = interval(Duration::from_secs_f64(DT));
    // If we ever fall behind, skip missed ticks rather than bursting to catch
    // up (avoids a death spiral). Sim time tracks completed ticks regardless.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(tick_hz = TICK_HZ, "authoritative game loop started");

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                game.tick();
            }
            maybe_input = rx.recv() => {
                match maybe_input {
                    Some(input) => game.handle_input(input),
                    // All senders dropped: nothing can ever drive the game again.
                    None => break,
                }
            }
            // A blocking estimate rollout finished — free its connection to
            // request another. (`estimate_done_tx` also lives in `game`, so this
            // channel never closes on its own; the loop exits via `rx` above.)
            Some(conn_id) = estimate_done_rx.recv() => {
                game.estimate_inflight.remove(&conn_id);
            }
        }
    }

    info!("authoritative game loop stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Vec2;

    #[test]
    fn command_signal_plan_ends_at_the_inbound_ghosts_meeting_point() {
        let cc = Vec2::ZERO;
        let ghost_pos = Vec2::new(20_000.0, 0.0);
        let ghost_vel = Vec2::new(-100.0, 0.0);
        let plan = command_signal_plan(400.0, cc, ghost_pos, ghost_vel);
        let expected = ghost_pos + ghost_vel * plan.travel_time;
        assert!(plan.meeting_point.distance(expected) < 1e-6);
        assert!((sim::transit::delay(cc, expected, 400.0) - plan.travel_time).abs() <= sim::DT);
        assert!(plan.meeting_point.x < ghost_pos.x);
        assert!(plan.hops.is_empty());
    }

    #[test]
    fn outbound_signal_meeting_is_later_than_the_static_sighting() {
        let cc = Vec2::ZERO;
        let ghost_pos = Vec2::new(20_000.0, 0.0);
        let ghost_vel = Vec2::new(100.0, 0.0);
        let plan = command_signal_plan(400.0, cc, ghost_pos, ghost_vel);
        assert!(plan.travel_time > sim::transit::delay(cc, ghost_pos, 400.0));
        assert!(plan.meeting_point.x > ghost_pos.x);
    }

    #[test]
    fn an_expired_estimate_reads_presumed_never_confirmed() {
        let mut world = World::new(sim::SimConfig::for_players(0xE571, 4));
        let owner = PlayerId(881);
        world.step(&[Command::AddPlayer {
            id: owner,
            name: "Presumption".into(),
        }]);
        let fleet = *world
            .fleets
            .iter()
            .find(|(_, fleet)| fleet.owner == owner)
            .unwrap()
            .0;
        let pos = world.players[&owner].command_center + Vec2::new(50_000.0, 0.0);
        {
            let ship = world.fleets.get_mut(&fleet).unwrap();
            ship.pos = pos;
            ship.vel = Vec2::ZERO;
            ship.supplied = true;
        }
        world.step(&[Command::MoveShip {
            player_id: owner,
            ship_id: fleet,
            dest: world.players[&owner].command_center,
        }]);
        let pending = world.pending_commands(owner)[0];
        let mut timer_confirmation = false;
        while world.time <= pending.echo_at + sim::DT {
            timer_confirmation |= world.step(&[]).iter().any(|event| {
                matches!(event.payload, sim::EventPayload::OrderConfirmed { id, .. } if id == pending.id)
            });
        }
        assert!(
            !timer_confirmation,
            "an expired ordinary estimate is not evidence"
        );
        assert!(
            world
                .pending_commands(owner)
                .iter()
                .any(|order| order.id == pending.id),
            "the ordinary lifecycle remains presumed until a compliance-era map sample is served",
        );
        let plans = HashMap::from([(
            (owner, pending.id),
            ObservedOrderPlan {
                arrives_at: world.time - 2.0,
                response_at: world.time - 1.0,
                intent_path: Vec::new(),
            },
        )]);
        let rows = pending_order_views(&world, owner, &plans, world.time + 100.0);
        assert_eq!(
            rows.len(),
            1,
            "an expired estimate remains an unconfirmed lifecycle"
        );
        assert!(
            !rows[0].lost,
            "only arrived evidence or disclosed destruction may make it terminal"
        );
    }

    #[test]
    fn two_orders_to_one_fleet_are_reported_until_their_existing_expiry() {
        let mut world = World::new(sim::SimConfig::for_players(0x0DDE_2, 4));
        let owner = PlayerId(700);
        world.step(&[Command::AddPlayer {
            id: owner,
            name: "Queue Test".into(),
        }]);
        let cc = world.players[&owner].command_center;
        let fleet = *world
            .fleets
            .iter()
            .find(|(_, f)| f.owner == owner)
            .map(|(id, _)| id)
            .expect("the player starts with a fleet");
        let pos = cc + Vec2::new(120_000.0, 40_000.0);
        {
            let f = world.fleets.get_mut(&fleet).unwrap();
            f.pos = pos;
            f.vel = Vec2::ZERO;
            f.order = sim::ship::FleetOrder::Idle;
        }

        let first_dest = pos + Vec2::new(5_000.0, 0.0);
        let first_events = world.step(&[Command::MoveShip {
            player_id: owner,
            ship_id: fleet,
            dest: first_dest,
        }]);
        let first_id = first_events
            .iter()
            .find_map(|e| match &e.payload {
                sim::EventPayload::OrderScheduled { id, fleet: f, .. } if *f == fleet => Some(*id),
                _ => None,
            })
            .expect("the first validated order has an id");

        for _ in 0..(5 * sim::TICK_HZ) {
            world.step(&[]);
        }
        let second_dest = pos + Vec2::new(0.0, 9_000.0);
        let second_events = world.step(&[Command::MoveShip {
            player_id: owner,
            ship_id: fleet,
            dest: second_dest,
        }]);
        let second_id = second_events
            .iter()
            .find_map(|e| match &e.payload {
                sim::EventPayload::OrderScheduled { id, fleet: f, .. } if *f == fleet => Some(*id),
                _ => None,
            })
            .expect("the second validated order has an id");

        let queue = world.pending_commands(owner);
        assert_eq!(queue.len(), 2, "both outbound orders are reported");
        assert_ne!(
            first_id, second_id,
            "each scheduled command has a stable distinct id"
        );
        assert_eq!(
            queue.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![first_id, second_id]
        );
        assert_eq!(queue[0].dest, Some(first_dest));
        assert_eq!(queue[1].dest, Some(second_dest));
        for p in &queue {
            assert!(p.delivered_at > p.issued_at, "outbound clock follows issue");
            assert!(p.echo_at > p.delivered_at, "echo clock follows delivery");
        }

        let first_delivery = queue[0].delivered_at;
        let second_delivery = queue[1].delivered_at;
        while world.time < first_delivery + sim::DT {
            world.step(&[]);
        }
        assert_eq!(
            world.pending_commands(owner).len(),
            2,
            "the delivered first order and outbound second order coexist",
        );

        while world.time < second_delivery + sim::DT {
            world.step(&[]);
        }
        let after_supersession = world.pending_commands(owner);
        assert_eq!(
            after_supersession.len(),
            1,
            "the existing supersession expiry removes the older echo"
        );
        assert_eq!(after_supersession[0].id, second_id);

        let echo = after_supersession[0].echo_at;
        while world.time < echo + sim::DT {
            world.step(&[]);
        }
        assert_eq!(
            world.pending_commands(owner).len(),
            1,
            "the final row survives an expired estimate until the map serves compliance",
        );
        let confirmed = world.confirm_orders_from_served(
            owner,
            &[(fleet, after_supersession[0].delivered_at, false)],
        );
        assert_eq!(confirmed.len(), 1);
        assert!(
            world.pending_commands(owner).is_empty(),
            "served compliance retires the final row"
        );
    }

    /// The build CATALOGUE must offer every hull a corporation can actually
    /// build, and every structure. Structures ride `StructureKind::ALL` and so
    /// come along for free; SHIPS are hand-listed, and that list is exactly the
    /// kind of thing that silently rots — a hull the sim knows how to build but
    /// the catalogue never mentions is simply unbuildable, with no error
    /// anywhere to say so. (This test was written because the Troop Transport
    /// shipped that way and only a live client check caught it.)
    #[test]
    fn every_buildable_hull_and_structure_is_offered() {
        let opts = build_options();
        let keys: std::collections::BTreeSet<&str> = opts.iter().map(|o| o.key.as_str()).collect();

        // The hull slug is whatever the WIRE calls it — derived from serde, not
        // retyped here, so the catalogue key and the protocol can't drift apart.
        let slug_of = |k: sim::ShipKind| {
            serde_json::to_value(k)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        };
        for k in sim::ALL_SHIP_KINDS {
            let slug = slug_of(k);
            // The Authority's freighter is the ONE hull no corporation may lay.
            if k == sim::ShipKind::Freighter {
                assert!(
                    !keys.contains(slug.as_str()),
                    "the Authority's carrier must never be offered"
                );
                continue;
            }
            assert!(
                keys.contains(slug.as_str()),
                "hull `{slug}` is buildable in the sim but missing from the catalogue — it is unreachable from the UI",
            );
        }
        for k in sim::StructureKind::ALL {
            assert!(
                keys.contains(k.slug()),
                "structure `{}` is missing from the catalogue",
                k.slug()
            );
        }
        // And every offer must price out — a key with no recipe would render a
        // build button that can never be paid for.
        assert!(
            opts.iter().all(|o| !o.costs.is_empty()),
            "every build option carries a cost"
        );
    }

    fn concluded(started_at: f64, ended_at: f64, pos: Vec2) -> ConcludedBattle {
        ConcludedBattle {
            id: sim::EntityId(1),
            pos,
            started_at,
            ended_at,
            a_owner: PlayerId(1),
            d_owner: PlayerId(2),
            participants: vec![sim::EntityId(10), sim::EntityId(11)],
        }
    }

    /// The in-progress icon of a concluded battle lingers until the CONCLUSION's
    /// warp light arrives — `ended_at + transit::delay(pos, cc)` — exactly when the
    /// per-participant aftermath report lands (`ReportScheduler::due_for` gates on
    /// same report wavefront arrives (`event_time == ended_at`). So the icon flips to
    /// aftermath on ONE wavefront: no FTL early-vanish, no gap where the suppressed
    /// participant fleets re-appear. (The bug: the icon used to vanish at true
    /// `ended_at`, `delay` seconds before the aftermath, exposing the stale fleets.)
    #[test]
    fn concluded_icon_lingers_until_conclusion_light_matches_aftermath() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        // Battle 6000 su away → 4 s of warp light each way. It ran t=100..140.
        let pos = Vec2::new(6000.0, 0.0);
        let (started_at, ended_at) = (100.0, 140.0);
        let cb = concluded(started_at, ended_at, pos);
        let delay = sim::transit::delay(pos, cc, c); // 4 s
        let aftermath_arrival = ended_at + delay; // when due_for delivers it

        // Just after the conclusion's light for the START has been seen but the
        // conclusion's light has NOT yet arrived: the in-progress icon still shows
        // (this is the window where the fleets used to wrongly re-appear).
        assert!(
            cb.shows_in_progress(cc, c, ended_at + 1.0),
            "icon must persist through the light-in-flight gap"
        );
        assert!(
            cb.shows_in_progress(cc, c, aftermath_arrival - 0.001),
            "still in progress an instant before the aftermath"
        );

        // At the aftermath's arrival the icon is gone (strict upper bound) — the
        // aftermath (delivered on `arrival <= now`) takes over on the same instant.
        assert!(
            !cb.shows_in_progress(cc, c, aftermath_arrival),
            "icon flips off exactly as the aftermath lands"
        );
        assert!(
            !cb.shows_in_progress(cc, c, aftermath_arrival + 5.0),
            "and stays off after"
        );
    }

    /// The linger is per-viewer and light-honest: a FAR command center keeps the
    /// in-progress icon longer than a NEAR one, because its conclusion light takes
    /// longer to arrive — never a global FTL flip.
    #[test]
    fn linger_is_per_viewer_light_delayed() {
        let c = 300.0;
        let pos = Vec2::new(0.0, 0.0);
        let cb = concluded(0.0, 40.0, pos);
        let near = Vec2::new(300.0, 0.0); // 0.2 s of warp light
        let far = Vec2::new(9000.0, 0.0); // 6 s of warp light

        // 41 s after start (1 s after true end): near viewer's conclusion light has
        // arrived (icon gone); the far viewer's has not (icon still shown).
        assert!(
            !cb.shows_in_progress(near, c, 41.0),
            "near viewer already saw the conclusion"
        );
        assert!(
            cb.shows_in_progress(far, c, 41.0),
            "far viewer's conclusion light is still in flight"
        );
    }

    /// A viewer whose START light never arrived before the battle ended (it began
    /// and ended faster than its light could reach them) must NEVER see a phantom
    /// in-progress icon — the lower bound guards against conjuring one late.
    #[test]
    fn no_phantom_icon_when_start_light_never_arrived() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        // 6000 su away (4 s of warp light) but the battle lasted only 1 s (t=0..1).
        let cb = concluded(0.0, 1.0, Vec2::new(6000.0, 0.0));
        // The visible window is [started_at + delay, ended_at + delay) = [4, 5):
        // one honest second, shifted whole by warp-light delay.
        assert!(
            !cb.shows_in_progress(cc, c, 3.999),
            "no icon before the start light arrives"
        );
        assert!(cb.shows_in_progress(cc, c, 4.5), "the honest 1 s sighting");
        assert!(
            !cb.shows_in_progress(cc, c, 5.0),
            "gone once the conclusion light arrives"
        );
    }
}
