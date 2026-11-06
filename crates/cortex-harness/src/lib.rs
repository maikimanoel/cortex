//! Native supervision of explicitly governed, trusted development fixtures.
//!
//! The Linux path consumes the existing encrypted Workflow journal. It never
//! launches on replay or uncertain admission, creates no permission authority,
//! with an optional owned-scope adapter. Actual host containment requires separate
//! qualification; no AI provider or hostile executor is supported.

#[cfg(target_os = "linux")]
pub mod host_preflight;

#[cfg(target_os = "linux")]
pub mod domain_bootstrap;

#[cfg(target_os = "linux")]
mod domain_manager;
#[cfg(target_os = "linux")]
mod domain_runtime;
#[cfg(target_os = "linux")]
pub use domain_manager::{
    DomainCause, DomainDiagnostic, DomainPhaseDiagnostic, ServiceLimits, ServiceSelection,
    SystemdNonceDomain,
};
#[cfg(target_os = "linux")]
pub use domain_runtime::{
    DomainOperations, DomainPreparation, DomainRecoveryObservation, DomainRun, PreparedNonceDomain,
    cancel_nonce_domain, cancel_prepared_nonce_domain, execute_nonce_domain, inspect_nonce_domain,
    observe_nonce_domain_closure, prepare_nonce_domain, release_nonce_domain,
};

#[cfg(target_os = "linux")]
mod owned_scope;
#[cfg(target_os = "linux")]
mod supervisor;
#[cfg(target_os = "linux")]
pub use supervisor::{
    CaptureIds, HarnessError, IdentityCaptureIds, PinnedFixture, ReconciledReceipt, RunResult,
    execute_fixture, execute_fixture_with_identity_checkpoint, inspect_fixture,
};
