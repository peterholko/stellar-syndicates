//! Galaxy persistence: compressed binary disk checkpoints every 15 minutes and
//! on clean shutdown. PostgreSQL owns accounts, independently of galaxy saves.
//! The old JSON snapshot reader exists only for a one-time, fail-closed import.

pub(crate) mod store;
pub(crate) use store::Storage;

use anyhow::Context;
use sim::World;
use sqlx::postgres::PgPoolOptions;

pub async fn load_legacy_world(url: &str) -> anyhow::Result<Option<World>> {
    let pool = PgPoolOptions::new().max_connections(1).connect(url).await
        .context("checking legacy galaxy storage; refusing to start fresh on a database error")?;
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('snapshots') IS NOT NULL")
        .fetch_one(&pool).await?;
    if !exists { return Ok(None); }
    let row: Option<(serde_json::Value,)> = sqlx::query_as("SELECT world FROM snapshots ORDER BY tick DESC LIMIT 1")
        .fetch_optional(&pool).await?;
    let world = row.map(|(value,)| serde_json::from_value(migrate_world_json(value)))
        .transpose().context("legacy galaxy snapshot is unreadable; it was NOT reset")?;
    pool.close().await;
    Ok(world)
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
            let Some(fo) = entity.as_object_mut() else {
                continue;
            };
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
        assert_eq!(
            fleet["composition"]["raider"],
            json!(1),
            "one raider → fleet of one"
        );
    }

    #[test]
    fn migration_is_idempotent_and_new_snapshots_still_load() {
        // A real, current-shape world round-trips through migrate untouched.
        let mut w = World::new(SimConfig::for_players(999, 4));
        w.step(&[Command::AddPlayer {
            id: PlayerId(7),
            name: "Ada".into(),
        }]);
        let before = w.fleets.len();
        assert!(before > 0, "join spawns a starting fleet");
        let value = serde_json::to_value(&w).unwrap();
        let restored: World = serde_json::from_value(migrate_world_json(value)).unwrap();
        assert_eq!(
            restored.fleets.len(),
            before,
            "new snapshot survives migrate + reload"
        );
        // Every restored fleet has a non-empty composition (no lost ships).
        assert!(restored.fleets.values().all(|f| f.total_count() >= 1));
        // §founding: the reduced starting roster is one Interceptor (the
        // save-compatible internal kind remains Raider).
        assert!(
            restored
                .fleets
                .values()
                .any(|f| f.contains(ShipKind::Raider))
        );
    }
}
