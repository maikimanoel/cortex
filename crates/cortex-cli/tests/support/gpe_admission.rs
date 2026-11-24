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

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::{TaskId, binding::*, program::*, temporal::SourceReference};
    use std::cell::Cell;

    fn input() -> Operation {
        Operation {
            id: OperationId::new("new-owned-metadata").unwrap(),
            expected: 7,
            grant: CoordinationGrant {
                id: GrantId::new("fixture-grant").unwrap(),
                program: ProgramId::new("new-fixture").unwrap(),
                baseline: ContractRevision::new(1).unwrap(),
                actor: ActorId::new("operator").unwrap(),
                tasks: vec![TaskId::new("A").unwrap()],
                permissions: vec![CoordinationPermission::Claim],
                basis: SourceReference::operator_reported("fixture", "explicit", "1").unwrap(),
            },
            command: Command::Attempt {
                id: AttemptId::new("A-attempt").unwrap(),
                generation: ContractRevision::new(1).unwrap(),
                status: AttemptStatus::Unknown,
                basis: SourceReference::operator_reported("fixture", "unknown", "1").unwrap(),
            },
        }
    }
    fn failed() -> Result<AdmissionReceipt, AdmissionFailure> {
        Err(AdmissionFailure {
            completion: Completion::Indeterminate,
            error: ProgramStoreError::Deadline,
        })
    }
    fn receipt() -> AdmissionReceipt {
        AdmissionReceipt {
            store: StoreId::from_bytes([1; 16]),
            ordinal: 8,
            duplicate: true,
        }
    }
    #[test]
    fn canonical_presence_preserves_original_indeterminate_without_retry() {
        let calls = Cell::new(0);
        let result = confirm_metadata(failed(), &input(), receipt().store, || {
            calls.set(calls.get() + 1);
            Ok(Reconciliation::Present(receipt()))
        })
        .unwrap();
        let ObservedAdmission::Confirmed {
            original,
            receipt: actual,
        } = result
        else {
            panic!("original admission must not become successful")
        };
        assert_eq!(original.completion, Completion::Indeterminate);
        assert!(matches!(original.error, ProgramStoreError::Deadline));
        assert_eq!(actual, receipt());
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn absence_conflict_and_read_error_cannot_confirm_metadata() {
        for (case, observation) in [
            Ok(Reconciliation::Absent),
            Ok(Reconciliation::Conflict),
            Err(ProgramStoreError::Deadline),
        ]
        .into_iter()
        .enumerate()
        {
            let result = confirm_metadata(failed(), &input(), receipt().store, || observation);
            let Err(UnconfirmedAdmission::Reconciliation {
                original,
                observation: actual,
            }) = result
            else {
                panic!("unresolved input became confirmed")
            };
            assert_eq!(original.completion, Completion::Indeterminate);
            assert!(matches!(original.error, ProgramStoreError::Deadline));
            assert!(matches!(
                (case, actual),
                (0, Ok(Reconciliation::Absent))
                    | (1, Ok(Reconciliation::Conflict))
                    | (2, Err(ProgramStoreError::Deadline))
            ));
        }
    }
    #[test]
    fn wrong_store_ordinal_and_duplicate_flag_cannot_supply_a_receipt() {
        for bad in [
            AdmissionReceipt {
                store: StoreId::from_bytes([2; 16]),
                ..receipt()
            },
            AdmissionReceipt {
                ordinal: 9,
                ..receipt()
            },
            AdmissionReceipt {
                duplicate: false,
                ..receipt()
            },
        ] {
            assert!(matches!(
                confirm_metadata(failed(), &input(), receipt().store, || Ok(
                    Reconciliation::Present(bad)
                )),
                Err(UnconfirmedAdmission::Reconciliation { .. })
            ));
        }
        let mut overflow = input();
        overflow.expected = u64::MAX;
        assert!(
            confirm_metadata(failed(), &overflow, receipt().store, || Ok(
                Reconciliation::Present(receipt())
            ))
            .is_err()
        );
    }
    #[test]
    fn ordinary_success_does_not_invoke_reconciliation() {
        let direct = AdmissionReceipt {
            duplicate: false,
            ..receipt()
        };
        let result = confirm_metadata(Ok(direct), &input(), direct.store, || {
            panic!("successful admission was reread")
        })
        .unwrap();
        assert!(matches!(result, ObservedAdmission::Admitted(actual) if actual == direct));
    }
    #[test]
    fn rejected_rolled_back_and_other_causes_are_not_reinterpreted() {
        for original in [
            AdmissionFailure {
                completion: Completion::Rejected,
                error: ProgramStoreError::Deadline,
            },
            AdmissionFailure {
                completion: Completion::RolledBack,
                error: ProgramStoreError::Deadline,
            },
            AdmissionFailure {
                completion: Completion::Indeterminate,
                error: ProgramStoreError::Unsupported,
            },
        ] {
            let expected = original.completion;
            let result = confirm_metadata(Err(original), &input(), receipt().store, || {
                panic!("unsupported failure was reconciled")
            });
            assert!(
                matches!(result, Err(UnconfirmedAdmission::Original(actual)) if actual.completion == expected)
            );
        }
    }
    fn scope() -> cortex_core::program::operational::EffectScope {
        let operation = input();
        cortex_core::program::operational::EffectScope {
            program: operation.grant.program.clone(),
            baseline: operation.grant.baseline,
            task: TaskId::new("A").unwrap(),
            candidate: CandidateRequirement::new(
                RepositoryIdentity::new(
                    "/fixture".into(),
                    "/fixture/.git".into(),
                    "/fixture/.git".into(),
                )
                .unwrap(),
                cortex_core::CandidateId::from_git_commit(&"a".repeat(40)).unwrap(),
                WorktreeRequirement::Clean,
            ),
            attempt: AttemptId::new("A-attempt").unwrap(),
            generation: operation.grant.baseline,
            package: PackageId::new("fixture-package").unwrap(),
            package_revision: operation.grant.baseline,
        }
    }
    #[test]
    fn uncertain_dispatch_and_nested_domain_input_never_gain_launch_permission() {
        let mut operation = input();
        let scope = scope();
        operation.command = Command::SupervisedEffect(Box::new(EffectEvent::Dispatch {
            effect: cortex_core::program::operational::EffectId::new("fixture-effect").unwrap(),
            scope,
            at: 1,
            basis: SourceReference::operator_reported("fixture", "dispatch", "1").unwrap(),
        }));
        assert!(matches!(
            confirm_metadata(failed(), &operation, receipt().store, || panic!(
                "dispatch confirmed"
            )),
            Err(UnconfirmedAdmission::Original(_))
        ));
        operation.command = Command::DomainMember {
            program: operation.grant.program.clone(),
            operation: Box::new(input()),
        };
        assert!(matches!(
            confirm_metadata(failed(), &operation, receipt().store, || panic!(
                "nested operation confirmed"
            )),
            Err(UnconfirmedAdmission::Original(_))
        ));
    }

    #[test]
    fn process_preparation_uses_canonical_presence_without_caller_success() {
        use cortex_core::program::supervision::{ProcessLimits, ProcessSpec};
        // This is a policy input, not an executable fixture or a granted process.
        let reference = SourceReference::operator_reported("fixture", "preparation", "1").unwrap();
        let mut operation = input();
        operation.command = Command::SupervisedEffect(Box::new(EffectEvent::PrepareProcess {
            effect: cortex_core::program::operational::EffectId::new("fixture-effect").unwrap(),
            scope: scope(),
            at: 1,
            spec: Box::new(ProcessSpec {
                reference: reference.clone(),
                executable: "/fixture/never-launched".into(),
                executable_identity: reference.clone(),
                cwd: "/fixture".into(),
                args: vec![],
                prepared_input: OperationId::new("prepared-fixture").unwrap(),
                input_len: 1,
                host: reference.clone(),
                profile: reference.clone(),
                privacy: reference.clone(),
                approval: reference,
                limits: ProcessLimits {
                    wall_ms: 10_000,
                    cleanup_ms: 500,
                    input_bytes: 1,
                    output_bytes: 1,
                    frame_bytes: 1,
                    authority_poll_ms: 20,
                    executable_bytes: 1,
                    arguments: 0,
                    argument_bytes: 0,
                    transport: None,
                },
                containment: None,
                identity_checkpoint: None,
            }),
        }));
        let observed = confirm_metadata(failed(), &operation, receipt().store, || {
            Ok(Reconciliation::Present(receipt()))
        })
        .unwrap();
        let ObservedAdmission::Confirmed {
            original,
            receipt: confirmed,
        } = observed
        else {
            panic!("caller success invented")
        };
        assert_eq!(original.completion, Completion::Indeterminate);
        assert!(matches!(original.error, ProgramStoreError::Deadline));
        assert_eq!(confirmed, receipt());
    }
    #[test]
    fn uncertain_settlement_and_direct_domain_command_are_never_confirmed() {
        let mut operation = input();
        operation.command = Command::SupervisedEffect(Box::new(EffectEvent::Reconcile {
            effect: cortex_core::program::operational::EffectId::new("fixture-effect").unwrap(),
            scope: scope(),
            at: 1,
            resolution: cortex_core::program::operational::Reconciliation::Absent,
            basis: SourceReference::operator_reported("fixture", "unproven-absence", "1").unwrap(),
        }));
        assert!(matches!(
            confirm_metadata(failed(), &operation, receipt().store, || panic!(
                "settlement confirmed"
            )),
            Err(UnconfirmedAdmission::Original(_))
        ));
        operation.command = Command::Domain(Box::new(
            cortex_core::program::domain::DomainEvent::CreateRequested {
                domain: cortex_core::program::domain::DomainId::new("fixture-domain").unwrap(),
                at: 1,
            },
        ));
        assert!(matches!(
            confirm_metadata(failed(), &operation, receipt().store, || panic!(
                "domain creation confirmed"
            )),
            Err(UnconfirmedAdmission::Original(_))
        ));
    }
}
