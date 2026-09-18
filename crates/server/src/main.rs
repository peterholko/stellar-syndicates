//! Stellar Syndicates authoritative server.
//!
//! Wires the four architectural pieces (§14) together:
//! * the pure `sim` core (the `World`),
//! * a single game-loop task that owns the world and the session registry,
//! * axum + WebSockets as pure I/O,
//! * PostgreSQL accounts and compressed galaxy checkpoints on disk.
//!
//! Configuration via environment:
//! * `PORT`         — HTTP/WS listen port (default 8080)
//! * `BIND_ADDR`    — HTTP/WS bind IP (default 0.0.0.0)
//! * `GALAXY_SEED`  — u64 seed for deterministic generation (default 0xC0FFEE)
//! * `MAX_PLAYERS`  — sizes the galaxy (default 4)
//! * `BOT_PLAYERS`  — optional headless corporations, fresh galaxies only;
//!                   saved bots resume automatically when this is unset
//! * `HOME_RING_SU`  — optional absolute home-ring radius override
//! * `SIM_PACING`    — sim seconds per wall second (default 1; set 4 for fast playtests)
//! * `DATABASE_URL` — account DB fallback; old galaxy snapshots are imported once.
//! * `ACCOUNTS_DATABASE_URL` — account DB (falls back to DATABASE_URL); required.
//! * `APP_ORIGIN` — public HTTPS origin; defaults to localhost for development.
//! * `GALAXY_DATA_DIR` — disk checkpoints (default saves/galaxy-PORT).
//! Full saves every 15 wall minutes + shutdown; `--reset-galaxy` archives them.

mod auth;
mod estimate;
mod game_loop;
mod persistence;
mod protocol;
mod reports;
mod session;
mod timeline;
mod transactions;
mod view;
mod ws;
mod wire;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use axum::extract::{FromRef, State};
use axum::routing::get;
use axum::{Json, Router};
use tokio::sync::{mpsc, watch};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use sim::{SimConfig, World};

use crate::session::{GameHandle, ServerStatus};

/// Shared HTTP state. `GameHandle` drives the game loop (`/ws`); the status
/// receiver exposes session/ops meta (`/status`). Each handler extracts only
/// the part it needs via `FromRef`.
#[derive(Clone)]
struct AppState {
    game: GameHandle,
    status: watch::Receiver<ServerStatus>,
    auth: auth::AuthStore,
}

impl FromRef<AppState> for auth::AuthStore {
    fn from_ref(s: &AppState) -> Self { s.auth.clone() }
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

fn env_positive_f64(key: &str) -> Option<f64> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &f64| v.is_finite() && *v > 0.0)
}

fn pacing_scale() -> f64 {
    // Keep a malformed test setting from creating either a stalled server or a
    // scheduler storm. The bounds are operational, not game-balance tunables.
    env_positive_f64("SIM_PACING")
        .filter(|scale| (0.1..=16.0).contains(scale))
        .unwrap_or(game_loop::DEFAULT_SIM_PACING)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging: respect RUST_LOG, default to info.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let port = env_u64("PORT", 8080) as u16;
    let bind_addr = std::env::var("BIND_ADDR")
        .ok()
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    let seed = env_u64("GALAXY_SEED", 0xC0FFEE);
    let max_players = env_u64("MAX_PLAYERS", 4) as u32;
    let requested_bots = std::env::var("BOT_PLAYERS").ok()
        .map(|value| value.parse::<u32>()).transpose()
        .map_err(|_| anyhow::anyhow!("BOT_PLAYERS must be a non-negative integer"))?;
    let pacing_scale = pacing_scale();
    let mut reset_galaxy = false;
    for arg in std::env::args().skip(1) {
        anyhow::ensure!(arg == "--reset-galaxy", "unknown server argument: {arg}");
        reset_galaxy = true;
    }
    if reset_galaxy {
        anyhow::ensure!(requested_bots.is_none_or(|count| count <= max_players),
            "BOT_PLAYERS must not exceed MAX_PLAYERS; current save was not reset");
    }
    // Authentication and galaxy storage are both mandatory and fail-closed.
    // Do this before starting the world, not after opening a name-only socket.
    let auth = auth::AuthStore::from_env(port).await?;

    let mut config = SimConfig::for_players(seed, max_players);
    // Keep playtest maps reproducible across home-spacing revisions. The normal
    // default preserves the established chart; HOME_RING_SU can restore an
    // archived layout or stage a different spacing without changing any other
    // seeded geography.
    if let Some(home_ring_su) = env_positive_f64("HOME_RING_SU") {
        config.home_ring_frac = (home_ring_su / config.galaxy_radius).clamp(0.01, 0.96);
    }

    let data_dir = std::env::var_os("GALAXY_DATA_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| format!("saves/galaxy-{port}").into());
    let (storage, checkpoint) = persistence::Storage::open(data_dir, reset_galaxy).await?;
    let legacy = if checkpoint.is_none() && !reset_galaxy {
        match std::env::var("DATABASE_URL").ok().filter(|url| !url.trim().is_empty())
            .or_else(|| std::env::var("ACCOUNTS_DATABASE_URL").ok().filter(|url| !url.trim().is_empty())) {
            Some(url) => persistence::load_legacy_world(&url).await?,
            None => None,
        }
    } else { None };
    let world = if checkpoint.is_some() {
        info!("resuming galaxy from last saved checkpoint");
        None
    } else { Some(match legacy {
        Some(mut w) => {
            info!(
                tick = w.tick,
                players = w.players.len(),
                "importing legacy PostgreSQL galaxy snapshot (database copy retained)"
            );
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
                home_ring_su = config.galaxy_radius * config.home_ring_frac,
                c = config.c,
                max_players,
                pacing_scale,
                "initialising fresh galaxy"
            );
            World::new(config)
        }
    }) };

    if std::env::var_os("SNAPSHOT_EVERY_TICKS").is_some() {
        tracing::warn!("SNAPSHOT_EVERY_TICKS is obsolete; full checkpoints run every 15 wall minutes");
    }
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (status_tx, status_rx) = watch::channel(ServerStatus::default());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let handle = GameHandle::new(input_tx);
    // Bind before physics starts; health/WS aren't served until the last saved
    // checkpoint is restored (or a fresh galaxy's initial save is durable).
    let addr = SocketAddr::new(bind_addr, port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let signal_shutdown = shutdown_tx.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = signal_shutdown.send(true);
    });
    let mut game_task = tokio::spawn(game_loop::run(
        world, storage, checkpoint, pacing_scale, requested_bots, status_tx, input_rx,
        shutdown_rx.clone(), ready_tx,
    ));
    tokio::select! {
        result = ready_rx => {
            if result.is_err() { return game_task.await?; }
        }
        result = &mut game_task => return result?,
    }

    let state = AppState {
        game: handle,
        status: status_rx,
        auth: auth.clone(),
    };

    // HTTP / WebSocket surface.
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/status", get(status_handler))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
        .merge(auth::routes(auth))
        // Serve a production client build if present (one-command run); during
        // development the client is served by Vite on its own port instead.
        .fallback_service(ServeDir::new("client/dist"))
        // Same-origin cookies only. Vite proxies the API/WS in development;
        // permissive CORS would be inappropriate for authenticated accounts.
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<axum::body::Body>| {
            // Paths are sufficient for access diagnostics. Never log query
            // strings, cookies, Authorization headers or credential bodies.
            tracing::info_span!("http", method = %request.method(), path = request.uri().path())
        }));

    info!(%addr, "server listening (ws://<host>:{port}/ws)");
    let mut http_shutdown = shutdown_rx;
    let http = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            while !*http_shutdown.borrow() {
                if http_shutdown.changed().await.is_err() { break; }
            }
        }).into_future();
    tokio::pin!(http);
    // Stop on a failed checkpoint instead of allowing an unbounded unsaved
    // interval. SIGTERM/Ctrl-C awaits the final save before ending the process.
    tokio::select! {
        result = &mut game_task => {
            let _ = shutdown_tx.send(true);
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), &mut http).await;
            result??;
        }
        result = &mut http => {
            let _ = shutdown_tx.send(true);
            game_task.await??;
            result?;
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)] {
        let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await.expect("install Ctrl-C handler");
}
