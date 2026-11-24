//! Execute explicit domain reservations through the canonical program journal.
//!
//! The caller selects the finite population and ordering, including which
//! tasks to reconsider after an accepted dependency or resource observation.
//! This boundary owns no priority, clock, resource pool or permission policy.

use cortex_core::program::domain::DomainEvent;
use cortex_evidence::program::{Command, Operation};
use cortex_storage::{
    history_store::{Completion, StoreKey},
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
        let decision = match store.admit(key, operation) {
            Ok(receipt) if receipt.duplicate => ReservationDecision::AlreadyRetained(receipt),
            Ok(receipt) => ReservationDecision::Reserved(receipt),
            Err(failure)
                if failure.completion != Completion::Indeterminate
                    && matches!(failure.error, ProgramStoreError::Domain(_)) =>
            {
                ReservationDecision::Held(failure)
            }
            Err(failure) => {
                decisions.push(ReservationDecision::Stopped(failure));
                break;
            }
        };
        decisions.push(decision);
    }
    Ok(decisions)
}
