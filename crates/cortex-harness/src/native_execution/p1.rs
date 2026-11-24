//! Explicit initial P1 managed-domain driver for a single nominal raw-copy attempt.
//!
//! This driver has a callable read-only path. Strict execution additionally
//! requires the selected host's trusted approval and independent observer
//! integration, which the current native profile cannot establish.
use super::{FileRole, cgroup::DelegationAuthorization, managed_backend::ManagedExecutionDomain};
use crate::{
    domain_coordination::{ReservationBatchError, ReservationDecision, admit_domain_reservations},
    domain_manager::DomainDiagnostic,
    domain_runtime::{
        DomainManager, DomainOperations, DomainRun, ExecutableDomainRun,
        prepare_managed_executable_domain_until, release_managed_executable_domain,
    },
};
use cortex_core::{
    binding::CandidateRequirement,
    program::{
        domain::{DomainEvent, DomainIntent, PoolIdentity},
        supervision::ProcessCompletion,
    },
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::program::{Command, CoordinationKernel, KernelError, Operation};
use cortex_storage::{
    history_store::{Access, StoreKey},
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
        /// Actual commit-before-create/bind/release/report prefix.
        lifecycle: Box<DomainRun>,
        /// Bounded preliminary raw-copy comparison.
        copy: RawCopyP1Observation,
    },
}

fn live_budget(deadline: Instant, cancel: &AtomicBool) -> Result<(), P1Refusal> {
    let now = Instant::now();
    if cancel.load(Ordering::SeqCst)
        || deadline
            .checked_duration_since(now)
            .is_none_or(|remaining| remaining.is_zero() || remaining > MAX_CASE_INTERVAL)
    {
        return Err(P1Refusal::DeadlineOrCancellation);
    }
    Ok(())
}

fn check_binding(binding: &RawCopyP1Binding<'_>) -> Result<(), P1Refusal> {
    if !matches!(
        binding.observer.origin(),
        SourceOrigin::ObservationReference(_)
    ) || binding.observer == binding.approval
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
    live_budget(binding.deadline, cancel)?;
    let mut kernel = store
        .load(key, Access::ReadOnly)
        .map_err(|_| P1Refusal::Journal)?;
    let intent = reservation_intent(&kernel, binding)?;
    let plan = backend.plan();
    if intent.configuration != *backend.configuration()
        || plan.digest() != binding.plan_digest
        || plan.uses_json_lines()
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

// This is deliberately a single gate before reservation, reused by the
// sequencing tests. Configuration, caller reference syntax and current local
// metadata alone cannot authenticate this application-specific prerequisite.
fn after_independent_qualification<T>(
    qualified: Result<(), P1Refusal>,
    attempt: impl FnOnce() -> Result<T, P1Refusal>,
) -> Result<T, P1Refusal> {
    qualified?;
    attempt()
}

/// Strict request entry. It requires the exact selected binding and actual
/// readbacks, then refuses until the independent external qualification
/// integration exists. No Boolean, caller-supplied witness trait, or test
/// configuration can turn the current profile into live permission.
pub fn run_raw_copy_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: Option<&RawCopyP1Binding<'_>>,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Run, P1Refusal> {
    let binding = binding.ok_or(P1Refusal::MissingBinding)?;
    inspect_raw_copy_attempt(store, key, backend, Some(binding), cancel)?;
    after_independent_qualification(Err(P1Refusal::IndependentQualificationUnavailable), || {
        execute_raw_copy_attempt(store, key, backend, binding, cancel)
    })
}

fn execute_raw_copy_attempt(
    store: &ProgramStore,
    key: &StoreKey,
    backend: &mut ManagedExecutionDomain,
    binding: &RawCopyP1Binding<'_>,
    cancel: &AtomicBool,
) -> Result<RawCopyP1Run, P1Refusal> {
    live_budget(binding.deadline, cancel)?;
    let mut decisions = admit_domain_reservations(
        store,
        key,
        vec![binding.reservation.clone()],
        NonZeroUsize::MIN,
    )
    .map_err(P1Refusal::ReservationSelection)?;
    let decision = decisions.pop().ok_or(P1Refusal::Journal)?;
    let ReservationDecision::Reserved(reservation) = decision else {
        return Ok(RawCopyP1Run::Reservation(decision));
    };
    let preparation = prepare_managed_executable_domain_until(
        store,
        key,
        backend,
        binding.operations,
        cancel,
        Some(binding.deadline),
    );
    let Some(prepared) = preparation.prepared else {
        return Ok(RawCopyP1Run::Observed {
            reservation,
            lifecycle: Box::new(preparation.run),
            copy: RawCopyP1Observation::Unresolved,
        });
    };
    let run = release_managed_executable_domain(store, key, backend, prepared, cancel);
    let copy = assess_copy(&run, backend.plan().input());
    Ok(RawCopyP1Run::Observed {
        reservation,
        lifecycle: Box::new(run.domain),
        copy,
    })
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
