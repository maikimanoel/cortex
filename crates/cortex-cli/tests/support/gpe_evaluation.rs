//! Complete development population; no manufactured task endpoint assessment.
use super::*;
use cortex_core::evaluation as contract;
use cortex_evidence::evaluation::{
    self,
    program::{ProgramAttempt, ProgramEvaluation, ProgramTrial},
};
use std::time::Instant;
pub(super) struct WorkflowEvaluation {
    evaluation: ProgramEvaluation,
    spec: contract::EvaluationSpecification,
    oracles: Vec<evaluation::OracleContract>,
    clock: Instant,
    releases: Vec<(String, evaluation::TrialRelease)>,
    starts: Vec<(String, u64)>,
}
fn trial(label: &str) -> contract::TrialId {
    contract::TrialId::new(&fixture_id(&format!("trial-{label}"))).unwrap()
}
impl WorkflowEvaluation {
    pub(super) fn new(c: &Campaign, kernel: &CoordinationKernel) -> Self {
        let clock = Instant::now();
        let at = SystemTime::now();
        let tree =
            contract::SourceTreeId::new(&git(&c.fixture, &["rev-parse", "HEAD^{tree}"])).unwrap();
        let cases: Vec<_> = ["A", "B"]
            .into_iter()
            .map(|name| {
                let binding = kernel
                    .state()
                    .readiness()
                    .scope(&task(name))
                    .unwrap()
                    .context()
                    .contract()
                    .clone();
                contract::EvaluationCase {
                    id: contract::CaseId::new(&fixture_id(&format!("case-{name}"))).unwrap(),
                    revision: binding.revision(),
                    source: contract::SourceIdentity::new(
                        binding.candidate().clone(),
                        tree.clone(),
                    )
                    .unwrap(),
                    contract: binding,
                    oracle: source(&format!("independent-literal-{name}-v1")),
                    family: contract::WorkloadFamily::DependencyHeavy,
                    access: contract::CaseAccess::Development,
                }
            })
            .collect();
        let oracles = cases
            .iter()
            .zip(["A", "B"])
            .map(|(case, name)| evaluation::OracleContract {
                case: case.id.clone(),
                revision: case.revision,
                oracle: case.oracle.clone(),
                profile: verification_context(kernel, name, None).1,
            })
            .collect::<Vec<_>>();
        let configuration = contract::RunConfiguration {
            reference: source("complete-development-workflow-configuration-v1"),
            profile: contract::ExecutionProfile::ProviderFreeFixture,
            model: contract::ModelConfiguration::NotApplicable,
            tools: vec![source("pinned-rust-1-99-1")],
            harness: source("native-harness-v1"),
            environment: source("trusted-local-linux"),
            information: source("declared-two-task-artifact-workflow"),
            envelope: demand(),
        };
        let mut arms = vec![contract::ComparisonArm {
            strategy: contract::ExecutionStrategy::GovernedParallelEngineering,
            configuration: configuration.clone(),
        }];
        for (strategy, label) in [
            (
                contract::ExecutionStrategy::SequentialExecution,
                "sequential",
            ),
            (
                contract::ExecutionStrategy::DirectParallelExecution,
                "direct-parallel",
            ),
        ] {
            let mut future = configuration.clone();
            future.reference = source(&format!("planned-{label}-unqualified"));
            future.profile = contract::ExecutionProfile::LiveProvider;
            future.model = contract::ModelConfiguration::Unestablished;
            arms.push(contract::ComparisonArm {
                strategy,
                configuration: future,
            });
        }
        let mut trials = Vec::new();
        let mut mappings = Vec::new();
        for (label, name, repetition) in
            [("A", "A", 1), ("B", "B", 1), ("synthetic-unknown", "A", 2)]
        {
            let t = contract::TrialDefinition {
                id: trial(label),
                case: cases[usize::from(name == "B")].id.clone(),
                strategy: arms[0].strategy,
                repetition,
            };
            mappings.push(ProgramTrial {
                trial: t.id.clone(),
                task: task(name),
                process_profile: Some(source("trusted-direct-child-profile")),
            });
            trials.push(t);
        }
        for (i, arm) in arms.iter().enumerate().skip(1) {
            for (case, name) in cases.iter().zip(["A", "B"]) {
                let t = contract::TrialDefinition {
                    id: trial(&format!("planned-{i}-{name}")),
                    case: case.id.clone(),
                    strategy: arm.strategy,
                    repetition: 1,
                };
                mappings.push(ProgramTrial {
                    trial: t.id.clone(),
                    task: task(name),
                    process_profile: None,
                });
                trials.push(t);
            }
        }
        let spec = contract::EvaluationSpecification::new(
            source("complete-development-workflow-v1"),
            cases,
            arms,
            trials,
            contract::EvaluationLimits::new(2, 7, 8).unwrap(),
        )
        .unwrap();
        let ledger = evaluation::EvaluationLedger::new(spec.clone(), oracles.clone()).unwrap();
        let mut evaluation = ProgramEvaluation::new(kernel, ledger, mappings).unwrap();
        let mut releases = Vec::new();
        for label in ["A", "B", "synthetic-unknown"] {
            let release = evaluation::TrialRelease {
                reference: source(&format!("task-release-{label}")),
                clock: source("controller-monotonic-clock-lifetime"),
                at,
                authorization: source("owner-authorized-bounded-development-scenario"),
            };
            evaluation
                .release_trial(&trial(label), release.clone())
                .unwrap();
            releases.push((label.into(), release));
        }
        c.record("evaluation-population.txt","declared_cases=2 declared_trials=7 intended_actual_attempts=3 future_arms_not_run=2; third trial is an explicit synthetic Unknown control, not independent work or automatic retry; no accepted endpoint\n");
        Self {
            evaluation,
            spec,
            oracles,
            clock,
            releases,
            starts: Vec::new(),
        }
    }
    pub(super) fn register(
        &mut self,
        c: &Campaign,
        store: &ProgramStore,
        scope: &EffectScope,
        effect: &EffectId,
        label: &str,
    ) {
        let before = snapshot(&c.state);
        let kernel = load(c, store);
        let start = u64::try_from(self.clock.elapsed().as_nanos()).unwrap();
        self.evaluation
            .register_attempt(
                &kernel,
                ProgramAttempt {
                    definition: evaluation::AttemptDefinition {
                        id: contract::AttemptId::new(scope.attempt.as_str()).unwrap(),
                        trial: trial(label),
                        predecessor: None,
                        executor: source("audited-trusted-fixture-controller"),
                        start_ns: Some(start),
                    },
                    scope: scope.clone(),
                    claim: op(&format!("claim-{label}")),
                    package: op(&format!("package-{label}")),
                    reservation: op(&format!("reserve-{label}")),
                    effect: effect.clone(),
                },
            )
            .unwrap();
        self.starts.push((label.into(), start));
        assert_eq!(snapshot(&c.state), before);
        assert_eq!(load(c, store).revision(), kernel.revision());
    }
    pub(super) fn record(
        &mut self,
        c: &Campaign,
        store: &ProgramStore,
        scope: &EffectScope,
        label: &str,
        execution: evaluation::AttemptExecution,
    ) {
        let before = snapshot(&c.state);
        let kernel = load(c, store);
        let case = &self.spec.cases()[usize::from(scope.task == task("B"))];
        let start = self.starts.iter().find(|(id, _)| id == label).unwrap().1;
        let observed = evaluation::AttemptObservation {
            id: ObservationId::new(&fixture_id(&format!("evaluation-{label}")), 1).unwrap(),
            attempt: contract::AttemptId::new(scope.attempt.as_str()).unwrap(),
            context: evaluation::ObservationContext {
                specification: self.spec.reference().clone(),
                case: case.id.clone(),
                case_revision: case.revision,
                source: case.source.clone(),
                oracle: case.oracle.clone(),
                configuration: self.spec.arms()[0].configuration.clone(),
            },
            origin: evaluation::ObservationOrigin::DevelopmentFixture,
            execution,
            release: Some(
                self.releases
                    .iter()
                    .find(|(id, _)| id == label)
                    .unwrap()
                    .1
                    .clone(),
            ),
            interval: Some(
                evaluation::ElapsedInterval::new(
                    start,
                    u64::try_from(self.clock.elapsed().as_nanos()).unwrap(),
                    evaluation::TimingBasis::MonotonicObservation,
                )
                .unwrap(),
            ),
            phases: vec![],
            metrics: evaluation::MetricObservations::default(),
            endpoint: None,
            basis: source(if label == "synthetic-unknown" {
                "observed-synthetic-unknown-journal-no-child"
            } else {
                "actual-terminal-fixture-process-and-byte-oracle"
            }),
            recorded_at: SystemTime::now(),
        };
        self.evaluation.record(&kernel, observed).unwrap();
        assert_eq!(snapshot(&c.state), before);
        assert_eq!(load(c, store).revision(), kernel.revision());
    }
    pub(super) fn finish(mut self, c: &Campaign, store: &ProgramStore) {
        let before = snapshot(&c.state);
        let kernel = load(c, store);
        let result = self
            .evaluation
            .inspect(&kernel, &self.spec, &self.oracles)
            .unwrap();
        c.record(
            "evaluation-coverage.txt",
            &format!("{:?}\n", result.coverage),
        );
        let report = &result.evaluation;
        assert_eq!(report.arms.len(), 3);
        assert_eq!(report.arms.iter().map(|a| a.trials.len()).sum::<usize>(), 7);
        assert_eq!(report.arms[0].population.registered, 3);
        assert_eq!(report.arms[0].population.completed, 2);
        assert_eq!(report.arms[0].population.unknown, 1);
        assert_eq!(report.arms[0].population.incomplete, 0);
        assert_eq!(report.arms[1].population.registered, 0);
        assert_eq!(report.arms[2].population.registered, 0);
        assert_eq!(report.arms[0].reported_accepted_trials, 0);
        assert!(!report.primary_outcome_available);
        assert_eq!(
            report.eligibility,
            evaluation::ComparisonEligibility::NotComparable
        );
        assert_eq!(
            report.scope,
            evaluation::MeasurementScope::DevelopmentFixture
        );
        assert!(
            !result.coverage.is_empty(),
            "material invalidation remains visible"
        );
        for attempt in self.evaluation.ledger().attempts() {
            assert_eq!(
                attempt
                    .observation()
                    .unwrap()
                    .metrics
                    .value(evaluation::Metric::CpuTimeNanos),
                None
            );
        }
        let mut bytes = Vec::new();
        report.write_redacted(&mut bytes).unwrap();
        c.record("evaluation.txt", &String::from_utf8(bytes).unwrap());
        self.evaluation
            .inspect(&kernel, &self.spec, &self.oracles)
            .unwrap();
        assert_eq!(snapshot(&c.state), before);
        assert_eq!(load(c, store).revision(), kernel.revision());
        assert_eq!(
            kernel.fixture_effects().effects().last().unwrap().status,
            EffectStatus::Unknown
        );
        assert_eq!(
            kernel
                .fixture_effects()
                .effects()
                .last()
                .unwrap()
                .intent
                .demand,
            demand()
        );
    }
}
