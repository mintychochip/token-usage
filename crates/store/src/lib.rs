//! Durable store for usage observations.
//!
//! Same-identity ingest updates the existing total. Different harnesses stay
//! distinct. Persistence is a JSON file replaced atomically.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use token_usage_domain::{ObservationIdentity, UsageObservation};

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

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoreFile {
    version: u32,
    sessions: Vec<UsageObservation>,
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
        if !path.exists() {
            write_atomic(&path, &StoreFile { version: 1, sessions: Vec::new() })?;
        }
        Ok(Self {
            path,
            lock: Mutex::new(()),
        })
    }

    /// Persist `observation`. A second report for the same identity replaces
    /// the stored totals instead of adding a sibling record.
    pub fn ingest(&self, observation: UsageObservation) -> Result<UsageObservation, StoreError> {
        let _guard = self.lock.lock().expect("store lock");
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

    /// Read the stored total for `identity`, if any.
    pub fn get(&self, identity: &ObservationIdentity) -> Result<Option<UsageObservation>, StoreError> {
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
        let bytes = fs::read(&self.path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn write_atomic(path: &Path, file: &StoreFile) -> Result<(), StoreError> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut out = fs::File::create(&tmp)?;
        let payload = serde_json::to_vec_pretty(file)?;
        out.write_all(&payload)?;
        out.write_all(b"\n")?;
        out.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}
