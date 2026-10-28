//! Bounded local Git observation adapter for Cortex domain consumers.
//!
//! This library exposes inspection and its typed adapter. The binary separately
//! composes Core/Evidence for the bounded check command. Core remains independent
//! of infrastructure; library types have no serialization compatibility promise.

mod inspection;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod support;

/// Explicit committed JSON reference extraction and candidate-scoped inspection.
#[cfg(target_os = "linux")]
pub mod knowledge;

/// Explicit operator-controlled encrypted canonical-history persistence.
#[cfg(target_os = "linux")]
pub mod history_store;

pub use inspection::{InspectionError, RepositoryObservation, inspect, inspect_candidate};
