use super::*;

pub(crate) struct TestDir(pub PathBuf);
impl TestDir {
    pub fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("stellar-storage-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        // Exactly this test's UUID directory, never a live data directory.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn checkpoint_frame_rejects_corruption_truncation_and_trailing_data() {
    let valid = frame(b"snapshot");
    assert_eq!(read_frame(&mut Cursor::new(&valid)).unwrap(), b"snapshot");
    for index in [0, 8, 16, 24, HEADER] {
        let mut bytes = valid.clone();
        bytes[index] ^= 64;
        assert!(read_frame(&mut Cursor::new(bytes)).is_err());
    }
    for len in [0, HEADER - 1, valid.len() - 1] {
        assert!(read_frame(&mut Cursor::new(&valid[..len])).is_err());
    }
    let mut trailing = valid;
    trailing.push(0);
    assert!(read_frame(&mut Cursor::new(trailing)).is_err());
}

#[tokio::test]
async fn first_boot_creates_a_durable_nested_data_directory() {
    let dir = TestDir::new();
    let nested = dir.0.join("saves/galaxy-1234");
    let (store, loaded) = Storage::open(nested.clone(), false).await.unwrap();
    assert!(loaded.is_none());
    store.checkpoint(GalaxyCheckpoint::fixture()).await.unwrap();
    drop(store);
    assert!(Storage::open(nested, false).await.unwrap().1.is_some());
}

#[tokio::test]
async fn binary_checkpoint_roundtrips_and_rejects_a_second_writer() {
    let dir = TestDir::new();
    let (store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    assert!(loaded.is_none());
    let saved = GalaxyCheckpoint::fixture();
    let expected = serde_json::to_value(&saved.world).unwrap();
    store.checkpoint(saved).await.unwrap();
    assert!(
        Storage::open(dir.0.clone(), false).await.is_err(),
        "exclusive galaxy ownership"
    );
    drop(store);
    let (_store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    assert!(serde_json::to_value(&loaded.unwrap().world).unwrap() == expected);
    assert!(
        numbered_files(&dir.0, "journal-", ".ssj")
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn retains_three_generations_and_falls_back_to_the_last_valid_snapshot() {
    let dir = TestDir::new();
    let (store, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    let mut saved = GalaxyCheckpoint::fixture();
    for _ in 0..7 {
        store.checkpoint(saved.clone()).await.unwrap();
        saved.world.step(&[]);
    }
    drop(store);
    let checkpoints = numbered_files(&dir.0, "checkpoint-", ".ssg").unwrap();
    assert_eq!(checkpoints.len(), 3);
    // Two damaged saves fall back to tick 4, not an unsaved reconstruction of 6.
    for (_, path) in checkpoints.iter().rev().take(2) {
        fs::write(path, b"broken checkpoint").unwrap();
    }
    let (store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    let recovered = loaded.unwrap();
    assert_eq!(recovered.world.tick, 4);
    store.checkpoint(recovered).await.unwrap();
    assert!(
        checkpoints[0].1.exists(),
        "keep good fallback while rebuilding backups"
    );
}

#[tokio::test]
async fn missing_or_fully_corrupt_snapshots_fail_closed() {
    let dir = TestDir::new();
    let (store, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    store.checkpoint(GalaxyCheckpoint::fixture()).await.unwrap();
    drop(store);
    let checkpoint = numbered_files(&dir.0, "checkpoint-", ".ssg")
        .unwrap()
        .pop()
        .unwrap()
        .1;
    fs::write(&checkpoint, b"corrupt").unwrap();
    assert!(Storage::open(dir.0.clone(), false).await.is_err());
    fs::remove_file(checkpoint).unwrap();
    assert!(
        Storage::open(dir.0.clone(), false).await.is_err(),
        "initialized galaxy cannot silently reset"
    );
}

#[tokio::test]
async fn reset_archives_only_galaxy_files() {
    let dir = TestDir::new();
    let (store, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    store.checkpoint(GalaxyCheckpoint::fixture()).await.unwrap();
    drop(store);
    let unrelated = dir.0.join("operator-notes.txt");
    fs::write(&unrelated, b"keep").unwrap();
    let journal = dir.0.join("journal-00000000000000000001.ssj");
    fs::write(&journal, b"legacy journal").unwrap();
    let (_store, loaded) = Storage::open(dir.0.clone(), true).await.unwrap();
    assert!(loaded.is_none());
    assert_eq!(fs::read(unrelated).unwrap(), b"keep");
    let archive = fs::read_dir(&dir.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("archive-")
        })
        .unwrap();
    assert_eq!(
        numbered_files(&archive, "checkpoint-", ".ssg")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fs::read(archive.join(journal.file_name().unwrap())).unwrap(),
        b"legacy journal"
    );
    assert!(archive.join("initialized").exists());
    assert!(
        numbered_files(&dir.0, "checkpoint-", ".ssg")
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn checkpoint_publication_failure_leaves_the_old_snapshot_intact() {
    let dir = TestDir::new();
    let (store, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    let mut state = GalaxyCheckpoint::fixture();
    store.checkpoint(state.clone()).await.unwrap();
    state.world.step(&[]);
    let blocked = dir.0.join("checkpoint-00000000000000000002.ssg");
    fs::create_dir(&blocked).unwrap();
    assert!(store.checkpoint(state).await.is_err());
    fs::remove_dir(blocked).unwrap();
    drop(store);
    let (_store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    assert_eq!(loaded.unwrap().world.tick, 0);
}

#[tokio::test]
async fn incomplete_temporary_files_are_ignored_and_partial_checkpoints_fall_back() {
    let dir = TestDir::new();
    let (store, _) = Storage::open(dir.0.clone(), false).await.unwrap();
    store.checkpoint(GalaxyCheckpoint::fixture()).await.unwrap();
    drop(store);
    fs::write(dir.0.join(".checkpoint-2.interrupted.tmp"), b"partial").unwrap();
    fs::write(
        dir.0.join("checkpoint-00000000000000000002.ssg"),
        b"SSSAVE01",
    )
    .unwrap();
    let (store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    assert_eq!(loaded.as_ref().unwrap().world.tick, 0);
    store.checkpoint(loaded.unwrap()).await.unwrap();
    assert!(dir.0.join("checkpoint-00000000000000000003.ssg").exists());
}

#[tokio::test]
async fn old_build_snapshot_loads_without_replaying_or_changing_legacy_journals() {
    let dir = TestDir::new();
    let mut state = serde_json::to_value(GalaxyCheckpoint::fixture()).unwrap();
    state["pacing_scale"] = serde_json::json!(4.0);
    state["sessions"] = serde_json::json!([]);
    state["timeline_sent"] = serde_json::json!([]);
    let legacy = serde_json::json!({
        "format": 1,
        "sequence": 999,
        "replay_build": "a-different-server-build",
        "state": state
    });
    let compressed = zstd::stream::encode_all(encode(&legacy).unwrap().as_slice(), 3).unwrap();
    fs::write(
        dir.0.join("checkpoint-00000000000000000001.ssg"),
        frame(&compressed),
    )
    .unwrap();
    let journal = dir.0.join("journal-00000000000000001000.ssj");
    // Even unreadable old journals cannot block valid snapshot-only recovery.
    fs::write(&journal, b"unknown or damaged journal contents").unwrap();
    let (store, loaded) = Storage::open(dir.0.clone(), false).await.unwrap();
    assert_eq!(loaded.as_ref().unwrap().world.tick, 0);
    store.checkpoint(loaded.unwrap()).await.unwrap();
    assert_eq!(
        fs::read(&journal).unwrap(),
        b"unknown or damaged journal contents"
    );
    let newest = numbered_files(&dir.0, "checkpoint-", ".ssg")
        .unwrap()
        .pop()
        .unwrap()
        .1;
    let saved: serde_json::Value = decode(
        &zstd::stream::decode_all(
            read_frame(&mut File::open(newest).unwrap())
                .unwrap()
                .as_slice(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["format"], FORMAT);
    for field in ["sequence", "replay_build"] {
        assert!(saved.get(field).is_none());
    }
    for field in ["sessions", "pacing_scale", "timeline_sent"] {
        assert!(saved["state"].get(field).is_none());
    }
}

#[tokio::test]
async fn journal_without_a_snapshot_never_generates_a_fresh_galaxy() {
    let dir = TestDir::new();
    fs::write(dir.0.join("journal-00000000000000000001.ssj"), b"legacy").unwrap();
    assert!(Storage::open(dir.0.clone(), false).await.is_err());
}
