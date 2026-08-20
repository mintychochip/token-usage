//! Durable store for usage observations.
//!
//! Same-identity ingest updates the existing total. Different harnesses stay
//! distinct. Persistence is a JSON file replaced atomically.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use token_usage_domain::{Harness, ObservationIdentity, UsageObservation};

/// Failures from opening or writing the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize error: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// File-backed observation store. One total per `(harness, session_id)`.
pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

/// When a harness last had its on-disk sessions scanned into the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessSync {
    pub harness: Harness,
    pub last_synced_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoreFile {
    version: u32,
    sessions: Vec<UsageObservation>,
    #[serde(default)]
    harness_syncs: Vec<HarnessSync>,
}

impl FileStore {
    /// Open (or create) a store at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        Ok(Self {
            path,
            lock: Mutex::new(()),
        })
    }

    /// Persist `observation`, stamping `last_synced_at` with the current time.
    pub fn ingest(&self, observation: UsageObservation) -> Result<UsageObservation, StoreError> {
        self.ingest_at(observation, unix_now())
    }

    /// Persist `observation` with an explicit last-synced timestamp.
    pub fn ingest_at(
        &self,
        observation: UsageObservation,
        last_synced_at: u64,
    ) -> Result<UsageObservation, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        let observation = observation.with_last_synced_at(last_synced_at);
        let mut file = self.load()?;
        if let Some(existing) = file
            .sessions
            .iter_mut()
            .find(|row| row.identity() == observation.identity())
        {
            *existing = observation.clone();
        } else {
            file.sessions.push(observation.clone());
        }
        write_atomic(&self.path, &file)?;
        Ok(observation)
    }

    /// Record that `harness` was scanned at `last_synced_at`.
    pub fn record_harness_sync(
        &self,
        harness: Harness,
        last_synced_at: u64,
    ) -> Result<HarnessSync, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        let mut file = self.load()?;
        let record = HarnessSync {
            harness,
            last_synced_at,
        };
        if let Some(existing) = file
            .harness_syncs
            .iter_mut()
            .find(|row| row.harness == harness)
        {
            *existing = record.clone();
        } else {
            file.harness_syncs.push(record.clone());
        }
        write_atomic(&self.path, &file)?;
        Ok(record)
    }

    /// Last time this harness's on-disk store was scanned, if ever.
    pub fn harness_last_synced(&self, harness: Harness) -> Result<Option<u64>, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        Ok(self
            .load()?
            .harness_syncs
            .into_iter()
            .find(|row| row.harness == harness)
            .map(|row| row.last_synced_at))
    }

    /// True when this harness has never been scanned into the store.
    pub fn needs_first_sync(&self, harness: Harness) -> Result<bool, StoreError> {
        Ok(self.harness_last_synced(harness)?.is_none())
    }

    /// Every harness scan record.
    pub fn list_harness_syncs(&self) -> Result<Vec<HarnessSync>, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        Ok(self.load()?.harness_syncs)
    }

    /// Read the stored total for `identity`, if any.
    pub fn get(
        &self,
        identity: &ObservationIdentity,
    ) -> Result<Option<UsageObservation>, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        let file = self.load()?;
        Ok(file
            .sessions
            .into_iter()
            .find(|row| row.identity() == identity))
    }

    /// Every stored identity, so different harnesses are queryable together.
    pub fn list(&self) -> Result<Vec<UsageObservation>, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
        Ok(self.load()?.sessions)
    }

    fn load(&self) -> Result<StoreFile, StoreError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(StoreFile {
                version: 2,
                sessions: Vec::new(),
                harness_syncs: Vec::new(),
            }),
            Err(err) => Err(StoreError::Io(err)),
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_atomic(path: &Path, file: &StoreFile) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = tempfile::Builder::new()
        .prefix(".store")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    let payload = serde_json::to_vec_pretty(file)?;
    tmp.write_all(&payload)?;
    tmp.write_all(b"\n")?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
