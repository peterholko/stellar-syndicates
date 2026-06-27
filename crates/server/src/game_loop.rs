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

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, info};

use sim::{Command, World, DT, TICK_HZ};

use crate::persistence::{to_json, PersistJob, PersistenceHandle};
use crate::protocol::{ClientMsg, ServerMsg};
use crate::session::{ConnInfo, GameInput, Sessions};

/// Push a per-player message every N sim ticks. At 30 Hz, N=3 → ~10 Hz network
/// updates: visibly live without flooding the socket.
const BROADCAST_EVERY: u64 = 3;

/// Default full-world snapshot cadence: every 20 s at the tick rate.
pub const DEFAULT_SNAPSHOT_EVERY: u64 = 20 * TICK_HZ as u64;

struct GameLoop {
    world: World,
    sessions: Sessions,
    /// Commands accumulated since the last tick, applied at the next boundary.
    pending: Vec<Command>,
    persistence: PersistenceHandle,
    /// Take a snapshot every this many ticks.
    snapshot_every: u64,
}

impl GameLoop {
    fn new(world: World, persistence: PersistenceHandle, snapshot_every: u64) -> Self {
        GameLoop {
            world,
            sessions: Sessions::new(),
            pending: Vec::new(),
            persistence,
            snapshot_every: snapshot_every.max(1),
        }
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
                // Greet this connection immediately with its identity + clock.
                self.sessions.send_to_conn(
                    conn_id,
                    ServerMsg::Welcome {
                        player_id,
                        name: name.clone(),
                        tick_hz: TICK_HZ,
                        tick: self.world.tick,
                        sim_time: self.world.time,
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
        }

        if self.world.tick.is_multiple_of(self.snapshot_every) {
            self.persistence.submit(PersistJob::Snapshot {
                tick: self.world.tick,
                time: self.world.time,
                world: to_json(&self.world),
            });
        }
    }

    /// Push every connection its own per-player message. In M1 the content is
    /// the live tick; M3 replaces this with each player's delayed/fogged view.
    fn broadcast(&self) {
        let players_online = self.sessions.online_player_count();
        for (_conn_id, info) in self.sessions.iter_conns() {
            let msg = ServerMsg::Tick {
                tick: self.world.tick,
                sim_time: self.world.time,
                players_online,
            };
            // Non-blocking: never let one slow client stall the authoritative
            // loop. A full queue means the client is behind; dropping this
            // (now-stale) tick is correct — the next one supersedes it.
            let _ = info.outbound.try_send(msg);
        }
    }
}

/// Run the authoritative loop until all [`GameHandle`]s are dropped.
pub async fn run(
    world: World,
    persistence: PersistenceHandle,
    snapshot_every: u64,
    mut rx: mpsc::UnboundedReceiver<GameInput>,
) {
    let mut game = GameLoop::new(world, persistence, snapshot_every);

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
