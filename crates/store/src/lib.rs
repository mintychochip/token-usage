//! Durable store for usage observations.
//!
//! Same-identity ingest updates the existing total. Different harnesses stay
//! distinct. Persistence is a JSON file replaced atomically, guarded across
//! processes by an advisory `<path>.lock` file (flock); the sidecar is
//! harmless if left behind. Same-identity re-ingest takes new counts but
//! carries forward metadata (`model`, `recorded_at`) the payload omitted.

use fs4::fs_std::FileExt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toktally_domain::{Harness, ObservationIdentity, UsageObservation};

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
}

impl FileStore {
    /// Path of the advisory lock sidecar guarding cross-process access.
    fn lock_path(&self) -> PathBuf {
        let mut s = self.path.as_os_str().to_os_string();
        s.push(".lock");
        PathBuf::from(s)
    }

    /// Run `f` with an exclusive interprocess lock over the store file.
    fn write_locked<T>(
        &self,
        f: impl FnOnce(&mut StoreFile) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let lock = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.lock_path())?;
        lock.lock_exclusive()?;
        let outcome = {
            let mut file = self.load()?;
            let result = f(&mut file);
            if result.is_ok() {
                write_atomic(&self.path, &file)?;
            }
            result
        };
        drop(lock); // release flock before returning
        outcome
    }

    /// Run `f` with a shared interprocess lock over the store file.
    fn read_locked<T>(&self, f: impl FnOnce(&StoreFile) -> T) -> Result<T, StoreError> {
        // create(true) so reads on a brand-new store do not fail on a
        // missing sidecar; the file stays zero-length.
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.lock_path())?;
        lock.lock_shared()?;
        let outcome = Ok(f(&self.load()?));
        drop(lock); // release flock before returning
        outcome
    }

    /// Store location on disk; the lock sidecar is `<path>.lock`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Merge `incoming` over `existing`: counts and classification take the
    /// new value; optional metadata the payload omitted is carried forward.
    fn merge_observation(
        existing: &UsageObservation,
        incoming: UsageObservation,
    ) -> UsageObservation {
        let mut merged = incoming;
        if merged.model().is_none() {
            if let Some(model) = existing.model() {
                merged = merged.with_model(model);
            }
        }
        if merged.recorded_at().is_none() {
            if let Some(at) = existing.recorded_at() {
                merged = merged.with_recorded_at(at);
            }
        }
        merged
    }
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
        Ok(Self { path })
    }

    /// Persist `observation`, stamping `last_synced_at` with the current time.
    pub fn ingest(&self, observation: UsageObservation) -> Result<UsageObservation, StoreError> {
        self.ingest_at(observation, unix_now())
    }

    /// Persist many observations in one atomic write. Same-identity entries are
    /// replaced; new identities are appended. Far cheaper than N `ingest_at` calls.
    pub fn bulk_ingest(&self, observations: Vec<UsageObservation>) -> Result<(), StoreError> {
        self.write_locked(move |file| {
            for observation in observations {
                if let Some(existing) = file
                    .sessions
                    .iter_mut()
                    .find(|row| row.identity() == observation.identity())
                {
                    *existing = Self::merge_observation(existing, observation);
                } else {
                    file.sessions.push(observation);
                }
            }
            Ok(())
        })
    }
    pub fn ingest_at(
        &self,
        observation: UsageObservation,
        last_synced_at: u64,
    ) -> Result<UsageObservation, StoreError> {
        self.write_locked(|file| {
            let observation = observation.with_last_synced_at(last_synced_at);
            if let Some(existing) = file
                .sessions
                .iter_mut()
                .find(|row| row.identity() == observation.identity())
            {
                *existing = Self::merge_observation(existing, observation.clone());
            } else {
                file.sessions.push(observation.clone());
            }
            Ok(observation)
        })
    }

    /// Record that `harness` was scanned at `last_synced_at`.
    pub fn record_harness_sync(
        &self,
        harness: Harness,
        last_synced_at: u64,
    ) -> Result<HarnessSync, StoreError> {
        self.write_locked(|file| {
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
            Ok(record)
        })
    }

    /// Last time this harness's on-disk store was scanned, if ever.
    pub fn harness_last_synced(&self, harness: Harness) -> Result<Option<u64>, StoreError> {
        self.read_locked(|file| {
            file.harness_syncs
                .iter()
                .find(|row| row.harness == harness)
                .map(|row| row.last_synced_at)
        })
    }

    /// True when this harness has never been scanned into the store.
    pub fn needs_first_sync(&self, harness: Harness) -> Result<bool, StoreError> {
        Ok(self.harness_last_synced(harness)?.is_none())
    }

    /// Every harness scan record.
    pub fn list_harness_syncs(&self) -> Result<Vec<HarnessSync>, StoreError> {
        self.read_locked(|file| file.harness_syncs.clone())
    }

    /// Read the stored total for `identity`, if any.
    pub fn get(
        &self,
        identity: &ObservationIdentity,
    ) -> Result<Option<UsageObservation>, StoreError> {
        self.read_locked(|file| {
            file.sessions
                .iter()
                .find(|row| row.identity() == identity)
                .cloned()
        })
    }

    /// Every stored identity, so different harnesses are queryable together.
    pub fn list(&self) -> Result<Vec<UsageObservation>, StoreError> {
        self.read_locked(|file| file.sessions.clone())
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
