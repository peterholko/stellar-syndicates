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

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, info};

use sim::{Command, PlayerId, World, DT, TICK_HZ};

use crate::persistence::{to_json, PersistJob, PersistenceHandle};
use crate::protocol::{
    ClientMsg, GalaxyInfo, InvSlot, MarketView, OrderView, PriceView, ServerMsg, WalletView,
};
use crate::reports::ReportScheduler;
use crate::session::{ConnInfo, GameInput, ServerStatus, Sessions};
use crate::view::{self, PositionHistory, PriceHistory};

/// Push a per-player message every N sim ticks. At 30 Hz, N=3 → ~10 Hz network
/// updates: visibly live without flooding the socket.
const BROADCAST_EVERY: u64 = 3;

/// Default full-world snapshot cadence: every 20 s at the tick rate.
pub const DEFAULT_SNAPSHOT_EVERY: u64 = 20 * TICK_HZ as u64;

struct GameLoop {
    world: World,
    sessions: Sessions,
    /// Per-player lightspeed view filter — keeps position history and builds
    /// each player's delayed/fogged view (§14).
    history: PositionHistory,
    /// Lagged hub-price ticker history (§9) — each player reads prices delayed
    /// by their light-distance from the hub.
    prices: PriceHistory,
    /// Delayed delivery of discrete reports (raid outcomes) — each player learns
    /// them on their own clock (§8).
    reports: ReportScheduler,
    /// Commands accumulated since the last tick, applied at the next boundary.
    pending: Vec<Command>,
    persistence: PersistenceHandle,
    /// Take a snapshot every this many ticks.
    snapshot_every: u64,
    /// Publishes server/ops status for the `/status` endpoint (meta channel).
    status_tx: watch::Sender<ServerStatus>,
}

impl GameLoop {
    fn new(
        world: World,
        persistence: PersistenceHandle,
        snapshot_every: u64,
        status_tx: watch::Sender<ServerStatus>,
    ) -> Self {
        let history = PositionHistory::for_world(&world);
        let prices = PriceHistory::for_world(&world);
        GameLoop {
            world,
            sessions: Sessions::new(),
            history,
            prices,
            reports: ReportScheduler::new(),
            pending: Vec::new(),
            persistence,
            snapshot_every: snapshot_every.max(1),
            status_tx,
        }
    }

    /// Send the issuing player the outbound command-signal feedback for an order
    /// to one of THEIR ships. The comet's duration is the player's OBSERVED
    /// staleness of that ship (its ghost age), so it meets the ghost and reveals
    /// no true distance. Skipped if the player doesn't own the ship or it's
    /// currently dark to them.
    fn emit_command_signal(&self, player_id: PlayerId, ship_id: sim::EntityId) {
        let Some(corp) = self.world.players.get(&player_id) else {
            return;
        };
        let owns = self
            .world
            .ships
            .get(&ship_id)
            .map(|s| s.owner == player_id)
            .unwrap_or(false);
        if !owns {
            return;
        }
        let cc = corp.command_center;
        let c = self.world.config.c;
        let now = self.world.time;
        // Observed one-way light delay to the ship (its ghost staleness). Falls
        // back to ~0 if just spawned at home. The order reaches the ship one
        // delay out; the light of its maneuver returns one delay back — so the
        // player sees the reaction after the full round trip (the three clocks).
        let age = self.history.observed_age(ship_id, cc, c, now).unwrap_or(0.0);
        self.sessions.send_to_player(
            player_id,
            ServerMsg::CommandSignal {
                ship_id,
                depart_time: now,
                arrive_time: now + age,
                observe_time: now + 2.0 * age,
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

    fn handle_input(&mut self, input: GameInput) {
        match input {
            GameInput::Connect {
                conn_id,
                player_id,
                name,
                outbound,
            } => {
                let newly_online = self.sessions.insert(
                    conn_id,
                    ConnInfo {
                        player_id,
                        name: name.clone(),
                        outbound,
                    },
                );
                // Greet this connection immediately with its identity, clock,
                // and the static galaxy geography.
                self.sessions.send_to_conn(
                    conn_id,
                    ServerMsg::Welcome {
                        player_id,
                        name: name.clone(),
                        tick_hz: TICK_HZ,
                        tick: self.world.tick,
                        sim_time: self.world.time,
                        galaxy: GalaxyInfo {
                            hub: self.world.hub,
                            radius: self.world.config.galaxy_radius,
                            c: self.world.config.c,
                            sensor_range: self.world.config.sensor_range,
                            systems: self.world.systems.clone(),
                        },
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
                        self.emit_command_signal(player_id, ship_id);
                        self.pending.push(Command::MoveShip {
                            player_id,
                            ship_id,
                            dest,
                        });
                    }
                }
                ClientMsg::CommitRaid { raider_id, target_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, raider_id);
                        self.pending.push(Command::CommitRaid {
                            player_id,
                            raider_id,
                            target_id,
                        });
                    }
                }
                ClientMsg::RecallRaid { raider_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, raider_id);
                        self.pending.push(Command::RecallRaid { player_id, raider_id });
                    }
                }
                ClientMsg::MarketBuy { commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketBuy { player_id, commodity, units });
                    }
                }
                ClientMsg::MarketSell { commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketSell { player_id, commodity, units });
                    }
                }
                ClientMsg::PlaceLimitOrder { side, commodity, units, limit_price } => {
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
        let events = self.world.step(&commands);

        // Record true positions into the view filter's history every tick so
        // the retarded-time boundary resolves at full temporal resolution.
        self.history.record(&self.world);
        self.prices.record(&self.world);
        // Queue any discrete events (raid outcomes) for delayed per-player
        // delivery.
        self.reports.ingest(&events);
        // Route economy news to the owning player immediately (their own
        // action / a delivery at their doorstep).
        for ev in &events {
            if let sim::EventPayload::Trade(te) = &ev.payload {
                self.sessions
                    .send_to_player(te.player(), ServerMsg::Trade { trade: *te });
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
        let c = self.world.config.c;
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
        for player_id in self.sessions.online_players() {
            let Some(corp) = self.world.players.get(&player_id) else {
                continue;
            };
            let cc = corp.command_center;
            let ghosts = self.history.view_for(player_id, cc, c, now);
            let anchors = view::filter_anchors(&self.world.home_slots, player_id, cc, c, now);

            // Lagged hub ticker: prices as of the light that has reached this
            // player's command center from the hub.
            let staleness = hub.distance(cc) / c;
            let lagged = self.prices.at(now - staleness);
            let prices = lagged
                .map(|m| {
                    m.iter()
                        .map(|(commodity, price)| PriceView { commodity: *commodity, price: *price })
                        .collect()
                })
                .unwrap_or_default();
            let market = MarketView { prices, staleness };

            // The player's own wallet — fresh (their local treasury + holdings +
            // resting limit orders).
            let wallet = WalletView {
                credits: corp.credits,
                valuation: corp.valuation,
                inventory: corp
                    .inventory
                    .iter()
                    .map(|(commodity, units)| InvSlot { commodity: *commodity, units: *units })
                    .collect(),
                orders: self
                    .world
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
                    .collect(),
            };

            views.insert(
                player_id,
                ServerMsg::View {
                    tick,
                    sim_time: now,
                    command_center: cc,
                    anchors,
                    ghosts,
                    market,
                    wallet,
                },
            );
            let due = self.reports.due_for(player_id, cc, c, now);
            if !due.is_empty() {
                reports.insert(
                    player_id,
                    due.into_iter().map(|r| ServerMsg::Report { report: r }).collect(),
                );
            }
        }

        for (_conn_id, info) in self.sessions.iter_conns() {
            if let Some(view) = views.get(&info.player_id) {
                // Non-blocking: never let one slow client stall the loop; a full
                // queue means the client is behind, so dropping this stale view
                // is correct — the next supersedes it.
                let _ = info.outbound.try_send(view.clone());
            }
            if let Some(reps) = reports.get(&info.player_id) {
                for r in reps {
                    let _ = info.outbound.try_send(r.clone());
                }
            }
        }
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
    let mut game = GameLoop::new(world, persistence, snapshot_every, status_tx);

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
        }
    }

    info!("authoritative game loop stopped");
}
