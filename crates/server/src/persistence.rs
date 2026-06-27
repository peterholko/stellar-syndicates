//! Persistence — append-only event log + periodic full-state snapshots, kept
//! strictly **off the hot path** (§14).
//!
//! The game loop never awaits the database. It pushes [`PersistJob`]s into an
//! unbounded channel; a dedicated task drains them and writes to whichever
//! [`Persistence`] backend is configured. If no `DATABASE_URL` is set (or the
//! database is unreachable), we fall back to [`NoopPersistence`] so the server
//! runs with zero database setup — the plan explicitly allows stubbing
//! persistence behind a clean interface and continuing.

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// A unit of work for the persistence task. Cheap to construct on the hot path
/// (just owns already-serialised JSON).
#[derive(Debug)]
pub enum PersistJob {
    /// Events that occurred at a given tick.
    Events {
        tick: u64,
        time: f64,
        events: Vec<serde_json::Value>,
    },
    /// A full-world snapshot.
    Snapshot {
        tick: u64,
        time: f64,
        world: serde_json::Value,
    },
}

/// A persistence backend. Implemented by a real Postgres store and a no-op
/// stub; selection happens at startup via [`init_persistence`].
pub trait Persistence: Send + Sync {
    /// Run any one-time setup (migrations). Called once at startup.
    fn init(&self) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    fn record_events(
        &self,
        tick: u64,
        time: f64,
        events: &[serde_json::Value],
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;

    fn save_snapshot(
        &self,
        tick: u64,
        time: f64,
        world: &serde_json::Value,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

/// Real Postgres-backed persistence via sqlx.
pub struct PgPersistence {
    pool: PgPool,
}

impl Persistence for PgPersistence {
    async fn init(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        info!("postgres migrations applied");
        Ok(())
    }

    async fn record_events(
        &self,
        tick: u64,
        time: f64,
        events: &[serde_json::Value],
    ) -> anyhow::Result<()> {
        for ev in events {
            sqlx::query("INSERT INTO events (tick, sim_time, payload) VALUES ($1, $2, $3)")
                .bind(tick as i64)
                .bind(time)
                .bind(ev)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn save_snapshot(
        &self,
        tick: u64,
        time: f64,
        world: &serde_json::Value,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO snapshots (tick, sim_time, world) VALUES ($1, $2, $3)
             ON CONFLICT (tick) DO UPDATE SET sim_time = EXCLUDED.sim_time, world = EXCLUDED.world",
        )
        .bind(tick as i64)
        .bind(time)
        .bind(world)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// In-memory stub: counts what it would have written and logs. Lets the whole
/// server run without a database.
#[derive(Default)]
pub struct NoopPersistence;

impl Persistence for NoopPersistence {
    async fn init(&self) -> anyhow::Result<()> {
        warn!("persistence: running WITHOUT a database (in-memory stub). Set DATABASE_URL to enable Postgres.");
        Ok(())
    }

    async fn record_events(
        &self,
        _tick: u64,
        _time: f64,
        _events: &[serde_json::Value],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn save_snapshot(
        &self,
        tick: u64,
        _time: f64,
        _world: &serde_json::Value,
    ) -> anyhow::Result<()> {
        tracing::debug!(tick, "persistence(noop): snapshot dropped");
        Ok(())
    }
}

/// Either backend, chosen at runtime. Implements [`Persistence`] by delegating,
/// so the persistence task can hold one concrete type with no `dyn`/`async`
/// trait-object gymnastics.
pub enum AnyPersistence {
    Pg(PgPersistence),
    Noop(NoopPersistence),
}

impl Persistence for AnyPersistence {
    async fn init(&self) -> anyhow::Result<()> {
        match self {
            AnyPersistence::Pg(p) => p.init().await,
            AnyPersistence::Noop(p) => p.init().await,
        }
    }
    async fn record_events(
        &self,
        tick: u64,
        time: f64,
        events: &[serde_json::Value],
    ) -> anyhow::Result<()> {
        match self {
            AnyPersistence::Pg(p) => p.record_events(tick, time, events).await,
            AnyPersistence::Noop(p) => p.record_events(tick, time, events).await,
        }
    }
    async fn save_snapshot(
        &self,
        tick: u64,
        time: f64,
        world: &serde_json::Value,
    ) -> anyhow::Result<()> {
        match self {
            AnyPersistence::Pg(p) => p.save_snapshot(tick, time, world).await,
            AnyPersistence::Noop(p) => p.save_snapshot(tick, time, world).await,
        }
    }
}

/// Bounded persistence backlog. If the database stalls, the game loop keeps
/// running and the backlog is capped here rather than growing without bound;
/// jobs past the cap are dropped (and logged) — acceptable because persistence
/// is off the critical path and the next snapshot re-establishes full state.
const PERSIST_CAPACITY: usize = 4096;

/// A cheap, cloneable handle the game loop uses to enqueue persistence work
/// without ever awaiting the database.
#[derive(Clone)]
pub struct PersistenceHandle {
    tx: mpsc::Sender<PersistJob>,
}

impl PersistenceHandle {
    /// Fire-and-forget. Returns immediately; never blocks the tick loop. Drops
    /// (and warns) if the backlog is full so a slow DB cannot leak memory.
    pub fn submit(&self, job: PersistJob) {
        match self.tx.try_send(job) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("persistence backlog full — dropping job (database too slow?)");
            }
            // Persistence task gone away: drop silently, the game continues.
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

/// Connect to Postgres if `DATABASE_URL` is set and reachable, otherwise fall
/// back to the no-op stub. Spawns the background persistence task and returns a
/// handle for the game loop.
pub async fn init_persistence() -> PersistenceHandle {
    let backend = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => match connect_pg(&url).await {
            Ok(pool) => {
                info!("persistence: connected to Postgres");
                AnyPersistence::Pg(PgPersistence { pool })
            }
            Err(e) => {
                warn!(error = %e, "persistence: Postgres unreachable, falling back to in-memory stub");
                AnyPersistence::Noop(NoopPersistence)
            }
        },
        _ => AnyPersistence::Noop(NoopPersistence),
    };

    if let Err(e) = backend.init().await {
        warn!(error = %e, "persistence: init failed, continuing without durable storage");
    }

    let (tx, rx) = mpsc::channel(PERSIST_CAPACITY);
    tokio::spawn(persistence_task(backend, rx));
    PersistenceHandle { tx }
}

async fn connect_pg(url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await?;
    Ok(pool)
}

/// Drains persistence jobs and writes them to the backend. Errors are logged
/// and swallowed — a persistence failure must never take down the game.
async fn persistence_task(backend: AnyPersistence, mut rx: mpsc::Receiver<PersistJob>) {
    while let Some(job) = rx.recv().await {
        let result = match &job {
            PersistJob::Events { tick, time, events } => {
                backend.record_events(*tick, *time, events).await
            }
            PersistJob::Snapshot { tick, time, world } => {
                backend.save_snapshot(*tick, *time, world).await
            }
        };
        if let Err(e) = result {
            warn!(error = %e, "persistence write failed (continuing)");
        }
    }
}

/// Helper: serialise any value to JSON for storage, logging on failure.
pub fn to_json<T: Serialize>(value: &T) -> serde_json::Value {
    match serde_json::to_value(value) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "failed to serialise value for persistence; storing null");
            serde_json::Value::Null
        }
    }
}
