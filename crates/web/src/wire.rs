//! Flat JSON shape used by the HTTP API and reporter stdout.

use serde::{Deserialize, Serialize};
use toktally_domain::{
    DomainError, ExtraCounts, Harness, ObservationIdentity, ObservationSource, SessionId,
    SessionStoreCompleteness, UsageCounts, UsageObservation,
};

/// Wire representation of a stored observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireObservation {
    pub harness: Harness,
    pub session_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "ExtraCounts::is_empty")]
    pub extras: ExtraCounts,
    pub source: ObservationSource,
    pub completeness: SessionStoreCompleteness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl WireObservation {
    /// Convert a domain observation into the public JSON shape.
    pub fn from_observation(obs: &UsageObservation) -> Self {
        Self {
            harness: obs.identity().harness(),
            session_id: obs.identity().session_id().as_str().to_string(),
            input_tokens: obs.counts().input_tokens(),
            output_tokens: obs.counts().output_tokens(),
            extras: obs.counts().extras().clone(),
            source: obs.source(),
            completeness: obs.completeness(),
            last_synced_at: obs.last_synced_at(),
            recorded_at: obs.recorded_at(),
            model: obs.model().map(str::to_string),
        }
    }

    /// Parse a wire body into a domain observation.
    pub fn into_observation(self) -> Result<UsageObservation, DomainError> {
        let mut obs = UsageObservation::new(
            ObservationIdentity::new(self.harness, SessionId::parse(self.session_id)?),
            UsageCounts::new(self.input_tokens, self.output_tokens).with_extras(self.extras),
            self.source,
            self.completeness,
        );
        if let Some(at) = self.last_synced_at {
            obs = obs.with_last_synced_at(at);
        }
        if let Some(at) = self.recorded_at {
            obs = obs.with_recorded_at(at);
        }
        if let Some(model) = self.model {
            obs = obs.with_model(model);
        }
        Ok(obs)
    }
}

/// List envelope so clients can query every harness together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSessionList {
    pub sessions: Vec<WireObservation>,
}

/// Per-harness scan timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireHarnessSync {
    pub harness: Harness,
    pub last_synced_at: u64,
}

/// Envelope for GET /v1/sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSyncStatus {
    pub harnesses: Vec<WireHarnessSync>,
}

/// Body for POST /v1/sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WireSyncRequest {
    pub harness: Option<String>,
    pub force: bool,
}
