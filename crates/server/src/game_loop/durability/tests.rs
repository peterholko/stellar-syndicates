use super::*;
use crate::persistence::store::{decode, encode, tests::TestDir};

fn restore(saved: GalaxyCheckpoint, pacing: f64) -> GameLoop {
    let (status, _) = watch::channel(ServerStatus::default());
    let (estimates, _) = mpsc::unbounded_channel();
    GameLoop::restore(saved, pacing, status, estimates)
}

fn fresh() -> GameLoop {
    restore(GalaxyCheckpoint::fixture(), 1.0)
}

fn connect(id: u64, player: u64, divisor: u64) -> GameInput {
    let (outbound, _) = mpsc::channel(256);
    let (view_tx, _) = watch::channel(None);
    let (replace_tx, _) = watch::channel(false);
    GameInput::Connect {
        conn_id: id,
        player_id: PlayerId(player),
        name: format!("Durable Corp {player}"),
        outbound,
        view_tx,
        replace_tx,
        view_divisor: divisor,
    }
}

fn ticks(game: &mut GameLoop, n: u64) {
    for _ in 0..n {
        game.tick();
    }
}

fn normalize(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "frontiers" | "observed_order_plans") {
                    if let Some(array) = value.as_array_mut() {
                        array.sort_by_cached_key(|x| x.to_string());
                    }
                }
                normalize(value);
            }
        }
        serde_json::Value::Array(array) => {
            for value in array {
                normalize(value);
            }
        }
        _ => {}
    }
}

fn picture(game: &GameLoop) -> serde_json::Value {
    let mut value = serde_json::to_value(game.durable_checkpoint()).unwrap();
    normalize(&mut value);
    value
}

#[tokio::test]
async fn crash_restores_only_the_saved_instant_including_orders_and_delayed_light() {
    let dir = TestDir::new();
    let (storage, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    let mut game = fresh();
    game.handle_input(connect(1, 10, 1));
    game.handle_input(connect(2, 20, 2));
    ticks(&mut game, 6);
    let fleet = game
        .world
        .fleets
        .values()
        .find(|f| f.owner == PlayerId(10))
        .unwrap()
        .id;
    let from = game.world.fleets[&fleet].pos;
    game.handle_input(GameInput::Intent {
        conn_id: 1,
        msg: ClientMsg::MoveShip {
            ship_id: fleet,
            dest: from + sim::Vec2::new(12_000.0, 8_000.0),
        },
    });
    ticks(&mut game, 60);
    game.handle_input(GameInput::Intent {
        conn_id: 1,
        msg: ClientMsg::MarketBuy {
            commodity: sim::Commodity::Fuel,
            units: 2,
            max_unit_price: None,
        },
    });
    assert!(!game.pending.is_empty());
    let expected = picture(&game);
    storage.checkpoint(game.durable_checkpoint()).await.unwrap();
    // Advance RAM and process further commands without another save.
    ticks(&mut game, 90);
    game.handle_input(GameInput::Intent {
        conn_id: 1,
        msg: ClientMsg::MoveShip {
            ship_id: fleet,
            dest: from,
        },
    });
    ticks(&mut game, 120);
    assert!(picture(&game) != expected);
    drop(storage); // crash, deliberately no final checkpoint
    let (_storage, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let recovered = restore(loaded.unwrap(), 1.0);
    assert!(
        picture(&recovered) == expected,
        "physics, pending orders and delayed reports roll back together"
    );
    assert_eq!(recovered.sessions.connection_count(), 0);
}

#[tokio::test]
async fn checkpoint_keeps_accepted_commands_waiting_for_the_next_tick() {
    let dir = TestDir::new();
    let (storage, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    let mut game = fresh();
    game.handle_input(connect(1, 10, 1));
    assert_eq!(game.pending.len(), 1);
    storage.checkpoint(game.durable_checkpoint()).await.unwrap();
    drop(storage);
    let (_storage, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let mut recovered = restore(loaded.unwrap(), 1.0);
    assert_eq!(recovered.pending.len(), 1);
    game.handle_input(GameInput::Disconnect { conn_id: 1 });
    recovered.tick();
    game.tick();
    assert_eq!(recovered.world.players.len(), 1);
    assert!(recovered.pending.is_empty());
    assert!(picture(&game) == picture(&recovered));
    recovered.tick();
    assert_eq!(
        recovered.world.players.len(),
        1,
        "saved command executes once"
    );
}

#[test]
fn restore_drops_old_sessions_uses_current_pacing_and_never_fast_forwards() {
    let mut game = fresh();
    game.handle_input(connect(1, 10, 1));
    ticks(&mut game, 6);
    let saved: GalaxyCheckpoint = decode(&encode(&game.durable_checkpoint()).unwrap()).unwrap();
    let recovered = restore(saved, 4.0);
    assert_eq!(recovered.sessions.connection_count(), 0);
    assert_eq!(recovered.pacing_scale, 4.0);
    assert_eq!(recovered.broadcast_every, broadcast_every_for(4.0));
    assert_eq!(recovered.world.tick, game.world.tick);
    assert_eq!(recovered.world.time, game.world.time);
    assert_eq!(recovered.galaxy_instance_id, game.galaxy_instance_id);
    assert!(picture(&recovered) == picture(&game));
}

#[tokio::test]
async fn clean_shutdown_saves_and_commands_do_not_write_between_checkpoints() {
    let dir = TestDir::new();
    let (storage, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let (status, mut status_rx) = watch::channel(ServerStatus::default());
    let (input, input_rx) = mpsc::unbounded_channel();
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (ready, ready_rx) = oneshot::channel();
    let task = tokio::spawn(run(
        Some(fresh().world),
        storage,
        loaded,
        1.0,
        status,
        input_rx,
        shutdown_rx,
        ready,
    ));
    ready_rx.await.unwrap();
    let initial_path = dir.0.join("checkpoint-00000000000000000001.ssg");
    let initial_bytes = std::fs::read(&initial_path).unwrap();
    input.send(connect(1, 10, 1)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while status_rx.borrow().online_players == 0 {
            status_rx.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read(&initial_path).unwrap(), initial_bytes);
    assert!(!dir.0.join("checkpoint-00000000000000000002.ssg").exists());
    assert!(!std::fs::read_dir(&dir.0).unwrap().any(|e| {
        e.unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "ssj")
    }));
    shutdown.send(true).unwrap();
    task.await.unwrap().unwrap();
    let (storage, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let saved = loaded.unwrap();
    assert!(saved.world.players.contains_key(&PlayerId(10)));
    assert!(dir.0.join("checkpoint-00000000000000000002.ssg").exists());
    let instance = saved.reporting.galaxy_instance_id.clone().unwrap();

    // Resuming does not publish a redundant initial save or rotate the backups.
    let (status, _) = watch::channel(ServerStatus::default());
    let (_input, input_rx) = mpsc::unbounded_channel();
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (ready, ready_rx) = oneshot::channel();
    let task = tokio::spawn(run(
        None,
        storage,
        Some(saved),
        1.0,
        status,
        input_rx,
        shutdown_rx,
        ready,
    ));
    ready_rx.await.unwrap();
    assert!(!dir.0.join("checkpoint-00000000000000000003.ssg").exists());
    shutdown.send(true).unwrap();
    task.await.unwrap().unwrap();
    let (_storage, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let recovered = restore(loaded.unwrap(), 1.0);
    assert_eq!(recovered.galaxy_instance_id, instance);
    assert_eq!(recovered.sessions.connection_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn full_checkpoint_interval_is_fifteen_wall_minutes_not_ten_seconds() {
    let start = tokio::time::Instant::now();
    let mut timer = checkpoint_timer();
    assert!(
        tokio::time::timeout(Duration::from_secs(899), timer.tick())
            .await
            .is_err()
    );
    assert_eq!(timer.tick().await, start + Duration::from_secs(900));
    assert_eq!(timer.tick().await, start + Duration::from_secs(1800));
}

#[test]
fn checkpoint_binary_roundtrip_preserves_unmodified_reports_and_full_width_ids() {
    let mut game = fresh();
    game.handle_input(connect(u64::MAX, u64::MAX - 1, 2));
    ticks(&mut game, 3);
    let saved = decode::<GalaxyCheckpoint>(&encode(&game.durable_checkpoint()).unwrap()).unwrap();
    assert!(picture(&restore(saved, 1.0)) == picture(&game));
}
