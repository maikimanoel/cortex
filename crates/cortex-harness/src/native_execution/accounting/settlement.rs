//! Collector-derived CPU accounting with a separate authenticated fence report.

use super::{CpuCounterObservation, ManagedCpuObservations, monotonic_cpu_history};
use crate::native_execution::p1::trust::{Role, TrialTrust};
use cortex_core::{
    binding::ObservationId,
    program::{
        CoordinationGrant, OperationId,
        domain::{
            ClosureReceipt, Consumption, ConsumptionObservation, DomainEvent, DomainIntent,
            HistoricalOutcome,
        },
        operational::EffectOutcome,
    },
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::program::{Command, Operation};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{AdmissionFailure, AdmissionReceipt, ProgramStore, ProgramStoreError},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fmt, os::fd::OwnedFd, time::Instant};

/// Which existing canonical operation should carry the observed CPU debit.
#[derive(Clone, Copy)]
pub enum CollectedCpuClosureAction {
    /// Close the domain and record measured CPU.
    Close,
    /// Record measured CPU for a previously closed domain.
    Settle,
}

/// The exact native readback whose coverage the observer must establish.
#[derive(Clone, Copy)]
pub enum CollectedCpuClosureCut {
    /// After a successfully reported managed transport end.
    NormalEnd,
    /// After a durably requested cancellation, regardless of historical result.
    AfterCancellation,
}

/// Selection of one existing canonical closure operation. The amount is derived
/// from the controller's observation and receives no caller-supplied override.
#[derive(Clone)]
pub struct CollectedCpuClosure {
    /// Original durable operation ID proposed by the coordinator.
    pub operation: OperationId,
    /// Exact store predecessor, never automatically replaced.
    pub expected_revision: u64,
    /// Existing canonical operation tick.
    pub at: u64,
    /// Current canonical authority for the selected transition.
    pub grant: CoordinationGrant,
    /// Existing Close or Settle transition.
    pub action: CollectedCpuClosureAction,
    /// Required native lifecycle cut.
    pub cut: CollectedCpuClosureCut,
}

/// Why a collected CPU closure could not be published.
pub enum CollectedCpuClosureError {
    /// The time remaining on the original attempt cannot fit this operation.
    Deadline,
    /// The source, authority, lifecycle or store identity changed.
    Correspondence,
    /// Missing, incomplete, contradictory or still-populated counter evidence.
    Counter,
    /// Protected trust configuration, peer, or complete independent report absent.
    Observer,
    /// Read-only reconstruction failed.
    Store(ProgramStoreError),
    /// Canonical publication refused or became indeterminate. The exact input is
    /// retained for explicit reconciliation and does not authorize a retry.
    Admission {
        /// Input actually offered to the canonical store.
        operation: Box<Operation>,
        /// Original completion classification.
        failure: AdmissionFailure,
    },
}

impl fmt::Debug for CollectedCpuClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deadline => f.write_str("Deadline"),
            Self::Correspondence => f.write_str("Correspondence"),
            Self::Counter => f.write_str("Counter"),
            Self::Observer => f.write_str("Observer"),
            Self::Store(error) => f.debug_tuple("Store").field(error).finish(),
            Self::Admission { failure, .. } => f.debug_tuple("Admission").field(failure).finish(),
        }
    }
}

struct TerminalSelection {
    digest: [u8; 32],
    started_ns: u64,
    first_held_ns: u64,
    before_ns: u64,
    after_ns: u64,
    usage_micros: u64,
}

fn select(
    history: &ManagedCpuObservations,
    request: &CollectedCpuClosure,
) -> Result<TerminalSelection, CollectedCpuClosureError> {
    use CollectedCpuClosureError as Refusal;
    if history.contradictory() || history.incomplete() {
        return Err(Refusal::Counter);
    }
    let terminal = match request.cut {
        CollectedCpuClosureCut::NormalEnd if history.after_cancellation().is_none() => {
            history.end_of_transport()
        }
        CollectedCpuClosureCut::AfterCancellation => history.after_cancellation(),
        CollectedCpuClosureCut::NormalEnd => None,
    }
    .ok_or(Refusal::Counter)?;
    let CpuCounterObservation::LowerBoundMicros(usage_micros) = terminal.counter else {
        return Err(Refusal::Counter);
    };
    let (before_ns, after_ns) = terminal.monotonic_ns.ok_or(Refusal::Counter)?;
    let (first_held_ns, first_end_ns) =
        history.first_held().monotonic_ns.ok_or(Refusal::Counter)?;
    let started_ns = history.started_ns().ok_or(Refusal::Counter)?;
    if terminal.populated != Some(false)
        || started_ns > first_held_ns
        || first_held_ns > first_end_ns
        || first_end_ns > before_ns
        || before_ns > after_ns
    {
        return Err(Refusal::Counter);
    }
    let identity = history.identity();
    let cut = match request.cut {
        CollectedCpuClosureCut::NormalEnd => "normal",
        CollectedCpuClosureCut::AfterCancellation => "cancelled",
    };
    let action = match request.action {
        CollectedCpuClosureAction::Close => "close",
        CollectedCpuClosureAction::Settle => "settle",
    };
    let digest = Sha256::digest(
        serde_json::to_vec(&json!([
            "cortex-collected-cpu-observer-v1",
            history.store().bytes(),
            history.domain().as_str(),
            [
                identity.pool.physical,
                identity.pool.incarnation,
                identity.pool.host,
                identity.pool.boot,
                identity.request.as_str(),
                identity.manager,
                identity.invocation,
                identity.control_group,
                identity.cgroup_device,
                identity.cgroup_inode,
                identity.boot
            ],
            [
                request.operation.as_str(),
                request.grant.id.as_str(),
                request.expected_revision,
                request.at,
                cut,
                action
            ],
            [started_ns, first_held_ns, before_ns, after_ns, usage_micros]
        ]))
        .map_err(|_| Refusal::Correspondence)?,
    )
    .into();
    Ok(TerminalSelection {
        digest,
        started_ns,
        first_held_ns,
        before_ns,
        after_ns,
        usage_micros,
    })
}

pub(in crate::native_execution) fn challenge(
    history: &ManagedCpuObservations,
    request: &CollectedCpuClosure,
) -> Result<[u8; 32], CollectedCpuClosureError> {
    Ok(select(history, request)?.digest)
}

// One ephemeral observer packet. The separate selected observer asserts that
// it verified fresh zero-before-entry, the entire attributed domain (including
// launchers/helpers), absence of live writers, no re-entry, restart or snapshot
// resurrection, and quiescence across the selected held counter read.
fn observer_report(
    selection: &TerminalSelection,
    packet: &[u8],
) -> Result<SourceReference, CollectedCpuClosureError> {
    use CollectedCpuClosureError::Observer;
    let value: Value = serde_json::from_slice(packet).map_err(|_| Observer)?;
    let Some(
        [
            kind,
            digest,
            baseline,
            initial_cpu,
            fence,
            through,
            scope,
            sequence,
        ],
    ) = value.as_array().map(Vec::as_slice)
    else {
        return Err(Observer);
    };
    if kind.as_str() != Some("whole-domain-cpu-fence-v1")
        || digest != &json!(selection.digest)
        || initial_cpu.as_u64() != Some(0)
        || !baseline
            .as_u64()
            .is_some_and(|cut| (selection.started_ns..selection.first_held_ns).contains(&cut))
        || !fence
            .as_u64()
            .is_some_and(|cut| (selection.first_held_ns..=selection.before_ns).contains(&cut))
        || through.as_u64() != Some(selection.after_ns)
    {
        return Err(Observer);
    }
    let observation = ObservationId::new(
        scope.as_str().ok_or(Observer)?,
        sequence.as_u64().ok_or(Observer)?,
    )
    .map_err(|_| Observer)?;
    SourceReference::observation_reference(
        observation,
        "authenticated-managed-cpu-observer",
        "whole-domain-cpu-fence-v1",
    )
    .map_err(|_| Observer)
}

fn admit(
    store: &ProgramStore,
    key: &StoreKey,
    history: &ManagedCpuObservations,
    intent: &DomainIntent,
    request: CollectedCpuClosure,
    packet: &[u8],
    before_submit: impl FnOnce() -> Result<(), CollectedCpuClosureError>,
) -> Result<AdmissionReceipt, CollectedCpuClosureError> {
    use CollectedCpuClosureError as Refusal;
    let selection = select(history, &request)?;
    let evidence = observer_report(&selection, packet)?;
    if store.id() != history.store() || history.domain() != &intent.id {
        return Err(Refusal::Correspondence);
    }
    check_store_budget(store, history.interval().1)?;
    let kernel = store.load(key, Access::ReadOnly).map_err(Refusal::Store)?;
    let record = kernel
        .domains()
        .and_then(|projection| projection.domain(history.domain()))
        .ok_or(Refusal::Correspondence)?;
    let binding = record
        .binding_observation
        .as_ref()
        .ok_or(Refusal::Correspondence)?;
    let outcome_matches = match request.cut {
        CollectedCpuClosureCut::NormalEnd => {
            !record.cancelled
                && record.outcome == HistoricalOutcome::Reported(EffectOutcome::Succeeded)
        }
        CollectedCpuClosureCut::AfterCancellation => record.cancelled,
    };
    if &record.intent != intent
        || record.identity.as_ref() != Some(history.identity())
        || &binding.identity != history.identity()
        || binding.configuration != intent.configuration
        || !outcome_matches
        || !monotonic_cpu_history(record)
        || record
            .closure_observations
            .iter()
            .chain(
                record
                    .closure_tree_observations
                    .iter()
                    .map(|tree| &tree.sample),
            )
            .any(|sample| {
                sample
                    .cpu_micros
                    .is_some_and(|usage| usage > selection.usage_micros)
            })
    {
        return Err(Refusal::Correspondence);
    }
    if matches!(
        binding.basis.origin(),
        SourceOrigin::ObservationReference(_)
    ) && evidence.origin() == binding.basis.origin()
    {
        return Err(Refusal::Observer);
    }
    let receipt = ClosureReceipt {
        domain: history.domain().clone(),
        identity: history.identity().clone(),
        configuration: intent.configuration.clone(),
        scope: intent.effect.scope.clone(),
        whole_domain: true,
        restart_prevented: true,
        snapshot_safe: true,
        quiescent: true,
        released: Vec::new(),
        consumption: vec![ConsumptionObservation {
            dimension: Consumption::CpuExecutionMillis,
            measured: Some(selection.usage_micros.div_ceil(1000)),
        }],
        evidence,
    };
    let event = match request.action {
        CollectedCpuClosureAction::Close => DomainEvent::Closed {
            receipt,
            at: request.at,
        },
        CollectedCpuClosureAction::Settle => DomainEvent::Settle {
            receipt,
            at: request.at,
        },
    };
    let operation = Operation {
        id: request.operation,
        expected: request.expected_revision,
        grant: request.grant,
        command: Command::Domain(Box::new(event)),
    };
    before_submit()?;
    check_store_budget(store, history.interval().1)?;
    store
        .admit(key, operation.clone())
        .map_err(|failure| Refusal::Admission {
            operation: Box::new(operation),
            failure,
        })
}

fn check_store_budget(
    store: &ProgramStore,
    deadline: Instant,
) -> Result<(), CollectedCpuClosureError> {
    if deadline
        .checked_duration_since(Instant::now())
        .is_none_or(|left| left.is_zero() || store.read_limits().timeout() >= left)
    {
        return Err(CollectedCpuClosureError::Deadline);
    }
    Ok(())
}

pub(in crate::native_execution) fn submit(
    store: &ProgramStore,
    key: &StoreKey,
    history: &ManagedCpuObservations,
    intent: &DomainIntent,
    request: CollectedCpuClosure,
    observer: OwnedFd,
) -> Result<AdmissionReceipt, CollectedCpuClosureError> {
    use CollectedCpuClosureError as Refusal;
    if store.id() != history.store() {
        return Err(Refusal::Correspondence);
    }
    let _ = select(history, &request)?;
    let deadline = history.interval().1;
    check_store_budget(store, deadline)?;
    let trust = TrialTrust::load().map_err(|_| Refusal::Observer)?;
    let observer = trust
        .channel(observer, Role::Observer)
        .map_err(|_| Refusal::Observer)?;
    let mut bytes = [0; crate::domain_bootstrap::FRAME_LIMIT];
    let length = trust
        .receive(&observer, &mut bytes, deadline)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut {
                Refusal::Deadline
            } else {
                Refusal::Observer
            }
        })?;
    admit(
        store,
        key,
        history,
        intent,
        request,
        &bytes[..length],
        || trust.check_current().map_err(|_| Refusal::Observer),
    )
}

#[cfg(test)]
pub(super) mod tests;
