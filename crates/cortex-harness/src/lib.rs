//! Native supervision of explicitly governed, trusted development fixtures.
//!
//! The Linux path consumes the existing encrypted Workflow journal. It never
//! launches on replay or uncertain admission, creates no permission authority,
//! and supports no AI provider, hostile executor or process-tree isolation.

#[cfg(target_os = "linux")]
mod supervisor;
#[cfg(target_os = "linux")]
pub use supervisor::{
    CaptureIds, HarnessError, PinnedFixture, ReconciledReceipt, RunResult, execute_fixture,
    inspect_fixture,
};
