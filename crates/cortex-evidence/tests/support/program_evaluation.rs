#![allow(dead_code)]
//! Reusable pure program/evaluation fixtures. No subprocess or filesystem effects.
#[path = "program.rs"]
pub mod fixture;
#[path = "operational.rs"]
mod op;
use cortex_core::{
    binding::*,
    evaluation::*,
    program::{context::*, operational::*, *},
};
use cortex_evidence::{
    ExecutionObservation,
    evaluation::{program::*, *},
    program::{Command, CoordinationKernel},
    unified::*,
};
// Shared by Evidence and CLI integration targets with different fixture subsets.
#[allow(unused_imports)]
pub use fixture::{
    admit, binding, candidate, capture, definition, initialized, operation, prepare_release,
    release, rev, source, task,
};
use std::time::{Duration, UNIX_EPOCH};

pub fn initial_kernel() -> CoordinationKernel {
    let mut kernel = initialized();
    op::effect(
        &mut kernel,
        "evaluation-policy",
        EffectEvent::Policy(op::policy()),
    );
    kernel
}
pub fn specification(kernel: &CoordinationKernel) -> EvaluationSpecification {
    let cases: Vec<_> = kernel
        .state()
        .definition()
        .graph()
        .tasks()
        .iter()
        .map(|task| EvaluationCase {
            id: CaseId::new(task.id().as_str()).unwrap(),
            revision: rev(1),
            contract: task.context().contract().clone(),
            source: SourceIdentity::new(
                task.context().contract().candidate().clone(),
                SourceTreeId::new(&"a".repeat(40)).unwrap(),
            )
            .unwrap(),
            oracle: source(&format!("oracle-{}", task.id().as_str())),
            family: WorkloadFamily::DependencyHeavy,
            access: CaseAccess::Development,
        })
        .collect();
    let config = RunConfiguration {
        reference: source("evaluation-run"),
        profile: ExecutionProfile::Synthetic,
        model: ModelConfiguration::NotApplicable,
        tools: vec![],
        harness: source("synthetic-model"),
        environment: source("synthetic-environment"),
        information: source("explicit-case-population"),
        envelope: op::demand(),
    };
    let arms = vec![
        ComparisonArm {
            strategy: ExecutionStrategy::GovernedParallelEngineering,
            configuration: config.clone(),
        },
        ComparisonArm {
            strategy: ExecutionStrategy::SequentialExecution,
            configuration: RunConfiguration {
                reference: source("planned-run"),
                profile: ExecutionProfile::LiveProvider,
                model: ModelConfiguration::Unestablished,
                ..config
            },
        },
    ];
    let trials = cases
        .iter()
        .flat_map(|case| {
            [
                TrialDefinition {
                    id: TrialId::new(&format!("trial-{}", case.id.as_str())).unwrap(),
                    case: case.id.clone(),
                    strategy: ExecutionStrategy::GovernedParallelEngineering,
                    repetition: 1,
                },
                TrialDefinition {
                    id: TrialId::new(&format!("planned-{}", case.id.as_str())).unwrap(),
                    case: case.id.clone(),
                    strategy: ExecutionStrategy::SequentialExecution,
                    repetition: 1,
                },
            ]
        })
        .collect();
    EvaluationSpecification::new(
        source("whole-program-specification"),
        cases,
        arms,
        trials,
        EvaluationLimits::new(8, 16, 16).unwrap(),
    )
    .unwrap()
}
pub fn oracles(spec: &EvaluationSpecification) -> Vec<OracleContract> {
    spec.cases()
        .iter()
        .map(|case| OracleContract {
            case: case.id.clone(),
            revision: case.revision,
            oracle: case.oracle.clone(),
            profile: fixture::profile(),
        })
        .collect()
}
pub fn mappings(spec: &EvaluationSpecification) -> Vec<ProgramTrial> {
    spec.trials()
        .iter()
        .map(|trial| ProgramTrial {
            trial: trial.id.clone(),
            task: spec
                .cases()
                .iter()
                .find(|case| case.id == trial.case)
                .unwrap()
                .contract
                .task()
                .id()
                .clone(),
            process_profile: None,
        })
        .collect()
}
pub fn declared(kernel: &CoordinationKernel) -> ProgramEvaluation {
    let spec = specification(kernel);
    let trials = mappings(&spec);
    ProgramEvaluation::new(
        kernel,
        EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
        trials,
    )
    .unwrap()
}
pub fn reserve_attempt(
    kernel: &mut CoordinationKernel,
    id: &str,
    task_name: &str,
    predecessor: Option<&str>,
) -> ProgramAttempt {
    let claim = OperationId::new(&format!("claim-{id}")).unwrap();
    let canonical_attempt = fixture::attempt(&format!("canonical-{id}"), task_name, "operator");
    admit(
        kernel,
        claim.as_str(),
        Command::Claim(canonical_attempt.clone()),
    );
    let request = PackageRequest {
        id: PackageId::new(&format!("package-{id}")).unwrap(),
        revision: rev(1),
        task: task(task_name),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Explicit synthetic metadata".into(),
        deliverable: "Observation correspondence".into(),
        stop_rules: "No process, provider or automatic retry".into(),
    };
    let package = OperationId::new(&format!("package-{id}")).unwrap();
    admit(kernel, package.as_str(), Command::Package(request.clone()));
    let scope = EffectScope {
        program: kernel.state().definition().id().clone(),
        baseline: rev(1),
        task: task(task_name),
        candidate: kernel
            .state()
            .readiness()
            .scope(&task(task_name))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: canonical_attempt.id,
        generation: rev(1),
        package: request.id,
        package_revision: rev(1),
    };
    reserve_existing(kernel, id, scope, claim, package, predecessor)
}
pub fn reserve_existing(
    kernel: &mut CoordinationKernel,
    id: &str,
    scope: EffectScope,
    claim: OperationId,
    package: OperationId,
    predecessor: Option<&str>,
) -> ProgramAttempt {
    let mut grant = op::proposed_grant(&scope);
    grant.id = GrantId::new(&format!("grant-{id}")).unwrap();
    op::effect(
        kernel,
        &format!("grant-{id}"),
        EffectEvent::Grant(grant.clone()),
    );
    let effect = EffectId::new(&format!("effect-{id}")).unwrap();
    let reservation = OperationId::new(&format!("reserve-{id}")).unwrap();
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        reservation.as_str(),
        EffectEvent::Reserve {
            intent: EffectIntent {
                id: effect.clone(),
                grant: grant.id,
                scope: scope.clone(),
                demand: op::demand(),
                retry_of: None,
            },
            at,
        },
    );
    ProgramAttempt {
        definition: AttemptDefinition {
            id: cortex_core::evaluation::AttemptId::new(id).unwrap(),
            trial: TrialId::new(&format!("trial-{}", scope.task.as_str())).unwrap(),
            predecessor: predecessor.map(|id| cortex_core::evaluation::AttemptId::new(id).unwrap()),
            executor: source("synthetic-executor"),
            start_ns: None,
        },
        scope,
        claim,
        package,
        reservation,
        effect,
    }
}
pub fn report(kernel: &mut CoordinationKernel, attempt: &ProgramAttempt, outcome: EffectOutcome) {
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        &format!("dispatch-{}", attempt.effect.as_str()),
        EffectEvent::Dispatch {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at,
            basis: source("synthetic-dispatch"),
        },
    );
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        &format!("report-{}", attempt.effect.as_str()),
        EffectEvent::Report {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at,
            outcome,
            basis: source("synthetic-report"),
        },
    );
}
pub fn unknown(kernel: &mut CoordinationKernel, attempt: &ProgramAttempt) {
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        &format!("dispatch-{}", attempt.effect.as_str()),
        EffectEvent::Dispatch {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at,
            basis: source("synthetic-dispatch"),
        },
    );
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        &format!("unknown-{}", attempt.effect.as_str()),
        EffectEvent::Unknown {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at,
            basis: source("synthetic-unknown"),
        },
    );
}
pub fn settle_for_retry(
    kernel: &mut CoordinationKernel,
    attempt: &ProgramAttempt,
    outcome: EffectOutcome,
) {
    let at = kernel.fixture_effects().last_tick() + 1;
    op::effect(
        kernel,
        &format!("reconcile-{}", attempt.effect.as_str()),
        EffectEvent::Reconcile {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at,
            resolution: Reconciliation::Observed {
                outcome,
                usage: op::demand(),
            },
            basis: source("explicit-synthetic-settlement"),
        },
    );
    admit(
        kernel,
        &format!("cancel-{}", attempt.effect.as_str()),
        Command::Attempt {
            id: attempt.scope.attempt.clone(),
            generation: attempt.scope.generation,
            status: AttemptStatus::Cancelled,
            basis: source("separate-explicit-retry-authorization"),
        },
    );
}
pub fn observation(
    evaluation: &ProgramEvaluation,
    attempt: &ProgramAttempt,
    execution: AttemptExecution,
) -> AttemptObservation {
    let spec = evaluation.ledger().specification();
    let trial = spec
        .trials()
        .iter()
        .find(|trial| trial.id == attempt.definition.trial)
        .unwrap();
    let case = spec
        .cases()
        .iter()
        .find(|case| case.id == trial.case)
        .unwrap();
    let arm = spec
        .arms()
        .iter()
        .find(|arm| arm.strategy == trial.strategy)
        .unwrap();
    AttemptObservation {
        id: ObservationId::new(
            &format!("observation-{}", attempt.definition.id.as_str()),
            1,
        )
        .unwrap(),
        attempt: attempt.definition.id.clone(),
        context: ObservationContext {
            specification: spec.reference().clone(),
            case: case.id.clone(),
            case_revision: case.revision,
            source: case.source.clone(),
            oracle: case.oracle.clone(),
            configuration: arm.configuration.clone(),
        },
        origin: ObservationOrigin::Synthetic,
        execution,
        release: None,
        interval: None,
        phases: vec![],
        metrics: MetricObservations::default(),
        endpoint: None,
        basis: source("synthetic-result-model"),
        recorded_at: UNIX_EPOCH + Duration::from_secs(5),
    }
}
pub fn inspect(
    evaluation: &mut ProgramEvaluation,
    kernel: &CoordinationKernel,
) -> ProgramEvaluationReport {
    let spec = evaluation.ledger().specification().clone();
    let oracles = evaluation.ledger().oracles().to_vec();
    evaluation.inspect(kernel, &spec, &oracles).unwrap()
}
pub fn exited(code: u32) -> AttemptExecution {
    AttemptExecution::Observed(ExecutionObservation::Exited { code })
}
pub fn endpoint(evaluation: &ProgramEvaluation, attempt: &ProgramAttempt) -> EndpointEvidence {
    let spec = evaluation.ledger().specification();
    let trial = spec
        .trials()
        .iter()
        .find(|t| t.id == attempt.definition.trial)
        .unwrap();
    let case = spec.cases().iter().find(|c| c.id == trial.case).unwrap();
    let contract = &case.contract;
    let candidate = capture(contract, 2);
    let report = ReportedObservation::new(
        ObservationId::new(&format!("semantic-{}", attempt.definition.id.as_str()), 1).unwrap(),
        capture(contract, 1),
        fixture::profile(),
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
                        "Independent literal synthetic criterion",
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ),
        "Synthetic result, not integrated acceptance",
    )
    .unwrap();
    let mut record = EvidenceRecord::new(report);
    record
        .assess(
            AssessmentId::new(&format!("assessment-{}", attempt.definition.id.as_str())).unwrap(),
            UNIX_EPOCH + Duration::from_secs(3),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "Independent hypothetical criterion",
            &ValidationContext::new(contract, Some(&candidate), Some(&fixture::profile())),
        )
        .unwrap();
    EndpointEvidence {
        source: case.source.clone(),
        record,
        candidate,
        acceptance: Some(AcceptanceClaim {
            assessor: source("separate-author"),
            authority: source("explicit-local-only-acceptance"),
            separation: ReviewerSeparation::SeparateAuthorshipSharedContext,
            scope: ResultScope::Local,
            accepted_elapsed_ns: None,
        }),
    }
}
