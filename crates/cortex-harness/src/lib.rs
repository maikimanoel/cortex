//! Native supervision of explicitly governed, trusted development fixtures.
//!
//! The Linux path consumes the existing encrypted Workflow journal. It never
//! launches on replay or uncertain admission, creates no permission authority,
//! with an optional owned-scope adapter. Actual host containment requires separate
//! qualification; no AI provider or hostile executor is supported.

#[cfg(target_os = "linux")]
pub mod host_preflight;

#[cfg(target_os = "linux")]
mod owned_scope;
#[cfg(target_os = "linux")]
mod supervisor;
#[cfg(target_os = "linux")]
pub use supervisor::{
    CaptureIds, HarnessError, PinnedFixture, ReconciledReceipt, RunResult, execute_fixture,
    inspect_fixture,
};
