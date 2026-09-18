//! The authoritative game loop — the heartbeat of the server (§14).
//!
//! A single Tokio task owns the [`World`] and the [`Sessions`] registry. Because
//! nothing else can touch them, there are no locks and no data races on game
//! state. The loop:
//!   1. ticks the fixed-step world at [`TICK_HZ`] × the runtime pacing scale;
//!   2. folds player intents / session events into sim commands at tick
//!      boundaries;
//!   3. pushes every connection its own per-player message (M1: the live tick;
//!      from M3: the delayed/fogged view);
//!   4. checkpoints the complete galaxy every 15 minutes and on clean shutdown.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, info};

use sim::{Command, DT, PlayerId, TICK_HZ, World};

mod bots;
mod durability;
pub(crate) use durability::{GalaxyCheckpoint, run};
use crate::protocol::{
    BuildOptionView, ClientMsg, GalaxyInfo, InvSlot, MarketView, NebulaInfo, OrderView, PriceView,
    ServerMsg,
    StockSlot, SystemInfo, WalletView,
};
use crate::reports::ReportScheduler;
use crate::session::{ConnId, ConnInfo, GameInput, ServerStatus, Sessions};
use crate::timeline::Timeline;
use crate::transactions::{PendingTransaction, TransactionDetails, TransactionHistory};
use crate::view::{self, PositionHistory, PriceHistory};

/// Push a per-player message every N sim ticks. At 30 Hz, N=3 → ~10 Hz network
/// updates: visibly live without flooding the socket.
const BROADCAST_EVERY: u64 = 3;

/// Standard wall pacing: one simulation second per real second. Accelerated
/// playtests remain opt-in with `SIM_PACING=4`; gameplay constants stay unchanged.
pub const DEFAULT_SIM_PACING: f64 = 1.0;

fn broadcast_every_for(pacing_scale: f64) -> u64 {
    (BROADCAST_EVERY as f64 * pacing_scale).round().max(1.0) as u64
}

/// Full checkpoints are wall-time bookkeeping, independent of playtest pacing.
pub const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(15 * 60);

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Commands addressed to the fixed Market Hub do not own a fleet-order
/// lifecycle, but still need the outbound map grammar. Every fleet-targeted
/// command is omitted: the sim queues it and its accepted `OrderScheduled`
/// event emits the lifecycle-linked `CommandSignal` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchChevronTarget {
    MarketHub,
}

fn dispatch_chevron_targets(msg: &ClientMsg) -> Vec<DispatchChevronTarget> {
    use DispatchChevronTarget::MarketHub;

    match msg {
        ClientMsg::MarketBuy { .. }
        | ClientMsg::MarketSell { .. }
        | ClientMsg::BookFreightOut { .. }
        | ClientMsg::BookFreightIn { .. }
        | ClientMsg::RequestFuelRescue { .. }
        | ClientMsg::PayReinstatement { .. }
        | ClientMsg::PlaceLimitOrder { .. }
        | ClientMsg::CancelLimitOrder { .. }
        | ClientMsg::StockSystem { .. }
        | ClientMsg::HireSpecialist { .. }
        | ClientMsg::BuyModule { .. }
        | ClientMsg::SellModule { .. } => vec![MarketHub],
        _ => Vec::new(),
    }
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
                configuration: pending.configuration,
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct MarketAccountSample {
    standing: f64,
    credits: f64,
    valuation: f64,
    warehouse: BTreeMap<sim::Commodity, u32>,
    orders: Vec<sim::market::LimitOrder>,
}

/// Change-compressed hub-account history. Recording only when truth changes
/// keeps the cost proportional to economic events rather than tick count, while
/// retaining one sample before the light horizon makes every delayed lookup
/// total across quiet periods.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
                standing: corp.tca_standing,
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
        let end = history.partition_point(|(at, _)| *at <= target);
        end.checked_sub(1).map(|i| &history[i].1)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ScheduledTradeReport {
    arrives_at: f64,
    #[serde(default)]
    trade: Option<sim::TradeEvent>,
    #[serde(default)]
    transaction: Option<PendingTransaction>,
}

/// Where the fact represented by an economy receipt physically occurred. Most
/// Exchange administration originates at the Market Hub; local loading,
/// delivery and automation originates at the named system. This position is the
/// one source for both live receipt delay and the retained check-in timeline.
pub(crate) fn trade_report_origin(world: &World, trade: &sim::TradeEvent) -> sim::Vec2 {
    trade.physical_origin(world)
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ReportingCheckpoint {
    #[serde(default)]
    galaxy_instance_id: Option<String>,
    history: PositionHistory,
    prices: PriceHistory,
    market_accounts: MarketAccountHistory,
    trade_reports: Vec<ScheduledTradeReport>,
    #[serde(default)]
    transactions: TransactionHistory,
    reports: ReportScheduler,
    timeline: Timeline,
    concluded_battles: Vec<ConcludedBattle>,
    observed_order_plans: Vec<(PlayerId, u64, ObservedOrderPlan)>,
}

struct GameLoop {
    world: World,
    bots: BTreeMap<PlayerId, bots::Bot>,
    /// UI-history namespace, outside deterministic simulation state. A fresh
    /// game (even with the same seed) cannot inherit another game's dismissals.
    galaxy_instance_id: String,
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
    transactions: TransactionHistory,
    /// Delayed delivery of discrete reports (raid outcomes) — each player learns
    /// them on their own clock (§8).
    reports: ReportScheduler,
    /// Per-player retained check-in timeline (§16, Layer 3) — what became
    /// observable, buffered across disconnects, for the "welcome back" digest.
    timeline: Timeline,
    /// Last journal revision sent; the retained length eventually stops growing.
    timeline_sent: HashMap<PlayerId, usize>,
    /// Battles that have concluded but whose conclusion light is still in flight
    /// to some viewer — kept so the in-progress icon lingers until the aftermath
    /// lands (see [`ConcludedBattle`]). Ephemeral awareness state, like `reports`.
    concluded_battles: Vec<ConcludedBattle>,
    /// §emplacements: demolished structures still visible on old light.
    /// Commands accumulated since the last tick, applied at the next boundary.
    pending: Vec<Command>,
    /// Owner-only order clocks solved once from the served ghost at issue time.
    /// The sim's authoritative delivery/echo stamps never enter a player View.
    observed_order_plans: HashMap<(PlayerId, u64), ObservedOrderPlan>,
    /// Sim seconds advanced per wall second. This changes only scheduling: the
    /// pure world still advances in fixed `DT` steps, preserving every ratio.
    pacing_scale: f64,
    /// Keep filtered network Views near 10 Hz in wall time even while ticks run
    /// faster, otherwise 4× pacing would also multiply the expensive view load.
    broadcast_every: u64,
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
    #[cfg(test)]
    fn checkpoint_snapshot(&self) -> serde_json::Value {
        let reporting = serde_json::json!({
            "galaxy_instance_id": &self.galaxy_instance_id,
            "history": &self.history, "prices": &self.prices,
            "market_accounts": &self.market_accounts,
            "trade_reports": &self.trade_reports, "reports": &self.reports,
            "transactions": &self.transactions,
            "timeline": &self.timeline, "concluded_battles": &self.concluded_battles,
            "observed_order_plans": self.observed_order_plans.iter()
                .map(|((player, id), plan)| (player, id, plan)).collect::<Vec<_>>(),
        });
        let mut snapshot = serde_json::to_value(&self.world).unwrap();
        snapshot["reporting_checkpoint"] = serde_json::Value::String(reporting.to_string());
        snapshot
    }

    fn new(
        mut world: World,
        pacing_scale: f64,
        status_tx: watch::Sender<ServerStatus>,
        estimate_done_tx: mpsc::UnboundedSender<ConnId>,
    ) -> Self {
        let restored = world.reporting_checkpoint.take().and_then(|saved| {
            match serde_json::from_str::<ReportingCheckpoint>(&saved) {
                Ok(state) => Some(state),
                Err(error) => {
                    tracing::warn!(%error, "reporting checkpoint unreadable; waiting for fresh light");
                    None
                }
            }
        });
        let history = PositionHistory::for_world(&world);
        let prices = PriceHistory::for_world(&world);
        let mut market_accounts = MarketAccountHistory::for_world(&world);
        market_accounts.record(&world);
        let mut game = GameLoop {
            galaxy_instance_id: restored.as_ref()
                .and_then(|saved| saved.galaxy_instance_id.as_ref())
                .filter(|id| !id.is_empty()).cloned()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            world,
            bots: BTreeMap::new(),
            sessions: Sessions::new(),
            history,
            prices,
            market_accounts,
            trade_reports: Vec::new(),
            transactions: TransactionHistory::default(),
            reports: ReportScheduler::new(),
            timeline: Timeline::new(),
            timeline_sent: HashMap::new(),
            concluded_battles: Vec::new(),
            pending: Vec::new(),
            observed_order_plans: HashMap::new(),
            pacing_scale,
            broadcast_every: broadcast_every_for(pacing_scale),
            status_tx,
            estimate_inflight: HashSet::new(),
            estimate_done_tx,
        };
        if let Some(saved) = restored {
            game.history = saved.history;
            game.prices = saved.prices;
            game.market_accounts = saved.market_accounts;
            game.trade_reports = saved.trade_reports;
            game.transactions = saved.transactions;
            game.reports = saved.reports;
            game.timeline = saved.timeline;
            game.concluded_battles = saved.concluded_battles;
            game.observed_order_plans = saved.observed_order_plans.into_iter()
                .map(|(player, id, plan)| ((player, id), plan)).collect();
        }
        game.transactions.initialize(&game.world, &game.timeline);
        game
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

    /// Emit the violet outbound chevron for fixed-Hub instructions which do not
    /// own a pending fleet-order lifecycle.
    fn emit_dispatch_chevron(
        &mut self,
        player_id: PlayerId,
        target: DispatchChevronTarget,
        depart_time: f64,
    ) {
        let Some(corp) = self.world.players.get(&player_id) else {
            return;
        };
        let cc = corp.command_center;
        let c = self.world.config.c;
        let DispatchChevronTarget::MarketHub = target;
        let target_pos = self.world.hub;
        let travel_time = sim::transit::delay(cc, target_pos, c);
        self.sessions.send_to_player(
            player_id,
            ServerMsg::CommandChevron {
                fleet_id: None,
                target_pos,
                depart_time,
                arrive_time: depart_time + travel_time,
            },
        );
    }

    /// Publish current session/ops status (cheap; replaces the watched value).
    fn publish_status(&self) {
        let _ = self.status_tx.send(ServerStatus {
            online_players: self.sessions.online_player_count(),
            connections: self.sessions.connection_count(),
            bot_players: self.bots.len(),
            bot_commands: self.bots.values().map(|bot| bot.commands_sent).sum(),
            corporations: self.world.players.len(),
            galaxy_systems: self.world.systems.len(),
            tick: self.world.tick,
            sim_time: self.world.time,
        });
    }

    fn schedule_trade_reports(&mut self, events: &[sim::Event]) {
        for event in events {
            let Some(transaction) = PendingTransaction::from_event(event) else {
                continue;
            };
            let Some(corp) = self.world.players.get(&transaction.owner) else {
                continue;
            };
            let origin = event.origin.or_else(|| event.physical_origin(&self.world))
                .expect("market transaction has a physical origin");
            let delay = sim::transit::delay(origin, corp.command_center, self.world.config.c);
            self.trade_reports.push(ScheduledTradeReport {
                arrives_at: event.time + delay,
                trade: match event.payload { sim::EventPayload::Trade(trade) => Some(trade), _ => None },
                transaction: Some(transaction),
            });
        }
    }

    fn deliver_trade_reports(&mut self) {
        let now = self.world.time;
        let mut waiting = Vec::with_capacity(self.trade_reports.len());
        for report in self.trade_reports.drain(..) {
            if report.arrives_at <= now + 1e-9 {
                // One wavefront: append even while offline, but ONLY when the
                // original receipt arrives. History queries never price truth.
                // Legacy pending receipts did not retain their emission time.
                let details = report.transaction.map(|entry|
                    (entry.owner, Some(entry.occurred_at), entry.details))
                    .or_else(|| report.trade.map(|trade|
                        (trade.player(), None, TransactionDetails::Trade { trade })));
                if let Some((owner, occurred_at, details)) = details {
                    let entry = self.transactions.record(owner, occurred_at, report.arrives_at, details);
                    self.sessions.send_to_player(owner, ServerMsg::TransactionRecorded { player_id: owner, entry });
                }
                if let Some(trade) = report.trade {
                    self.sessions.send_to_player(trade.player(), ServerMsg::Trade { trade });
                }
            } else {
                waiting.push(report);
            }
        }
        self.trade_reports = waiting;
    }

    fn corporation_named(&self, name: &str) -> PlayerId {
        // Names remain public diplomacy addresses, not account identities.
        // Keep unknown-name refusals on the ordinary delayed sim path. Zero
        // is never allocated to a PostgreSQL account.
        let key = name.trim().to_lowercase();
        self.world.players.iter().find(|(_, corporation)| corporation.name.to_lowercase() == key)
            .map_or(PlayerId(0), |(id, _)| *id)
    }

    fn handle_input(&mut self, input: GameInput) {
        match input {
            GameInput::Connect {
                conn_id,
                player_id,
                name,
                outbound,
                view_tx,
                replace_tx,
                view_divisor,
            } => {
                // A legacy name-only corporation is NOT claimable by registering
                // its public name. Migration requires an explicit operator binding.
                if self.world.players.iter().any(|(id, corp)| *id != player_id
                    && corp.name.to_lowercase() == name.to_lowercase()) {
                    let _ = outbound.try_send(ServerMsg::Error {
                        message: "This galaxy has an older corporation with that name. Contact the operator to migrate it.".into(),
                    });
                    return;
                }
                let inserted = self.sessions.insert(
                    conn_id,
                    ConnInfo {
                        player_id,
                        name: name.clone(),
                        outbound,
                        view_tx,
                        replace_tx,
                        view_divisor,
                        last_view_broadcast: None,
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
                        pacing_scale: self.pacing_scale,
                        tick: self.world.tick,
                        sim_time: self.world.time,
                        galaxy: GalaxyInfo {
                            instance_id: self.galaxy_instance_id.clone(),
                            hub: self.world.hub,
                            radius: self.world.config.galaxy_radius,
                            nebulas: nebula_infos(&self.world),
                            c: self.world.config.c,
                            jump_range: sim::transit::JUMP_RANGE,
                            jump_spool_s: sim::transit::JUMP_SPOOL_S,
                            hyperlimit: sim::transit::HYPERLIMIT,
                            sensor_range: self.world.config.sensor_range,
                            raider_speed: sim::ShipKind::Raider.max_speed(),
                            // Array-bubble tunables so the client renders its own
                            // arrays' coverage (§buildings step 2b).
                            // Scout multiplier on a Raider-bearing sensor fleet.
                            scout_sensor_mult: sim::ship::SCOUT_SENSOR_MULT,
                            convoy_sensor_mult: sim::ship::CONVOY_SENSOR_MULT,
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
                            migrant_cohort_people: sim::migration::MIGRANT_COHORT_PEOPLE,
                            migration_base_interval_s: sim::migration::MIGRATION_BASE_INTERVAL_S,
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
                            industry_catalog: sim::industry::catalog(),
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
                self.timeline_sent.insert(player_id, self.timeline.revision(player_id));
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
                    %player_id, conn_id,
                    newly_online = inserted.newly_online,
                    replaced_conn = ?inserted.replaced_conn,
                    view_hz = 10 / view_divisor,
                    online_players = self.sessions.online_player_count(),
                    connections = self.sessions.connection_count(),
                    "player connected"
                );
            }
            GameInput::Disconnect { conn_id } => {
                if let Some(player_id) = self.sessions.remove(conn_id) {
                    info!(
                        %player_id, conn_id,
                        online_players = self.sessions.online_player_count(),
                        "player disconnected"
                    );
                }
            }
            GameInput::Intent { conn_id, msg } => match {
                // One choke point for fixed-Hub command feedback. Every command
                // addressed to a fleet now emits its lifecycle-linked signal
                // only after the sim accepts it into the delayed order queue.
                if let Some(player_id) = self.sessions.player_of(conn_id) {
                    for target in dispatch_chevron_targets(&msg) {
                        self.emit_dispatch_chevron(player_id, target, self.world.time);
                    }
                }
                msg
            } {
                ClientMsg::Ping => {
                    debug!(conn_id, "ping");
                }
                ClientMsg::RequestTransactions { before, request_id } => {
                    // Reading the CC's already-received records is local, not a
                    // new hub order. Page size is bounded; no per-View history.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        let (entries, next_before) = self.transactions.page(player_id, before);
                        self.sessions.send_to_conn(conn_id, ServerMsg::Transactions {
                            player_id, request_id, before, entries, next_before,
                            since: self.transactions.since(),
                        });
                    }
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
                ClientMsg::HoldFleet { ship_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HoldFleet { player_id, ship_id });
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
                ClientMsg::GuardFleet {
                    interceptor_id,
                    target_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::GuardFleet {
                            player_id,
                            interceptor_id,
                            target_id,
                        });
                    }
                }
                ClientMsg::RefuelFleet { fleet_id, target_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RefuelFleet { player_id, fleet_id, target_id });
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
                ClientMsg::DefendSystem { fleet_id, system_id, pursuit_radius } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DefendSystem { player_id, fleet_id, system_id, pursuit_radius });
                    }
                }
                ClientMsg::ExploreSite { fleet_id, site_id, task } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ExploreSite { player_id, fleet_id, site_id, task });
                    }
                }
                ClientMsg::AnnotateExploration { entry } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AnnotateExploration { player_id, entry });
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
                ClientMsg::SetFleetMission { fleet_id, mission } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetMission { player_id, fleet_id, mission });
                    }
                }
                ClientMsg::SetFreightRoute { fleet_id, route } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::SetFreightRoute { player_id, fleet_id, route }); }
                }
                ClientMsg::DeployOutpost { fleet_id, system_id, body_id, outpost, commodity } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::DeployOutpost { player_id, fleet_id, system_id, body_id, outpost, commodity }); }
                }
                ClientMsg::SkimFuel { fleet_id, system_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::SkimFuel { player_id, fleet_id, system_id }); }
                }
                ClientMsg::ReserveProject { system_id, target, reserve } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::ReserveProject { player_id, system_id, target, reserve }); }
                }
                ClientMsg::StartColonyProject { system_id, body_id, project, commodity } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::StartColonyProject { player_id, system_id, body_id, project, commodity }); }
                }
                ClientMsg::SetColonyProjectActive { system_id, project, active } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) { self.pending.push(Command::SetColonyProjectActive { player_id, system_id, project, active }); }
                }
                ClientMsg::SetFleetPosture { fleet_id, posture } => {
                    // Fleet-local policy is delivered through the ordinary order queue.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetPosture {
                            player_id,
                            fleet_id,
                            posture,
                        });
                    }
                }
                ClientMsg::RecruitCaptain { system_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RecruitCaptain {
                            player_id,
                            system_id,
                        });
                    }
                }
                ClientMsg::AssignCaptain {
                    captain_id,
                    fleet_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AssignCaptain {
                            player_id,
                            captain_id,
                            fleet_id,
                        });
                    }
                }
                ClientMsg::ReserveCaptain { captain_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ReserveCaptain {
                            player_id,
                            captain_id,
                        });
                    }
                }
                ClientMsg::TrainCaptain {
                    captain_id,
                    attribute,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TrainCaptain {
                            player_id,
                            captain_id,
                            attribute,
                        });
                    }
                }
                // Alliance administration travels to the syndicate's headquarters.
                ClientMsg::CreateSyndicate { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending
                            .push(Command::CreateSyndicate { player_id, name });
                    }
                }
                ClientMsg::InviteToSyndicate { name } => {
                    // Resolve the public corporation address to its durable ID.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        let invitee = self.corporation_named(&name);
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
                ClientMsg::SetSyndicateRole { member, role } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetSyndicateRole {
                            player_id,
                            member,
                            role,
                        });
                    }
                }
                ClientMsg::AcceptOperation { operation_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AcceptOperation {
                            player_id,
                            operation_id,
                        });
                    }
                }
                ClientMsg::AbandonOperation { operation_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AbandonOperation {
                            player_id,
                            operation_id,
                        });
                    }
                }
                ClientMsg::AssignOperationFleet {
                    operation_id,
                    fleet_id,
                    protected_fleet,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AssignOperationFleet {
                            player_id,
                            operation_id,
                            fleet_id,
                            protected_fleet,
                        });
                    }
                }
                ClientMsg::RecoverOperation {
                    operation_id,
                    fleet_id,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RecoverOperation {
                            player_id,
                            operation_id,
                            fleet_id,
                        });
                    }
                }
                ClientMsg::ContributeOperationCargo {
                    operation_id,
                    commodity,
                    units,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ContributeOperationCargo {
                            player_id,
                            operation_id,
                            commodity,
                            units,
                        });
                    }
                }
                ClientMsg::CreateSyndicateOperation { system_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::CreateSyndicateOperation {
                            player_id,
                            system_id,
                        });
                    }
                }
                ClientMsg::ProposeTreaty {
                    target_name,
                    treaty,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ProposeTreaty {
                            player_id,
                            target: self.corporation_named(&target_name),
                            treaty,
                        });
                    }
                }
                ClientMsg::RespondTreaty {
                    proposal_id,
                    accept,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RespondTreaty {
                            player_id,
                            proposal_id,
                            accept,
                        });
                    }
                }
                ClientMsg::DeclareWar { target_name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DeclareWar {
                            player_id,
                            target: self.corporation_named(&target_name),
                        });
                    }
                }
                ClientMsg::CancelTreaty { target } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::CancelTreaty { player_id, target });
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
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketBuy {
                            player_id,
                            commodity,
                            units,
                            max_unit_price,
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
                ClientMsg::HaulToSystem { fleet_id, system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HaulToSystem {
                            player_id,
                            fleet_id,
                            system,
                        });
                    }
                }
                ClientMsg::RequestFuelRescue { fleet_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RequestFuelRescue {
                            player_id,
                            fleet_id,
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
                    refining_ore,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetAssignment {
                            player_id,
                            system_id,
                            structure,
                            workers,
                            specialists,
                            body_id,
                            refining_ore,
                        });
                    }
                }
                ClientMsg::SetMigrationPolicy {
                    system_id,
                    body_id,
                    policy,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetMigrationPolicy {
                            player_id,
                            system_id,
                            body_id,
                            policy,
                        });
                    }
                }
                ClientMsg::RelocateMigrants {
                    from_system,
                    from_body,
                    to_system,
                    to_body,
                } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RelocateMigrants {
                            player_id,
                            from_system,
                            from_body,
                            to_system,
                            to_body,
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
                        let arrays = self.world.known_sensor_sources(player_id);
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

        self.think_bots();
        if self.world.tick.is_multiple_of(self.broadcast_every) {
            self.broadcast();
            self.publish_status();
        }

    }

    /// Push every connection its own per-player delayed/fogged view, each
    /// computed from THAT player's command center (§6, §14). No player ever
    /// receives true positions or another player's view — the fairness
    /// guarantee, enforced by [`PositionHistory::view_for`].
    fn broadcast(&mut self) {
        // `broadcast` itself remains the canonical 10 Hz opportunity. The sole
        // connection for each corporation decides whether this opportunity is
        // due (desktop every one, mobile every two), so expensive fog/light
        // construction scales with frames actually delivered rather than with
        // accumulated darkness or a hidden parallel client.
        let broadcast_index = self.world.tick / self.broadcast_every;
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

        // Build each DUE player's view once, plus any delayed reports whose
        // light has now reached them. Everything is computed from THIS player's
        // command center and light-gated. A connection whose corporation isn't
        // in the world yet (AddPlayer not processed) simply stays due.
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
        for player_id in self.sessions.players_due_for_view(broadcast_index) {
            let Some(corp) = self.world.players.get(&player_id) else {
                continue;
            };
            let cc = corp.command_center;
            let known_systems = self.world.information.systems_for(&self.world, player_id);
            let known_emplacements = self.world.information.emplacements(cc, c, now);
            let known_syndicates = self.world.information.syndicates(cc, c, now);
            let known_ally = |owner| owner != player_id && known_syndicates.iter()
                .any(|s| s.members.contains(&player_id) && s.members.contains(&owner));
            let known_standing = self.world.information.standing(player_id, cc, c, now);
            // The viewer's standing SENSOR-ARRAY bubbles (§buildings step 2b) join
            // their coverage — same shared source of truth as the sim's pickets.
            let arrays = self.world.known_sensor_sources(player_id);
            // BATTLES (§battles-take-time), STRICTLY light-gated: a battle (and its
            // participants, revealed by weapons fire) appears only once the light
            // of its start has reached THIS player's command center. Subsequent
            // roster changes ride their OWN reports, not the opening wavefront.
            let mut battles: Vec<crate::protocol::BattleView> = Vec::new();
            let mut battle_reveal: std::collections::BTreeSet<sim::EntityId> =
                std::collections::BTreeSet::new();
            for b in self.world.active_battles() {
                let delay = sim::transit::delay(b.pos, cc, c);
                if now >= b.started_at + delay {
                    let participants = view::battle_participants_at(
                        self.world.battle_records.get(&b.id), &b.participants, now, delay);
                    battle_reveal.extend(participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: b.id,
                        pos: b.pos,
                        age: delay,
                        started_at: b.started_at,
                        own: player_id == b.a_owner || player_id == b.d_owner,
                        participants,
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
                    let delay = sim::transit::delay(cb.pos, cc, c);
                    let participants = view::battle_participants_at(
                        self.world.battle_records.get(&cb.id), &cb.participants, now, delay);
                    battle_reveal.extend(participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: cb.id,
                        pos: cb.pos,
                        age: delay,
                        started_at: cb.started_at,
                        own: player_id == cb.a_owner || player_id == cb.d_owner,
                        participants,
                    });
                }
            }
            // §node: this viewer's regional dark-fleet effects (Veil quiets its
            // holders' dark fleets; Deep Scan resolves exact composition in-region).
            let (veil_regions, deep_scan_regions) = self.world.known_node_regions(player_id);
            let picture = self.history.picture_for_with_arrays(
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
            let mut ghosts = picture.ghosts;
            let jump_departures = picture.jump_departures;
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
            let emplacements_view: Vec<crate::protocol::EmplacementView> = known_emplacements
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
            // The emplacement history retains the standing report until its
            // destruction light arrives; no separate truth-based add-back list.
            // Fleet-local settings already ride the history report. Only the
            // viewer's known diplomatic tint is attached here.
            for g in ghosts.iter_mut() {
                // Onboard facts already ride the same emission as g.pos.
                // §syndicates Part 1: friendly ALLY tint — the owner (already on
                // the ghost) is a syndicate member as THIS viewer knows it
                // (light-delayed membership; `known_ally` returns false for own).
                g.ally = known_ally(g.owner);
                // §TCA: an Authority freighter's MANIFEST is two-tier PER ENTRY —
                // your own lots are always yours to see (they're your property),
                // everyone else's only from inside sensor range (`revealed`, the
                // same Tier-2 gate that governs a convoy's cargo). A distant rival
                // sees the hull go by and learns nothing about who ships what.
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
                    battle_id: r.battle_id,
                    aftermath: r.aftermath.clone(),
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
                &known_standing
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
                    standing: known_standing,
                    standing_sig,
                    reports: battle_reports,
                    reports_sig,
                    captures: capture_reports,
                    captures_sig,
                    exploration_journal: self.world.exploration.journal_for(player_id),
                    exploration_journal_version: self.world.exploration.journal_version_for(player_id),
                },
            );
            let anchors = view::filter_anchors(&self.world.home_slots, player_id, cc, c, now);
            // §syndicates Part 2: each syndicate ally's relayable scout intel (their
            // command center is the relay source). The View chain-light-delays each
            // ally's snapshots to this viewer, provenance preserved.
            let own_intel = self.world.information.intel(player_id, cc, c, now);
            let relay_intel: Vec<_> = self.world.players.iter().filter(|(id, _)| known_ally(**id))
                .map(|(&id, ally)| (id, ally.command_center,
                    self.world.information.intel(id, ally.command_center, c,
                        now - sim::transit::delay(ally.command_center, cc, c))))
                .collect();
            let ally_intel: Vec<_> = relay_intel.iter().map(|(id, cc, intel)|
                view::AllyIntel { id: *id, cc: *cc, intel }).collect();
            // One arrived site report supplies ownership, stores, workers and
            // jobs together. Never splice current truth into the colony panel.
            let known_builds: Vec<_> = known_systems.iter().filter_map(|sys|
                self.world.information.site(sys.id, cc, c, now))
                .flat_map(|report| report.builds.iter().cloned()).collect();
            let mut systems = view::filter_systems(
                &known_systems,
                player_id,
                cc,
                c,
                now,
                &known_builds,
                self.world.tick,
                DT,
                &own_intel,
                &ally_intel,
                &corp.surveyed,
            );
            // §syndicates Part 1: friendly ALLY tint on systems whose (light-gated
            // known) owner is a syndicate member as THIS viewer knows it. Composes
            // both light-gates; grants no owner-only data (Part 1 is tint only).
            for sv in systems.iter_mut() {
                if sv.owner == Some(player_id)
                    && let Some(report) = self.world.information.site(sv.id, cc, c, now)
                {
                    view::apply_production_report(sv, report);
                }
                sv.ally = sv
                    .owner
                    .is_some_and(known_ally);
                // §ground G4: the PRE-COMMIT LANDING ESTIMATE. Only for a
                // besieger who actually has marines in orbit here — the person
                // making the decision, and nobody else. It is sampled from the
                // REAL ground engine, so it can never drift from the fight it
                // predicts, and it is computed from state this viewer already
                // holds (their own troops, the garrison their `ground` readout
                // already shows), so it discloses nothing new.
                if let Some(g) = sv.ground.as_mut()
                    && sv.owner != Some(player_id)
                    && let Some(sys) = known_systems.iter().find(|s| s.id == sv.id)
                    && sys.blockade.is_some_and(|b| b.by == player_id)
                {
                    let marines: u32 = ghosts.iter()
                        .filter(|g| g.own && g.pos.distance(sys.pos) <= sim::ship::COLONY_CLAIM_RADIUS)
                        .map(|g| self.history.own_marines_at(player_id, g.id, now - g.age))
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
                    && let Some((ships, fed)) = self.world.information.site(sv.id, cc, c, now)
                        .and_then(|report| report.garrison)
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
                        fed: own && self.world.information.site(sv.id, cc, c, now)
                            .is_some_and(|report| report.node_fed),
                        region_radius: if own { sim::NODE_REGION_RADIUS } else { 0.0 },
                    });
                }
            }
            // The arrived syndicate roster and invitations, additionally privacy-gated.
            let syndicate = known_syndicates.iter().find(|s| s.members.contains(&player_id))
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
                                role: s
                                    .role_of(*m)
                                    .unwrap_or(sim::SyndicateRole::Member),
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
                        my_role: s
                            .role_of(player_id)
                            .unwrap_or(sim::SyndicateRole::Member),
                    })
                });
            let syndicate_invites: Vec<crate::protocol::SyndicateInviteView> = known_syndicates.iter()
                .filter(|s| s.invites.contains(&player_id))
                .map(|s| crate::protocol::SyndicateInviteView {
                    id: s.id,
                    name: s.name.clone(),
                })
                .collect();
            let operation_positions: Vec<(sim::EntityId, sim::Vec2)> = self
                .world
                .systems
                .iter()
                .map(|system| (system.id, system.pos))
                .collect();
            let operations: Vec<crate::protocol::OperationView> = self
                .world
                .operations
                .values()
                .filter_map(|operation| {
                    let known = operation.known.get(&player_id)?;
                    let mut kind = operation.kind.clone();
                    if let sim::OperationKind::SyndicateMegaproject { stage, .. } = &mut kind {
                        *stage = known.stage;
                    }
                    Some(crate::protocol::OperationView {
                        id: operation.id,
                        issuer: operation.issuer,
                        scope: operation.scope.clone(),
                        kind,
                        state: known.state,
                        progress: known.progress,
                        goal: known.goal,
                        stage: known.stage,
                        reported_at: known.reported_at,
                        offered_at: operation.offered_at,
                        starts_at: operation.starts_at,
                        expires_at: operation.expires_at,
                        target_pos: operation
                            .kind
                            .target_pos(&operation_positions, self.world.hub),
                        reward: operation.reward,
                        briefing: operation.briefing.clone(),
                        joined: operation.participants.contains(&player_id),
                        assigned_fleet: self.world.information.fleet_assets(cc, c, now).iter()
                            .find(|f| f.owner == player_id && f.operations.contains(&operation.id)).map(|f| f.id),
                        winner: (known.state == sim::OperationState::Completed)
                            .then_some(operation.winner)
                            .flatten(),
                    })
                })
                .collect();
            let relations = self
                .world
                .diplomacy
                .values()
                .flat_map(|rows| rows.values())
                .filter_map(|relation| {
                    let other = relation.other(player_id)?;
                    let declaration_known = relation.pending_war.is_some_and(|pending| {
                        pending.declarer == player_id || now + 1e-9 >= pending.target_notified_at
                    });
                    let known = relation.known_by(player_id);
                    let known_reprisal = relation
                        .known_reprisal_until(player_id)
                        .filter(|until| *until > now);
                    let known_separation = relation.known_separation_until(player_id);
                    (known != sim::diplomacy::RelationState::Neutral
                        || declaration_known
                        || known_separation > now
                        || known_reprisal.is_some())
                        .then(|| crate::protocol::DiplomacyRelationView {
                            other,
                            name: self
                                .world
                                .players
                                .get(&other)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| "Unknown corporation".to_string()),
                            state: known,
                            war_activates_at: declaration_known
                                .then(|| relation.pending_war.map(|p| p.activates_at))
                                .flatten(),
                            separation_until: known_separation,
                            reprisal_until: known_reprisal,
                        })
                })
                .collect();
            let incoming = self
                .world
                .treaty_proposals
                .values()
                .filter(|proposal| {
                    proposal.to == player_id && proposal.visible_to_target_at <= now + 1e-9
                })
                .map(|proposal| crate::protocol::TreatyProposalView {
                    id: proposal.id,
                    from: proposal.from,
                    name: self
                        .world
                        .players
                        .get(&proposal.from)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "Unknown corporation".to_string()),
                    treaty: proposal.kind,
                    expires_at: proposal.expires_at,
                })
                .collect();
            let diplomacy = Some(Box::new(crate::protocol::DiplomacyView {
                relations,
                incoming,
            }));
            // §research R6: the viewer's OWN corporation research picture. It is
            // always present; social affiliation has no bearing on Programme Boards.
            let research = Some(Box::new(research_view(&self.world, player_id)));

            // §captains: the roster is an owner-only ledger, but assigned
            // progression must still ride the fleet's served light. Reserve
            // officers are stationary at a named owned system; their report is
            // the last progression already known when they left a formation.
            // A casualty status is withheld until the wreck report's own
            // wavefront arrives, so the ledger cannot announce a loss before the
            // map does.
            let served_captains: std::collections::BTreeMap<
                u32,
                crate::protocol::CaptainView,
            > = ghosts
                .iter()
                .filter_map(|ghost| ghost.captain.clone().map(|captain| (captain.id, captain)))
                .collect();
            let captain_truth = |captain: &sim::Captain| {
                let sighting = captain.sighting();
                crate::protocol::CaptainView {
                    id: captain.id,
                    name: captain.name.clone(),
                    portrait: captain.portrait,
                    level: sighting.level,
                    title: sighting.title,
                    portrait_age: sighting.portrait_age,
                    command_capacity: sighting.command_capacity,
                    xp: sighting.xp,
                    next_level_xp: sighting.next_level_xp,
                    unspent: sighting.unspent,
                    attributes: sighting.attributes,
                }
            };
            let known_captains = self.world.information.captains(player_id, cc, c, now);
            let captains: Vec<crate::protocol::CaptainRosterView> = known_captains.iter()
                .map(|captain| {
                    let casualty_known = captain
                        .missing_report_at
                        .is_some_and(|arrival| now >= arrival);
                    let physically_local = captain.assigned_fleet.is_none();
                    crate::protocol::CaptainRosterView {
                        id: captain.id,
                        name: captain.name.clone(),
                        portrait: captain.portrait,
                        assigned_fleet: if casualty_known {
                            None
                        } else {
                            captain.assigned_fleet
                        },
                        stationed_system: if casualty_known {
                            None
                        } else {
                            captain.stationed_system
                        },
                        recovering_until: casualty_known.then_some(captain.recover_at).flatten(),
                        loss_fate: casualty_known.then_some(captain.loss_fate).flatten(),
                        report: served_captains
                            .get(&captain.id)
                            .cloned()
                            .or_else(|| physically_local.then(|| captain_truth(captain)))
                            .or_else(|| casualty_known.then(|| captain_truth(captain))),
                    }
                })
                .collect();
            let captain_capacity = self.world.captain_capacity(player_id);

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
                            depth: sim::market::liquidity(*commodity).depth,
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
            let known_account = self.market_accounts.at(player_id, now - staleness);
            let standing = known_account.map_or(sim::tca::TCA_STANDING_MAX, |a| a.standing);
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
            // The programme may know an encounter has been assigned before the
            // privateer's own light reaches the CC. Do not put its true entity id
            // on the wire until it is present in this viewer's served picture.
            let known_privateer = corp
                .founding
                .privateer
                .filter(|id| ghosts.iter().any(|ghost| ghost.id == *id));
            let founding = crate::protocol::FoundingView {
                stage: corp.founding.stage,
                protected: corp.founding.protected(now),
                protection_min_until: corp.founding.started_at
                    + sim::founding::FOUNDER_PROTECTION_MIN_S,
                protection_max_until: corp.founding.started_at
                    + sim::founding::FOUNDER_PROTECTION_MAX_S,
                expansion_unlocked: corp.founding.expansion_unlocked(),
                interceptor: corp.founding.interceptor,
                privateer: (corp.founding.stage == sim::FoundingStage::DefeatPrivateer)
                    .then_some(known_privateer)
                    .flatten(),
                bounty_received: corp.founding.reward_granted,
                opening_exports: corp
                    .founding
                    .opening_export_reports
                    .iter()
                    .filter_map(|(&commodity, &report_at)| (report_at <= now).then_some(commodity))
                    .collect(),
                survey_candidates: corp.founding.survey_candidates.clone(),
            };

            let wallet = WalletView {
                report_pending: known_account.is_none(),
                credits: known_account.map_or(0.0, |account| account.credits),
                valuation: known_account.map_or(0.0, |account| account.valuation),
                warehouse: known_account
                    .into_iter().flat_map(|account| account.warehouse.iter())
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
                    Vec::new()
                },
                // Sum arrived Fuel reports; an empire total is no exemption.
                fuel_total: known_systems
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
            // center + standing arrays + their Raider fleets' bubbles) gates a
            // third party's bucket access to a battle site.
            let coverage = self.history.coverage_for(player_id, cc, c, now, &arrays);
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
                    known_syndicates.iter().find(|s| s.members.contains(&corp))
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
                terms: known_systems
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
                    .information.shipments(player_id, cc, c, now)
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
                    captains,
                    captain_capacity,
                    jump_departures,
                    emplacements: emplacements_view,
                    market,
                    wallet,
                    charter,
                    founding,
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
                    operations,
                    exploration_sites: self.world.exploration.reports_for(player_id),
                    midgame_stage: corp.midgame_stage,
                    diplomacy,
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
            let jlen = self.timeline.revision(player_id);
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
                let jlen = self.timeline.revision(owner);
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
        }

        for (_conn_id, info) in self.sessions.iter_conns_mut() {
            if let Some(view) = views.get(&info.player_id) {
                // Last-write-wins: overwrite this connection's latest-View slot.
                // A slow client simply never sees the frames it fell behind on —
                // the writer always emits the freshest, never a stale backlog.
                // (Err only if the writer task is already gone; harmless.)
                let _ = info.view_tx.send(Some(view.clone()));
                info.last_view_broadcast = Some(broadcast_index);
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
                let cc = self.world.players[&info.player_id].command_center;
                let rankings = self.world.information.rankings(cc, self.world.hub, self.world.config.c, self.world.time);
                send_sections(sec, sig_of(&rankings), rankings, info);
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
    exploration_journal: Vec<sim::sites::JournalEntry>,
    exploration_journal_version: u64,
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
    let send_journal = sent.exploration_journal_version != Some(sec.exploration_journal_version);
    if !(send_standing || send_reports || send_captures || send_ranks || send_journal) {
        return;
    }
    let msg = ServerMsg::Sections {
        standing_orders: send_standing.then(|| sec.standing.clone()),
        battle_reports: send_reports.then(|| sec.reports.clone()),
        capture_reports: send_captures.then(|| sec.captures.clone()),
        rankings: send_ranks.then(|| rankings.to_vec()),
        exploration_journal: send_journal.then(|| sec.exploration_journal.clone()),
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
        if send_journal {
            info.sent.exploration_journal_version = Some(sec.exploration_journal_version);
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

fn nebula_infos(world: &sim::World) -> Vec<NebulaInfo> {
    world
        .nebulas
        .iter()
        .map(|region| NebulaInfo {
            id: region.id,
            kind: region.kind,
            name: region.name.clone(),
            center: region.center,
            radius_x: region.radius_x,
            radius_y: region.radius_y,
            rotation: region.rotation,
            signature_mult: region.kind.signature_mult(),
            sensor_mult: region.kind.sensor_mult(),
            jump_range_mult: region.kind.jump_range_mult(),
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
        ("tiny_freighter", "Tiny Freighter", BuildKind::Ship { ship: ShipKind::TinyFreighter }),
        ("small_freighter", "Small Freighter", BuildKind::Ship { ship: ShipKind::SmallFreighter }),
        ("large_freighter", "Large Freighter", BuildKind::Ship { ship: ShipKind::LargeFreighter }),
        ("heavy_freighter", "Heavy Freighter", BuildKind::Ship { ship: ShipKind::HeavyFreighter }),
        ("bulk_freighter", "Bulk Freighter", BuildKind::Ship { ship: ShipKind::BulkFreighter }),
        (
            "convoy",
            "Medium Freighter",
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
            "Interceptor",
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
    // Equipment catalog, keyed `module:<slug>` so the client routes
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
                research_prerequisite: match what {
                    BuildKind::Upgrade { upgrade } => upgrade.research_prerequisite(),
                    _ => None,
                },
                conversion: match what {
                    BuildKind::Upgrade { upgrade } => sim::production::CONVERTERS.iter()
                        .find(|converter| converter.structure == upgrade)
                        .map(|converter| crate::protocol::ConversionRecipeView {
                            output: converter.output,
                            rate: converter.rate,
                            inputs: converter.inputs.to_vec(),
                            byproducts: converter.byproducts().to_vec(),
                        }),
                    _ => None,
                },
                refining_recipes: if matches!(what, BuildKind::Upgrade { upgrade: sim::build::StructureKind::Smelter }) {
                    sim::production::ORE_REFINING.iter().map(|r| crate::protocol::ConversionRecipeView {
                        output: r.output, rate: r.rate, inputs: r.inputs.to_vec(), byproducts: r.byproducts().to_vec(),
                    }).collect()
                } else { Vec::new() },
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
            // Derive the research reward copy from the same gate sent to the
            // builder. No second handwritten list of unlocks can drift from it.
            let structures = sim::build::StructureKind::ALL.into_iter()
                .filter(|kind| kind.research_prerequisite() == Some(id))
                .map(|kind| kind.title())
                .collect::<Vec<_>>();
            Some(crate::protocol::ProgrammeInfo {
                id: id.to_string(),
                field: p.field.slug().to_string(),
                school: p.school.map(|s| s.slug().to_string()),
                tier: p.tier,
                name: p.name.to_string(),
                blurb: if structures.is_empty() { p.blurb.to_string() }
                    else { format!("Unlocks {}. {}", structures.join(", "), p.blurb) },
                cost: sim::research::cost_of(id),
            })
        })
        .collect()
}

/// §research R6: build the viewer's OWN corporation research picture (owner-only).
fn research_view(world: &sim::World, owner: sim::PlayerId) -> crate::protocol::ResearchView {
    use crate::protocol::{AcademyRow, ActiveResearchView, ProgrammeDynView, ResearchView};
    let rs = &world.players[&owner].research;
    let now = world.time;
    let metric = |m| world.corporation_metric(owner, m);

    // The per-Academy contribution table (the same factor chain the clock uses).
    let cc = world.players[&owner].command_center;
    let contribs: Vec<_> = world.information.owned_sites(owner, cc, world.config.c, now)
        .flat_map(|report| report.academies.iter())
        .filter(|lab| rs.active.as_deref() == Some(lab.programme.as_str())).collect();
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
                recovered_data: rs.recovered_data.get(id).copied(),
            })
        })
        .collect();

    ResearchView {
        blueprints: rs.blueprints.iter().copied().collect(),
        active,
        queue: rs.queue.clone(),
        rate,
        stalled: rs.stalled,
        academies,
        programmes,
    }
}

/// In-memory, production-path packets for binary interoperability/size tests.
/// No listening socket, database, or live galaxy is involved.
#[cfg(test)]
pub(crate) async fn binary_protocol_fixtures() -> Vec<ServerMsg> {
    let world = World::new(sim::SimConfig::for_players(0xC0FFEE, 5));
    let (status_tx, _) = watch::channel(ServerStatus::default());
    let (estimate_tx, _) = mpsc::unbounded_channel();
    let mut game = GameLoop::new(world, 1.0, status_tx, estimate_tx);
    let mut receivers = Vec::new();
    for index in 0..5u64 {
        let name = format!("Binary test corporation {index} 星");
        let (outbound, rx) = mpsc::channel(128);
        let (view_tx, view_rx) = watch::channel(None);
        let (replace_tx, _) = watch::channel(false);
        game.handle_input(GameInput::Connect {
            conn_id: index, player_id: crate::protocol::player_id_from_name(&name), name,
            outbound, view_tx, replace_tx, view_divisor: if index == 4 { 2 } else { 1 },
        });
        receivers.push((rx, view_rx));
    }
    let mut messages = Vec::new();
    for _ in 0..4 {
        for _ in 0..3 { game.tick(); }
        game.broadcast();
        for (rx, view_rx) in &mut receivers {
            while let Ok(message) = rx.try_recv() { messages.push(message); }
            if view_rx.has_changed().unwrap() {
                if let Some(message) = view_rx.borrow_and_update().clone() { messages.push(message); }
            }
        }
    }
    // A 30-second, 60-hull replay slice from the actual tactical engine. It is
    // passed through record_rounds_range AFTER the same arrived-prefix gate.
    let side = |fleet, kind| (0..30).map(|id| (sim::EntityId(fleet), sim::ship::Ship::new(id, kind, sim::Loadout::default()))).collect::<Vec<_>>();
    let a = side(900, sim::ShipKind::Raider);
    let d = side(901, sim::ShipKind::Corvette);
    let mut tactical = sim::tactical::TacticalState::open(123, 900, &a, &d, 0, 0.0, sim::Vec2::new(1.0, 0.0));
    let sides = [1, 2].map(|id| sim::SideRecord {
        corp: sim::PlayerId(id), initial: Default::default(), initial_loadouts: Default::default(),
        posture: sim::EngagementPolicy::EngageAny, platform_tiers: 0,
    });
    let mut record = sim::BattleRecord::open(sim::EntityId(900), sim::Vec2::ZERO, None, false, 0, sides);
    for step in 1..=60 {
        let out = tactical.step(false, [sim::tactical::SideMods::default(); 2]);
        record.flush_step(step * 15, tactical.keyframe(out.deaths), Default::default());
    }
    let records = std::collections::BTreeMap::from([(record.id, record)]);
    let spec = view::visible_record_specs(&records, sim::PlayerId(1), sim::Vec2::ZERO,
        game.world.config.c, 30.0, &[], &|_| None).remove(0);
    messages.push(ServerMsg::BattleRecords {
        updates: vec![crate::protocol::BattleRecordUpdate {
            id: spec.id, header: Some(view::record_header(&records[&spec.id], &spec)),
            new_rounds: view::record_rounds_range(&records[&spec.id], 0, spec.arrived_len, true),
            light_frontier_tick: spec.frontier_tick, outcome: None,
        }], removed: Vec::new(),
    });
    messages.push(ServerMsg::CommandSignal {
        order_id: 42, ship_id: sim::EntityId(u64::MAX), depart_time: 12345.678901234567,
        arrive_time: 12500.123456789, hops: Vec::new(),
    });
    messages.push(ServerMsg::OrderConfirmed { order_id: 42, ship_id: sim::EntityId(u64::MAX), kind: sim::OrderKind::Move });
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Vec2;

    #[test]
    fn every_freighter_quote_uses_its_authoritative_construction_manifest() {
        let options = build_options();
        for (kind, key) in sim::ship::PLAYER_FREIGHTERS.into_iter().zip([
            "tiny_freighter", "small_freighter", "convoy", "large_freighter",
            "heavy_freighter", "bulk_freighter",
        ]) {
            let option = options.iter().find(|o| o.key == key).unwrap();
            let recipe = sim::build::recipe_for(sim::build::BuildKind::Ship { ship: kind });
            let served: Vec<_> = option.costs.iter()
                .map(|slot| (slot.commodity, slot.units as f64)).collect();
            assert_eq!(served, recipe.costs, "{key}: menus and reserved projects price the sim recipe");
            assert_eq!(option.build_secs, recipe.build_ticks as f64 / sim::config::TICK_HZ as f64);
        }
    }

    #[test]
    fn ore_recipes_are_served_from_the_sim_catalog_with_secondary_yields() {
        let options = build_options();
        let smelter = options.iter().find(|o| o.key == "smelter").unwrap();
        assert_eq!(smelter.refining_recipes.len(), 5);
        assert_eq!(smelter.research_prerequisite, Some("mat_enrichment"));
        for (served, recipe) in smelter.refining_recipes.iter().zip(sim::production::ORE_REFINING) {
            assert_eq!(served.inputs, recipe.inputs);
            assert_eq!(served.byproducts, recipe.byproducts());
            assert_eq!((served.output,served.rate),(recipe.output,recipe.rate));
        }
        assert!(options.iter().filter(|o| o.key != "smelter").all(|o| o.refining_recipes.is_empty()));
    }

    #[test]
    fn discovery_notes_retry_on_backpressure_and_only_resend_on_edit_or_reconnect() {
        let (tx, mut rx) = mpsc::channel(1);
        let (view_tx, _) = watch::channel(None);
        let (replace_tx, _) = watch::channel(false);
        let mut conn = ConnInfo { player_id: PlayerId(80), name: "Journal".into(), outbound: tx,
            view_tx, replace_tx, view_divisor: 1, last_view_broadcast: None, sent: Default::default() };
        let mut section = SectionData { standing: vec![], standing_sig: 0, reports: vec![], reports_sig: 0,
            captures: vec![], captures_sig: 0, exploration_journal_version: 1,
            exploration_journal: vec![sim::sites::JournalEntry { id: sim::EntityId(42),
                kind: sim::sites::JournalKind::Site, pinned: true, note: "Return with escorts".into() }] };
        conn.outbound.try_send(ServerMsg::Error { message: "occupy queue".into() }).unwrap();
        send_sections(&section, 0, &[], &mut conn);
        assert!(conn.sent.exploration_journal_version.is_none(), "failed enqueue must retry");
        rx.try_recv().unwrap();
        send_sections(&section, 0, &[], &mut conn);
        assert!(matches!(rx.try_recv().unwrap(), ServerMsg::Sections { exploration_journal: Some(j), .. } if j == section.exploration_journal));
        send_sections(&section, 0, &[], &mut conn);
        assert!(rx.try_recv().is_err(), "never retransmit notes at 10 Hz");
        conn.sent = Default::default();
        send_sections(&section, 0, &[], &mut conn);
        assert!(matches!(rx.try_recv().unwrap(), ServerMsg::Sections { exploration_journal: Some(j), .. } if j.len() == 1));
        section.exploration_journal.clear(); section.exploration_journal_version += 1;
        send_sections(&section, 0, &[], &mut conn);
        assert!(matches!(rx.try_recv().unwrap(), ServerMsg::Sections { exploration_journal: Some(j), standing_orders: None, .. } if j.is_empty()),
            "clearing the journal sends an empty section, not an absent one");
    }

    fn reporting_game(world: World) -> GameLoop {
        let (status, _) = watch::channel(ServerStatus::default());
        let (estimate, _) = mpsc::unbounded_channel();
        GameLoop::new(world, 1.0, status, estimate)
    }

    #[tokio::test]
    async fn market_history_waits_for_light_and_survives_offline_restart() {
        let owner = PlayerId(801);
        let mut world = World::new(sim::SimConfig::for_players(801, 2));
        world.step(&[Command::AddPlayer { id: owner, name: "Ledger".into() }]);
        let start = world.time;
        let arrival = start + sim::transit::delay(world.hub, world.players[&owner].command_center, world.config.c);
        assert!(arrival > start + 1.0);
        let mut game = reporting_game(world);
        let trade = sim::TradeEvent::Sold { player: owner, commodity: sim::Commodity::MetallicOre,
            units: 150, unit_price: 8.22, penalty: 1.25 };
        game.schedule_trade_reports(&[sim::Event::new(start, sim::EventPayload::Trade(trade))]);
        game.world.time = arrival - 0.001;
        game.deliver_trade_reports();
        assert!(game.transactions.page(owner, None).0.is_empty(), "no count, ID or receipt before its light");

        let bytes = crate::persistence::store::encode(&game.durable_checkpoint()).unwrap();
        let (status, _) = watch::channel(ServerStatus::default());
        let (estimates, _) = mpsc::unbounded_channel();
        let mut restored = GameLoop::restore(crate::persistence::store::decode(&bytes).unwrap(), 1.0, status, estimates);
        restored.world.time = arrival;
        restored.deliver_trade_reports();
        let rows = restored.transactions.page(owner, None).0;
        assert_eq!(rows.len(), 1, "offline receipts are retained");
        assert_eq!(rows[0].occurred_at, Some(start));
        assert!((rows[0].reported_at - arrival).abs() < 1e-9);
        assert!(matches!(rows[0].details, TransactionDetails::Trade { trade: sim::TradeEvent::Sold {
            units: 150, unit_price, penalty, .. } } if unit_price == 8.22 && penalty == 1.25));
        restored.deliver_trade_reports();
        assert_eq!(restored.transactions.page(owner, None).0.len(), 1, "exactly one entry, not once per view");
        let reopened = reporting_game(serde_json::from_value(restored.checkpoint_snapshot()).unwrap());
        assert_eq!(reopened.transactions.page(owner, None).0.len(), 1);
    }

    #[tokio::test]
    async fn transaction_pages_are_authenticated_local_reads_not_hub_orders() {
        let owner = PlayerId(801);
        let rival = PlayerId(802);
        let mut world = World::new(sim::SimConfig::for_players(801, 2));
        world.step(&[Command::AddPlayer { id: owner, name: "Ledger".into() }]);
        let mut game = reporting_game(world);
        for id in [owner, rival] {
            game.transactions.record(id, Some(0.0), 0.0, TransactionDetails::EarlierReport {
                text: if id == owner { "own receipt" } else { "private rival receipt" }.into(),
            });
        }
        let (outbound, mut rx) = mpsc::channel(16);
        let (view_tx, _) = watch::channel(None);
        let (replace_tx, _) = watch::channel(false);
        game.handle_input(GameInput::Connect { conn_id: 91, player_id: owner, name: "Ledger".into(),
            outbound, view_tx, replace_tx, view_divisor: 1 });
        while rx.try_recv().is_ok() {}
        let pending = game.pending.len();
        game.handle_input(GameInput::Intent { conn_id: 91,
            msg: ClientMsg::RequestTransactions { before: None, request_id: 7 } });
        let ServerMsg::Transactions { player_id, request_id, entries, next_before, .. } = rx.try_recv().unwrap()
            else { panic!("history request must not issue a comet or hub command") };
        assert_eq!(player_id, owner);
        assert_eq!(request_id, 7);
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0].details, TransactionDetails::EarlierReport { text } if text == "own receipt"));
        assert!(next_before.is_none());
        assert_eq!(pending, game.pending.len());
        assert!(rx.try_recv().is_err());
        game.handle_input(GameInput::Intent { conn_id: 999,
            msg: ClientMsg::RequestTransactions { before: None, request_id: 8 } });
        assert!(rx.try_recv().is_err(), "no client-selected owner or unauthenticated page");
    }

    #[tokio::test]
    async fn legacy_market_history_recovers_only_arrived_reports_without_inventing_times() {
        let owner = PlayerId(801);
        let mut world = World::new(sim::SimConfig::for_players(801, 2));
        world.step(&[Command::AddPlayer { id: owner, name: "Ledger".into() }]);
        let mut game = reporting_game(world);
        let trade = sim::TradeEvent::Sold { player: owner, commodity: sim::Commodity::MetallicOre,
            units: 150, unit_price: 8.22, penalty: 0.0 };
        let event = sim::Event::new(game.world.time, sim::EventPayload::Trade(trade));
        game.timeline.ingest(&[event.clone()], &game.world);
        game.schedule_trade_reports(&[event]);
        let arrival = game.trade_reports[0].arrives_at;
        let old_save = |game: &GameLoop| -> World {
            let mut snapshot = game.checkpoint_snapshot();
            let mut report: serde_json::Value = serde_json::from_str(snapshot["reporting_checkpoint"].as_str().unwrap()).unwrap();
            report.as_object_mut().unwrap().remove("transactions");
            for pending in report["trade_reports"].as_array_mut().unwrap() {
                pending.as_object_mut().unwrap().remove("transaction");
            }
            snapshot["reporting_checkpoint"] = serde_json::Value::String(report.to_string());
            serde_json::from_value(snapshot).unwrap()
        };
        let mut restored = reporting_game(old_save(&game));
        assert!(restored.transactions.page(owner, None).0.is_empty());
        restored.world.time = arrival;
        restored.deliver_trade_reports();
        let rows = restored.transactions.page(owner, None).0;
        assert_eq!(rows.len(), 1);
        assert!(rows[0].occurred_at.is_none(), "old pending receipt lacks exact event time");

        game.world.time = arrival;
        game.trade_reports.clear();
        game.timeline.promote(arrival + 1e-6);
        let imported = reporting_game(old_save(&game));
        let rows = imported.transactions.page(owner, None).0;
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0].details, TransactionDetails::EarlierReport { text }
            if text.contains("Sold 150 ferrite ore")));
        let reopened = reporting_game(serde_json::from_value(imported.checkpoint_snapshot()).unwrap());
        assert_eq!(reopened.transactions.page(owner, None).0.len(), 1, "legacy import happens once");
    }

    #[tokio::test]
    async fn authenticated_ids_resolve_diplomacy_without_claiming_legacy_names() {
        let owner = PlayerId(77001);
        let legacy = PlayerId(77002);
        let mut world = World::new(sim::SimConfig::for_players(801, 4));
        world.step(&[
            Command::AddPlayer { id: owner, name: "Aurora".into() },
            Command::AddPlayer { id: legacy, name: "Old Charter".into() },
        ]);
        let mut game = reporting_game(world);
        assert_eq!(game.corporation_named(" AURORA "), owner);
        assert_eq!(game.corporation_named("not a corporation"), PlayerId(0));
        let (outbound, mut rx) = mpsc::channel(16);
        let (view_tx, _) = watch::channel(None);
        let (replace_tx, _) = watch::channel(false);
        game.handle_input(GameInput::Connect { conn_id: 91, player_id: PlayerId(99001),
            name: "Old Charter".into(), outbound, view_tx, replace_tx, view_divisor: 1 });
        assert!(matches!(rx.try_recv().unwrap(), ServerMsg::Error { .. }));
        assert!(game.sessions.player_of(91).is_none());
        assert!(!game.world.players.contains_key(&PlayerId(99001)));
    }

    #[tokio::test]
    async fn battle_mark_namespace_survives_restart_not_a_same_seed_fresh_galaxy() {
        let game = reporting_game(World::new(sim::SimConfig::for_players(801, 4)));
        let fresh = reporting_game(World::new(sim::SimConfig::for_players(801, 4)));
        assert!(uuid::Uuid::parse_str(&game.galaxy_instance_id).is_ok());
        assert_ne!(game.galaxy_instance_id, fresh.galaxy_instance_id,
            "same seed and entity counters must not reuse the dismissal namespace");
        let saved = game.checkpoint_snapshot();
        let restored = reporting_game(serde_json::from_value(saved.clone()).unwrap());
        assert_eq!(game.galaxy_instance_id, restored.galaxy_instance_id,
            "a saved-game restart is still the same game");

        // A pre-identity checkpoint is upgraded once; that new identity then
        // survives future restarts. No guess based on seed or event counters.
        let mut legacy_world = saved;
        let mut legacy: serde_json::Value = serde_json::from_str(legacy_world["reporting_checkpoint"].as_str().unwrap()).unwrap();
        legacy.as_object_mut().unwrap().remove("galaxy_instance_id");
        legacy_world["reporting_checkpoint"] = serde_json::Value::String(legacy.to_string());
        let upgraded = reporting_game(serde_json::from_value(legacy_world).unwrap());
        assert_ne!(game.galaxy_instance_id, upgraded.galaxy_instance_id);
        let reopened = reporting_game(serde_json::from_value(upgraded.checkpoint_snapshot()).unwrap());
        assert_eq!(upgraded.galaxy_instance_id, reopened.galaxy_instance_id);
    }

    #[tokio::test]
    async fn restart_preserves_arrived_metadata_and_future_receipts() {
        let mut world = World::new(sim::SimConfig::for_players(801, 4));
        let owner = PlayerId(801);
        world.step(&[Command::AddPlayer { id: owner, name: "Checkpoint".into() }]);
        let cc = world.players[&owner].command_center;
        let fleet = *world.fleets.iter().find(|(_, f)| f.owner == owner).unwrap().0;
        let pos = cc + Vec2::new(20_000.0, 0.0);
        let start = world.time;
        let leg = sim::transit::delay(pos, cc, world.config.c);
        {
            let f = world.fleets.get_mut(&fleet).unwrap();
            f.pos = pos;
            f.vel = Vec2::ZERO;
            f.order = sim::ship::FleetOrder::Idle;
            f.set_cargo_stacks(vec![sim::Cargo { commodity: sim::Commodity::Alloys, units: 10 }]);
            f.supplied = true;
            f.posture = sim::EngagementPosture::Passive;
        }
        let mut game = reporting_game(world);
        game.history.record(&game.world);
        game.prices.record(&game.world);
        let old_credits = game.world.players[&owner].credits;
        game.world.time = start + 5.0;
        {
            let f = game.world.fleets.get_mut(&fleet).unwrap();
            f.set_cargo_stacks(vec![sim::Cargo { commodity: sim::Commodity::Alloys, units: 85 }]);
            f.supplied = false;
            f.posture = sim::EngagementPosture::Defensive;
            f.ships[0].hp *= 0.5;
        }
        game.world.players.get_mut(&owner).unwrap().credits += 77.0;
        game.history.record(&game.world);
        game.market_accounts.record(&game.world);
        game.timeline.ingest(&[sim::Event::new(game.world.time, sim::EventPayload::OrderRejected {
            owner, fleet, target: None, reason: sim::event::OrderRejectReason::Unsupplied,
        }).at_origin(pos)], &game.world);
        game.observed_order_plans.insert((owner, 42), ObservedOrderPlan {
            arrives_at: start + 10.0, response_at: start + 20.0, intent_path: Vec::new(),
        });
        let restored_world: World = serde_json::from_value(game.checkpoint_snapshot()).unwrap();
        let mut restored = reporting_game(restored_world);
        assert_eq!(restored.observed_order_plans[&(owner, 42)].response_at, start + 20.0);
        assert!(restored.market_accounts.at(owner, start - 0.001).is_none(), "never backfill old light from newest truth");
        assert_eq!(restored.market_accounts.at(owner, start + 4.999).unwrap().credits, old_credits);
        assert_eq!(restored.market_accounts.at(owner, start + 5.0).unwrap().credits, old_credits + 77.0);
        let picture = |g: &GameLoop, now| g.history.view_for(owner, cc, g.world.config.c, now)
            .into_iter().find(|f| f.id == fleet).unwrap();
        let before = picture(&restored, start + leg + 4.0);
        assert_eq!(before.cargo_manifest[0].units, 10);
        assert!(before.supplied);
        assert_eq!(before.posture, Some(sim::EngagementPosture::Passive));
        let after = picture(&restored, start + leg + 5.0);
        assert_eq!(after.cargo_manifest[0].units, 85);
        assert!(!after.supplied);
        assert_eq!(after.posture, Some(sim::EngagementPosture::Defensive));
        assert!(after.damage.unwrap() > before.damage.unwrap());
        restored.timeline.promote(start + leg + 4.999);
        assert_eq!(restored.timeline.journal_len(owner), 0);
        restored.timeline.promote(start + leg + 5.0);
        assert_eq!(restored.timeline.journal_len(owner), 1);
    }

    #[test]
    fn fast_pacing_keeps_views_at_the_same_wall_cadence() {
        assert_eq!(DEFAULT_SIM_PACING, 1.0);
        assert_eq!(broadcast_every_for(DEFAULT_SIM_PACING), 3);
        let fast_pacing = 4.0;
        assert_eq!(broadcast_every_for(fast_pacing), 12);

        let standard_view_hz = TICK_HZ as f64 / broadcast_every_for(1.0) as f64;
        let fast_view_hz = TICK_HZ as f64 * fast_pacing
            / broadcast_every_for(fast_pacing) as f64;
        assert!((standard_view_hz - 10.0).abs() < f64::EPSILON);
        assert!((fast_view_hz - standard_view_hz).abs() < f64::EPSILON);
    }

    #[test]
    fn dispatch_chevrons_cover_only_unqueued_market_hub_orders() {
        let fleet = sim::EntityId(71);
        let other = sim::EntityId(72);
        assert!(
            dispatch_chevron_targets(&ClientMsg::SetFleetPosture {
                fleet_id: fleet,
                posture: sim::EngagementPosture::Defensive,
            })
            .is_empty(),
            "fleet settings use the sim's delayed order lifecycle",
        );
        assert!(
            dispatch_chevron_targets(&ClientMsg::MergeFleets {
                into: fleet,
                from: other,
            })
            .is_empty(),
            "fleet reorganization uses the sim's delayed order lifecycle",
        );
        assert!(
            dispatch_chevron_targets(&ClientMsg::HubUnload { fleet_id: fleet }).is_empty(),
            "unloading uses the sim's delayed order lifecycle",
        );
        assert_eq!(
            dispatch_chevron_targets(&ClientMsg::MarketBuy {
                commodity: sim::Commodity::Fuel,
                units: 1,
                max_unit_price: None,
            }),
            vec![DispatchChevronTarget::MarketHub],
        );
        assert!(
            dispatch_chevron_targets(&ClientMsg::MoveShip {
                ship_id: fleet,
                dest: Vec2::ZERO,
            })
            .is_empty(),
            "scheduled movement keeps its lifecycle-linked CommandSignal",
        );
    }

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
    fn two_orders_remain_until_served_evidence_not_true_supersession() {
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
            2,
            "true supersession must not retire an older receipt before its light arrives"
        );
        assert_eq!(after_supersession[0].id, first_id);

        let echo = after_supersession[0].echo_at;
        while world.time < echo + sim::DT {
            world.step(&[]);
        }
        assert_eq!(
            world.pending_commands(owner).len(),
            2,
            "both rows survive expired estimates until the map serves compliance",
        );
        let confirmed = world.confirm_orders_from_served(
            owner,
            &[(fleet, after_supersession[1].delivered_at, false)],
        );
        assert_eq!(confirmed.len(), 2);
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
        let research = research_catalog();
        for kind in sim::build::StructureKind::ALL {
            let option = opts.iter().find(|o| o.key == kind.slug()).unwrap();
            assert_eq!(option.research_prerequisite, kind.research_prerequisite());
            if let Some(id) = option.research_prerequisite {
                let programme = research.iter().find(|p| p.id == id).expect("unlock is visible in Research");
                assert!(programme.blurb.starts_with("Unlocks "));
                assert!(programme.blurb.contains(kind.title()), "{} advertises {}", id, kind.title());
            }
        }
        // The UI reads the live recipe and research gate, including fractional
        // inputs; no parallel client conversion table can silently disagree.
        for converter in &sim::production::CONVERTERS {
            let option = opts.iter().find(|o| o.key == converter.structure.slug()).unwrap();
            let recipe = option.conversion.as_ref().expect("factory has a production recipe");
            assert_eq!(recipe.output, converter.output);
            assert_eq!(recipe.rate, converter.rate);
            assert_eq!(recipe.inputs, converter.inputs);
            assert_eq!(option.research_prerequisite, converter.structure.research_prerequisite());
        }
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

    #[test]
    fn fifteen_second_battle_delay_keeps_the_icon_and_final_frame_on_one_wavefront() {
        let (cc, c) = (Vec2::ZERO, 400.0);
        let pos = Vec2::new(30_000.0, 0.0);
        let delay = sim::transit::delay(pos, cc, c);
        assert_eq!(delay, 15.0);
        let cb = concluded(0.0, 40.0, pos);
        let sides = [PlayerId(1), PlayerId(2)].map(|corp| sim::SideRecord {
            corp, initial: Default::default(), initial_loadouts: Default::default(),
            posture: sim::EngagementPolicy::EngageAny, platform_tiers: 0,
        });
        let mut record = sim::BattleRecord::open(cb.id, pos, None, false, 0, sides);
        for tick in (15..=1200).step_by(15) {
            record.flush_step(tick, sim::combat::Keyframe::default(), Default::default());
        }
        record.finalize(1200, sim::RaidOutcome::TargetDestroyed, Default::default(), Default::default());
        let records = BTreeMap::from([(cb.id, record)]);
        // The record is already finished in TRUTH throughout this sweep. Neither
        // the live marker nor the served ending may use that fact ahead of light.
        for now in [40.0, 54.999, 55.0] {
            let spec = view::visible_record_specs(&records, PlayerId(1), cc, c, now, &[], &|_| None).remove(0);
            let arrived = now >= cb.ended_at + delay;
            assert_eq!(cb.shows_in_progress(cc, c, now), !arrived);
            assert_eq!(spec.outcome.is_some(), arrived);
            assert_eq!(spec.arrived_len == 80, arrived);
            assert!(spec.frontier_tick as f64 * DT + delay <= now);
            eprintln!("15s battle delay: now={now:.3}, live_icon={}, rounds={}/80, outcome={:?}",
                cb.shows_in_progress(cc, c, now), spec.arrived_len, spec.outcome);
        }
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
