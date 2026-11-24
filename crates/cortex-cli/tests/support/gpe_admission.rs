//! Explicit read-only confirmation for selected operations in owned test stores.
use super::admission_storage as cortex_storage;
use cortex_core::program::operational::{EffectEvent, EffectStatus};
use cortex_evidence::program::{Command, CoordinationKernel, Operation};
use cortex_storage::{
    history_store::{Access, Completion, StoreId, StoreKey},
    program_store::{
        AdmissionFailure, AdmissionReceipt, ProgramProfile, ProgramStore, ProgramStoreError,
        Reconciliation,
    },
};

#[derive(Debug)]
pub(super) enum ObservedAdmission {
    Admitted(AdmissionReceipt),
    Confirmed {
        original: AdmissionFailure,
        receipt: AdmissionReceipt,
    },
}
#[derive(Debug)]
pub(super) enum UnconfirmedAdmission {
    Original(AdmissionFailure),
    Reconciliation {
        original: AdmissionFailure,
        observation: Result<Reconciliation, ProgramStoreError>,
    },
    Reservation {
        original: AdmissionFailure,
        receipt: AdmissionReceipt,
        current_read: Result<bool, Box<ProgramStoreError>>,
    },
}

impl std::fmt::Display for UnconfirmedAdmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Original(original) => write!(f, "original admission {original:?}"),
            Self::Reconciliation {
                original,
                observation,
            } => {
                write!(
                    f,
                    "original admission {original:?}, canonical read {observation:?}"
                )
            }
            Self::Reservation {
                original,
                receipt,
                current_read,
            } => write!(
                f,
                "original admission {original:?}, canonical presence {receipt:?}, current reservation {current_read:?}"
            ),
        }
    }
}

fn metadata_only(command: &Command) -> bool {
    match command {
        Command::Notify(_)
        | Command::Observe(_)
        | Command::Assess(_)
        | Command::Examine(_)
        | Command::InvalidateEvidence { .. }
        | Command::Release(_)
        | Command::OrderingPlan(_)
        | Command::Claim(_)
        | Command::Attempt { .. }
        | Command::Handoff { .. }
        | Command::Amend { .. }
        | Command::Package(_)
        | Command::PrepareExecutorInput(_) => true,
        Command::SupervisedEffect(event) => {
            matches!(event.as_ref(), EffectEvent::PrepareProcess { .. })
        }
        _ => false,
    }
}

/// The read is a separate observation, never a second admission or launch.
/// The closure must call the existing store's read-only reconciliation with the
/// exact retained initial definition and input. Diagnostics are not an input.
pub(super) fn confirm_metadata(
    result: Result<AdmissionReceipt, AdmissionFailure>,
    input: &Operation,
    store: StoreId,
    inspect: impl FnOnce() -> Result<Reconciliation, ProgramStoreError>,
) -> Result<ObservedAdmission, UnconfirmedAdmission> {
    let original = match result {
        Ok(receipt) => return Ok(ObservedAdmission::Admitted(receipt)),
        Err(original) => original,
    };
    if original.completion != Completion::Indeterminate
        || !matches!(original.error, ProgramStoreError::Deadline)
        || !metadata_only(&input.command)
    {
        return Err(UnconfirmedAdmission::Original(original));
    }
    let observation = inspect();
    if let Ok(Reconciliation::Present(receipt)) = observation
        && receipt.store == store
        && receipt.duplicate
        && input.expected.checked_add(1) == Some(receipt.ordinal)
    {
        return Ok(ObservedAdmission::Confirmed { original, receipt });
    }
    Err(UnconfirmedAdmission::Reconciliation {
        original,
        observation,
    })
}

/// Confirm the single fresh reservation in the synthetic unknown-outcome control.
/// A later revision can invalidate scope or release the reservation, so this
/// control requires the exact retained post-reserve revision. The next process
/// preparation and dispatch still run their own canonical currentness checks.
pub(super) fn confirm_synthetic_reservation(
    result: Result<AdmissionReceipt, AdmissionFailure>,
    input: &Operation,
    expected_store: StoreId,
    before: &CoordinationKernel,
    store: &ProgramStore,
    key: &StoreKey,
) -> Result<ObservedAdmission, UnconfirmedAdmission> {
    let original = match result {
        Ok(receipt) => return Ok(ObservedAdmission::Admitted(receipt)),
        Err(original) => original,
    };
    let Command::SupervisedEffect(event) = &input.command else {
        return Err(UnconfirmedAdmission::Original(original));
    };
    let EffectEvent::Reserve { intent, .. } = event.as_ref() else {
        return Err(UnconfirmedAdmission::Original(original));
    };
    if original.completion != Completion::Indeterminate
        || !matches!(original.error, ProgramStoreError::Deadline)
        || store.profile() != ProgramProfile::SupervisedFixturesV3
        || store.id() != expected_store
        || input.expected != before.revision()
        || intent.retry_of.is_some()
    {
        return Err(UnconfirmedAdmission::Original(original));
    }
    let observation = store.reconcile(key, before.initial(), input, Access::ReadOnly);
    let receipt = match observation {
        Ok(Reconciliation::Present(receipt))
            if receipt.store == expected_store
                && receipt.duplicate
                && input.expected.checked_add(1) == Some(receipt.ordinal) =>
        {
            receipt
        }
        _ => {
            return Err(UnconfirmedAdmission::Reconciliation {
                original,
                observation,
            });
        }
    };
    let current_read = store.load(key, Access::ReadOnly).map(|current| {
        // Correlate the fresh projection with the receipt even if another
        // writer advanced the journal between the two independent reads.
        current.revision() == receipt.ordinal
            && current.initial() == before.initial()
            && current.operations().last() == Some(input)
            && current.fixture_effects().effects().iter().any(|record| {
                record.intent == *intent
                    && record.status == EffectStatus::Reserved
                    && record.pending()
                    && record.dispatch_at.is_none()
                    && !record.cancellation_requested
            })
    });
    if matches!(current_read, Ok(true)) {
        return Ok(ObservedAdmission::Confirmed { original, receipt });
    }
    Err(UnconfirmedAdmission::Reservation {
        original,
        receipt,
        current_read: current_read.map_err(Box::new),
    })
}
