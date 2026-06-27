//! The multiplayer session layer.
//!
//! Connections never touch game state directly. Each connection talks to the
//! single game-loop task through a [`GameHandle`] by sending [`GameInput`]
//! messages; the loop owns the [`Sessions`] registry (so it is lock-free by
//! construction, §14). The registry maps connections to player identities and
//! holds each connection's per-player outbound stream.
//!
//! A player may hold more than one connection at once (e.g. two browser tabs);
//! the registry tracks them per-player so a corporation is only considered
//! "offline" when its last connection drops — important for M6 where a
//! disconnected corporation keeps running on standing orders.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::mpsc;

use sim::PlayerId;

use crate::protocol::{ClientMsg, ServerMsg};

/// Server/ops status — connection-level meta, NOT part of any player's
/// fairness-bound game view. Exposed on the `/status` HTTP endpoint for
/// monitoring and for verifying the session layer. Carrying this on the game
/// `View` would leak join/leave timing faster than light (§6), so it lives here.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ServerStatus {
    pub online_players: usize,
    pub connections: usize,
    pub tick: u64,
    pub sim_time: f64,
}

/// Per-connection identifier, unique for the lifetime of the process.
pub type ConnId = u64;

/// Bounded capacity of each connection's outbound queue. At the ~10 Hz
/// broadcast rate this is several seconds of buffered frames; a client too slow
/// to keep up has its *stale* frames dropped rather than growing memory without
/// bound (each Tick supersedes the last, so dropping old ones is correct).
pub const OUTBOUND_CAPACITY: usize = 64;

/// Everything the loop needs to know about one live connection.
pub struct ConnInfo {
    pub player_id: PlayerId,
    /// Held per-connection for the player roster surfaced in M3+ views.
    #[allow(dead_code)]
    pub name: String,
    /// The connection's private outbound stream. The loop pushes this player's
    /// filtered view here; a writer task forwards it to the socket. Bounded so a
    /// stalled client cannot make the server leak memory.
    pub outbound: mpsc::Sender<ServerMsg>,
}

/// Messages a connection sends to the authoritative loop.
pub enum GameInput {
    /// A connection has identified as a player and is ready to receive its
    /// stream.
    Connect {
        conn_id: ConnId,
        player_id: PlayerId,
        name: String,
        outbound: mpsc::Sender<ServerMsg>,
    },
    /// A connection has closed.
    Disconnect { conn_id: ConnId },
    /// A player intent arrived on a connection.
    Intent { conn_id: ConnId, msg: ClientMsg },
}

/// The connection registry, owned exclusively by the game loop.
#[derive(Default)]
pub struct Sessions {
    conns: HashMap<ConnId, ConnInfo>,
    by_player: HashMap<PlayerId, HashSet<ConnId>>,
}

impl Sessions {
    pub fn new() -> Self {
        Sessions::default()
    }

    /// Register a connection. Returns `true` if this is the player's *first*
    /// live connection (i.e. the corporation just came online).
    pub fn insert(&mut self, conn_id: ConnId, info: ConnInfo) -> bool {
        let player_id = info.player_id;
        self.conns.insert(conn_id, info);
        let set = self.by_player.entry(player_id).or_default();
        let was_offline = set.is_empty();
        set.insert(conn_id);
        was_offline
    }

    /// Remove a connection. Returns the player it belonged to and whether that
    /// player is now fully offline (no remaining connections).
    pub fn remove(&mut self, conn_id: ConnId) -> Option<(PlayerId, bool)> {
        let info = self.conns.remove(&conn_id)?;
        let now_offline = if let Some(set) = self.by_player.get_mut(&info.player_id) {
            set.remove(&conn_id);
            if set.is_empty() {
                self.by_player.remove(&info.player_id);
                true
            } else {
                false
            }
        } else {
            true
        };
        Some((info.player_id, now_offline))
    }

    /// Number of distinct players with at least one live connection.
    pub fn online_player_count(&self) -> usize {
        self.by_player.len()
    }

    /// All distinct players with at least one live connection.
    pub fn online_players(&self) -> Vec<PlayerId> {
        self.by_player.keys().copied().collect()
    }

    /// Number of live connections (may exceed player count).
    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }

    /// Look up the player a connection belongs to. (Used from M3 to route
    /// per-connection intents through the per-player view filter.)
    #[allow(dead_code)]
    pub fn player_of(&self, conn_id: ConnId) -> Option<PlayerId> {
        self.conns.get(&conn_id).map(|c| c.player_id)
    }

    /// Send a message to one specific connection. Non-blocking: if the
    /// connection's queue is full (a stalled client) the message is dropped.
    pub fn send_to_conn(&self, conn_id: ConnId, msg: ServerMsg) {
        if let Some(info) = self.conns.get(&conn_id) {
            let _ = info.outbound.try_send(msg);
        }
    }

    /// Send a message to every live connection of a player (e.g. immediate
    /// feedback for that player's own action).
    pub fn send_to_player(&self, player_id: PlayerId, msg: ServerMsg) {
        if let Some(conns) = self.by_player.get(&player_id) {
            for conn_id in conns {
                self.send_to_conn(*conn_id, msg.clone());
            }
        }
    }

    /// Iterate over every live connection — used to push each one its own
    /// per-player message every broadcast tick.
    pub fn iter_conns(&self) -> impl Iterator<Item = (&ConnId, &ConnInfo)> {
        self.conns.iter()
    }
}

/// A cheap, cloneable handle every connection uses to talk to the game loop.
#[derive(Clone)]
pub struct GameHandle {
    tx: mpsc::UnboundedSender<GameInput>,
    conn_counter: Arc<AtomicU64>,
}

impl GameHandle {
    pub fn new(tx: mpsc::UnboundedSender<GameInput>) -> Self {
        GameHandle {
            tx,
            conn_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Allocate a fresh connection id.
    pub fn next_conn_id(&self) -> ConnId {
        self.conn_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Send an input to the loop. Silently drops if the loop is gone (the
    /// connection will notice via its own closed channel).
    pub fn send(&self, input: GameInput) {
        let _ = self.tx.send(input);
    }
}
