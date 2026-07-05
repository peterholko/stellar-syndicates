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
use sim::World;
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

    /// Load the most recent full-world snapshot, if any — the basis for a
    /// restart (§14: restart = load latest snapshot, continue forward).
    fn load_latest_world(&self) -> impl std::future::Future<Output = Option<World>> + Send;
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

    async fn load_latest_world(&self) -> Option<World> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT world FROM snapshots ORDER BY tick DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();
        let value = migrate_world_json(row.map(|(v,)| v)?);
        match serde_json::from_value::<World>(value) {
            Ok(w) => {
                info!(tick = w.tick, players = w.players.len(), "restored world from snapshot");
                Some(w)
            }
            Err(e) => {
                warn!(error = %e, "snapshot found but failed to deserialize; starting fresh");
                None
            }
        }
    }
}

/// Migrate a persisted `World` snapshot forward to the current schema
/// (§FLEETS). The single→fleet refactor made two wire changes:
///
///   1. `world.ships` → `world.fleets` (the map key of the entity table);
///   2. each entity gained `composition: {kind: count}` and lost the scalar
///      `kind` field.
///
/// EVERY PERSISTED SHIP BECOMES A FLEET OF ONE: an old entity `{kind: "raider",
/// …}` migrates to `{composition: {"raider": 1}, …}`. serde ignores the leftover
/// `kind`/unknown fields, and the new `damage` pool defaults to empty — so a
/// pre-fleet snapshot restores as an identical N=1 world. Idempotent: a snapshot
/// already in the new shape passes through untouched.
pub fn migrate_world_json(mut value: serde_json::Value) -> serde_json::Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    // (1) Rename the entity table `ships` → `fleets` if the old key is present
    // and the new one isn't.
    if let Some(ships) = obj.remove("ships") {
        obj.entry("fleets").or_insert(ships);
    }
    // (2) Give every entity a composition if it only has a scalar `kind`.
    if let Some(fleets) = obj.get_mut("fleets").and_then(|f| f.as_object_mut()) {
        for entity in fleets.values_mut() {
            let Some(fo) = entity.as_object_mut() else { continue };
            if fo.contains_key("composition") {
                continue; // already a fleet — leave it be (idempotent)
            }
            if let Some(kind) = fo.get("kind").and_then(|k| k.as_str()).map(str::to_owned) {
                let mut comp = serde_json::Map::new();
                comp.insert(kind, serde_json::Value::from(1u32));
                fo.insert("composition".to_string(), serde_json::Value::Object(comp));
            }
        }
    }
    value
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

    async fn load_latest_world(&self) -> Option<World> {
        None // no database — nothing to restore
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
    async fn load_latest_world(&self) -> Option<World> {
        match self {
            AnyPersistence::Pg(p) => p.load_latest_world().await,
            AnyPersistence::Noop(p) => p.load_latest_world().await,
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
/// back to the no-op stub. Runs migrations, **loads the latest world snapshot**
/// (so the galaxy survives a restart, §14), spawns the background persistence
/// task, and returns the handle plus any restored world.
pub async fn init_persistence() -> (PersistenceHandle, Option<World>) {
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

    // Restore the most recent snapshot, if any (the basis for a restart).
    let restored = backend.load_latest_world().await;

    let (tx, rx) = mpsc::channel(PERSIST_CAPACITY);
    tokio::spawn(persistence_task(backend, rx));
    (PersistenceHandle { tx }, restored)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sim::{Command, PlayerId, ShipKind, SimConfig, World};

    #[test]
    fn migrates_old_ship_snapshot_to_fleet_of_one() {
        // An old-shape entity: scalar `kind`, no `composition`, under `ships`.
        let old = json!({
            "tick": 7,
            "ships": {
                "42": { "id": "42", "owner": "1", "kind": "raider", "pos": {"x": 0.0, "y": 0.0} }
            }
        });
        let migrated = migrate_world_json(old);
        let obj = migrated.as_object().unwrap();
        assert!(!obj.contains_key("ships"), "ships key renamed away");
        let fleet = &migrated["fleets"]["42"];
        assert_eq!(fleet["composition"]["raider"], json!(1), "one raider → fleet of one");
    }

    #[test]
    fn migration_is_idempotent_and_new_snapshots_still_load() {
        // A real, current-shape world round-trips through migrate untouched.
        let mut w = World::new(SimConfig::for_players(999, 4));
        w.step(&[Command::AddPlayer { id: PlayerId(7), name: "Ada".into() }]);
        let before = w.fleets.len();
        assert!(before > 0, "join spawns a starting fleet");
        let value = serde_json::to_value(&w).unwrap();
        let restored: World = serde_json::from_value(migrate_world_json(value)).unwrap();
        assert_eq!(restored.fleets.len(), before, "new snapshot survives migrate + reload");
        // Every restored fleet has a non-empty composition (no lost ships).
        assert!(restored.fleets.values().all(|f| f.total_count() >= 1));
        assert!(restored.fleets.values().any(|f| f.contains(ShipKind::Convoy)));
    }
}
