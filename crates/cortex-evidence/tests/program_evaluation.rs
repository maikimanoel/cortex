//! Independent whole-population oracles over canonical model fixtures; no child launches.
use model::fixture;
#[path = "support/program_evaluation.rs"]
mod model;
#[path = "support/supervision.rs"]
mod supervised;
use cortex_core::{
    binding::*,
    evaluation::*,
    program::{operational::*, supervision::*, *},
};
use cortex_evidence::{
    ExecutionObservation,
    evaluation::{program::*, *},
    program::{Command, CoordinationKernel},
    unified::*,
};
use model::*;
use std::time::{Duration, UNIX_EPOCH};

fn registered(id: &str, name: &str) -> (CoordinationKernel, ProgramEvaluation, ProgramAttempt) {
    let mut kernel = initial_kernel();
    let mut evaluation = declared(&kernel);
    if name == "B" {
        release(&mut kernel, "AB");
    }
    let attempt = reserve_attempt(&mut kernel, id, name, None);
    evaluation
        .register_attempt(&kernel, attempt.clone())
        .unwrap();
    (kernel, evaluation, attempt)
}
fn gpe(report: &ProgramEvaluationReport) -> &ArmInspection {
    report
        .evaluation
        .arms
        .iter()
        .find(|arm| arm.strategy == ExecutionStrategy::GovernedParallelEngineering)
        .unwrap()
}
fn complete(
    kernel: &mut CoordinationKernel,
    evaluation: &mut ProgramEvaluation,
    attempt: &ProgramAttempt,
    outcome: EffectOutcome,
    execution: AttemptExecution,
) {
    report(kernel, attempt, outcome);
    evaluation
        .record(kernel, observation(evaluation, attempt, execution))
        .unwrap();
}
fn replace_arms(
    spec: &EvaluationSpecification,
    arms: Vec<ComparisonArm>,
) -> EvaluationSpecification {
    EvaluationSpecification::new(
        spec.reference().clone(),
        spec.cases().to_vec(),
        arms,
        spec.trials().to_vec(),
        spec.limits(),
    )
    .unwrap()
}

#[test]
fn successful_producer_preserves_declared_population() {
    let (mut k, mut e, a) = registered("producer", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.registered, 1);
    assert_eq!(gpe(&r).population.completed, 1);
    assert_eq!(gpe(&r).trials.len(), 6);
    assert_eq!(gpe(&r).reported_accepted_trials, 0);
    assert!(!r.evaluation.primary_outcome_available);
}
#[test]
fn failed_producer_remains_in_population() {
    let (mut k, mut e, a) = registered("failed", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Failed, exited(7));
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.failed, 1);
    assert_eq!(gpe(&r).population.completed, 1);
}
#[test]
fn successful_consumer_requires_real_canonical_release() {
    let (mut k, mut e, a) = registered("consumer", "B");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.completed, 1);
    assert_eq!(
        gpe(&r)
            .trials
            .iter()
            .find(|t| t.trial.as_str() == "trial-B")
            .unwrap()
            .attempts,
        1
    );
    assert!(
        k.state()
            .readiness()
            .task(&task("B"))
            .unwrap()
            .dependency_ready()
    );
}
#[test]
fn interrupted_attempt_remains_in_population() {
    let (mut k, mut e, a) = registered("interrupted", "A");
    complete(
        &mut k,
        &mut e,
        &a,
        EffectOutcome::Interrupted,
        AttemptExecution::Observed(ExecutionObservation::Cancelled),
    );
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.interrupted, 1);
    assert_eq!(gpe(&r).population.completed, 0);
}
#[test]
fn unknown_retains_reservation_and_incomplete_success_measure() {
    let (mut k, mut e, a) = registered("unknown", "A");
    unknown(&mut k, &a);
    let retained = k.fixture_effects().clone();
    e.record(&k, observation(&e, &a, AttemptExecution::Unknown))
        .unwrap();
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.unknown, 1);
    assert_eq!(gpe(&r).population.completed, 0);
    assert!(k.fixture_effects() == &retained);
    assert!(!r.evaluation.primary_outcome_available);
}
#[test]
fn separately_authorized_retry_retains_failure_and_one_trial() {
    let (mut k, mut e, a) = registered("failed", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Failed, exited(1));
    settle_for_retry(&mut k, &a, EffectOutcome::Failed);
    let b = reserve_attempt(&mut k, "repair", "A", Some("failed"));
    e.register_attempt(&k, b.clone()).unwrap();
    complete(&mut k, &mut e, &b, EffectOutcome::Succeeded, exited(0));
    let r = inspect(&mut e, &k);
    let arm = gpe(&r);
    assert_eq!(arm.population.registered, 2);
    assert_eq!(arm.population.failed, 1);
    assert_eq!(arm.population.completed, 2);
    assert_eq!(
        arm.trials
            .iter()
            .find(|t| t.trial.as_str() == "trial-A")
            .unwrap()
            .retries,
        1
    );
    assert_eq!(arm.reported_accepted_trials, 0);
}
#[test]
fn planned_arm_is_not_run_and_never_fabricated_as_zero_attempt_cost() {
    let k = initial_kernel();
    let mut e = declared(&k);
    let r = inspect(&mut e, &k);
    let planned = r
        .evaluation
        .arms
        .iter()
        .find(|a| a.strategy == ExecutionStrategy::SequentialExecution)
        .unwrap();
    assert_eq!(planned.trials.len(), 6);
    assert_eq!(planned.population.registered, 0);
    assert!(planned.metrics.iter().all(|metric| metric.total.is_none()));
    assert!(
        r.evaluation
            .issues
            .contains(&ComparisonIssue::MissingObservation)
    );
}
#[test]
fn omitted_producer_is_visible_despite_successful_consumer() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let omitted = reserve_attempt(&mut k, "omitted-producer", "A", None);
    report(&mut k, &omitted, EffectOutcome::Succeeded);
    release(&mut k, "AB");
    let consumer = reserve_attempt(&mut k, "successful-consumer", "B", None);
    e.register_attempt(&k, consumer.clone()).unwrap();
    complete(
        &mut k,
        &mut e,
        &consumer,
        EffectOutcome::Succeeded,
        exited(0),
    );
    let r = inspect(&mut e, &k);
    assert!(r.coverage.contains(&ProgramCoverageIssue::UnmappedAttempt(
        omitted.scope.attempt
    )));
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::UnmappedEffect(omitted.effect))
    );
    assert_eq!(
        r.evaluation.eligibility,
        ComparisonEligibility::NotComparable
    );
    assert!(!r.evaluation.primary_outcome_available);
}
#[test]
fn duplicate_trial_mapping_is_rejected() {
    let k = initial_kernel();
    let spec = specification(&k);
    let mut map = mappings(&spec);
    map[1] = map[0].clone();
    assert!(matches!(
        ProgramEvaluation::new(
            &k,
            EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
            map
        ),
        Err(ProgramEvaluationError::Population)
    ));
}
#[test]
fn missing_trial_mapping_is_rejected() {
    let k = initial_kernel();
    let spec = specification(&k);
    let mut map = mappings(&spec);
    map.pop();
    assert!(matches!(
        ProgramEvaluation::new(
            &k,
            EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
            map
        ),
        Err(ProgramEvaluationError::Population)
    ));
}
#[test]
fn changed_oracle_retains_invalidation_without_relabeling_results() {
    let (mut k, mut e, a) = registered("producer", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    let original = e.ledger().attempts()[0].observation().unwrap().clone();
    let spec = e.ledger().specification().clone();
    let mut changed = e.ledger().oracles().to_vec();
    changed[0].profile = VerificationProfile::new(
        fixture::profile().provenance().clone(),
        "changed",
        "synthetic-test",
        &[],
    )
    .unwrap();
    let r = e.inspect(&k, &spec, &changed).unwrap();
    assert_eq!(
        r.evaluation.eligibility,
        ComparisonEligibility::NotComparable
    );
    assert!(e.ledger().first_invalidation().is_some());
    assert_eq!(e.ledger().attempts()[0].observation(), Some(&original));
}
#[test]
fn changed_source_and_report_configuration_are_rejected() {
    let (mut k, mut e, a) = registered("producer", "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    for source_change in [true, false] {
        let mut o = observation(&e, &a, exited(0));
        if source_change {
            o.context.source = SourceIdentity::new(
                CandidateRequirement::new(
                    o.context.source.candidate().repository().clone(),
                    candidate("f"),
                    o.context.source.candidate().worktree(),
                ),
                SourceTreeId::new(&"f".repeat(40)).unwrap(),
            )
            .unwrap();
        } else {
            o.context.configuration.environment = source("different-environment");
        }
        assert!(matches!(
            e.record(&k, o),
            Err(ProgramEvaluationError::Evaluation(
                EvaluationInputError::Correspondence
            ))
        ));
    }
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn foreign_program_cannot_supply_same_labels() {
    let (mut k, mut e, a) = registered("producer", "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let d = k.initial();
    let foreign = ProgramDefinition::new(
        ProgramId::new("foreign-program").unwrap(),
        d.outcome().clone(),
        d.revision(),
        d.predecessor(),
        d.approval().clone(),
        d.graph().clone(),
        d.workstreams().to_vec(),
        d.increments().to_vec(),
        d.limits(),
    )
    .unwrap();
    let other = CoordinationKernel::new(foreign).unwrap();
    assert!(matches!(
        e.record(&other, observation(&e, &a, exited(0))),
        Err(ProgramEvaluationError::Identity)
    ));
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn missing_metric_and_measured_zero_remain_distinct() {
    let (mut k, mut e, a) = registered("producer", "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    o.metrics = MetricObservations::new(&[(Metric::CpuTimeNanos, Some(0))], None).unwrap();
    e.record(&k, o).unwrap();
    let observed = e.ledger().attempts()[0].observation().unwrap();
    assert_eq!(observed.metrics.value(Metric::CpuTimeNanos), Some(0));
    assert_eq!(observed.metrics.value(Metric::DiskBytesWritten), None);
    let r = inspect(&mut e, &k);
    assert!(gpe(&r).metrics.iter().all(|metric| metric.total.is_none()));
}
#[test]
fn incompatible_clock_provenance_is_rejected() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let release = TrialRelease {
        reference: source("release-A"),
        clock: source("clock-one"),
        at: UNIX_EPOCH + Duration::from_secs(1),
        authorization: source("explicit-test-release"),
    };
    e.release_trial(&TrialId::new("trial-A").unwrap(), release.clone())
        .unwrap();
    let mut a = reserve_attempt(&mut k, "producer", "A", None);
    a.definition.start_ns = Some(0);
    e.register_attempt(&k, a.clone()).unwrap();
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    o.interval = Some(ElapsedInterval::new(0, 10, TimingBasis::Estimate).unwrap());
    o.release = Some(TrialRelease {
        clock: source("clock-two"),
        ..release
    });
    assert!(matches!(
        e.record(&k, o),
        Err(ProgramEvaluationError::Evaluation(
            EvaluationInputError::Timing
        ))
    ));
}
#[test]
fn eager_retry_without_predecessor_observation_is_rejected() {
    let (mut k, mut e, a) = registered("first", "A");
    report(&mut k, &a, EffectOutcome::Failed);
    settle_for_retry(&mut k, &a, EffectOutcome::Failed);
    let retry = reserve_attempt(&mut k, "second", "A", Some("first"));
    assert!(matches!(
        e.register_attempt(&k, retry),
        Err(ProgramEvaluationError::Evaluation(
            EvaluationInputError::Identity
        ))
    ));
    assert_eq!(e.ledger().attempts().len(), 1);
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn registration_does_not_dispatch_or_authorize_effects() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = reserve_attempt(&mut k, "producer", "A", None);
    let before = k.operations().to_vec();
    let projection = k.fixture_effects().clone();
    e.register_attempt(&k, a).unwrap();
    assert!(k.operations() == before);
    assert!(k.fixture_effects() == &projection);
    assert_eq!(
        k.fixture_effects().effects()[0].status,
        EffectStatus::Reserved
    );
    assert!(
        k.fixture_effects().effects()[0]
            .process_observations
            .is_empty()
    );
}
#[test]
fn unknown_cannot_be_recorded_as_success() {
    let (mut k, mut e, a) = registered("unknown", "A");
    unknown(&mut k, &a);
    assert!(matches!(
        e.record(&k, observation(&e, &a, exited(0))),
        Err(ProgramEvaluationError::Outcome)
    ));
    assert!(e.ledger().attempts()[0].observation().is_none());
    assert_eq!(
        k.fixture_effects().effects()[0].status,
        EffectStatus::Unknown
    );
}
#[test]
fn repeated_successful_attempt_is_not_additional_accepted_work() {
    let (mut k, mut e, a) = registered("first", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    settle_for_retry(&mut k, &a, EffectOutcome::Succeeded);
    let retry = reserve_attempt(&mut k, "second", "A", Some("first"));
    e.register_attempt(&k, retry.clone()).unwrap();
    complete(&mut k, &mut e, &retry, EffectOutcome::Succeeded, exited(0));
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.completed, 2);
    assert_eq!(gpe(&r).reported_accepted_trials, 0);
    assert_eq!(
        gpe(&r)
            .trials
            .iter()
            .filter(|trial| trial.attempts > 0)
            .count(),
        1
    );
}
#[test]
fn synthetic_report_cannot_be_presented_as_live_or_development_execution() {
    let (mut k, mut e, a) = registered("producer", "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    o.context.configuration.profile = ExecutionProfile::LiveProvider;
    assert!(e.record(&k, o).is_err());
    let mut o = observation(&e, &a, exited(0));
    o.origin = ObservationOrigin::DevelopmentFixture;
    assert!(matches!(
        e.record(&k, o),
        Err(ProgramEvaluationError::Correspondence)
    ));
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn local_endpoint_cannot_claim_integrated_acceptance() {
    let (mut k, mut e, a) = registered("producer", "A");
    report(&mut k, &a, EffectOutcome::Succeeded);
    let mut o = observation(&e, &a, exited(0));
    let mut endpoint = endpoint(&e, &a);
    endpoint.acceptance.as_mut().unwrap().scope = ResultScope::Integrated;
    o.endpoint = Some(endpoint);
    assert!(matches!(
        e.record(&k, o),
        Err(ProgramEvaluationError::Outcome)
    ));
    assert!(e.ledger().attempts()[0].observation().is_none());
}
#[test]
fn inspection_mutates_only_advisory_invalidation_not_canonical_history() {
    let (mut k, mut e, a) = registered("producer", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    let before = k.operations().to_vec();
    let effects = k.fixture_effects().clone();
    let revision = k.revision();
    let spec = e.ledger().specification().clone();
    let mut changed = spec.cases().to_vec();
    changed[0].source = SourceIdentity::new(
        changed[0].source.candidate().clone(),
        SourceTreeId::new(&"b".repeat(40)).unwrap(),
    )
    .unwrap();
    let current = EvaluationSpecification::new(
        spec.reference().clone(),
        changed,
        spec.arms().to_vec(),
        spec.trials().to_vec(),
        spec.limits(),
    )
    .unwrap();
    e.inspect(&k, &current, &oracles(&current)).unwrap();
    assert!(e.ledger().first_invalidation().is_some());
    assert_eq!(k.revision(), revision);
    assert!(k.operations() == before);
    assert!(k.fixture_effects() == &effects);
    assert_eq!(e.ledger().attempts().len(), 1);
}
#[test]
fn complete_declared_population_and_missing_attempts_are_present() {
    let (mut k, mut e, a) = registered("producer", "A");
    complete(&mut k, &mut e, &a, EffectOutcome::Succeeded, exited(0));
    let b = reserve_attempt(&mut k, "open-neighbor", "D", None);
    e.register_attempt(&k, b).unwrap();
    let r = inspect(&mut e, &k);
    assert_eq!(
        r.evaluation
            .arms
            .iter()
            .map(|arm| arm.trials.len())
            .sum::<usize>(),
        12
    );
    assert_eq!(gpe(&r).population.registered, 2);
    assert_eq!(gpe(&r).population.incomplete, 1);
    assert_eq!(gpe(&r).population.completed, 1);
}
#[test]
fn predeclaration_after_canonical_attempt_is_rejected() {
    let mut k = initial_kernel();
    let spec = specification(&k);
    reserve_attempt(&mut k, "already-started", "A", None);
    assert!(matches!(
        ProgramEvaluation::new(
            &k,
            EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
            mappings(&spec)
        ),
        Err(ProgramEvaluationError::AlreadyStarted)
    ));
}
#[test]
fn registration_after_dispatch_is_rejected() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let a = reserve_attempt(&mut k, "late", "A", None);
    report(&mut k, &a, EffectOutcome::Succeeded);
    assert!(matches!(
        e.register_attempt(&k, a),
        Err(ProgramEvaluationError::AlreadyStarted)
    ));
    assert!(e.ledger().attempts().is_empty());
}
#[test]
fn duplicate_canonical_attempt_cannot_be_relabelled() {
    let (k, mut e, a) = registered("producer", "A");
    let mut duplicate = a;
    duplicate.definition.id =
        cortex_core::evaluation::AttemptId::new("new-evaluation-label").unwrap();
    assert!(matches!(
        e.register_attempt(&k, duplicate),
        Err(ProgramEvaluationError::Identity)
    ));
    assert_eq!(e.ledger().attempts().len(), 1);
}
#[test]
fn missing_or_foreign_operation_witness_is_rejected() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let mut a = reserve_attempt(&mut k, "producer", "A", None);
    a.reservation = OperationId::new("absent-reservation").unwrap();
    assert!(matches!(
        e.register_attempt(&k, a),
        Err(ProgramEvaluationError::Identity)
    ));
    assert!(e.ledger().attempts().is_empty());
}
#[test]
fn foreign_task_correspondence_is_rejected() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let mut a = reserve_attempt(&mut k, "producer", "A", None);
    a.scope.task = task("D");
    assert!(matches!(
        e.register_attempt(&k, a),
        Err(ProgramEvaluationError::Identity)
    ));
    assert!(e.ledger().attempts().is_empty());
}
#[test]
fn forked_prior_witness_blocks_next_registration() {
    let (k, mut e, _a) = registered("first", "A");
    let mut fork = initial_kernel();
    let mut op = operation(
        &fork,
        "different-history",
        Command::Claim(fixture::attempt("other", "A", "operator")),
    );
    op.grant.basis = source("different-grant");
    op.grant.id = GrantId::new("different-grant").unwrap();
    fork.admit(op).unwrap();
    let b = reserve_attempt(&mut fork, "neighbor", "D", None);
    assert!(matches!(
        e.register_attempt(&fork, b),
        Err(ProgramEvaluationError::Correspondence)
    ));
    assert_eq!(e.ledger().attempts().len(), 1);
    assert!(k.revision() > 0);
}
#[test]
fn historical_unknown_records_after_generation_invalidation() {
    let mut k = initial_kernel();
    let mut e = declared(&k);
    let r = prepare_release(&mut k, "AB", true);
    let mut changed = r.examinations[0].clone();
    admit(&mut k, "release-AB", Command::Release(Box::new(r)));
    let a = reserve_attempt(&mut k, "consumer", "B", None);
    e.register_attempt(&k, a.clone()).unwrap();
    unknown(&mut k, &a);
    changed.profile = Some(
        VerificationProfile::new(
            fixture::profile().provenance().clone(),
            "changed-profile",
            "synthetic-test",
            &[],
        )
        .unwrap(),
    );
    admit(&mut k, "invalidate-support", Command::Examine(changed));
    let before = k.operations().to_vec();
    e.record(&k, observation(&e, &a, AttemptExecution::Unknown))
        .unwrap();
    let r = inspect(&mut e, &k);
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::StaleGeneration(a.scope.attempt))
    );
    assert_eq!(gpe(&r).population.unknown, 1);
    assert_eq!(
        r.evaluation.eligibility,
        ComparisonEligibility::NotComparable
    );
    assert!(k.operations() == before);
}
#[test]
fn declaration_profile_must_match_canonical_prepared_process() {
    for valid in [false, true] {
        let (configured, scope, process) = supervised::configured_kernel(None);
        let mut kernel =
            CoordinationKernel::new_supervised_fixture(configured.initial().clone()).unwrap();
        let spec = specification(&kernel);
        let mut arms = spec.arms().to_vec();
        for arm in &mut arms {
            arm.configuration.envelope = supervised::demand();
        }
        let spec = replace_arms(&spec, arms);
        let mut map = mappings(&spec);
        for item in &mut map {
            item.process_profile = Some(if valid {
                process.profile.clone()
            } else {
                source("foreign-prepared-profile")
            });
        }
        let mut e = ProgramEvaluation::new(
            &kernel,
            EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
            map,
        )
        .unwrap();
        for operation in configured.operations() {
            kernel.admit(operation.clone()).unwrap();
        }
        supervised::prepare(&mut kernel, &scope, &process);
        let a = ProgramAttempt {
            definition: AttemptDefinition {
                id: cortex_core::evaluation::AttemptId::new("prepared").unwrap(),
                trial: TrialId::new("trial-A").unwrap(),
                predecessor: None,
                executor: source("synthetic-executor"),
                start_ns: None,
            },
            scope: scope.clone(),
            claim: OperationId::new("claim-A").unwrap(),
            package: OperationId::new("package-A").unwrap(),
            reservation: OperationId::new("reserve").unwrap(),
            effect: supervised::intent(&scope).id,
        };
        let result = e.register_attempt(&kernel, a);
        if valid {
            result.unwrap();
            assert_eq!(e.ledger().attempts().len(), 1);
        } else {
            assert!(matches!(
                result,
                Err(ProgramEvaluationError::Correspondence)
            ));
            assert!(e.ledger().attempts().is_empty());
        }
    }
}

// Pure canonical models of captured process facts; these never spawn a process.
fn captured_attempt() -> (
    CoordinationKernel,
    ProgramEvaluation,
    ProgramAttempt,
    ProcessSpec,
) {
    let (configured, scope, process) = supervised::configured_kernel(None);
    let mut kernel =
        CoordinationKernel::new_supervised_fixture(configured.initial().clone()).unwrap();
    let spec = specification(&kernel);
    let mut arms = spec.arms().to_vec();
    for arm in &mut arms {
        arm.configuration.envelope = supervised::demand();
        arm.configuration.harness = source("captured-fixture-observation");
    }
    let spec = replace_arms(&spec, arms);
    let mut map = mappings(&spec);
    for item in &mut map {
        item.process_profile = Some(process.profile.clone());
    }
    let mut evaluation = ProgramEvaluation::new(
        &kernel,
        EvaluationLedger::new(spec.clone(), oracles(&spec)).unwrap(),
        map,
    )
    .unwrap();
    for operation in configured.operations() {
        kernel.admit(operation.clone()).unwrap();
    }
    supervised::prepare(&mut kernel, &scope, &process);
    let attempt = ProgramAttempt {
        definition: AttemptDefinition {
            id: cortex_core::evaluation::AttemptId::new("captured").unwrap(),
            trial: TrialId::new("trial-A").unwrap(),
            predecessor: None,
            executor: source("synthetic-executor"),
            start_ns: None,
        },
        scope: scope.clone(),
        claim: OperationId::new("claim-A").unwrap(),
        package: OperationId::new("package-A").unwrap(),
        reservation: OperationId::new("reserve").unwrap(),
        effect: supervised::intent(&scope).id,
    };
    evaluation
        .register_attempt(&kernel, attempt.clone())
        .unwrap();
    let dispatch = supervised::dispatch_operation(&kernel, &scope);
    kernel.admit(dispatch).unwrap();
    (kernel, evaluation, attempt, process)
}
fn capture_terminal(
    kernel: &mut CoordinationKernel,
    attempt: &ProgramAttempt,
    process: &ProcessSpec,
    completion: ProcessCompletion,
    malformed: bool,
) {
    let mut captured = supervised::observation(
        process,
        4,
        completion,
        Quiescence::DirectChildReapedStreamsClosed,
    );
    captured.malformed_output = malformed;
    supervised::effect(
        kernel,
        "terminal-capture",
        EffectEvent::ProcessObserved {
            effect: attempt.effect.clone(),
            scope: attempt.scope.clone(),
            at: 4,
            observation: captured,
        },
    );
}
#[test]
fn captured_cancellation_cannot_be_relabelled_as_timeout() {
    let (mut k, mut e, a, process) = captured_attempt();
    capture_terminal(
        &mut k,
        &a,
        &process,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Cancelled,
        },
        false,
    );
    assert!(matches!(
        e.record(
            &k,
            observation(
                &e,
                &a,
                AttemptExecution::Observed(ExecutionObservation::TimedOut)
            )
        ),
        Err(ProgramEvaluationError::Correspondence)
    ));
    e.record(
        &k,
        observation(
            &e,
            &a,
            AttemptExecution::Observed(ExecutionObservation::Cancelled),
        ),
    )
    .unwrap();
    assert_eq!(gpe(&inspect(&mut e, &k)).population.interrupted, 1);
}
#[test]
fn captured_timeout_cannot_be_relabelled_as_cancellation() {
    let (mut k, mut e, a, process) = captured_attempt();
    capture_terminal(
        &mut k,
        &a,
        &process,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Timeout,
        },
        false,
    );
    assert!(matches!(
        e.record(
            &k,
            observation(
                &e,
                &a,
                AttemptExecution::Observed(ExecutionObservation::Cancelled)
            )
        ),
        Err(ProgramEvaluationError::Correspondence)
    ));
    e.record(
        &k,
        observation(
            &e,
            &a,
            AttemptExecution::Observed(ExecutionObservation::TimedOut),
        ),
    )
    .unwrap();
}
#[test]
fn zero_exit_with_known_mechanism_failure_remains_explicitly_unrepresented() {
    let (mut k, mut e, a, process) = captured_attempt();
    capture_terminal(
        &mut k,
        &a,
        &process,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        true,
    );
    assert_eq!(
        k.fixture_effects().effects()[0].report,
        Some(EffectOutcome::Failed)
    );
    assert!(matches!(
        e.record(&k, observation(&e, &a, exited(0))),
        Err(ProgramEvaluationError::Outcome)
    ));
    let r = inspect(&mut e, &k);
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::UnrepresentedOutcome {
                attempt: a.scope.attempt,
                outcome: EffectOutcome::Failed
            })
    );
    assert_eq!(gpe(&r).population.incomplete, 1);
    assert_eq!(gpe(&r).population.completed, 0);
    assert_eq!(
        r.evaluation.eligibility,
        ComparisonEligibility::NotComparable
    );
}
#[test]
fn unsupported_captured_interruption_remains_visible_without_invented_cause() {
    let (mut k, mut e, a, process) = captured_attempt();
    capture_terminal(
        &mut k,
        &a,
        &process,
        ProcessCompletion::Interrupted {
            reason: InterruptionReason::Io,
        },
        false,
    );
    assert!(matches!(
        e.record(
            &k,
            observation(
                &e,
                &a,
                AttemptExecution::Observed(ExecutionObservation::Cancelled)
            )
        ),
        Err(ProgramEvaluationError::Correspondence)
    ));
    let r = inspect(&mut e, &k);
    assert!(
        r.coverage
            .contains(&ProgramCoverageIssue::UnrepresentedOutcome {
                attempt: a.scope.attempt,
                outcome: EffectOutcome::Interrupted
            })
    );
    assert_eq!(gpe(&r).population.incomplete, 1);
    assert!(!r.evaluation.primary_outcome_available);
}
#[test]
fn explicit_model_reconciliation_can_supply_previously_unreported_outcome() {
    let (mut k, mut e, a) = registered("reconciled", "A");
    unknown(&mut k, &a);
    settle_for_retry(&mut k, &a, EffectOutcome::Failed);
    let effect = &k.fixture_effects().effects()[0];
    assert_eq!(effect.status, EffectStatus::ReconciledReported);
    assert!(effect.report.is_none());
    e.record(&k, observation(&e, &a, exited(7))).unwrap();
    assert_eq!(gpe(&inspect(&mut e, &k)).population.failed, 1);
}
#[test]
fn later_model_reconciliation_cannot_rewrite_original_unknown_observation() {
    let (mut k, mut e, a) = registered("retained", "A");
    unknown(&mut k, &a);
    e.record(&k, observation(&e, &a, AttemptExecution::Unknown))
        .unwrap();
    let original = e.ledger().attempts()[0].observation().cloned().unwrap();
    settle_for_retry(&mut k, &a, EffectOutcome::Failed);
    assert!(e.record(&k, observation(&e, &a, exited(7))).is_err());
    assert!(e.ledger().attempts()[0].observation() == Some(&original));
    assert_eq!(gpe(&inspect(&mut e, &k)).population.unknown, 1);
}

#[test]
fn captured_exit_with_unknown_cleanup_remains_unknown() {
    let (mut k, mut e, a, process) = captured_attempt();
    let captured = supervised::observation(
        &process,
        4,
        ProcessCompletion::Exited {
            code: Some(0),
            signal: None,
        },
        Quiescence::Unknown,
    );
    supervised::effect(
        &mut k,
        "uncertain-cleanup",
        EffectEvent::ProcessObserved {
            effect: a.effect.clone(),
            scope: a.scope.clone(),
            at: 4,
            observation: captured,
        },
    );
    assert_eq!(
        k.fixture_effects().effects()[0].status,
        EffectStatus::Unknown
    );
    e.record(&k, observation(&e, &a, AttemptExecution::Unknown))
        .unwrap();
    let r = inspect(&mut e, &k);
    assert_eq!(gpe(&r).population.unknown, 1);
    assert!(
        !r.coverage
            .iter()
            .any(|i| matches!(i, ProgramCoverageIssue::UnrepresentedOutcome { .. }))
    );
}
