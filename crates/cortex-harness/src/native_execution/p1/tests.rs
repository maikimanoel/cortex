//! Controlled preflight/journal cuts. No test starts or contacts a host manager.
use super::*;
use crate::{
    domain_runtime::tests::fixture,
    native_execution::{backend::ExecutionObservation, transport::CapturedStreams},
};
use cortex_core::{
    binding::ObservationId,
    program::{OperationId, supervision::Quiescence},
};
use cortex_storage::program_store::ReadLimits;
use std::{fs, os::unix::fs::DirBuilderExt};

#[test]
fn candidate_revocation_reuse_and_staleness_refuse_before_managed_effects() {
    let (mut kernel, intent) = fixture::coordinator();
    let command = Command::Domain(Box::new(DomainEvent::Reserve {
        intent: intent.clone(),
        at: 1,
    }));
    let operation = fixture::domain_operation(&kernel, "p1-reserve", command);
    let ids = DomainOperations {
        domain: intent.id.clone(),
        create: OperationId::new("p1-create").unwrap(),
        bind: OperationId::new("p1-bind").unwrap(),
        release: OperationId::new("p1-release").unwrap(),
        report: OperationId::new("p1-report").unwrap(),
        grant: operation.grant.clone(),
        basis: fixture::source("source simulation"),
        at: 2,
    };
    let observer = SourceReference::observation_reference(
        ObservationId::new("controlled-observer", 1).unwrap(),
        "source simulation",
        "1",
    )
    .unwrap();
    let approval = fixture::source("separate simulated owner");
    let delegation = DelegationAuthorization {
        root: "/unused-source-simulation".into(),
        device: 1,
        inode: 1,
        boot: intent.pool.boot.clone(),
    };
    let wrong_candidate = CandidateRequirement::new(
        intent.effect.scope.candidate.repository().clone(),
        cortex_core::CandidateId::new("1111111111111111111111111111111111111111").unwrap(),
        intent.effect.scope.candidate.worktree(),
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let bind = |candidate, delegation, reservation, operations| RawCopyP1Binding {
        approval: &approval,
        observer: &observer,
        candidate,
        target: ManagedP1Target {
            pool: &intent.pool,
            delegation,
            manager: "synthetic-only",
            manager_version: "synthetic-only",
        },
        controller_sha256: [0; 32],
        controller_max_bytes: 1,
        fixture_sha256: [0; 32],
        plan_digest: "synthetic-only",
        reservation,
        operations,
        deadline,
    };
    assert!(matches!(
        reservation_intent(
            &kernel,
            &bind(&wrong_candidate, &delegation, &operation, &ids)
        ),
        Err(P1Refusal::Correspondence)
    ));
    let candidate = &intent.effect.scope.candidate;
    let wrong_boot = DelegationAuthorization {
        boot: "old boot".into(),
        root: delegation.root.clone(),
        device: delegation.device,
        inode: delegation.inode,
    };
    assert!(matches!(
        reservation_intent(&kernel, &bind(candidate, &wrong_boot, &operation, &ids)),
        Err(P1Refusal::Correspondence)
    ));
    let mut wrong_grant = ids.clone();
    wrong_grant.grant.tasks.clear();
    assert!(matches!(
        reservation_intent(
            &kernel,
            &bind(candidate, &delegation, &operation, &wrong_grant)
        ),
        Err(P1Refusal::Correspondence)
    ));
    let mut aliased = ids.clone();
    aliased.report = ids.release.clone();
    assert!(matches!(
        reservation_intent(&kernel, &bind(candidate, &delegation, &operation, &aliased)),
        Err(P1Refusal::Correspondence)
    ));
    let binding = bind(candidate, &delegation, &operation, &ids);
    assert!(reservation_intent(&kernel, &binding).is_ok());
    let original = kernel.revision();
    assert!(matches!(
        after_independent_qualification(
            Err(P1Refusal::IndependentQualificationUnavailable),
            || {
                let _ = kernel.admit(operation.clone()).unwrap();
                Ok(())
            },
        ),
        Err(P1Refusal::IndependentQualificationUnavailable)
    ));
    assert_eq!(kernel.revision(), original);
    let _ = kernel.admit(operation.clone()).unwrap();
    assert!(matches!(
        reservation_intent(&kernel, &binding),
        Err(P1Refusal::Correspondence)
    ));
    let mut stale = operation;
    stale.id = OperationId::new("p1-stale").unwrap();
    assert!(matches!(
        reservation_intent(&kernel, &bind(candidate, &delegation, &stale, &ids)),
        Err(P1Refusal::Admission(_))
    ));
}

#[test]
fn expired_cancelled_and_unbounded_cases_are_refused() {
    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);
    let now = Instant::now();
    assert!(matches!(
        live_budget(now, &active),
        Err(P1Refusal::DeadlineOrCancellation)
    ));
    assert!(matches!(
        live_budget(now + Duration::from_secs(10), &cancelled),
        Err(P1Refusal::DeadlineOrCancellation),
    ));
    assert!(matches!(
        live_budget(now + Duration::from_secs(61), &active),
        Err(P1Refusal::DeadlineOrCancellation),
    ));
    assert!(live_budget(now + Duration::from_secs(10), &active).is_ok());
    assert!(matches!(
        controller_digest(1, now + Duration::from_secs(10), &active),
        Err(P1Refusal::Controller),
    ));
}

#[test]
fn copy_oracle_requires_durable_report_and_eof_in_addition_to_matching_bytes() {
    let (kernel, _) = fixture::coordinator();
    let root = std::env::temp_dir().join(format!("cortex-p1-copy-oracle-{}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let store = ProgramStore::create_domain_fixture(
        &root.join("store"),
        &StoreKey::new(&[17; 32]).unwrap(),
        kernel.initial().clone(),
        ReadLimits::new(16_000_000, MAX_CASE_INTERVAL).unwrap(),
    )
    .unwrap();
    let mut run = ExecutableDomainRun {
        domain: DomainRun {
            creation: None,
            binding: None,
            release: None,
            nonce: None,
            report: None,
            diagnostic: None,
            recovery_only: false,
        },
        execution: Some(ExecutionObservation {
            streams: CapturedStreams {
                stdout: b"expected".to_vec(),
                stderr: Vec::new(),
                completion: ProcessCompletion::Exited {
                    code: Some(0),
                    signal: None,
                },
                quiescence: Quiescence::Unknown,
                input_complete: true,
                stdout_complete: true,
                stderr_complete: true,
                elapsed: Duration::ZERO,
            },
            artifacts: Vec::new(),
            domain_populated: Some(false),
            structured: None,
        }),
    };
    // Controlled receipt shape. This demonstrates the raw-copy oracle's
    // refusal conditions only. Canonical ordering is verified elsewhere.
    assert_eq!(
        assess_copy(&run, b"expected"),
        RawCopyP1Observation::Unresolved
    );
    run.domain.report = Some(AdmissionReceipt {
        store: store.id(),
        ordinal: 4,
        duplicate: false,
    });
    assert_eq!(
        assess_copy(&run, b"expected"),
        RawCopyP1Observation::Matches
    );
    assert_eq!(assess_copy(&run, b"changed"), RawCopyP1Observation::Differs);
    run.execution.as_mut().unwrap().streams.stderr_complete = false;
    assert_eq!(
        assess_copy(&run, b"expected"),
        RawCopyP1Observation::Unresolved
    );
    fs::remove_dir_all(root).unwrap();
}
