//! Session identity within a harness.

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Opaque session identifier assigned by a harness (or a reserved global id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Reserved id for a harness-wide approximation that is not a real session.
    pub const HARNESS_GLOBAL: &'static str = "__harness_global__";

    /// Parse a non-empty session id.
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptySessionId);
        }
        Ok(SessionId(trimmed.to_string()))
    }

    /// The reserved identity used when a harness reports a global snapshot.
    pub fn harness_global() -> Self {
        SessionId(Self::HARNESS_GLOBAL.to_string())
    }

    /// Whether this is the reserved harness-global identity.
    pub fn is_harness_global(&self) -> bool {
        self.0 == Self::HARNESS_GLOBAL
    }

    /// Borrow the id as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
