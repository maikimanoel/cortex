//! Deterministic admission faults; no physical worker or historical store.
use super::supervised::supervised_fixture as supervised;
use super::*;
use crate as admission_storage;
#[path = "../../../../cortex-cli/tests/support/gpe_admission.rs"]
mod gpe_admission;
use crate::program_store::diagnostics::{ADMISSION_OBSERVATION_CAPACITY, Recorder};
use crate::program_store::diagnostics::{AdmissionErrorKind, AdmissionOutcome};
use cortex_core::program::operational::{EffectEvent, EffectStatus};
use cortex_core::program::scheduling::Resource;

fn prepared(f: &Temp) -> (ProgramStore, CoordinationKernel, Operation) {
    let (k, scope, _) = supervised::prepared_kernel(None);
    let store =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for operation in k.operations() {
        store.admit(&key(), operation.clone()).unwrap();
    }
    let input = supervised::dispatch_operation(&k, &scope);
    (store, k, input)
}
fn stages(report: &AdmissionDiagnostics) -> Vec<AdmissionStage> {
    report.events().map(|e| e.stage).collect()
}
fn assert_complete(report: &AdmissionDiagnostics, input: &Operation, store: &ProgramStore) {
    assert_eq!(report.operation(), Some(&input.id));
    assert_eq!(report.expected(), Some(input.expected));
    assert_eq!(report.store(), Some(store.id()));
    assert!(report.complete());
    assert!(!report.overflowed());
    assert_eq!(report.dropped_events(), 0);
    let times: Vec<_> = report.events().map(|e| e.elapsed).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]));
    assert!(times.len() <= ADMISSION_OBSERVATION_CAPACITY);
}

#[test]
fn diagnostic_stale_predecessor_rejects_without_mutation() {
    let f = Temp::new();
    let store = create(&f);
    let mut input = claim();
    input.expected = 99;
    let before = snapshot(&f);
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_with_diagnostics(&key(), input.clone(), &mut report)
        .unwrap_err();
    assert_eq!(error.completion, Completion::Rejected);
    assert!(matches!(error.error, ProgramStoreError::Domain(_)));
    assert_complete(&report, &input, &store);
    let events = stages(&report);
    assert!(events.contains(&AdmissionStage::RollbackAcknowledged));
    assert!(!events.contains(&AdmissionStage::Insert));
    assert!(!events.contains(&AdmissionStage::CommitAcknowledged));
    assert_eq!(snapshot(&f), before);
    assert_eq!(store.load(&key(), Access::ReadOnly).unwrap().revision(), 0);
    eprintln!("CASE A: stale predecessor rejected; no write; real rollback acknowledged");
}

#[test]
fn diagnostic_postcommit_deadline_retains_dispatch_and_prevents_relaunch() {
    let f = Temp::new();
    let (store, kernel, input) = prepared(&f);
    let mut report = AdmissionDiagnostics::default();
    let result = store.admit_diagnostic_observed(
        &key(),
        input.clone(),
        |_, _| {},
        |_| panic!("uncertain admission must not launch"),
        &mut report,
        Some(AdmissionFault::Deadline(Stage::AfterCommit)),
    );
    let error = result.unwrap_err();
    assert_eq!(error.completion, Completion::Indeterminate);
    assert!(matches!(error.error, ProgramStoreError::Deadline));
    assert_complete(&report, &input, &store);
    let events = stages(&report);
    assert!(events.contains(&AdmissionStage::CommitAcknowledged));
    assert!(!events.contains(&AdmissionStage::RollbackRequested));
    assert_eq!(
        events.last(),
        Some(&AdmissionStage::Finished(AdmissionOutcome::Failed(
            Completion::Indeterminate
        )))
    );
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let loaded = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(loaded.revision(), kernel.revision() + 1);
    assert!(loaded.operations().last() == Some(&input));
    assert_eq!(
        loaded.fixture_effects().effects()[0].status,
        EffectStatus::DispatchUncertain
    );
    for resource in [
        Resource::WorkerSlots,
        Resource::ElapsedMillis,
        Resource::OutputBytes,
    ] {
        assert_eq!(
            loaded.fixture_effects().accounted(resource.clone()),
            kernel.fixture_effects().accounted(resource)
        );
    }
    assert!(matches!(
        reopened
            .reconcile(&key(), kernel.initial(), &input, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(_)
    ));
    let (receipt, launch) = store
        .dispatch_fixture(&key(), input.clone(), |_| panic!("duplicate relaunched"))
        .unwrap();
    assert!(receipt.duplicate && launch.is_none());
    let mut changed = input;
    changed.id = OperationId::new("changed-uncertain-dispatch").unwrap();
    changed.expected = loaded.revision();
    assert!(
        store
            .dispatch_fixture(&key(), changed, |_| panic!("changed identity relaunched"))
            .is_err()
    );
    eprintln!(
        "CASE B: commit acknowledged, caller Indeterminate, persisted dispatch uncertain, reservation retained, no launch"
    );
}

#[test]
fn diagnostic_precommit_deadline_rolls_back_and_retains_reservation() {
    let f = Temp::new();
    let (store, kernel, input) = prepared(&f);
    let before = snapshot(&f);
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_diagnostic_observed(
            &key(),
            input.clone(),
            |_, _| {},
            |_| panic!("rolled-back operation launched"),
            &mut report,
            Some(AdmissionFault::Deadline(Stage::BeforeCommit)),
        )
        .unwrap_err();
    assert_eq!(error.completion, Completion::RolledBack);
    assert!(matches!(error.error, ProgramStoreError::Deadline));
    assert_complete(&report, &input, &store);
    let events = stages(&report);
    assert!(events.contains(&AdmissionStage::InsertAcknowledged));
    assert!(events.contains(&AdmissionStage::RollbackAcknowledged));
    assert!(!events.contains(&AdmissionStage::Commit));
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let loaded = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.operations() == kernel.operations());
    assert!(loaded.fixture_effects() == kernel.fixture_effects());
    assert!(matches!(
        store
            .reconcile(&key(), kernel.initial(), &input, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Absent
    ));
    // SQLite can update file metadata during a proven rollback; compare payload bytes.
    let after = snapshot(&f);
    assert_eq!(
        before.iter().map(|v| (&v.0, &v.1)).collect::<Vec<_>>(),
        after.iter().map(|v| (&v.0, &v.1)).collect::<Vec<_>>()
    );
    eprintln!("CASE C: inserted then rollback acknowledged; predecessor and reservation retained");
}

#[test]
fn diagnostic_denied_rollback_reports_uncertainty_without_absence_claim() {
    let f = Temp::new();
    let (store, kernel, input) = prepared(&f);
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_diagnostic_observed(
            &key(),
            input.clone(),
            |_, _| {},
            |_| panic!("unacknowledged rollback launched"),
            &mut report,
            Some(AdmissionFault::RollbackDenied),
        )
        .unwrap_err();
    assert_eq!(error.completion, Completion::Indeterminate);
    assert!(matches!(error.error, ProgramStoreError::Deadline));
    assert_complete(&report, &input, &store);
    let events = stages(&report);
    assert!(events.contains(&AdmissionStage::InsertAcknowledged));
    assert!(events.contains(&AdmissionStage::RollbackRequested));
    assert!(events.contains(&AdmissionStage::RollbackFailed));
    assert!(!events.contains(&AdmissionStage::RollbackAcknowledged));
    assert!(!events.contains(&AdmissionStage::CommitAcknowledged));
    assert_eq!(
        events.last(),
        Some(&AdmissionStage::Finished(AdmissionOutcome::Failed(
            Completion::Indeterminate
        )))
    );
    // A fresh observation after connection teardown is distinct from the failed
    // rollback acknowledgement. It must not relabel the retained failure.
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let loaded = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert!(loaded.operations() == kernel.operations());
    assert!(loaded.fixture_effects() == kernel.fixture_effects());
    assert_eq!(error.completion, Completion::Indeterminate);
    eprintln!(
        "CASE D: SQLite denied actual ROLLBACK; Indeterminate retained; connection-close cleanup is not an acknowledgement"
    );
}

#[test]
fn diagnostic_postlock_deadline_has_no_transaction_or_rollback() {
    let f = Temp::new();
    let store = create(&f);
    let input = claim();
    let before = snapshot(&f);
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_diagnostic_observed(
            &key(),
            input.clone(),
            |_, _| {},
            |_| (),
            &mut report,
            Some(AdmissionFault::Deadline(Stage::AfterLock)),
        )
        .unwrap_err();
    assert_eq!(error.completion, Completion::Rejected);
    assert!(matches!(error.error, ProgramStoreError::Deadline));
    assert_complete(&report, &input, &store);
    assert_eq!(
        stages(&report),
        vec![
            AdmissionStage::Entered,
            AdmissionStage::LockRequested,
            AdmissionStage::LockAcquired,
            AdmissionStage::LocationCheck,
            AdmissionStage::PostLockDeadlineCheck,
            AdmissionStage::Failure(AdmissionErrorKind::Deadline),
            AdmissionStage::Finished(AdmissionOutcome::Failed(Completion::Rejected)),
        ]
    );
    assert_eq!(snapshot(&f), before);
    eprintln!("CASE E: deterministic post-lock deadline; no connection, transaction or write");
}

#[test]
fn diagnostic_duplicate_and_disabled_paths_keep_identical_receipts_and_journal() {
    let f = Temp::new();
    let store = create(&f);
    let input = claim();
    let normal = store.admit(&key(), input.clone()).unwrap();
    let before = snapshot(&f);
    let mut report = AdmissionDiagnostics::default();
    let duplicate = store
        .admit_with_diagnostics(&key(), input.clone(), &mut report)
        .unwrap();
    assert_eq!(duplicate.ordinal, normal.ordinal);
    assert!(duplicate.duplicate);
    assert_complete(&report, &input, &store);
    assert!(!stages(&report).contains(&AdmissionStage::CommitAcknowledged));
    assert_eq!(
        stages(&report).last(),
        Some(&AdmissionStage::Finished(AdmissionOutcome::Duplicate {
            ordinal: 1
        }))
    );
    assert_eq!(snapshot(&f), before);
    assert_eq!(
        std::mem::size_of::<crate::program_store::diagnostics::Disabled>(),
        0
    );
}

#[test]
fn diagnostic_overflow_is_explicit_and_cannot_be_complete() {
    let mut report = AdmissionDiagnostics::default();
    let input = claim();
    report.begin(StoreId::from_bytes([7; 16]), &input);
    for _ in 0..ADMISSION_OBSERVATION_CAPACITY {
        report.record(AdmissionStage::Connect);
    }
    report.finish(&Ok((
        AdmissionReceipt {
            store: StoreId::from_bytes([7; 16]),
            ordinal: 1,
            duplicate: false,
        },
        Some(()),
    )));
    assert_eq!(report.events().count(), ADMISSION_OBSERVATION_CAPACITY);
    assert_eq!(report.dropped_events(), 2);
    assert!(report.overflowed());
    assert!(!report.complete());
}

#[test]
fn diagnostic_unwind_retains_incomplete_prefix_and_redacts_sensitive_identity() {
    let f = Temp::new();
    let store = create(&f);
    let mut input = claim();
    input.id = OperationId::new("sensitive-operation-sentinel").unwrap();
    let mut report = AdmissionDiagnostics::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.admit_diagnostic_observed(
            &key(),
            input.clone(),
            |stage, _| {
                if stage == Stage::BeforeCommit {
                    panic!("controlled interruption");
                }
            },
            |_| (),
            &mut report,
            None,
        )
    }));
    assert!(result.is_err());
    assert!(!report.complete());
    assert!(!report.overflowed());
    assert_eq!(report.operation(), Some(&input.id));
    let text = format!("{report:?}");
    assert!(!text.contains("sensitive-operation-sentinel"));
    assert!(!text.contains(f.0.to_str().unwrap()));
    assert!(!stages(&report).contains(&AdmissionStage::RollbackAcknowledged));
    assert!(
        !stages(&report)
            .iter()
            .any(|s| matches!(s, AdmissionStage::Finished(_)))
    );
    assert_eq!(store.load(&key(), Access::ReadOnly).unwrap().revision(), 0);
}

#[test]
fn diagnostic_enabled_and_disabled_appends_have_equal_canonical_state() {
    let first = Temp::new();
    let second = Temp::new();
    let ordinary = create(&first);
    let observed = create(&second);
    let input = claim();
    let normal = ordinary.admit(&key(), input.clone()).unwrap();
    let mut report = AdmissionDiagnostics::default();
    let instrumented = observed
        .admit_with_diagnostics(&key(), input.clone(), &mut report)
        .unwrap();
    assert_eq!(normal.store, ordinary.id());
    assert_eq!(instrumented.store, observed.id());
    assert_eq!((normal.ordinal, normal.duplicate), (1, false));
    assert_eq!((instrumented.ordinal, instrumented.duplicate), (1, false));
    assert_complete(&report, &input, &observed);
    let events = stages(&report);
    assert!(events.contains(&AdmissionStage::InsertAcknowledged));
    assert!(events.contains(&AdmissionStage::CommitAcknowledged));
    assert_eq!(
        events.last(),
        Some(&AdmissionStage::Finished(AdmissionOutcome::Appended {
            ordinal: 1
        }))
    );
    let left = ordinary.load(&key(), Access::ReadOnly).unwrap();
    let right = observed.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(left.revision(), 1);
    assert_eq!(right.revision(), 1);
    assert!(left.operations() == std::slice::from_ref(&input));
    assert!(right.operations() == left.operations());
    assert!(right.fixture_effects() == left.fixture_effects());
}

#[test]
fn diagnostic_held_lifecycle_lock_is_busy_not_deadline() {
    let f = Temp::new();
    let mut store = create(&f);
    let normal_limits = store.limits;
    let before = snapshot(&f);
    let held = store.location.lock_for(normal_limits.timeout()).unwrap();
    store.limits = ReadLimits::new(normal_limits.bytes(), Duration::from_nanos(1)).unwrap();
    let input = claim();
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_with_diagnostics(&key(), input.clone(), &mut report)
        .unwrap_err();
    assert_eq!(error.completion, Completion::Rejected);
    assert!(
        matches!(error.error, ProgramStoreError::Storage(ref error) if error.kind() == ErrorKind::Busy)
    );
    assert_complete(&report, &input, &store);
    assert_eq!(
        stages(&report),
        vec![
            AdmissionStage::Entered,
            AdmissionStage::LockRequested,
            AdmissionStage::Failure(AdmissionErrorKind::Storage(ErrorKind::Busy)),
            AdmissionStage::Finished(AdmissionOutcome::Failed(Completion::Rejected))
        ]
    );
    drop(held);
    store.limits = normal_limits;
    assert_eq!(snapshot(&f), before);
    assert_eq!(store.load(&key(), Access::ReadOnly).unwrap().revision(), 0);
}

#[test]
fn diagnostic_postcommit_metadata_requires_exact_canonical_confirmation() {
    let f = Temp::new();
    let store = create(&f);
    let input = claim();
    let mut report = AdmissionDiagnostics::default();
    let error = store
        .admit_diagnostic_observed(
            &key(),
            input.clone(),
            |_, _| {},
            |_| panic!("uncertain metadata invoked launch callback"),
            &mut report,
            Some(AdmissionFault::Deadline(Stage::AfterCommit)),
        )
        .unwrap_err();
    assert_eq!(error.completion, Completion::Indeterminate);
    assert!(matches!(error.error, ProgramStoreError::Deadline));
    assert_complete(&report, &input, &store);
    assert!(stages(&report).contains(&AdmissionStage::CommitAcknowledged));
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let loaded = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(loaded.revision(), 1);
    assert!(loaded.operations() == std::slice::from_ref(&input));
    assert_eq!(
        reopened
            .reconcile(&key(), &definition(), &input, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(AdmissionReceipt {
            store: store.id(),
            ordinal: 1,
            duplicate: true
        })
    );
    let mut changed = input.clone();
    changed.expected = 1;
    assert_eq!(
        reopened
            .reconcile(&key(), &definition(), &changed, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    changed = input.clone();
    changed.grant.actor = ActorId::new("changed-actor").unwrap();
    assert_eq!(
        reopened
            .reconcile(&key(), &definition(), &changed, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    assert_eq!(error.completion, Completion::Indeterminate);
}

fn fresh_reservation(f: &Temp) -> (ProgramStore, CoordinationKernel, Operation) {
    let (before, scope, _) = supervised::configured_kernel(None);
    let store = ProgramStore::create_supervised_fixture(
        &f.store(),
        &key(),
        before.initial().clone(),
        limits(),
    )
    .unwrap();
    for prior in before.operations() {
        store.admit(&key(), prior.clone()).unwrap();
    }
    let input = operation(
        &before,
        "reserve-synthetic-unknown",
        Input::SupervisedEffect(Box::new(EffectEvent::Reserve {
            intent: supervised::intent(&scope),
            at: before.fixture_effects().grants().last().unwrap().not_before,
        })),
    );
    (store, before, input)
}

#[test]
fn ordinary_fixture_reserve_late_ack_requires_exact_readback() {
    let f = Temp::new();
    let (store, before, input) = fresh_reservation(&f);
    let mut diagnostics = AdmissionDiagnostics::default();
    let failure = store
        .admit_diagnostic_observed(
            &key(),
            input.clone(),
            |_, _| {},
            |_| panic!("reservation invoked an execution callback"),
            &mut diagnostics,
            Some(AdmissionFault::Deadline(Stage::AfterCommit)),
        )
        .unwrap_err();
    assert_eq!(failure.completion, Completion::Indeterminate);
    assert!(matches!(failure.error, ProgramStoreError::Deadline));
    eprintln!("Synthetic encrypted-store postcommit fault: {diagnostics:?}");
    assert!(stages(&diagnostics).contains(&AdmissionStage::CommitAcknowledged));
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let after = reopened.load(&key(), Access::ReadOnly).unwrap();
    let record = after.fixture_effects().effects().last().unwrap();
    assert_eq!(record.status, EffectStatus::Reserved);
    assert_eq!(record.dispatch_at, None);
    assert!(
        after
            .operations()
            .last()
            .is_some_and(|retained| retained == &input)
    );
    let presence = reopened.reconcile(&key(), before.initial(), &input, Access::ReadOnly);
    assert!(matches!(
        presence, Ok(Reconciliation::Present(receipt))
            if receipt.ordinal == input.expected + 1 && receipt.store == store.id() && receipt.duplicate
    ));
    // This command stays outside the existing metadata readback policy.
    assert!(matches!(
        gpe_admission::confirm_metadata(
            Err(AdmissionFailure {
                completion: Completion::Indeterminate,
                error: ProgramStoreError::Deadline,
            }),
            &input,
            store.id(),
            || panic!("metadata confirmation must not read a reserve"),
        ),
        Err(gpe_admission::UnconfirmedAdmission::Original(_))
    ));
    let snapshot_before_read = snapshot(&f);
    let observed = gpe_admission::confirm_synthetic_reservation(
        Err(failure),
        &input,
        store.id(),
        &before,
        &reopened,
        &key(),
    )
    .unwrap();
    let gpe_admission::ObservedAdmission::Confirmed { original, receipt } = observed else {
        panic!("read-back converted the original reservation completion");
    };
    assert_eq!(original.completion, Completion::Indeterminate);
    assert!(matches!(original.error, ProgramStoreError::Deadline));
    assert_eq!(Some(receipt.ordinal), input.expected.checked_add(1));
    assert_eq!(receipt.store, store.id());
    assert!(receipt.duplicate);
    assert_eq!(snapshot(&f), snapshot_before_read);
}

#[test]
fn fixture_reservation_readback_refuses_conflict_and_later_disposition() {
    use gpe_admission::UnconfirmedAdmission::{Original, Reconciliation as Readback, Reservation};
    let f = Temp::new();
    let (store, before, input) = fresh_reservation(&f);
    let failure = || AdmissionFailure {
        completion: Completion::Indeterminate,
        error: ProgramStoreError::Deadline,
    };
    let confirm = |request: &Operation, expected, key: &StoreKey| {
        gpe_admission::confirm_synthetic_reservation(
            Err(failure()),
            request,
            expected,
            &before,
            &store,
            key,
        )
    };
    let empty = snapshot(&f);
    assert!(matches!(
        confirm(&input, store.id(), &key()),
        Err(Readback {
            observation: Ok(Reconciliation::Absent),
            ..
        })
    ));
    assert_eq!(snapshot(&f), empty);

    let first = store.admit(&key(), input.clone()).unwrap();
    assert!(!first.duplicate);
    let direct = gpe_admission::confirm_synthetic_reservation(
        Ok(first),
        &input,
        store.id(),
        &before,
        &store,
        &key(),
    )
    .unwrap();
    assert!(
        matches!(direct, gpe_admission::ObservedAdmission::Admitted(receipt) if receipt == first)
    );
    let retained = snapshot(&f);
    let wrong_key = StoreKey::new(&[36; 32]).unwrap();
    assert!(matches!(
        confirm(&input, store.id(), &wrong_key),
        Err(Readback {
            observation: Err(_),
            ..
        })
    ));
    assert!(matches!(
        confirm(&input, StoreId::from_bytes([2; 16]), &key()),
        Err(Original(_))
    ));
    for predecessor in [0, u64::MAX] {
        let mut changed = input.clone();
        changed.expected = predecessor;
        assert!(matches!(
            confirm(&changed, store.id(), &key()),
            Err(Original(_))
        ));
    }
    let mut changed_grant = input.clone();
    changed_grant.grant.actor = ActorId::new("changed-actor").unwrap();
    let mut changed_demand = input.clone();
    let Input::SupervisedEffect(event) = &mut changed_demand.command else {
        unreachable!()
    };
    let EffectEvent::Reserve { intent, .. } = event.as_mut() else {
        unreachable!()
    };
    intent.demand[0].amount = Some(intent.demand[0].amount.unwrap() + 1);
    let mut changed_scope = input.clone();
    let Input::SupervisedEffect(event) = &mut changed_scope.command else {
        unreachable!()
    };
    let EffectEvent::Reserve { intent, .. } = event.as_mut() else {
        unreachable!()
    };
    intent.scope.generation = rev(2);
    for changed in [changed_grant, changed_demand, changed_scope] {
        assert!(matches!(
            confirm(&changed, store.id(), &key()),
            Err(Readback {
                observation: Ok(Reconciliation::Conflict),
                ..
            })
        ));
    }
    let latest = store.load(&key(), Access::ReadOnly).unwrap();
    let record = &latest.fixture_effects().effects()[0];
    let next = supervised::dispatch_operation(&latest, &record.intent.scope);
    assert!(matches!(
        confirm(&next, store.id(), &key()),
        Err(Original(_))
    ));
    for error in [
        AdmissionFailure {
            completion: Completion::Indeterminate,
            error: ProgramStoreError::Unsupported,
        },
        AdmissionFailure {
            completion: Completion::RolledBack,
            error: ProgramStoreError::Deadline,
        },
    ] {
        assert!(matches!(
            gpe_admission::confirm_synthetic_reservation(
                Err(error),
                &input,
                store.id(),
                &before,
                &store,
                &key()
            ),
            Err(Original(_))
        ));
    }
    assert_eq!(snapshot(&f), retained);
    let revoke = operation(
        &latest,
        "revoke-retained-grant",
        Input::SupervisedEffect(Box::new(EffectEvent::Revoke {
            grant: record.intent.grant.clone(),
            at: latest.fixture_effects().last_tick(),
            basis: source("fixture-revoked"),
        })),
    );
    store.admit(&key(), revoke).unwrap();
    let revoked = snapshot(&f);
    assert!(matches!(
        confirm(&input, store.id(), &key()),
        Err(Reservation {
            current_read: Ok(false),
            ..
        })
    ));
    assert_eq!(snapshot(&f), revoked);
    let current = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        current.fixture_effects().effects()[0].status,
        EffectStatus::Reserved
    );
    let cancel = operation(
        &current,
        "cancel-retained-reservation",
        Input::SupervisedEffect(Box::new(EffectEvent::Cancel {
            effect: record.intent.id.clone(),
            scope: record.intent.scope.clone(),
            at: current.fixture_effects().last_tick(),
            basis: source("fixture-cancelled"),
        })),
    );
    store.admit(&key(), cancel).unwrap();
    assert_eq!(
        store
            .load(&key(), Access::ReadOnly)
            .unwrap()
            .fixture_effects()
            .effects()[0]
            .status,
        EffectStatus::CancelledBeforeDispatch
    );
    let cancelled = snapshot(&f);
    assert!(matches!(
        confirm(&input, store.id(), &key()),
        Err(Reservation {
            current_read: Ok(false),
            ..
        })
    ));
    assert_eq!(snapshot(&f), cancelled);
}
