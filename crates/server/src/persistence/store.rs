//! Snapshot-only disk store. The live galaxy stays in memory; full snapshots
//! are checksummed MessagePack + zstd, installed by fsync/rename. Three valid
//! generations remain recoverable. No per-command writes or startup replay.

use crate::game_loop::GalaxyCheckpoint;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const FORMAT: u32 = 2;
const SAVE_MAGIC: &[u8; 8] = b"SSSAVE01";
const HEADER: usize = 56; // magic + length + inverted length + SHA-256(length + payload)
const MAX_SAVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const GENERATIONS: usize = 3;

#[derive(Serialize, Deserialize)]
struct Saved {
    format: u32,
    state: GalaxyCheckpoint,
}

struct Disk {
    dir: PathBuf,
    _lock: File,
    generation: u64,
    retained: Vec<u64>, // verified generations, newest first
}

#[derive(Clone)]
pub(crate) struct Storage {
    disk: Arc<Mutex<Disk>>,
}

pub(crate) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    value.serialize(
        &mut rmp_serde::Serializer::new(&mut bytes)
            .with_struct_map()
            .with_human_readable(),
    )?;
    Ok(bytes)
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut cursor = Cursor::new(bytes);
    let mut decoder = rmp_serde::Deserializer::new(&mut cursor).with_human_readable();
    let value = T::deserialize(&mut decoder)?;
    ensure!(
        cursor.position() == bytes.len() as u64,
        "trailing data in persisted payload"
    );
    Ok(value)
}

fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options
        .open(path)
        .with_context(|| format!("open {}", path.display()))?)
}

fn sync_dir(dir: &Path) -> Result<()> {
    Ok(File::open(dir)?.sync_all()?)
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let len = (payload.len() as u64).to_le_bytes();
    let mut hash = Sha256::new();
    hash.update(len);
    hash.update(payload);
    let mut bytes = Vec::with_capacity(HEADER + payload.len());
    bytes.extend_from_slice(SAVE_MAGIC);
    bytes.extend_from_slice(&len);
    // Validate length before allocating/decoding a possibly damaged snapshot.
    bytes.extend_from_slice(&(!(payload.len() as u64)).to_le_bytes());
    bytes.extend_from_slice(&hash.finalize());
    bytes.extend_from_slice(payload);
    bytes
}

fn read_frame(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut header = [0; HEADER];
    reader
        .read_exact(&mut header)
        .context("incomplete checkpoint header")?;
    ensure!(&header[..8] == SAVE_MAGIC, "invalid checkpoint header");
    let len = u64::from_le_bytes(header[8..16].try_into()?);
    ensure!(
        u64::from_le_bytes(header[16..24].try_into()?) == !len,
        "corrupt checkpoint length"
    );
    ensure!(len <= MAX_SAVE_BYTES, "checkpoint exceeds size limit");
    let mut payload = Vec::new();
    reader.take(len).read_to_end(&mut payload)?;
    ensure!(payload.len() as u64 == len, "incomplete checkpoint payload");
    ensure!(reader.read(&mut [0])? == 0, "trailing checkpoint data");
    let mut hash = Sha256::new();
    hash.update(&header[8..16]);
    hash.update(&payload);
    ensure!(
        hash.finalize()[..] == header[24..],
        "checkpoint checksum mismatch"
    );
    Ok(payload)
}

fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let tmp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    let mut file = private_file(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&tmp, dir.join(name))?;
    sync_dir(dir)
}

fn numbered_files(dir: &Path, prefix: &str, suffix: &str) -> Result<Vec<(u64, PathBuf)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if let Some(number) = name
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(suffix))
        {
            let id = number
                .parse::<u64>()
                .context("malformed persistence filename")?;
            ensure!(
                path.symlink_metadata()?.file_type().is_file(),
                "persistence entry is not a regular file"
            );
            files.push((id, path));
        }
    }
    files.sort_by_key(|(id, _)| *id);
    Ok(files)
}

fn read_saved(path: &Path) -> Result<Saved> {
    let compressed = read_frame(&mut File::open(path)?)?;
    let mut bytes = Vec::new();
    zstd::stream::read::Decoder::new(compressed.as_slice())?
        .take(MAX_SAVE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_SAVE_BYTES,
        "checkpoint decompression limit exceeded"
    );
    let saved: Saved = decode(&bytes)?;
    // Format 1 used the same snapshot encoding with extra journal/build fields.
    // Ignore those fields: restoring a saved instant needs schema compatibility,
    // not the old executable or deterministic replay of intervening commands.
    ensure!(
        matches!(saved.format, 1 | FORMAT),
        "unsupported checkpoint format; migration required"
    );
    saved.state.validate()?;
    Ok(saved)
}

impl Storage {
    pub async fn open(dir: PathBuf, reset: bool) -> Result<(Self, Option<GalaxyCheckpoint>)> {
        tokio::task::spawn_blocking(move || Self::open_sync(&dir, reset)).await?
    }

    fn open_sync(dir: &Path, reset: bool) -> Result<(Self, Option<GalaxyCheckpoint>)> {
        let absolute = if dir.is_absolute() {
            dir.to_owned()
        } else {
            std::env::current_dir()?.join(dir)
        };
        let existing_parent = absolute
            .ancestors()
            .find(|path| path.exists())
            .context("galaxy data directory has no existing ancestor")?
            .canonicalize()?;
        let new_directory = !absolute.exists();
        fs::create_dir_all(dir)?;
        let dir = dir.canonicalize()?;
        ensure!(
            dir.parent().is_some(),
            "a filesystem root cannot be a galaxy data directory"
        );
        if new_directory {
            // Persist new parent entries too: fsyncing a save alone doesn't
            // commit its path after power loss during the first boot.
            for parent in dir.ancestors().skip(1) {
                sync_dir(parent)?;
                if parent == existing_parent {
                    break;
                }
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::read_dir(&dir)?.next().is_none() {
                fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
            }
        }
        let lock = private_file(&dir.join("galaxy.lock"))?;
        lock.try_lock()
            .context("galaxy already open by another server (use a separate GALAXY_DATA_DIR)")?;
        let mut checkpoints = numbered_files(&dir, "checkpoint-", ".ssg")?;
        // Legacy journals are never read, replayed, changed or newly created.
        // Recognize their filenames only for recoverable explicit resets and
        // to refuse a fresh galaxy if its only remaining save data is a journal.
        let mut legacy_journals = numbered_files(&dir, "journal-", ".ssj")?;
        if reset
            && (!checkpoints.is_empty()
                || !legacy_journals.is_empty()
                || dir.join("initialized").exists())
        {
            // Move only resolved galaxy files, never account tables or unrelated
            // user files. An explicit reset remains recoverable from this archive.
            let archive = dir.join(format!("archive-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&archive)?;
            for (_, path) in checkpoints.iter().chain(&legacy_journals) {
                fs::rename(
                    path,
                    archive.join(path.file_name().context("galaxy filename")?),
                )?;
            }
            if dir.join("initialized").exists() {
                fs::rename(dir.join("initialized"), archive.join("initialized"))?;
            }
            sync_dir(&archive)?;
            sync_dir(&dir)?;
            tracing::warn!(path = %archive.display(), "explicit galaxy reset; previous saves archived (accounts untouched)");
            checkpoints.clear();
            legacy_journals.clear();
        }
        let generation = checkpoints.last().map_or(0, |(id, _)| *id);
        let mut saved = None;
        let mut retained = Vec::new();
        for (id, path) in checkpoints.iter().rev() {
            match read_saved(path) {
                Ok(state) => {
                    retained.push(*id);
                    if saved.is_none() {
                        saved = Some(state.state);
                    }
                }
                Err(error) => {
                    tracing::error!(path = %path.display(), %error, "checkpoint unreadable; trying retained generation")
                }
            }
        }
        ensure!(
            saved.is_some()
                || (checkpoints.is_empty()
                    && legacy_journals.is_empty()
                    && !dir.join("initialized").exists()),
            "no valid galaxy checkpoint; refusing to silently generate a new galaxy"
        );
        if !legacy_journals.is_empty() {
            tracing::warn!(
                "legacy journals left untouched; progress after the restored checkpoint is intentionally not replayed"
            );
        }
        tracing::info!(path = %dir.display(), generation, "galaxy disk storage ready");
        Ok((
            Self {
                disk: Arc::new(Mutex::new(Disk {
                    dir,
                    _lock: lock,
                    generation,
                    retained,
                })),
            },
            saved,
        ))
    }

    pub fn start_checkpoint(&self, state: GalaxyCheckpoint) -> tokio::task::JoinHandle<Result<()>> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            let tick = state.world.tick;
            let started = std::time::Instant::now();
            state.validate()?;
            let payload = encode(&Saved {
                format: FORMAT,
                state,
            })?;
            ensure!(
                payload.len() as u64 <= MAX_SAVE_BYTES,
                "checkpoint too large"
            );
            let compressed = zstd::stream::encode_all(payload.as_slice(), 3)?;
            ensure!(
                compressed.len() as u64 <= MAX_SAVE_BYTES,
                "compressed checkpoint too large"
            );
            // All encoding and I/O are off the game task. The single writer
            // publishes one complete, internally consistent captured instant.
            let mut disk = this
                .disk
                .lock()
                .map_err(|_| anyhow::anyhow!("storage writer panicked"))?;
            let generation = disk
                .generation
                .checked_add(1)
                .context("checkpoint generation exhausted")?;
            let name = format!("checkpoint-{generation:020}.ssg");
            atomic_write(&disk.dir, &name, &frame(&compressed))?;
            disk.generation = generation;
            disk.retained.insert(0, generation);
            if !disk.dir.join("initialized").exists() {
                atomic_write(&disk.dir, "initialized", b"galaxy-storage-v2\n")?;
            }
            disk.prune()?;
            tracing::info!(
                tick,
                bytes = compressed.len(),
                ms = started.elapsed().as_millis(),
                "galaxy checkpoint saved"
            );
            Ok(())
        })
    }

    pub async fn checkpoint(&self, state: GalaxyCheckpoint) -> Result<()> {
        self.start_checkpoint(state).await?
    }
}

impl Disk {
    fn prune(&mut self) -> Result<()> {
        let checkpoints = numbered_files(&self.dir, "checkpoint-", ".ssg")?;
        if checkpoints.len() <= GENERATIONS || self.retained.len() < GENERATIONS {
            return Ok(());
        }
        // Keep three VALID generations, not just three filenames. A damaged
        // newest file mustn't evict the last good fallback during recovery.
        self.retained.truncate(GENERATIONS);
        let oldest = self.retained[GENERATIONS - 1];
        for (generation, path) in &checkpoints {
            if *generation < oldest {
                fs::remove_file(path)?;
            }
        }
        sync_dir(&self.dir)
    }
}

#[cfg(test)]
pub(crate) mod tests;
