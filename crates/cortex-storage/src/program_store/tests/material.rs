//! Protected-material storage tests use fresh encrypted fixtures and no host effects.

use super::*;
use cortex_core::{knowledge::SourcePath, parallel::*, temporal::SourceReference};
use cortex_evidence::{
    program::material::*,
    readiness::{EdgeEvidenceContract, PredicateCorrespondence},
    unified::ReportedObservation,
};

fn material_release(store: &ProgramStore) -> Operation {
    let mut template = initialized();
    for op in template.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    let prefix = template.revision() as usize;
    let mut release = prepare_release(&mut template, "AB", true);
    let [observed, assessed, report] = &template.operations()[prefix..] else {
        panic!("expected one report, assessment and artifact declaration");
    };
    let Input::Observe(observation) = observed.command.clone() else {
        panic!("expected the exact assessed report");
    };
    let Input::Assess(assessment) = &assessed.command else {
        panic!("expected the assessment");
    };
    let Event::Review(review) = release.review.event() else {
        panic!("expected the existing edge review");
    };
    let reviewer = grant(1, "material-reviewer");
    let approval =
        SourceReference::operator_reported(reviewer.actor.as_str(), "scoped review", "1").unwrap();
    let selected_report = NotificationId::new("material-ab-second-report").unwrap();
    release.review = Notification::new(
        release.review.id().clone(),
        release.review.graph().clone(),
        Event::Review(Box::new(EdgeReview::new(
            review.edge().clone(),
            selected_report.clone(),
            review.scopes()[0].clone(),
            review.scopes()[1].clone(),
            ReviewRole::Coordinator,
            ReviewAttestation::new(approval, reviewer.basis.clone(), None),
            review.supports().to_vec(),
        ))),
    )
    .unwrap();
    let op_id = OperationId::new("protected-ab").unwrap();
    let mut report = report.clone();
    report.expected = prefix as u64;
    report.grant = grant(1, "material-author");
    store.admit(&key(), report.clone()).unwrap();
    let Input::Notify(notification) = &report.command else {
        panic!("expected artifact report notification");
    };
    assert!(matches!(notification.event(), Event::Artifact(_)));
    store
        .admit(
            &key(),
            Operation {
                id: OperationId::new("typed-material-secondary-author").unwrap(),
                expected: prefix as u64 + 1,
                grant: grant(1, "material-secondary-author"),
                command: Input::Notify(
                    Notification::new(
                        selected_report.clone(),
                        notification.graph().clone(),
                        notification.event().clone(),
                    )
                    .unwrap(),
                ),
            },
        )
        .unwrap();

    let mut observe = observed.clone();
    observe.expected = prefix as u64 + 2;
    observe.grant = reviewer.clone();
    observe.command = Input::ObserveMaterial(Box::new(MaterialObservation {
        report: observation,
        subject: MaterialSubject::Release {
            contract: Box::new(release.contract.clone()),
            release: op_id.clone(),
            review: release.review.id().clone(),
            artifact_report: selected_report,
            mapping: MaterialMapping {
                source: MaterialFile::new(
                    SourcePath::new(b"worker-file").unwrap(),
                    MaterialMode::Executable,
                    3,
                    [0x57; 32],
                )
                .unwrap(),
                destination: SourcePath::new(b"chosen-destination").unwrap(),
            },
        },
    }));
    // A separate valid repository serving the same immutable commit must not
    // acquire provenance merely by changing the report and criterion together.
    let mut substituted = observe.clone();
    let Input::ObserveMaterial(material) = &mut substituted.command else {
        unreachable!()
    };
    let MaterialSubject::Release { contract, .. } = &mut material.subject else {
        unreachable!()
    };
    let original = contract.artifact_requirement();
    let sibling = binding("B", "b", 1);
    let replacement = CandidateRequirement::new(
        sibling.candidate().repository().clone(),
        original.commit().clone(),
        original.worktree(),
    );
    let old = material.report.subject().contract();
    let changed_criterion =
        CandidateBindingContract::new(old.task().clone(), old.revision(), replacement.clone());
    let mappings = contract
        .correspondences()
        .iter()
        .map(|entry| {
            PredicateCorrespondence::new(
                entry.predicate().clone(),
                changed_criterion.clone(),
                entry.profile().clone(),
                entry.coverage(),
            )
        })
        .collect();
    **contract = EdgeEvidenceContract::new(
        contract.scope().clone(),
        contract.revision(),
        replacement,
        mappings,
        contract.basis().clone(),
        release.limits,
    )
    .unwrap();
    let old = &material.report;
    material.report = ReportedObservation::new(
        old.id().clone(),
        capture(&changed_criterion, 1),
        old.profile().clone(),
        old.times(),
        old.kind().clone(),
        old.detail(),
    )
    .unwrap();
    let failure = store.admit(&key(), substituted).unwrap_err();
    assert!(matches!(
        failure.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Stale))
    ));

    let mut self_report = observe.clone();
    // The original producer cannot become independent merely by selecting a
    // different operator's duplicate declaration with a distinct report ID.
    self_report.grant = report.grant;
    let fail = store.admit(&key(), self_report).unwrap_err();
    assert!(matches!(
        fail.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Authority))
    ));
    store.admit(&key(), observe).unwrap();

    let mut unaccepted = assessed.clone();
    unaccepted.expected = prefix as u64 + 3;
    unaccepted.grant = grant(1, "material-author");
    let err = store.admit(&key(), unaccepted).unwrap_err();
    assert!(matches!(
        err.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Authority))
    ));
    let mut assessed = assessed.clone();
    assessed.expected = prefix as u64 + 3;
    assessed.grant = reviewer.clone();
    store.admit(&key(), assessed).unwrap();
    Operation {
        id: op_id,
        expected: prefix as u64 + 4,
        grant: reviewer,
        command: Input::ReleaseMaterial(Box::new(MaterialRelease {
            material: cortex_evidence::readiness::EvidenceReference::new(
                assessment.examination.observation.clone(),
                Some(assessment.id.clone()),
            ),
            release,
        })),
    }
}

#[test]
fn material_release_uncertain_commit_reconciles_and_invalidation_survives_reopen() {
    let f = Temp::new();
    let store = ProgramStore::create_protected_material(&f.store(), &key(), definition(), limits())
        .unwrap();
    let release = material_release(&store);
    let before = store.load(&key(), Access::ReadOnly).unwrap();
    let exact = snapshot(&f);
    let mut changed = release.clone();
    changed.id = OperationId::new("changed-protected-id").unwrap();
    assert!(store.admit(&key(), changed).is_err());
    let mut changed = release.clone();
    changed.expected -= 1;
    assert!(store.admit(&key(), changed).is_err());
    let mut unsupported = release.clone();
    unsupported.grant = grant(1, "material-author");
    assert!(store.admit(&key(), unsupported).is_err());
    assert_eq!(snapshot(&f), exact);
    assert!(before.accepted_material(&release.id).is_err());

    let mut diagnostics = diagnostics::AdmissionDiagnostics::default();
    let failure = store
        .admit_diagnostic_observed(
            &key(),
            release.clone(),
            |_, _| {},
            |_| panic!("indeterminate metadata never invokes an effect callback"),
            &mut diagnostics,
            Some(AdmissionFault::Deadline(Stage::AfterCommit)),
        )
        .unwrap_err();
    assert_eq!(failure.completion, Completion::Indeterminate);
    assert!(matches!(failure.error, ProgramStoreError::Deadline));
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(reopened.id(), store.id());
    assert_eq!(reopened.profile(), ProgramProfile::ProtectedMaterialV5);
    let matches = |expected| {
        reopened
            .reconcile(&key(), &definition(), expected, Access::ReadOnly)
            .unwrap()
    };
    let after_lost_ack = snapshot(&f);
    assert!(matches!(matches(&release), Reconciliation::Present(receipt)
        if receipt.duplicate && receipt.ordinal == release.expected + 1 && receipt.store == store.id()));
    let mut changed = release.clone();
    changed.grant = grant(1, "other-reviewer");
    assert_eq!(matches(&changed), Reconciliation::Conflict);
    assert_eq!(snapshot(&f), after_lost_ack);
    assert!(store.admit(&key(), release.clone()).unwrap().duplicate);
    let current = reopened.load(&key(), Access::ReadOnly).unwrap();
    let (_, mapping) = current.accepted_material(&release.id).unwrap();
    assert_eq!(mapping.source.mode(), MaterialMode::Executable);
    assert_eq!(mapping.source.path().bytes(), b"worker-file");
    assert_eq!(mapping.source.length(), 3);
    assert_eq!(mapping.source.sha256(), &[0x57; 32]);
    assert_eq!(mapping.destination.bytes(), b"chosen-destination");
    let Input::ReleaseMaterial(material) = &release.command else {
        panic!("expected a material release");
    };
    let invalidation = operation(
        &current,
        "invalidate-protected-input",
        Input::InvalidateEvidence {
            observation: material.material.observation().clone(),
            reason: "scoped verification changed".into(),
        },
    );
    let mut outside = invalidation.clone();
    outside.grant = grant(1, "unrelated-reviewer");
    outside.grant.tasks = vec![task("C")];
    let failure = store.admit(&key(), outside).unwrap_err();
    assert!(matches!(
        failure.error,
        ProgramStoreError::Domain(KernelError::Core(ProgramError::Authority))
    ));
    assert_eq!(
        reopened.load(&key(), Access::ReadOnly).unwrap().revision(),
        current.revision()
    );
    store.admit(&key(), invalidation).unwrap();
    let recovered = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert!(recovered.accepted_material(&release.id).is_err());
    assert_eq!(recovered.revision(), current.revision() + 1);
    assert!(matches!(matches(&release), Reconciliation::Present(receipt) if receipt.duplicate));
}

#[test]
fn legacy_release_and_profile_gates_preserve_old_identity_without_material() {
    let legacy = Temp::new();
    let protected = Temp::new();
    let old = create(&legacy);
    let mut legacy_scope = initialized();
    release(&mut legacy_scope, "AB");
    let new =
        ProgramStore::create_protected_material(&protected.store(), &key(), definition(), limits())
            .unwrap();
    for operation in legacy_scope.operations() {
        old.admit(&key(), operation.clone()).unwrap();
        new.admit(&key(), operation.clone()).unwrap();
    }
    let id = &legacy_scope.operations().last().unwrap().id;
    for store in [&old, &new] {
        let k = store.load(&key(), Access::ReadOnly).unwrap();
        assert!(k.operations() == legacy_scope.operations());
        assert!(k.accepted_material(id).is_err());
        assert!(k.state().readiness().task(&task("B")).unwrap().eligible());
    }

    let draft = Temp::new();
    let store =
        ProgramStore::create_protected_material(&draft.store(), &key(), definition(), limits())
            .unwrap();
    let operation = material_release(&store);
    let existing = snapshot(&legacy);
    let failure = old.admit(&key(), operation).unwrap_err();
    assert!(matches!(failure.error, ProgramStoreError::Unsupported));
    assert_eq!(snapshot(&legacy), existing);

    let unknown = Temp::new();
    let store =
        ProgramStore::create_protected_material(&unknown.store(), &key(), definition(), limits())
            .unwrap();
    let conn = history_store::connect(&store.location, &key(), Access::Recover).unwrap();
    conn.execute("UPDATE program_meta SET format=6", [])
        .unwrap();
    drop(conn);
    let saved = snapshot(&unknown);
    assert!(matches!(
        ProgramStore::open(&unknown.store(), &key(), Access::ReadOnly, limits()),
        Err(ProgramStoreError::Unsupported)
    ));
    assert_eq!(snapshot(&unknown), saved);
}
