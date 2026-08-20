//! Pure domain types for toktally observations.
//!
//! No HTTP, no filesystem. Adapters translate harness payloads into these
//! types; the store is the write path.

mod counts;
mod error;
mod harness;
mod observation;
mod session;

pub use counts::{ExtraCounts, UsageCounts};
pub use error::DomainError;
pub use harness::Harness;
pub use observation::{
    ObservationIdentity, ObservationSource, SessionStoreCompleteness, UsageObservation,
};
pub use session::SessionId;
