//! Stellar Syndicates authoritative server.
//!
//! Wires the four architectural pieces (§14) together:
//! * the pure `sim` core (the `World`),
//! * a single game-loop task that owns the world and the session registry,
//! * axum + WebSockets as pure I/O,
//! * async Postgres persistence off the hot path.
//!
//! Configuration via environment:
//! * `PORT`         — HTTP/WS listen port (default 8080)
//! * `GALAXY_SEED`  — u64 seed for deterministic generation (default 0xC0FFEE)
//! * `MAX_PLAYERS`  — sizes the galaxy (default 4)
//! * `DATABASE_URL` — Postgres DSN; if unset/unreachable, persistence is a
//!   no-op stub and the server still runs.

mod estimate;
mod game_loop;
mod persistence;
mod protocol;
mod reports;
mod session;
mod timeline;
mod view;
mod ws;

use std::net::SocketAddr;

use axum::extract::{FromRef, State};
use axum::routing::get;
use axum::{Json, Router};
use tokio::sync::{mpsc, watch};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use sim::{SimConfig, World};

use crate::session::{GameHandle, ServerStatus};

/// Shared HTTP state. `GameHandle` drives the game loop (`/ws`); the status
/// receiver exposes session/ops meta (`/status`). Each handler extracts only
/// the part it needs via `FromRef`.
#[derive(Clone)]
struct AppState {
    game: GameHandle,
    status: watch::Receiver<ServerStatus>,
}

impl FromRef<AppState> for GameHandle {
    fn from_ref(s: &AppState) -> Self {
        s.game.clone()
    }
}
impl FromRef<AppState> for watch::Receiver<ServerStatus> {
    fn from_ref(s: &AppState) -> Self {
        s.status.clone()
    }
}

async fn status_handler(State(rx): State<watch::Receiver<ServerStatus>>) -> Json<ServerStatus> {
    Json(rx.borrow().clone())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging: respect RUST_LOG, default to info.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port = env_u64("PORT", 8080) as u16;
    let seed = env_u64("GALAXY_SEED", 0xC0FFEE);
    let max_players = env_u64("MAX_PLAYERS", 4) as u32;

    let config = SimConfig::for_players(seed, max_players);

    // Persistence (off the hot path). Falls back to an in-memory stub if no DB.
    // If a snapshot exists, the galaxy is restored from it (surviving a restart).
    let (persistence, restored) = persistence::init_persistence().await;
    let world = match restored {
        Some(mut w) => {
            info!(tick = w.tick, players = w.players.len(), "resuming galaxy from snapshot");
            // §explore: heal a pre-feature snapshot — recompute band terciles if
            // defaulted, and seed each corp's survey knowledge (owned systems +
            // home radius) so live corps don't wake up amnesiac. Pure fixup;
            // harmless (no-op) on a current snapshot.
            w.fixup_after_load();
            w
        }
        None => {
            info!(
                seed = config.seed,
                galaxy_radius = config.galaxy_radius,
                c = config.c,
                max_players,
                "initialising fresh galaxy"
            );
            World::new(config)
        }
    };

    // Spawn the single authoritative game loop, owning the world.
    let snapshot_every = env_u64("SNAPSHOT_EVERY_TICKS", game_loop::DEFAULT_SNAPSHOT_EVERY);
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (status_tx, status_rx) = watch::channel(ServerStatus::default());
    let handle = GameHandle::new(input_tx);
    tokio::spawn(game_loop::run(
        world,
        persistence,
        snapshot_every,
        status_tx,
        input_rx,
    ));

    let state = AppState {
        game: handle,
        status: status_rx,
    };

    // HTTP / WebSocket surface.
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/status", get(status_handler))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        // Serve a production client build if present (one-command run); during
        // development the client is served by Vite on its own port instead.
        .fallback_service(ServeDir::new("client/dist"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "server listening (ws://<host>:{port}/ws)");
    axum::serve(listener, app).await?;

    Ok(())
}
