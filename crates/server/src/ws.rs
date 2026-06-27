//! WebSocket I/O — pure plumbing between a socket and the game loop (§14).
//!
//! Each connection: splits the socket, spawns a writer task that drains this
//! connection's private (bounded) outbound channel to the wire and emits
//! keepalive pings, and runs a read loop that turns inbound JSON into
//! [`GameInput`]s for the loop. The first message must be a [`ClientMsg::Join`];
//! everything after is an intent. The handler holds no game state.
//!
//! Robustness (so a flaky client can't strand server resources):
//!   * the outbound channel is **bounded** — a stalled client drops stale
//!     frames instead of growing memory without bound;
//!   * the writer sends periodic **pings**; a healthy browser auto-replies with
//!     a pong, which resets the read deadline;
//!   * the read loop has an **idle timeout** — a half-open (broken-but-not-
//!     closed) connection is detected and torn down instead of hanging forever;
//!   * teardown of the writer task is **time-bounded**.

use std::time::Duration;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{interval, timeout, MissedTickBehavior};
use tracing::{debug, warn};

use crate::protocol::{player_id_from_name, ClientMsg, ServerMsg};
use crate::session::{GameHandle, GameInput, OUTBOUND_CAPACITY};

/// How often the server pings an otherwise-idle connection.
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Tear a connection down if nothing (not even a pong) arrives in this long.
/// Must exceed `PING_INTERVAL` so healthy idle clients (which pong every ping)
/// are never falsely dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Longest a corporation name may be (defends against oversized join frames).
const MAX_NAME_LEN: usize = 64;

/// axum handler: upgrade the HTTP request to a WebSocket.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(handle): State<GameHandle>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, handle))
}

async fn handle_socket(socket: WebSocket, handle: GameHandle) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let conn_id = handle.next_conn_id();

    // This connection's private, bounded outbound stream.
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMsg>(OUTBOUND_CAPACITY);

    // Writer task: forward queued ServerMsgs and emit keepalive pings.
    let mut writer = tokio::spawn(async move {
        let mut ping = interval(PING_INTERVAL);
        ping.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // Skip the immediate first tick.
        ping.tick().await;
        loop {
            tokio::select! {
                maybe = out_rx.recv() => match maybe {
                    Some(msg) => {
                        let json = match serde_json::to_string(&msg) {
                            Ok(j) => j,
                            Err(e) => { warn!(error = %e, "failed to serialise ServerMsg"); continue; }
                        };
                        if ws_tx.send(Message::Text(Utf8Bytes::from(json))).await.is_err() {
                            break;
                        }
                    }
                    None => break, // outbound sender dropped: connection closing
                },
                _ = ping.tick() => {
                    if ws_tx.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ws_tx.close().await;
    });

    let mut joined = false;

    loop {
        // Idle timeout detects half-open connections.
        let frame = match timeout(READ_TIMEOUT, ws_rx.next()).await {
            Ok(Some(Ok(m))) => m,
            Ok(Some(Err(e))) => {
                debug!(conn_id, error = %e, "websocket recv error");
                break;
            }
            Ok(None) => break, // stream ended (clean close)
            Err(_elapsed) => {
                debug!(conn_id, "idle timeout — tearing down half-open connection");
                break;
            }
        };

        match frame {
            Message::Text(text) => {
                match serde_json::from_str::<ClientMsg>(text.as_str()) {
                    Ok(ClientMsg::Join { name }) => {
                        if joined {
                            debug!(conn_id, "duplicate join ignored");
                            continue;
                        }
                        let trimmed = name.trim();
                        if trimmed.is_empty() {
                            let _ = out_tx.try_send(ServerMsg::Error {
                                message: "name must not be empty".into(),
                            });
                            continue;
                        }
                        if trimmed.chars().count() > MAX_NAME_LEN {
                            let _ = out_tx.try_send(ServerMsg::Error {
                                message: format!("name too long (max {MAX_NAME_LEN} characters)"),
                            });
                            continue;
                        }
                        let player_id = player_id_from_name(trimmed);
                        joined = true;
                        handle.send(GameInput::Connect {
                            conn_id,
                            player_id,
                            name: trimmed.to_string(),
                            outbound: out_tx.clone(),
                        });
                    }
                    Ok(other) => {
                        if joined {
                            handle.send(GameInput::Intent { conn_id, msg: other });
                        } else {
                            let _ = out_tx.try_send(ServerMsg::Error {
                                message: "send a Join message first".into(),
                            });
                        }
                    }
                    Err(e) => {
                        let _ = out_tx.try_send(ServerMsg::Error {
                            message: format!("malformed message: {e}"),
                        });
                    }
                }
            }
            Message::Close(_) => break,
            // A pong (reply to our keepalive ping) simply resets the read
            // deadline by virtue of arriving; nothing else to do. axum answers
            // inbound pings automatically.
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
        }
    }

    // Connection is going away. Deregister (updates the online count), then drop
    // our outbound sender so the writer task ends; bound the wait so a wedged
    // socket write can't keep this task alive indefinitely.
    if joined {
        handle.send(GameInput::Disconnect { conn_id });
    }
    drop(out_tx);
    if timeout(Duration::from_secs(2), &mut writer).await.is_err() {
        writer.abort();
    }
    debug!(conn_id, "connection closed");
}
