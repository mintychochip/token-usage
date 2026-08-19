//! Domain-level validation errors.

use thiserror::Error;

/// Failures that occur while constructing domain values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    /// The string does not name a supported harness.
    #[error("unknown harness: {0}")]
    UnknownHarness(String),
    /// Session identity requires a non-empty id.
    #[error("session id must not be empty")]
    EmptySessionId,
}
