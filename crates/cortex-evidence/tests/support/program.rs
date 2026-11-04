#![allow(dead_code)]
#[path = "../../../cortex-core/tests/support/program.rs"]
mod core_fixture;
pub use core_fixture::*;
use cortex_core::{binding::*, parallel::*, program::*};
use cortex_evidence::{program::*, readiness::*, unified::*, *};
use std::time::{Duration, UNIX_EPOCH};
pub fn operation(k: &CoordinationKernel, id: &str, command: Command) -> Operation {
    Operation {
        id: OperationId::new(id).unwrap(),
        expected: k.revision(),
        grant: grant(k.state().definition().revision().get(), "operator"),
        command,
    }
}
pub fn admit(k: &mut CoordinationKernel, id: &str, command: Command) {
    k.admit(operation(k, id, command)).unwrap();
}
pub fn initialized() -> CoordinationKernel {
    let mut k = CoordinationKernel::new(definition()).unwrap();
    for m in ["A", "B", "C", "D", "E", "F"] {
        let n = notify(
            k.state(),
            &format!("ready-{m}"),
            Event::Preconditions {
                scope: k.state().readiness().scope(&task(m)).unwrap(),
                ready: true,
                source: source("ready"),
            },
        );
        admit(&mut k, &format!("ready-{m}"), Command::Notify(n));
    }
    let n = notify(
        k.state(),
        "capacity",
        Event::Capacity {
            available: 2,
            source: source("simulation"),
        },
    );
    admit(&mut k, "capacity", Command::Notify(n));
    k
}
pub fn support_limits() -> SupportLimits {
    SupportLimits::new(128, 64, 32, 8_000_000).unwrap()
}
pub fn profile() -> VerificationProfile {
    VerificationProfile::new(
        Provenance::new(
            "original finite fixture verifier",
            "exact candidate and criterion",
            "offline fixture",
        )
        .unwrap(),
        "1",
        "synthetic-test",
        &[("fixture", "1")],
    )
    .unwrap()
}
pub fn prepare_release(k: &mut CoordinationKernel, edge: &str, assessed: bool) -> ReleaseInput {
    let id = EdgeId::new(edge).unwrap();
    let scope = EdgeScopeSnapshot::capture(k.state().readiness(), &id).unwrap();
    let producer = scope.producer().clone();
    let consumer = scope.consumer().clone();
    let artifact = scope.requirement().artifact().clone();
    let requirement = CandidateRequirement::new(
        producer
            .context()
            .contract()
            .candidate()
            .repository()
            .clone(),
        artifact.candidate().clone(),
        WorktreeRequirement::Clean,
    );
    let verification = CandidateBindingContract::new(
        cortex_core::TaskContract::new(
            task(&format!("verify-{edge}")),
            "Verify the scoped artifact interface",
            "Exact complete artifact criterion",
        )
        .unwrap(),
        rev(1),
        requirement.clone(),
    );
    let p = profile();
    let obs = ObservationId::new(&format!("evidence-{edge}"), 1).unwrap();
    let assessment = AssessmentId::new(&format!("assess-{edge}")).unwrap();
    let report = ReportedObservation::new(
        obs.clone(),
        capture(&verification, 1),
        p.clone(),
        ObservationTimes::new(
            UNIX_EPOCH + Duration::from_secs(1),
            UNIX_EPOCH + Duration::from_secs(2),
        )
        .unwrap(),
        ObservationKind::Execution(
            VerificationOutcome::new(
                ExecutionObservation::Exited { code: 0 },
                1,
                Some(
                    OracleEvaluation::new(
                        OracleConclusion::Satisfied,
                        "fixture criterion observed",
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ),
        "Synthetic evidence, not host qualification",
    )
    .unwrap();
    admit(k, &format!("observe-{edge}"), Command::Observe(report));
    let examination = Examination {
        observation: obs.clone(),
        contract: verification.clone(),
        candidate: Some(capture(&verification, 2)),
        profile: Some(p.clone()),
    };
    if assessed {
        admit(
            k,
            &format!("assess-{edge}"),
            Command::Assess(Box::new(AssessmentInput {
                examination: examination.clone(),
                id: assessment.clone(),
                at: UNIX_EPOCH + Duration::from_secs(3),
                result: AssessmentResult::Pass,
                applicability: Applicability::Applicable,
                rationale: "Matches exact declared predicate".into(),
            })),
        );
    }
    let report_id = NotificationId::new(&format!("artifact-{edge}")).unwrap();
    let artifact_report = Notification::new(
        report_id.clone(),
        k.state().definition().graph().reference().clone(),
        Event::Artifact(ArtifactReport::new(
            producer.clone(),
            artifact.clone(),
            scope.requirement().kind(),
            scope.requirement().level(),
            source("artifact-claim"),
        )),
    )
    .unwrap();
    admit(
        k,
        &format!("artifact-{edge}"),
        Command::Notify(artifact_report),
    );
    let contract = EdgeEvidenceContract::new(
        scope.clone(),
        rev(1),
        requirement,
        scope
            .requirement()
            .predicates()
            .iter()
            .map(|predicate| {
                PredicateCorrespondence::new(
                    predicate.clone(),
                    verification.clone(),
                    p.clone(),
                    CriterionCoverage::ExactSingleCriterion,
                )
            })
            .collect(),
        source("exact-complete-correspondence"),
        support_limits(),
    )
    .unwrap();
    let supports = scope
        .requirement()
        .predicates()
        .iter()
        .map(|predicate| {
            PredicateSupport::new(
                predicate.clone(),
                source("observation"),
                source("assessment"),
                source("context"),
            )
        })
        .collect();
    let review = Notification::new(
        NotificationId::new(&format!("review-{edge}")).unwrap(),
        k.state().definition().graph().reference().clone(),
        Event::Review(Box::new(EdgeReview::new(
            id,
            report_id,
            producer.clone(),
            consumer.clone(),
            ReviewRole::Coordinator,
            ReviewAttestation::new(source("reviewer"), source("coordination-approval"), None),
            supports,
        ))),
    )
    .unwrap();
    ReleaseInput {
        contract,
        producer: Some(capture(producer.context().contract(), 2)),
        consumer: Some(capture(consumer.context().contract(), 2)),
        examinations: vec![examination],
        bindings: scope
            .requirement()
            .predicates()
            .iter()
            .map(|predicate| {
                WitnessBinding::new(
                    predicate.clone(),
                    EvidenceReference::new(obs.clone(), Some(assessment.clone())),
                    artifact.clone(),
                )
            })
            .collect(),
        review,
        limits: support_limits(),
    }
}
pub fn release(k: &mut CoordinationKernel, edge: &str) {
    let r = prepare_release(k, edge, true);
    admit(k, &format!("release-{edge}"), Command::Release(Box::new(r)));
}
