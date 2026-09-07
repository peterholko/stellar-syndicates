//! WebSocket I/O — pure plumbing between a socket and the game loop (§14).
//!
//! Each connection: splits the socket, spawns a writer task that drains this
//! connection's private (bounded) outbound channel to the wire and emits
//! keepalive pings, and runs a read loop that turns inbound binary frames into
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

use axum::extract::State;
use axum::http::HeaderMap;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval, timeout};
use tracing::{debug, warn};

use crate::auth::{AuthError, AuthSession, AuthStore, AUTH_REQUIRED_CLOSE_CODE};
use crate::protocol::{ClientMsg, ServerMsg};
use crate::session::{GameHandle, GameInput, OUTBOUND_CAPACITY};
use crate::wire;

/// How often the server pings an otherwise-idle connection.
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// Tear a connection down if nothing (not even a pong) arrives in this long.
/// Must exceed `PING_INTERVAL` so healthy idle clients (which pong every ping)
/// are never falsely dropped.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Private close code: a newer login now owns this corporation's sole session.
/// The browser treats it as terminal instead of entering its reconnect loop.
const SESSION_REPLACED_CLOSE_CODE: u16 = 4001;

fn view_divisor(view_hz: Option<u8>) -> u64 {
    match view_hz {
        Some(5) => 2,
        // Missing or unsupported requests retain the established 10 Hz stream.
        _ => 1,
    }
}

/// axum handler: upgrade the HTTP request to a WebSocket.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(handle): State<GameHandle>,
    State(auth): State<AuthStore>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthError> {
    // Both the HTTP handshake and its Origin are authenticated. Join is only
    // readiness/cadence, never authority to choose a corporation by public name.
    auth.check_origin(&headers)?;
    let session = auth.authenticate(&headers).await?;
    Ok(ws.protocols([wire::SUBPROTOCOL])
        .max_message_size(wire::MAX_CLIENT_FRAME)
        .max_frame_size(wire::MAX_CLIENT_FRAME)
        .on_upgrade(move |socket| handle_socket(socket, handle, session)))
}

/// Serialize one `ServerMsg` and write it to the socket. `Ok(())` means the
/// connection is still healthy. A serialization failure is FATAL: skipping a
/// reliable increment after its cursor advanced would silently lose evidence.
/// Reconnecting resets cursors and resends the permitted history instead.
async fn write_msg(ws_tx: &mut SplitSink<WebSocket, Message>, msg: &ServerMsg) -> Result<(), ()> {
    let bytes = match wire::encode_server(msg) {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(error = %e, "failed to serialise ServerMsg");
            return Err(());
        }
    };
    ws_tx
        .send(Message::Binary(bytes.into()))
        .await
        .map_err(|_| ())
}

async fn handle_socket(mut socket: WebSocket, handle: GameHandle, session: AuthSession) {
    if socket.protocol().is_none_or(|protocol| protocol != wire::SUBPROTOCOL) {
        let _ = socket.send(Message::Close(Some(CloseFrame {
            code: wire::PROTOCOL_CLOSE_CODE,
            reason: Utf8Bytes::from_static("binary protocol required; reload the game"),
        }))).await;
        return;
    }
    let (mut ws_tx, mut ws_rx) = socket.split();
    let conn_id = handle.next_conn_id();

    // This connection's private, bounded stream of DISCRETE messages.
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMsg>(OUTBOUND_CAPACITY);
    // The connection's LATEST View, last-write-wins: a stalled client that
    // recovers skips straight to the current world instead of draining a backlog
    // of stale frames. Seeded empty until the first broadcast.
    let (view_tx, mut view_rx) = watch::channel::<Option<ServerMsg>>(None);
    // A new login for this corporation closes this socket deliberately. A
    // watch signal reaches both halves of the split socket without putting
    // session policy into either I/O loop.
    let (replace_tx, mut reader_replace_rx) = watch::channel(false);
    let mut writer_replace_rx = reader_replace_rx.clone();
    let mut reader_auth_rx = session.revoked;
    let mut writer_auth_rx = reader_auth_rx.clone();
    let expires_in = (session.expires_at - chrono::Utc::now()).to_std().unwrap_or_default();
    let auth_deadline = tokio::time::Instant::now() + expires_in;

    // Writer task: forward the latest View + queued discrete messages, and emit
    // keepalive pings.
    let mut writer = tokio::spawn(async move {
        let mut ping = interval(PING_INTERVAL);
        ping.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // Skip the immediate first tick.
        ping.tick().await;
        loop {
            if *writer_auth_rx.borrow() || tokio::time::Instant::now() >= auth_deadline {
                let _ = ws_tx.send(Message::Close(Some(CloseFrame {
                    code: AUTH_REQUIRED_CLOSE_CODE,
                    reason: Utf8Bytes::from_static("sign in again"),
                }))).await;
                break;
            }
            tokio::select! {
                biased;
                changed = writer_auth_rx.changed() => {
                    if changed.is_err() {
                        let _ = ws_tx.send(Message::Close(Some(CloseFrame {
                            code: AUTH_REQUIRED_CLOSE_CODE,
                            reason: Utf8Bytes::from_static("sign in again"),
                        }))).await;
                        break;
                    }
                    continue;
                },
                _ = tokio::time::sleep_until(auth_deadline) => continue,
                // Latest View — borrow_and_update() hands us only the newest
                // value, so any frames the loop pushed while we were busy writing
                // collapse into this one send.
                changed = view_rx.changed() => match changed {
                    Ok(()) => {
                        let latest = view_rx.borrow_and_update().clone();
                        if let Some(msg) = latest {
                            if write_msg(&mut ws_tx, &msg).await.is_err() { break; }
                        }
                    }
                    Err(_) => break, // view sender dropped: connection closing
                },
                maybe = out_rx.recv() => match maybe {
                    Some(msg) => {
                        if write_msg(&mut ws_tx, &msg).await.is_err() { break; }
                    }
                    None => break, // outbound sender dropped: connection closing
                },
                changed = writer_replace_rx.changed() => match changed {
                    Ok(()) if *writer_replace_rx.borrow_and_update() => {
                        let _ = ws_tx.send(Message::Close(Some(CloseFrame {
                            code: SESSION_REPLACED_CLOSE_CODE,
                            reason: Utf8Bytes::from_static("session replaced by a newer login"),
                        }))).await;
                        break;
                    }
                    Ok(()) => {}
                    Err(_) => break,
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
        if *reader_auth_rx.borrow() || tokio::time::Instant::now() >= auth_deadline { break; }
        // Idle timeout detects half-open connections.
        let incoming = tokio::select! {
            biased;
            _ = reader_auth_rx.changed() => break,
            _ = tokio::time::sleep_until(auth_deadline) => break,
            changed = reader_replace_rx.changed() => match changed {
                Ok(()) if *reader_replace_rx.borrow_and_update() => break,
                Ok(()) => continue,
                Err(_) => break,
            },
            incoming = timeout(READ_TIMEOUT, ws_rx.next()) => incoming,
        };
        let frame = match incoming {
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
            Message::Binary(bytes) => match wire::decode_client(&bytes) {
                Ok(ClientMsg::Join { view_hz, .. }) => {
                    if joined {
                        debug!(conn_id, "duplicate join ignored");
                        continue;
                    }
                    joined = true;
                    handle.send(GameInput::Connect {
                        conn_id,
                        player_id: session.account.player_id(),
                        name: session.account.corporation_name.clone(),
                        outbound: out_tx.clone(),
                        view_tx: view_tx.clone(),
                        replace_tx: replace_tx.clone(),
                        view_divisor: view_divisor(view_hz),
                    });
                }
                Ok(other) => {
                    if joined {
                        handle.send(GameInput::Intent {
                            conn_id,
                            msg: other,
                        });
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
            },
            Message::Text(_) => {
                let _ = out_tx.try_send(ServerMsg::Error {
                    message: "binary protocol required; reload the game".into(),
                });
                break;
            }
            Message::Close(_) => break,
            // A pong (reply to our keepalive ping) simply resets the read
            // deadline by virtue of arriving; nothing else to do. axum answers
            // inbound pings automatically.
            Message::Ping(_) | Message::Pong(_) => {}
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

#[cfg(test)]
mod tests;
