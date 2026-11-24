//! Owned Git and SQLCipher evidence under the explicit local review profile.

use super::*;
use cortex_cli::program_execution::{
    AcceptedMaterialObservation, MaterialReleaseRequest, ProtectedAcceptanceRequest,
    ProtectedIntegrationRequest, accept_protected_integration, observe_protected_integration,
    observe_release_material,
};
use cortex_core::temporal::SourceReference;
use cortex_evidence::{
    program::{
        AssessmentInput, Examination, Operation, ReleaseInput,
        material::{MaterialMode, MaterialRelease, MaterialSubject},
    },
    readiness::{EvidenceReference, WitnessBinding},
    unified::{Applicability, AssessmentId, AssessmentResult},
};
use cortex_storage::program_store::{ProgramProfile, Reconciliation};
use std::time::SystemTime;

const REVIEWER: &str = "independent-fixture-reviewer";
const LIMIT: usize = 512;

fn record(
    store: &ProgramStore,
    key: &StoreKey,
    id: &str,
    actor: &str,
    command: Command,
) -> Operation {
    Operation {
        id: OperationId::new(id).unwrap(),
        expected: store.load(key, Access::ReadOnly).unwrap().revision(),
        grant: fixture::grant(1, actor),
        command,
    }
}

fn proposed_release(store: &ProgramStore, key: &StoreKey, edge: &str) -> ReleaseInput {
    let initial = store.load(key, Access::ReadOnly).unwrap().initial().clone();
    let mut draft = CoordinationKernel::new(initial).unwrap();
    let mut release = prepared_observed_release(&mut draft, edge);
    let Event::Review(review) = release.review.event() else {
        unreachable!();
    };
    let report = draft
        .operations()
        .iter()
        .find_map(|op| match &op.command {
            Command::Notify(n) if n.id() == review.report() => Some(n.clone()),
            _ => None,
        })
        .unwrap();
    let producer = format!("author-{}", review.scopes()[0].task().as_str());
    store
        .admit(
            key,
            record(
                store,
                key,
                &format!("producer-report-{edge}"),
                &producer,
                Command::Notify(report),
            ),
        )
        .unwrap();
    let grant = fixture::grant(1, REVIEWER);
    release.review = Notification::new(
        release.review.id().clone(),
        release.review.graph().clone(),
        Event::Review(Box::new(EdgeReview::new(
            review.edge().clone(),
            review.report().clone(),
            review.scopes()[0].clone(),
            review.scopes()[1].clone(),
            ReviewRole::Coordinator,
            ReviewAttestation::new(
                SourceReference::operator_reported(REVIEWER, "material-review", "1").unwrap(),
                grant.basis,
                None,
            ),
            review.supports().to_vec(),
        ))),
    )
    .unwrap();
    release
}

fn assess_operation(
    store: &ProgramStore,
    key: &StoreKey,
    observed: &AcceptedMaterialObservation,
    actor: &str,
    id: &str,
) -> (Operation, EvidenceReference, Examination) {
    let Command::ObserveMaterial(material) = &observed.operation.command else {
        unreachable!();
    };
    let subject = material.report.subject();
    let contract = subject.contract();
    let context = |advance| Examination {
        observation: material.report.id().clone(),
        contract: contract.clone(),
        candidate: Some(
            inspect_candidate(
                contract.candidate().repository().root(),
                contract,
                ObservationId::new(subject.id().scope(), subject.id().sequence() + advance)
                    .unwrap(),
            )
            .unwrap(),
        ),
        profile: Some(material.report.profile().clone()),
    };
    let id = AssessmentId::new(id).unwrap();
    let assessment = AssessmentInput {
        examination: context(1),
        id: id.clone(),
        at: SystemTime::now(),
        result: AssessmentResult::Pass,
        applicability: Applicability::Applicable,
        rationale: "The independently selected exact fixture criterion holds.".into(),
    };
    (
        record(
            store,
            key,
            id.as_str(),
            actor,
            Command::Assess(Box::new(assessment)),
        ),
        EvidenceReference::new(material.report.id().clone(), Some(id)),
        context(2),
    )
}

fn release_bytes(
    store: &ProgramStore,
    key: &StoreKey,
    edge: &str,
    destination: &str,
    independent_bytes: &[u8],
    legacy: bool,
) -> OperationId {
    let mut release = proposed_release(store, key, edge);
    let Event::Review(review) = release.review.event() else {
        unreachable!();
    };
    let id = OperationId::new(&format!("release-typed-{edge}")).unwrap();
    let source = SourcePath::new(b"part.txt").unwrap();
    let dest = SourcePath::new(destination.as_bytes()).unwrap();
    let pred = release.contract.correspondences()[0].predicate();
    let revision = store.load(key, Access::ReadOnly).unwrap().revision();
    let request = || MaterialReleaseRequest {
        contract: &release.contract,
        predicate: pred,
        release: id.clone(),
        review: release.review.id().clone(),
        artifact_report: review.report().clone(),
        source: &source,
        destination: &dest,
        expected: independent_bytes,
        expected_mode: MaterialMode::Regular,
        max_bytes: LIMIT,
        expected_revision: revision,
        candidate_observation: ObservationId::new(&format!("material-candidate-{edge}"), 1)
            .unwrap(),
    };
    let observe = |actor| IntegrationObservationRequest {
        operation: OperationId::new(&format!("observe-typed-{edge}")).unwrap(),
        review: fixture::grant(1, actor),
        observation: ObservationId::new(&format!("material-evidence-{edge}"), 1).unwrap(),
    };
    let author = format!("author-{}", review.scopes()[0].task().as_str());
    let refusal = observe_release_material(store, key, request(), observe(&author))
        .err()
        .expect("the producer cannot review its material");
    assert!(
        matches!(refusal, IntegrationError::Admission { .. }),
        "{refusal:?}"
    );

    assert_eq!(
        store.load(key, Access::ReadOnly).unwrap().revision(),
        revision
    );
    let observed = observe_release_material(store, key, request(), observe(REVIEWER)).unwrap();
    assert!(observed.satisfied);
    let (assessment, evidence, examination) = assess_operation(
        store,
        key,
        &observed,
        REVIEWER,
        &format!("assess-typed-{edge}"),
    );
    release.bindings = vec![WitnessBinding::new(
        pred.clone(),
        evidence.clone(),
        release.contract.scope().requirement().artifact().clone(),
    )];
    release.examinations = vec![examination];
    store.admit(key, assessment).unwrap();
    let command = if legacy {
        Command::Release(Box::new(release))
    } else {
        Command::ReleaseMaterial(Box::new(MaterialRelease {
            release,
            material: evidence,
        }))
    };
    let mut operation = record(store, key, id.as_str(), REVIEWER, command);
    let first = store.admit(key, operation.clone()).unwrap();
    assert!(!first.duplicate);
    let duplicate = store.admit(key, operation.clone()).unwrap();
    assert!(duplicate.duplicate && duplicate.ordinal == first.ordinal);
    operation.expected += 1;
    assert_eq!(
        store
            .reconcile(
                key,
                store.load(key, Access::ReadOnly).unwrap().initial(),
                &operation,
                Access::ReadOnly,
            )
            .unwrap(),
        Reconciliation::Conflict
    );
    id
}

fn claim_integrator(store: &ProgramStore, key: &StoreKey) {
    let definition = store.load(key, Access::ReadOnly).unwrap();
    for (id, event) in [
        (
            "material-integrator-ready",
            Event::Preconditions {
                scope: definition.state().readiness().scope(&task("F")).unwrap(),
                ready: true,
                source: fixture::source("bounded integration fixture ready"),
            },
        ),
        (
            "material-integrator-capacity",
            Event::Capacity {
                available: 1,
                source: fixture::source("one local operator slot"),
            },
        ),
    ] {
        store
            .admit(
                key,
                record(
                    store,
                    key,
                    id,
                    "operator",
                    Command::Notify(fixture::notify(definition.state(), id, event)),
                ),
            )
            .unwrap();
    }
    store
        .admit(
            key,
            record(
                store,
                key,
                "claim-integrated-candidate",
                "author-F",
                Command::Claim(fixture::attempt("integration-claim", "F", "author-F")),
            ),
        )
        .unwrap();
}

#[test]
fn protected_material_requires_exact_approved_paths_and_separate_current_judgment() {
    let root = OwnedTestRoot(
        std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("cortex-protected-material-{}", process::id())),
    );
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    let left = candidate(
        &root.0.join("B"),
        &[("part.txt", b"2\n"), ("spare.txt", b"2\n")],
        None,
    );
    let right = candidate(&root.0.join("E"), &[("part.txt", b"3\n")], None);
    let integrated = candidate(
        &root.0.join("F"),
        &[
            ("left.txt", b"2\n"),
            ("right.txt", b"3\n"),
            ("summary.txt", b"sum:5\n"),
        ],
        None,
    );
    let alias = candidate(
        &root.0.join("E-aliased"),
        &[("part.txt", b"2\n"), ("spare.txt", b"2\n")],
        None,
    );
    assert_eq!(left.commit(), alias.commit());
    assert_ne!(left.repository().root(), alias.repository().root());
    let aliased_integration = candidate(
        &root.0.join("F-aliased"),
        &[
            ("left.txt", b"2\n"),
            ("right.txt", b"2\n"),
            ("summary.txt", b"sum:5\n"),
        ],
        None,
    );
    let changed_mode = candidate(
        &root.0.join("F-changed-mode"),
        &[
            ("left.txt", b"2\n"),
            ("right.txt", b"3\n"),
            ("summary.txt", b"sum:5\n"),
        ],
        Some("right.txt"),
    );
    let key = StoreKey::new(&[0x38; 32]).unwrap();
    let limits = ReadLimits::new(8_000_000, Duration::from_secs(10)).unwrap();
    for (name, worker, integrated, expected_b) in [
        ("legacy", &right, &integrated, &b"3\n"[..]),
        ("alias", &alias, &aliased_integration, &b"2\n"[..]),
        ("wrong-mode", &right, &changed_mode, &b"3\n"[..]),
        ("protected", &right, &integrated, &b"3\n"[..]),
    ] {
        let path = root.0.join(name);
        let store = ProgramStore::create_protected_material(
            &path,
            &key,
            definition(&left, worker, integrated),
            limits,
        )
        .unwrap();
        assert_eq!(store.profile(), ProgramProfile::ProtectedMaterialV5);
        let a = release_bytes(&store, &key, "BF", "left.txt", b"2\n", name == "legacy");
        let b = release_bytes(&store, &key, "EF", "right.txt", expected_b, false);
        claim_integrator(&store, &key);
        let part = SourcePath::new(b"part.txt").unwrap();
        let substitute = SourcePath::new(b"spare.txt").unwrap();
        let left_path = SourcePath::new(b"left.txt").unwrap();
        let right_path = SourcePath::new(b"right.txt").unwrap();
        let output = SourcePath::new(b"summary.txt").unwrap();
        let f = task("F");
        let before = store.load(&key, Access::ReadOnly).unwrap();
        let revision = before.revision();
        let request = |worker, expected, revision| ProtectedIntegrationRequest {
            integration: IntegrationRequest {
                task: &f,
                expected_revision: revision,
                constituents: [
                    IntegrationConstituent {
                        release: &a,
                        worker_path: worker,
                        integrated_path: &left_path,
                    },
                    IntegrationConstituent {
                        release: &b,
                        worker_path: &part,
                        integrated_path: &right_path,
                    },
                ],
                oracle: IntegrationOracle {
                    path: &output,
                    expected,
                },
                max_bytes: LIMIT,
                workspace_observation: ObservationId::new("F", 3).unwrap(),
            },
            profile: fixture::profile(),
            expected_output_mode: MaterialMode::Regular,
        };
        let observation = |suffix: &str, actor| IntegrationObservationRequest {
            operation: OperationId::new(&format!("protected-integration-{suffix}")).unwrap(),
            observation: ObservationId::new(&format!("integration-evidence-{suffix}"), 1).unwrap(),
            review: fixture::grant(1, actor),
        };
        if name != "protected" {
            assert_eq!(before.accepted_material(&a).is_err(), name == "legacy");
            assert!(before.accepted_material(&b).is_ok());
            assert!(matches!(
                observe_protected_integration(
                    &store,
                    &key,
                    request(&part, b"sum:5\n", revision),
                    observation(name, REVIEWER)
                ),
                Err(IntegrationError::Release)
            ));
            assert_eq!(
                store.load(&key, Access::ReadOnly).unwrap().revision(),
                revision
            );
            continue;
        }
        let (_retained, mapping) = before.accepted_material(&a).unwrap();
        assert_eq!(mapping.source.path(), &part);
        assert_eq!(mapping.source.length(), 2);
        assert_eq!(mapping.source.mode(), MaterialMode::Regular);
        // Independent literal SHA-256 of 2 followed by LF.
        assert_eq!(
            mapping.source.sha256(),
            &[
                0x53, 0xc2, 0x34, 0xe5, 0xe8, 0x47, 0x2b, 0x6a, 0xc5, 0x1c, 0x1a, 0xe1, 0xca, 0xb3,
                0xfe, 0x06, 0xfa, 0xd0, 0x53, 0xbe, 0xb8, 0xeb, 0xfd, 0x89, 0x77, 0xb0, 0x10, 0x65,
                0x5b, 0xfd, 0xd3, 0xc3,
            ]
        );
        for (worker, actor, name) in [
            (&substitute, REVIEWER, "unaccepted-same-bytes"),
            (&part, "author-F", "self-review"),
        ] {
            assert!(
                observe_protected_integration(
                    &store,
                    &key,
                    request(worker, b"sum:5\n", revision),
                    observation(name, actor),
                )
                .is_err()
            );
            assert_eq!(
                store.load(&key, Access::ReadOnly).unwrap().revision(),
                revision
            );
        }
        let bad = observe_protected_integration(
            &store,
            &key,
            request(&part, b"sum:4\n", revision),
            observation("wrong-oracle", REVIEWER),
        )
        .unwrap();
        assert!(!bad.satisfied);
        let (forged_pass, _, _) = assess_operation(&store, &key, &bad, REVIEWER, "forged-pass");
        let not_passed_rev = forged_pass.expected;
        assert!(store.admit(&key, forged_pass).is_err());
        let good = observe_protected_integration(
            &store,
            &key,
            request(&part, b"sum:5\n", not_passed_rev),
            observation("actual", REVIEWER),
        )
        .unwrap();
        assert!(good.satisfied);
        let (self_review, _, _) = assess_operation(&store, &key, &good, "author-F", "self-assess");
        assert!(store.admit(&key, self_review).is_err());
        let (pass, witness, _) = assess_operation(&store, &key, &good, REVIEWER, "actual-pass");
        store.admit(&key, pass).unwrap();
        let after_pass = store.load(&key, Access::ReadOnly).unwrap().revision();
        let acceptance = |actor, expected_revision| ProtectedAcceptanceRequest {
            task: &f,
            expected_revision,
            evidence: witness.clone(),
            profile: fixture::profile(),
            candidate_observation: ObservationId::new("F", 5).unwrap(),
            operation: OperationId::new("accept-integrated-candidate").unwrap(),
            review: fixture::grant(1, actor),
        };
        assert!(matches!(
            accept_protected_integration(&store, &key, acceptance(REVIEWER, after_pass - 1)),
            Err(IntegrationError::Context)
        ));
        assert!(matches!(
            accept_protected_integration(&store, &key, acceptance("author-F", after_pass)),
            Err(IntegrationError::Admission { .. })
        ));
        let dirty = integrated.repository().root().join("unreviewed.txt");
        fs::write(&dirty, b"cooperative fixture drift").unwrap();
        assert!(matches!(
            accept_protected_integration(&store, &key, acceptance(REVIEWER, after_pass)),
            Err(IntegrationError::Workspace(_))
        ));
        fs::remove_file(&dirty).unwrap();
        let mut changed_profile = acceptance(REVIEWER, after_pass);
        changed_profile.profile = cortex_evidence::unified::VerificationProfile::new(
            fixture::profile().provenance().clone(),
            "2",
            "synthetic-test",
            &[("fixture", "1")],
        )
        .unwrap();
        assert!(matches!(
            accept_protected_integration(&store, &key, changed_profile),
            Err(IntegrationError::Admission { .. })
        ));
        let (accept, receipt) =
            accept_protected_integration(&store, &key, acceptance(REVIEWER, after_pass)).unwrap();
        drop(store);
        let opened = ProgramStore::open(&path, &key, Access::Recover, limits).unwrap();
        let replayed = opened.load(&key, Access::ReadOnly).unwrap();
        assert_eq!(
            opened
                .reconcile(&key, replayed.initial(), &accept, Access::ReadOnly)
                .unwrap(),
            Reconciliation::Present(cortex_storage::program_store::AdmissionReceipt {
                duplicate: true,
                ..receipt
            })
        );
        let MaterialSubject::Integration {
            scope,
            output: retained,
            ..
        } = replayed.accepted_integration(&accept.id).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(scope.context().contract().candidate(), integrated);
        assert_eq!(retained.path(), &output);
        let withdrawal = record(
            &opened,
            &key,
            "withdraw-integration-assessment",
            REVIEWER,
            Command::InvalidateEvidence {
                observation: witness.observation().clone(),
                reason: "The scoped independent semantic support was withdrawn.".into(),
            },
        );
        opened.admit(&key, withdrawal).unwrap();
        assert!(
            opened
                .load(&key, Access::ReadOnly)
                .unwrap()
                .accepted_integration(&accept.id)
                .is_err()
        );
    }
}
