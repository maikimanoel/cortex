//! Bounded local Git observation adapter for Cortex domain consumers.
//!
//! This entry point exposes existing inspection without a new CLI command or
//! dependency from Core to infrastructure. No stable serialization is promised.

mod inspection;

pub use inspection::{InspectionError, RepositoryObservation, inspect, inspect_candidate};
