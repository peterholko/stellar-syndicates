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
use tokio::sync::{mpsc, watch};

use sim::{EntityId, PlayerId};

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

/// Bounded capacity of each connection's outbound queue. This now carries only
/// the low-volume DISCRETE messages (Welcome, Report, Timeline, CommandSignal,
/// EngagementEstimate, GalaxyUpdate, Trade, Error) — the high-frequency per-tick
/// View rides its own last-write-wins [`watch`] channel instead, so a slow
/// client can never accumulate a backlog of stale views here. 64 is ample
/// headroom for the discrete traffic.
pub const OUTBOUND_CAPACITY: usize = 64;

/// §perf Part A: one record's DELIVERY cursor for one connection — how much of
/// it this connection has already been sent. Pure bookkeeping about delivery,
/// never about visibility: what MAY be sent is decided upstream by the view
/// filter (light + fidelity); the cursor only prevents re-sending it.
pub struct RecordCursor {
    /// Rounds already delivered (the record's arrived prefix only ever grows).
    pub rounds_sent: usize,
    /// The outcome has been delivered (sent exactly once, when its light lands).
    pub outcome_sent: bool,
    /// Flagship names as last sent — the one mutable header field (a christening
    /// mid-record re-sends the small header).
    pub names: [Option<String>; 2],
}

/// §perf Part A/B: everything this CONNECTION has already been sent of the
/// change-gated sections. Lives (and dies) with the connection — a reconnect
/// starts empty, so a fresh socket naturally receives full state again.
/// Cursors/signatures are committed ONLY after a successful `try_send`, so a
/// full outbound queue means "retry next broadcast", never silent loss.
#[derive(Default)]
pub struct ConnSentState {
    /// Per-record delivery cursors (records currently known to this connection).
    pub records: HashMap<EntityId, RecordCursor>,
    /// Signature of the standing-orders list as last sent.
    pub standing_sig: Option<u64>,
    /// Signature of the retained battle-reports list as last sent.
    pub reports_sig: Option<u64>,
    /// Signature of the retained capture-reports list as last sent.
    pub captures_sig: Option<u64>,
    /// Signature of the published rankings as last sent.
    pub rankings_sig: Option<u64>,
}

/// Everything the loop needs to know about one live connection.
pub struct ConnInfo {
    pub player_id: PlayerId,
    /// Held per-connection for the player roster surfaced in M3+ views.
    #[allow(dead_code)]
    pub name: String,
    /// The connection's private stream of DISCRETE messages. Bounded so a
    /// stalled client cannot make the server leak memory.
    pub outbound: mpsc::Sender<ServerMsg>,
    /// The connection's LATEST per-player View, last-write-wins. A briefly
    /// stalled client that recovers jumps straight to the current world instead
    /// of replaying a queue of superseded frames (the old bounded mpsc dropped
    /// the *newest* view and kept the stale backlog — exactly backwards).
    pub view_tx: watch::Sender<Option<ServerMsg>>,
    /// §perf: what this connection has already been sent (delta bookkeeping).
    pub sent: ConnSentState,
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
        view_tx: watch::Sender<Option<ServerMsg>>,
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

    /// A clone of one connection's discrete-message sender, for handing to an
    /// off-thread task (e.g. an engagement-estimate rollout) that will deliver a
    /// reply itself. `None` if the connection has since dropped.
    pub fn outbound_of(&self, conn_id: ConnId) -> Option<mpsc::Sender<ServerMsg>> {
        self.conns.get(&conn_id).map(|c| c.outbound.clone())
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

    /// §perf: mutable iteration for the broadcast loop — the per-connection
    /// delta sender advances each connection's delivery cursors in place.
    pub fn iter_conns_mut(&mut self) -> impl Iterator<Item = (&ConnId, &mut ConnInfo)> {
        self.conns.iter_mut()
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
