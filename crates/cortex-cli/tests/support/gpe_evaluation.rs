//! Cross-candidate guard for one consumer-verification criterion, not a study of
//! whole-program throughput. Operational history retains its separate population.
use super::*;
use cortex_core::evaluation as contract;
use cortex_evidence::evaluation;

pub(super) struct ConsumerEvaluation {
    ledger: evaluation::EvaluationLedger,
    spec: contract::EvaluationSpecification,
    oracles: Vec<evaluation::OracleContract>,
    attempt: contract::AttemptId,
}

impl ConsumerEvaluation {
    /// Register the real consumer's verification attempt before its child starts.
    pub(super) fn new(
        c: &Campaign,
        kernel: &CoordinationKernel,
        scope: &EffectScope,
        effect: &EffectId,
    ) -> Self {
        let (binding, profile) = verification_context(kernel, "B", None);
        let source_identity = contract::SourceIdentity::new(
            binding.candidate().clone(),
            contract::SourceTreeId::new(&git(&c.fixture, &["rev-parse", "HEAD^{tree}"])).unwrap(),
        )
        .unwrap();
        let retained = kernel
            .fixture_effects()
            .effects()
            .iter()
            .find(|record| &record.intent.id == effect)
            .unwrap();
        assert!(&retained.intent.scope == scope);
        assert!(retained.process_observations.is_empty());
        let case = contract::EvaluationCase {
            id: contract::CaseId::new("consumer-verification").unwrap(),
            revision: binding.revision(),
            contract: binding,
            source: source_identity,
            oracle: source("independent-consumer-literal-v1"),
            family: contract::WorkloadFamily::DependencyHeavy,
            access: contract::CaseAccess::Development,
        };
        let arm = contract::ComparisonArm {
            strategy: contract::ExecutionStrategy::GovernedParallelEngineering,
            configuration: contract::RunConfiguration {
                reference: source("connected-consumer-verification"),
                profile: contract::ExecutionProfile::ProviderFreeFixture,
                model: contract::ModelConfiguration::NotApplicable,
                tools: vec![source("pinned-rust-1.99.1")],
                harness: retained.process_spec.as_ref().unwrap().reference.clone(),
                environment: source("trusted-local-linux"),
                information: source("exact-consumer-package-and-retained-release"),
                envelope: retained.intent.demand.clone(),
            },
        };
        let trial = contract::TrialDefinition {
            id: contract::TrialId::new("consumer-verification").unwrap(),
            case: case.id.clone(),
            strategy: arm.strategy,
            repetition: 1,
        };
        let oracles = vec![evaluation::OracleContract {
            case: case.id.clone(),
            revision: case.revision,
            oracle: case.oracle.clone(),
            profile,
        }];
        let spec = contract::EvaluationSpecification::new(
            source("connected-consumer-verification-specification"),
            vec![case],
            vec![arm],
            vec![trial.clone()],
            contract::EvaluationLimits::new(1, 1, 1).unwrap(),
        )
        .unwrap();
        let mut ledger = evaluation::EvaluationLedger::new(spec.clone(), oracles.clone()).unwrap();
        let attempt = contract::AttemptId::new(scope.attempt.as_str()).unwrap();
        ledger
            .register_attempt(evaluation::AttemptDefinition {
                id: attempt.clone(),
                trial: trial.id,
                predecessor: None,
                executor: source("audited-consumer-and-parent-verification"),
                start_ns: None,
            })
            .unwrap();
        let pending = ledger.inspect(&spec, &oracles).unwrap();
        assert_eq!(pending.arms[0].population.incomplete, 1);
        assert!(!pending.primary_outcome_available);
        Self {
            ledger,
            spec,
            oracles,
            attempt,
        }
    }

    /// Actual semantic Evidence cannot accept engineering throughput or change
    /// the operational journal, including its later invalidation and Unknown.
    pub(super) fn finish(mut self, c: &Campaign, store: &ProgramStore, v: &Verification) {
        let before = snapshot(&c.state);
        let kernel = load(c, store);
        let case = &self.spec.cases()[0];
        assert_eq!(&case.contract, &v.contract);
        assert_eq!(&self.oracles[0].profile, &v.profile);
        let record = kernel
            .records()
            .iter()
            .find(|record| record.observation().id() == &v.observation)
            .unwrap()
            .clone();
        let observed = evaluation::AttemptObservation {
            id: ObservationId::new("consumer-evaluation", 1).unwrap(),
            attempt: self.attempt.clone(),
            context: evaluation::ObservationContext {
                specification: self.spec.reference().clone(),
                case: case.id.clone(),
                case_revision: case.revision,
                source: case.source.clone(),
                oracle: case.oracle.clone(),
                configuration: self.spec.arms()[0].configuration.clone(),
            },
            origin: evaluation::ObservationOrigin::DevelopmentFixture,
            execution: evaluation::AttemptExecution::Observed(ExecutionObservation::Exited {
                code: 0,
            }),
            release: None,
            interval: None,
            phases: vec![],
            metrics: evaluation::MetricObservations::default(),
            endpoint: Some(evaluation::EndpointEvidence {
                source: case.source.clone(),
                record,
                candidate: inspect_candidate(
                    &c.fixture,
                    &v.contract,
                    ObservationId::new("consumer-evaluation-source", 1).unwrap(),
                )
                .unwrap(),
                acceptance: None,
            }),
            basis: source("actual-consumer-evidence-no-engineering-acceptance-or-clock"),
            recorded_at: SystemTime::now(),
        };
        let mut wrong = observed.clone();
        wrong.context.oracle = source("changed-consumer-oracle");
        assert!(matches!(
            self.ledger.record(wrong),
            Err(evaluation::EvaluationInputError::Correspondence)
        ));
        let mut wrong = observed.clone();
        wrong.origin = evaluation::ObservationOrigin::Synthetic;
        assert!(matches!(
            self.ledger.record(wrong),
            Err(evaluation::EvaluationInputError::ExecutionProfile)
        ));
        assert!(self.ledger.attempts()[0].observation().is_none());
        self.ledger.record(observed).unwrap();
        let report = self.ledger.inspect(&self.spec, &self.oracles).unwrap();
        assert_eq!(
            report.scope,
            evaluation::MeasurementScope::DevelopmentFixture
        );
        assert_eq!(report.arms[0].population.completed, 1);
        assert_eq!(
            report.eligibility,
            evaluation::ComparisonEligibility::NotComparable
        );
        assert_eq!(
            report.arms[0].trials[0].functional_result,
            Some(AssessmentResult::Pass)
        );
        assert_eq!(report.arms[0].reported_accepted_trials, 0);
        assert_eq!(
            report.arms[0].trials[0].functional_applicability,
            Some(Applicability::Applicable)
        );
        assert_eq!(
            report.arms[0].trials[0].functional_validity,
            Some(Validity::Current)
        );
        assert_eq!(report.arms[0].trials[0].reported_primary_elapsed_ns, None);
        assert!(!report.primary_outcome_available);
        assert_eq!(
            self.ledger.attempts()[0]
                .observation()
                .unwrap()
                .metrics
                .value(evaluation::Metric::CpuTimeNanos),
            None
        );
        let mut output = vec![];
        report.write_redacted(&mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("authority: NONE") && text.contains("improvement claim: NOT_ESTABLISHED")
        );
        c.record("evaluation.txt", &text);
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
        assert!(
            kernel
                .state()
                .readiness()
                .scope(&task("B"))
                .unwrap()
                .generation()
                .get()
                > 1
        );
        assert_eq!(snapshot(&c.state), before);
        assert_eq!(load(c, store).revision(), kernel.revision());
    }
}
