//! Execute explicit domain reservations through the canonical program journal.
//!
//! The caller selects the finite population and ordering, including which
//! tasks to reconsider after an accepted dependency or resource observation.
//! This boundary owns no priority, clock, resource pool or permission policy.

use cortex_core::{
    parallel::EdgeDecision,
    program::{OperationId, domain::DomainEvent},
};
use cortex_evidence::program::{Command, Operation};
use cortex_storage::{
    history_store::{Access, Completion, StoreKey},
    program_store::{
        AdmissionFailure, AdmissionReceipt, ProgramProfile, ProgramStore, ProgramStoreError,
    },
};
use std::num::NonZeroUsize;

/// A whole-batch refusal before any journal admission.
#[derive(Debug, PartialEq, Eq)]
pub enum ReservationBatchError {
    /// The explicit operation count exceeds the caller's bound.
    Limit,
    /// The store does not own recoverable execution domains.
    Profile,
    /// An operation at this zero-based index is outside the reservation boundary.
    Command(usize),
}

/// Canonical result of one reservation input, in caller-selected order.
#[derive(Debug)]
pub enum ReservationDecision {
    /// Newly retained finite resource reservation.
    Reserved(AdmissionReceipt),
    /// The identical operation was already retained. No new reservation.
    AlreadyRetained(AdmissionReceipt),
    /// Core or Evidence refused an operation. Independent inputs may proceed.
    Held(AdmissionFailure),
    /// The store could not confirm a safe continuation. Inspect or reconcile
    /// the exact operation before submitting anything that depends on it.
    Stopped(AdmissionFailure),
}

/// A refusal to reconsider a reservation on a particular accepted edge.
#[derive(Debug)]
pub enum EdgeReservationError {
    /// The store does not own recoverable execution domains.
    Profile,
    /// No reservation operation was supplied.
    Command,
    /// The current program journal could not be observed.
    Store(ProgramStoreError),
    /// The supplied expected revision does not name the observed journal cut.
    Revision {
        /// Observed authoritative journal ordinal at the read cut.
        observed: u64,
    },
    /// The specified operation did not release an edge that is still current.
    ReleaseUnavailable,
    /// The reservation is outside the exact released edge's consumer scope.
    DifferentConsumer,
}

/// Reconsider the consumer of one retained, currently released edge.
///
/// The caller supplies an exact new reservation, including its existing
/// authority and expected revision. This checks the causal edge at that
/// revision and submits to the canonical store, which rejects writes at a
/// stale revision and revalidates all required dependency edges and resources.
/// This can be called independently for each affected consumer.
pub fn admit_after_edge_release(
    store: &ProgramStore,
    key: &StoreKey,
    release: &OperationId,
    operation: Operation,
) -> Result<ReservationDecision, EdgeReservationError> {
    use EdgeReservationError as Refusal;
    if store.profile() != ProgramProfile::RecoverableDomainsV4 {
        return Err(Refusal::Profile);
    }
    let Command::Domain(event) = &operation.command else {
        return Err(Refusal::Command);
    };
    let DomainEvent::Reserve { intent, .. } = event.as_ref() else {
        return Err(Refusal::Command);
    };
    let kernel = store.load(key, Access::ReadOnly).map_err(Refusal::Store)?;
    if operation.expected != kernel.revision() {
        return Err(Refusal::Revision {
            observed: kernel.revision(),
        });
    }
    let scope = &intent.effect.scope;
    let member = kernel
        .domain_member(&scope.program)
        .ok_or(Refusal::ReleaseUnavailable)?;
    let retained = member
        .operations()
        .iter()
        .find(|operation| &operation.id == release)
        .ok_or(Refusal::ReleaseUnavailable)?;
    let Command::Release(input) = &retained.command else {
        return Err(Refusal::ReleaseUnavailable);
    };
    let edge = input.contract.scope().edge();
    let current = member
        .state()
        .readiness()
        .edge(edge)
        .map_err(|_| Refusal::ReleaseUnavailable)?;
    // Readiness can rebuild its projection during an amendment. A reused
    // review notification ID must not revive an older durable release.
    if !member
        .decisions()
        .iter()
        .rev()
        .find(|decision| &decision.edge == edge && decision.released)
        .is_some_and(|decision| &decision.operation == release)
        || !matches!(
            current.decision(),
            EdgeDecision::Released { review, .. } if review == input.review.id()
        )
    {
        return Err(Refusal::ReleaseUnavailable);
    }
    if input.contract.scope().consumer().task() != &scope.task {
        return Err(Refusal::DifferentConsumer);
    }
    drop(kernel);
    Ok(admit_reservation(store, key, operation))
}

/// Submit selected, exact `DomainEvent::Reserve` operations once each.
///
/// Every admission reloads under the existing store lock and checks dependencies,
/// package, candidate, grants, resources and expected revision at that cut.
/// Refusals preserve their typed causes. A stale input is never rebased and an
/// indeterminate commit stops the batch. On stop, the unreported suffix was
/// never attempted. A returned receipt authorizes no manager activity by itself.
pub fn admit_domain_reservations(
    store: &ProgramStore,
    key: &StoreKey,
    operations: Vec<Operation>,
    maximum: NonZeroUsize,
) -> Result<Vec<ReservationDecision>, ReservationBatchError> {
    if operations.len() > maximum.get() {
        return Err(ReservationBatchError::Limit);
    }
    if store.profile() != ProgramProfile::RecoverableDomainsV4 {
        return Err(ReservationBatchError::Profile);
    }
    if let Some(index) = operations.iter().position(|operation| {
        !matches!(&operation.command, Command::Domain(event) if matches!(
            event.as_ref(), DomainEvent::Reserve { .. }
        ))
    }) {
        return Err(ReservationBatchError::Command(index));
    }
    let mut decisions = Vec::with_capacity(operations.len());
    for operation in operations {
        let decision = admit_reservation(store, key, operation);
        let stopped = matches!(decision, ReservationDecision::Stopped(_));
        decisions.push(decision);
        if stopped {
            break;
        }
    }
    Ok(decisions)
}

fn admit_reservation(
    store: &ProgramStore,
    key: &StoreKey,
    operation: Operation,
) -> ReservationDecision {
    match store.admit(key, operation) {
        Ok(receipt) if receipt.duplicate => ReservationDecision::AlreadyRetained(receipt),
        Ok(receipt) => ReservationDecision::Reserved(receipt),
        Err(failure)
            if failure.completion != Completion::Indeterminate
                && matches!(failure.error, ProgramStoreError::Domain(_)) =>
        {
            ReservationDecision::Held(failure)
        }
        Err(failure) => ReservationDecision::Stopped(failure),
    }
}
