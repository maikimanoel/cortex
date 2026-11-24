//! Explicit initial P1 managed-domain driver for a single nominal raw-copy attempt.
//!
//! The independent trial path requires a protected host profile plus distinct
//! kernel-verified owner and observer channels. Inspecting a target, executing
//! one permitted trial and qualifying the result are separate operations.
use super::{
    FileRole, cgroup::DelegationAuthorization, managed::ManagerRecoveryState,
    managed_backend::ManagedExecutionDomain,
};
mod authorization;
pub(crate) mod trust;
use crate::{
    domain_coordination::{ReservationBatchError, ReservationDecision, admit_domain_reservations},
    domain_manager::DomainDiagnostic,
    domain_runtime::{
        DomainManager, DomainOperations, DomainTrialGuard, ExecutableDomainRun,
        prepare_managed_executable_domain_until, release_managed_executable_domain_with_trial,
    },
};
use authorization::AuthorizedTrial;
pub use authorization::RawCopyP1Authority;
use cortex_core::{
    binding::CandidateRequirement,
    program::{
        domain::{DomainEvent, DomainIntent, DomainPhase, DomainRecord, PoolIdentity},
        supervision::ProcessCompletion,
    },
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::program::{Command, CoordinationKernel, KernelError, Operation};
use cortex_storage::{
    history_store::{Access, StoreId, StoreKey},
    program_store::{AdmissionReceipt, ProgramStore},
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    num::NonZeroUsize,
    os::unix::fs::MetadataExt,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const MAX_CASE_INTERVAL: Duration = Duration::from_secs(60);
const MAX_CONTROLLER_BYTES: u64 = 64 * 1024 * 1024;

/// Exact selected single-host target. These are declared expectations for
/// preflight, not authorization or independent containment evidence.
pub struct ManagedP1Target<'a> {
    /// Expected immutable pool identity, including physical host and boot.
    pub pool: &'a PoolIdentity,
    /// Exact delegated root and expected kernel identity.
    pub delegation: &'a DelegationAuthorization,
    /// Exact current bus GUID and manager unique owner, joined by `|`.
    pub manager: &'a str,
    /// Exact typed systemd Manager.Version.
    pub manager_version: &'a str,
}

/// Supplied binding of one proposed P1-01 nominal attempt. This is a description
/// of approval to check with the selected authority, not a qualified grant.
pub struct RawCopyP1Binding<'a> {
    /// Exact selected encrypted ProgramStore incarnation.
    pub store: StoreId,
    /// Exact owner decision receipt for this one fixed managed attempt.
    pub approval: &'a SourceReference,
    /// Exact independent observer selected for controls, FD roles and cleanup.
    pub observer: &'a SourceReference,
    /// Current accepted candidate requirement for the operation.
    pub candidate: &'a CandidateRequirement,
    /// Immutable selected host.
    pub target: ManagedP1Target<'a>,
    /// Expected SHA-256 of the actual running controller binary.
    pub controller_sha256: [u8; 32],
    /// Explicit caller-selected byte limit for hashing that binary.
    pub controller_max_bytes: u64,
    /// Exact pinned raw-copy fixture executable SHA-256.
    pub fixture_sha256: [u8; 32],
    /// Exact approved execution plan digest, covering input and policy.
    pub plan_digest: &'a str,
    /// Exact canonical reservation to offer once, with its expected revision.
    pub reservation: &'a Operation,
    /// Exact existing create, bind, release, report and metadata grant identities.
    pub operations: &'a DomainOperations,
    /// Original monotonic deadline, never restarted at preflight or admission.
    pub deadline: Instant,
}

/// Read-only preflight facts, separately from qualification and authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawCopyP1Preflight {
    /// Actual observed journal revision.
    pub revision: u64,
    /// Exact current controller SHA-256 compared with the request.
    pub controller_sha256: [u8; 32],
}

/// Reasons a nominal managed attempt could not start.
#[derive(Debug)]
pub enum P1Refusal {
    /// A separate exact owner decision and named independent observer are required.
    MissingBinding,
    /// The original attempt ended or cancellation is currently requested.
    DeadlineOrCancellation,
    /// The selected cooperative store deadline does not fit the remaining case time.
    StoreReadBudget,
    /// The reserved inputs, configuration, operation identities or host differ.
    Correspondence,
    /// The journal is unavailable or its history cannot establish precedence.
    Journal,
    /// Current canonical admission refused the claimed input.
    Admission(KernelError),
    /// Actual controller binary differs or cannot be pinned within its bound.
    Controller,
    /// Native delegated controller or manager facts are absent or changed.
    Backend(DomainDiagnostic),
    /// This profile has no integration that authenticates the owner decision
    /// and independently establishes the requested host controls and observer.
    IndependentQualificationUnavailable,
    /// A configured principal refused, revoked, lost its channel or sent a
    /// response that differed from the exact current request.
    TrialAuthority,
    /// Canonical reservation coordination did not select this exact request.
    ReservationSelection(ReservationBatchError),
}

/// Captured raw-copy observation only. A byte match cannot qualify P1-01;
/// the independent peer, SCM_RIGHTS, descriptor and cleanup checks remain separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawCopyP1Observation {
    /// Report and bounded stream/exit observations match the expected copy.
    Matches,
    /// Complete evidence disagrees with the expected copy.
    Differs,
    /// The durable and transport facts do not permit a conclusion.
    Unresolved,
}

/// Actual effects returned by the managed attempt, if the independent gate is
/// eventually supplied. No state here grants a further attempt or cleanup.
pub enum RawCopyP1Run {
    /// Reservation was not freshly selected. No create or release was attempted.
    Reservation(ReservationDecision),
    /// The canonical durable lifecycle and native observations from one attempt.
    Observed {
        /// Fresh reservation receipt.
        reservation: AdmissionReceipt,
        /// Actual durable lifecycle prefix and retained bounded execution evidence.
        run: Box<ExecutableDomainRun>,
        /// Bounded preliminary raw-copy comparison.
        copy: RawCopyP1Observation,
    },
}

/// Read-only lookup of one retained P1 attempt. No bootstrap or authority is
/// reconstructed. An absent manager name cannot settle the retained effect.
pub struct RawCopyP1Recovery {
    /// Latest in-journal phase of the exact attempt.
    pub phase: DomainPhase,
    /// Current manager lookup, when the journal retained an exact binding.
    pub manager: Option<ManagerRecoveryState>,
}

/// P1-09 observation entry for an already retained raw-copy case. The
/// separately bounded observation interval authorizes no effect and does not
/// change the original execution deadline. An ambiguous Create without an
/// exact binding remains unobserved here.
pub fn inspect_raw_copy_recovery(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: &RawCopyP1Binding<'_>,
    observation_deadline: Instant,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Recovery, P1Refusal> {
    if binding.store != store.id() {
        return Err(P1Refusal::Correspondence);
    }
    check_store_budget(store.read_limits().timeout(), observation_deadline, cancel)?;
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| P1Refusal::Journal)?;
    let record = retained_raw_copy(&kernel, binding)?;
    if record.intent.configuration != *backend.configuration()
        || backend.plan().digest() != binding.plan_digest
    {
        return Err(P1Refusal::Correspondence);
    }
    live_budget(observation_deadline, cancel)?;
    let manager = record.binding_observation.as_ref().map(|bound| {
        backend.inspect_recovery(&record.intent, &bound.identity, observation_deadline)
    });
    live_budget(observation_deadline, cancel)?;
    Ok(RawCopyP1Recovery {
        phase: record.phase,
        manager,
    })
}

fn retained_raw_copy<'a>(
    kernel: &'a CoordinationKernel,
    binding: &RawCopyP1Binding<'_>,
) -> Result<&'a DomainRecord, P1Refusal> {
    let Command::Domain(event) = &binding.reservation.command else {
        return Err(P1Refusal::Correspondence);
    };
    let DomainEvent::Reserve { intent, .. } = event.as_ref() else {
        return Err(P1Refusal::Correspondence);
    };
    if intent.id != binding.operations.domain
        || &intent.effect.scope.candidate != binding.candidate
        || &intent.pool != binding.target.pool
        || !kernel
            .operations()
            .iter()
            .any(|entry| entry == binding.reservation)
    {
        return Err(P1Refusal::Correspondence);
    }
    kernel
        .domains()
        .and_then(|domains| domains.domain(&intent.id))
        .filter(|record| &record.intent == intent)
        .ok_or(P1Refusal::Correspondence)
}

fn live_budget(deadline: Instant, cancel: &AtomicBool) -> Result<Duration, P1Refusal> {
    if cancel.load(Ordering::SeqCst) {
        return Err(P1Refusal::DeadlineOrCancellation);
    }
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero() && *remaining <= MAX_CASE_INTERVAL)
        .ok_or(P1Refusal::DeadlineOrCancellation)
}

fn check_store_budget(
    read_timeout: Duration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), P1Refusal> {
    if read_timeout >= live_budget(deadline, cancel)? {
        return Err(P1Refusal::StoreReadBudget);
    }
    Ok(())
}

fn check_binding(binding: &RawCopyP1Binding<'_>) -> Result<(), P1Refusal> {
    if !matches!(
        binding.observer.origin(),
        SourceOrigin::ObservationReference(_)
    ) || binding.observer.origin() == binding.approval.origin()
    {
        return Err(P1Refusal::MissingBinding);
    }
    Ok(())
}

fn reservation_intent<'a>(
    kernel: &CoordinationKernel,
    binding: &RawCopyP1Binding<'a>,
) -> Result<&'a DomainIntent, P1Refusal> {
    let Command::Domain(event) = &binding.reservation.command else {
        return Err(P1Refusal::Correspondence);
    };
    let DomainEvent::Reserve { intent, at } = event.as_ref() else {
        return Err(P1Refusal::Correspondence);
    };
    let ids = binding.operations;
    let operation_ids = [
        &binding.reservation.id,
        &ids.create,
        &ids.bind,
        &ids.release,
        &ids.report,
    ];
    if intent.id != ids.domain
        || &intent.effect.scope.candidate != binding.candidate
        || &intent.pool != binding.target.pool
        || intent.pool.boot != binding.target.delegation.boot
        || binding.reservation.grant != ids.grant
        || *at > ids.at
        || operation_ids
            .iter()
            .enumerate()
            .any(|(i, id)| operation_ids[..i].contains(id))
        || kernel
            .operations()
            .iter()
            .any(|existing| operation_ids.contains(&&existing.id))
    {
        return Err(P1Refusal::Correspondence);
    }
    let preview = kernel
        .preview_admission(binding.reservation)
        .map_err(P1Refusal::Admission)?;
    if preview.is_duplicate() {
        return Err(P1Refusal::Correspondence);
    }
    Ok(intent)
}

fn controller_digest(
    max_bytes: u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<[u8; 32], P1Refusal> {
    if max_bytes == 0 || max_bytes > MAX_CONTROLLER_BYTES {
        return Err(P1Refusal::Controller);
    }
    let file = File::open("/proc/self/exe").map_err(|_| P1Refusal::Controller)?;
    let before = file.metadata().map_err(|_| P1Refusal::Controller)?;
    if !before.is_file() || before.len() == 0 || before.len() > max_bytes {
        return Err(P1Refusal::Controller);
    }
    let mut reader = (&file).take(max_bytes + 1);
    let mut digest = Sha256::new();
    let mut buffer = [0; 8192];
    let mut bytes = 0_u64;
    loop {
        live_budget(deadline, cancel)?;
        let count = reader
            .read(&mut buffer)
            .map_err(|_| P1Refusal::Controller)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        digest.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|_| P1Refusal::Controller)?;
    if bytes != before.len()
        || after.len() != bytes
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(P1Refusal::Controller);
    }
    Ok(digest.finalize().into())
}

/// P1-00 portion of the P1-01 attempt. No reservation, unit or process is
/// created. It binds actual local inputs and the current manager/delegation.
/// Effective ancestor controls, placement, cancellation rights and external
/// observer authentication still require independent qualification.
pub fn inspect_raw_copy_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &ManagedExecutionDomain,
    binding: Option<&RawCopyP1Binding<'_>>,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Preflight, P1Refusal> {
    let binding = binding.ok_or(P1Refusal::MissingBinding)?;
    check_binding(binding)?;
    if binding.store != store.id() {
        return Err(P1Refusal::Correspondence);
    }
    check_store_budget(store.read_limits().timeout(), binding.deadline, cancel)?;
    let mut kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| P1Refusal::Journal)?;
    let intent = reservation_intent(&kernel, binding)?;
    let plan = backend.plan();
    if intent.configuration != *backend.configuration()
        || plan.digest() != binding.plan_digest
        || plan.uses_json_lines()
        || !plan.spec.arguments.is_empty()
        || !plan.spec.environment.is_empty()
        || !plan.spec.artifacts.is_empty()
        || plan.input().len() > 65_536
        || !plan
            .spec
            .files
            .iter()
            .any(|file| file.role == FileRole::Executable && file.sha256 == binding.fixture_sha256)
    {
        return Err(P1Refusal::Correspondence);
    }
    let controller_sha256 =
        controller_digest(binding.controller_max_bytes, binding.deadline, cancel)?;
    if controller_sha256 != binding.controller_sha256 {
        return Err(P1Refusal::Controller);
    }
    let revision = kernel.revision();
    // Inspect the reservation's effects in memory, including the existing plan,
    // live-grant and DiskBytes checks. There is no durable admission here.
    let _ = kernel
        .admit(binding.reservation.clone())
        .map_err(P1Refusal::Admission)?;
    live_budget(binding.deadline, cancel)?;
    backend
        .check_p1_selection(
            binding.target.delegation,
            binding.target.manager,
            binding.target.manager_version,
            binding.deadline,
        )
        .map_err(P1Refusal::Backend)?;
    backend
        .check_context(&kernel, intent, binding.operations.at)
        .map_err(P1Refusal::Backend)?;
    live_budget(binding.deadline, cancel)?;
    Ok(RawCopyP1Preflight {
        revision,
        controller_sha256,
    })
}

/// Compatibility entry. A selection without the independently held authority
/// channels can only complete the read-only preflight.
pub fn run_raw_copy_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: Option<&RawCopyP1Binding<'_>>,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Run, P1Refusal> {
    let binding = binding.ok_or(P1Refusal::MissingBinding)?;
    inspect_raw_copy_attempt(store, key, backend, Some(binding), cancel)?;
    Err(P1Refusal::IndependentQualificationUnavailable)
}

/// Run the one permitted raw-copy trial on an explicitly contained host.
///
/// The root-managed trust profile and both live peers are required before
/// reservation. Create and Release recheck the corresponding scoped decision
/// after canonical intent is durable and before the actual effect. A refused,
/// duplicate or uncertain admission never initiates a new attempt. The trial's
/// observations still require a separate qualification decision.
pub fn run_authorized_raw_copy_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: &RawCopyP1Binding<'_>,
    authority: RawCopyP1Authority,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Run, P1Refusal> {
    inspect_raw_copy_attempt(store, key, backend, Some(binding), cancel)?;
    let trial = AuthorizedTrial::authenticate(store, binding, authority, cancel)?;
    trial.authorize_reservation()?;
    check_store_budget(store.read_limits().timeout(), binding.deadline, cancel)?;
    trial
        .check_live(binding.deadline)
        .map_err(P1Refusal::Backend)?;
    admit_one_and_attempt(store, key, binding.reservation, |reservation| {
        execute_reserved_raw_copy(store, key, backend, binding, cancel, reservation, &trial)
    })
}

fn admit_one_and_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    operation: &Operation,
    attempt: impl FnOnce(AdmissionReceipt) -> RawCopyP1Run,
) -> Result<RawCopyP1Run, P1Refusal> {
    let mut decisions =
        admit_domain_reservations(store, key, vec![operation.clone()], NonZeroUsize::MIN)
            .map_err(P1Refusal::ReservationSelection)?;
    match decisions.pop().ok_or(P1Refusal::Journal)? {
        ReservationDecision::Reserved(receipt) => Ok(attempt(receipt)),
        other => Ok(RawCopyP1Run::Reservation(other)),
    }
}

fn execute_reserved_raw_copy(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: &RawCopyP1Binding<'_>,
    cancel: &AtomicBool,
    reservation: AdmissionReceipt,
    trial: &dyn DomainTrialGuard,
) -> RawCopyP1Run {
    let preparation = prepare_managed_executable_domain_until(
        store,
        key,
        backend,
        binding.operations,
        cancel,
        Some(binding.deadline),
        Some(trial),
    );
    let Some(prepared) = preparation.prepared else {
        return RawCopyP1Run::Observed {
            reservation,
            run: Box::new(ExecutableDomainRun {
                domain: preparation.run,
                execution: None,
            }),
            copy: RawCopyP1Observation::Unresolved,
        };
    };
    let run = release_managed_executable_domain_with_trial(
        store,
        key,
        backend,
        prepared,
        cancel,
        Some(trial),
    );
    let copy = assess_copy(&run, backend.plan().input());
    RawCopyP1Run::Observed {
        reservation,
        run: Box::new(run),
        copy,
    }
}

fn assess_copy(run: &ExecutableDomainRun, input: &[u8]) -> RawCopyP1Observation {
    use RawCopyP1Observation::*;
    if run.domain.report.is_none() || run.domain.diagnostic.is_some() {
        return Unresolved;
    }
    let Some(capture) = &run.execution else {
        return Unresolved;
    };
    let stream = &capture.streams;
    if !stream.input_complete
        || !stream.stdout_complete
        || !stream.stderr_complete
        || capture.domain_populated != Some(false)
    {
        return Unresolved;
    }
    if matches!(
        stream.completion,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        }
    ) && stream.stdout == input
        && stream.stderr.is_empty()
    {
        Matches
    } else {
        Differs
    }
}

#[cfg(test)]
mod tests;
