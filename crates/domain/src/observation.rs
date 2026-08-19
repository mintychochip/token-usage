//! A single usage observation and its identity.

use serde::{Deserialize, Serialize};

use crate::{Harness, SessionId, UsageCounts};

/// Where an observation came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    /// A per-session plugin / hook report.
    PluginReport,
    /// A harness-wide `/usage` (or equivalent) approximation, not a session log.
    HarnessGlobalApproximation,
}

/// How complete the host's session store is for this observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStoreCompleteness {
    /// The host exposed a complete per-session record.
    Complete,
    /// Some session fields are present; others are missing or inferred.
    Partial,
    /// The host did not expose a usable session store (e.g. Grok Build).
    Unknown,
}

/// Identity of a stored usage total: one harness and one session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationIdentity {
    harness: Harness,
    session_id: SessionId,
}

impl ObservationIdentity {
    /// Bind a session id to a harness.
    pub fn new(harness: Harness, session_id: SessionId) -> Self {
        Self {
            harness,
            session_id,
        }
    }

    /// The harness this identity belongs to.
    pub fn harness(&self) -> Harness {
        self.harness
    }

    /// The session id within that harness.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
}

/// One usage snapshot from a harness plugin or a global approximation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageObservation {
    identity: ObservationIdentity,
    counts: UsageCounts,
    source: ObservationSource,
    completeness: SessionStoreCompleteness,
    /// Unix seconds when this identity was last written to the store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_synced_at: Option<u64>,
    /// Host model id when the payload named one. Used to look up internal prices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

impl UsageObservation {
    /// Construct an observation. All fields are required; extras live on `counts`.
    pub fn new(
        identity: ObservationIdentity,
        counts: UsageCounts,
        source: ObservationSource,
        completeness: SessionStoreCompleteness,
    ) -> Self {
        Self {
            identity,
            counts,
            source,
            completeness,
            last_synced_at: None,
            model: None,
        }
    }

    /// Stamp (or replace) the last-synced time.
    pub fn with_last_synced_at(mut self, unix_seconds: u64) -> Self {
        self.last_synced_at = Some(unix_seconds);
        self
    }

    /// Record the host model id when the payload supplied one.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        let model = model.into();
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            self.model = Some(trimmed.to_string());
        }
        self
    }

    /// Identity used for store lookup and merge.
    pub fn identity(&self) -> &ObservationIdentity {
        &self.identity
    }

    /// Token totals on this observation.
    pub fn counts(&self) -> &UsageCounts {
        &self.counts
    }

    /// Plugin report vs harness-global approximation.
    pub fn source(&self) -> ObservationSource {
        self.source
    }

    /// How complete the host session store was.
    pub fn completeness(&self) -> SessionStoreCompleteness {
        self.completeness
    }

    /// When this identity was last synced, if known.
    pub fn last_synced_at(&self) -> Option<u64> {
        self.last_synced_at
    }

    /// Host model id, if the harness named one.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}
