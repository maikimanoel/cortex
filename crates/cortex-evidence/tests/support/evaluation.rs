#![allow(dead_code)]
#[path = "../../../cortex-core/tests/support/evaluation.rs"]
mod core_fixture;
pub use core_fixture::*;
use cortex_core::{binding::*, evaluation::*};
use cortex_evidence::{ExecutionObservation, Provenance, evaluation::*, unified::*};
use std::time::{Duration, UNIX_EPOCH};
pub fn profile() -> VerificationProfile {
    VerificationProfile::new(
        Provenance::new(
            "independent literal oracle",
            "exact case and source separately retained",
            "provider-free synthetic contract test",
        )
        .unwrap(),
        "literal-oracle-v1",
        "synthetic-test",
        &[],
    )
    .unwrap()
}
pub fn oracles(spec: &EvaluationSpecification) -> Vec<OracleContract> {
    spec.cases()
        .iter()
        .map(|c| OracleContract {
            case: c.id.clone(),
            revision: c.revision,
            oracle: c.oracle.clone(),
            profile: profile(),
        })
        .collect()
}
pub fn ledger() -> EvaluationLedger {
    let spec = specification();
    EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap()
}
pub fn release(index: usize) -> TrialRelease {
    TrialRelease {
        reference: reference(&format!("release-{index}")),
        clock: reference(&format!("clock-{index}")),
        at: UNIX_EPOCH + Duration::from_secs(1),
        authorization: reference("explicit-synthetic-test-release"),
    }
}
pub fn register(
    ledger: &mut EvaluationLedger,
    index: usize,
    id: &str,
    predecessor: Option<&str>,
    start: Option<u64>,
) {
    let trial = ledger.specification().trials()[index].id.clone();
    if predecessor.is_none() && start.is_some() {
        ledger.release_trial(trial.clone(), release(index)).unwrap();
    }
    ledger
        .register_attempt(AttemptDefinition {
            id: AttemptId::new(id).unwrap(),
            trial,
            predecessor: predecessor.map(|s| AttemptId::new(s).unwrap()),
            executor: reference("evaluated-executor"),
            start_ns: start,
        })
        .unwrap();
}
pub fn endpoint(
    spec: &EvaluationSpecification,
    id: &str,
    execution: &AttemptExecution,
    result: AssessmentResult,
    claim: bool,
) -> EndpointEvidence {
    let case = &spec.cases()[0];
    let contract = case.contract.clone();
    let kind = match execution {
        AttemptExecution::Observed(execution) => {
            let completed = matches!(execution, ExecutionObservation::Exited { .. });
            ObservationKind::Execution(
                VerificationOutcome::new(
                    *execution,
                    u64::from(completed),
                    if completed {
                        Some(
                            OracleEvaluation::new(
                                if result == AssessmentResult::Pass {
                                    OracleConclusion::Satisfied
                                } else {
                                    OracleConclusion::Violated
                                },
                                "Fixed independent expected result",
                            )
                            .unwrap(),
                        )
                    } else {
                        None
                    },
                )
                .unwrap(),
            )
        }
        AttemptExecution::Unknown => ObservationKind::Inference,
        AttemptExecution::NotApplicable => ObservationKind::ScopeInspection(ScopeFinding::Absent),
    };
    let before = capture(&contract, &format!("capture-{id}"), 1);
    let candidate = capture(&contract, &format!("capture-{id}"), 2);
    let report = ReportedObservation::new(
        ObservationId::new(&format!("semantic-{id}"), 1).unwrap(),
        before,
        profile(),
        ObservationTimes::new(
            UNIX_EPOCH + Duration::from_secs(2),
            UNIX_EPOCH + Duration::from_secs(3),
        )
        .unwrap(),
        kind,
        "Explicit synthetic development report; not live execution",
    )
    .unwrap();
    let mut record = EvidenceRecord::new(report);
    record
        .assess(
            AssessmentId::new(&format!("assessment-{id}")).unwrap(),
            UNIX_EPOCH + Duration::from_secs(4),
            result,
            if result == AssessmentResult::NotApplicable {
                Applicability::NotApplicable
            } else {
                Applicability::Applicable
            },
            "Independent fixed test oracle and exact context",
            &ValidationContext::new(&contract, Some(&candidate), Some(&profile())),
        )
        .unwrap();
    EndpointEvidence {
        source: case.source.clone(),
        record,
        candidate,
        acceptance: claim.then(|| AcceptanceClaim {
            assessor: reference("separate-assessor"),
            authority: reference("synthetic-acceptance-claim"),
            separation: ReviewerSeparation::SeparateAuthorshipSharedContext,
            scope: ResultScope::Integrated,
            accepted_elapsed_ns: Some(100),
        }),
    }
}
pub fn observation(
    ledger: &EvaluationLedger,
    index: usize,
    id: &str,
    execution: AttemptExecution,
    result: Option<AssessmentResult>,
) -> AttemptObservation {
    let spec = ledger.specification();
    let trial = &spec.trials()[index];
    let case = &spec.cases()[0];
    let arm = spec
        .arms()
        .iter()
        .find(|a| a.strategy == trial.strategy)
        .unwrap();
    let unperformed = matches!(
        execution,
        AttemptExecution::Observed(
            ExecutionObservation::NotRun | ExecutionObservation::Unavailable
        ) | AttemptExecution::NotApplicable
    );
    AttemptObservation {
        id: ObservationId::new(&format!("observation-{id}"), 1).unwrap(),
        attempt: AttemptId::new(id).unwrap(),
        context: ObservationContext {
            specification: spec.reference().clone(),
            case: case.id.clone(),
            case_revision: case.revision,
            source: case.source.clone(),
            oracle: case.oracle.clone(),
            configuration: arm.configuration.clone(),
        },
        origin: if unperformed {
            ObservationOrigin::Unperformed
        } else {
            ObservationOrigin::Synthetic
        },
        execution: execution.clone(),
        release: (!unperformed).then(|| release(index)),
        interval: (!unperformed)
            .then(|| ElapsedInterval::new(0, 80, TimingBasis::Estimate).unwrap()),
        phases: vec![],
        metrics: MetricObservations::default(),
        endpoint: result.map(|r| endpoint(spec, id, &execution, r, false)),
        basis: reference("synthetic-observation-mechanism"),
        recorded_at: UNIX_EPOCH + Duration::from_secs(5),
    }
}
pub fn inspect(ledger: &mut EvaluationLedger) -> EvaluationReport {
    let spec = ledger.specification().clone();
    let oracles = ledger.oracles().to_vec();
    ledger.inspect(&spec, &oracles).unwrap()
}
