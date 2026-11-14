//! Real Git acquisition and actual monotonic measurement of a finite Rust fixture.
//! No model, external executor, strategy runtime or protected integration is invoked.
mod support;
use cortex_cli::inspect_candidate;
use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, evaluation::*, program::scheduling::*,
    temporal::SourceReference,
};
use cortex_evidence::{ExecutionObservation, Provenance, evaluation::*, unified::*};
use std::{
    path::PathBuf,
    time::{Instant, SystemTime},
};
use support::Repo;
fn reference(name: &str) -> SourceReference {
    SourceReference::operator_reported("trusted-evaluation-fixture-controller", name, "1").unwrap()
}
fn rev() -> ContractRevision {
    ContractRevision::new(1).unwrap()
}
fn text(repo: &Repo, args: &[&str]) -> String {
    String::from_utf8(repo.git(args).stdout)
        .unwrap()
        .trim()
        .into()
}
fn finite_calculation(values: &[u64]) -> u64 {
    values.iter().map(|value| value * value).sum()
}

fn run_fixture() -> (EvaluationReport, Vec<u8>) {
    let repo = Repo::new();
    repo.commit_file("fixture.txt", "input: 2, 3, 5, 7\n");
    let root = repo.path.canonicalize().unwrap();
    let identity = RepositoryIdentity::new(
        root,
        PathBuf::from(text(&repo, &["rev-parse", "--absolute-git-dir"])),
        PathBuf::from(text(
            &repo,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )),
    )
    .unwrap();
    let source = SourceIdentity::new(
        CandidateRequirement::new(
            identity,
            CandidateId::from_git_commit(&text(&repo, &["rev-parse", "HEAD"])).unwrap(),
            WorktreeRequirement::Clean,
        ),
        SourceTreeId::new(&text(&repo, &["rev-parse", "HEAD^{tree}"])).unwrap(),
    )
    .unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("fixture.squared-sum").unwrap(),
            "Check a finite provider-free calculation",
            "The squared sum of 2, 3, 5, 7 is exactly 87",
        )
        .unwrap(),
        rev(),
        source.candidate().clone(),
    );
    let case = EvaluationCase {
        id: CaseId::new("squared-sum").unwrap(),
        revision: rev(),
        contract: contract.clone(),
        source: source.clone(),
        oracle: reference("parent-literal-87"),
        family: WorkloadFamily::Independent,
        access: CaseAccess::Development,
    };
    let strategies = [
        ExecutionStrategy::SequentialExecution,
        ExecutionStrategy::DirectParallelExecution,
        ExecutionStrategy::GovernedParallelEngineering,
    ];
    let arms: Vec<_> = strategies
        .iter()
        .enumerate()
        .map(|(index, strategy)| ComparisonArm {
            strategy: *strategy,
            configuration: RunConfiguration {
                reference: reference(&format!("fixture-arm-{index}")),
                profile: ExecutionProfile::ProviderFreeFixture,
                model: ModelConfiguration::NotApplicable,
                tools: vec![reference("pinned-rust-1.99.1")],
                harness: reference(&format!("declared-future-strategy-{index}")),
                environment: reference("local-development-fixture"),
                information: reference("public-finite-input"),
                envelope: vec![
                    Demand {
                        resource: Resource::WorkerSlots,
                        amount: Some(1),
                    },
                    Demand {
                        resource: Resource::ElapsedMillis,
                        amount: Some(1000),
                    },
                ],
            },
        })
        .collect();
    let trials: Vec<_> = strategies
        .iter()
        .enumerate()
        .map(|(index, strategy)| TrialDefinition {
            id: TrialId::new(&format!("trial-{index}")).unwrap(),
            case: case.id.clone(),
            strategy: *strategy,
            repetition: 1,
        })
        .collect();
    let spec = EvaluationSpecification::new(
        reference("finite-fixture-evaluation"),
        vec![case.clone()],
        arms,
        trials,
        EvaluationLimits::new(1, 3, 3).unwrap(),
    )
    .unwrap();
    let profile=VerificationProfile::new(Provenance::new("trusted parent compares actual calculation against separately fixed literal 87","public finite input; exact real Git source and tree retained separately","in-process fixture, not Codex or GPE runtime; no model; no integrated engineering acceptance").unwrap(),"literal-v1","provider-free-local-fixture",&[("rust-toolchain","1.99.1")]).unwrap();
    let oracles = vec![OracleContract {
        case: case.id.clone(),
        revision: rev(),
        oracle: case.oracle.clone(),
        profile: profile.clone(),
    }];
    let mut ledger = EvaluationLedger::new(spec.clone(), oracles.clone()).unwrap();
    let release = TrialRelease {
        reference: reference("actual-fixture-release"),
        clock: reference("actual-parent-instant-lifetime"),
        at: SystemTime::now(),
        authorization: reference("owner-authorized-provider-free-development-fixture"),
    };
    let clock = Instant::now();
    ledger
        .release_trial(spec.trials()[0].id.clone(), release.clone())
        .unwrap();
    for index in 0..3 {
        let performed = index == 0;
        let id = AttemptId::new(&format!("attempt-{index}")).unwrap();
        let start = performed.then(|| u64::try_from(clock.elapsed().as_nanos()).unwrap());
        ledger
            .register_attempt(AttemptDefinition {
                id: id.clone(),
                trial: spec.trials()[index].id.clone(),
                predecessor: None,
                executor: reference("audited-finite-rust-calculation"),
                start_ns: start,
            })
            .unwrap();
        let before = inspect_candidate(
            &repo.path,
            &contract,
            ObservationId::new(&format!("source-{index}"), 1).unwrap(),
        )
        .unwrap();
        let mut phases = vec![];
        let (execution, result, kind, interval) = if performed {
            let acquired = u64::try_from(clock.elapsed().as_nanos()).unwrap();
            let actual = finite_calculation(&[2, 3, 5, 7]);
            let calculated = u64::try_from(clock.elapsed().as_nanos()).unwrap();
            assert_eq!(actual, 87, "independent fixture oracle rejected output");
            assert_ne!(
                actual, 88,
                "plausible wrong neighbor must not satisfy the oracle"
            );
            let end = u64::try_from(clock.elapsed().as_nanos()).unwrap();
            phases = vec![
                PhaseObservation {
                    phase: EngineeringPhase::Preparation,
                    interval: ElapsedInterval::new(
                        start.unwrap(),
                        acquired,
                        TimingBasis::MonotonicObservation,
                    )
                    .unwrap(),
                },
                PhaseObservation {
                    phase: EngineeringPhase::Implementation,
                    interval: ElapsedInterval::new(
                        acquired,
                        calculated,
                        TimingBasis::MonotonicObservation,
                    )
                    .unwrap(),
                },
                PhaseObservation {
                    phase: EngineeringPhase::Verification,
                    interval: ElapsedInterval::new(
                        calculated,
                        end,
                        TimingBasis::MonotonicObservation,
                    )
                    .unwrap(),
                },
            ];
            (
                ExecutionObservation::Exited { code: 0 },
                AssessmentResult::Pass,
                ObservationKind::Execution(
                    VerificationOutcome::new(
                        ExecutionObservation::Exited { code: 0 },
                        1,
                        Some(
                            OracleEvaluation::new(
                                OracleConclusion::Satisfied,
                                "Actual finite result equals independently established literal 87",
                            )
                            .unwrap(),
                        ),
                    )
                    .unwrap(),
                ),
                Some(
                    ElapsedInterval::new(start.unwrap(), end, TimingBasis::MonotonicObservation)
                        .unwrap(),
                ),
            )
        } else {
            let execution = if index == 1 {
                ExecutionObservation::NotRun
            } else {
                ExecutionObservation::Unavailable
            };
            (
                execution,
                if index == 1 {
                    AssessmentResult::NotRun
                } else {
                    AssessmentResult::Blocked
                },
                ObservationKind::Execution(VerificationOutcome::new(execution, 0, None).unwrap()),
                None,
            )
        };
        let observed = SystemTime::now();
        let recorded = SystemTime::now();
        let report=ReportedObservation::new(ObservationId::new(&format!("semantic-{index}"),1).unwrap(),before,profile.clone(),ObservationTimes::new(observed,recorded).unwrap(),kind,if performed {"Actual finite Rust calculation observed; no strategy implementation or integrated result"} else {"The proposed comparison runtime was not invoked; no qualification or provider permission"}).unwrap();
        let candidate = inspect_candidate(
            &repo.path,
            &contract,
            ObservationId::new(&format!("source-{index}"), 2).unwrap(),
        )
        .unwrap();
        let mut record = EvidenceRecord::new(report);
        record
            .assess(
                AssessmentId::new(&format!("assessment-{index}")).unwrap(),
                SystemTime::now(),
                result,
                Applicability::Applicable,
                "Actual scoped fixture observation or explicit unavailable/unperformed procedure",
                &ValidationContext::new(&contract, Some(&candidate), Some(&profile)),
            )
            .unwrap();
        ledger
            .record(AttemptObservation {
                id: ObservationId::new(&format!("measurement-{index}"), 1).unwrap(),
                attempt: id,
                context: ObservationContext {
                    specification: spec.reference().clone(),
                    case: case.id.clone(),
                    case_revision: rev(),
                    source: source.clone(),
                    oracle: case.oracle.clone(),
                    configuration: spec.arms()[index].configuration.clone(),
                },
                origin: if performed {
                    ObservationOrigin::DevelopmentFixture
                } else {
                    ObservationOrigin::Unperformed
                },
                execution: AttemptExecution::Observed(execution),
                release: performed.then(|| release.clone()),
                interval,
                phases,
                metrics: MetricObservations::default(),
                endpoint: Some(EndpointEvidence {
                    source: source.clone(),
                    record,
                    candidate,
                    acceptance: None,
                }),
                basis: reference("actual-in-process-fixture-capture"),
                recorded_at: SystemTime::now(),
            })
            .unwrap();
    }
    let observation = ledger.attempts()[0].observation().unwrap();
    assert!(observation.interval.is_some());
    assert_eq!(observation.phases.len(), 3);
    assert_eq!(observation.metrics.value(Metric::CpuTimeNanos), None);
    assert_eq!(observation.metrics.value(Metric::InputTokens), None);
    let report = ledger.inspect(&spec, &oracles).unwrap();
    let mut output = vec![];
    report.write_redacted(&mut output).unwrap();
    (report, output)
}
#[test]
fn real_fixture_measurement_preserves_scope_reproduction_and_unavailable_arms() {
    for _ in 0..2 {
        let (report, bytes) = run_fixture();
        assert_eq!(report.scope, MeasurementScope::DevelopmentFixture);
        assert_eq!(report.eligibility, ComparisonEligibility::NotComparable);
        assert!(!report.primary_outcome_available);
        assert_eq!(report.arms.len(), 3);
        assert_eq!(report.arms[0].population.completed, 1);
        assert_eq!(
            report.arms[0].trials[0].functional_result,
            Some(AssessmentResult::Pass)
        );
        assert_eq!(report.arms[0].reported_accepted_trials, 0);
        assert_eq!(report.arms[0].trials[0].reported_primary_elapsed_ns, None);
        assert_eq!(report.arms[1].population.not_run, 1);
        assert_eq!(report.arms[2].population.blocked, 1);
        assert!(
            report
                .arms
                .iter()
                .all(|a| a.trials.len() == 1 && a.population.registered == 1)
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("improvement claim: NOT_ESTABLISHED"));
        assert!(text.contains("blocked=1") && text.contains("not_run=1"));
        assert!(
            !text.contains("trusted-evaluation-fixture-controller")
                && !text.contains("cortex-002a")
        );
    }
}
