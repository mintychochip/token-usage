//! Flat JSON shape used by the HTTP API and reporter stdout.

use serde::{Deserialize, Serialize};
use token_usage_domain::{
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
        }
    }

    /// Parse a wire body into a domain observation.
    pub fn into_observation(self) -> Result<UsageObservation, DomainError> {
        Ok(UsageObservation::new(
            ObservationIdentity::new(self.harness, SessionId::parse(self.session_id)?),
            UsageCounts::new(self.input_tokens, self.output_tokens).with_extras(self.extras),
            self.source,
            self.completeness,
        ))
    }
}

/// List envelope so clients can query every harness together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSessionList {
    pub sessions: Vec<WireObservation>,
}
